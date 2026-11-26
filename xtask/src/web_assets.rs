//! Checkout-local publication, per-artifact manifest, and provenance for the product Web package.
use crate::web_dag::WebArtifactId;
use crate::{checks, lifecycle, process, tailwind};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

type Inventory = BTreeMap<PathBuf, String>;
type Parts = BTreeMap<String, String>;
type DigestInputs = Option<(String, Parts)>;

// Re-audit external inputs when the editor bundle recipe changes.
const REVIEWED_EDITOR_RECIPE: &str =
    "6bd6a58e136329259fef867474530b5519509a04e53907735847873d07163cc1";
const MANIFEST_SCHEMA: u32 = 1;
const MANIFEST_FILE: &str = "manifest.json";
const LEGACY_BINDGEN_FILE: &str = "bindgen.json";
const EDITOR_RUNTIME_DIR: &str = "apps/vink-web/editor";
const MARKDOWN_FIXTURE: &str = "services/vink-app/tests/fixtures/title-authority/markdown.json";
const RENDER_FIXTURE: &str = "services/vink-app/tests/fixtures/title-authority/render.json";
const BINDGEN_FLAGS: &[&str] = &["--target", "web", "--no-typescript", "--out-name", "vink"];
const PRODUCT_ASSETS: &[&str] = &["vink-editor.js", "vink.css", "fonts", "brand", "empty"];
// The reviewed CLI reads npm manifests by path if the input contains package_json metadata.
// Guard reuse to this CLI version and to modules without such references.
const REVIEWED_BINDGEN: &str = "wasm-bindgen 0.2.129";

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PackageManifest {
    schema_version: u32,
    package_path: String,
    profile: String,
    feature: String,
    asset_prefix: Option<String>,
    rustc_version: String,
    node_version: String,
    wasm_bindgen_version: String,
    artifacts: BTreeMap<WebArtifactId, ArtifactReceipt>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ArtifactReceipt {
    input_digest: Option<String>,
    parts: BTreeMap<String, String>,
    outputs: Inventory,
    output_digest: String,
}

#[derive(Clone, Debug)]
struct StepLog {
    id: WebArtifactId,
    status: &'static str,
    elapsed_ms: u64,
    digest: String,
    detail: String,
}

pub struct Package {
    active: PathBuf,
    state: PathBuf,
    pending: PathBuf,
    previous_manifest: Option<PackageManifest>,
    next_manifest: PackageManifest,
    steps: Vec<StepLog>,
    started: Instant,
}

impl Package {
    /// Callers hold the existing checkout artifact lock through publication.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        root: &Path,
        active: &Path,
        profile: &str,
        feature: &str,
        asset_prefix: Option<&str>,
        rustc_version: &str,
        node_version: &str,
        wasm_bindgen_version: &str,
    ) -> Result<Self, String> {
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

        let package_path = relative.to_string_lossy().replace('\\', "/");
        let previous_manifest = fs::read(state.join(MANIFEST_FILE))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<PackageManifest>(&bytes).ok())
            .filter(|m| m.schema_version == MANIFEST_SCHEMA && m.package_path == package_path);

        let next_manifest = PackageManifest {
            schema_version: MANIFEST_SCHEMA,
            package_path,
            profile: profile.to_owned(),
            feature: feature.to_owned(),
            asset_prefix: asset_prefix.map(str::to_owned),
            rustc_version: rustc_version.to_owned(),
            node_version: node_version.to_owned(),
            wasm_bindgen_version: wasm_bindgen_version.to_owned(),
            artifacts: BTreeMap::new(),
        };

        Ok(Self {
            active: active.into(),
            state,
            pending,
            previous_manifest,
            next_manifest,
            steps: Vec::new(),
            started: Instant::now(),
        })
    }

    pub fn stage(&self) -> PathBuf {
        self.pending.join("pkg")
    }

    pub fn record_step(
        &mut self,
        id: WebArtifactId,
        status: &'static str,
        elapsed_ms: u64,
        digest: String,
        detail: String,
    ) {
        self.steps.push(StepLog {
            id,
            status,
            elapsed_ms,
            digest,
            detail,
        });
    }

    pub fn tokens(&mut self, root: &Path) -> Result<(), String> {
        let start = Instant::now();
        crate::ui_tokens::run(root, true)?;
        let spec = crate::web_dag::TOKENS_SPEC;
        let files = |names: &[&str]| -> Result<Inventory, String> {
            names
                .iter()
                .map(|name| Ok((PathBuf::from(name), digest(&root.join(name))?)))
                .collect()
        };
        let input = inventory_digest(&files(spec.input_files)?)?;
        let outputs = files(spec.outputs)?;
        let output_digest = inventory_digest(&outputs)?;
        self.record_step(
            spec.id,
            "VERIFIED",
            start.elapsed().as_millis() as u64,
            output_digest.clone(),
            format!("{} generated files match", outputs.len()),
        );
        self.next_manifest.artifacts.insert(
            spec.id,
            ArtifactReceipt {
                input_digest: Some(input),
                parts: Parts::new(),
                outputs,
                output_digest,
            },
        );
        Ok(())
    }

    pub fn record_rust_wasm(
        &mut self,
        root: &Path,
        wasm: &Path,
        mut freshness: process::CargoFreshness,
        rebuild: impl FnOnce() -> Result<process::CargoFreshness, String>,
    ) -> Result<(String, bool), String> {
        let initial_cargo_ms = freshness.elapsed_ms;
        let hashing = Instant::now();
        let mut raw = digest_and_check_bindgen_manifest(wasm)?;
        let mut parts = BTreeMap::new();
        parts.insert("profile".into(), self.next_manifest.profile.clone());
        parts.insert("feature".into(), self.next_manifest.feature.clone());
        parts.insert("rustc".into(), self.next_manifest.rustc_version.clone());
        parts.insert(
            "asset_prefix".into(),
            self.next_manifest.asset_prefix.clone().unwrap_or_default(),
        );
        parts.insert(
            "flags_env".into(),
            env_digest(|key| {
                key.starts_with(b"CARGO_PROFILE_")
                    || key.starts_with(b"CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_")
                    || key.starts_with(b"CARGO_BUILD_RUST")
                    || matches!(
                        key,
                        b"RUSTFLAGS"
                            | b"CARGO_ENCODED_RUSTFLAGS"
                            | b"RUSTUP_TOOLCHAIN"
                            | b"RUSTC"
                            | b"RUSTC_WRAPPER"
                            | b"RUSTC_WORKSPACE_WRAPPER"
                    )
            })?,
        );
        let spec = crate::web_dag::RUST_WASM_SPEC;
        let mut files = Inventory::new();
        for path in spec.input_files {
            let p = root.join(path);
            if p.is_file() {
                files.insert(PathBuf::from(path), digest(&p)?);
            }
        }
        for dir in spec.input_dirs {
            if let Some(tree) = walk_tree_digest(root, &root.join(dir))? {
                files.extend(tree)
            }
        }
        parts.insert("workspace_inputs".into(), inventory_digest(&files)?);
        let key = combine_parts(&parts)?;
        // Cargo normally checks output existence and freshness. Reject a changed raw
        // output when the last successful build and the compiler agree on these inputs.
        if freshness.rebuilt_units == 0
            && self
                .prev_receipt(spec.id)
                .is_some_and(|r| r.input_digest.as_ref() == Some(&key) && r.output_digest != raw.0)
        {
            println!(
                "==> rust-wasm: MISS (fresh Cargo output failed SHA-256; rebuilding vink-web)"
            );
            let sibling = wasm
                .parent()
                .ok_or("missing WASM target directory")?
                .join("deps/vink_web.wasm");
            // Cargo may copy or hardlink the crate output to the top-level target.
            // Discard only copies with the demonstrably mismatching bytes.
            if sibling.is_file() && digest(&sibling)? == raw.0 {
                remove_file(&sibling)?;
            }
            remove_file(wasm)?;
            let previous = raw.0;
            freshness = rebuild()?;
            raw = digest_and_check_bindgen_manifest(wasm)?;
            if freshness.rebuilt_units == 0 && raw.0 == previous {
                return Err(
                    "Cargo retained the damaged vink-web WASM; inspect the checkout target".into(),
                );
            }
        }
        let (wasm_digest, has_no_pkg_json) = raw;
        let status = if freshness.rebuilt_units == 0 {
            "HIT"
        } else {
            "MISS"
        };
        let detail = format!(
            "Cargo fresh={} rebuilt={}",
            freshness.fresh_units, freshness.rebuilt_units
        );
        let elapsed_ms = initial_cargo_ms + hashing.elapsed().as_millis() as u64;
        let mut outputs = Inventory::new();
        outputs.insert(PathBuf::from("vink_web.wasm"), wasm_digest.clone());
        self.next_manifest.artifacts.insert(
            WebArtifactId::RustWasm,
            ArtifactReceipt {
                input_digest: Some(key),
                parts,
                outputs,
                output_digest: wasm_digest.clone(),
            },
        );
        println!(
            "==> {}: {status} ({detail}, {} ms, sha256={})",
            WebArtifactId::RustWasm.name(),
            elapsed_ms,
            short_hex(&wasm_digest)
        );
        self.record_step(
            WebArtifactId::RustWasm,
            status,
            elapsed_ms,
            wasm_digest.clone(),
            detail,
        );
        Ok((wasm_digest, has_no_pkg_json))
    }

    pub fn bindgen(
        &mut self,
        wasm: &Path,
        raw: (String, bool),
        release: bool,
    ) -> Result<(), String> {
        let manifest = &self.next_manifest;
        let tool = reusable_bindgen(&manifest.wasm_bindgen_version);
        let args: Vec<_> = BINDGEN_FLAGS
            .iter()
            .copied()
            .chain(release.then_some("--remove-name-section"))
            .collect();
        let meta = serde_json::to_vec(&(
            &manifest.feature,
            &manifest.profile,
            &manifest.asset_prefix,
            &manifest.wasm_bindgen_version,
            &args,
        ))
        .map_err(|e| e.to_string())?;
        let (initial_wasm_digest, initial_no_manifest) = raw;
        let compute_input = |wasm_digest: String| -> Result<DigestInputs, String> {
            let Some(tool) = &tool else { return Ok(None) };
            let env_hash =
                env_digest(|key| key.starts_with(b"WASM_BINDGEN_") || cargo_loader_path(key))?;
            let tool_env =
                serde_json::to_vec(&(digest(tool)?, env_hash)).map_err(|e| e.to_string())?;
            let mut parts = BTreeMap::new();
            parts.insert("wasm".into(), wasm_digest);
            parts.insert(
                "profile_feature_prefix".into(),
                crate::checks::sha256_hex(&meta),
            );
            parts.insert("tool_env".into(), crate::checks::sha256_hex(&tool_env));
            let combined = combine_parts(&parts)?;
            Ok(Some((combined, parts)))
        };
        let initial = compute_input(initial_wasm_digest)?;
        let key_ref = initial.as_ref().map(|(k, _)| k.as_str());
        let parts_map = initial.as_ref().map(|(_, p)| p.clone()).unwrap_or_default();
        self.bindgen_step(key_ref, parts_map, |stage| {
            let mut cmd = Command::new(tool.as_deref().unwrap_or(Path::new("wasm-bindgen")));
            cmd.args(&args).arg("--out-dir").arg(stage).arg(wasm);
            process::run(&mut cmd, "wasm-bindgen public package")?;
            if let Some((expected_key, _)) = &initial {
                let (after_wasm, _) = digest_and_check_bindgen_manifest(wasm)?;
                let after = compute_input(after_wasm)?;
                if after.as_ref().map(|(k, _)| k) != Some(expected_key) {
                    return Err(
                        "wasm-bindgen inputs changed during generation; retry the build".into(),
                    );
                }
            }
            let can_cache = initial.is_some() && initial_no_manifest;
            if initial.is_some() && !can_cache {
                println!(
                    "==> wasm-bindgen: receipt omitted (possible external npm manifest in wasm)"
                );
            }
            Ok(can_cache)
        })
    }

    fn bindgen_step(
        &mut self,
        key: Option<&str>,
        parts: BTreeMap<String, String>,
        generate: impl FnOnce(&Path) -> Result<bool, String>,
    ) -> Result<(), String> {
        let start = Instant::now();
        let stage = self.stage();
        let prev = self.prev_receipt(WebArtifactId::WasmBindgen).cloned();
        let reason = match (key, &prev) {
            (None, _) => "tool provenance is outside the reuse boundary".into(),
            (_, None) => "manifest missing or invalid".into(),
            (Some(key), Some(receipt)) if receipt.input_digest.as_deref() != Some(key) => {
                diff_parts(Some(receipt), &parts)
            }
            (Some(_), Some(receipt)) => match bindgen_inventory(&self.active) {
                Ok(outputs) if outputs == receipt.outputs => {
                    link_files(&self.active, &stage, &outputs)?;
                    let elapsed_ms = start.elapsed().as_millis() as u64;
                    let out_digest = receipt.output_digest.clone();
                    self.next_manifest
                        .artifacts
                        .insert(WebArtifactId::WasmBindgen, receipt.clone());
                    println!(
                        "==> {}: HIT (inputs and {} output digests match, {elapsed_ms} ms, digest={})",
                        WebArtifactId::WasmBindgen.name(),
                        outputs.len(),
                        short_hex(&out_digest)
                    );
                    self.record_step(
                        WebArtifactId::WasmBindgen,
                        "HIT",
                        elapsed_ms,
                        out_digest,
                        format!("{} files reused", outputs.len()),
                    );
                    return Ok(());
                }
                _ => "outputs missing, extra or modified".into(),
            },
        };
        println!("==> {}: MISS ({reason})", WebArtifactId::WasmBindgen.name());
        let can_cache = generate(&stage)?;
        let outputs = bindgen_inventory(&stage)?;
        let out_digest = inventory_digest(&outputs)?;
        let elapsed_ms = start.elapsed().as_millis() as u64;
        if let Some(input) = key.filter(|_| can_cache) {
            self.next_manifest.artifacts.insert(
                WebArtifactId::WasmBindgen,
                ArtifactReceipt {
                    input_digest: Some(input.into()),
                    parts,
                    outputs,
                    output_digest: out_digest.clone(),
                },
            );
        }
        self.record_step(
            WebArtifactId::WasmBindgen,
            "MISS",
            elapsed_ms,
            out_digest,
            reason,
        );
        Ok(())
    }

    pub fn tailwind_css(
        &mut self,
        root: &Path,
        release: bool,
        node_version: &str,
    ) -> Result<(), String> {
        let start = Instant::now();
        tailwind::validate_web_entry(root)?;
        let bin = tailwind::ensure_installed(root)?;
        self.generated_file(
            WebArtifactId::TailwindCss,
            "vink.css",
            start,
            || tailwind_inputs(root, &bin, release, node_version),
            |out| {
                tailwind::compile(root, &root.join(tailwind::WEB_ENTRY), out, release)?;
                Ok(true)
            },
        )
    }

    pub fn editor_js(&mut self, root: &Path, node_version: &str) -> Result<(), String> {
        let start = Instant::now();
        editor_ensure_installed(root)?;
        let dir = root.join(EDITOR_RUNTIME_DIR);
        let command = || {
            let mut cmd = Command::new("node");
            cmd.current_dir(&dir).arg("scripts/build.mjs");
            cmd
        };
        if digest(&dir.join("scripts/build.mjs"))? != REVIEWED_EDITOR_RECIPE {
            return self.generated_file(
                WebArtifactId::EditorJs,
                "vink-editor.js",
                start,
                || Ok(None),
                |out| {
                    process::run(command().arg(out), "build editor with changed recipe")?;
                    Ok(false)
                },
            );
        }
        process::run(
            command().arg("--check"),
            "editor type check and Markdown corpus",
        )?;
        self.generated_file(
            WebArtifactId::EditorJs,
            "vink-editor.js",
            start,
            || editor_inputs(root, node_version),
            |out| {
                let metadata = out.with_extension("inputs.tmp");
                process::run(
                    command().arg("--bundle").arg(out).arg(&metadata),
                    "bundle editor JS",
                )?;
                let inputs: Vec<PathBuf> = serde_json::from_slice(
                    &fs::read(&metadata).map_err(|e| path_error(&metadata, e))?,
                )
                .map_err(|e| e.to_string())?;
                remove_file(&metadata)?;
                Ok(!inputs.is_empty()
                    && inputs.iter().all(|p| {
                        p.components().all(|c| matches!(c, Component::Normal(_)))
                            && (p.starts_with("src") || p.starts_with("node_modules"))
                    }))
            },
        )
    }

    fn generated_file(
        &mut self,
        id: WebArtifactId,
        name: &str,
        start: Instant,
        inputs: impl Fn() -> Result<DigestInputs, String>,
        generate: impl FnOnce(&Path) -> Result<bool, String>,
    ) -> Result<(), String> {
        let initial = inputs()?;
        let prev = self.prev_receipt(id);
        let reason = match (&initial, prev) {
            (None, _) => "inputs or tool provenance outside reuse boundary".into(),
            (_, None) => "manifest missing or invalid".into(),
            (Some((key, parts)), Some(old)) if old.input_digest.as_ref() != Some(key) => {
                diff_parts(Some(old), parts)
            }
            (Some(_), Some(old)) => {
                if single_output_inventory(&self.active.join(name), name).as_ref()
                    == Ok(&old.outputs)
                {
                    let receipt = old.clone();
                    link_files(&self.active, &self.stage(), &receipt.outputs)?;
                    self.record_step(
                        id,
                        "HIT",
                        start.elapsed().as_millis() as u64,
                        receipt.output_digest.clone(),
                        format!("{name} reused"),
                    );
                    self.next_manifest.artifacts.insert(id, receipt);
                    return Ok(());
                }
                "output missing or modified".into()
            }
        };
        println!("==> {}: MISS ({reason})", id.name());
        let out = self.stage().join(name);
        let closed = generate(&out)?;
        let outputs = single_output_inventory(&out, name)?;
        let output_digest = inventory_digest(&outputs)?;
        if let Some((key, parts)) = initial {
            if inputs()?.as_ref().map(|(k, _)| k) != Some(&key) {
                return Err(format!(
                    "{name} inputs changed during generation; retry the build"
                ));
            }
            if closed {
                self.next_manifest.artifacts.insert(
                    id,
                    ArtifactReceipt {
                        input_digest: Some(key),
                        parts,
                        outputs,
                        output_digest: output_digest.clone(),
                    },
                );
            }
        }
        self.record_step(
            id,
            "MISS",
            start.elapsed().as_millis() as u64,
            output_digest,
            reason,
        );
        Ok(())
    }

    /// Static assets already have a source of truth. Compare the exact selected source set
    /// directly, including bytes, and stage only those files.
    pub fn static_files(
        &mut self,
        id: WebArtifactId,
        group: &str,
        source: &Path,
        include: impl Fn(&Path) -> bool,
    ) -> Result<(), String> {
        let start = Instant::now();
        let sources: Inventory = entries(source)?
            .into_iter()
            .filter(|entry| include(&entry.path()))
            .map(|entry| {
                let path = entry.path();
                Ok((PathBuf::from(entry.file_name()), digest(&path)?))
            })
            .collect::<Result<_, String>>()?;
        let source_digest = inventory_digest(&sources)?;
        let active = self.active.join(group);
        let stage = self.stage().join(group);
        create_dir_all(&stage)?;
        let hit = inventory(&active, |_| true).as_ref() == Ok(&sources);
        if hit {
            link_files(&active, &stage, &sources)?;
            println!(
                "==> Web {group}: HIT (source and output digests match, digest={})",
                short_hex(&source_digest)
            );
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
        let elapsed_ms = start.elapsed().as_millis() as u64;
        let mut parts = BTreeMap::new();
        parts.insert("source_files".into(), source_digest.clone());
        let receipt_outputs: Inventory = sources
            .into_iter()
            .map(|(k, v)| (PathBuf::from(group).join(k), v))
            .collect();
        self.next_manifest.artifacts.insert(
            id,
            ArtifactReceipt {
                input_digest: Some(source_digest.clone()),
                parts,
                outputs: receipt_outputs,
                output_digest: source_digest.clone(),
            },
        );
        self.record_step(
            id,
            if hit { "HIT" } else { "MISS" },
            elapsed_ms,
            source_digest,
            if hit {
                format!("{group} reused")
            } else {
                "missing, extra or modified files".into()
            },
        );
        Ok(())
    }

    fn prev_receipt(&self, id: WebArtifactId) -> Option<&ArtifactReceipt> {
        self.previous_manifest
            .as_ref()
            .and_then(|m| m.artifacts.get(&id))
    }

    /// Switch a complete package and its unified manifest under the checkout artifact lock.
    pub fn publish(self) -> Result<(), String> {
        let stage = self.stage();
        crate::web_boundary::artifacts(&stage)?;
        for required in ["vink.js", "vink_bg.wasm", "vink-editor.js", "vink.css"] {
            let meta = fs::symlink_metadata(stage.join(required))
                .map_err(|e| format!("missing Web output {required}: {e}"))?;
            if !meta.is_file() || meta.len() == 0 {
                return Err(format!("Web output {required} must be a nonempty file"));
            }
        }
        let published_inventory = inventory(&stage, |_| true)?;
        let package_digest = inventory_digest(&published_inventory)?;

        let manifest_bytes =
            serde_json::to_vec_pretty(&self.next_manifest).map_err(|e| e.to_string())?;
        let staged_manifest = self.pending.join(MANIFEST_FILE);
        fs::write(&staged_manifest, manifest_bytes).map_err(|e| path_error(&staged_manifest, e))?;

        let parent = self.active.parent().ok_or("Web package has no parent")?;
        create_dir_all(parent)?;
        let previous = self.state.join("previous");
        let had_active = self
            .active
            .try_exists()
            .map_err(|e| path_error(&self.active, e))?;
        if had_active {
            require_dir(&self.active)?;
        }
        // Cancellation is observed before changing the active package. Once the switch starts,
        // finish it or roll it back before returning to the caller.
        lifecycle::check_cancelled()?;
        if had_active {
            exchange_packages(&stage, &self.active, &previous)?;
        } else {
            rename(&stage, &self.active)?;
        }

        let active_manifest = self.state.join(MANIFEST_FILE);
        if let Err(error) = rename(&staged_manifest, &active_manifest) {
            // A failed rename leaves the old manifest in place. Restore its package too.
            let rollback = if had_active {
                exchange_packages(&stage, &self.active, &previous)
            } else {
                rename(&self.active, &stage)
            };
            rollback
                .map_err(|rollback| format!("{error}; restore previous package: {rollback}"))?;
            return Err(error);
        }

        // Remove any legacy bindgen.json after manifest commit; defer cleanup errors safely.
        let _ = remove_file(&self.state.join(LEGACY_BINDGEN_FILE));
        if let Err(error) = remove_dir(&previous) {
            eprintln!("xtask: Web package published; deferred backup cleanup: {error}");
        }

        let total_ms = self.started.elapsed().as_millis() as u64;
        println!(
            "==> Web bundle summary (profile={}, feature={}, total={total_ms} ms, pkg_digest={}):",
            self.next_manifest.profile,
            self.next_manifest.feature,
            short_hex(&package_digest)
        );
        for step in &self.steps {
            println!(
                "  {:<13} {:<8} {:>6} ms  {}  ({})",
                step.id.name(),
                step.status,
                step.elapsed_ms,
                short_hex(&step.digest),
                step.detail
            );
        }
        Ok(())
    }
}

