use super::*;

impl OpenCADStudio {
    pub(super) fn dispatch_layers(&mut self, cmd: &str, i: usize) -> Option<Task<Message>> {
        match cmd {
            // ── Layer object commands ──────────────────────────────────────
            "LAYOFF" => {
                let handles: Vec<_> = self.tabs[i]
                    .scene
                    .selected_entities()
                    .into_iter()
                    .map(|(h, _)| h)
                    .collect();
                if handles.is_empty() {
                    use crate::modules::draw::select::SelectObjectsCommand;
                    let cmd = SelectObjectsCommand::new("LAYOFF");
                    self.command_line.push_info(&cmd.prompt());
                    self.tabs[i].active_cmd = Some(Box::new(cmd));
                } else {
                    let layers: rustc_hash::FxHashSet<String> = self.tabs[i]
                        .scene
                        .selected_entities()
                        .into_iter()
                        .map(|(_, e)| e.common().layer.clone())
                        .collect();
                    self.push_undo_snapshot(i, "LAYOFF");
                    for name in &layers {
                        if name == "0" {
                            continue;
                        }
                        if let Some(dl) = self.tabs[i].scene.document.layers.get_mut(name) {
                            dl.turn_off();
                        }
                    }
                    self.tabs[i].scene.bump_geometry();
                    self.tabs[i].dirty = true;
                    self.refresh_layer_panel();
                    self.command_line.push_info("Layer(s) turned off.");
                }
            }

            "LAYFRZ" => {
                let handles: Vec<_> = self.tabs[i]
                    .scene
                    .selected_entities()
                    .into_iter()
                    .map(|(h, _)| h)
                    .collect();
                if handles.is_empty() {
                    use crate::modules::draw::select::SelectObjectsCommand;
                    let cmd = SelectObjectsCommand::new("LAYFRZ");
                    self.command_line.push_info(&cmd.prompt());
                    self.tabs[i].active_cmd = Some(Box::new(cmd));
                } else {
                    let layers: rustc_hash::FxHashSet<String> = self.tabs[i]
                        .scene
                        .selected_entities()
                        .into_iter()
                        .map(|(_, e)| e.common().layer.clone())
                        .collect();
                    self.push_undo_snapshot(i, "LAYFRZ");
                    for name in &layers {
                        if name == "0" {
                            continue;
                        }
                        if let Some(dl) = self.tabs[i].scene.document.layers.get_mut(name) {
                            dl.freeze();
                        }
                    }
                    self.tabs[i].scene.bump_geometry();
                    self.tabs[i].dirty = true;
                    self.refresh_layer_panel();
                    self.command_line.push_info("Layer(s) frozen.");
                }
            }

            "LAYLCK" => {
                let handles: Vec<_> = self.tabs[i]
                    .scene
                    .selected_entities()
                    .into_iter()
                    .map(|(h, _)| h)
                    .collect();
                if handles.is_empty() {
                    use crate::modules::draw::select::SelectObjectsCommand;
                    let cmd = SelectObjectsCommand::new("LAYLCK");
                    self.command_line.push_info(&cmd.prompt());
                    self.tabs[i].active_cmd = Some(Box::new(cmd));
                } else {
                    let layers: rustc_hash::FxHashSet<String> = self.tabs[i]
                        .scene
                        .selected_entities()
                        .into_iter()
                        .map(|(_, e)| e.common().layer.clone())
                        .collect();
                    self.push_undo_snapshot(i, "LAYLCK");
                    for name in &layers {
                        if let Some(dl) = self.tabs[i].scene.document.layers.get_mut(name) {
                            dl.lock();
                        }
                    }
                    self.tabs[i].scene.bump_geometry();
                    self.tabs[i].dirty = true;
                    self.refresh_layer_panel();
                    self.command_line.push_info("Layer(s) locked.");
                }
            }

            "LAYMCUR" => {
                let entities = self.tabs[i].scene.selected_entities();
                if entities.is_empty() {
                    use crate::modules::draw::select::SelectObjectsCommand;
                    let cmd = SelectObjectsCommand::new("LAYMCUR");
                    self.command_line.push_info(&cmd.prompt());
                    self.tabs[i].active_cmd = Some(Box::new(cmd));
                } else {
                    let layer = entities[0].1.common().layer.clone();
                    // Keep the document header (CLAYER) in sync, not just the
                    // per-tab default, so a later no-selection ribbon refresh
                    // (e.g. after Esc) doesn't snap back to the stale header
                    // layer. See #93.
                    let handle = self.tabs[i]
                        .scene
                        .document
                        .layers
                        .get(&layer)
                        .map(|l| l.handle)
                        .unwrap_or(acadrust::types::Handle::NULL);
                    self.tabs[i].scene.document.header.current_layer_name = layer.clone();
                    self.tabs[i].scene.document.header.current_layer_handle = handle;
                    self.tabs[i].active_layer = layer.clone();
                    self.ribbon.active_layer = layer.clone();
                    self.tabs[i].layers.current_layer = layer.clone();
                    self.tabs[i].dirty = true;
                    self.command_line
                        .push_info(&format!("Current layer set to \"{layer}\"."));
                    self.refresh_layer_panel();
                }
            }

            "LAYON" => {
                self.push_undo_snapshot(i, "LAYON");
                for name in self.tabs[i]
                    .scene
                    .document
                    .layers
                    .iter()
                    .map(|l| l.name.clone())
                    .collect::<Vec<_>>()
                {
                    if let Some(dl) = self.tabs[i].scene.document.layers.get_mut(&name) {
                        dl.turn_on();
                    }
                }
                self.tabs[i].scene.bump_geometry();
                self.tabs[i].dirty = true;
                self.refresh_layer_panel();
                self.command_line.push_info("All layers turned on.");
            }

            "LAYTHW" => {
                self.push_undo_snapshot(i, "LAYTHW");
                for name in self.tabs[i]
                    .scene
                    .document
                    .layers
                    .iter()
                    .map(|l| l.name.clone())
                    .collect::<Vec<_>>()
                {
                    if let Some(dl) = self.tabs[i].scene.document.layers.get_mut(&name) {
                        dl.thaw();
                    }
                }
                self.tabs[i].scene.bump_geometry();
                self.tabs[i].dirty = true;
                self.refresh_layer_panel();
                self.command_line.push_info("All layers thawed.");
            }

            "LAYULK" => {
                let handles: Vec<_> = self.tabs[i]
                    .scene
                    .selected_entities()
                    .into_iter()
                    .map(|(h, _)| h)
                    .collect();
                if handles.is_empty() {
                    use crate::modules::draw::select::SelectObjectsCommand;
                    let cmd = SelectObjectsCommand::new("LAYULK");
                    self.command_line.push_info(&cmd.prompt());
                    self.tabs[i].active_cmd = Some(Box::new(cmd));
                } else {
                    let layers: rustc_hash::FxHashSet<String> = self.tabs[i]
                        .scene
                        .selected_entities()
                        .into_iter()
                        .map(|(_, e)| e.common().layer.clone())
                        .collect();
                    self.push_undo_snapshot(i, "LAYULK");
                    for name in &layers {
                        if let Some(dl) = self.tabs[i].scene.document.layers.get_mut(name) {
                            dl.unlock();
                        }
                    }
                    self.tabs[i].scene.bump_geometry();
                    self.tabs[i].dirty = true;
                    self.refresh_layer_panel();
                    self.command_line.push_info("Layer(s) unlocked.");
                }
            }

            // LAYISO — turn off all layers except those used by selected entities
            "LAYISO" => {
                let sel_layers: rustc_hash::FxHashSet<String> = self.tabs[i]
                    .scene
                    .selected_entities()
                    .into_iter()
                    .map(|(_, e)| e.common().layer.clone())
                    .collect();
                if sel_layers.is_empty() {
                    self.command_line
                        .push_error("LAYISO: select entities on the layers to isolate first.");
                } else {
                    self.push_undo_snapshot(i, "LAYISO");
                    let names: Vec<String> = self.tabs[i]
                        .scene
                        .document
                        .layers
                        .iter()
                        .map(|l| l.name.clone())
                        .collect();
                    for name in names {
                        if !sel_layers.contains(&name) {
                            if let Some(dl) = self.tabs[i].scene.document.layers.get_mut(&name) {
                                dl.turn_off();
                            }
                        }
                    }
                    self.tabs[i].scene.bump_geometry();
                    self.tabs[i].dirty = true;
                    self.refresh_layer_panel();
                    self.command_line
                        .push_info(&format!("LAYISO: isolated {} layer(s).", sel_layers.len()));
                }
            }

            // ISOLATEOBJECTS — hide every object except the current selection
            "ISOLATEOBJECTS" => {
                if self.tabs[i].scene.selected.is_empty() {
                    self.command_line
                        .push_error("ISOLATEOBJECTS: select the objects to isolate first.");
                } else {
                    let n = self.tabs[i].scene.selected.len();
                    self.tabs[i].scene.isolate_selected();
                    self.command_line.push_info(&format!(
                        "Isolated {n} object(s). UNISOLATEOBJECTS to restore."
                    ));
                }
            }

            // HIDEOBJECTS — hide the current selection
            "HIDEOBJECTS" => {
                if self.tabs[i].scene.selected.is_empty() {
                    self.command_line
                        .push_error("HIDEOBJECTS: select the objects to hide first.");
                } else {
                    let n = self.tabs[i].scene.selected.len();
                    self.tabs[i].scene.hide_selected();
                    self.command_line
                        .push_info(&format!("Hid {n} object(s). UNISOLATEOBJECTS to restore."));
                }
            }

            // UNISOLATEOBJECTS — bring back everything hidden by Isolate / Hide
            "UNISOLATEOBJECTS" => {
                if self.tabs[i].scene.is_isolation_active() {
                    self.tabs[i].scene.end_isolation();
                    self.command_line
                        .push_info("Isolation ended — all objects shown.");
                } else {
                    self.command_line.push_info("No hidden objects.");
                }
            }

            // LAYUNISO — restore all layers that were turned off by LAYISO (turn all on)
            "LAYUNISO" => {
                self.push_undo_snapshot(i, "LAYUNISO");
                let names: Vec<String> = self.tabs[i]
                    .scene
                    .document
                    .layers
                    .iter()
                    .map(|l| l.name.clone())
                    .collect();
                for name in names {
                    if let Some(dl) = self.tabs[i].scene.document.layers.get_mut(&name) {
                        dl.turn_on();
                    }
                }
                self.tabs[i].scene.bump_geometry();
                self.tabs[i].dirty = true;
                self.refresh_layer_panel();
                self.command_line
                    .push_info("LAYUNISO: all layers restored.");
            }

            "LAYMATCH" | "LAYMCH" => {
                use crate::modules::draw::layers::match_layer::LayMatchCommand;
                let dest: Vec<_> = self.tabs[i]
                    .scene
                    .selected_entities()
                    .into_iter()
                    .map(|(h, _)| h)
                    .collect();
                let cmd = LayMatchCommand::new(dest);
                self.command_line.push_info(&cmd.prompt());
                self.tabs[i].active_cmd = Some(Box::new(cmd));
            }

            "MATCHPROP" | "MA" => {
                use crate::modules::draw::properties::match_prop::MatchPropCommand;
                self.tabs[i].scene.deselect_all();
                let cmd = MatchPropCommand::new();
                self.command_line.push_info(&cmd.prompt());
                self.tabs[i].active_cmd = Some(Box::new(cmd));
            }

            "GROUP" | "G" => {
                let handles: Vec<_> = self.tabs[i]
                    .scene
                    .selected_entities()
                    .into_iter()
                    .map(|(h, _)| h)
                    .collect();
                if handles.is_empty() {
                    use crate::modules::draw::select::SelectObjectsCommand;
                    let cmd = SelectObjectsCommand::new("GROUP");
                    self.command_line.push_info(&cmd.prompt());
                    self.tabs[i].active_cmd = Some(Box::new(cmd));
                } else {
                    let auto_name = super::super::helpers::next_group_auto_name(&self.tabs[i].scene);
                    use crate::modules::draw::groups::group::GroupCommand;
                    let cmd = GroupCommand::new(handles, auto_name);
                    self.command_line.push_info(&cmd.prompt());
                    self.tabs[i].active_cmd = Some(Box::new(cmd));
                }
            }

            "UNGROUP" | "UG" => {
                let handles: Vec<_> = self.tabs[i]
                    .scene
                    .selected_entities()
                    .into_iter()
                    .map(|(h, _)| h)
                    .collect();
                if handles.is_empty() {
                    use crate::modules::draw::groups::ungroup::UngroupCommand;
                    let cmd = UngroupCommand::new();
                    self.command_line.push_info(&cmd.prompt());
                    self.tabs[i].active_cmd = Some(Box::new(cmd));
                } else {
                    self.push_undo_snapshot(i, "UNGROUP");
                    let count = self.tabs[i].scene.delete_groups_containing(&handles);
                    self.tabs[i].dirty = true;
                    if count > 0 {
                        self.command_line
                            .push_info(&format!("{} group(s) dissolved.", count));
                    } else {
                        self.command_line
                            .push_info("No groups found for selected objects.");
                    }
                }
            }

            _ => return None,
        }
        Some(self.finish_dispatch(cmd))
    }
}
