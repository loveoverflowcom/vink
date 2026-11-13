//! Execute the typed local plan. No helper may append a full build or infer auth from `.env`.
use crate::{
    build, doctor,
    env::{self, Env},
    identity,
    lifecycle::{self, Guard, Lock},
    plan::*,
    process, root, ui_preview, web,
};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub fn execute(plan: &Plan, env: &Env) -> Result<(), String> {
    println!("{}", plan.describe());
    let _lock = Lock::acquire(&root().join("target/.xtask-artifacts.lock"))?;
    if plan.request.operation == Operation::Verify && plan.request.target == Target::Web {
        let mut report = process::Report::default();
        let passed = web::verify_selected(&mut report, env, plan);
        let _ = report.finish();
        return if passed {
            Ok(())
        } else {
            Err("selected Web verification failed (later checks NOT_RUN)".into())
        };
    }
    doctor::scoped(plan, env)?;
    if let Target::Unit(owner) = plan.request.target {
        return unit(owner, plan.request.filter.as_deref());
    }
    // Validate endpoint/capability configuration before building or migrating anything.
    let config =
        if plan.request.operation == Operation::Dev && plan.request.target != Target::Preview {
            Some(Local::resolve(plan, env)?)
        } else {
            None
        };
    if let Some(config) = &config {
        config.available(plan)?;
    }
    if plan.request.target == Target::Preview {
        if plan.request.operation == Operation::Verify {
            return if ui_preview::verify(env, "leptos") == std::process::ExitCode::SUCCESS {
                Ok(())
            } else {
                Err("preview verification failed".into())
            };
        }
        return ui_preview::run(
            env,
            "leptos",
            if plan.request.operation == Operation::Build {
                "build"
            } else {
                "run"
            },
        );
    }
    artifacts(plan)?;
    if let Some(config) = config {
        run_local(plan, env, &config)?;
    }
    Ok(())
}

pub fn artifacts(plan: &Plan) -> Result<(), String> {
    for artifact in &plan.artifacts {
        match artifact {
            Artifact::Bundle => build::bundle(plan.request.release)?,
            Artifact::Preview => return Err("preview build belongs to its adapter".into()),
            other => build::binary(*other, plan.request.release)?,
        }
    }
    Ok(())
}

fn unit(owner: UnitOwner, filter: Option<&str>) -> Result<(), String> {
    let mut args = vec!["test", "--locked", "-p", owner.package()];
    args.extend_from_slice(owner.test_targets());
    let mut list = process::cargo(&root());
    list.args(&args);
    if let Some(filter) = filter {
        list.arg(filter);
    }
    list.args(["--", "--list"]);
    env::clear_capabilities(&mut list);
    let out = process::output(&mut list, "discover selected unit tests")?;
    if !out.status.success() {
        eprintln!("{}", String::from_utf8_lossy(&out.stderr));
        return Err("unit test discovery failed".into());
    }
    let count = String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|line| line.ends_with(": test"))
        .count();
    if count == 0 {
        return Err("unit selection matched zero tests; refusing empty PASS".into());
    }
    println!(
        "==> selected {count} unit/contract tests for {}; database/browser checks NOT_SELECTED",
        owner.package()
    );
    let mut command = process::cargo(&root());
    command.args(&args);
    if let Some(filter) = filter {
        command.arg(filter);
    }
    env::clear_capabilities(&mut command);
    process::run(&mut command, "selected unit tests")
}

