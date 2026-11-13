//! Owned children, readiness and process-level checks, shared by dev and verification.
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};
/// Child process that is terminated when dropped, so a failed check never leaks a server.
pub struct Guard(pub Option<Child>);

impl Guard {
    pub fn pid(&self) -> u32 {
        self.0.as_ref().map(Child::id).unwrap_or_default()
    }

    /// SIGTERM, then wait up to `timeout` for a clean exit.
    pub fn terminate(&mut self, timeout: Duration) -> Result<ExitStatus, String> {
        let Some(mut child) = self.0.take() else {
            return Err("process already stopped".into());
        };
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            return Ok(status);
        }
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
        if self.0.is_some() {
            let _ = self.terminate(Duration::from_secs(3));
        }
    }
}

pub fn binary(root: &Path, name: &str) -> PathBuf {
    root.join("target/debug").join(name)
}

/// Without their credential the binaries exit 2 naming it: `serve` and the worker need
/// DATABASE_URL; `migrate` needs VINK_MIGRATE_DATABASE_URL even when DATABASE_URL is set.
pub fn config_rejection(root: &Path) -> Result<(), String> {
    let cases = [
        ("vink-app", None, "DATABASE_URL: missing"),
        ("vink-worker", None, "DATABASE_URL: missing"),
        (
            "vink-app",
            Some("migrate"),
            "VINK_MIGRATE_DATABASE_URL: missing",
        ),
    ];
    for (name, command, needle) in cases {
        let mut process = Command::new(binary(root, name));
        process
            .args(command)
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default());
        if command.is_some() {
            // The runtime credential must not stand in for the migration credential.
            process.env("DATABASE_URL", "postgres://runtime:unused@127.0.0.1:1/vink");
        }
        let out = process.output().map_err(|e| format!("{name}: {e}"))?;
        let stderr = String::from_utf8_lossy(&out.stderr);
        if out.status.code() != Some(2) || !stderr.contains(needle) || stderr.contains("unused@") {
            return Err(format!(
                "{name} {}: expected exit 2 naming `{needle}`, got {} / {stderr}",
                command.unwrap_or_default(),
                out.status
            ));
        }
    }
    println!("==> invalid configuration: serve, worker and migrate exit 2");
    Ok(())
}

/// Starts the worker, waits for its start log line, checks it reports `schema` and, when the
/// schema is current, a reachable discussion outbox (`outbox="ready"`, DISC-04) under this role;
/// sends SIGTERM and expects exit code 0.
pub fn worker_lifecycle(root: &Path, url: &str, schema: &str) -> Result<(), String> {
    let mut command = Command::new(binary(root, "vink-worker"));
    crate::env::clear_capabilities(&mut command);
    // Explicit local configuration, independent of any `.env` (CI has none).
    command
        .current_dir(root)
        .env("DATABASE_URL", url)
        .env("VINK_PROFILE", "local")
        .env("VINK_LOG_FORMAT", "text")
        // Lifecycle assertions consume INFO logs regardless of the invoking shell's filter.
        .env("RUST_LOG", "info,sqlx=warn")
        .env_remove("VINK_DEV_PRINCIPAL")
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|e| format!("spawn vink-worker: {e}"))?;
    let stderr = child.stderr.take().expect("piped stderr");
    let mut guard = Guard(Some(child));
    let lines = stream_lines(stderr);
    let ready = wait_for_line(&lines, "vink-worker started", Duration::from_secs(20))?;
    println!("==> {}", ready.trim());
    if !ready.contains(schema) {
        return Err(format!("worker did not report `{schema}`: {ready}"));
    }
    if schema.contains("current") && !ready.contains("outbox=\"ready\"") {
        return Err(format!("worker did not report a reachable outbox: {ready}"));
    }
    let status = guard.terminate(Duration::from_secs(10))?;
    if !status.success() {
        return Err(format!("worker exited with {status}"));
    }
    wait_for_line(&lines, "vink-worker stopped", Duration::from_secs(2))?;
    Ok(())
}