impl Drop for Package {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.pending);
    }
}

pub(crate) fn editor_ensure_installed(root: &Path) -> Result<(), String> {
    let mut problems = Vec::new();
    checks::editor_packages(root, &mut problems);
    if !problems.is_empty() {
        return Err(problems.join("\n"));
    }
    let dir = root.join(EDITOR_RUNTIME_DIR);
    require_local_path(root, &dir.join("node_modules"))?;
    let mut inputs = fs::read(dir.join("package.json"))
        .map_err(|e| format!("read {EDITOR_RUNTIME_DIR}/package.json: {e}"))?;
    let lock = fs::read(dir.join("package-lock.json"))
        .map_err(|e| format!("read {EDITOR_RUNTIME_DIR}/package-lock.json: {e}"))?;
    let manifest: serde_json::Value = serde_json::from_slice(&inputs)
        .map_err(|e| format!("parse {EDITOR_RUNTIME_DIR}/package.json: {e}"))?;
    inputs.push(0);
    inputs.extend(lock);

    let stamp = dir.join("node_modules/.vink-installed-lock.json");
    let tsc = dir.join("node_modules/typescript/bin/tsc");
    let esbuild_pkg = dir.join("node_modules/esbuild/package.json");
    let deps_ok = ["dependencies", "devDependencies"].iter().all(|section| {
        manifest[section]
            .as_object()
            .into_iter()
            .flatten()
            .all(|(name, _)| {
                dir.join("node_modules")
                    .join(name)
                    .join("package.json")
                    .is_file()
            })
    });
    if !tsc.is_file()
        || !esbuild_pkg.is_file()
        || !deps_ok
        || fs::read(&stamp).ok().as_ref() != Some(&inputs)
    {
        let mut install = Command::new("npm");
        install
            .current_dir(&dir)
            .args(["ci", "--ignore-scripts", "--no-audit", "--no-fund"]);
        process::run(&mut install, "npm ci (editor runtime, locked)")?;
        fs::write(&stamp, &inputs).map_err(|e| format!("write {}: {e}", stamp.display()))?;
    }
    if !tsc.is_file() || !esbuild_pkg.is_file() {
        return Err(format!(
            "editor runtime tools missing after npm ci: {}",
            dir.display()
        ));
    }
    Ok(())
}

