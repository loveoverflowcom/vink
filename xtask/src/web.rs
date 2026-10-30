//! Process-level checks: the web lane (real server + headless Chrome) and binary lifecycle.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::doctor;
use crate::env::Env;
use crate::process::Report;
use crate::root;

/// Scratch database recreated by every `verify web` run; never a developer database.
const SCRATCH_DATABASE: &str = "vink_verify_web";
const DROP_SCRATCH: &str = "DROP DATABASE IF EXISTS vink_verify_web WITH (FORCE)";
const CREATE_SCRATCH: &str = "CREATE DATABASE vink_verify_web";
/// `dev_seed::SAMPLE_AUTHOR`: the browser acts as the owner of the sample work.
const SAMPLE_AUTHOR: &str = "11111111-1111-4111-8111-111111111111";

/// Child process that is terminated when dropped, so a failed check never leaks a server.
struct Guard(Option<Child>);

impl Guard {
    fn pid(&self) -> u32 {
        self.0.as_ref().map(Child::id).unwrap_or_default()
    }

    /// SIGTERM, then wait up to `timeout` for a clean exit.
    fn terminate(&mut self, timeout: Duration) -> Result<ExitStatus, String> {
        let Some(mut child) = self.0.take() else {
            return Err("process already stopped".into());
        };
        let pid = child.id().to_string();
        let _ = Command::new("kill").args(["-TERM", &pid]).status();
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
                return Ok(status);
            }
            if Instant::now() > deadline {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("did not exit within {timeout:?} after SIGTERM"));
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn binary(root: &Path, name: &str) -> PathBuf {
    root.join("target/debug").join(name)
}

/// Both binaries refuse to start without DATABASE_URL, with exit code 2 and a clear message.
pub fn config_rejection(root: &Path) -> Result<(), String> {
    for name in ["vink-app", "vink-worker"] {
        let out = Command::new(binary(root, name))
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .output()
            .map_err(|e| format!("{name}: {e}"))?;
        let stderr = String::from_utf8_lossy(&out.stderr);
        if out.status.code() != Some(2) || !stderr.contains("DATABASE_URL: missing") {
            return Err(format!(
                "{name}: expected exit 2 naming DATABASE_URL, got {} / {stderr}",
                out.status
            ));
        }
    }
    println!("==> invalid configuration: both binaries exit 2");
    Ok(())
}

/// Starts the worker, waits for its ready log line, sends SIGTERM and expects exit code 0.
pub fn worker_lifecycle(root: &Path, env: &Env) -> Result<(), String> {
    let url = env.get("DATABASE_URL").ok_or("DATABASE_URL is not set")?;
    let mut command = Command::new(binary(root, "vink-worker"));
    // Explicit local configuration, independent of any `.env` (CI has none).
    command
        .current_dir(root)
        .env("DATABASE_URL", url)
        .env("VINK_PROFILE", "local")
        .env("VINK_LOG_FORMAT", "text")
        .env_remove("VINK_DEV_PRINCIPAL")
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|e| format!("spawn vink-worker: {e}"))?;
    let stderr = child.stderr.take().expect("piped stderr");
    let mut guard = Guard(Some(child));
    let lines = stream_lines(stderr);
    let ready = wait_for_line(&lines, "vink-worker ready", Duration::from_secs(20))?;
    println!("==> {}", ready.trim());
    if ready.contains("unavailable") {
        return Err(format!("worker could not reach PostgreSQL: {ready}"));
    }
    let status = guard.terminate(Duration::from_secs(10))?;
    if !status.success() {
        return Err(format!("worker exited with {status}"));
    }
    wait_for_line(&lines, "vink-worker stopped", Duration::from_secs(2))?;
    Ok(())
}

fn stream_lines(stream: impl Read + Send + 'static) -> mpsc::Receiver<String> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stream).lines().map_while(Result::ok) {
            if sender.send(line).is_err() {
                break;
            }
        }
    });
    receiver
}

fn wait_for_line(
    lines: &mpsc::Receiver<String>,
    needle: &str,
    timeout: Duration,
) -> Result<String, String> {
    let deadline = Instant::now() + timeout;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        match lines.recv_timeout(left) {
            Ok(line) if strip_ansi(&line).contains(needle) => return Ok(strip_ansi(&line)),
            Ok(_) => {}
            Err(_) => return Err(format!("no `{needle}` log line within {timeout:?}")),
        }
    }
}

