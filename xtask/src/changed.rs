//! Advisory selection over local Git inputs. Only the explicit selectors execute workloads.
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write,
    path::Path,
    process::Command,
};

const USAGE: &str = "verify changed --base REV (--head REV | --worktree) --plan";
const OWNER_DIRS: &[(&str, &str)] = &[
    ("packages/ui-system/leptos/preview/", "preview"),
    ("packages/ui-system/leptos/", "ui"),
    ("packages/vink-patterns/leptos/", "patterns"),
    ("packages/markdown-text/", "markdown"),
    ("packages/expressions/", "expressions"),
    ("packages/store-postgres/", "store"),
    ("services/vink-app/", "app"),
    ("services/vink-identity/", "identity"),
    ("services/vink-notifications/", "notifications"),
    ("apps/vink-web/", "web"),
    ("xtask/", "xtask"),
];

fn git(root: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let out = Command::new("git")
        .env("GIT_NO_LAZY_FETCH", "1")
        .args(["--no-optional-locks", "-c", "core.fsmonitor=false"])
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|e| format!("read Git inputs: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git {} failed: {}",
            args[0],
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(out.stdout)
}

fn commit(root: &Path, rev: &str) -> Result<String, String> {
    if rev.is_empty() || rev.starts_with('-') {
        return Err("expected a nonempty commit reference".into());
    }
    let bytes = git(
        root,
        &[
            "rev-parse",
            "--verify",
            "--end-of-options",
            &format!("{rev}^{{commit}}"),
        ],
    )?;
    Ok(std::str::from_utf8(&bytes)
        .map_err(|e| e.to_string())?
        .trim()
        .into())
}

#[derive(Debug)]
struct Change<'a> {
    layer: &'static str,
    status: &'a str,
    paths: Vec<&'a str>,
}

fn fields(raw: &[u8]) -> Result<impl Iterator<Item = &str>, String> {
    let data = std::str::from_utf8(raw)
        .map_err(|_| "non-UTF-8 Git path: inspect bytes and select full owner coverage manually")?;
    Ok(data.split_terminator('\0'))
}

fn changes<'a>(raw: &'a [u8], layer: &'static str) -> Result<Vec<Change<'a>>, String> {
    let mut entries = fields(raw)?;
    let mut result = Vec::new();
    while let Some(status) = entries.next() {
        let count = match status.as_bytes().first() {
            Some(b'R' | b'C') => 2,
            Some(b'A' | b'D' | b'M' | b'T' | b'U') => 1,
            _ => {
                return Err(format!(
                    "unmapped Git status {status:?}: manual owner review required"
                ));
            }
        };
        let paths: Vec<_> = entries.by_ref().take(count).collect();
        if paths.len() != count || paths.contains(&"") {
            return Err("incomplete Git name-status record".into());
        }
        result.push(Change {
            layer,
            status,
            paths,
        });
    }
    Ok(result)
}

fn diff(root: &Path, endpoints: &[&str]) -> Result<Vec<u8>, String> {
    let mut args = vec![
        "diff",
        "--name-status",
        "-z",
        "--no-ext-diff",
        "--no-textconv",
        "--ignore-submodules=none",
        "--find-renames",
    ];
    args.extend(endpoints);
    args.push("--");
    git(root, &args)
}

#[derive(Default)]
struct Selection {
    commands: BTreeMap<String, BTreeSet<&'static str>>,
    broad: bool,
}

impl Selection {
    fn add(&mut self, cmd: &str, reason: &'static str) {
        self.commands.entry(cmd.into()).or_default().insert(reason);
    }

    fn fallback(&mut self, reason: &'static str) {
        self.broad = true;
        for cmd in [
            "verify quick",
            "verify discussion",
            "verify expressions",
            "verify kani",
            "verify postgres",
            "verify web",
            "verify preview",
            "verify web --scenario standalone",
            "verify web --scenario perf-baseline --release",
            "verify web --scenario return-position --release",
        ] {
            self.add(cmd, reason);
        }
    }

    fn web(&mut self) {
        self.add(
            "verify static tokens",
            "dependency: token generator precedes CSS",
        );
        self.add(
            "verify web",
            "dependency: shared Web package and SSR/hydration consumers",
        );
        self.add(
            "verify web --scenario standalone",
            "dependency: standalone CSR has a separate package and fixture",
        );
    }