struct Local {
    bind: SocketAddr,
    origin: String,
    socket: PathBuf,
    outbox: PathBuf,
    uid: String,
}
impl Local {
    fn resolve(plan: &Plan, env: &Env) -> Result<Self, String> {
        if env.get("VINK_PROFILE").is_some_and(|p| p != "local") {
            return Err(
                "dev is local-only; VINK_PROFILE must be local (production is never unlocked)"
                    .into(),
            );
        }
        let app = plan.services.contains(&Service::App);
        let bind: SocketAddr = app
            .then(|| env.get("VINK_BIND"))
            .flatten()
            .unwrap_or_else(|| "127.0.0.1:3000".into())
            .parse()
            .map_err(|_| "VINK_BIND must be a loopback socket address")?;
        if !bind.ip().is_loopback() || bind.port() == 0 {
            return Err("dev requires a nonzero loopback VINK_BIND port".into());
        }
        let origin = app
            .then(|| env.get("VINK_PUBLIC_ORIGIN"))
            .flatten()
            .unwrap_or_else(|| format!("http://{bind}"));
        if origin != format!("http://{bind}") {
            return Err("dev requires VINK_PUBLIC_ORIGIN to equal http://VINK_BIND".into());
        }
        let socket = root().join(
            env.get("VINK_IDENTITY_SOCKET")
                .unwrap_or_else(|| "target/identity/identity.sock".into()),
        );
        let outbox = root().join(
            env.get("VINK_IDENTITY_LOCAL_OUTBOX")
                .unwrap_or_else(|| "target/identity/outbox".into()),
        );
        if plan.request.auth == Auth::Fixture && env.get("VINK_DEV_PRINCIPAL").is_none() {
            return Err("--auth fixture requires explicit VINK_DEV_PRINCIPAL; no automatic sample principal".into());
        }
        if plan.services.contains(&Service::Identity) {
            if env
                .get("VINK_IDENTITY_MAIL_TRANSPORT")
                .is_some_and(|s| s != "local-outbox")
            {
                return Err(
                    "local Identity requires local-outbox; SMTP is not enabled by dev".into(),
                );
            }
            if env
                .get("VINK_IDENTITY_AUTHORITY")
                .is_some_and(|s| s != "native")
            {
                return Err(
                    "--auth identity requires native authority (username/password and email codes)"
                        .into(),
                );
            }
        }
        // Read-only UID discovery; only the executor calls this, never --plan.
        let uid = std::os::unix::fs::MetadataExt::uid(
            &std::fs::metadata(root()).map_err(|e| e.to_string())?,
        )
        .to_string();
        Ok(Self {
            bind,
            origin,
            socket,
            outbox,
            uid,
        })
    }
    fn available(&self, plan: &Plan) -> Result<(), String> {
        if plan.services.contains(&Service::App) {
            let socket = if self.bind.is_ipv4() {
                tokio::net::TcpSocket::new_v4()
            } else {
                tokio::net::TcpSocket::new_v6()
            }
            .map_err(|e| e.to_string())?;
            socket.set_reuseaddr(true).map_err(|e| e.to_string())?;
            socket
                .bind(self.bind)
                .map_err(|_| format!("{} is occupied; no process was stopped", self.bind))?;
        }
        if plan.services.contains(&Service::Identity) && self.socket.symlink_metadata().is_ok() {
            return Err(format!(
                "Identity socket already exists: {}; stop its owner or inspect a stale socket manually",
                self.socket.display()
            ));
        }
        Ok(())
    }
}

/// Least-capability environment. Migration URLs are never passed to serve/worker/browser.
fn command(plan: &Plan, env: &Env, config: &Local, artifact: Artifact, operation: &str) -> Command {
    let bin = match artifact {
        Artifact::Identity => "vink-identity",
        Artifact::Worker => "vink-worker",
        _ => "vink-app",
    };
    let mut cmd = Command::new(lifecycle::binary(&root(), bin));
    cmd.current_dir(root());
    if !operation.is_empty() {
        cmd.arg(operation);
    }
    env::clear_capabilities(&mut cmd);
    cmd.env("VINK_LOG_FORMAT", "text")
        .env("VINK_PROFILE", "local");
    if operation == "migrate" {
        env.copy(
            &mut cmd,
            &[if artifact == Artifact::Identity {
                "VINK_IDENTITY_MIGRATE_DATABASE_URL"
            } else {
                "VINK_MIGRATE_DATABASE_URL"
            }],
        );
        return cmd;
    }
    if artifact == Artifact::Identity {
        env.copy(
            &mut cmd,
            &["VINK_IDENTITY_DATABASE_URL", "VINK_IDENTITY_CALLER_UID"],
        );
        cmd.env("VINK_IDENTITY_SOCKET", &config.socket)
            .env("VINK_IDENTITY_AUTHORITY", "native")
            .env("VINK_IDENTITY_MAIL_TRANSPORT", "local-outbox")
            .env("VINK_IDENTITY_LOCAL_OUTBOX", &config.outbox);
        if env.get("VINK_IDENTITY_CALLER_UID").is_none() {
            cmd.env("VINK_IDENTITY_CALLER_UID", &config.uid);
        }
    } else {
        env.copy(&mut cmd, &["DATABASE_URL", "VINK_TRUSTED_PROXIES"]);
        cmd.env("VINK_BIND", config.bind.to_string())
            .env("VINK_PUBLIC_ORIGIN", &config.origin)
            .env("VINK_AUTH_SIGN_IN", "disabled");
        if artifact == Artifact::Worker {
            env.copy(
                &mut cmd,
                &["VINK_WORKER_OUTBOX", "VINK_WORKER_OUTBOX_CONCURRENCY"],
            );
            // Evaluation is deliberately not activated by a local worker workflow.
        } else {
            cmd.env("VINK_SITE_ROOT", root().join("target/site"));
            match plan.request.auth {
                Auth::None => {}
                Auth::Fixture => env.copy(&mut cmd, &["VINK_DEV_PRINCIPAL"]),
                Auth::Identity => {
                    cmd.env("VINK_IDENTITY_SOCKET", &config.socket)
                        .env(
                            "VINK_IDENTITY_SERVER_UID",
                            env.get("VINK_IDENTITY_SERVER_UID")
                                .unwrap_or_else(|| config.uid.clone()),
                        )
                        .env(
                            "VINK_AUTH_SIGN_IN",
                            env.get("VINK_AUTH_SIGN_IN")
                                .unwrap_or_else(|| "enabled".into()),
                        );
                }
            }
        }
    }
    cmd
}

