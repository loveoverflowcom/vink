//! Checkout-local publication and provenance for the product Web package.
use crate::{lifecycle, process};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

type Inventory = BTreeMap<PathBuf, String>;

const BINDGEN_FLAGS: &[&str] = &["--target", "web", "--no-typescript", "--out-name", "vink"];
const PRODUCT_ASSETS: &[&str] = &["vink-editor.js", "vink.css", "fonts", "brand", "empty"];
// The reviewed CLI reads npm manifests by path if the input contains package_json metadata.
// Guard reuse to this CLI version and to modules without such references.
const REVIEWED_BINDGEN: &str = "wasm-bindgen 0.2.129";

pub struct Package {
    active: PathBuf,
    state: PathBuf,
    pending: PathBuf,
}

impl Package {
    /// Callers hold the existing checkout artifact lock through publication.
    pub fn new(root: &Path, active: &Path) -> Result<Self, String> {
        let relative = active
            .strip_prefix(root)
            .map_err(|e| format!("Web package must be in the checkout: {e}"))?;
        let identity = serde_json::to_vec(&(root, relative)).map_err(|e| e.to_string())?;
        let key = crate::checks::sha256_hex(&identity);
        let state = root.join("target/.xtask-web").join(key);
        require_local_path(root, &state)?;
        require_local_path(root, active)?;
        create_dir_all(&state)?;
        let previous = state.join("previous");
        require_local_path(root, &previous)?;
        if previous
            .try_exists()
            .map_err(|e| path_error(&previous, e))?
        {
            if active.try_exists().map_err(|e| path_error(active, e))? {
                remove_dir(&previous)?;
            } else {
                rename(&previous, active)?;
            }
        }
        // Each invocation uses a distinct pending path. A cancelled tool cannot write into
        // the following run's staging area, even if the parent was killed before reaping it.
        for entry in entries(&state)? {
            if entry.file_name().to_string_lossy().starts_with("pending-") {
                remove_dir(&entry.path())?;
            }
        }
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos();
        let pending = state.join(format!("pending-{}-{stamp}", std::process::id()));
        create_dir_all(&pending.join("pkg"))?;
        Ok(Self {
            active: active.into(),
            state,
            pending,
        })
    }

    pub fn stage(&self) -> PathBuf {
        self.pending.join("pkg")
    }

    pub fn bindgen(
        &self,
        wasm: &Path,
        feature: &str,
        profile: &str,
        prefix: Option<&str>,
        version: &str,
        release: bool,
    ) -> Result<(), String> {
        let tool = reusable_bindgen(version);
        let args: Vec<_> = BINDGEN_FLAGS
            .iter()
            .copied()
            .chain(release.then_some("--remove-name-section"))
            .collect();
        let input = || {
            let Some(tool) = &tool else { return Ok(None) };
            let env: BTreeMap<_, _> = std::env::vars_os()
                .filter(|(k, _)| {
                    let key = k.as_encoded_bytes();
                    key.starts_with(b"WASM_BINDGEN_") || cargo_loader_path(key)
                })
                .map(|(k, v)| (k, crate::checks::sha256_hex(v.as_encoded_bytes())))
                .collect();
            let key = serde_json::to_vec(&(
                feature,
                profile,
                prefix,
                version,
                &args,
                env.into_iter().collect::<Vec<_>>(),
                digest(wasm)?,
                digest(tool)?,
            ))
            .map_err(|e| e.to_string())?;
            Ok::<_, String>(Some(crate::checks::sha256_hex(&key)))
        };
        let key = input()?;
        self.bindgen_step(key.as_deref(), |stage| {
            let mut cmd = Command::new(tool.as_deref().unwrap_or(Path::new("wasm-bindgen")));
            cmd.args(&args).arg("--out-dir").arg(stage).arg(wasm);
            process::run(&mut cmd, "wasm-bindgen public package")?;
            if key.is_some() && input()? != key {
                return Err(
                    "wasm-bindgen inputs changed during generation; retry the build".into(),
                );
            }
            // A path to a package.json in wasm-bindgen metadata is stored verbatim. A conservative
            // byte search also rejects unrelated uses of that string. Bound the one-time scan.
            let can_cache = key.is_some() && no_external_bindgen_manifest(wasm)?;
            if key.is_some() && !can_cache {
            println!("==> wasm-bindgen: receipt omitted (possible external npm manifest or input too large)");
        }
        Ok(can_cache)
        })
    }