pub fn stream_lines(stream: impl Read + Send + 'static) -> mpsc::Receiver<String> {
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

pub fn wait_for_line(
    lines: &mpsc::Receiver<String>,
    needle: &str,
    timeout: Duration,
) -> Result<String, String> {
    let deadline = Instant::now() + timeout;
    loop {
        check_cancelled()?;
        let left = deadline
            .saturating_duration_since(Instant::now())
            .min(Duration::from_millis(200));
        match lines.recv_timeout(left) {
            Ok(line) if strip_ansi(&line).contains(needle) => return Ok(strip_ansi(&line)),
            Ok(_) => {}
            Err(mpsc::RecvTimeoutError::Timeout) if Instant::now() < deadline => {}
            Err(_) => return Err(format!("no `{needle}` log line within {timeout:?}")),
        }
    }
}

pub fn strip_ansi(line: &str) -> String {
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

pub fn free_port() -> Result<u16, String> {
    TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr())
        .map(|a| a.port())
        .map_err(|e| format!("no free port: {e}"))
}

/// Minimal HTTP/1.1 GET returning the status code; enough for readiness polling.
pub fn http_status(port: u16, path: &str) -> Option<u16> {
    http_status_at(std::net::SocketAddr::from(([127, 0, 0, 1], port)), path)
}

pub fn http_status_at(address: std::net::SocketAddr, path: &str) -> Option<u16> {
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(1)).ok()?;
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

/// Advisory file lock, released by the OS even on a crash. The file is never unlinked: unlinking
/// a locked inode would let another run lock a different file at the same path.
pub struct Lock(std::fs::File);
impl Lock {
    pub fn acquire(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(path)
            .map_err(|e| e.to_string())?;
        file.try_lock().map_err(|_|format!("resource is already in use: {}; stop the owning run first (do not delete the lock file)",path.display()))?;
        Ok(Self(file))
    }
}
impl Drop for Lock {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}

/// All fixed-name scratch adapters share this database-level lock, including other worktrees.
/// It protects setup/cleanup only across runs; test concurrency inside one run is unchanged.
pub struct ScratchLock {
    _connection: sqlx::PgConnection,
    _runtime: tokio::runtime::Runtime,
}
impl ScratchLock {
    pub fn acquire(url: &str) -> Result<Self, String> {
        use sqlx::Connection;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| e.to_string())?;
        let lock_url = crate::pgurl::PgUrl::parse(url)?.with_database("postgres");
        let connection = runtime.block_on(async {
            let mut connection = sqlx::PgConnection::connect(&lock_url)
                .await
                .map_err(|_| "cannot connect to scratch lock database `postgres`")?;
            let locked: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock(184, 1)")
                .fetch_one(&mut connection)
                .await
                .map_err(|_| "cannot acquire scratch database lock")?;
            if !locked {
                return Err(
                    "another xtask verification owns scratch resources on this PostgreSQL server"
                        .to_string(),
                );
            }
            Ok(connection)
        })?;
        Ok(Self {
            _connection: connection,
            _runtime: runtime,
        })
    }
}

static CANCELLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub fn cancelled() -> bool {
    CANCELLED.load(std::sync::atomic::Ordering::Relaxed)
}
pub fn check_cancelled() -> Result<(), String> {
    if cancelled() {
        Err("interrupted; owned resources stopped, logs retained".into())
    } else {
        Ok(())
    }
}
pub fn install_signals() -> Result<(), String> {
    let (tx, rx) = mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build();
        match runtime {
            Err(e) => {
                let _ = tx.send(Err(e.to_string()));
            }
            Ok(runtime) => runtime.block_on(async {
                use tokio::signal::unix::{SignalKind, signal};
                let signals = signal(SignalKind::interrupt())
                    .and_then(|int| signal(SignalKind::terminate()).map(|term| (int, term)));
                match signals {
                    Err(e) => {
                        let _ = tx.send(Err(e.to_string()));
                    }
                    Ok((mut int, mut term)) => {
                        let _ = tx.send(Ok(()));
                        tokio::select! { _=int.recv()=>{}, _=term.recv()=>{} }
                        CANCELLED.store(true, std::sync::atomic::Ordering::Relaxed);
                    }
                }
            }),
        }
    });
    rx.recv().map_err(|e| e.to_string())?
}

pub fn run_directory(root: &Path, label: &str) -> Result<PathBuf, String> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let path = root
        .join("target")
        .join(label)
        .join(format!("{stamp}-{}", std::process::id()));
    std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    println!("==> logs/evidence: {}", path.display());
    Ok(path)
}

/// Short Unix socket path (macOS has a 104-byte limit), created exclusively and removed only
/// after the child guards that use it have dropped. Evidence/outbox files remain in the run dir.
pub struct SocketDir(pub PathBuf);
impl SocketDir {
    pub fn new() -> Result<Self, String> {
        use std::os::unix::fs::DirBuilderExt;
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos();
        let dir = PathBuf::from(format!("/tmp/vink-s-{}-{stamp:x}", std::process::id()));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&dir)
            .map_err(|e| e.to_string())?;
        Ok(Self(dir))
    }
}
impl Drop for SocketDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[cfg(test)]
mod ownership_tests {
    use super::*;
    #[test]
    #[ignore = "requires VINK_TEST_DATABASE_URL on a local PostgreSQL test server"]
    fn scratch_lock_serializes_runs_and_releases_on_drop() {
        let url = std::env::var(crate::TEST_DATABASE_URL).expect("test PostgreSQL URL");
        let first = ScratchLock::acquire(&url).unwrap();
        assert!(ScratchLock::acquire(&url).is_err());
        drop(first);
        drop(ScratchLock::acquire(&url).unwrap());
    }

    #[test]
    fn duplicate_lock_refuses_and_release_does_not_require_pid_files() {
        let dir = std::env::temp_dir().join(format!("vink-lock-test-{}", std::process::id()));
        let path = dir.join("lock");
        let first = Lock::acquire(&path).unwrap();
        assert!(Lock::acquire(&path).is_err());
        drop(first);
        drop(Lock::acquire(&path).unwrap());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn guard_does_not_signal_a_reaped_child() {
        let child = Command::new("sh").args(["-c", "exit 7"]).spawn().unwrap();
        let mut guard = Guard(Some(child));
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(
            guard.terminate(Duration::from_secs(1)).unwrap().code(),
            Some(7)
        );
    }
}