fn strip_ansi(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            for c in chars.by_ref() {
                if c.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Replaces the database name in a PostgreSQL URL.
fn with_database(url: &str, database: &str) -> Result<String, String> {
    let scheme_end = url.find("://").ok_or("DATABASE_URL has no scheme")? + 3;
    let (base, query) = match url.find('?') {
        Some(i) => (&url[..i], &url[i..]),
        None => (url, ""),
    };
    let path_start = base[scheme_end..]
        .find('/')
        .map(|i| scheme_end + i)
        .unwrap_or(base.len());
    Ok(format!("{}/{database}{query}", &base[..path_start]))
}

fn recreate_scratch_database(url: &str) -> Result<String, String> {
    let admin = url.to_string();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    runtime.block_on(async move {
        use sqlx::{Connection, Executor};
        let mut connection = sqlx::PgConnection::connect(&admin)
            .await
            .map_err(|e| format!("connect: {e}"))?;
        connection
            .execute(DROP_SCRATCH)
            .await
            .map_err(|e| format!("drop scratch database: {e}"))?;
        connection
            .execute(CREATE_SCRATCH)
            .await
            .map_err(|e| format!("create scratch database (needs CREATEDB): {e}"))?;
        Ok::<_, String>(())
    })?;
    with_database(url, SCRATCH_DATABASE)
}

fn drop_scratch_database(url: &str) {
    let admin = url.to_string();
    if let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        runtime.block_on(async move {
            use sqlx::{Connection, Executor};
            if let Ok(mut connection) = sqlx::PgConnection::connect(&admin).await {
                let _ = connection.execute(DROP_SCRATCH).await;
            }
        });
    }
}

fn free_port() -> Result<u16, String> {
    TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr())
        .map(|a| a.port())
        .map_err(|e| format!("no free port: {e}"))
}

/// Minimal HTTP/1.1 GET returning the status code; enough for readiness polling.
fn http_status(port: u16, path: &str) -> Option<u16> {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n"
    )
    .ok()?;
    let mut head = [0u8; 16];
    stream.read_exact(&mut head).ok()?;
    std::str::from_utf8(&head[9..12]).ok()?.parse().ok()
}

struct Server {
    port: u16,
    env: Vec<(String, String)>,
    log: PathBuf,
}

impl Server {
    fn command(&self, root: &Path, args: &[&str]) -> Command {
        let mut command = Command::new(binary(root, "vink-app"));
        command
            .current_dir(root)
            .args(args)
            .envs(self.env.iter().map(|(k, v)| (k, v)));
        command
    }

