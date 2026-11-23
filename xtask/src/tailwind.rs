//! Shared Tailwind CSS v4 compiler adapter for Web (`/pkg/vink.css`) and Leptos preview
//! (`vink-preview.css`). Pins `tailwindcss` + `@tailwindcss/cli` in a dedicated CSS owner
//! package (`packages/ui-system/leptos/tailwind`) with `npm ci --ignore-scripts`, explicit
//! `@layer` coexistence, and `source(none)` + allowlisted `@source` roots.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::process;

pub const DIR: &str = "packages/ui-system/leptos/tailwind";
pub const WEB_ENTRY: &str = "packages/ui-system/leptos/tailwind/web.css";
pub const PREVIEW_ENTRY: &str = "packages/ui-system/leptos/tailwind/preview.css";
/// Tailwind v4.1.8 fixed extraction for Leptos `class:` syntax (tailwindlabs/tailwindcss#18093).
pub const MIN_VERSION: (u32, u32, u32) = (4, 1, 8);

static INSTALL_LOCK: Mutex<()> = Mutex::new(());
static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Ensures the locked Tailwind v4 CLI is installed without running npm lifecycle scripts.
pub fn ensure_installed(root: &Path) -> Result<PathBuf, String> {
    let _guard = INSTALL_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = root.join(DIR);
    let lock = std::fs::read(dir.join("package-lock.json"))
        .map_err(|e| format!("read {DIR}/package-lock.json: {e}"))?;
    let bin = dir.join("node_modules/.bin/tailwindcss");
    let stamp = dir.join("node_modules/.vink-installed-lock.json");
    if !bin.is_file() || std::fs::read(&stamp).ok().as_ref() != Some(&lock) {
        let mut install = Command::new("npm");
        install
            .current_dir(&dir)
            .args(["ci", "--ignore-scripts", "--no-audit", "--no-fund"]);
        process::run(&mut install, "npm ci (Tailwind v4 compiler, locked)")?;
        std::fs::write(&stamp, &lock).map_err(|e| format!("write {}: {e}", stamp.display()))?;
    }
    if !bin.is_file() {
        return Err(format!(
            "Tailwind compiler binary missing after npm ci: {}",
            bin.display()
        ));
    }
    Ok(bin)
}

/// Compiles `input` into `output` using the pinned Tailwind v4 CLI.
///
/// Removes any pre-existing `output` before compiling and writes via a sibling temporary file so
/// a compiler failure can never leave stale CSS behind as a successful build artifact.
pub fn compile(root: &Path, input: &Path, output: &Path, minify: bool) -> Result<(), String> {
    let bin = ensure_installed(root)?;
    let parent = output
        .parent()
        .ok_or_else(|| format!("invalid CSS output path: {}", output.display()))?;
    std::fs::create_dir_all(parent).map_err(|e| format!("create {}: {e}", parent.display()))?;
    if output.exists() {
        std::fs::remove_file(output)
            .map_err(|e| format!("remove stale {}: {e}", output.display()))?;
    }
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let seq = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let name = output
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("out.css");
    let tmp = parent.join(format!(".{name}.tmp.{}.{stamp}.{seq}", std::process::id()));

    let mut cmd = Command::new(&bin);
    cmd.current_dir(root.join(DIR))
        .arg("--input")
        .arg(input)
        .arg("--output")
        .arg(&tmp);
    if minify {
        cmd.arg("--minify");
    }
    let result = cmd
        .output()
        .map_err(|e| format!("run Tailwind compiler {}: {e}", bin.display()))?;
    if !result.status.success() {
        let _ = std::fs::remove_file(&tmp);
        let stderr = String::from_utf8_lossy(&result.stderr).trim().to_string();
        return Err(format!(
            "Tailwind compile {} failed ({}): {stderr}",
            input.display(),
            result.status
        ));
    }
    let bytes = std::fs::metadata(&tmp).map(|m| m.len()).unwrap_or_default();
    if bytes == 0 {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!(
            "Tailwind compile {} produced an empty stylesheet",
            input.display()
        ));
    }
    std::fs::rename(&tmp, output).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("commit {}: {e}", output.display())
    })?;
    Ok(())
}

