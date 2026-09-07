//! Configuration API and embedded web UI.
//!
//! The UI is compiled into the binary so deployment is a single file — which
//! is what makes this usable on buildroot/openHC, a dedicated Pi image, or as
//! a plain binary, without shipping an asset directory alongside.

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Path, State};
use axum::http::{header, StatusCode, Uri};
use axum::response::sse::{Event as SseEvent, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post, put};
use axum::{Json, Router};
use futures_util::stream::Stream;
use rust_embed::RustEmbed;
use serde::Deserialize;
use serde_json::{json, Value};
use tracing::info;

use siri_remote::bluez;
use crate::config::{default_mappings, Config, Remote};
use crate::control4::Client;
use crate::supervisor::{self, AppState};

/// The compiled UI, embedded at build time by `build.rs`.
#[derive(RustEmbed)]
#[folder = "web/dist/"]
struct Assets;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/status", get(status))
        .route("/api/config", get(get_config).put(put_config))
        .route("/api/commands", get(known_commands))
        .route("/api/scan", post(scan))
        .route("/api/remotes", post(add_remote))
        .route("/api/remotes/{address}", delete(remove_remote))
        .route("/api/remotes/{address}/mappings", put(put_mappings))
        .route("/api/remotes/{address}/test", post(test_remote))
        .route("/api/c4/health", get(c4_health))
        .route("/api/c4/rooms", get(c4_rooms))
        .route("/api/c4/commands", get(c4_commands))
        .route("/api/c4/presets", get(c4_presets))
        .route("/api/c4/buttons", get(c4_buttons))
        .route("/api/c4/command", post(c4_command))
        .route("/api/events", get(events))
        .fallback(static_asset)
        .with_state(Arc::new(state))
}

type St = State<Arc<AppState>>;

fn err(e: impl std::fmt::Display) -> (StatusCode, Json<Value>) {
    (
        StatusCode::BAD_GATEWAY,
        Json(json!({ "error": e.to_string() })),
    )
}

async fn static_asset(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };
    match Assets::get(path).or_else(|| Assets::get("index.html")) {
        Some(f) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            ([(header::CONTENT_TYPE, mime.as_ref())], f.data.into_owned()).into_response()
        }
        None => (StatusCode::NOT_FOUND, "not found").into_response(),
    }
}

async fn status(State(s): St) -> Json<Value> {
    let cfg = s.config.read().await.clone();
    let workers = s.workers.read().await;
    let mut remotes = Vec::new();
    for r in &cfg.remotes {
        let st = match workers.get(&r.address.to_uppercase()) {
            Some(h) => serde_json::to_value(h.status.read().await.clone()).unwrap_or(Value::Null),
            None => Value::Null,
        };
        remotes.push(json!({
            "address": r.address,
            "name": r.name,
            "enabled": r.enabled,
            "room": cfg.room_for(r),
            "status": st,
        }));
    }
    let adapter = bluez::info().await.ok();
    Json(json!({
        "remotes": remotes,
        "adapter": adapter,
        "control4": {
            "host": cfg.control4.host,
            "port": cfg.control4.port,
            "configured": !cfg.control4.host.trim().is_empty(),
        },
        "voice": {
            "enabled": cfg.voice.enabled,
            "keep_recordings": cfg.voice.keep_recordings,
            "audio_dir": cfg.voice.audio_dir,
        },
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

async fn get_config(State(s): St) -> Json<Config> {
    Json(s.config.read().await.clone())
}

async fn put_config(
    State(s): St,
    Json(new): Json<Config>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    {
        let mut cfg = s.config.write().await;
        *cfg = new;
        cfg.save(&s.config_path).map_err(err)?;
    }
    supervisor::reconcile(&s).await;
    Ok(Json(json!({ "ok": true })))
}

/// Controls the remote exposes, so the UI can build a mapping form without
/// hard-coding them in JavaScript.
async fn known_commands() -> Json<Value> {
    Json(json!({
        "controls": [
            "MENU", "PLAY_PAUSE", "VOLUME_UP", "VOLUME_DOWN", "AIRPLAY",
            "TOUCHPAD", "TOUCHPAD_2", "SIRI",
            "SWIPE_UP", "SWIPE_DOWN", "SWIPE_LEFT", "SWIPE_RIGHT"
        ],
        "commands": [
            "MENU", "BACK", "ENTER", "UP", "DOWN", "LEFT", "RIGHT",
            "PLAYPAUSE", "PLAY", "PAUSE", "STOP", "SKIP_FWD", "SKIP_REV",
            "VOLUME_UP", "VOLUME_DOWN", "MUTE_TOGGLE", "ROOM_OFF",
            "GUIDE", "INFO", "EXIT", "RECORD", "POWER",
            "CHANNEL_UP", "CHANNEL_DOWN",
            "PROGRAM_A", "PROGRAM_B", "PROGRAM_C", "PROGRAM_D",
            "CUSTOM_1", "CUSTOM_2", "CUSTOM_3"
        ],
        "hold_pairs": {
            "VOLUME_UP":   { "start": "START_VOL_UP",   "stop": "STOP_VOL_UP" },
            "VOLUME_DOWN": { "start": "START_VOL_DOWN", "stop": "STOP_VOL_DOWN" },
            "CHANNEL_UP":  { "start": "START_CH_UP",    "stop": "STOP_CH_UP" },
            "CHANNEL_DOWN":{ "start": "START_CH_DOWN",  "stop": "STOP_CH_DOWN" }
        }
    }))
}

#[derive(Deserialize)]
struct ScanReq {
    #[serde(default = "default_scan_secs")]
    seconds: u64,
}

fn default_scan_secs() -> u64 {
    20
}

async fn scan(Json(req): Json<ScanReq>) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let found = bluez::scan(req.seconds.clamp(5, 120)).await.map_err(err)?;
    Ok(Json(json!({ "devices": found })))
}

#[derive(Deserialize)]
struct AddRemote {
    address: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    room: String,
}

/// Pair, then add to the config and start a worker.
async fn add_remote(
    State(s): St,
    Json(req): Json<AddRemote>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let addr_type = bluez::pair(&req.address).await.map_err(err)?;
    info!(address = %req.address, %addr_type, "paired");

    {
        let mut cfg = s.config.write().await;
        if let Some(existing) = cfg
            .remotes
            .iter_mut()
            .find(|r| r.address.eq_ignore_ascii_case(&req.address))
        {
            existing.enabled = true;
            existing.addr_type = addr_type.clone();
            if !req.name.is_empty() {
                existing.name = req.name.clone();
            }
            if !req.room.is_empty() {
                existing.room = req.room.clone();
            }
        } else {
            cfg.remotes.push(Remote {
                address: req.address.to_uppercase(),
                name: if req.name.is_empty() {
                    req.address.clone()
                } else {
                    req.name.clone()
                },
                enabled: true,
                addr_type: addr_type.clone(),
                room: req.room.clone(),
                mappings: default_mappings(),
            });
        }
        cfg.save(&s.config_path).map_err(err)?;
    }
    supervisor::reconcile(&s).await;
    Ok(Json(json!({ "ok": true, "addr_type": addr_type })))
}

async fn remove_remote(
    State(s): St,
    Path(address): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    {
        let mut cfg = s.config.write().await;
        cfg.remotes
            .retain(|r| !r.address.eq_ignore_ascii_case(&address));
        cfg.save(&s.config_path).map_err(err)?;
    }
    let _ = bluez::remove(&address).await;
    supervisor::reconcile(&s).await;
    Ok(Json(json!({ "ok": true })))
}

async fn put_mappings(
    State(s): St,
    Path(address): Path<String>,
    Json(mappings): Json<std::collections::BTreeMap<String, crate::config::Action>>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    {
        let mut cfg = s.config.write().await;
        let Some(r) = cfg
            .remotes
            .iter_mut()
            .find(|r| r.address.eq_ignore_ascii_case(&address))
        else {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "no such remote" })),
            ));
        };
        r.mappings = mappings;
        cfg.save(&s.config_path).map_err(err)?;
    }
    Ok(Json(json!({ "ok": true })))
}