fn tailwind_inputs(
    root: &Path,
    bin: &Path,
    release: bool,
    node_version: &str,
) -> Result<DigestInputs, String> {
    let Some(node) = reusable_node(&[
        "NODE_OPTIONS",
        "NODE_PATH",
        "BROWSERSLIST_CONFIG",
        "NAPI_RS_NATIVE_LIBRARY_PATH",
        "TAILWINDCSS_OXIDE_PATH",
    ]) else {
        return Ok(None);
    };
    #[derive(Deserialize)]
    struct CssInputs {
        css: Vec<PathBuf>,
        scanned: Vec<PathBuf>,
    }

    let tw_dir = root.join(tailwind::DIR);
    let mut probe = Command::new(&node);
    let repo = root.canonicalize().map_err(|e| path_error(root, e))?;
    probe
        .current_dir(&tw_dir)
        .args([
            "--input-type=module",
            "--eval",
            include_str!("tailwind_inputs.mjs"),
        ])
        .arg(repo.join(tailwind::WEB_ENTRY))
        .args(tailwind::WEB_SOURCE_ROOTS.iter().map(|dir| repo.join(dir)));
    let result = process::output(&mut probe, "Tailwind dependency inventory")?;
    if !result.status.success() {
        return Ok(None);
    }
    let closure: CssInputs =
        serde_json::from_slice(&result.stdout).map_err(|e| format!("Tailwind inventory: {e}"))?;
    let mut css_map = Inventory::new();
    for path in closure.css {
        let canonical = path.canonicalize().map_err(|e| path_error(&path, e))?;
        let Ok(rel) = canonical.strip_prefix(&repo) else {
            return Ok(None);
        };
        css_map.insert(rel.to_owned(), digest(&canonical)?);
    }
    let mut rust_map = Inventory::new();
    for dir in tailwind::WEB_SOURCE_ROOTS {
        let Some(tree) = walk_tree_digest(root, &root.join(dir))? else {
            return Ok(None);
        };
        rust_map.extend(tree);
    }
    let pkg_bytes = tailwind::package_inputs(&tw_dir)?;
    let mut lock_files = Inventory::new();
    lock_files.insert("package_inputs".into(), checks::sha256_hex(&pkg_bytes));
    let Some(installed) = installed_digest(&tw_dir)? else {
        return Ok(None);
    };
    lock_files.insert("node_modules".into(), installed);
    lock_files.insert(
        "bin".into(),
        digest(&fs::canonicalize(bin).map_err(|e| path_error(bin, e))?)?,
    );
    let env_hash = env_digest(|k| {
        k.starts_with(b"NODE_")
            || k.starts_with(b"LIGHTNINGCSS_")
            || k.starts_with(b"NAPI_")
            || k.starts_with(b"BROWSERSLIST")
            || k.starts_with(b"TAILWIND_")
            || k.starts_with(b"OXIDE_")
            || cargo_loader_path(k)
    })?;
    let toolchain_bytes =
        serde_json::to_vec(&(digest(&node)?, node_version, env_hash)).map_err(|e| e.to_string())?;

    let mut parts = BTreeMap::new();
    // Pin the compiler invocation and the dependency probe built into this xtask.
    let recipe = concat!(
        include_str!("tailwind.rs"),
        "\0",
        include_str!("tailwind_inputs.mjs")
    );
    parts.insert(
        "tailwind_recipe".into(),
        checks::sha256_hex(recipe.as_bytes()),
    );
    parts.insert("css".into(), inventory_digest(&css_map)?);
    parts.insert("rust_sources".into(), inventory_digest(&rust_map)?);
    parts.insert(
        "scanned_files".into(),
        checks::sha256_hex(&serde_json::to_vec(&closure.scanned).map_err(|e| e.to_string())?),
    );
    parts.insert("tailwind_lock".into(), inventory_digest(&lock_files)?);
    parts.insert("profile_flags".into(), format!("minify={release}"));
    parts.insert("toolchain_env".into(), checks::sha256_hex(&toolchain_bytes));
    let combined = combine_parts(&parts)?;
    Ok(Some((combined, parts)))
}

