// HostSession — plugin-facing API implemented inside `app` (private field access).

use std::any::{Any, TypeId};

use acadrust::{CadDocument, EntityType, Handle};

use super::OpenCADStudio;
use crate::command::CadCommand;

/// Session adapter: one active document tab, command line, undo.
pub(crate) struct HostSession<'a> {
    app: &'a mut OpenCADStudio,
    tab: usize,
}

impl<'a> HostSession<'a> {
    pub(crate) fn new(app: &'a mut OpenCADStudio, tab: usize) -> Self {
        Self { app, tab }
    }

    pub fn tab_index(&self) -> usize {
        self.tab
    }

    pub fn document(&self) -> &CadDocument {
        &self.app.tabs[self.tab].scene.document
    }

    pub fn document_mut(&mut self) -> &mut CadDocument {
        &mut self.app.tabs[self.tab].scene.document
    }

    pub fn entities(&self) -> impl Iterator<Item = &EntityType> {
        self.document().entities()
    }

    pub fn entities_mut(&mut self) -> impl Iterator<Item = &mut EntityType> {
        self.document_mut().entities_mut()
    }

    pub fn add_entity(&mut self, entity: EntityType) -> Handle {
        self.app.tabs[self.tab].scene.add_entity(entity)
    }

    pub fn bump_geometry(&mut self) {
        self.app.tabs[self.tab].scene.bump_geometry();
    }

    pub fn push_undo(&mut self, label: &str) {
        self.app.push_undo_snapshot(self.tab, label);
    }

    pub fn set_dirty(&mut self) {
        self.app.tabs[self.tab].dirty = true;
    }

    pub fn push_info(&mut self, msg: &str) {
        self.app.command_line.push_info(msg);
    }

    pub fn push_output(&mut self, msg: &str) {
        self.app.command_line.push_output(msg);
    }

    pub fn push_error(&mut self, msg: &str) {
        self.app.command_line.push_error(msg);
    }

    pub fn set_active_command(&mut self, cmd: Box<dyn CadCommand>) {
        self.app.tabs[self.tab].active_cmd = Some(cmd);
    }

    pub fn plugin_state<T: Any + Send + Sync + 'static>(
        &self,
        plugin_id: &'static str,
    ) -> Option<&T> {
        self.app.tabs[self.tab].plugin_state(plugin_id, TypeId::of::<T>())
    }

    pub fn plugin_state_mut<T: Any + Send + Sync + 'static>(
        &mut self,
        plugin_id: &'static str,
    ) -> Option<&mut T> {
        self.app.tabs[self.tab].plugin_state_mut(plugin_id, TypeId::of::<T>())
    }

    pub fn ensure_plugin_state<T: Any + Send + Sync + 'static>(
        &mut self,
        plugin_id: &'static str,
        init: impl FnOnce() -> T,
    ) -> &mut T {
        self.app.tabs[self.tab].ensure_plugin_state(plugin_id, init)
    }
}