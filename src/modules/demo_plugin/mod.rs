// Minimal in-tree add-on — validates plugin host on `feature/plugin-host`.

pub mod dispatch;
pub mod manifest;
pub mod plugin;
pub mod register;

use crate::modules::{CadModule, IconKind, ModuleEvent, RibbonGroup, RibbonItem, ToolDef};

inventory::submit!(crate::command::CommandRegistration {
    names: &["DP_HELLO"]
});

pub struct DemoPluginModule;

impl CadModule for DemoPluginModule {
    fn id(&self) -> &'static str {
        "demo_plugin"
    }

    fn title(&self) -> &'static str {
        "Demo Plugin"
    }

    fn ribbon_groups(&self) -> Vec<RibbonGroup> {
        vec![RibbonGroup {
            title: "Smoke",
            tools: vec![RibbonItem::LargeTool(ToolDef {
                id: "DP_HELLO",
                label: "Hello",
                icon: IconKind::Glyph("★"),
                event: ModuleEvent::Command("DP_HELLO".to_string()),
            })],
        }]
    }
}

#[cfg(test)]
mod tests {
    use crate::plugin::all_ribbon_modules;

    #[test]
    fn ribbon_tab_is_registered() {
        let titles: Vec<&str> = all_ribbon_modules().iter().map(|m| m.title()).collect();
        assert!(titles.contains(&"Demo Plugin"), "tabs: {titles:?}");
    }
}