use crate::plugin::manifest::{ApiVersion, PluginManifest};

pub const PLUGIN_ID: &str = "opencad.my_plugin";

pub static MANIFEST: PluginManifest = PluginManifest {
    id: PLUGIN_ID,
    name: "My Plugin",
    version: "0.1.0",
    description: "Short description of what this add-on does",
    api_version: ApiVersion::CURRENT,
    ribbon_order: 60,
    xdata_apps: &["MYPLUGIN_RECORD"],
    command_prefixes: &["MP_"],
};