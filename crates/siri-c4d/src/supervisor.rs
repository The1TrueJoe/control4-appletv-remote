//! Runs one worker per configured remote and turns their events into Control4
//! commands.
//!
//! Each remote gets its own blocking ATT session on a dedicated thread (the
//! socket is not a tokio type), forwarding events over a channel to an async
//! handler that talks HTTP. Workers restart themselves when a remote sleeps,
//! so several remotes coexist on one Pi without interfering.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use siri_remote::{protocol, session::SessionConfig, Event, Session};
use tokio::sync::{broadcast, mpsc, RwLock};
use tracing::{debug, info, warn};

use crate::config::{Action, Config};
use crate::control4::Client;
use siri_remote::SwipeDetector;
use crate::voice;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<RwLock<Config>>,
    pub config_path: std::path::PathBuf,
    /// Live event feed for the web UI.
    pub events: broadcast::Sender<serde_json::Value>,
    pub workers: Arc<RwLock<HashMap<String, WorkerHandle>>>,
}

pub struct WorkerHandle {
    pub task: tokio::task::JoinHandle<()>,
    pub status: Arc<RwLock<RemoteStatus>>,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct RemoteStatus {
    pub connected: bool,
    pub mtu: u16,
    pub audio_available: bool,
    pub battery: Option<u8>,
    pub power: Option<String>,
    pub last_event: Option<String>,
    pub last_error: Option<String>,
}

impl AppState {
    pub fn emit(&self, kind: &str, address: &str, mut extra: serde_json::Value) {
        let obj = extra.as_object_mut();
        let mut v = json!({
            "kind": kind,
            "address": address,
            "at": chrono_now(),
        });
        if let (Some(o), Some(dst)) = (obj, v.as_object_mut()) {
            for (k, val) in o.iter() {
                dst.insert(k.clone(), val.clone());
            }
        }
        // A send error just means nobody is watching the UI.
        let _ = self.events.send(v);
    }
}

/// Seconds-resolution timestamp without pulling in a date library.
fn chrono_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    secs.to_string()
}

/// Start (or restart) workers so the running set matches the config.
pub async fn reconcile(state: &AppState) {
    let cfg = state.config.read().await.clone();
    let wanted: Vec<_> = cfg
        .remotes
        .iter()
        .filter(|r| r.enabled && !r.address.trim().is_empty())
        .map(|r| r.address.to_uppercase())
        .collect();

    let mut workers = state.workers.write().await;

    // Stop workers for remotes that were removed or disabled.
    let running: Vec<String> = workers.keys().cloned().collect();
    for addr in running {
        if !wanted.contains(&addr) {
            if let Some(h) = workers.remove(&addr) {
                info!(%addr, "stopping worker");
                h.task.abort();
            }
        }
    }

    for addr in wanted {
        if workers.contains_key(&addr) {
            continue;
        }
        let status = Arc::new(RwLock::new(RemoteStatus::default()));
        let task = tokio::spawn(worker(state.clone(), addr.clone(), status.clone()));
        workers.insert(addr, WorkerHandle { task, status });
    }
}

