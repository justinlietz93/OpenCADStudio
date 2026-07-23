//! Shared annotative-object detection + annotation-scale resolution.
//!
//! Both the Properties panel (Annotative row / applied scale name) and the
//! tessellation bake (which scales annotative content by the current annotation
//! scale) must agree on *which* entities are annotative — so that logic lives
//! here, once. An entity is annotative if it carries a per-object annotation
//! context, the legacy annotative XDATA, or an annotative style.

use acadrust::entities::{EntityCommon, EntityType};
use acadrust::objects::{
    Dictionary, MTextContext, ObjectContextData, ObjectContextKind, ObjectType,
};
use acadrust::types::{Vector2, Vector3};
use acadrust::{CadDocument, Handle};

/// Resolve a handle to a `Dictionary` object, if it is one.
pub fn as_dict(doc: &CadDocument, handle: Handle) -> Option<&Dictionary> {
    match doc.objects.get(&handle) {
        Some(ObjectType::Dictionary(d)) => Some(d),
        _ => None,
    }
}

/// Resolve the drawing's root named-objects dictionary, creating one if the
/// file has none reachable.
///
/// The canonical pointer is `header.named_objects_dict_handle`, but DWGs
/// written by some programs leave it dangling — pointing at a handle that never
/// loaded, or at a non-dictionary — while the real named-object sub-dictionaries
/// (`ACAD_LAYOUT`, `ACAD_SCALELIST`, …) are instead owned by an unrelated handle
/// that is not a dictionary. When the pointer can't be resolved we adopt any
/// top-level (`owner == NULL`) dictionary as the root; failing that we synthesise
/// a fresh, empty root so that *registering* a new named-object entry (a page
/// setup, an annotation scale, the `CTAB` variable) actually persists instead of
/// silently no-opping against a missing dictionary.
///
/// Idempotent: the resolved or created handle is written back to the header, so
/// later calls return the same dictionary rather than minting another root. On a
/// well-formed drawing this returns the existing root untouched.
pub fn root_named_dict_handle(doc: &mut CadDocument) -> Handle {
    let h = doc.header.named_objects_dict_handle;
    if matches!(doc.objects.get(&h), Some(ObjectType::Dictionary(_))) {
        return h;
    }
    // A top-level dictionary is already present (the standard root shape) — adopt
    // the richest one (matching the DWG writer's own root heuristic) and repair
    // the stale header pointer.
    if let Some(root) = doc
        .objects
        .iter()
        .filter_map(|(k, o)| match o {
            ObjectType::Dictionary(d) if d.owner.is_null() => Some((*k, d.entries.len())),
            _ => None,
        })
        .max_by_key(|&(_, n)| n)
        .map(|(k, _)| k)
    {
        doc.header.named_objects_dict_handle = root;
        return root;
    }
    // Nothing reachable — build a fresh, empty root named-objects dictionary.
    let nh = doc.allocate_handle();
    let mut d = Dictionary::new();
    d.handle = nh;
    d.owner = Handle::NULL;
    doc.objects.insert(nh, ObjectType::Dictionary(d));
    doc.header.named_objects_dict_handle = nh;
    nh
}

/// Set the per-object annotative flag on the entity types that carry one
/// (MTEXT, MULTILEADER). Turning it off also strips the per-object annotation
/// context and legacy markers via [`clear_annotation_context`] so the object
/// stops resolving annotative; turning it on leaves the base geometry as the
/// single (implicit, current-scale) representation. Other entity types get
/// their annotative state from a style and are not toggled here.
pub fn set_entity_annotative(doc: &mut CadDocument, handle: Handle, want: bool) {
    if let Some(e) = doc.get_entity_mut(handle) {
        match e {
            EntityType::MText(t) => t.is_annotative = want,
            EntityType::MultiLeader(m) => m.enable_annotation_scale = want,
            _ => {}
        }
    }
    if !want {
        clear_annotation_context(doc, handle);
    }
}