fn editor_inputs(root: &Path, node_version: &str) -> Result<DigestInputs, String> {
    let Some(node) = reusable_node(&["NODE_OPTIONS", "NODE_PATH", "ESBUILD_BINARY_PATH"]) else {
        return Ok(None);
    };
    let dir = root.join(EDITOR_RUNTIME_DIR);
    let Some(ts_map) = walk_tree_digest(root, &dir.join("src"))? else {
        return Ok(None);
    };
    // Guard against relative imports escaping `editor/src/` so the source closure stays complete.
    for rel in ts_map.keys() {
        let ext = rel.extension().and_then(|e| e.to_str());
        if matches!(ext, Some("ts" | "tsx" | "js" | "mjs" | "mts")) {
            let text = fs::read_to_string(root.join(rel))
                .map_err(|e| format!("read {}: {e}", rel.display()))?;
            if text.contains("'../") || text.contains("\"../") || text.contains("`../") {
                return Ok(None);
            }
        }
    }
    let Some(scripts_map) = walk_tree_digest(root, &dir.join("scripts"))? else {
        return Ok(None);
    };
    if scripts_map
        .get(Path::new("apps/vink-web/editor/scripts/build.mjs"))
        .map(String::as_str)
        != Some(REVIEWED_EDITOR_RECIPE)
    {
        return Ok(None);
    }
    let mut fixtures = Inventory::new();
    for rel in [MARKDOWN_FIXTURE, RENDER_FIXTURE] {
        fixtures.insert(PathBuf::from(rel), digest(&root.join(rel))?);
    }

    let mut lock_files = Inventory::new();
    for rel in ["package.json", "package-lock.json", "tsconfig.json"] {
        lock_files.insert(rel.into(), digest(&dir.join(rel))?);
    }
    // Custom config inheritance, resolver plugins or ambient path aliases need an open build.
    let config: serde_json::Value = match serde_json::from_slice(
        &fs::read(dir.join("tsconfig.json")).map_err(|e| e.to_string())?,
    ) {
        Ok(config) => config,
        Err(_) => return Ok(None),
    };
    if ["extends", "references"]
        .iter()
        .any(|k| config.get(k).is_some())
        || ["paths", "baseUrl", "plugins", "jsxImportSource"]
            .iter()
            .any(|k| config["compilerOptions"].get(k).is_some())
        || ts_map.keys().any(|p| {
            p.file_name()
                .is_some_and(|n| n == "tsconfig.json" || n == "package.json")
        })
    {
        return Ok(None);
    }
    let Some(installed) = installed_digest(&dir)? else {
        return Ok(None);
    };
    lock_files.insert("node_modules".into(), installed);
    let env_hash = env_digest(|k| {
        k.starts_with(b"NODE_") || k.starts_with(b"ESBUILD_") || cargo_loader_path(k)
    })?;
    let toolchain_bytes =
        serde_json::to_vec(&(digest(&node)?, node_version, env_hash)).map_err(|e| e.to_string())?;

    let mut parts = BTreeMap::new();
    parts.insert("ts_sources".into(), inventory_digest(&ts_map)?);
    parts.insert("scripts".into(), inventory_digest(&scripts_map)?);
    parts.insert("markdown_fixtures".into(), inventory_digest(&fixtures)?);
    parts.insert("editor_lock".into(), inventory_digest(&lock_files)?);
    parts.insert("toolchain_env".into(), checks::sha256_hex(&toolchain_bytes));
    let combined = combine_parts(&parts)?;
    Ok(Some((combined, parts)))
}

