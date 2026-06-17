use crate::plugin::host::HostApi;

pub fn handle(host: &mut dyn HostApi, cmd: &str) -> bool {
    // "DP_IMPORT <path>" arrives from ModuleEvent::PluginFileDialog with the
    // path in its original case (the command line is bypassed).
    if let Some(path) = cmd.strip_prefix("DP_IMPORT ") {
        host.push_info(&format!("demo_plugin imported: {path}"));
        return true;
    }
    match cmd {
        "DP_HELLO" => {
            host.push_info("Hello from demo_plugin (plugin host OK).");
            true
        }
        _ => false,
    }
}