use std::path::Path;
use std::process::{Command, ExitCode};

pub fn cargo(root: &Path) -> Command {
    let mut command = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()));
    command.current_dir(root);
    command
}

pub fn run(command: &mut Command, label: &str) -> Result<(), String> {
    println!("==> {label}");
    let status = command
        .status()
        .map_err(|e| format!("{label}: cannot start {:?}: {e}", command.get_program()))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{label}: exited with {status}"))
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
