// Plugin traits — HostSession lives in `app::plugin_host` (same-crate field access).

pub(crate) use crate::app::plugin_host::HostSession;

use crate::modules::CadModule;

use super::manifest::PluginManifest;

/// Add-on package entry point (phase 1: in-tree, in-process).
///
/// One `PluginRegistration` per package — ribbon tab, manifest, and command
/// dispatch are owned here. See `docs/plugin-architecture.md`.
pub trait BuiltinPlugin: Send + Sync {
    fn manifest(&self) -> &'static PluginManifest;
    fn ribbon(&self) -> Box<dyn CadModule>;
    fn dispatch(&self, host: &mut HostSession<'_>, cmd: &str) -> bool;
}