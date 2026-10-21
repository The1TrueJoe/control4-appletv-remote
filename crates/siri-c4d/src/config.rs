//! On-disk configuration.
//!
//! Everything the web UI edits lives here. The file is written atomically so a
//! crash mid-save cannot leave a truncated config behind.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

pub const DEFAULT_PATH: &str = "/etc/siri-c4d/config.toml";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub web: Web,
    pub bluetooth: Bluetooth,
    pub control4: Control4,
    pub voice: Voice,
    pub remotes: Vec<Remote>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            web: Web::default(),
            bluetooth: Bluetooth::default(),
            control4: Control4::default(),
            voice: Voice::default(),
            remotes: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Web {
    pub bind: String,
    pub port: u16,
}

impl Default for Web {
    fn default() -> Self {
        Web {
            bind: "0.0.0.0".into(),
            port: 8752,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Bluetooth {
    /// Adapter to use, by BD address or kernel name (`hci1`). Empty means
    /// BlueZ's default.
    ///
    /// Naming one matters when a USB dongle sits alongside a built-in radio:
    /// microphone audio needs a Bluetooth 4.2+ controller, and a Raspberry
    /// Pi 3's built-in radio is 4.1 and will never carry it.
    pub adapter: String,
}

impl Default for Bluetooth {
    fn default() -> Self {
        Bluetooth { adapter: String::new() }
    }
}

/// Connection to the Control4 Room Control Webhook root driver.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Control4 {
    pub host: String,
    pub port: u16,
    /// Root driver password, if one is set. Sent as `Authorization: Bearer`.
    pub password: String,
    /// Room id or name used when a remote does not name its own. Leave empty
    /// when exactly one room is linked — the driver then infers it.
    pub room: String,
    pub timeout_ms: u64,
}

impl Default for Control4 {
    fn default() -> Self {
        Control4 {
            host: String::new(),
            port: 5080,
            password: String::new(),
            room: String::new(),
            timeout_ms: 2500,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Voice {
    pub enabled: bool,
    /// Where Siri recordings are written, as 16 kHz mono WAV.
    pub audio_dir: PathBuf,
    /// Turn off to receive voice events without keeping the audio.
    pub keep_recordings: bool,
}

impl Default for Voice {
    fn default() -> Self {
        Voice {
            enabled: true,
            audio_dir: PathBuf::from("/var/lib/siri-c4d/audio"),
            keep_recordings: true,
        }
    }
}

/// What a control on the remote should do.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Action {
    /// Fire a room command on press.
    Command { command: String },
    /// Hold-style command: one command on press, another on release. This is
    /// how Control4 does continuous volume and channel ramps.
    Hold { start: String, stop: String },
    /// Run a source preset configured on the room child driver.
    Preset { preset: String },
    /// Trigger a custom room button.
    Button {
        button: String,
        #[serde(default = "tap")]
        action: String,
    },
    /// Explicitly do nothing.
    None,
}

fn tap() -> String {
    "tap".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Remote {
    pub address: String,
    pub name: String,
    pub enabled: bool,
    /// "public" or "random"; BlueZ reports which.
    pub addr_type: String,
    /// Overrides [`Control4::room`] for this remote, so two remotes on one Pi
    /// can drive different rooms.
    pub room: String,
    /// Control name (see [`siri_remote::Button`], plus SWIPE_UP/DOWN/LEFT/RIGHT)
    /// mapped to an action.
    pub mappings: BTreeMap<String, Action>,
}

impl Default for Remote {
    fn default() -> Self {
        Remote {
            address: String::new(),
            name: String::new(),
            enabled: true,
            addr_type: "public".into(),
            room: String::new(),
            mappings: default_mappings(),
        }
    }
}

/// Sensible Siri-Remote-to-Control4 defaults.
///
/// Volume uses the `START_`/`STOP_` pair so holding the button ramps, which is
/// what those commands exist for.
pub fn default_mappings() -> BTreeMap<String, Action> {
    let mut m = BTreeMap::new();
    let cmd = |c: &str| Action::Command { command: c.into() };
    m.insert("MENU".into(), cmd("MENU"));
    m.insert("PLAY_PAUSE".into(), cmd("PLAYPAUSE"));
    m.insert("TOUCHPAD".into(), cmd("ENTER"));
    m.insert("TOUCHPAD_2".into(), cmd("BACK"));
    m.insert("AIRPLAY".into(), cmd("ROOM_OFF"));
    m.insert(
        "VOLUME_UP".into(),
        Action::Hold {
            start: "START_VOL_UP".into(),
            stop: "STOP_VOL_UP".into(),
        },
    );
    m.insert(
        "VOLUME_DOWN".into(),
        Action::Hold {
            start: "START_VOL_DOWN".into(),
            stop: "STOP_VOL_DOWN".into(),
        },
    );
    m.insert("SWIPE_UP".into(), cmd("UP"));
    m.insert("SWIPE_DOWN".into(), cmd("DOWN"));
    m.insert("SWIPE_LEFT".into(), cmd("LEFT"));
    m.insert("SWIPE_RIGHT".into(), cmd("RIGHT"));
    // Siri is handled by the voice pipeline, not as a button press.
    m.insert("SIRI".into(), Action::None);
    m
}

impl Config {
    pub fn load(path: &Path) -> Result<Config> {
        if !path.exists() {
            return Ok(Config::default());
        }
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading {}", path.display()))?;
        toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
    }

    /// Write atomically: a crash mid-write must not truncate the config.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = toml::to_string_pretty(self)?;
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    pub fn remote(&self, address: &str) -> Option<&Remote> {
        self.remotes
            .iter()
            .find(|r| r.address.eq_ignore_ascii_case(address))
    }

    pub fn room_for(&self, remote: &Remote) -> String {
        if remote.room.trim().is_empty() {
            self.control4.room.clone()
        } else {
            remote.room.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapter_defaults_to_automatic() {
        assert!(Config::default().bluetooth.adapter.is_empty());
    }

    #[test]
    fn defaults_round_trip_through_toml() {
        let c = Config::default();
        let text = toml::to_string_pretty(&c).unwrap();
        let back: Config = toml::from_str(&text).unwrap();
        assert_eq!(back.web.port, c.web.port);
        assert_eq!(back.control4.port, 5080);
    }

    #[test]
    fn default_mappings_cover_every_physical_control() {
        let m = default_mappings();
        for name in [
            "MENU", "PLAY_PAUSE", "TOUCHPAD", "AIRPLAY", "VOLUME_UP", "VOLUME_DOWN", "SIRI",
        ] {
            assert!(m.contains_key(name), "missing mapping for {name}");
        }
    }

    #[test]
    fn volume_uses_hold_semantics() {
        match default_mappings().get("VOLUME_UP") {
            Some(Action::Hold { start, stop }) => {
                assert_eq!(start, "START_VOL_UP");
                assert_eq!(stop, "STOP_VOL_UP");
            }
            other => panic!("expected hold action, got {other:?}"),
        }
    }

    #[test]
    fn per_remote_room_overrides_global() {
        let mut c = Config::default();
        c.control4.room = "1".into();
        let mut r = Remote::default();
        assert_eq!(c.room_for(&r), "1");
        r.room = "Kitchen".into();
        assert_eq!(c.room_for(&r), "Kitchen");
    }

    #[test]
    fn actions_serialise_tagged() {
        let a = Action::Command { command: "MENU".into() };
        let j = serde_json::to_string(&a).unwrap();
        assert!(j.contains("\"kind\":\"command\""), "got {j}");
    }
}