    fn start(&self, root: &Path) -> Result<Guard, String> {
        let log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log)
            .map_err(|e| format!("{}: {e}", self.log.display()))?;
        let child = self
            .command(root, &["serve"])
            .stdout(Stdio::null())
            .stderr(log)
            .spawn()
            .map_err(|e| format!("spawn vink-app: {e}"))?;
        let mut guard = Guard(Some(child));
        let deadline = Instant::now() + Duration::from_secs(30);
        while Instant::now() < deadline {
            if let Some(Ok(Some(status))) = guard.0.as_mut().map(Child::try_wait) {
                return Err(format!(
                    "vink-app exited early with {status}; see {}",
                    self.log.display()
                ));
            }
            if http_status(self.port, "/health/ready") == Some(200) {
                println!(
                    "==> vink-app ready on port {} (pid {})",
                    self.port,
                    guard.pid()
                );
                return Ok(guard);
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        Err(format!("not ready within 30s; see {}", self.log.display()))
    }
}

fn smoke(
    root: &Path,
    node_args: &[&str],
    chrome: &str,
    base: &str,
    state: &Path,
) -> Result<(), String> {
    let mut command = Command::new("node");
    command
        .current_dir(root)
        .arg(root.join("apps/vink-web/e2e/browser-smoke.mjs"))
        .args(["--base", base, "--chrome", chrome, "--state"])
        .arg(state)
        .args(node_args);
    crate::process::run(
        &mut command,
        &format!("browser smoke {}", node_args.join(" ")),
    )
}

pub fn verify(report: &mut Report, env: &Env) -> bool {
    let root = root();
    let Some(url) = env.get("DATABASE_URL") else {
        report.fail(
            "prerequisites",
            "DATABASE_URL is not set (PostgreSQL is required)",
        );
        return false;
    };
    let chrome = doctor::chrome_binary(env);
    if !report.step(
        "prerequisites (PostgreSQL, wasm-bindgen, Node 22+, Chrome)",
        || {
            doctor::probe_database(&url)?;
            doctor::require_wasm_bindgen(&root)?;
            doctor::node_version()?;
            chrome
                .as_ref()
                .map(|_| ())
                .ok_or_else(|| "Chrome/Chromium not found; set VINK_CHROME".to_string())
        },
    ) {
        return false;
    }
    let chrome = chrome.unwrap_or_default();
    if !report.step("build server + browser bundle", || crate::build(env, false)) {
        return false;
    }

    let out_dir = root.join("target/verify-web");
    let _ = std::fs::remove_dir_all(&out_dir);
    if let Err(e) = std::fs::create_dir_all(&out_dir) {
        report.fail("prepare target/verify-web", &e.to_string());
        return false;
    }
    let mut scratch_url = String::new();
    if !report.step("scratch database: create, migrate, seed", || {
        scratch_url = recreate_scratch_database(&url)?;
        Ok(())
    }) {
        return false;
    }
    let port = match free_port() {
        Ok(port) => port,
        Err(e) => {
            report.fail("free port", &e);
            return false;
        }
    };
    let origin = format!("http://127.0.0.1:{port}");
    let server = Server {
        port,
        env: vec![
            ("DATABASE_URL".into(), scratch_url.clone()),
            ("VINK_PROFILE".into(), "local".into()),
            ("VINK_BIND".into(), format!("127.0.0.1:{port}")),
            ("VINK_PUBLIC_ORIGIN".into(), origin.clone()),
            (
                "VINK_SITE_ROOT".into(),
                root.join("target/site").display().to_string(),
            ),
            ("VINK_DEV_PRINCIPAL".into(), SAMPLE_AUTHOR.into()),
            ("VINK_LOG_FORMAT".into(), "text".into()),
        ],
        log: out_dir.join("server.log"),
    };
    let ok = run_web_checks(
        report,
        &root,
        &server,
        &chrome,
        &origin,
        &out_dir.join("smoke-state.json"),
    );
    drop_scratch_database(&url);
    ok
}

fn run_web_checks(
    report: &mut Report,
    root: &Path,
    server: &Server,
    chrome: &str,
    origin: &str,
    state: &Path,
) -> bool {
    for step in ["migrate", "dev-seed"] {
        let mut command = server.command(root, &[step]);
        if let Err(e) = crate::process::run(&mut command, &format!("vink-app {step} (scratch)")) {
            report.fail("scratch database: migrate + seed", &e);
            return false;
        }
    }
    let mut guard = match server.start(root) {
        Ok(guard) => guard,
        Err(e) => {
            report.fail("server starts and reports ready", &e);
            return false;
        }
    };
    if !report.step(
        "SSR HTML, hydration, client navigation, save, reload, conflict",
        || smoke(root, &["--phase", "main"], chrome, origin, state),
    ) {
        return false;
    }
    if !report.step("server stops gracefully on SIGTERM", || {
        let status = guard.terminate(Duration::from_secs(15))?;
        status
            .success()
            .then_some(())
            .ok_or(format!("exited with {status}"))
    }) {
        return false;
    }
    let mut restarted = match server.start(root) {
        Ok(guard) => guard,
        Err(e) => {
            report.fail("server restarts", &e);
            return false;
        }
    };
    let ok = report.step(
        "confirmed draft survives server restart (browser reload)",
        || smoke(root, &["--phase", "after-restart"], chrome, origin, state),
    );
    let _ = restarted.terminate(Duration::from_secs(15));
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_only_the_database_name() {
        assert_eq!(
            with_database("postgres://u:p@h:5432/vink?sslmode=disable", "x").unwrap(),
            "postgres://u:p@h:5432/x?sslmode=disable"
        );
        assert_eq!(
            with_database("postgres://u@h", "x").unwrap(),
            "postgres://u@h/x"
        );
    }

    #[test]
    fn strips_terminal_colors() {
        assert_eq!(strip_ansi("\u{1b}[32m INFO\u{1b}[0m ready"), " INFO ready");
    }
}
