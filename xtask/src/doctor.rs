//! Prerequisite checks. Prints locations and versions, never credentials.

use std::path::Path;
use std::process::{Command, ExitCode};
use std::str::FromStr;
use std::time::Duration;

use sqlx::ConnectOptions;
use sqlx::postgres::PgConnectOptions;

use crate::env::Env;
use crate::root;

pub fn run(env: &Env) -> ExitCode {
    let root = root();
    let mut failed = false;
    let mut line = |required: bool, name: &str, result: Result<String, String>| match result {
        Ok(detail) => println!("[PASS] {name}: {detail}"),
        Err(fix) if required => {
            failed = true;
            println!("[FAIL] {name}: {fix}");
        }
        Err(fix) => println!("[WARN] {name}: {fix}"),
    };

    line(true, "rust toolchain", toolchain(&root));
    line(true, "wasm32 target", wasm_target(&root));
    line(
        true,
        "wasm-bindgen CLI",
        require_wasm_bindgen(&root).map(|v| v.to_string()),
    );
    line(
        false,
        ".env",
        if env.has_file() {
            Ok("loaded (real environment variables take precedence)".into())
        } else {
            Err("not found; `cp .env.example .env` for local defaults".into())
        },
    );
    match env.get("DATABASE_URL") {
        None => line(
            true,
            "DATABASE_URL",
            Err("not set; start infra/dev/compose.yaml and copy .env.example to .env".into()),
        ),
        Some(url) => {
            line(true, "DATABASE_URL", describe_database(&url));
            line(
                true,
                "PostgreSQL reachable",
                probe_database(&url).map(|()| "SELECT 1 ok".into()),
            );
        }
    }
    line(
        false,
        "container runtime (local PostgreSQL)",
        first_available(&["podman", "docker"], "--version").ok_or_else(|| {
            "neither podman nor docker found; any PostgreSQL 17 works via DATABASE_URL".into()
        }),
    );
    line(false, "Node.js >= 22 (verify web)", node_version());
    line(
        false,
        "Chrome/Chromium (verify web)",
        chrome_binary(env).ok_or_else(|| "not found; install Chrome or set VINK_CHROME".into()),
    );
    if failed {
        println!("\nSome prerequisites for `cargo xtask dev` are missing.");
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn output(program: &str, args: &[&str], dir: &Path) -> Option<String> {
    let out = Command::new(program)
        .args(args)
        .current_dir(dir)
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn pinned_toolchain(root: &Path) -> Option<String> {
    let text = std::fs::read_to_string(root.join("rust-toolchain.toml")).ok()?;
    text.lines()
        .find_map(|l| l.trim().strip_prefix("channel = "))
        .map(|v| v.trim_matches('"').to_string())
}

fn toolchain(root: &Path) -> Result<String, String> {
    let pinned = pinned_toolchain(root).ok_or("rust-toolchain.toml has no channel")?;
    let version = output("rustc", &["--version"], root).ok_or("rustc not found; install rustup")?;
    if version.starts_with(&format!("rustc {pinned} ")) {
        Ok(version)
    } else {
        Err(format!(
            "expected {pinned}, found `{version}`; run `rustup toolchain install` in the repository"
        ))
    }
}

fn wasm_target(root: &Path) -> Result<String, String> {
    let targets = output("rustup", &["target", "list", "--installed"], root).unwrap_or_default();
    if targets.lines().any(|t| t == "wasm32-unknown-unknown") {
        Ok("installed".into())
    } else {
        Err("missing; run `rustup target add wasm32-unknown-unknown`".into())
    }
}

fn locked_wasm_bindgen(root: &Path) -> Result<String, String> {
    let lock =
        std::fs::read_to_string(root.join("Cargo.lock")).map_err(|e| format!("Cargo.lock: {e}"))?;
    let mut lines = lock.lines();
    while let Some(line) = lines.next() {
        if line == "name = \"wasm-bindgen\""
            && let Some(version) = lines.next().and_then(|l| l.strip_prefix("version = "))
        {
            return Ok(version.trim_matches('"').to_string());
        }
    }
    Err("wasm-bindgen not found in Cargo.lock".into())
}

/// The CLI must equal the locked `wasm-bindgen` crate, or the generated JS will not load.
pub fn require_wasm_bindgen(root: &Path) -> Result<String, String> {
    let locked = locked_wasm_bindgen(root)?;
    let install = format!("cargo install wasm-bindgen-cli --version {locked} --locked");
    let found = output("wasm-bindgen", &["--version"], root)
        .ok_or_else(|| format!("not found; run `{install}`"))?;
    if found.split_whitespace().nth(1) == Some(locked.as_str()) {
        Ok(found)
    } else {
        Err(format!(
            "found `{found}`, Cargo.lock needs {locked}; run `{install}`"
        ))
    }
}

fn describe_database(url: &str) -> Result<String, String> {
    let options =
        PgConnectOptions::from_str(url).map_err(|_| "not a valid PostgreSQL URL".to_string())?;
    Ok(format!(
        "{}:{} database `{}` (credentials hidden)",
        options.get_host(),
        options.get_port(),
        options.get_database().unwrap_or("<default>")
    ))
}

pub fn probe_database(url: &str) -> Result<(), String> {
    let options = PgConnectOptions::from_str(url)
        .map_err(|_| "DATABASE_URL is not a valid PostgreSQL URL".to_string())?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    runtime.block_on(async move {
        let connect = async {
            let mut connection = options.connect().await?;
            sqlx::query("SELECT 1").execute(&mut connection).await?;
            Ok::<_, sqlx::Error>(())
        };
        match tokio::time::timeout(Duration::from_secs(5), connect).await {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) => Err(format!(
                "cannot connect ({}); is PostgreSQL running? `podman compose -f infra/dev/compose.yaml up -d`",
                redact(&error.to_string())
            )),
            Err(_) => Err("connection timed out after 5s".into()),
        }
    })
}

/// sqlx errors do not normally include the URL; strip anything URL-like defensively.
fn redact(message: &str) -> String {
    message
        .split_whitespace()
        .map(|word| if word.contains("://") { "<url>" } else { word })
        .collect::<Vec<_>>()
        .join(" ")
}

fn first_available(programs: &[&str], flag: &str) -> Option<String> {
    programs
        .iter()
        .find_map(|p| output(p, &[flag], Path::new(".")))
}

pub fn node_version() -> Result<String, String> {
    let version = output("node", &["--version"], Path::new("."))
        .ok_or("node not found; install Node.js 22 or newer")?;
    let major: u32 = version
        .trim_start_matches('v')
        .split('.')
        .next()
        .and_then(|m| m.parse().ok())
        .unwrap_or(0);
    if major >= 22 {
        Ok(version)
    } else {
        Err(format!(
            "found {version}; Node.js 22+ is needed for the built-in WebSocket client"
        ))
    }
}

pub fn chrome_binary(env: &Env) -> Option<String> {
    if let Some(path) = env.get("VINK_CHROME") {
        return Some(path);
    }
    [
        "google-chrome",
        "google-chrome-stable",
        "chromium",
        "chromium-browser",
        "chrome",
    ]
    .iter()
    .find(|p| output(p, &["--version"], Path::new(".")).is_some())
    .map(|p| p.to_string())
}
