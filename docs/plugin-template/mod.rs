pub mod dispatch;
pub mod manifest;
pub mod plugin;
pub mod register;

use crate::modules::{CadModule, IconKind, ModuleEvent, RibbonGroup, RibbonItem, ToolDef};

inventory::submit!(crate::command::CommandRegistration {
    names: &["MP_HELLO"]
});

pub struct MyPluginModule;

impl CadModule for MyPluginModule {
    fn id(&self) -> &'static str {
        "my_plugin"
    }

    fn title(&self) -> &'static str {
        "My Plugin"
    }

    fn ribbon_groups(&self) -> Vec<RibbonGroup> {
        vec![RibbonGroup {
            title: "Tools",
            tools: vec![RibbonItem::LargeTool(ToolDef {
                id: "MP_HELLO",
                label: "Hello",
                icon: IconKind::Glyph("★"),
                event: ModuleEvent::Command("MP_HELLO".to_string()),
            })],
        }]
    }
}