    fn path(&mut self, path: &str) -> &'static str {
        if matches!(
            path,
            "AGENTS.md"
                | "docs/development/scoped-workflows.md"
                | "docs/development/local-resource-budget.md"
                | "docs/development/feature-map.md"
                | "docs/development/README.md"
        ) || (path.starts_with(".agents/skills/") && path.ends_with(".md"))
        {
            self.add(
                "verify static docs",
                "selected: contributor guidance, skill metadata and local links",
            );
            return "guidance; review changed instructions and their command owners";
        }
        if matches!(
            path,
            "apps/vink-web/style/vink.css" | "packages/ui-system/leptos/tailwind/web.css"
        ) {
            self.web();
            self.add(
                "verify static repo",
                "selected: Tailwind input and package boundary guards",
            );
            return "Web stylesheet feeds SSR and standalone; review affected visual routes";
        }
        if path == "apps/vink-web/e2e/web-perf-baseline.test.mjs" {
            self.add(
                "verify static perf",
                "selected: Node performance analyzer and negative controls",
            );
            return "performance analyzer test; browser measurements use a separate fixture";
        }
        let browser = match path {
            "apps/vink-web/e2e/web-perf-baseline.mjs" => {
                self.add(
                    "verify static perf",
                    "dependency: Node analyzer imports the browser performance module",
                );
                Some("verify web --scenario perf-baseline --release")
            }
            "apps/vink-web/e2e/discovery-return-position.mjs" => {
                Some("verify web --scenario return-position --release")
            }
            "apps/vink-web/e2e/author-works-smoke.mjs"
            | "apps/vink-web/e2e/draft-pagination-smoke.mjs" => Some("verify web --scenario desk"),
            "apps/vink-web/e2e/desk-empty-smoke.mjs" => {
                self.add(
                    "verify web --scenario password",
                    "dependency: passwordAuth uses the signed-out Desk fixture before sign-in",
                );
                Some("verify web --scenario desk")
            }
            "apps/vink-web/e2e/editor-paper-first-smoke.mjs"
            | "apps/vink-web/e2e/editor-writing-smoke.mjs" => {
                Some("verify web --scenario editor-chrome")
            }
            "apps/vink-web/e2e/editor-desk-regressions.mjs" => {
                Some("verify web --scenario editor-format --scenario editor-first-draft")
            }
            "apps/vink-web/e2e/public-storage-smoke.mjs" => Some("verify web --scenario storage"),
            _ => None,
        };
        if let Some(cmd) = browser {
            self.add(
                cmd,
                "selected: browser dispatcher calls this scenario fixture",
            );
            return "owned browser assertion; use real product and fresh profile";
        }
        if path.starts_with("apps/vink-web/editor/") {
            self.web();
            self.add(
                "verify static repo",
                "selected: Editor packages and Web boundary",
            );
            return "Editor package runs typecheck, Markdown corpus and bundler; review affected IME and visual claims";
        }
        if path == "packages/ui-system/shared/tokens.json"
            || path.starts_with("packages/ui-system/leptos/style/")
            || path == "packages/vink-patterns/leptos/patterns.css"
        {
            self.web();
            self.add(
                "verify preview",
                "dependency: shared tokens/styles feed Preview",
            );
            self.fallback("fallback: shared UI input; review native and diagnostic consumers too");
            return "shared UI; review CMP and native targets in the feature map";
        }
        let Some(&(dir, owner)) = OWNER_DIRS.iter().find(|(dir, _)| path.starts_with(dir)) else {
            self.fallback("fallback: shared, external or unclassified input");
            return "fallback: unknown or shared path; determine owners and additional gates";
        };
        let local = &path[dir.len()..];
        if local == "tests/postgres.rs"
            && matches!(owner, "store" | "app" | "identity" | "notifications")
        {
            self.add(
                &format!("verify postgres {owner}"),
                "selected: owning real PostgreSQL test binary",
            );
            return "PostgreSQL test target; use a disposable cluster";
        }
        if matches!(owner, "app" | "identity" | "notifications")
            && local.ends_with(".sql")
            && (local.starts_with("migrations/") || local.starts_with("migrations-rollback/"))
        {
            self.add(
                "verify postgres",
                "selected: migrations, cross-service lifecycle, provisioning and rollback",
            );
            self.web();
            return "SQL migration; review data consumers and migration-specific recovery cases";
        }
        if local.starts_with("src/") && local.ends_with(".rs") {
            self.add(
                &format!("verify check {owner} --target all-targets"),
                "selected: compile owner and its targets",
            );
            self.add(
                &format!("verify unit {owner}"),
                "selected: link and run owner unit targets",
            );
            match owner {
                "preview" => {
                    self.add(
                        "verify check preview --target csr",
                        "dependency: Preview WASM",
                    );
                    self.add(
                        "verify preview",
                        "dependency: Preview CSS scanner and browser fixture",
                    );
                    return "Preview Rust owner and its browser";
                }
                "store" => {
                    for consumer in ["app", "identity", "notifications", "xtask"] {
                        self.add(
                            &format!("verify unit {consumer}"),
                            "dependency: Rust consumer of vink-store-postgres",
                        );
                    }
                    self.add(
                        "verify postgres",
                        "dependency: all database owners use the pool/migrator and lifecycle",
                    );
                    self.web();
                    return "shared store; check its consumers, real database and Web scratch lifecycle";
                }
                "expressions" => {
                    self.add(
                        "verify static assets",
                        "dependency: expression catalog and manifest",
                    );
                    self.add("verify expressions", "dependency: model and Rust replay");
                    self.add("verify kani", "dependency: expression policy harnesses");
                    self.add(
                        "verify preview",
                        "dependency: expression renderer in Preview",
                    );
                    return "expression policy and its Preview consumer; review CMP contract when applicable";
                }
                "notifications" => {
                    self.add(
                        "verify postgres",
                        "dependency: service, App Gateway/worker, role and recovery boundaries",
                    );
                    self.add("verify unit app", "dependency: App Gateway and worker");
                    return "notification service and its inter-process consumers";
                }
                "web" | "patterns" | "ui" | "markdown" => self.web(),
                _ => {}
            }
        }
        self.fallback("fallback: transitive consumers, feature variants, contracts or verification implementation");
        "fallback: inspect consumers and feature-specific boundaries alongside the owner"
    }

    fn describe(&self, out: &mut String) -> Result<(), String> {
        let mut commands: Vec<_> = self.commands.iter().collect();
        // Print the cheap edit loop first. The list is advisory and never executed as a batch.
        commands.sort_by_key(|(cmd, _)| match cmd.split_ascii_whitespace().nth(1) {
            Some("static") => 0,
            Some("check") => 1,
            Some("unit") => 2,
            _ => 3,
        });
        for (cmd, reasons) in commands {
            writeln!(out, "\nselected: cargo xtask {cmd}").unwrap();
            for reason in reasons {
                writeln!(out, "  {reason}").unwrap();
            }
            // Legacy full lanes use their existing dispatcher. Scoped commands go through the
            // same parser and Plan::resolve as execution, including full Postgres inspection.
            if matches!(
                cmd.as_str(),
                "verify quick" | "verify discussion" | "verify expressions" | "verify kani"
            ) {
                out.push_str("  full lane; see its inventory and tool prerequisites in the development guide\n");
            } else {
                let mut args: Vec<_> = cmd.split_ascii_whitespace().collect();
                args.push("--plan");
                let invocation = crate::cli::parse(&args)?.ok_or("missing scoped selector")?;
                out.push_str(&invocation.plan.describe());
            }
        }
        if self.broad {
            out.push_str("\nfallback: manual coverage review required for opt-in, platform, formal and release-specific claims.\n");
        }
        for group in [
            "static",
            "check",
            "unit",
            "postgres",
            "web",
            "preview",
            "quick",
            "discussion",
            "expressions",
            "kani",
        ] {
            if !self
                .commands
                .keys()
                .any(|cmd| cmd.split_ascii_whitespace().nth(1) == Some(group))
            {
                writeln!(out, "not-selected as explicit root: verify {group} (no mapped input requires this selector; broader lanes can include it)").unwrap();
            }
        }
        out.push_str("Existing acceptance, platform, opt-in and merge/release obligations require separate review.\n");
        Ok(())
    }
}