/// Derive the per-scale context payload for an entity from its current
/// placement. Returns the concrete class name and the context kind, or `None`
/// for entity types that do not carry a per-object annotation context (their
/// annotative state comes from a style, e.g. DIMENSION/TABLE).
fn context_kind_for(entity: &EntityType) -> Option<(&'static str, ObjectContextKind)> {
    match entity {
        EntityType::Insert(ins) => Some((
            "ACDB_BLKREFOBJECTCONTEXTDATA_CLASS",
            ObjectContextKind::BlkRef {
                rotation: ins.rotation,
                insertion: ins.insert_point,
                scale_factor: Vector3::new(ins.x_scale(), ins.y_scale(), ins.z_scale()),
            },
        )),
        EntityType::Text(t) => Some((
            "ACDB_TEXTOBJECTCONTEXTDATA_CLASS",
            ObjectContextKind::Text {
                horizontal_mode: t.horizontal_alignment as i16,
                rotation: t.rotation,
                insertion: Vector2::new(t.insertion_point.x, t.insertion_point.y),
                alignment: t
                    .alignment_point
                    .map(|p| Vector2::new(p.x, p.y))
                    .unwrap_or(Vector2::new(0.0, 0.0)),
            },
        )),
        EntityType::MText(m) => Some((
            "ACDB_MTEXTOBJECTCONTEXTDATA_CLASS",
            ObjectContextKind::MText(MTextContext {
                attachment: m.attachment_point as i32,
                // MTEXT stores a text X-axis direction; derive it from rotation.
                x_axis_dir: Vector3::new(m.rotation.cos(), m.rotation.sin(), 0.0),
                insertion: m.insertion_point,
                rect_width: m.rectangle_width,
                rect_height: 0.0,
                extents_width: 0.0,
                extents_height: 0.0,
                column_type: 0,
                columns: None,
            }),
        )),
        _ => None,
    }
}

/// Give an entity a per-object annotation context for `scale_handle`,
/// synthesizing the extension-dictionary chain it hangs from when absent:
///
/// ```text
/// entity xdict → "AcDbContextDataManager" → "ACDB_ANNOTATIONSCALES" → "*An" → leaf
/// ```
///
/// The leaf is an [`ObjectContextData`] whose placement is copied from the
/// entity's current geometry and whose `340` handle references `scale_handle`
/// (an `AcDbScale` in `ACAD_SCALELIST`). Idempotent: a leaf for that scale is
/// not duplicated. Exactly one leaf per object is marked `is_default` (the
/// first one created — the native representation). Returns `false` for entity
/// kinds that carry no per-object context (their annotative-ness is style-driven).
pub fn create_annotation_context(
    doc: &mut CadDocument,
    entity_handle: Handle,
    scale_handle: Handle,
) -> bool {
    let Some((class_name, kind)) = doc.get_entity(entity_handle).and_then(context_kind_for) else {
        return false;
    };
    // The writer emits a 500+ class number only for registered classes.
    doc.register_object_context_class(class_name);

    // Extension dictionary (hard-owns its entries; 280 = 1). Create it if the
    // entity has none, and point the entity at it.
    let xdict_h = match doc
        .get_entity(entity_handle)
        .and_then(|e| e.common().xdictionary_handle)
    {
        Some(h) if as_dict(doc, h).is_some() => h,
        _ => {
            let h = doc.allocate_handle();
            let mut d = Dictionary::new();
            d.handle = h;
            d.owner = entity_handle;
            d.hard_owner = true;
            doc.objects.insert(h, ObjectType::Dictionary(d));
            if let Some(e) = doc.get_entity_mut(entity_handle) {
                e.common_mut().xdictionary_handle = Some(h);
            }
            h
        }
    };

    let mgr_h = get_or_create_child_dict(doc, xdict_h, "AcDbContextDataManager");
    let coll_h = get_or_create_child_dict(doc, mgr_h, "ACDB_ANNOTATIONSCALES");

    // Idempotent: if a leaf already applies to this scale, keep it.
    let existing = as_dict(doc, coll_h)
        .map(|d| {
            d.entries.iter().any(|(_, lh)| {
                matches!(
                    doc.objects.get(lh),
                    Some(ObjectType::ObjectContextData(c)) if c.scale == scale_handle
                )
            })
        })
        .unwrap_or(false);
    if existing {
        return true;
    }

    // The first representation created is the default (native) one.
    let is_default = as_dict(doc, coll_h).map(|d| d.entries.is_empty()).unwrap_or(true);
    let n = as_dict(doc, coll_h).map(|d| d.entries.len()).unwrap_or(0) + 1;
    let key = format!("*A{n}");

    let leaf_h = doc.allocate_handle();
    let leaf = ObjectContextData {
        handle: leaf_h,
        owner_handle: coll_h,
        reactors: vec![coll_h],
        xdictionary_handle: None,
        class_version: 3,
        is_default,
        scale: scale_handle,
        kind,
        source_raw: None,
        source_handle_bits: 0,
        source_version: None,
    };
    doc.objects
        .insert(leaf_h, ObjectType::ObjectContextData(leaf));
    if let Some(ObjectType::Dictionary(coll)) = doc.objects.get_mut(&coll_h) {
        coll.add_entry(key, leaf_h);
    }
    true
}