// npm creates relative executable links in .bin. Hash link targets and every installed
// file, including native packages. Any external link or linked directory disables reuse.
fn installed_digest(dir: &Path) -> Result<Option<String>, String> {
    fn visit(root: &Path, dir: &Path, files: &mut Inventory) -> Result<bool, String> {
        for entry in entries(dir)? {
            let path = entry.path();
            let rel = path
                .strip_prefix(root)
                .map_err(|e| e.to_string())?
                .to_owned();
            let kind = entry.file_type().map_err(|e| path_error(&path, e))?;
            if kind.is_dir() {
                if !visit(root, &path, files)? {
                    return Ok(false);
                }
            } else if kind.is_symlink() {
                let Ok(target) = path.canonicalize() else {
                    return Ok(false);
                };
                let Ok(inner) = target.strip_prefix(root) else {
                    return Ok(false);
                };
                if !target.is_file() {
                    return Ok(false);
                }
                files.insert(rel, format!("link:{}", inner.display()));
            } else if kind.is_file() {
                files.insert(rel, digest(&path)?);
            } else {
                return Ok(false);
            }
        }
        Ok(true)
    }
    let root = dir.join("node_modules");
    require_dir(&root)?;
    let root = root.canonicalize().map_err(|e| path_error(&root, e))?;
    let mut files = Inventory::new();
    if visit(&root, &root, &mut files)? {
        Ok(Some(inventory_digest(&files)?))
    } else {
        Ok(None)
    }
}

fn walk_tree_digest(root: &Path, dir: &Path) -> Result<Option<Inventory>, String> {
    fn walk(canonical_root: &Path, current: &Path, into: &mut Inventory) -> Result<bool, String> {
        let meta = match fs::symlink_metadata(current) {
            Ok(meta) => meta,
            Err(_) => return Ok(false),
        };
        if meta.file_type().is_symlink() || !meta.is_dir() {
            return Ok(false);
        }
        for entry in entries(current)? {
            let path = entry.path();
            let meta = fs::symlink_metadata(&path).map_err(|e| path_error(&path, e))?;
            if meta.file_type().is_symlink() {
                return Ok(false);
            }
            if meta.is_dir() {
                if !walk(canonical_root, &path, into)? {
                    return Ok(false);
                }
            } else if meta.is_file() {
                let canon = fs::canonicalize(&path).map_err(|e| path_error(&path, e))?;
                let Ok(rel) = canon.strip_prefix(canonical_root) else {
                    return Ok(false);
                };
                if rel.to_str().is_none() {
                    return Ok(false);
                }
                into.insert(rel.to_path_buf(), digest(&path)?);
            } else {
                return Ok(false);
            }
        }
        Ok(true)
    }

    let canonical_root =
        fs::canonicalize(root).map_err(|e| format!("canonicalize {}: {e}", root.display()))?;
    let mut out = Inventory::new();
    if walk(&canonical_root, dir, &mut out)? {
        Ok(Some(out))
    } else {
        Ok(None)
    }
}

fn combine_parts(parts: &BTreeMap<String, String>) -> Result<String, String> {
    let encoded = serde_json::to_vec(parts).map_err(|e| e.to_string())?;
    Ok(checks::sha256_hex(&encoded))
}

fn inventory_digest(inventory: &Inventory) -> Result<String, String> {
    let encoded = serde_json::to_vec(inventory).map_err(|e| e.to_string())?;
    Ok(checks::sha256_hex(&encoded))
}

fn diff_parts(prev: Option<&ArtifactReceipt>, next: &BTreeMap<String, String>) -> String {
    let Some(prev) = prev else {
        return "manifest missing or invalid".into();
    };
    let mut keys = BTreeSet::new();
    for k in prev.parts.keys().chain(next.keys()) {
        if prev.parts.get(k) != next.get(k) {
            keys.insert(k.as_str());
        }
    }
    if keys.is_empty() {
        "input digest changed".into()
    } else {
        format!(
            "changed: {}",
            keys.into_iter().collect::<Vec<_>>().join(", ")
        )
    }
}

fn short_hex(hex: &str) -> &str {
    hex.get(..12).unwrap_or(hex)
}

