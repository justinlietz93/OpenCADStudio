//! Shared annotative-object detection + annotation-scale resolution.
//!
//! Both the Properties panel (Annotative row / applied scale name) and the
//! tessellation bake (which scales annotative content by the current annotation
//! scale) must agree on *which* entities are annotative — so that logic lives
//! here, once. An entity is annotative if it carries a per-object annotation
//! context, the legacy annotative XDATA, or an annotative style.

use acadrust::entities::{EntityCommon, EntityType};
use acadrust::objects::{Dictionary, ObjectType};
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

/// Whether an object carries a per-object annotation context — its extension
/// dictionary holds an `AcDbContextDataManager`. This catches objects that are
/// annotative by context even when their style is not.
fn has_context_manager(doc: &CadDocument, common: &EntityCommon) -> bool {
    common
        .xdictionary_handle
        .and_then(|h| as_dict(doc, h))
        .map(|d| {
            d.entries
                .iter()
                .any(|(k, _)| k.eq_ignore_ascii_case("AcDbContextDataManager"))
        })
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

