use super::plugin::MyPlugin;

inventory::submit! {
    crate::plugin::registry::PluginRegistration {
        construct: || Box::new(MyPlugin),
    }
}