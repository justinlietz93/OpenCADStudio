// XREF resolution — scan a loaded document for external-reference blocks and
// populate them with geometry from the referenced DWG/DXF files.

use acadrust::entities::{Block, BlockEnd};
use acadrust::tables::TableEntry;
use acadrust::types::{Handle, Vector3};
use acadrust::{CadDocument, EntityType};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Status of an external reference block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XrefStatus {
    /// File was found and loaded successfully.
    Loaded,
    /// File path is set but the file could not be found or read.
    NotFound,
    /// XRef is marked Unloaded in the host DWG — we honor that and
    /// skip resolving the external file. The user can re-load via UI.
    Unloaded,
}

/// Describes a single external reference found in a document.
#[derive(Debug, Clone)]
pub struct XrefInfo {
    /// Block name (e.g. the filename stem).
    pub name: String,
    /// Resolved file path (or raw path if not found).
    pub path: String,
    pub status: XrefStatus,
}

/// Scan `doc` for XREF block-records, resolve their paths relative to
/// `base_dir`, and populate each xref block with entities from the
/// referenced file.
///
/// Returns a list of [`XrefInfo`] describing each xref block found.
pub fn resolve_xrefs(doc: &mut CadDocument, base_dir: &Path) -> Vec<XrefInfo> {
    // Collect xref blocks: (name, raw_path, block_record_handle)
    //
    // Note: `BlockRecord::is_loaded` (the DWG R2000+ "loaded" bit) turned out
    // to be unreliable as a signal of AutoCAD's Unload state — different
    // files with xrefs the manager calls "Loaded" carry both `Some(true)`
    // and `Some(false)` on disk. Until we identify the actual on-disk
    // representation of XREF→Unload (likely in an XREF dependency record
    // or ACAD_XREF_PANEL_DATA xrecord), we auto-resolve every xref and
    // rely on frustum culling to keep the GPU bounded.
    let xref_entries: Vec<(String, String, Handle)> = doc
        .block_records
        .iter()
        .filter(|br| (br.flags.is_xref || br.flags.is_xref_overlay) && !br.xref_path.is_empty())
        .map(|br| (br.name.clone(), br.xref_path.clone(), br.handle))
        .collect();

    let mut result = Vec::new();

    for (block_name, raw_path, br_handle) in xref_entries {
        let resolved = resolve_path(&raw_path, base_dir);

        let status = match &resolved {
            None => XrefStatus::NotFound,
            Some(p) => match super::load_file(p) {
                Err(_) => XrefStatus::NotFound,
                Ok(xref_doc) => {
                    ensure_block_entities(doc, &block_name);
                    merge_xref_into_block(doc, &block_name, br_handle, xref_doc);
                    XrefStatus::Loaded
                }
            },
        };

        result.push(XrefInfo {
            name: block_name,
            path: resolved
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or(raw_path),
            status,
        });
    }

    result
}

/// Try to build an absolute path from a raw xref path string.
/// Handles absolute paths, relative paths, and Windows-style separators.
fn resolve_path(raw: &str, base_dir: &Path) -> Option<PathBuf> {
    let normalised = raw.replace('\\', "/");
    let p = PathBuf::from(&normalised);

    if p.is_absolute() {
        if p.exists() {
            return Some(p);
        }
        // Fallback: try the filename in base_dir.
        if let Some(fname) = p.file_name() {
            let c = base_dir.join(fname);
            if c.exists() {
                return Some(c);
            }
        }
        return None;
    }

    // Relative path against base_dir.
    let candidate = base_dir.join(&p);
    if candidate.exists() {
        return Some(candidate);
    }

    // Last resort: just the filename.
    if let Some(fname) = p.file_name() {
        let c = base_dir.join(fname);
        if c.exists() {
            return Some(c);
        }
    }

    None
}

/// Make sure `doc` has BLOCK + ENDBLK entities for `block_name`.
/// These are required so renderers can find the block content.
fn ensure_block_entities(doc: &mut CadDocument, block_name: &str) {
    let has_block = doc
        .entities()
        .any(|e| matches!(e, EntityType::Block(b) if b.name == block_name));
    if has_block {
        return;
    }
    let b = Block::new(block_name, Vector3::zero());
    let _ = doc.add_entity(EntityType::Block(b));
    let _ = doc.add_entity(EntityType::BlockEnd(BlockEnd::new()));
}