    fn bindgen_step(
        &self,
        key: Option<&str>,
        generate: impl FnOnce(&Path) -> Result<bool, String>,
    ) -> Result<(), String> {
        let stage = self.stage();
        let receipt = fs::read(self.state.join("bindgen.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<BindgenReceipt>(&bytes).ok());
        let reason = match (key, receipt) {
            (None, _) => "tool provenance is outside the reuse boundary",
            (_, None) => "receipt missing or invalid",
            (Some(key), Some(receipt)) if key != receipt.input => "input changed",
            (Some(_), Some(receipt)) => match bindgen_inventory(&self.active) {
                Ok(outputs) if outputs == receipt.outputs => {
                    link_files(&self.active, &stage, &outputs)?;
                    self.write_receipt(&receipt)?;
                    println!("==> wasm-bindgen: HIT (inputs and output digests match)");
                    return Ok(());
                }
                _ => "outputs missing, extra or modified",
            },
        };
        println!("==> wasm-bindgen: MISS ({reason})");
        let can_cache = generate(&stage)?;
        let outputs = bindgen_inventory(&stage)?;
        if let Some(input) = key.filter(|_| can_cache) {
            self.write_receipt(&BindgenReceipt {
                input: input.into(),
                outputs,
            })?;
        }
        Ok(())
    }

    fn write_receipt(&self, receipt: &BindgenReceipt) -> Result<(), String> {
        let path = self.pending.join("bindgen.json");
        let bytes = serde_json::to_vec(receipt).map_err(|e| e.to_string())?;
        fs::write(&path, bytes).map_err(|e| path_error(&path, e))
    }

    /// Static assets already have a source of truth. Compare the exact selected source set
    /// directly, including bytes, and stage only those files. No separate receipt is required.
    pub fn static_files(
        &self,
        group: &str,
        source: &Path,
        include: impl Fn(&Path) -> bool,
    ) -> Result<(), String> {
        let sources: Inventory = entries(source)?
            .into_iter()
            .filter(|entry| include(&entry.path()))
            .map(|entry| {
                let path = entry.path();
                Ok((PathBuf::from(entry.file_name()), digest(&path)?))
            })
            .collect::<Result<_, String>>()?;
        let active = self.active.join(group);
        let stage = self.stage().join(group);
        create_dir_all(&stage)?;
        if inventory(&active, |_| true).as_ref() == Ok(&sources) {
            link_files(&active, &stage, &sources)?;
            println!("==> Web {group}: HIT (source and output digests match)");
        } else {
            println!("==> Web {group}: MISS (missing, extra or modified files)");
            for name in sources.keys() {
                fs::copy(source.join(name), stage.join(name))
                    .map_err(|e| path_error(&source.join(name), e))?;
            }
        }
        if inventory(&stage, |_| true)? != sources {
            return Err(format!(
                "Web {group} changed while staging assets; retry the build"
            ));
        }
        Ok(())
    }

    /// Expose only a complete package. The old directory is kept private during the switch so a
    /// rename error can restore it. Only the artifact-locked caller consumes a successful build.
    pub fn publish(self) -> Result<(), String> {
        let stage = self.stage();
        crate::web_boundary::artifacts(&stage)?;
        lifecycle::check_cancelled()?;
        let parent = self.active.parent().ok_or("Web package has no parent")?;
        create_dir_all(parent)?;
        let previous = self.state.join("previous");
        let had_active = self
            .active
            .try_exists()
            .map_err(|e| path_error(&self.active, e))?;
        if had_active {
            require_dir(&self.active)?;
            rename(&self.active, &previous)?;
        }
        if let Err(error) = rename(&stage, &self.active) {
            if had_active {
                rename(&previous, &self.active)
                    .map_err(|rollback| format!("{error}; restore previous package: {rollback}"))?;
            }
            return Err(error);
        }
        let staged_receipt = self.pending.join("bindgen.json");
        let active_receipt = self.state.join("bindgen.json");
        if staged_receipt.is_file() {
            rename(&staged_receipt, &active_receipt)?;
        } else {
            remove_file(&active_receipt)?;
        }
        remove_dir(&previous)?;
        lifecycle::check_cancelled()
    }
}

impl Drop for Package {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.pending);
    }
}

