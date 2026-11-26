//! `cargo xtask ui-preview leptos|cmp build|verify|run` — the two hosts of the Preview app.
//!
//! The Preview app (`apps/vink-preview`, #552) is the only executable home of UI specimens,
//! mocks and demos; both hosts render the shared catalog IDs through their renderer's public
//! API and call no service. Leptos (`apps/vink-preview/leptos`): a client-rendered wasm bundle
//! plus the generated tokens, served from `target/ui-preview/leptos` by a loopback static server;
//! `verify` drives it in headless Chrome and writes captures there. CMP
//! (`apps/vink-preview/cmp`, Gradle project `:preview` of the build in `packages/ui-system/cmp`:
//! pinned wrapper, JDK from the machine — never auto-provisioned); `verify` runs the common and
//! desktop tests, including the capture test that writes PNGs under
//! `apps/vink-preview/cmp/build/captures`.
//! Android/iOS targets are opt-in (`VINK_UI_NATIVE_TARGETS=1`) so the JVM preview needs neither
//! the Android SDK nor Xcode.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::env::Env;
use crate::process::{self, Report, cargo};

/// Leptos host of the Preview app (crate `vink-ui-preview`).
const LEPTOS_HOST: &str = "apps/vink-preview/leptos";
/// Gradle build that owns the CMP renderer and maps the Preview app's `:preview` project.
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
            crate::lifecycle::check_cancelled()
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
    crate::doctor::node_version()?;
    crate::doctor::npm_version()?;
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
    std::fs::copy(
        root.join(LEPTOS_HOST).join("index.html"),
        out.join("index.html"),
    )
    .map_err(|e| format!("copy {LEPTOS_HOST}/index.html: {e}"))?;
    // Remove unbundled legacy CSS files from earlier builds so only `vink-preview.css` is served.
    for stale in ["tokens.css", "vink-ui.css", "patterns.css", "preview.css"] {
        let path = out.join(stale);
        if path.exists() {
            let _ = std::fs::remove_file(path);
        }
    }
    crate::tailwind::compile(
        root,
        &root.join(crate::tailwind::PREVIEW_ENTRY),
        &out.join("vink-preview.css"),
        false,
    )?;
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
    crate::expressions::build(root, &pkg.join("expressions"))?;
    build_hydration_fixture(root, "expression_hydration", "expressions.html")?;
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
        build_hydration_fixture(root, "media_hydration", "media-hydration.html")
    }) {
        return false;
    }
    if !report.step("Leptos preview: build notice hydration fixture", || {
        build_hydration_fixture(root, "notice_hydration", "notice-hydration.html")
    }) {
        return false;
    }
    report.step(
        "Leptos preview: headless Chrome scenarios and captures",
        || {
            process::run(
                Command::new("node")
                    .arg("--test")
                    .arg(root.join(LEPTOS_HOST).join("e2e/focus-capture.test.mjs"))
                    .arg(root.join(LEPTOS_HOST).join("e2e/overlay-capture.test.mjs")),
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
                .arg(root.join(LEPTOS_HOST).join("e2e/preview-smoke.mjs"))
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

fn build_hydration_fixture(root: &Path, example: &str, html_name: &str) -> Result<(), String> {
    let out = root.join(OUT_DIR);
    process::run(
        cargo(root)
            .args([
                "run",
                "--locked",
                "-p",
                "vink-ui-preview",
                "--example",
                example,
                "--",
            ])
            .arg(out.join(html_name)),
        &format!("render preview hydration fixture {example} as HTML"),
    )?;
    process::run(
        cargo(root).args([
            "build",
            "--locked",
            "-p",
            "vink-ui-preview",
            "--example",
            example,
            "--target",
            "wasm32-unknown-unknown",
            "--features",
            "media-hydrate",
        ]),
        &format!("build preview hydration bundle {example}"),
    )?;
    process::run(
        Command::new("wasm-bindgen")
            .args(["--target", "web", "--no-typescript", "--out-dir"])
            .arg(out.join("pkg"))
            .arg(root.join(format!(
                "target/wasm32-unknown-unknown/debug/examples/{example}.wasm"
            ))),
        &format!("wasm-bindgen → preview hydration fixture {example}"),
    )
}

/// Serves files below `site` on loopback until the process ends. Only GET of existing files;
/// paths are resolved component by component so `..` cannot leave the directory.
fn serve(listener: TcpListener, site: PathBuf) {
    if listener.set_nonblocking(true).is_err() {
        return;
    }
    while !crate::lifecycle::cancelled() {
        match listener.accept() {
            Ok((stream, _)) => {
                let site = site.clone();
                std::thread::spawn(move || {
                    let _ = respond(stream, &site);
                });
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(std::time::Duration::from_millis(50))
            }
            Err(_) => break,
        }
    }
}

fn respond(mut stream: TcpStream, site: &Path) -> std::io::Result<()> {
    // macOS inherits O_NONBLOCK from the accept-loop listener. Each request has its own
    // thread, so stream in blocking mode instead of truncating a large WASM on WouldBlock.
    stream.set_nonblocking(false)?;
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
        Some("svg") => "image/svg+xml",
        Some("json") => "application/json",
        Some("md") => "text/plain; charset=utf-8",
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
        let site = crate::root().join("packages/ui-system/leptos");
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

    #[test]
    fn nonblocking_listener_streams_a_complete_large_asset() {
        use std::io::Read;
        use std::time::{Duration, SystemTime, UNIX_EPOCH};
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let site =
            std::env::temp_dir().join(format!("vink-preview-{}-{stamp}", std::process::id()));
        std::fs::create_dir(&site).unwrap();
        let body = vec![42_u8; 4 * 1024 * 1024];
        std::fs::write(site.join("large.wasm"), &body).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let mut client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        client
            .write_all(b"GET /large.wasm HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .unwrap();
        // A completed loopback connect can still precede readiness on a nonblocking listener.
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        let stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(error)
                    if error.kind() == std::io::ErrorKind::WouldBlock
                        && std::time::Instant::now() < deadline =>
                {
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(error) => panic!("accept preview fixture: {error}"),
            }
        };
        stream
            .set_write_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        let worker_site = site.clone();
        let worker = std::thread::spawn(move || respond(stream, &worker_site));
        // Leave enough data queued to exercise socket backpressure before the client drains it.
        std::thread::sleep(Duration::from_millis(100));
        let mut response = Vec::new();
        let read = client.read_to_end(&mut response);
        let served = worker.join().unwrap();
        std::fs::remove_dir_all(site).unwrap();
        read.expect("client receives the entire asset without a reset");
        served.expect("server streams despite listener nonblocking mode");
        let header_end = response.windows(4).position(|p| p == b"\r\n\r\n").unwrap() + 4;
        assert_eq!(&response[header_end..], &body);
        assert!(
            String::from_utf8_lossy(&response[..header_end])
                .contains("Content-Type: application/wasm")
        );
    }
}