/// Merge an external-reference document into `doc`'s xref block.
///
/// Copies the xref's model-space entities into the host xref block, AND
/// (crucially for correct rendering) merges the xref's layer / linetype
/// tables and any *nested* block records into the host doc under prefixed
/// names ("{xref_name}|{symbol_name}"). Without this remapping, every
/// xref entity using ByLayer resolves against the host doc's layer table
/// — which doesn't know about the xref's layers — and silently falls
/// back to WHITE / 1 px line weight. AutoCAD's BIND command uses the
/// same naming scheme.
fn merge_xref_into_block(
    doc: &mut CadDocument,
    xref_block_name: &str,
    br_handle: Handle,
    xref_doc: CadDocument,
) {
    let prefix = xref_block_name;

    // ── Layers ──────────────────────────────────────────────────────────
    // Prefix every xref layer (including "0"). Entity layer references are
    // remapped below so the resolver finds the merged copy. Host layers
    // (incl. its own "0") are untouched — no collisions.
    let mut layer_map: HashMap<String, String> = HashMap::new();
    for layer in xref_doc.layers.iter() {
        let old = layer.name.clone();
        let new = format!("{}|{}", prefix, old);
        let mut cloned = layer.clone();
        cloned.name = new.clone();
        cloned.set_handle(doc.allocate_handle());
        doc.layers.add_or_replace(cloned);
        layer_map.insert(old.to_uppercase(), new);
    }

    // ── Linetypes ───────────────────────────────────────────────────────
    // Skip the three sentinel names — "ByLayer" / "ByBlock" / "Continuous"
    // are magic strings the resolver matches verbatim in both docs.
    let mut linetype_map: HashMap<String, String> = HashMap::new();
    for lt in xref_doc.line_types.iter() {
        let old = lt.name.clone();
        if is_sentinel_linetype(&old) {
            continue;
        }
        let new = format!("{}|{}", prefix, old);
        let mut cloned = lt.clone();
        cloned.name = new.clone();
        cloned.set_handle(doc.allocate_handle());
        doc.line_types.add_or_replace(cloned);
        linetype_map.insert(old.to_uppercase(), new);
    }

    // ── Block records (nested blocks inside the xref) ───────────────────
    // First pass: create each prefixed BR with its host handle + empty
    // entity_handles. Second pass (in the entity loop below) populates
    // them by inserting entities with `owner_handle` set.
    //
    // Tracks (xref-doc BR handle → host BR handle) so entities owned by
    // a nested xref block can be routed to the right host block_record.
    let mut br_handle_map: HashMap<Handle, Handle> = HashMap::new();
    let mut block_name_map: HashMap<String, String> = HashMap::new();
    for br in xref_doc.block_records.iter() {
        // Skip layout block records (*Model_Space, *Paper_Space, *Paper_Space0…)
        // and any further-nested xrefs (we don't recurse into xref-of-xref).
        if br.name.starts_with('*') || br.flags.is_xref || br.flags.is_xref_overlay {
            continue;
        }
        let old = br.name.clone();
        let new = format!("{}|{}", prefix, old);
        let mut cloned = br.clone();
        cloned.name = new.clone();
        cloned.entity_handles.clear();
        cloned.insert_handles.clear();
        // Detach foreign layout pointer — it refers to a Layout in xref_doc
        // we don't import.
        cloned.layout = Handle::NULL;
        let new_h = doc.allocate_handle();
        cloned.set_handle(new_h);
        cloned.block_entity_handle = doc.allocate_handle();
        cloned.block_end_handle = doc.allocate_handle();
        br_handle_map.insert(br.handle, new_h);
        block_name_map.insert(old.to_uppercase(), new);
        doc.block_records.add_or_replace(cloned);
    }

    // ── Entities ────────────────────────────────────────────────────────
    let xref_ms_handle = xref_doc.header.model_space_block_handle;
    let entities: Vec<EntityType> = xref_doc
        .entities()
        .filter(|e| !matches!(e, EntityType::Block(_) | EntityType::BlockEnd(_)))
        .cloned()
        .collect();

    for mut entity in entities {
        // Remap layer / linetype names so the host's resolver hits the
        // copies we just inserted.
        {
            let c = entity.common_mut();
            if let Some(new_layer) = layer_map.get(&c.layer.to_uppercase()) {
                c.layer = new_layer.clone();
            }
            if !is_sentinel_linetype(&c.linetype) {
                if let Some(new_lt) = linetype_map.get(&c.linetype.to_uppercase()) {
                    c.linetype = new_lt.clone();
                }
            }
        }

        // INSERTs reference blocks by name, not handle — remap.
        if let EntityType::Insert(ins) = &mut entity {
            if let Some(new_name) = block_name_map.get(&ins.block_name.to_uppercase()) {
                ins.block_name = new_name.clone();
            }
        }

        // Route entity to the correct host block record. Entities owned by
        // the xref's model_space land in the host xref block (br_handle);
        // entities owned by one of the nested BRs land in its prefixed
        // counterpart. Paper-space / unknown owners are skipped.
        let old_owner = entity.common().owner_handle;
        let new_owner = if old_owner == xref_ms_handle {
            br_handle
        } else if let Some(&h) = br_handle_map.get(&old_owner) {
            h
        } else {
            continue;
        };
        entity.common_mut().owner_handle = new_owner;
        // Clear the foreign handle so acadrust assigns a new one.
        set_handle(&mut entity, Handle::NULL);
        let _ = doc.add_entity(entity);
    }
}

fn is_sentinel_linetype(name: &str) -> bool {
    name.eq_ignore_ascii_case("ByLayer")
        || name.eq_ignore_ascii_case("ByBlock")
        || name.eq_ignore_ascii_case("Continuous")
}

/// Set the handle field of any entity variant.
fn set_handle(entity: &mut EntityType, h: Handle) {
    entity.common_mut().handle = h;
}