fn private_directory(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    if !path.exists() {
        std::fs::DirBuilder::new()
            .recursive(true)
            .create(path)
            .map_err(|e| e.to_string())?;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}
fn spawn(mut cmd: Command, log: &Path) -> Result<Guard, String> {
    let file = std::fs::File::create(log).map_err(|e| e.to_string())?;
    let child = cmd
        .stdout(Stdio::null())
        .stderr(file)
        .spawn()
        .map_err(|e| e.to_string())?;
    println!(
        "==> owned {} pid={} log={}",
        cmd.get_program().to_string_lossy(),
        child.id(),
        log.display()
    );
    Ok(Guard(Some(child)))
}
fn alive(child: &mut Guard) -> Result<(), String> {
    lifecycle::check_cancelled()?;
    if let Some(status) = child
        .0
        .as_mut()
        .ok_or("child missing")?
        .try_wait()
        .map_err(|e| e.to_string())?
    {
        return Err(format!("owned child exited {status}; see retained logs"));
    }
    Ok(())
}
fn ready(child: &mut Guard, mut probe: impl FnMut() -> bool) -> Result<(), String> {
    ready_with_timeout(child, &mut probe, Duration::from_secs(25))
}
fn ready_with_timeout(
    child: &mut Guard,
    mut probe: impl FnMut() -> bool,
    timeout: Duration,
) -> Result<(), String> {
    let deadline = Instant::now() + timeout;
    loop {
        alive(child)?;
        if probe() {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "capability not ready after {timeout:?}; see retained logs"
            ));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}
fn run_local(plan: &Plan, env: &Env, config: &Local) -> Result<(), String> {
    let logs = lifecycle::run_directory(&root(), "dev-runs")?;
    let mut children = vec![];
    let _socket_lock = if plan.services.contains(&Service::Identity) {
        private_directory(config.socket.parent().ok_or("socket parent missing")?)?;
        Some(Lock::acquire(&config.socket.with_extension("xtask.lock"))?)
    } else {
        None
    };
    config.available(plan)?;
    for owner in &plan.migrations {
        let artifact = if *owner == "Identity" {
            Artifact::Identity
        } else {
            Artifact::App
        };
        process::run(
            &mut command(plan, env, config, artifact, "migrate"),
            &format!("migrate {owner}"),
        )?;
    }
    if plan.request.seed {
        process::run(
            &mut command(plan, env, config, Artifact::App, "dev-seed"),
            "explicit App sample seed",
        )?;
    }
    for service in &plan.services {
        match service {
            Service::Identity => {
                private_directory(config.socket.parent().ok_or("socket parent missing")?)?;
                private_directory(&config.outbox)?;
                let mut child = spawn(
                    command(plan, env, config, Artifact::Identity, "serve"),
                    &logs.join("identity.log"),
                )?;
                ready(&mut child, || {
                    identity::request(&config.socket, "GET", "/health/ready", &[], b"")
                        .is_ok_and(|(s, _)| s == 200)
                })?;
                children.push(child);
            }
            Service::App => {
                let mut child = spawn(
                    command(
                        plan,
                        env,
                        config,
                        Artifact::App,
                        if plan.request.target == Target::Api {
                            "serve-api"
                        } else {
                            "serve"
                        },
                    ),
                    &logs.join("app.log"),
                )?;
                ready(&mut child, || {
                    lifecycle::http_status_at(config.bind, "/health/ready") == Some(200)
                })?;
                children.push(child);
                println!("==> ready {} auth={:?}", config.origin, plan.request.auth);
            }
            Service::Worker => {
                let log = logs.join("worker.log");
                let mut cmd = command(plan, env, config, Artifact::Worker, "");
                cmd.env("RUST_LOG", "info,sqlx=warn");
                let mut child = spawn(cmd, &log)?;
                ready(&mut child, || {
                    std::fs::read_to_string(&log).is_ok_and(|s| {
                        s.contains("vink-worker started")
                            && s.contains("current")
                            && (s.contains("outbox=\"ready\"") || s.contains("outbox=\"disabled\""))
                    })
                })?;
                children.push(child);
                println!("==> worker ready (no HTTP server)");
            }
            Service::Preview => unreachable!(),
        }
    }
    let result = loop {
        if let Some(error) = children.iter_mut().find_map(|c| alive(c).err()) {
            break Err(error);
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    for child in children.iter_mut().rev() {
        let _ = child.terminate(Duration::from_secs(10));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn readiness_propagates_child_failure_and_timeout() {
        let mut failed = Guard(Some(
            Command::new("sh").args(["-c", "exit 7"]).spawn().unwrap(),
        ));
        assert!(
            ready_with_timeout(&mut failed, || false, Duration::from_secs(2))
                .unwrap_err()
                .contains("exit status: 7")
        );
        let mut waiting = Guard(Some(Command::new("sleep").arg("10").spawn().unwrap()));
        assert!(
            ready_with_timeout(&mut waiting, || false, Duration::from_millis(20))
                .unwrap_err()
                .contains("not ready")
        );
        assert!(!waiting.terminate(Duration::from_secs(1)).unwrap().success());
    }
    #[test]
    fn worker_does_not_validate_unselected_app_bind_or_auth_configuration() {
        let env = Env::fixture(&[
            ("VINK_PROFILE", "local"),
            ("VINK_BIND", "invalid"),
            ("VINK_PUBLIC_ORIGIN", "invalid"),
            ("VINK_IDENTITY_MAIL_TRANSPORT", "smtp"),
        ]);
        let plan = crate::cli::parse(&["dev", "worker"]).unwrap().unwrap().plan;
        assert!(Local::resolve(&plan, &env).is_ok());
    }
    #[test]
    fn explicit_scope_overrides_accidental_auth_and_privileged_settings() {
        let env = Env::fixture(&[
            ("DATABASE_URL", "runtime"),
            ("VINK_MIGRATE_DATABASE_URL", "migrator-secret"),
            ("VINK_TEST_DATABASE_URL", "admin-secret"),
            ("VINK_DEV_PRINCIPAL", "sample"),
            ("VINK_IDENTITY_SOCKET", "ambient.sock"),
            ("VINK_AUTH_SIGN_IN", "enabled"),
        ]);
        let plan = crate::cli::parse(&["dev", "api"]).unwrap().unwrap().plan;
        let config = Local {
            bind: "127.0.0.1:3000".parse().unwrap(),
            origin: "http://127.0.0.1:3000".into(),
            socket: "a.sock".into(),
            outbox: "outbox".into(),
            uid: "1".into(),
        };
        let cmd = command(&plan, &env, &config, Artifact::App, "serve-api");
        let vars: std::collections::BTreeMap<_, _> = cmd
            .get_envs()
            .map(|(k, v)| {
                (
                    k.to_string_lossy().into_owned(),
                    v.map(|v| v.to_string_lossy().into_owned()),
                )
            })
            .collect();
        assert_eq!(vars["VINK_DEV_PRINCIPAL"], None);
        assert_eq!(vars["VINK_IDENTITY_SOCKET"], None);
        assert_eq!(vars["VINK_AUTH_SIGN_IN"], Some("disabled".into()));
        assert_eq!(vars["VINK_MIGRATE_DATABASE_URL"], None);
        assert_eq!(vars[crate::TEST_DATABASE_URL], None);
    }
}
