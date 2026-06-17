// Plugin traits — HostSession lives in `app::plugin_host` (same-crate field
// access) and implements the stable `HostApi` contract plugins target.

pub(crate) use crate::app::plugin_host::HostSession;
/// The stable runtime surface a plugin's `dispatch` receives.
pub use ocs_plugin_api::host::HostApi;

use crate::modules::CadModule;

use super::manifest::PluginManifest;

/// Add-on package entry point (phase 1: in-tree, in-process).
///
/// One `PluginRegistration` per package — ribbon tab, manifest, and command
/// dispatch are owned here. `dispatch` receives `&mut dyn HostApi` (the stable
/// `ocs_plugin_api` contract), not the host's concrete session type. See
/// `docs/plugin-architecture.md`.
pub trait BuiltinPlugin: Send + Sync {
    fn manifest(&self) -> &'static PluginManifest;
    fn ribbon(&self) -> Box<dyn CadModule>;
    fn dispatch(&self, host: &mut dyn HostApi, cmd: &str) -> bool;
}