/// Fire a remote's mapped action for a control, to check wiring end to end
/// without walking to the other room.
#[derive(Deserialize)]
struct TestReq {
    control: String,
}

async fn test_remote(
    State(s): St,
    Path(address): Path<String>,
    Json(req): Json<TestReq>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let cfg = s.config.read().await.clone();
    let Some(remote) = cfg.remote(&address) else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "no such remote" })),
        ));
    };
    let Some(action) = remote.mappings.get(&req.control) else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "no mapping for that control" })),
        ));
    };
    let client = Client::new(&cfg.control4).map_err(err)?;
    let room = cfg.room_for(remote);
    client.dispatch(&room, action, true).await.map_err(err)?;
    // Release immediately so a hold mapping does not ramp forever.
    let _ = client.dispatch(&room, action, false).await;
    Ok(Json(json!({ "ok": true })))
}

async fn c4_client(s: &AppState) -> Result<Client, (StatusCode, Json<Value>)> {
    let cfg = s.config.read().await.clone();
    Client::new(&cfg.control4).map_err(err)
}

async fn c4_health(State(s): St) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    Ok(Json(c4_client(&s).await?.health().await.map_err(err)?))
}

async fn c4_rooms(State(s): St) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    Ok(Json(c4_client(&s).await?.rooms().await.map_err(err)?))
}

async fn c4_commands(State(s): St) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    Ok(Json(c4_client(&s).await?.commands().await.map_err(err)?))
}

/// Presets and custom buttons are configured on the room child drivers, so
/// the UI reads them back rather than trying to define them here.
async fn c4_presets(State(s): St) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    Ok(Json(c4_client(&s).await?.presets().await.map_err(err)?))
}

async fn c4_buttons(State(s): St) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    Ok(Json(c4_client(&s).await?.buttons().await.map_err(err)?))
}

#[derive(Deserialize)]
struct C4Cmd {
    #[serde(default)]
    room: String,
    command: String,
}

async fn c4_command(
    State(s): St,
    Json(req): Json<C4Cmd>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let cfg = s.config.read().await.clone();
    let room = if req.room.is_empty() {
        cfg.control4.room.clone()
    } else {
        req.room
    };
    let client = Client::new(&cfg.control4).map_err(err)?;
    Ok(Json(client.command(&room, &req.command).await.map_err(err)?))
}

/// Live event feed, so the pairing page can show button presses as they happen
/// — which is how you confirm the right remote is talking to the right room.
async fn events(State(s): St) -> Sse<impl Stream<Item = Result<SseEvent, Infallible>>> {
    let mut rx = s.events.subscribe();
    let stream = async_stream::stream! {
        loop {
            match rx.recv().await {
                Ok(v) => yield Ok(SseEvent::default().data(v.to_string())),
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(_) => break,
            }
        }
    };
    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}