/// True when `common` belongs to an annotative object whose per-object scale
/// contexts are *all* bound to scales other than the current one — an
/// off-scale representation that must stay hidden. A drawing may materialise
/// each annotation scale as its own object (e.g. a block insert per scale on a
/// "0 @ <scale>" layer); only the current scale's representation should draw.
///
/// False for non-annotative objects (no per-object context — the vast
/// majority) and for objects whose contexts include the current scale. Gated
/// on an extension dictionary so non-annotative entities skip the lookup.
pub fn annotative_offscale(doc: &CadDocument, common: &EntityCommon) -> bool {
    if !common
        .xdictionary_handle
        .map(|h| !h.is_null())
        .unwrap_or(false)
    {
        return false;
    }
    let scales = object_scale_memberships(doc, common.handle);
    if scales.is_empty() {
        return false;
    }
    let cur = &doc.header.current_annotation_scale;
    if scales.iter().any(|(name, _)| name.eq_ignore_ascii_case(cur)) {
        return false;
    }
    // Off-scale (no context for the current scale). If some representation in
    // the drawing DOES provide the current scale, hide this one — the matching
    // representation is the one to show.
    if current_scale_provided(doc) {
        return true;
    }
    // The current scale is unsupported by any representation. Fall back to the
    // base "1:1" representation: keep it, hide the enlarged copies — otherwise
    // every scale representation stacks (or, if all were hidden, the object
    // vanishes). Without this, opening at e.g. CANNOSCALE 10:1 shows both a 1×
    // and a 10× copy of the same block.
    !scales.iter().any(|(name, _)| name.eq_ignore_ascii_case("1:1"))
}

/// Whether any annotative representation in the drawing targets the current
/// annotation scale.
fn current_scale_provided(doc: &CadDocument) -> bool {
    let cur = &doc.header.current_annotation_scale;
    doc.objects.values().any(|o| {
        if let ObjectType::ObjectContextData(cd) = o {
            if let Some(ObjectType::Scale(s)) = doc.objects.get(&cd.scale) {
                return s.name.eq_ignore_ascii_case(cur);
            }
        }
        false
    })
}

/// The annotation scales an object currently carries a per-object context for,
/// as `(scale name, scale handle)` pairs (one per representation). Empty when
/// the object has no per-object context chain.
pub fn object_scale_memberships(doc: &CadDocument, entity: Handle) -> Vec<(String, Handle)> {
    let mut out = Vec::new();
    let Some(coll_h) = annotation_scales_dict(doc, entity) else {
        return out;
    };
    if let Some(coll) = as_dict(doc, coll_h) {
        for (_, lh) in &coll.entries {
            if let Some(ObjectType::ObjectContextData(leaf)) = doc.objects.get(lh) {
                if let Some(ObjectType::Scale(s)) = doc.objects.get(&leaf.scale) {
                    out.push((s.name.clone(), leaf.scale));
                }
            }
        }
    }
    out
}

