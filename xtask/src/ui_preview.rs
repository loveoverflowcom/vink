//! `cargo xtask ui-preview leptos|cmp build|verify|run` — the two UI system previews.
//!
//! Both render the shared catalog IDs through their renderer's public API and call no service.
//! Leptos: a client-rendered wasm bundle plus the generated tokens, served from
//! `target/ui-preview/leptos` by a loopback static server; `verify` drives it in headless Chrome
//! and writes captures there. CMP: the Gradle build in `packages/ui-system/cmp` (pinned wrapper,
//! JDK from the machine — never auto-provisioned); `verify` runs the common and desktop tests,
//! including the capture test that writes PNGs under `cmp/preview/build/captures`.
//! Android/iOS targets are opt-in (`VINK_UI_NATIVE_TARGETS=1`) so the JVM preview needs neither
//! the Android SDK nor Xcode.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::env::Env;
use crate::process::{self, Report, cargo};

const LEPTOS_DIR: &str = "packages/ui-system/leptos";
const CMP_DIR: &str = "packages/ui-system/cmp";
const OUT_DIR: &str = "target/ui-preview/leptos";
const DEFAULT_BIND: &str = "127.0.0.1:3019";

pub fn run(env: &Env, renderer: &str, action: &str) -> Result<(), String> {
    let root = crate::root();
    match (renderer, action) {
        ("leptos", "build") => leptos_build(&root),
        ("leptos", "run") => {
            leptos_build(&root)?;
            let bind = env
                .get("VINK_UI_PREVIEW_BIND")
                .unwrap_or_else(|| DEFAULT_BIND.into());
            let listener =
                TcpListener::bind(&bind).map_err(|e| format!("bind preview on {bind}: {e}"))?;
            println!("==> Vink UI preview (Leptos) on http://{bind}/  (Ctrl-C to stop)");
            serve(listener, root.join(OUT_DIR));
            Ok(())
        }
        ("cmp", "build") => gradle(&root, env, &cmp_build_tasks(env), "CMP build"),
        ("cmp", "run") => gradle(&root, env, &[":preview:run"], "CMP desktop preview"),
        (target @ ("android" | "ios"), action) => native(&root, env, target, action),
        _ => Err(format!("unknown ui-preview command: {renderer} {action}")),
    }
}

/// `verify` for either renderer, reported as PASS/FAIL rows.
pub fn verify(env: &Env, renderer: &str) -> std::process::ExitCode {
    let root = crate::root();
    let mut report = Report::default();
    let _ = report.step("ui tokens: generated files match", || {
        crate::ui_tokens::run(&root, true)
    }) && match renderer {
        "leptos" => leptos_verify(&mut report, &root, env),
        "cmp" => report.step("CMP: common + desktop tests and captures", || {
            gradle(&root, env, &cmp_verify_tasks(env), "CMP verify")
        }),
        target => report.step(&format!("{target}: diagnostic runtime"), || {
            native(&root, env, target, "verify")
        }),
    };
    report.finish()
}

