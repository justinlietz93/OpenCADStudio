use crate::plugin::host::{BuiltinPlugin, HostApi};
use crate::plugin::manifest::PluginManifest;

use super::dispatch;
use super::manifest;

pub struct DemoPlugin;

impl BuiltinPlugin for DemoPlugin {
    fn manifest(&self) -> &'static PluginManifest {
        &manifest::MANIFEST
    }

    fn ribbon(&self) -> Box<dyn crate::modules::CadModule> {
        Box::new(super::DemoPluginModule)
    }

    fn dispatch(&self, host: &mut dyn HostApi, cmd: &str) -> bool {
        dispatch::handle(host, cmd)
    }
}