pub fn describe(root: &Path, args: &[&str]) -> Result<String, String> {
    let (base_ref, head_ref, worktree) = match args {
        ["--base", base, "--head", head, "--plan"] => (*base, *head, false),
        ["--base", base, "--worktree", "--plan"] => (*base, "HEAD", true),
        _ => return Err(format!("usage: cargo xtask {USAGE}")),
    };
    let base = commit(root, base_ref)?;
    let head = commit(root, head_ref)?;
    let (selection, mut out, _) = inspect(root, &base, &head, worktree)?;
    if selection.commands.is_empty() {
        return Ok(out);
    }
    selection.describe(&mut out)?;
    let scope = if worktree { "worktree" } else { "committed" };
    writeln!(
        out,
        "to revalidate and execute one selected command, append: --from-diff {base} {head} {scope}"
    )
    .unwrap();
    Ok(out)
}

fn inspect(
    root: &Path,
    base: &str,
    head: &str,
    worktree: bool,
) -> Result<(Selection, String, bool), String> {
    let committed = diff(root, &[base, head])?;
    let mut inputs = changes(&committed, "committed")?;
    let (index, unstaged, untracked);
    if worktree {
        let flags = git(root, &["ls-files", "-v", "-z"])?;
        if fields(&flags)?.any(|entry| {
            entry.starts_with('S') || entry.starts_with(|c: char| c.is_ascii_lowercase())
        }) {
            return Err("worktree has skip-worktree/assume-unchanged entries; inspect those inputs explicitly".into());
        }
        index = diff(root, &["--cached", head])?;
        unstaged = diff(root, &[])?;
        untracked = git(root, &["ls-files", "--others", "--exclude-standard", "-z"])?;
        inputs.extend(changes(&index, "index")?);
        inputs.extend(changes(&unstaged, "worktree")?);
        for path in fields(&untracked)? {
            inputs.push(Change {
                layer: "untracked",
                status: "?",
                paths: vec![path],
            });
        }
    }
    let mode = if worktree {
        "committed + index + unstaged + nonignored untracked"
    } else {
        "committed endpoints only"
    };
    let mut out = format!(
        "base={base} head={head}\nrange: endpoint trees ({mode})\nevidence: NOT_RUN; advisory plan, recompute after input changes. Cargo bootstrap may compile/link xtask.\n"
    );
    let dirty = inputs.iter().any(|input| input.layer != "committed");
    if inputs.is_empty() {
        out.push_str(
            "no changed paths in this scope; no checks selected and no coverage established.\n",
        );
        return Ok((Selection::default(), out, dirty));
    }
    let mut selected = Selection::default();
    for change in inputs {
        for path in change.paths {
            let reason = selected.path(path);
            writeln!(out, "{} {} {path:?}: {reason}", change.layer, change.status).unwrap();
        }
        if matches!(change.status, "T" | "U") {
            selected.fallback("fallback: file type changed or index is unmerged");
        }
    }
    Ok((selected, out, dirty))
}

/// The normal explicit command still owns admission, locking and execution. This optional
/// suffix only checks that the requested selector belongs to the current diff.
pub fn revalidate(
    root: &Path,
    command: &[&str],
    base: &str,
    head: &str,
    mode: &str,
) -> Result<(), String> {
    if !matches!(mode, "committed" | "worktree") {
        return Err("expected --from-diff BASE_SHA HEAD_SHA committed|worktree".into());
    }
    if commit(root, base)? != base || commit(root, "HEAD")? != head {
        return Err("pinned base or checked-out HEAD differs; generate a new diff plan".into());
    }
    let (selection, _, dirty) = inspect(root, base, head, true)?;
    if mode == "committed" && dirty {
        return Err("committed plan has dirty inputs; replan with --worktree".into());
    }
    if !selection.commands.contains_key(&command.join(" ")) {
        return Err("command is not selected by the current diff; generate a new plan".into());
    }
    println!(
        "revalidated {mode} at {head}; current selected roots: {:?}; executing only: {}",
        selection.commands.keys().collect::<Vec<_>>(),
        command.join(" ")
    );
    Ok(())
}

#[cfg(test)]
mod tests;
