use crate::plugin::manifest::{ApiVersion, PluginManifest};

pub const PLUGIN_ID: &str = "opencad.demo_plugin";

pub static MANIFEST: PluginManifest = PluginManifest {
    id: PLUGIN_ID,
    name: "Demo Plugin",
    version: "0.1.0",
    description: "Minimal add-on for plugin-host integration tests",
    api_version: ApiVersion::CURRENT,
    ribbon_order: 99,
    xdata_apps: &[],
    command_prefixes: &["DP_"],
};