fn leptos_build(root: &Path) -> Result<(), String> {
    crate::ui_tokens::run(root, true)?;
    crate::doctor::require_wasm_bindgen(root)?;
    let mut wasm = cargo(root);
    wasm.args([
        "build",
        "--locked",
        "-p",
        "vink-ui-preview",
        "--lib",
        "--target",
        "wasm32-unknown-unknown",
        "--features",
        "csr",
    ]);
    process::run(&mut wasm, "build preview bundle (vink-ui-preview, csr)")?;
    let out = root.join(OUT_DIR);
    let pkg = out.join("pkg");
    std::fs::create_dir_all(&pkg).map_err(|e| format!("create {}: {e}", pkg.display()))?;
    let mut bindgen = Command::new("wasm-bindgen");
    bindgen
        .args(["--target", "web", "--no-typescript", "--out-dir"])
        .arg(&pkg)
        .arg(root.join("target/wasm32-unknown-unknown/debug/vink_ui_preview.wasm"));
    process::run(&mut bindgen, "wasm-bindgen → target/ui-preview/leptos/pkg")?;
    for (from, to) in [
        ("preview/index.html", "index.html"),
        ("style/tokens.css", "tokens.css"),
        ("style/vink-ui.css", "vink-ui.css"),
        ("preview/style/preview.css", "preview.css"),
    ] {
        std::fs::copy(root.join(LEPTOS_DIR).join(from), out.join(to))
            .map_err(|e| format!("copy {from}: {e}"))?;
    }
    std::fs::copy(
        root.join("packages/vink-patterns/leptos/patterns.css"),
        out.join("patterns.css"),
    )
    .map_err(|e| format!("copy patterns: {e}"))?;
    let fixtures = out.join("fixtures");
    std::fs::create_dir_all(&fixtures).map_err(|e| format!("create preview fixtures: {e}"))?;
    std::fs::write(
        fixtures.join("sample.webp"),
        crate::ui_tokens::preview_webp(root)?,
    )
    .map_err(|e| format!("write preview WebP: {e}"))?;
    let fonts = out.join("fonts");
    std::fs::create_dir_all(&fonts).map_err(|e| format!("create {}: {e}", fonts.display()))?;
    for entry in std::fs::read_dir(root.join(crate::checks::FONTS_DIR))
        .map_err(|e| format!("read fonts: {e}"))?
        .filter_map(Result::ok)
    {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.ends_with(".woff2") || name.starts_with("OFL") {
            std::fs::copy(entry.path(), fonts.join(&name))
                .map_err(|e| format!("copy font {name}: {e}"))?;
        }
    }
    println!("==> preview ready in {OUT_DIR}");
    Ok(())
}

fn leptos_verify(report: &mut Report, root: &Path, env: &Env) -> bool {
    if !report.step("Leptos preview: build", || leptos_build(root)) {
        return false;
    }
    let mut chrome = None;
    let mut node = Ok(String::new());
    if !report.step("Leptos preview: prerequisites (Node 22+, Chrome)", || {
        node = crate::doctor::node_version();
        node.clone()?;
        chrome = crate::doctor::chrome_binary(env);
        chrome
            .as_ref()
            .map(|_| ())
            .ok_or_else(|| "Chrome/Chromium not found; set VINK_CHROME".to_string())
    }) {
        return false;
    }
    if !report.step("Leptos preview: build media hydration fixture", || {
        build_media_hydration(root)
    }) {
        return false;
    }
    report.step(
        "Leptos preview: headless Chrome scenarios and captures",
        || {
            process::run(
                Command::new("node")
                    .arg("--test")
                    .arg(
                        root.join(LEPTOS_DIR)
                            .join("preview/e2e/focus-capture.test.mjs"),
                    )
                    .arg(
                        root.join(LEPTOS_DIR)
                            .join("preview/e2e/overlay-capture.test.mjs"),
                    ),
                "focused specimen and overlay geometry regressions",
            )?;
            let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| format!("bind: {e}"))?;
            let port = listener
                .local_addr()
                .map_err(|e| format!("local addr: {e}"))?
                .port();
            let site = root.join(OUT_DIR);
            std::thread::spawn(move || serve(listener, site));
            let mut smoke = Command::new("node");
            smoke
                .arg(root.join(LEPTOS_DIR).join("preview/e2e/preview-smoke.mjs"))
                .args(["--base", &format!("http://127.0.0.1:{port}/")])
                .args(["--chrome", chrome.as_deref().unwrap_or_default()])
                .arg("--out")
                .arg(root.join(OUT_DIR).join("captures"))
                .arg("--catalog")
                .arg(root.join(crate::ui_tokens::CATALOG));
            if env.get("VINK_UI_FOCUS_CAPTURE").as_deref() == Some("1") {
                smoke.args(["--focus", "1"]);
            }
            process::run(&mut smoke, "preview smoke (Chrome DevTools Protocol)")
        },
    )
}

fn build_media_hydration(root: &Path) -> Result<(), String> {
    let out = root.join(OUT_DIR);
    process::run(
        cargo(root)
            .args([
                "run",
                "--locked",
                "-p",
                "vink-ui-preview",
                "--example",
                "media_hydration",
                "--",
            ])
            .arg(out.join("media-hydration.html")),
        "render the preview media hydration fixture as HTML",
    )?;
    process::run(
        cargo(root).args([
            "build",
            "--locked",
            "-p",
            "vink-ui-preview",
            "--example",
            "media_hydration",
            "--target",
            "wasm32-unknown-unknown",
            "--features",
            "media-hydrate",
        ]),
        "build the preview media hydration bundle",
    )?;
    process::run(
        Command::new("wasm-bindgen")
            .args(["--target", "web", "--no-typescript", "--out-dir"])
            .arg(out.join("pkg"))
            .arg(root.join("target/wasm32-unknown-unknown/debug/examples/media_hydration.wasm")),
        "wasm-bindgen → preview media hydration fixture",
    )
}

