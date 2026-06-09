use super::plugin::DemoPlugin;

inventory::submit! {
    crate::plugin::registry::PluginRegistration {
        construct: || Box::new(DemoPlugin),
    }
}