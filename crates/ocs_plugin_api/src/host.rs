//! Runtime host surface (`host` feature).
//!
//! [`HostApi`] is the `acadrust`-typed adapter a plugin uses at *dispatch* time
//! — document access, entity creation, XDATA, undo, and the command line. It is
//! the stable counterpart to the dependency-free manifest/ribbon contract: a
//! plugin's `dispatch` receives `&mut dyn HostApi` rather than the host's
//! concrete session type, so an out-of-tree add-on compiles against this crate
//! alone.
//!
//! Per-tab plugin state is keyed by `manifest.id`. The trait exposes it in an
//! object-safe `Any` form; use the [`plugin_state`], [`plugin_state_mut`] and
//! [`ensure_plugin_state`] helpers for the ergonomic typed access.

use std::any::Any;

use acadrust::xdata::ExtendedDataRecord;
use acadrust::{CadDocument, EntityType, Handle};

/// The plugin-facing runtime surface for one active document tab.
pub trait HostApi {
    /// Index of the tab this session targets.
    fn tab_index(&self) -> usize;

    // ── Document ────────────────────────────────────────────────────────────
    fn document(&self) -> &CadDocument;
    fn document_mut(&mut self) -> &mut CadDocument;
    /// Add an entity to the active document, returning its handle.
    fn add_entity(&mut self, entity: EntityType) -> Handle;
    /// Mark the scene geometry dirty so it is re-tessellated next frame.
    fn bump_geometry(&mut self);

    // ── XDATA ───────────────────────────────────────────────────────────────
    /// Read the XDATA record for `app_name` on entity `handle`, if any.
    fn read_record(&self, handle: Handle, app_name: &str) -> Option<&ExtendedDataRecord>;
    /// Attach `record` to entity `handle`, replacing any existing record for the
    /// same application and registering the APPID. Returns `false` if the entity
    /// does not exist.
    fn write_record(&mut self, handle: Handle, record: ExtendedDataRecord) -> bool;
    /// Remove the XDATA record for `app_name` from entity `handle`. Returns
    /// `true` if a record was removed.
    fn remove_record(&mut self, handle: Handle, app_name: &str) -> bool;

    // ── Undo / dirty ────────────────────────────────────────────────────────
    fn push_undo(&mut self, label: &str);
    fn set_dirty(&mut self);

    // ── Command line ────────────────────────────────────────────────────────
    fn push_info(&mut self, msg: &str);
    fn push_output(&mut self, msg: &str);
    fn push_error(&mut self, msg: &str);

    // ── Per-tab plugin state (object-safe; use the typed helpers below) ──────
    fn plugin_state_any(&self, plugin_id: &str) -> Option<&(dyn Any + Send + Sync)>;
    fn plugin_state_any_mut(&mut self, plugin_id: &str)
        -> Option<&mut (dyn Any + Send + Sync)>;
    /// Get the state for `plugin_id`, inserting `init()`'s result if absent.
    fn ensure_plugin_state_any(
        &mut self,
        plugin_id: &'static str,
        init: &mut dyn FnMut() -> Box<dyn Any + Send + Sync>,
    ) -> &mut (dyn Any + Send + Sync);
}

/// Typed read of per-tab plugin state stored under `plugin_id`.
pub fn plugin_state<'a, T: Any + Send + Sync>(
    host: &'a dyn HostApi,
    plugin_id: &str,
) -> Option<&'a T> {
    host.plugin_state_any(plugin_id)?.downcast_ref::<T>()
}

/// Typed mutable access to per-tab plugin state stored under `plugin_id`.
pub fn plugin_state_mut<'a, T: Any + Send + Sync>(
    host: &'a mut dyn HostApi,
    plugin_id: &str,
) -> Option<&'a mut T> {
    host.plugin_state_any_mut(plugin_id)?.downcast_mut::<T>()
}

/// Typed get-or-insert of per-tab plugin state stored under `plugin_id`.
pub fn ensure_plugin_state<'a, T: Any + Send + Sync>(
    host: &'a mut dyn HostApi,
    plugin_id: &'static str,
    init: impl FnOnce() -> T,
) -> &'a mut T {
    let mut init = Some(init);
    let any = host.ensure_plugin_state_any(plugin_id, &mut || {
        Box::new((init.take().expect("init called once"))())
    });
    any.downcast_mut::<T>()
        .expect("plugin state type mismatch for plugin_id")
}