/// Serves files below `site` on loopback until the process ends. Only GET of existing files;
/// paths are resolved component by component so `..` cannot leave the directory.
fn serve(listener: TcpListener, site: PathBuf) {
    for stream in listener.incoming().filter_map(Result::ok) {
        let site = site.clone();
        std::thread::spawn(move || {
            let _ = respond(stream, &site);
        });
    }
}

fn respond(mut stream: TcpStream, site: &Path) -> std::io::Result<()> {
    let mut line = String::new();
    BufReader::new(&stream).read_line(&mut line)?;
    let mut parts = line.split_whitespace();
    let (method, target) = (parts.next().unwrap_or(""), parts.next().unwrap_or("/"));
    let path = target.split(['?', '#']).next().unwrap_or("/");
    let file = resolve(site, path);
    let body = match (method, file.as_ref().and_then(|f| std::fs::read(f).ok())) {
        ("GET", Some(body)) => body,
        _ => {
            stream.write_all(
                b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            )?;
            return Ok(());
        }
    };
    let kind = content_type(file.as_deref().unwrap_or(site));
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(&body)
}

fn resolve(site: &Path, path: &str) -> Option<PathBuf> {
    let mut file = site.to_path_buf();
    for part in path.split('/').filter(|p| !p.is_empty()) {
        if part == "." || part == ".." || part.contains('\\') {
            return None;
        }
        file.push(part);
    }
    if path.ends_with('/') {
        file.push("index.html");
    }
    file.is_file().then_some(file)
}

fn content_type(file: &Path) -> &'static str {
    match file.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("wasm") => "application/wasm",
        Some("woff2") => "font/woff2",
        Some("webp") => "image/webp",
        Some("txt") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn native_targets(env: &Env) -> bool {
    env.get("VINK_UI_NATIVE_TARGETS").as_deref() == Some("1")
}

fn cmp_build_tasks(env: &Env) -> Vec<&'static str> {
    let mut tasks = vec![":ui:compileKotlinDesktop", ":preview:compileKotlinDesktop"];
    if native_targets(env) {
        tasks.extend([
            ":ui:compileKotlinIosSimulatorArm64",
            ":ui:compileKotlinIosArm64",
            ":ui:compileDebugKotlinAndroid",
            ":patterns:compileKotlinIosSimulatorArm64",
            ":patterns:compileKotlinIosArm64",
            ":patterns:compileDebugKotlinAndroid",
        ]);
    }
    tasks
}

fn cmp_verify_tasks(env: &Env) -> Vec<&'static str> {
    let mut tasks = vec![
        ":ui:desktopTest",
        ":patterns:desktopTest",
        ":diagnostics:desktopTest",
        ":preview:desktopTest",
    ];
    if native_targets(env) {
        tasks.extend([
            ":ui:iosSimulatorArm64Test",
            ":ui:compileDebugKotlinAndroid",
            ":patterns:iosSimulatorArm64Test",
            ":patterns:compileKotlinIosArm64",
            ":patterns:compileDebugKotlinAndroid",
            ":diagnostics:iosSimulatorArm64Test",
        ]);
    }
    tasks
}

fn gradle(root: &Path, env: &Env, tasks: &[&str], label: &str) -> Result<(), String> {
    let dir = root.join(CMP_DIR);
    let mut command = Command::new(dir.join("gradlew"));
    command.current_dir(&dir).args([
        "--no-daemon",
        "--console=plain",
        "-Dorg.gradle.java.installations.auto-download=false",
    ]);
    if native_targets(env)
        || tasks.iter().any(|t| {
            t.starts_with(":androidDiagnostic:")
                || t.starts_with(":diagnostics:") && t.contains("Ios")
        })
    {
        command.arg("-Pvink.ui.nativeTargets=true");
    }
    command.args(tasks);
    process::run(
        &mut command,
        &format!("{label}: gradlew {}", tasks.join(" ")),
    )
}