/// Remove the per-object context representation that applies to `scale_handle`,
/// keeping the object's other representations. When that was the object's last
/// representation the whole context chain (and the annotative markers) are torn
/// down via [`clear_annotation_context`] so the object becomes non-annotative.
/// Returns `true` if a representation was removed.
pub fn remove_annotation_context_for_scale(
    doc: &mut CadDocument,
    entity: Handle,
    scale_handle: Handle,
) -> bool {
    let Some(coll_h) = annotation_scales_dict(doc, entity) else {
        return false;
    };
    // Find the leaf that applies to this scale.
    let leaf = as_dict(doc, coll_h).and_then(|c| {
        c.entries.iter().find_map(|(_, lh)| {
            matches!(
                doc.objects.get(lh),
                Some(ObjectType::ObjectContextData(o)) if o.scale == scale_handle
            )
            .then_some(*lh)
        })
    });
    let Some(leaf_h) = leaf else {
        return false;
    };
    // If this is the object's only representation, fully de-annotate it (drop the
    // whole chain AND the native flag, like the Yes→No toggle) so it stops
    // resolving annotative; otherwise drop just this leaf.
    let last = as_dict(doc, coll_h).map(|c| c.entries.len() <= 1).unwrap_or(true);
    if last {
        set_entity_annotative(doc, entity, false);
        return true;
    }
    doc.objects.remove(&leaf_h);
    if let Some(ObjectType::Dictionary(coll)) = doc.objects.get_mut(&coll_h) {
        coll.entries.retain(|(_, h)| *h != leaf_h);
    }
    true
}

/// Resolve an entity's `ACDB_ANNOTATIONSCALES` collection dictionary handle, if
/// its context chain exists.
fn annotation_scales_dict(doc: &CadDocument, entity: Handle) -> Option<Handle> {
    let xd = doc.get_entity(entity).and_then(|e| e.common().xdictionary_handle)?;
    let mgr = as_dict(doc, xd).and_then(|d| d.get("AcDbContextDataManager"))?;
    as_dict(doc, mgr).and_then(|d| d.get("ACDB_ANNOTATIONSCALES"))
}

/// Get the child dictionary stored under `key` in `parent_h`, creating an empty
/// one (owned by `parent_h`) and registering the entry when absent.
fn get_or_create_child_dict(doc: &mut CadDocument, parent_h: Handle, key: &str) -> Handle {
    if let Some(h) = as_dict(doc, parent_h).and_then(|d| d.get(key)) {
        return h;
    }
    let h = doc.allocate_handle();
    let mut d = Dictionary::new();
    d.handle = h;
    d.owner = parent_h;
    doc.objects.insert(h, ObjectType::Dictionary(d));
    if let Some(ObjectType::Dictionary(p)) = doc.objects.get_mut(&parent_h) {
        p.add_entry(key, h);
    }
    h
}

/// Remove an entity's per-object annotation context — the extension-dictionary
/// `AcDbContextDataManager` → `ACDB_ANNOTATIONSCALES` → per-scale leaf subtree —
/// and the legacy annotative XDATA markers, so [`is_annotative`] no longer fires
/// on it. The shared `SCALE` objects in `ACAD_SCALELIST` are document-level and
/// left intact.
pub fn clear_annotation_context(doc: &mut CadDocument, handle: Handle) {
    if let Some(xdict_h) = doc.get_entity(handle).and_then(|e| e.common().xdictionary_handle) {
        // Collect the manager subtree (manager dict, its scales dict, the leaves)
        // before mutating, then drop them.
        let mut remove = Vec::new();
        if let Some(mgr_h) = as_dict(doc, xdict_h).and_then(|d| d.get("AcDbContextDataManager")) {
            remove.push(mgr_h);
            if let Some(scales_h) =
                as_dict(doc, mgr_h).and_then(|d| d.get("ACDB_ANNOTATIONSCALES"))
            {
                remove.push(scales_h);
                if let Some(scales) = as_dict(doc, scales_h) {
                    for (_, leaf) in &scales.entries {
                        remove.push(*leaf);
                    }
                }
            }
        }
        if let Some(ObjectType::Dictionary(xd)) = doc.objects.get_mut(&xdict_h) {
            xd.entries.retain(|(k, _)| k != "AcDbContextDataManager");
        }
        for h in remove {
            doc.objects.remove(&h);
        }
    }
    // Strip the legacy annotative XDATA markers the detection also honours.
    crate::scene::view::dispatch::set_entity_xdata(doc, handle, "AcAnnoPO", None);
    crate::scene::view::dispatch::set_entity_xdata(doc, handle, "AcAnnotativeData", None);
}