#[derive(Serialize, Deserialize)]
struct BindgenReceipt {
    input: String,
    outputs: Inventory,
}

fn bindgen_inventory(pkg: &Path) -> Result<Inventory, String> {
    let outputs = inventory(pkg, |name| {
        !PRODUCT_ASSETS.contains(&name) && name != "expressions"
    })?;
    for name in ["vink.js", "vink_bg.wasm"] {
        if !outputs.contains_key(Path::new(name))
            || fs::metadata(pkg.join(name)).map(|m| m.len()).unwrap_or(0) == 0
        {
            return Err(format!("wasm-bindgen did not produce nonempty {name}"));
        }
    }
    Ok(outputs)
}

pub fn require_local_path(root: &Path, path: &Path) -> Result<(), String> {
    let mut at = root.to_path_buf();
    for component in path
        .strip_prefix(root)
        .map_err(|e| e.to_string())?
        .components()
    {
        let std::path::Component::Normal(name) = component else {
            return Err(format!(
                "Web output path must stay in the checkout: {}",
                path.display()
            ));
        };
        at.push(name);
        match fs::symlink_metadata(&at) {
            Ok(meta) if meta.is_dir() => {}
            Ok(_) => return Err(format!("expected real directory {}", at.display())),
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(path_error(&at, e)),
        }
    }
    Ok(())
}

fn require_dir(path: &Path) -> Result<(), String> {
    let meta = fs::symlink_metadata(path).map_err(|e| path_error(path, e))?;
    if !meta.is_dir() {
        return Err(format!("expected real directory {}", path.display()));
    }
    Ok(())
}

fn inventory(dir: &Path, include: impl Fn(&str) -> bool) -> Result<Inventory, String> {
    fn walk(
        dir: &Path,
        prefix: &Path,
        into: &mut Inventory,
        include: &impl Fn(&str) -> bool,
    ) -> Result<(), String> {
        for entry in entries(dir)? {
            let name = entry.file_name();
            let name = name.to_str().ok_or("Web artifact path must be UTF-8")?;
            if prefix.as_os_str().is_empty() && !include(name) {
                continue;
            }
            let kind = entry
                .file_type()
                .map_err(|e| path_error(&entry.path(), e))?;
            let relative = prefix.join(name);
            if kind.is_dir() {
                walk(&entry.path(), &relative, into, include)?;
            } else {
                into.insert(relative, digest(&entry.path())?);
            }
        }
        Ok(())
    }
    require_dir(dir)?;
    let mut data = Inventory::new();
    walk(dir, Path::new(""), &mut data, &include)?;
    Ok(data)
}

fn link_files(from: &Path, to: &Path, files: &Inventory) -> Result<(), String> {
    for name in files.keys() {
        let to = to.join(name);
        create_dir_all(to.parent().ok_or("Web asset has no parent")?)?;
        fs::hard_link(from.join(name), &to).map_err(|e| path_error(&to, e))?;
    }
    Ok(())
}

