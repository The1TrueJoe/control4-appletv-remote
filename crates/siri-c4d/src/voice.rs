//! What happens to the microphone stream.
//!
//! Deliberately narrow: this writes the recording and announces where it went.
//! Speech recognition is **not** part of this project — the remote's audio is
//! 16 kHz mono PCM, which is what every offline recogniser wants, so consumers
//! can pick their own without this daemon having an opinion.
//!
//! For live audio rather than files, use the `siri-events` binary's
//! `--pcm-fifo`, which streams PCM as it arrives.

use std::path::PathBuf;

use serde_json::json;
use siri_remote::{audio, protocol};
use tracing::warn;

use crate::config::Config;
use crate::supervisor::AppState;

pub async fn handle_session(state: &AppState, cfg: &Config, address: &str, pcm: Vec<i16>) {
    if !cfg.voice.enabled || pcm.is_empty() {
        return;
    }
    if !cfg.voice.keep_recordings {
        return;
    }

    match write_recording(cfg, address, &pcm).await {
        Ok(path) => {
            state.emit(
                "voice_saved",
                address,
                json!({
                    "path": path.display().to_string(),
                    "sample_rate": protocol::AUDIO_SAMPLE_RATE,
                    "channels": protocol::AUDIO_CHANNELS,
                }),
            );
        }
        Err(e) => {
            warn!(error = %e, "could not write recording");
            state.emit("voice_error", address, json!({ "error": e.to_string() }));
        }
    }
}

async fn write_recording(cfg: &Config, address: &str, pcm: &[i16]) -> anyhow::Result<PathBuf> {
    tokio::fs::create_dir_all(&cfg.voice.audio_dir).await?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let path = cfg
        .voice
        .audio_dir
        .join(format!("siri-{}-{stamp}.wav", address.replace(':', "")));
    let wav = audio::wav_bytes(pcm, protocol::AUDIO_SAMPLE_RATE, protocol::AUDIO_CHANNELS);
    tokio::fs::write(&path, wav).await?;
    Ok(path)
}