/// Does a style name resolve to `name` (or to "Standard" when `name` is blank)?
fn name_matches(style_name: &str, name: &str) -> bool {
    style_name.eq_ignore_ascii_case(name)
        || (name.trim().is_empty() && style_name.eq_ignore_ascii_case("Standard"))
}

fn text_style_annotative(doc: &CadDocument, name: &str) -> bool {
    doc.text_styles
        .iter()
        .find(|s| name_matches(&s.name, name))
        .is_some_and(|s| s.annotative)
}

fn dim_style_annotative(doc: &CadDocument, name: &str) -> bool {
    doc.dim_styles
        .iter()
        .find(|s| name_matches(&s.name, name))
        .is_some_and(|s| s.annotative)
}

fn mleader_style_annotative(doc: &CadDocument, handle: Option<Handle>) -> bool {
    let Some(h) = handle else {
        return false;
    };
    doc.objects.iter().any(|(oh, o)| {
        matches!(o, ObjectType::MultiLeaderStyle(s) if *oh == h && s.is_annotative)
    })
}

fn table_style_annotative(doc: &CadDocument, handle: Option<Handle>) -> bool {
    let Some(h) = handle else {
        return false;
    };
    doc.objects
        .iter()
        .any(|(oh, o)| matches!(o, ObjectType::TableStyle(s) if *oh == h && s.annotative))
}

/// Whether an object carries a per-object annotation context with at least one
/// per-scale representation — its extension dictionary holds an
/// `AcDbContextDataManager` whose `ACDB_ANNOTATIONSCALES` collection is
/// non-empty. This catches objects that are annotative by context even when
/// their style is not.
///
/// The non-empty requirement matters: a context manager with an *empty*
/// `ACDB_ANNOTATIONSCALES` is a single-representation marker with no per-scale
/// reps (common in files where objects were flagged annotative but never given
/// a scale). Such an object has nothing to scale *to*, so it must render at its
/// base geometry — treating it as annotative would (mis)scale it by the
/// annotation factor in annotation-scaled viewports, ballooning the text.
fn has_context_manager(doc: &CadDocument, common: &EntityCommon) -> bool {
    let key = |d: &Dictionary, name: &str| {
        d.entries
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, h)| *h)
    };
    let Some(xd) = common.xdictionary_handle.and_then(|h| as_dict(doc, h)) else {
        return false;
    };
    let Some(mgr) = key(xd, "AcDbContextDataManager").and_then(|h| as_dict(doc, h)) else {
        return false;
    };
    key(mgr, "ACDB_ANNOTATIONSCALES")
        .and_then(|h| as_dict(doc, h))
        .map(|coll| !coll.entries.is_empty())
        .unwrap_or(false)
}

/// Whether an entity participates in annotation scaling.
pub fn is_annotative(doc: &CadDocument, entity: &EntityType) -> bool {
    // Per-object annotation context (works regardless of style).
    if has_context_manager(doc, entity.common()) {
        return true;
    }
    // Legacy annotative XDATA markers.
    let xd = &entity.common().extended_data;
    if xd.get_record("AcAnnoPO").is_some() || xd.get_record("AcAnnotativeData").is_some() {
        return true;
    }
    // Annotative via the assigned style (or the entity's own flag).
    match entity {
        EntityType::Text(t) => text_style_annotative(doc, &t.style),
        EntityType::MText(t) => t.is_annotative || text_style_annotative(doc, &t.style),
        EntityType::Dimension(d) => dim_style_annotative(doc, &d.base().style_name),
        EntityType::Leader(l) => dim_style_annotative(doc, &l.dimension_style),
        EntityType::MultiLeader(ml) => {
            ml.enable_annotation_scale || mleader_style_annotative(doc, ml.style_handle)
        }
        EntityType::Table(t) => table_style_annotative(doc, t.table_style_handle),
        _ => false,
    }
}

