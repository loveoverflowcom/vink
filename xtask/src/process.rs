use std::path::Path;
use std::process::{Command, ExitCode};

pub fn cargo(root: &Path) -> Command {
    let mut command = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()));
    command
        .current_dir(root)
        .env("CARGO_TARGET_DIR", root.join("target"));
    command
}

pub fn run(command: &mut Command, label: &str) -> Result<(), String> {
    println!("==> {label}");
    let mut child = spawn_group(command, label)?;
    let status = wait_owned(&mut child)?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{label}: exited with {status}"))
    }
}

fn spawn_group(command: &mut Command, label: &str) -> Result<std::process::Child, String> {
    use std::os::unix::process::CommandExt;
    crate::lifecycle::check_cancelled()?;
    command.process_group(0);
    command
        .spawn()
        .map_err(|e| format!("{label}: cannot start {:?}: {e}", command.get_program()))
}
fn wait_owned(child: &mut std::process::Child) -> Result<std::process::ExitStatus, String> {
    loop {
        if crate::lifecycle::cancelled() {
            let group = format!("-{}", child.id());
            let _ = Command::new("kill").args(["-TERM", "--", &group]).status();
            // Keep the unreaped group leader until descendants have received their stop signal.
            std::thread::sleep(std::time::Duration::from_secs(1));
            let _ = Command::new("kill").args(["-KILL", "--", &group]).status();
            let _ = child.wait();
            return Err("interrupted; owned command group stopped".into());
        }
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(50)),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error.to_string());
            }
        }
    }
}

/// Capture both pipes while still supervising cancellation (test discovery can compile too).
pub fn output(command: &mut Command, label: &str) -> Result<std::process::Output, String> {
    use std::io::Read;
    command
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = spawn_group(command, label)?;
    let mut stdout = child.stdout.take().ok_or("missing stdout pipe")?;
    let mut stderr = child.stderr.take().ok_or("missing stderr pipe")?;
    let out = std::thread::spawn(move || {
        let mut bytes = vec![];
        stdout.read_to_end(&mut bytes).map(|_| bytes)
    });
    let err = std::thread::spawn(move || {
        let mut bytes = vec![];
        stderr.read_to_end(&mut bytes).map(|_| bytes)
    });
    let status = wait_owned(&mut child);
    let stdout = out
        .join()
        .map_err(|_| "stdout reader failed")?
        .map_err(|e| e.to_string())?;
    let stderr = err
        .join()
        .map_err(|_| "stderr reader failed")?
        .map_err(|e| e.to_string())?;
    Ok(std::process::Output {
        status: status?,
        stdout,
        stderr,
    })
}

/// Requires each named test in what `command` (a `cargo test ... --` invocation with its filters)
/// selects. A filter that matches nothing still exits 0, so a renamed or deleted replay test would
/// otherwise turn its step into a vacuous PASS.
pub fn require_tests(command: &mut Command, label: &str, names: &[&str]) -> Result<(), String> {
    println!("==> {label}: required tests are selected");
    let output = output(command.arg("--list"), label)?;
    if !output.status.success() {
        eprintln!("{}", String::from_utf8_lossy(&output.stderr));
        return Err(format!(
            "{label}: listing tests exited with {}",
            output.status
        ));
    }
    let listed = String::from_utf8_lossy(&output.stdout);
    let missing: Vec<&str> = names
        .iter()
        .copied()
        .filter(|name| !listed.lines().any(|line| line == format!("{name}: test")))
        .collect();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!("{label}: not selected: {}", missing.join(", ")))
    }
}

/// Ordered PASS/FAIL results; steps after a failure are not run.
#[derive(Default)]
pub struct Report {
    rows: Vec<(String, Result<(), String>)>,
}

impl Report {
    pub fn step(&mut self, name: &str, check: impl FnOnce() -> Result<(), String>) -> bool {
        let result = check();
        let ok = result.is_ok();
        if let Err(message) = &result {
            eprintln!("xtask: {name}: {message}");
        }
        self.rows.push((name.to_string(), result));
        ok
    }

    pub fn fail(&mut self, name: &str, message: &str) {
        self.step(name, || Err(message.to_string()));
    }

    pub fn finish(self) -> ExitCode {
        println!("\nverification summary");
        let mut failed = false;
        for (name, result) in &self.rows {
            match result {
                Ok(()) => println!("  PASS  {name}"),
                Err(message) => {
                    failed = true;
                    println!("  FAIL  {name}: {message}");
                }
            }
        }
        if failed {
            println!("  (later steps NOT_RUN)");
            ExitCode::FAILURE
        } else {
            ExitCode::SUCCESS
        }
    }
}
