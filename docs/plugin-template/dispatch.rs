use crate::plugin::host::HostSession;

use super::manifest::PLUGIN_ID;

pub fn handle(host: &mut HostSession<'_>, cmd: &str) -> bool {
    let _ = (host, PLUGIN_ID);
    match cmd {
        "MP_HELLO" => {
            host.push_info("Hello from my_plugin");
            true
        }
        _ => false,
    }
}