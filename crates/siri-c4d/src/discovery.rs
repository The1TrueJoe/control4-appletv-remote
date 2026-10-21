//! Advertise this bridge over mDNS so the Control4 driver can find it.
//!
//! Service type `_appletv-bridge._tcp`. TXT keys match `docs/FORWARDER.md`:
//! `name`, `room`, `port` (the web UI / status port), `model`, `id`, `path`.
//! The driver browses this to list and bind bridges without the installer
//! typing an IP.

use std::collections::HashMap;

use anyhow::Result;
use mdns_sd::{ServiceDaemon, ServiceInfo};

use crate::config::Config;

pub const SERVICE_TYPE: &str = "_appletv-bridge._tcp.local.";

/// Register the service and return the running daemon. Keep the returned value
/// alive for as long as the bridge should be discoverable (dropping it
/// unregisters). Errors are non-fatal to the daemon — advertising is a
/// convenience, not required for operation.
pub fn advertise(cfg: &Config) -> Result<ServiceDaemon> {
    let mdns = ServiceDaemon::new()?;

    let host = hostname();
    let id = machine_id().unwrap_or_else(|| host.clone());
    let instance = format!("Siri Remote Bridge ({host})");
    let host_name = format!("{host}.local.");

    let mut props: HashMap<String, String> = HashMap::new();
    props.insert("name".into(), instance.clone());
    props.insert("room".into(), cfg.control4.room.clone());
    props.insert("port".into(), cfg.web.port.to_string());
    props.insert("model".into(), "pi".into());
    props.insert("id".into(), id);
    props.insert("path".into(), "/".into());

    // Empty IP + enable_addr_auto() lets mdns-sd fill in this host's addresses
    // and keep them current as interfaces change.
    let info = ServiceInfo::new(
        SERVICE_TYPE,
        &instance,
        &host_name,
        "",
        cfg.web.port,
        props,
    )?
    .enable_addr_auto();

    mdns.register(info)?;
    tracing::info!("advertising {SERVICE_TYPE} as {instance:?} on port {}", cfg.web.port);
    Ok(mdns)
}

fn hostname() -> String {
    std::fs::read_to_string("/etc/hostname")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "siri-c4d".into())
}

fn machine_id() -> Option<String> {
    for p in ["/etc/machine-id", "/var/lib/dbus/machine-id"] {
        if let Ok(s) = std::fs::read_to_string(p) {
            let s = s.trim();
            if !s.is_empty() {
                return Some(s.to_string());
            }
        }
    }
    None
}
