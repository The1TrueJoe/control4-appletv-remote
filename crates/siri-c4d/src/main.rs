//! Bridge Apple TV Siri Remotes to a Control4 room.
//!
//! Runs any number of remotes at once, each optionally driving a different
//! room, and serves a self-contained web UI for pairing and mapping.

mod config;
mod control4;
mod supervisor;
mod voice;
mod web;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use tokio::sync::{broadcast, RwLock};
use tracing::info;
use tracing_subscriber::EnvFilter;

use config::Config;
use supervisor::AppState;

#[derive(Parser)]
#[command(name = "siri-c4d", version, about = "Siri Remote to Control4 bridge")]
struct Cli {
    /// Configuration file.
    #[arg(short, long, default_value = config::DEFAULT_PATH)]
    config: PathBuf,

    /// Override the web UI bind address.
    #[arg(long)]
    bind: Option<String>,

    /// Override the web UI port.
    #[arg(long)]
    port: Option<u16>,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Scan for nearby remotes and print what was found.
    Scan {
        #[arg(long, default_value_t = 20)]
        seconds: u64,
    },
    /// Pair a remote and add it to the configuration.
    Pair {
        address: String,
        #[arg(long, default_value = "")]
        name: String,
        #[arg(long, default_value = "")]
        room: String,
    },
    /// Print the effective configuration and exit.
    ShowConfig,
    /// Check that the Control4 driver is reachable.
    Check,
    /// List Bluetooth adapters.
    Adapters,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_target(false)
        .init();

    let cli = Cli::parse();
    let mut cfg = Config::load(&cli.config)
        .with_context(|| format!("loading {}", cli.config.display()))?;
    if let Some(b) = &cli.bind {
        cfg.web.bind = b.clone();
    }
    if let Some(p) = cli.port {
        cfg.web.port = p;
    }

    match cli.command {
        Some(Command::Scan { seconds }) => {
            let a = cfg.bluetooth.adapter.trim().to_string();
            return cmd_scan(seconds, (!a.is_empty()).then_some(a.as_str())).await;
        }
        Some(Command::Pair { address, name, room }) => {
            return cmd_pair(&cli.config, cfg, address, name, room).await
        }
        Some(Command::Adapters) => return cmd_adapters().await,
        Some(Command::ShowConfig) => {
            println!("{}", toml::to_string_pretty(&cfg)?);
            return Ok(());
        }
        Some(Command::Check) => return cmd_check(&cfg).await,
        None => {}
    }

    // First run: write a config so there is something to edit in the UI.
    if !cli.config.exists() {
        if let Err(e) = cfg.save(&cli.config) {
            info!("could not write {}: {e}", cli.config.display());
        }
    }

    let (events, _) = broadcast::channel(256);
    let state = AppState {
        config: Arc::new(RwLock::new(cfg.clone())),
        config_path: cli.config.clone(),
        events,
        workers: Arc::new(RwLock::new(Default::default())),
    };

    supervisor::reconcile(&state).await;

    let addr: SocketAddr = format!("{}:{}", cfg.web.bind, cfg.web.port)
        .parse()
        .context("invalid web bind address")?;
    let app = web::router(state);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!("web UI on http://{addr}");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown())
        .await?;
    Ok(())
}

async fn shutdown() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let term = async {
        if let Ok(mut s) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            s.recv().await;
        }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = term => {},
    }
    info!("shutting down");
}

async fn cmd_scan(seconds: u64, adapter: Option<&str>) -> Result<()> {
    println!("scanning {seconds}s -- press a button on the remote, or hold Menu+Volume Up\n");
    let found = siri_remote::bluez::scan(seconds, adapter).await?;
    if found.is_empty() {
        println!("nothing found. The remote only advertises briefly after a button press.");
        return Ok(());
    }
    println!("{:<20} {:>5}  {:<8} {:<10} NAME", "ADDRESS", "RSSI", "TYPE", "PAIRED");
    for d in found {
        println!(
            "{:<20} {:>5}  {:<8} {:<10} {}{}",
            d.address,
            d.rssi.map(|r| r.to_string()).unwrap_or_else(|| "-".into()),
            d.addr_type,
            d.paired,
            d.name,
            if d.likely_remote { "   <-- likely a Siri Remote" } else { "" }
        );
    }
    Ok(())
}

async fn cmd_adapters() -> Result<()> {
    for a in siri_remote::bluez::adapters().await? {
        println!(
            "{:<8} {:<20} powered={} discovering={}",
            a.name, a.address, a.powered, a.discovering
        );
    }
    println!("\nMicrophone audio needs a Bluetooth 4.2+ adapter.");
    println!("Set it under [bluetooth] adapter in the config, or in the web UI.");
    Ok(())
}

async fn cmd_pair(
    path: &PathBuf,
    mut cfg: Config,
    address: String,
    name: String,
    room: String,
) -> Result<()> {
    let chosen = cfg.bluetooth.adapter.trim().to_string();
    let addr_type =
        siri_remote::bluez::pair(&address, (!chosen.is_empty()).then_some(chosen.as_str())).await?;
    let address = address.to_uppercase();
    if let Some(r) = cfg
        .remotes
        .iter_mut()
        .find(|r| r.address.eq_ignore_ascii_case(&address))
    {
        r.enabled = true;
        r.addr_type = addr_type.clone();
    } else {
        cfg.remotes.push(config::Remote {
            address: address.clone(),
            name: if name.is_empty() { address.clone() } else { name },
            enabled: true,
            addr_type: addr_type.clone(),
            room,
            mappings: config::default_mappings(),
        });
    }
    cfg.save(path)?;
    println!("paired {address} ({addr_type}) and saved to {}", path.display());
    Ok(())
}

async fn cmd_check(cfg: &Config) -> Result<()> {
    if cfg.control4.host.trim().is_empty() {
        println!("Control4 host is not configured yet");
        return Ok(());
    }
    let client = control4::Client::new(&cfg.control4)?;
    match client.health().await {
        Ok(v) => println!("control4 ok: {v}"),
        Err(e) => println!("control4 unreachable: {e}"),
    }
    match client.rooms().await {
        Ok(v) => println!("rooms: {v}"),
        Err(e) => println!("rooms unavailable: {e}"),
    }
    Ok(())
}
