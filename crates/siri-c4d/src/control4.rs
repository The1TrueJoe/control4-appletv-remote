//! Client for the Control4 Room Control Webhook driver.
//!
//! Matches the driver's HTTP API: `/health`, `/rooms`, `/commands`,
//! `/presets`, `/buttons`, `/command`, `/preset`, `/button`. Auth, when a root
//! password is set, goes in an `Authorization: Bearer` header.

use std::time::Duration;

use anyhow::{anyhow, Result};
use serde_json::{json, Value};

use crate::config::{Action, Control4 as C4Config};

#[derive(Clone)]
pub struct Client {
    http: reqwest::Client,
    base: String,
    password: String,
}

impl Client {
    pub fn new(cfg: &C4Config) -> Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_millis(cfg.timeout_ms.max(250)))
            .build()?;
        Ok(Client {
            http,
            base: format!("http://{}:{}", cfg.host.trim(), cfg.port),
            password: cfg.password.clone(),
        })
    }

    pub fn configured(&self) -> bool {
        !self.base.contains("://:")
    }

    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        let b = self.http.request(method, format!("{}{}", self.base, path));
        if self.password.is_empty() {
            b
        } else {
            b.bearer_auth(&self.password)
        }
    }

    async fn get_json(&self, path: &str) -> Result<Value> {
        if !self.configured() {
            return Err(anyhow!("Control4 host is not configured"));
        }
        let r = self.request(reqwest::Method::GET, path).send().await?;
        let status = r.status();
        let body: Value = r.json().await.unwrap_or(Value::Null);
        if !status.is_success() {
            return Err(anyhow!("{path} returned HTTP {status}: {body}"));
        }
        Ok(body)
    }

    async fn post_json(&self, path: &str, body: Value) -> Result<Value> {
        if !self.configured() {
            return Err(anyhow!("Control4 host is not configured"));
        }
        let r = self
            .request(reqwest::Method::POST, path)
            .json(&body)
            .send()
            .await?;
        let status = r.status();
        let out: Value = r.json().await.unwrap_or(Value::Null);
        if !status.is_success() {
            return Err(anyhow!("{path} returned HTTP {status}: {out}"));
        }
        Ok(out)
    }

    /// `/health` needs no auth, so it is the honest reachability probe.
    pub async fn health(&self) -> Result<Value> {
        self.get_json("/health").await
    }

    pub async fn rooms(&self) -> Result<Value> {
        self.get_json("/rooms").await
    }

    pub async fn commands(&self) -> Result<Value> {
        self.get_json("/commands").await
    }

    pub async fn presets(&self) -> Result<Value> {
        self.get_json("/presets").await
    }

    pub async fn buttons(&self) -> Result<Value> {
        self.get_json("/buttons").await
    }

    pub async fn command(&self, room: &str, command: &str) -> Result<Value> {
        let mut body = json!({ "command": command });
        // The driver infers the room when exactly one is linked, so only send
        // it when we actually have one.
        if !room.trim().is_empty() {
            body["room"] = json!(room);
        }
        self.post_json("/command", body).await
    }

    pub async fn preset(&self, room: &str, preset: &str) -> Result<Value> {
        let mut body = json!({ "preset": preset });
        if !room.trim().is_empty() {
            body["room"] = json!(room);
        }
        self.post_json("/preset", body).await
    }

    pub async fn button(&self, room: &str, button: &str, action: &str) -> Result<Value> {
        let mut body = json!({ "button": button, "action": action });
        if !room.trim().is_empty() {
            body["room"] = json!(room);
        }
        self.post_json("/button", body).await
    }

    /// Dispatch a mapped action. `pressed` distinguishes press from release so
    /// [`Action::Hold`] can send its start and stop commands.
    pub async fn dispatch(&self, room: &str, action: &Action, pressed: bool) -> Result<Option<Value>> {
        match action {
            Action::None => Ok(None),
            Action::Command { command } => {
                if pressed {
                    self.command(room, command).await.map(Some)
                } else {
                    Ok(None)
                }
            }
            Action::Hold { start, stop } => {
                let c = if pressed { start } else { stop };
                self.command(room, c).await.map(Some)
            }
            Action::Preset { preset } => {
                if pressed {
                    self.preset(room, preset).await.map(Some)
                } else {
                    Ok(None)
                }
            }
            Action::Button { button, action } => {
                if pressed {
                    self.button(room, button, action).await.map(Some)
                } else {
                    Ok(None)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unconfigured_host_is_detected() {
        let c = Client::new(&C4Config::default()).unwrap();
        assert!(!c.configured());
    }

    #[test]
    fn configured_host_builds_a_base_url() {
        let cfg = C4Config {
            host: "10.0.0.5".into(),
            port: 5080,
            ..Default::default()
        };
        let c = Client::new(&cfg).unwrap();
        assert!(c.configured());
        assert_eq!(c.base, "http://10.0.0.5:5080");
    }
}