/// Repository check: exact package/lockfile pins (>= 4.1.8) and allowlisted `source(none)` entries.
pub fn check(root: &Path, problems: &mut Vec<String>) {
    let read = |path: &Path| std::fs::read_to_string(path).ok();
    let parse = |name: &str, problems: &mut Vec<String>| -> Option<serde_json::Value> {
        match read(&root.join(DIR).join(name)).map(|t| serde_json::from_str(&t)) {
            Some(Ok(value)) => Some(value),
            Some(Err(error)) => {
                problems.push(format!("{DIR}/{name}: {error}"));
                None
            }
            None => {
                problems.push(format!("{DIR}/{name} is missing"));
                None
            }
        }
    };
    if let (Some(manifest), Some(lock)) = (
        parse("package.json", problems),
        parse("package-lock.json", problems),
    ) {
        let mut versions = std::collections::BTreeMap::new();
        for section in ["dependencies", "devDependencies"] {
            let declared = manifest[section].as_object().cloned().unwrap_or_default();
            for (name, version) in &declared {
                let version = version.as_str().unwrap_or_default();
                if let Some(parsed) = parse_exact_semver(version) {
                    versions.insert(name.clone(), (version.to_string(), parsed));
                } else {
                    problems.push(format!(
                        "{DIR}/package.json: {name} must pin an exact version, not `{version}`"
                    ));
                }
            }
            if lock["packages"][""][section] != manifest[section] {
                problems.push(format!(
                    "{DIR}/package-lock.json: {section} differ from package.json; run `npm install --ignore-scripts` in {DIR}"
                ));
            }
        }
        match (
            versions.get("tailwindcss"),
            versions.get("@tailwindcss/cli"),
        ) {
            (Some((tw_s, tw_v)), Some((cli_s, _))) => {
                if tw_s != cli_s {
                    problems.push(format!(
                        "{DIR}/package.json: tailwindcss ({tw_s}) and @tailwindcss/cli ({cli_s}) must pin the exact same version"
                    ));
                }
                if *tw_v < MIN_VERSION {
                    problems.push(format!(
                        "{DIR}/package.json: tailwindcss {tw_s} is below {}.{}.{} (required for Leptos `class:` extraction)",
                        MIN_VERSION.0, MIN_VERSION.1, MIN_VERSION.2
                    ));
                }
            }
            _ => problems.push(format!(
                "{DIR}/package.json: must pin both `tailwindcss` and `@tailwindcss/cli`"
            )),
        }
    }

    for entry in [WEB_ENTRY, PREVIEW_ENTRY] {
        let entry_path = root.join(entry);
        let Some(text) = read(&entry_path) else {
            problems.push(format!("{entry} is missing"));
            continue;
        };
        if !text.contains("@layer theme, base, legacy, components, utilities;") {
            problems.push(format!(
                "{entry}: must declare `@layer theme, base, legacy, components, utilities;`"
            ));
        }
        if !text.contains("@import \"tailwindcss/utilities.css\" layer(utilities) source(none);") {
            problems.push(format!(
                "{entry}: must scope scanning with `source(none)` plus explicit `@source` roots"
            ));
        }
        let mut source_count = 0usize;
        for (line_no, line) in text.lines().enumerate() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("@source ") {
                source_count += 1;
                let target = rest
                    .trim_end_matches(';')
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'');
                let resolved = entry_path.parent().map(|p| p.join(target));
                let canonical = resolved
                    .as_ref()
                    .and_then(|p| std::fs::canonicalize(p).ok());
                let root_canon = std::fs::canonicalize(root).ok();
                match (canonical, root_canon) {
                    (Some(dir), Some(repo)) if dir.is_dir() && dir.starts_with(&repo) => {
                        let rel = dir
                            .strip_prefix(&repo)
                            .unwrap_or(&dir)
                            .to_string_lossy()
                            .replace('\\', "/");
                        let allowed = rel == "apps/vink-web/src"
                            || rel == "packages/ui-system/leptos/src"
                            || rel == "packages/ui-system/leptos/preview/src"
                            || rel == "packages/ui-system/leptos/preview/examples"
                            || rel == "packages/vink-patterns/leptos/src";
                        if !allowed {
                            problems.push(format!(
                                "{entry}:{}: @source `{target}` resolves to `{rel}`, outside the Rust UI allowlist",
                                line_no + 1
                            ));
                        }
                    }
                    _ => problems.push(format!(
                        "{entry}:{}: @source `{target}` does not resolve to an existing directory inside the repository",
                        line_no + 1
                    )),
                }
            }
        }
        if source_count == 0 {
            problems.push(format!("{entry}: must declare at least one `@source` root"));
        }
    }
}