fn digest(path: &Path) -> Result<String, String> {
    let meta = fs::symlink_metadata(path).map_err(|e| path_error(path, e))?;
    if !meta.is_file() {
        return Err(format!("expected regular file {}", path.display()));
    }
    let mut file = fs::File::open(path).map_err(|e| path_error(path, e))?;
    let mut hash = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(|e| path_error(path, e))?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(hash
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn no_external_bindgen_manifest(wasm: &Path) -> Result<bool, String> {
    let bytes = fs::metadata(wasm).map_err(|e| path_error(wasm, e))?.len();
    if bytes > 128 * 1024 * 1024 {
        return Ok(false);
    }
    let bytes = fs::read(wasm).map_err(|e| path_error(wasm, e))?;
    Ok(!bytes
        .split(|&c| c == b'p')
        .skip(1)
        .any(|suffix| suffix.starts_with(b"ackage.json")))
}

// Cargo itself sets these on macOS/Linux before running an xtask binary. Their values
// participate in the key. Explicit library injection and alternate loaders disable reuse.
fn cargo_loader_path(key: &[u8]) -> bool {
    matches!(key, b"DYLD_FALLBACK_LIBRARY_PATH" | b"LD_LIBRARY_PATH")
}

fn reusable_bindgen(version: &str) -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;

    if version != REVIEWED_BINDGEN
        || std::env::vars_os().any(|(k, _)| {
            let k = k.as_encoded_bytes();
            (k.starts_with(b"LD_") || k.starts_with(b"DYLD_")) && !cargo_loader_path(k)
        })
    {
        return None;
    }
    let cwd = std::env::current_dir().ok()?;
    let path = std::env::var_os("PATH")?;
    let executable = std::env::split_paths(&path).find_map(|dir| {
        let candidate = cwd.join(dir).join("wasm-bindgen");
        let meta = fs::metadata(&candidate).ok()?;
        (meta.is_file() && meta.permissions().mode() & 0o111 != 0).then_some(candidate)
    })?;
    let executable = executable.canonicalize().ok()?;
    let mut magic = [0; 4];
    fs::File::open(&executable)
        .ok()?
        .read_exact(&mut magic)
        .ok()?;
    // A shell/Node wrapper can consult undeclared inputs. Leave it on the normal build path.
    matches!(
        magic,
        [0x7f, b'E', b'L', b'F'] | [0xcf, 0xfa, 0xed, 0xfe] | [0xca, 0xfe, 0xba, 0xbe]
    )
    .then_some(executable)
}

fn entries(dir: &Path) -> Result<Vec<fs::DirEntry>, String> {
    fs::read_dir(dir)
        .map_err(|e| path_error(dir, e))?
        .collect::<io::Result<_>>()
        .map_err(|e| path_error(dir, e))
}

fn create_dir_all(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path).map_err(|e| path_error(path, e))
}

fn remove_file(path: &Path) -> Result<(), String> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(path_error(path, e)),
    }
}

fn remove_dir(path: &Path) -> Result<(), String> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(path_error(path, e)),
    }
}

fn rename(from: &Path, to: &Path) -> Result<(), String> {
    fs::rename(from, to).map_err(|e| format!("rename {} → {}: {e}", from.display(), to.display()))
}

