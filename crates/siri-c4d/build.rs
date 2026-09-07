//! Compile the web UI and hand the output to `rust-embed`.
//!
//! The UI is TypeScript in `web/src`, compiled to `web/dist` and embedded in
//! the binary, so deployment stays a single file with no asset directory to
//! ship alongside it.
//!
//! Node is only needed when the UI sources change. If `web/dist` is already
//! present and current — as in a release tarball or an offline build — this
//! does nothing.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

fn main() {
    let web = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("web");
    let dist = web.join("dist");

    for watched in ["src", "css", "index.html", "package.json", "tsconfig.json"] {
        println!("cargo:rerun-if-changed={}", web.join(watched).display());
    }

    if !needs_build(&web, &dist) {
        return;
    }

    if !have("npm") {
        if dist.join("index.html").exists() {
            println!(
                "cargo:warning=web/dist is stale and npm is unavailable; \
                 embedding the existing build. Install Node and re-run to refresh."
            );
            return;
        }
        panic!(
            "the web UI has not been built and npm is unavailable.\n\
             Install Node, or build it elsewhere and copy web/dist into place:\n\
             \n    cd {}/web && npm ci && npm run build\n",
            env!("CARGO_MANIFEST_DIR")
        );
    }

    if !web.join("node_modules").exists() {
        run(&web, "npm", &["ci", "--no-audit", "--no-fund"])
            // `npm ci` needs a lockfile; fall back for a fresh checkout.
            .or_else(|_| run(&web, "npm", &["install", "--no-audit", "--no-fund"]))
            .expect("installing web UI dependencies");
    }
    run(&web, "npm", &["run", "build"]).expect("building the web UI");
}

fn have(program: &str) -> bool {
    Command::new(program)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn run(dir: &Path, program: &str, args: &[&str]) -> Result<(), String> {
    let out = Command::new(program)
        .args(args)
        .current_dir(dir)
        .output()
        .map_err(|e| format!("running {program}: {e}"))?;
    if out.status.success() {
        return Ok(());
    }
    Err(format!(
        "{program} {} failed:\n{}\n{}",
        args.join(" "),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    ))
}

/// Rebuild when any source is newer than the newest build output.
fn needs_build(web: &Path, dist: &Path) -> bool {
    let Some(built) = newest(&dist.join("js")) else {
        return true;
    };
    if !dist.join("index.html").exists() {
        return true;
    }
    ["src", "css", "index.html"]
        .iter()
        .filter_map(|p| newest(&web.join(p)))
        .any(|source| source > built)
}

/// Most recent modification time anywhere under `path`.
fn newest(path: &Path) -> Option<SystemTime> {
    let meta = std::fs::metadata(path).ok()?;
    if meta.is_file() {
        return meta.modified().ok();
    }
    std::fs::read_dir(path)
        .ok()?
        .filter_map(|e| newest(&e.ok()?.path()))
        .max()
}