/// One remote, forever: connect, stream events, reconnect when it sleeps.
async fn worker(state: AppState, address: String, status: Arc<RwLock<RemoteStatus>>) {
    loop {
        let cfg = state.config.read().await.clone();
        let Some(remote) = cfg.remote(&address).cloned() else {
            return;
        };
        if !remote.enabled {
            return;
        }

        let (tx, mut rx) = mpsc::unbounded_channel::<Event>();

        let mut scfg = SessionConfig::new(&remote.address);
        scfg.addr_type = if remote.addr_type.eq_ignore_ascii_case("random") {
            siri_remote::att::BDADDR_LE_RANDOM
        } else {
            siri_remote::att::BDADDR_LE_PUBLIC
        };
        scfg.connect_timeout = Duration::from_secs(900);
        let chosen = cfg.bluetooth.adapter.trim();
        if !chosen.is_empty() {
            scfg.adapter = Some(chosen.to_string());
        }

        // The ATT socket is blocking, so it lives on its own thread.
        let blocking = tokio::task::spawn_blocking(move || {
            Session::run(&scfg, |ev| tx.send(ev).is_ok())
        });

        let handler = handle_events(state.clone(), address.clone(), status.clone(), &mut rx);
        let (session_result, _) = tokio::join!(blocking, handler);

        {
            let mut s = status.write().await;
            s.connected = false;
            if let Ok(Err(e)) = &session_result {
                s.last_error = Some(e.to_string());
            }
        }
        state.emit("disconnected", &address, json!({}));

        // Nothing to reconnect to until the remote is pressed again; a short
        // pause keeps this from becoming a hot loop if the adapter is down.
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

async fn handle_events(
    state: AppState,
    address: String,
    status: Arc<RwLock<RemoteStatus>>,
    rx: &mut mpsc::UnboundedReceiver<Event>,
) {
    let mut swipes = SwipeDetector::default();

    while let Some(ev) = rx.recv().await {
        let cfg = state.config.read().await.clone();
        let Some(remote) = cfg.remote(&address).cloned() else {
            continue;
        };
        let room = cfg.room_for(&remote);
        let client = Client::new(&cfg.control4).ok();

        match ev {
            Event::Connected { mtu, kernel_rcv_mtu } => {
                {
                    let mut s = status.write().await;
                    s.connected = true;
                    s.mtu = mtu;
                    s.audio_available = kernel_rcv_mtu >= protocol::MIN_MTU;
                    s.last_error = None;
                }
                info!(%address, mtu, kernel_rcv_mtu, "connected");
                state.emit(
                    "connected",
                    &address,
                    json!({ "mtu": mtu, "kernel_rcv_mtu": kernel_rcv_mtu }),
                );
            }
            Event::AudioUnavailable { mtu, needed } => {
                warn!(%address, mtu, needed, "kernel socket too small for microphone frames");
                state.emit(
                    "audio_unavailable",
                    &address,
                    json!({ "mtu": mtu, "needed": needed }),
                );
            }
            Event::ButtonDown(b) => {
                state.emit("button", &address, json!({ "button": b.as_str(), "state": "down" }));
                dispatch(&state, &client, &room, &remote, b.as_str(), true).await;
            }
            Event::ButtonUp(b) => {
                state.emit("button", &address, json!({ "button": b.as_str(), "state": "up" }));
                dispatch(&state, &client, &room, &remote, b.as_str(), false).await;
            }
            Event::Touch { fingers, pressed } => {
                if let Some(sw) = swipes.update(&fingers) {
                    state.emit("swipe", &address, json!({ "swipe": sw.as_str() }));
                    dispatch(&state, &client, &room, &remote, sw.as_str(), true).await;
                }
                if let Some(f) = fingers.first() {
                    debug!(%address, x = f.x, y = f.y, p = f.pressure, pressed, "touch");
                }
            }
            Event::Battery(pct) => {
                status.write().await.battery = Some(pct);
                state.emit("battery", &address, json!({ "percent": pct }));
            }
            Event::Power(p) => {
                status.write().await.power = Some(p.as_str().to_string());
                state.emit("power", &address, json!({ "state": p.as_str() }));
            }
            Event::VoiceStart => {
                state.emit("voice_start", &address, json!({}));
            }
            Event::VoiceAudio(_) => {
                // Streamed for future live recognition; the completed session
                // below is what currently drives commands.
            }
            Event::VoiceEnd {
                pcm,
                frames,
                undecodable,
                concealed,
                held,
            } => {
                let secs = pcm.len() as f32 / protocol::AUDIO_SAMPLE_RATE as f32;
                state.emit(
                    "voice_end",
                    &address,
                    json!({
                        "frames": frames,
                        "undecodable": undecodable,
                        "concealed": concealed,
                        "seconds": secs,
                        "held_ms": held.as_millis() as u64,
                    }),
                );
                voice::handle_session(&state, &cfg, &address, pcm).await;
            }
            Event::Disconnected(reason) => {
                status.write().await.last_error = Some(reason.clone());
                info!(%address, %reason, "link lost");
            }
        }
    }
}

async fn dispatch(
    state: &AppState,
    client: &Option<Client>,
    room: &str,
    remote: &crate::config::Remote,
    control: &str,
    pressed: bool,
) {
    let Some(action) = remote.mappings.get(control) else {
        return;
    };
    if matches!(action, Action::None) {
        return;
    }
    let Some(client) = client else { return };
    match client.dispatch(room, action, pressed).await {
        Ok(Some(_)) => {
            state.emit(
                "command",
                &remote.address,
                json!({ "control": control, "ok": true }),
            );
        }
        Ok(None) => {}
        Err(e) => {
            warn!(control, error = %e, "control4 dispatch failed");
            state.emit(
                "command",
                &remote.address,
                json!({ "control": control, "ok": false, "error": e.to_string() }),
            );
        }
    }
}
