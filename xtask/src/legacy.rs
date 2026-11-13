//! Compatibility for the original unscoped dev/db commands.
use crate::process::cargo;
use crate::{env, identity, lifecycle, process, root};
pub fn app_command(env: &env::Env, command: &str) -> Result<(), String> {
    let root = root();
    let mut run = cargo(&root);
    run.args([
        "run", "--locked", "-q", "-p", "vink-app", "--bin", "vink-app", "--", command,
    ]);
    local_identity_env(env, &mut run)?;
    process::run(&mut run, &format!("vink-app {command}"))
}

/// Migrates each owner with its own migration credential: the App always, Identity when `.env`
/// configures it. There is no cross-owner transaction; a failure leaves the earlier owner migrated.
pub fn migrate_owners(env: &env::Env) -> Result<(), String> {
    app_command(env, "migrate")?;
    if env.get("VINK_IDENTITY_MIGRATE_DATABASE_URL").is_some() {
        let root = root();
        let mut run = cargo(&root);
        run.args([
            "run",
            "--locked",
            "-q",
            "-p",
            "vink-identity",
            "--bin",
            "vink-identity",
            "--",
            "migrate",
        ]);
        env.apply(&mut run);
        process::run(&mut run, "vink-identity migrate")?;
    }
    Ok(())
}

/// Local stack only: when `.env` names `VINK_IDENTITY_SOCKET`, create its directory (mode 0700)
/// and, where unset, make the developer's own user both the allowed caller and the expected server.
/// Production configures both user ids explicitly.
fn local_identity_env(env: &env::Env, command: &mut std::process::Command) -> Result<(), String> {
    env.apply(command);
    let Some(socket) = env.get("VINK_IDENTITY_SOCKET") else {
        return Ok(());
    };
    let socket = root().join(socket);
    let dir = socket
        .parent()
        .ok_or("VINK_IDENTITY_SOCKET has no directory")?;
    if !dir.exists() {
        std::fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
        std::fs::set_permissions(dir, std::os::unix::fs::PermissionsExt::from_mode(0o700))
            .map_err(|e| format!("chmod {}: {e}", dir.display()))?;
    }
    let uid =
        std::os::unix::fs::MetadataExt::uid(&std::fs::metadata(dir).map_err(|e| e.to_string())?)
            .to_string();
    for key in ["VINK_IDENTITY_CALLER_UID", "VINK_IDENTITY_SERVER_UID"] {
        if env.get(key).is_none() {
            command.env(key, &uid);
        }
    }
    Ok(())
}

/// Starts Identity when configured and waits until it reports ready on its socket.
fn start_identity(env: &env::Env) -> Result<Option<lifecycle::Guard>, String> {
    let Some(socket) = env.get("VINK_IDENTITY_SOCKET") else {
        return Ok(None);
    };
    let root = root();
    let socket = root.join(socket);
    let mut command = std::process::Command::new(root.join("target/debug/vink-identity"));
    command.arg("serve").current_dir(&root);
    local_identity_env(env, &mut command)?;
    let child = command
        .spawn()
        .map_err(|e| format!("spawn vink-identity: {e}"))?;
    let mut guard = lifecycle::Guard(Some(child));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    let mut last = String::new();
    while std::time::Instant::now() < deadline {
        if let Some(Ok(Some(status))) = guard.0.as_mut().map(|c| c.try_wait()) {
            return Err(format!(
                "vink-identity exited with {status}; see its log above"
            ));
        }
        match identity::request(&socket, "GET", "/health/ready", &[], b"") {
            Ok((200, _)) => {
                println!("==> vink-identity ready on {}", socket.display());
                return Ok(Some(guard));
            }
            Ok((_, body)) => last = body,
            Err(_) => {}
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    let _ = guard.terminate(std::time::Duration::from_secs(10));
    Err(format!(
        "vink-identity not ready within 20s: {last} (provisioned and migrated? see services/vink-identity/README.md)"
    ))
}

/// Build, migrate every configured owner, start Identity (if configured) and wait for it, then
/// serve the App in the foreground. Ctrl-C reaches both processes; Identity is stopped with
/// SIGTERM when the App exits on its own.
pub fn dev(env: &env::Env) -> Result<(), String> {
    crate::build::legacy(env, false)?;
    migrate_owners(env)?;
    let root = root();
    let identity = start_identity(env)?;
    let origin = env
        .get("VINK_PUBLIC_ORIGIN")
        .unwrap_or_else(|| "http://127.0.0.1:3000".into());
    println!("==> serving on {origin}  (sample data: `cargo xtask db seed`; Ctrl-C to stop)");
    let mut serve = std::process::Command::new(root.join("target/debug/vink-app"));
    serve.arg("serve").current_dir(&root);
    if env.get("VINK_SITE_ROOT").is_none() {
        serve.env("VINK_SITE_ROOT", root.join("target/site"));
    }
    local_identity_env(env, &mut serve)?;
    let served = process::run(&mut serve, "vink-app serve");
    if let Some(mut identity) = identity {
        let _ = identity.terminate(std::time::Duration::from_secs(10));
    }
    served
}