/// Native hosts are opt-in and share `:diagnostics`; JVM/Web never configure them implicitly.
fn native(root: &Path, env: &Env, target: &str, action: &str) -> Result<(), String> {
    if target == "android" {
        let task = match action {
            "build" => ":androidDiagnostic:assembleDebug",
            "verify" => ":androidDiagnostic:connectedDebugAndroidTest",
            _ => ":androidDiagnostic:installDebug",
        };
        gradle(root, env, &[task], "Android diagnostic")?;
        if action == "run" {
            let sdk = env
                .get("ANDROID_HOME")
                .or_else(|| env.get("ANDROID_SDK_ROOT"))
                .ok_or("set ANDROID_HOME for the Android diagnostic host")?;
            let mut adb = Command::new(Path::new(&sdk).join("platform-tools/adb"));
            if let Some(serial) = env.get("ANDROID_SERIAL") {
                adb.args(["-s", &serial]);
            }
            adb.args([
                "shell",
                "am",
                "start",
                "-S",
                "-n",
                "com.vietink.ui.diagnostic/vink.ui.diagnostic.DiagnosticActivity",
            ]);
            for (key, value) in replay_env(env) {
                adb.args(["--es", key, &value]);
            }
            process::run(&mut adb, "Android diagnostic launch")?;
        }
        return Ok(());
    }
    let device = env.get("VINK_UI_IOS_DEVICE").ok_or("set VINK_UI_IOS_DEVICE to an installed simulator UDID (`xcrun simctl list devices`); iOS diagnostic is simulator-only")?;
    gradle(
        root,
        env,
        &[":diagnostics:linkDebugFrameworkIosSimulatorArm64"],
        "iOS diagnostic framework",
    )?;
    let dir = root.join(CMP_DIR).join("iosDiagnostic");
    process::run(
        Command::new("xcodegen")
            .current_dir(&dir)
            .args(["generate", "--spec", "project.yml"]),
        "generate iOS diagnostic project (XcodeGen)",
    )?;
    let mut build = Command::new("xcodebuild");
    build.current_dir(&dir).args([
        "-project",
        "VinkUIDiagnostic.xcodeproj",
        "-scheme",
        "VinkUIDiagnostic",
        "-destination",
        &format!("platform=iOS Simulator,id={device}"),
        "-derivedDataPath",
        "build/DerivedData",
        // One explicit simulator; do not clone it or dispatch tests across user's simulators.
        "-parallel-testing-enabled",
        "NO",
        if action == "verify" { "test" } else { "build" },
    ]);
    process::run(&mut build, "iOS diagnostic Xcode simulator build/test")?;
    if action == "run" {
        process::run(
            Command::new("xcrun")
                .args(["simctl", "install", &device])
                .arg(dir.join(
                    "build/DerivedData/Build/Products/Debug-iphonesimulator/VinkUIDiagnostic.app",
                )),
            "install iOS diagnostic",
        )?;
        let mut launch = Command::new("xcrun");
        launch.args([
            "simctl",
            "launch",
            "--terminate-running-process",
            &device,
            "com.vietink.ui.diagnostic",
        ]);
        for (key, value) in replay_env(env) {
            launch.arg(format!("--{key}={value}"));
        }
        process::run(&mut launch, "launch iOS diagnostic")?;
    }
    Ok(())
}

fn replay_env(env: &Env) -> Vec<(&'static str, String)> {
    [
        ("entry", "VINK_UI_ENTRY"),
        ("scenario", "VINK_UI_SCENARIO"),
        ("theme", "VINK_UI_THEME"),
        ("motion", "VINK_UI_MOTION"),
        ("scale", "VINK_UI_SCALE"),
    ]
    .into_iter()
    .filter_map(|(key, variable)| env.get(variable).map(|value| (key, value)))
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_paths_cannot_escape_the_site() {
        let site = crate::root().join(LEPTOS_DIR);
        assert_eq!(resolve(&site, "/../Cargo.toml"), None);
        assert_eq!(resolve(&site, "/style/../../Cargo.toml"), None);
        assert!(resolve(&site, "/style/vink-ui.css").is_some());
        assert_eq!(resolve(&site, "/missing.css"), None);
    }

    #[test]
    fn wasm_is_served_with_its_streaming_type() {
        assert_eq!(content_type(Path::new("a/b.wasm")), "application/wasm");
        assert_eq!(content_type(Path::new("x.woff2")), "font/woff2");
    }
}