fn parse_exact_semver(version: &str) -> Option<(u32, u32, u32)> {
    let mut parts = version.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    parts.next().is_none().then_some((major, minor, patch))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::sha256_hex;

    fn temp_dir(label: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("vink-tw-{label}-{}-{stamp}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn repeated_compile_is_deterministic_and_failure_never_keeps_stale_output() {
        let root = crate::root();
        let dir = temp_dir("det");
        let out1 = dir.join("out1.css");
        let out2 = dir.join("out2.css");
        let entry = root.join(WEB_ENTRY);
        compile(&root, &entry, &out1, true).expect("first web compile");
        compile(&root, &entry, &out2, true).expect("second web compile");
        let bytes1 = std::fs::read(&out1).unwrap();
        let bytes2 = std::fs::read(&out2).unwrap();
        assert_eq!(
            sha256_hex(&bytes1),
            sha256_hex(&bytes2),
            "repeated builds with identical inputs must produce identical CSS"
        );
        let text = String::from_utf8(bytes1).unwrap();
        assert!(
            text.contains("url(\"fonts/be-vietnam-pro-400.woff2\")")
                || text.contains("url(fonts/be-vietnam-pro-400.woff2)"),
            "relative font URLs must stay intact for /pkg/vink.css"
        );
        assert!(
            text.contains("brand/vpen-r1-lockup-light.svg"),
            "relative brand URLs must stay intact for /pkg/vink.css"
        );

        // Intentional compiler failure: stale output must be deleted and compile must fail.
        let stale = dir.join("stale.css");
        std::fs::write(&stale, "/* stale output that must not survive failure */").unwrap();
        let broken = dir.join("broken.css");
        std::fs::write(&broken, "@import \"./missing-file-does-not-exist.css\";\n").unwrap();
        let err = compile(&root, &broken, &stale, false).unwrap_err();
        assert!(err.contains("Tailwind compile"), "{err}");
        assert!(
            !stale.exists(),
            "failed compile must never leave stale CSS behind"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn source_none_extracts_leptos_syntaxes_rebuilds_new_utilities_and_ignores_excluded_dirs() {
        let root = crate::root();
        let dir = temp_dir("scan");
        let allowed = dir.join("allowed");
        let excluded = dir.join("excluded");
        std::fs::create_dir_all(&allowed).unwrap();
        std::fs::create_dir_all(&excluded).unwrap();

        ensure_installed(&root).expect("install pinned tailwind");
        let tw_pkg = root.join(DIR).join("node_modules/tailwindcss");
        let theme = root.join(crate::ui_tokens::TAILWIND_THEME_OUT);
        let tokens = root.join(crate::ui_tokens::CSS_OUT);
        let entry = dir.join("entry.css");
        std::fs::write(
            &entry,
            format!(
                "@layer theme, base, legacy, components, utilities;\n\
                 @import \"{}\" layer(theme);\n\
                 @import \"{}\";\n\
                 @import \"{}\" layer(base);\n\
                 @import \"{}\" layer(utilities) source(none);\n\
                 @source \"./allowed\";\n",
                tw_pkg.join("theme.css").display(),
                theme.display(),
                tokens.display(),
                tw_pkg.join("utilities.css").display(),
            ),
        )
        .unwrap();

        // Excluded file outside `@source` carries canary utilities that must never leak in.
        std::fs::write(
            excluded.join("manuscript.md"),
            "class=\"p-[876px] tracking-[0.987em]\"\n",
        )
        .unwrap();

        // Allowed Rust source exercises static, class:, tuple, conditional, responsive, state,
        // forced-colors, and cleared default palette/type negative controls.
        let component = allowed.join("pilot.rs");
        std::fs::write(
            &component,
            r#"
            fn pilot(err: bool, banner: bool) {
                let tone = if err { "bg-vk-error-container text-vk-on-error-container" } else { "bg-vk-surface text-vk-on-surface" };
                let _ = (tone, "inline-flex items-center gap-vk-2 font-vk-ui text-vk-label duration-vk-short ease-vk-emphasized sm:px-vk-6 hover:underline focus-visible:outline-[3px] bg-red-500 text-sm");
                // Real Leptos `view!` `class:` syntax (no whitespace workaround) and tuple syntax:
                let _ = view! {
                    <div
                        class:rounded-none=banner
                        class:bg-vk-warning-container=err
                        class=("forced-colors:border", true)
                    />
                };
            }
            "#,
        )
        .unwrap();

        let out = dir.join("compiled.css");
        compile(&root, &entry, &out, false).expect("compile fixture");
        let css = std::fs::read_to_string(&out).unwrap();

        for expected in [
            ".inline-flex",
            ".gap-vk-2",
            "gap: var(--vk-space-2);",
            ".bg-vk-error-container",
            "background-color: var(--vk-error-container);",
            ".bg-vk-warning-container",
            ".rounded-none",
            ".font-vk-ui",
            ".text-vk-label",
            ".duration-vk-short",
            "transition-duration: var(--vk-duration-short);",
            ".ease-vk-emphasized",
            ".sm\\:px-vk-6",
            ".hover\\:underline:hover",
            ".focus-visible\\:outline-\\[3px\\]:focus-visible",
            ".forced-colors\\:border",
        ] {
            assert!(css.contains(expected), "missing `{expected}` in:\n{css}");
        }
        // Inline theme mapping: no `:root` re-declaration of `--color-vk-*`, no excluded canary,
        // no default Tailwind palette/type emission.
        for forbidden in [
            "--color-vk-surface:",
            "876px",
            "0.987em",
            ".bg-red-500",
            ".text-sm",
        ] {
            assert!(
                !css.contains(forbidden),
                "unexpected `{forbidden}` leaked into compiled CSS"
            );
        }

        // Adding a new utility must be picked up on rebuild.
        assert!(!css.contains(".gap-vk-8"));
        let mut updated = std::fs::read_to_string(&component).unwrap();
        updated.push_str("\nconst ADDED: &str = \"gap-vk-8\";\n");
        std::fs::write(&component, updated).unwrap();
        compile(&root, &entry, &out, false).expect("rebuild with added utility");
        let rebuilt = std::fs::read_to_string(&out).unwrap();
        assert!(
            rebuilt.contains(".gap-vk-8") && rebuilt.contains("gap: var(--vk-space-8);"),
            "rebuild must include newly added utility"
        );

        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn check_rejects_unpinned_or_old_tailwind_and_broad_source_scan() {
        let root = temp_dir("check");
        let dir = root.join(DIR);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("package.json"),
            r#"{"devDependencies":{"tailwindcss":"4.1.7","@tailwindcss/cli":"^4.1.7"}}"#,
        )
        .unwrap();
        std::fs::write(
            dir.join("package-lock.json"),
            r#"{"packages":{"":{"devDependencies":{"tailwindcss":"4.1.7","@tailwindcss/cli":"^4.1.7"}}}}"#,
        )
        .unwrap();
        std::fs::write(
            dir.join("web.css"),
            "@import \"tailwindcss\";\n@source \"../../../..\";\n",
        )
        .unwrap();
        std::fs::write(dir.join("preview.css"), "@import \"tailwindcss\";\n").unwrap();
        let mut problems = Vec::new();
        check(&root, &mut problems);
        std::fs::remove_dir_all(root).unwrap();
        assert!(
            problems
                .iter()
                .any(|p| p.contains("must pin an exact version")),
            "{problems:?}"
        );
        assert!(
            problems.iter().any(|p| p.contains("source(none)")),
            "{problems:?}"
        );
        assert!(
            problems
                .iter()
                .any(|p| p.contains("outside the Rust UI allowlist")),
            "{problems:?}"
        );
    }
}
