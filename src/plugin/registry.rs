// Compile-time plugin registry via `inventory`.

use super::host::{BuiltinPlugin, HostSession};
use crate::app::OpenCADStudio;
use crate::modules::{registry as core_registry, CadModule};

pub struct PluginRegistration {
    pub construct: fn() -> Box<dyn BuiltinPlugin>,
}

inventory::collect!(PluginRegistration);

/// Construct every registered built-in plugin (once per process).
pub fn all_plugins() -> Vec<Box<dyn BuiltinPlugin>> {
    inventory::iter::<PluginRegistration>
        .into_iter()
        .map(|r| (r.construct)())
        .collect()
}

/// Core ribbon tabs plus add-on tabs (sorted by `manifest.ribbon_order`).
pub fn all_ribbon_modules() -> Vec<Box<dyn CadModule>> {
    let mut core = core_registry::all_modules();
    let mut addons: Vec<(i32, Box<dyn CadModule>)> = all_plugins()
        .into_iter()
        .map(|p| (p.manifest().ribbon_order, p.ribbon()))
        .collect();
    addons.sort_by_key(|(order, _)| *order);
    core.extend(addons.into_iter().map(|(_, ribbon)| ribbon));
    core
}

/// Try each plugin until one handles `cmd`. Returns true if handled.
pub(crate) fn try_dispatch(app: &mut OpenCADStudio, tab: usize, cmd: &str) -> bool {
    let mut host = HostSession::new(app, tab);
    for plugin in all_plugins() {
        if plugin.dispatch(&mut host, cmd) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::OpenCADStudio;
    #[test]
    fn discovers_registered_plugins() {
        let plugins = all_plugins();
        assert!(
            !plugins.is_empty(),
            "expected at least one PluginRegistration (demo_plugin)"
        );
        assert!(
            plugins
                .iter()
                .any(|p| p.manifest().id == "opencad.demo_plugin"),
            "demo_plugin missing; ids: {:?}",
            plugins.iter().map(|p| p.manifest().id).collect::<Vec<_>>()
        );
    }

    #[test]
    fn addon_ribbon_tabs_merge_after_core() {
        let titles: Vec<&str> = all_ribbon_modules().iter().map(|m| m.title()).collect();
        assert!(titles.contains(&"Demo Plugin"), "ribbon tabs: {titles:?}");
        let core = core_registry::all_modules();
        assert_eq!(titles.len(), core.len() + all_plugins().len());
    }

    #[test]
    fn try_dispatch_routes_demo_command() {
        let mut app = OpenCADStudio::new_for_test();
        assert!(try_dispatch(&mut app, 0, "DP_HELLO"));
        let info = app.command_history_info();
        assert!(
            info.iter().any(|t| t.contains("demo_plugin") && t.contains("plugin host OK")),
            "info history: {info:?}"
        );
    }

    #[test]
    fn unknown_plugin_command_falls_through() {
        let mut app = OpenCADStudio::new_for_test();
        assert!(!try_dispatch(&mut app, 0, "DP_NOPE"));
    }
}