fn single_output_inventory(path: &Path, rel_name: &str) -> Result<Inventory, String> {
    let meta = fs::symlink_metadata(path).map_err(|e| path_error(path, e))?;
    if !meta.is_file() || meta.len() == 0 {
        return Err(format!(
            "Web output {rel_name} must be a nonempty regular file"
        ));
    }
    let mut out = Inventory::new();
    out.insert(PathBuf::from(rel_name), digest(path)?);
    Ok(out)
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
        let Component::Normal(name) = component else {
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

/// Streams `wasm` once to compute its SHA-256 digest and verify it contains no `package.json`
/// metadata reference (in O(1) memory, supporting large debug WASM outputs).
fn digest_and_check_bindgen_manifest(wasm: &Path) -> Result<(String, bool), String> {
    const NEEDLE: &[u8] = b"package.json";
    const OVERLAP: usize = NEEDLE.len() - 1;
    let meta = fs::symlink_metadata(wasm).map_err(|e| path_error(wasm, e))?;
    if !meta.is_file() {
        return Err(format!("expected regular file {}", wasm.display()));
    }
    let mut file = fs::File::open(wasm).map_err(|e| path_error(wasm, e))?;
    let mut hash = Sha256::new();
    let mut buf = vec![0u8; OVERLAP + 256 * 1024];
    let mut carry = 0usize;
    let finder = memchr::memmem::Finder::new(NEEDLE);
    let mut found = false;
    loop {
        let count = file
            .read(&mut buf[carry..])
            .map_err(|e| path_error(wasm, e))?;
        if count == 0 {
            break;
        }
        let fresh = &buf[carry..carry + count];
        hash.update(fresh);
        if !found {
            let window_len = carry + count;
            let window = &buf[..window_len];
            found = finder.find(window).is_some();
            if !found {
                let keep = window_len.min(OVERLAP);
                buf.copy_within(window_len - keep..window_len, 0);
                carry = keep;
            } else {
                carry = 0;
            }
        }
    }
    let hex: String = hash
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    Ok((hex, !found))
}

fn env_digest(include: impl Fn(&[u8]) -> bool) -> Result<String, String> {
    let vars: BTreeMap<_, _> = std::env::vars_os()
        .filter(|(k, _)| include(k.as_encoded_bytes()))
        .map(|(k, v)| (k, checks::sha256_hex(v.as_encoded_bytes())))
        .collect();
    let encoded =
        serde_json::to_vec(&vars.into_iter().collect::<Vec<_>>()).map_err(|e| e.to_string())?;
    Ok(checks::sha256_hex(&encoded))
}

// Cargo itself sets these on macOS/Linux before running an xtask binary. Their values
// participate in the key. Explicit library injection and alternate loaders disable reuse.
fn cargo_loader_path(key: &[u8]) -> bool {
    matches!(key, b"DYLD_FALLBACK_LIBRARY_PATH" | b"LD_LIBRARY_PATH")
}

fn has_unsafe_loader_env() -> bool {
    std::env::vars_os().any(|(k, _)| {
        let k = k.as_encoded_bytes();
        (k.starts_with(b"LD_") || k.starts_with(b"DYLD_")) && !cargo_loader_path(k)
    })
}

fn find_native_executable(program: &str) -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    let cwd = std::env::current_dir().ok()?;
    let path = std::env::var_os("PATH")?;
    let executable = std::env::split_paths(&path).find_map(|dir| {
        let candidate = cwd.join(dir).join(program);
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

fn reusable_bindgen(version: &str) -> Option<PathBuf> {
    if version != REVIEWED_BINDGEN || has_unsafe_loader_env() {
        return None;
    }
    find_native_executable("wasm-bindgen")
}

// The editor and Tailwind's env shebang resolve Node from their package directories.
// Relative PATH entries (including an empty entry) can name a different executable there.
fn node_search_path_is_absolute(path: &std::ffi::OsStr) -> bool {
    std::env::split_paths(path).all(|dir| dir.is_absolute())
}

fn reusable_node(forbidden_env: &[&str]) -> Option<PathBuf> {
    if !node_search_path_is_absolute(&std::env::var_os("PATH")?)
        || has_unsafe_loader_env()
        || forbidden_env
            .iter()
            .any(|key| std::env::var_os(key).is_some_and(|v| !v.is_empty()))
    {
        return None;
    }
    find_native_executable("node")
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

// Swap real directories without an interval where the served package is absent.
// The old package stays in our private staging directory until the receipt commits.
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn exchange_packages(stage: &Path, active: &Path, _backup: &Path) -> Result<(), String> {
    use rustix::fs::{CWD, RenameFlags, renameat_with};
    renameat_with(CWD, stage, CWD, active, RenameFlags::EXCHANGE).map_err(|error| {
        format!(
            "exchange {} and {}: {error}",
            stage.display(),
            active.display()
        )
    })
}

// Preserve the legacy lock-protected switch on other Unix hosts.
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn exchange_packages(stage: &Path, active: &Path, backup: &Path) -> Result<(), String> {
    rename(active, backup)?;
    if let Err(error) = rename(stage, active) {
        rename(backup, active)?;
        return Err(error);
    }
    rename(backup, stage)
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
            Package::new(
                &self.0,
                &self.0.join(path),
                "dev",
                "hydrate",
                None,
                "rustc 1.99.1",
                "v26.12.0",
                REVIEWED_BINDGEN,
            )
            .unwrap()
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

    fn stage_product_outputs(pkg: &Package) {
        let stage = pkg.stage();
        if !stage.join("vink.js").exists() {
            bindgen_fixture(&stage).unwrap();
        }
        if !stage.join("vink-editor.js").exists() {
            fs::write(stage.join("vink-editor.js"), "export {};").unwrap();
        }
        if !stage.join("vink.css").exists() {
            fs::write(stage.join("vink.css"), "body{}").unwrap();
        }
    }

    #[test]
    fn node_provenance_rejects_cwd_dependent_search_paths() {
        use std::ffi::OsStr;
        assert!(node_search_path_is_absolute(OsStr::new("/usr/bin:/bin")));
        for path in [
            "./bin:/usr/bin",
            "/usr/bin:tools",
            ":/usr/bin",
            "/usr/bin:",
            "",
        ] {
            assert!(!node_search_path_is_absolute(OsStr::new(path)), "{path:?}");
        }
    }

    #[test]
    fn bindgen_receipt_rejects_damaged_output_and_different_inputs_or_paths() {
        let fixture = Fixture::new();
        let pkg = fixture.0.join(SSR);
        let build = |path: &str, key: &str| {
            let mut p = fixture.package(path);
            let mut parts = BTreeMap::new();
            parts.insert("wasm".into(), key.into());
            p.bindgen_step(Some(key), parts, bindgen_fixture).unwrap();
            stage_product_outputs(&p);
            p.publish().unwrap();
        };
        build(SSR, "dev-hydrate");

        let mut warm = fixture.package(SSR);
        let mut parts = BTreeMap::new();
        parts.insert("wasm".into(), "dev-hydrate".into());
        warm.bindgen_step(Some("dev-hydrate"), parts, |_| panic!("unexpected bindgen"))
            .unwrap();
        stage_product_outputs(&warm);
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
        let mut switched = fixture.package(SSR);
        let mut parts = BTreeMap::new();
        parts.insert("wasm".into(), "release-perf-probe".into());
        switched
            .bindgen_step(Some("release-perf-probe"), parts, |p| {
                bindgen_fixture(p)?;
                fs::write(p.join("vink.js"), "perf fixture").unwrap();
                Ok(true)
            })
            .unwrap();
        stage_product_outputs(&switched);
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
        let mut other_worktree = second.package(SSR);
        let mut parts = BTreeMap::new();
        parts.insert("wasm".into(), "release-perf-probe".into());
        other_worktree
            .bindgen_step(Some("release-perf-probe"), parts, bindgen_fixture)
            .unwrap();
        stage_product_outputs(&other_worktree);
        other_worktree.publish().unwrap();
    }

    #[test]
    fn failed_incomplete_and_interrupted_staging_do_not_certify_output() {
        let fixture = Fixture::new();
        let mut p = fixture.package(SSR);
        p.bindgen_step(Some("old"), BTreeMap::new(), bindgen_fixture)
            .unwrap();
        stage_product_outputs(&p);
        p.publish().unwrap();
        let mut failed = fixture.package(SSR);
        let receipt = failed.state.join(MANIFEST_FILE);
        let old = fs::read(&receipt).unwrap();
        assert!(
            failed
                .bindgen_step(Some("changed"), BTreeMap::new(), |stage| {
                    fs::write(stage.join("vink.js"), "partial").unwrap();
                    Err("tool failure".into())
                })
                .is_err()
        );
        assert!(
            failed
                .bindgen_step(Some("changed"), BTreeMap::new(), |_| Ok(true))
                .is_err()
        );
        assert_eq!(fs::read(&receipt).unwrap(), old);
        assert_eq!(
            fs::read_to_string(failed.active.join("vink.js")).unwrap(),
            "const fixture = 1;"
        );
        drop(failed);

        let mut crash = fixture.package(SSR);
        crash
            .bindgen_step(Some("changed"), BTreeMap::new(), bindgen_fixture)
            .unwrap();
        rename(&crash.active, &crash.state.join("previous")).unwrap();
        let abandoned_stage = crash.pending.clone();
        std::mem::forget(crash);
        let mut after = fixture.package(SSR);
        assert!(!abandoned_stage.exists());
        assert_eq!(fs::read(&receipt).unwrap(), old);
        after
            .bindgen_step(Some("old"), BTreeMap::new(), |_| {
                panic!("lost old package after rename")
            })
            .unwrap();
        stage_product_outputs(&after);
        after.publish().unwrap();

        fs::write(&receipt, b"{invalid").unwrap();
        let mut corrupt_manifest = fixture.package(SSR);
        corrupt_manifest
            .bindgen_step(Some("old"), BTreeMap::new(), bindgen_fixture)
            .unwrap();
        stage_product_outputs(&corrupt_manifest);
        fs::create_dir(corrupt_manifest.stage().join("fixtures")).unwrap();
        assert!(
            corrupt_manifest
                .publish()
                .unwrap_err()
                .contains("forbidden product artifact")
        );
        assert!(!fixture.0.join(SSR).join("fixtures").exists());
    }

    #[test]
    fn manifest_errors_roll_back_an_existing_or_first_package() {
        for had_active in [true, false] {
            for cacheable in [true, false] {
                let fixture = Fixture::new();
                if had_active {
                    let mut old = fixture.package(SSR);
                    old.bindgen_step(Some("old"), BTreeMap::new(), bindgen_fixture)
                        .unwrap();
                    stage_product_outputs(&old);
                    old.publish().unwrap();
                }
                let mut next = fixture.package(SSR);
                let previous_outputs = inventory(&next.active, |_| true).ok();
                let receipt = next.state.join(MANIFEST_FILE);
                remove_file(&receipt).unwrap();
                // A directory at the manifest path refuses rename.
                fs::create_dir(&receipt).unwrap();
                let input = cacheable.then_some("new");
                next.bindgen_step(input, BTreeMap::new(), |stage| {
                    bindgen_fixture(stage)?;
                    fs::write(stage.join("vink.js"), "new package").unwrap();
                    Ok(cacheable)
                })
                .unwrap();
                stage_product_outputs(&next);
                let active = next.active.clone();
                let backup = next.state.join("previous");

                let error = next.publish().unwrap_err();
                assert!(error.contains(MANIFEST_FILE), "{error}");
                assert_eq!(active.exists(), had_active);
                assert_eq!(inventory(&active, |_| true).ok(), previous_outputs);
                assert!(receipt.is_dir());
                assert!(!backup.exists());
            }
        }
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
            let mut p = fixture.package(SSR);
            p.static_files(WebArtifactId::StaticFonts, "fonts", &source, |path| {
                path.extension().is_some_and(|e| e == "woff2")
            })
            .unwrap();
            stage_product_outputs(&p);
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
        assert!(
            Package::new(
                &fixture.0,
                &fixture.0.join(SSR),
                "dev",
                "hydrate",
                None,
                "rustc 1.99.1",
                "v26.12.0",
                REVIEWED_BINDGEN
            )
            .is_err()
        );
    }

    #[test]
    fn generated_assets_require_current_input_and_complete_output() {
        let fixture = Fixture::new();
        for (id, name) in [
            (WebArtifactId::TailwindCss, "vink.css"),
            (WebArtifactId::EditorJs, "vink-editor.js"),
        ] {
            let build = |key: &str, should_build: bool| {
                let mut package = fixture.package(SSR);
                bindgen_fixture(&package.stage()).unwrap();
                stage_product_outputs(&package);
                remove_file(&package.stage().join(name)).unwrap();
                let mut built = false;
                package
                    .generated_file(
                        id,
                        name,
                        Instant::now(),
                        || Ok(Some((key.to_owned(), Parts::new()))),
                        |out| {
                            built = true;
                            fs::write(out, key).unwrap();
                            Ok(true)
                        },
                    )
                    .unwrap();
                assert_eq!(built, should_build);
                package.publish().unwrap();
            };
            build("first", true);
            build("first", false);
            fs::write(fixture.0.join(SSR).join(name), "broken").unwrap();
            build("first", true);
            remove_file(&fixture.0.join(SSR).join(name)).unwrap();
            build("first", true);
            build("second", true);
            build("second", false);
        }
    }

    #[test]
    fn fresh_cargo_output_with_changed_bytes_rebuilds_just_the_damaged_wasm() {
        let fixture = Fixture::new();
        let wasm = fixture.0.join("vink_web.wasm");
        let sibling = fixture.0.join("deps/vink_web.wasm");
        fs::create_dir(sibling.parent().unwrap()).unwrap();
        let observation = |rebuilt_units| process::CargoFreshness {
            fresh_units: 1,
            rebuilt_units,
            elapsed_ms: 0,
        };
        fs::write(&wasm, b"raw v1").unwrap();
        let mut first = fixture.package(SSR);
        first
            .record_rust_wasm(&fixture.0, &wasm, observation(1), || {
                panic!("already compiled")
            })
            .unwrap();
        stage_product_outputs(&first);
        first.publish().unwrap();
        fs::write(&wasm, b"damaged").unwrap();
        fs::hard_link(&wasm, &sibling).unwrap();
        let mut next = fixture.package(SSR);
        let recovered = next
            .record_rust_wasm(&fixture.0, &wasm, observation(0), || {
                assert!(!wasm.exists() && !sibling.exists());
                fs::write(&wasm, b"raw v1").unwrap();
                Ok(observation(1))
            })
            .unwrap();
        assert_eq!(recovered.0, checks::sha256_hex(b"raw v1"));
    }

    #[test]
    fn streaming_wasm_check_detects_package_json_across_chunk_boundaries() {
        let fixture = Fixture::new();
        let clean = fixture.0.join("clean.wasm");
        let dirty = fixture.0.join("dirty.wasm");
        let mut payload = vec![b'x'; 256 * 1024 + 32];
        fs::write(&clean, &payload).unwrap();
        let (clean_hash, clean_ok) = digest_and_check_bindgen_manifest(&clean).unwrap();
        assert!(clean_ok);
        assert_eq!(clean_hash, digest(&clean).unwrap());

        // Span `package.json` exactly across the 256 KiB chunk boundary (after OVERLAP=11).
        let boundary = 256 * 1024 - 5;
        payload[boundary..boundary + 12].copy_from_slice(b"package.json");
        fs::write(&dirty, &payload).unwrap();
        let (dirty_hash, dirty_ok) = digest_and_check_bindgen_manifest(&dirty).unwrap();
        assert!(!dirty_ok);
        assert_eq!(dirty_hash, digest(&dirty).unwrap());
    }

    #[test]
    fn tailwind_and_editor_input_closures_detect_dirty_untracked_deleted_and_pin_drift() {
        let root = crate::root();
        let fixture = Fixture::new();
        // Copy only Tailwind and Editor minimal fixtures to an isolated temp root.
        let tw_dir = fixture.0.join(tailwind::DIR);
        fs::create_dir_all(&tw_dir).unwrap();
        for name in ["package.json", "package-lock.json", "web.css"] {
            fs::copy(root.join(tailwind::DIR).join(name), tw_dir.join(name)).unwrap();
        }
        tailwind::ensure_installed(&fixture.0).unwrap();
        for css in [
            "packages/ui-system/leptos/style/tailwind-theme.css",
            "packages/ui-system/leptos/style/tokens.css",
            "packages/ui-system/leptos/style/vink-ui.css",
            "packages/vink-patterns/leptos/patterns.css",
            "apps/vink-web/style/vink.css",
        ] {
            let dst = fixture.0.join(css);
            fs::create_dir_all(dst.parent().unwrap()).unwrap();
            fs::write(&dst, "/* css */\n").unwrap();
        }
        for dir in tailwind::WEB_SOURCE_ROOTS {
            fs::create_dir_all(fixture.0.join(dir)).unwrap();
        }
        fs::write(
            fixture.0.join("apps/vink-web/src/lib.rs"),
            "let c = \"flex\";",
        )
        .unwrap();
        fs::create_dir_all(fixture.0.join("services/vink-app/src")).unwrap();
        fs::write(
            fixture.0.join("services/vink-app/src/lib.rs"),
            "/* unrelated */",
        )
        .unwrap();

        let bin = tw_dir.join("node_modules/.bin/tailwindcss");
        let (base_key, base_parts) = tailwind_inputs(&fixture.0, &bin, false, "v26.12.0")
            .unwrap()
            .unwrap();

        // Unrelated crate outside @source must NOT invalidate Tailwind.
        fs::write(
            fixture.0.join("services/vink-app/src/lib.rs"),
            "/* edited */",
        )
        .unwrap();
        let (after_unrelated, _) = tailwind_inputs(&fixture.0, &bin, false, "v26.12.0")
            .unwrap()
            .unwrap();
        assert_eq!(base_key, after_unrelated);

        // Untracked file inside allowlisted Rust @source MUST change rust_sources only.
        let extra_rs = fixture.0.join("apps/vink-web/src/extra_view.rs");
        fs::write(&extra_rs, "let cls = \"p-vk-4\";").unwrap();
        let (after_rust, rust_parts) = tailwind_inputs(&fixture.0, &bin, false, "v26.12.0")
            .unwrap()
            .unwrap();
        assert_ne!(base_key, after_rust);
        assert!(
            diff_parts(
                Some(&ArtifactReceipt {
                    input_digest: Some(base_key.clone()),
                    parts: base_parts.clone(),
                    outputs: Inventory::new(),
                    output_digest: String::new(),
                }),
                &rust_parts
            )
            .contains("rust_sources")
        );
        fs::remove_file(&extra_rs).unwrap();
        assert_eq!(
            tailwind_inputs(&fixture.0, &bin, false, "v26.12.0")
                .unwrap()
                .unwrap()
                .0,
            base_key
        );

        // CSS change only changes `css`, release flag only changes `profile_flags`.
        fs::write(
            fixture.0.join("apps/vink-web/style/vink.css"),
            ".x { color: red; }",
        )
        .unwrap();
        let (_, css_parts) = tailwind_inputs(&fixture.0, &bin, false, "v26.12.0")
            .unwrap()
            .unwrap();
        assert_ne!(css_parts["css"], base_parts["css"]);
        assert_eq!(css_parts["rust_sources"], base_parts["rust_sources"]);

        let (_, rel_parts) = tailwind_inputs(&fixture.0, &bin, true, "v26.12.0")
            .unwrap()
            .unwrap();
        assert_ne!(rel_parts["profile_flags"], base_parts["profile_flags"]);

        // Symlink inside @source fails closed (disables reuse).
        std::os::unix::fs::symlink(
            fixture.0.join("apps/vink-web/src/lib.rs"),
            fixture.0.join("apps/vink-web/src/symlink.rs"),
        )
        .unwrap();
        assert!(
            tailwind_inputs(&fixture.0, &bin, false, "v26.12.0")
                .unwrap()
                .is_none()
        );

        fs::remove_file(fixture.0.join("apps/vink-web/src/symlink.rs")).unwrap();
        let nested = fixture.0.join("apps/vink-web/style/nested.css");
        fs::write(&nested, ".đẹp { opacity: .5; }").unwrap();
        fs::write(
            fixture.0.join("apps/vink-web/style/vink.css"),
            "@import './nested.css';",
        )
        .unwrap();
        let first_nested = tailwind_inputs(&fixture.0, &bin, false, "v26.12.0")
            .unwrap()
            .unwrap()
            .0;
        fs::write(&nested, ".đẹp { opacity: .7; }").unwrap();
        assert_ne!(
            first_nested,
            tailwind_inputs(&fixture.0, &bin, false, "v26.12.0")
                .unwrap()
                .unwrap()
                .0
        );
        fs::write(&nested, "@plugin './unbounded.js';").unwrap();
        assert!(
            tailwind_inputs(&fixture.0, &bin, false, "v26.12.0")
                .unwrap()
                .is_none()
        );
        // Editor input closure: TS, scripts, markdown fixtures, and parent-escape fail-closed.
        let ed_dir = fixture.0.join(EDITOR_RUNTIME_DIR);
        fs::create_dir_all(ed_dir.join("src")).unwrap();
        fs::create_dir_all(ed_dir.join("scripts")).unwrap();
        fs::create_dir_all(ed_dir.join("node_modules/typescript/bin")).unwrap();
        fs::create_dir_all(ed_dir.join("node_modules/esbuild/lib")).unwrap();
        for name in ["package.json", "package-lock.json", "tsconfig.json"] {
            fs::copy(root.join(EDITOR_RUNTIME_DIR).join(name), ed_dir.join(name)).unwrap();
        }
        let ed_manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(ed_dir.join("package.json")).unwrap()).unwrap();
        for section in ["dependencies", "devDependencies"] {
            for (name, _) in ed_manifest[section].as_object().into_iter().flatten() {
                let p = ed_dir.join("node_modules").join(name).join("package.json");
                fs::create_dir_all(p.parent().unwrap()).unwrap();
                fs::write(&p, "{}").unwrap();
            }
        }
        fs::write(ed_dir.join("node_modules/.package-lock.json"), "{}").unwrap();
        fs::write(ed_dir.join("node_modules/typescript/bin/tsc"), "").unwrap();
        fs::write(ed_dir.join("node_modules/esbuild/lib/main.js"), "").unwrap();
        fs::write(ed_dir.join("src/editor.ts"), "import './markdown';\n").unwrap();
        fs::write(ed_dir.join("src/markdown.ts"), "export const x = 1;\n").unwrap();
        fs::copy(
            root.join(EDITOR_RUNTIME_DIR).join("scripts/build.mjs"),
            ed_dir.join("scripts/build.mjs"),
        )
        .unwrap();
        for fx in [MARKDOWN_FIXTURE, RENDER_FIXTURE] {
            let p = fixture.0.join(fx);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, "[]").unwrap();
        }

        let (ed_base, ed_parts) = editor_inputs(&fixture.0, "v26.12.0").unwrap().unwrap();
        fs::write(fixture.0.join(MARKDOWN_FIXTURE), "[{\"id\":\"a\"}]").unwrap();
        let (ed_fx, ed_fx_parts) = editor_inputs(&fixture.0, "v26.12.0").unwrap().unwrap();
        assert_ne!(ed_base, ed_fx);
        assert_ne!(
            ed_fx_parts["markdown_fixtures"],
            ed_parts["markdown_fixtures"]
        );
        assert_eq!(ed_fx_parts["ts_sources"], ed_parts["ts_sources"]);

        let transitive = ed_dir.join("node_modules/@codemirror/view/dist/index.js");
        fs::create_dir_all(transitive.parent().unwrap()).unwrap();
        fs::write(&transitive, "edited installed JS").unwrap();
        assert_ne!(
            editor_inputs(&fixture.0, "v26.12.0").unwrap().unwrap().0,
            ed_fx
        );
        fs::remove_file(&transitive).unwrap();
        // Relative import escaping `editor/src/` disables reuse cleanly.
        fs::write(ed_dir.join("src/editor.ts"), "import '../../outside';\n").unwrap();
        assert!(editor_inputs(&fixture.0, "v26.12.0").unwrap().is_none());
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
            let mut p = Package::new(
                &fixture.0,
                &fixture.0.join(SSR),
                profile,
                "hydrate",
                None,
                "rustc 1.99.1",
                "v26.12.0",
                REVIEWED_BINDGEN,
            )
            .unwrap();
            p.bindgen(
                &wasm,
                digest_and_check_bindgen_manifest(&wasm).unwrap(),
                release,
            )
            .unwrap();
            stage_product_outputs(&p);
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