fn path_error(path: &Path, error: io::Error) -> String {
    format!("{}: {error}", path.display())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::MetadataExt;

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let serial = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let dir = std::env::temp_dir()
                .join(format!("vink-web-assets-{}-{serial}", std::process::id()));
            fs::create_dir(&dir).unwrap();
            Self(dir)
        }

        fn package(&self, path: &str) -> Package {
            Package::new(&self.0, &self.0.join(path)).unwrap()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    const SSR: &str = "target/site/pkg";
    const CSR: &str = "target/spa/_v/fixture/pkg";
    const SCRATCH: &str = "target/verify-spa/_v/fixture/pkg";

    fn bindgen_fixture(stage: &Path) -> Result<bool, String> {
        fs::write(stage.join("vink.js"), "const fixture = 1;").unwrap();
        fs::write(stage.join("vink_bg.wasm"), b"wasm fixture").unwrap();
        fs::create_dir(stage.join("snippets")).unwrap();
        fs::write(stage.join("snippets/bridge.js"), "export {};").unwrap();
        Ok(true)
    }

    #[test]
    fn bindgen_receipt_rejects_damaged_output_and_different_inputs_or_paths() {
        let fixture = Fixture::new();
        let pkg = fixture.0.join(SSR);
        let build = |path: &str, key: &str| {
            let p = fixture.package(path);
            p.bindgen_step(Some(key), bindgen_fixture).unwrap();
            p.publish().unwrap();
        };
        build(SSR, "dev-hydrate");

        let warm = fixture.package(SSR);
        warm.bindgen_step(Some("dev-hydrate"), |_| panic!("unexpected bindgen"))
            .unwrap();
        warm.publish().unwrap();

        let digest_before = digest(&pkg.join("vink_bg.wasm")).unwrap();
        for (name, damaged) in [
            ("vink.js", Some("")),
            ("vink_bg.wasm", Some("WASM fixture")),
            ("snippets/bridge.js", None),
            ("unexpected.js", Some("extra")),
        ] {
            if let Some(bytes) = damaged {
                fs::write(pkg.join(name), bytes).unwrap();
            } else {
                fs::remove_file(pkg.join(name)).unwrap();
            }
            build(SSR, "dev-hydrate");
            assert_eq!(digest(&pkg.join("vink_bg.wasm")).unwrap(), digest_before);
            assert!(!pkg.join("unexpected.js").exists());
        }
        let switched = fixture.package(SSR);
        switched
            .bindgen_step(Some("release-perf-probe"), |p| {
                bindgen_fixture(p)?;
                fs::write(p.join("vink.js"), "perf fixture").unwrap();
                Ok(true)
            })
            .unwrap();
        switched.publish().unwrap();
        assert_eq!(
            fs::read_to_string(pkg.join("vink.js")).unwrap(),
            "perf fixture"
        );

        for path in [CSR, SCRATCH] {
            build(path, "release-perf-probe");
            assert_eq!(
                fs::read_to_string(fixture.0.join(path).join("vink.js")).unwrap(),
                "const fixture = 1;"
            );
        }
        let second = Fixture::new();
        let other_worktree = second.package(SSR);
        other_worktree
            .bindgen_step(Some("release-perf-probe"), bindgen_fixture)
            .unwrap();
        other_worktree.publish().unwrap();
    }

    #[test]
    fn failed_incomplete_and_interrupted_staging_do_not_certify_output() {
        let fixture = Fixture::new();
        let p = fixture.package(SSR);
        p.bindgen_step(Some("old"), bindgen_fixture).unwrap();
        p.publish().unwrap();
        let failed = fixture.package(SSR);
        let receipt = failed.state.join("bindgen.json");
        let old = fs::read(&receipt).unwrap();
        assert!(
            failed
                .bindgen_step(Some("changed"), |stage| {
                    fs::write(stage.join("vink.js"), "partial").unwrap();
                    Err("tool failure".into())
                })
                .is_err()
        );
        assert!(failed.bindgen_step(Some("changed"), |_| Ok(true)).is_err());
        assert_eq!(fs::read(&receipt).unwrap(), old);
        assert_eq!(
            fs::read_to_string(failed.active.join("vink.js")).unwrap(),
            "const fixture = 1;"
        );
        drop(failed);

        let crash = fixture.package(SSR);
        crash
            .bindgen_step(Some("changed"), bindgen_fixture)
            .unwrap();
        rename(&crash.active, &crash.state.join("previous")).unwrap();
        let abandoned_stage = crash.pending.clone();
        std::mem::forget(crash);
        let after = fixture.package(SSR);
        assert!(!abandoned_stage.exists());
        assert_eq!(fs::read(&receipt).unwrap(), old);
        after
            .bindgen_step(Some("old"), |_| panic!("lost old package after rename"))
            .unwrap();
        after.publish().unwrap();

        let missing_receipt = fixture.package(SSR);
        fs::write(receipt, b"{invalid").unwrap();
        missing_receipt
            .bindgen_step(Some("old"), bindgen_fixture)
            .unwrap();
        fs::create_dir(missing_receipt.stage().join("fixtures")).unwrap();
        assert!(
            missing_receipt
                .publish()
                .unwrap_err()
                .contains("forbidden product artifact")
        );
        assert!(!fixture.0.join(SSR).join("fixtures").exists());
    }

    #[test]
    fn static_files_reuse_verified_bytes_and_remove_deleted_assets() {
        let fixture = Fixture::new();
        let source = fixture.0.join("source");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("current.woff2"), b"font").unwrap();
        fs::write(source.join("removed.woff2"), b"old font").unwrap();
        fs::write(source.join("README.md"), b"private notes").unwrap();
        let sync = || {
            let p = fixture.package(SSR);
            p.static_files("fonts", &source, |path| {
                path.extension().is_some_and(|e| e == "woff2")
            })
            .unwrap();
            p.publish().unwrap();
        };
        sync();
        let out = fixture.0.join(SSR).join("fonts");
        let old = fs::metadata(out.join("current.woff2")).unwrap().ino();
        sync();
        assert_eq!(fs::metadata(out.join("current.woff2")).unwrap().ino(), old);
        fs::remove_file(source.join("removed.woff2")).unwrap();
        fs::write(out.join("current.woff2"), b"junk").unwrap();
        sync();
        assert!(!out.join("removed.woff2").exists());
        assert!(!out.join("README.md").exists());
        assert_eq!(fs::read(out.join("current.woff2")).unwrap(), b"font");
    }

    #[test]
    fn package_paths_refuse_symlinks_and_existing_lock_serializes_writers() {
        let fixture = Fixture::new();
        let lock_path = fixture.0.join("target/.xtask-artifacts.lock");
        let lock = lifecycle::Lock::acquire(&lock_path).unwrap();
        assert!(lifecycle::Lock::acquire(&lock_path).is_err());
        drop(lock);
        assert!(lifecycle::Lock::acquire(&lock_path).is_ok());

        std::os::unix::fs::symlink(&fixture.0, fixture.0.join("target/site")).unwrap();
        assert!(Package::new(&fixture.0, &fixture.0.join(SSR)).is_err());
    }

    #[test]
    #[ignore = "requires the real pinned wasm-bindgen CLI on PATH"]
    fn real_bindgen_reuses_then_rebuilds_for_dirty_wasm_and_profile() {
        let fixture = Fixture::new();
        let wasm = fixture.0.join("input.wasm");
        let mut bytes = *b"\0asm\x01\0\0\0\x01\x05\x01\x60\0\x01\x7f\x03\x02\x01\0\x07\x07\x01\x03get\0\0\x0a\x06\x01\x04\0\x41\x01\x0b";
        fs::write(&wasm, bytes).unwrap();
        let mtime = fs::metadata(&wasm).unwrap().modified().unwrap();
        let active = fixture.0.join(SSR);
        let build = |profile: &str, release: bool| {
            let p = fixture.package(SSR);
            p.bindgen(&wasm, "hydrate", profile, None, REVIEWED_BINDGEN, release)
                .unwrap();
            p.publish().unwrap();
            let artifact = active.join("vink_bg.wasm");
            (
                fs::metadata(&artifact).unwrap().ino(),
                digest(&artifact).unwrap(),
            )
        };
        let cold = build("dev", false);
        let warm = build("dev", false);
        assert_eq!(cold, warm);
        bytes[bytes.len() - 2] = 2;
        fs::write(&wasm, bytes).unwrap();
        fs::File::options()
            .write(true)
            .open(&wasm)
            .unwrap()
            .set_times(fs::FileTimes::new().set_modified(mtime))
            .unwrap();
        let edited = build("dev", false);
        assert_ne!(warm.1, edited.1);
        let release = build("wasm-release", true);
        assert_ne!(release.0, edited.0);
        assert_eq!(build("wasm-release", true), release);

        println!(
            "output digests cold={} edited={} release={}",
            cold.1, edited.1, release.1
        );
    }
}
