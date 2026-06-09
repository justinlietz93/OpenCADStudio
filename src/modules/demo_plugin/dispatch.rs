use crate::plugin::host::HostSession;

pub fn handle(host: &mut HostSession<'_>, cmd: &str) -> bool {
    match cmd {
        "DP_HELLO" => {
            host.push_info("Hello from demo_plugin (plugin host OK).");
            true
        }
        _ => false,
    }
}