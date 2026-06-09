use crate::plugin::host::{BuiltinPlugin, HostSession};
use crate::plugin::manifest::PluginManifest;

use super::dispatch;
use super::manifest;

pub struct MyPlugin;

impl BuiltinPlugin for MyPlugin {
    fn manifest(&self) -> &'static PluginManifest {
        &manifest::MANIFEST
    }

    fn ribbon(&self) -> Box<dyn crate::modules::CadModule> {
        Box::new(super::MyPluginModule)
    }

    fn dispatch(&self, host: &mut HostSession<'_>, cmd: &str) -> bool {
        dispatch::handle(host, cmd)
    }
}