//! Dependency facts read from the checkout's own manifests and entry files. The planner never
//! keeps a second copy of these graphs: Cargo owns crate edges, targets and features; the
//! Tailwind entries own their CSS imports and scanned sources; the Kani manifests own the
//! production modules they compile; ES module specifiers own browser-fixture imports.
use crate::plan::UnitOwner;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[derive(Clone, Debug)]
pub struct Target {
    pub kind: String,
    pub name: String,
    pub src: String,
}

#[derive(Clone, Debug)]
pub struct Package {
    pub name: String,
    /// Repository-relative directory with a trailing slash.
    pub dir: String,
    pub owner: Option<UnitOwner>,
    pub targets: Vec<Target>,
    pub features: BTreeSet<String>,
    /// Workspace path dependencies of every kind (normal, dev, build; optional included).
    pub deps: BTreeSet<String>,
}

impl Package {
    pub fn label(&self) -> String {
        match self.owner {
            Some(owner) => format!("{} ({})", owner.name(), self.name),
            None => self.name.clone(),
        }
    }

    pub fn tests(&self) -> impl Iterator<Item = &Target> {
        self.targets.iter().filter(|t| t.kind == "test")
    }
}

#[derive(Clone, Debug)]
pub struct Stylesheet {
    pub entry: String,
    /// Local CSS files reachable through `@import`, including the entry.
    pub files: BTreeSet<String>,
    /// `@source` directories whose class names Tailwind scans.
    pub sources: BTreeSet<String>,
}

#[derive(Clone, Debug)]
pub struct Graph {
    pub packages: Vec<Package>,
    /// Module path -> modules that import it (relative ES specifiers only).
    pub importers: BTreeMap<String, BTreeSet<String>>,
    pub stylesheets: Vec<Stylesheet>,
    /// Directory or file prefixes compiled or read by an owner-local Kani manifest.
    pub kani_inputs: BTreeSet<String>,
    /// Non-module compile inputs (file, or directory with a trailing slash) -> the Rust
    /// sources that embed them: `include_str!`/`include_bytes!`, `#[wasm_bindgen(module)]`,
    /// `sqlx::migrate!` and `#[path]`.
    pub includes: BTreeMap<String, BTreeSet<String>>,
}

pub const BROWSER_DIRS: [&str; 2] = [
    "apps/vink-web/e2e/",
    "packages/ui-system/leptos/preview/e2e/",
];

impl Graph {
    pub fn load(root: &Path) -> Result<Self, String> {
        let packages = cargo_packages(root)?;
        let mut importers: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for dir in BROWSER_DIRS {
            for (module, imports) in module_imports(root, dir)? {
                for import in imports {
                    importers.entry(import).or_default().insert(module.clone());
                }
            }
        }
        let stylesheets = [crate::tailwind::WEB_ENTRY, crate::tailwind::PREVIEW_ENTRY]
            .into_iter()
            .map(|entry| stylesheet(root, entry))
            .collect::<Result<_, _>>()?;
        let includes = compile_inputs(root, &packages)?;
        Ok(Self {
            packages,
            importers,
            stylesheets,
            kani_inputs: kani_inputs(root)?,
            includes,
        })
    }

    /// Rust sources that embed `path` or a directory containing it.
    pub fn includers(&self, path: &str) -> Vec<String> {
        let mut found = BTreeSet::new();
        for (input, sources) in &self.includes {
            let hit = if input.ends_with('/') {
                path.starts_with(input.as_str())
            } else {
                input == path
            };
            if hit {
                found.extend(sources.iter().filter(|s| s.as_str() != path).cloned());
            }
        }
        found.into_iter().collect()
    }

    pub fn package_of(&self, path: &str) -> Option<&Package> {
        self.packages
            .iter()
            .filter(|p| path.starts_with(&p.dir))
            .max_by_key(|p| p.dir.len())
    }

    #[cfg(test)]
    pub fn package(&self, name: &str) -> Option<&Package> {
        self.packages.iter().find(|p| p.name == name)
    }

    /// Workspace packages that depend on `name`, directly or transitively.
    pub fn dependents(&self, name: &str) -> Vec<&Package> {
        let mut found = BTreeSet::new();
        let mut queue = vec![name.to_string()];
        while let Some(current) = queue.pop() {
            for package in &self.packages {
                if package.deps.contains(&current) && found.insert(package.name.clone()) {
                    queue.push(package.name.clone());
                }
            }
        }
        found.remove(name);
        self.packages
            .iter()
            .filter(|p| found.contains(&p.name))
            .collect()
    }

    /// Modules that import `module`, transitively, excluding `module`.
    pub fn importer_closure(&self, module: &str) -> BTreeSet<String> {
        let mut found = BTreeSet::new();
        let mut queue = vec![module.to_string()];
        while let Some(current) = queue.pop() {
            for importer in self.importers.get(&current).into_iter().flatten() {
                if found.insert(importer.clone()) {
                    queue.push(importer.clone());
                }
            }
        }
        found.remove(module);
        found
    }
}

fn relative(root: &Path, absolute: &str) -> Result<String, String> {
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let path = Path::new(absolute);
    let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    path.strip_prefix(&root)
        .map(|p| p.to_string_lossy().into_owned())
        .map_err(|_| format!("Cargo reported {absolute} outside the checkout"))
}

/// `cargo metadata --no-deps --offline`: manifests only; no resolution, build or network.
fn cargo_packages(root: &Path) -> Result<Vec<Package>, String> {
    let mut command = crate::process::cargo(root);
    command
        .args([
            "metadata",
            "--format-version",
            "1",
            "--no-deps",
            "--offline",
        ])
        .env("CARGO_NET_OFFLINE", "true");
    let out = command
        .output()
        .map_err(|e| format!("cargo metadata: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "cargo metadata (read-only manifest graph) failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let meta: serde_json::Value =
        serde_json::from_slice(&out.stdout).map_err(|e| format!("cargo metadata JSON: {e}"))?;
    let members: BTreeSet<&str> = meta["workspace_members"]
        .as_array()
        .ok_or("cargo metadata: missing workspace_members")?
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    let names: BTreeSet<&str> = meta["packages"]
        .as_array()
        .ok_or("cargo metadata: missing packages")?
        .iter()
        .filter_map(|p| p["name"].as_str())
        .collect();
    let mut packages = Vec::new();
    for p in meta["packages"].as_array().into_iter().flatten() {
        if !members.contains(p["id"].as_str().unwrap_or_default()) {
            continue;
        }
        let name = p["name"].as_str().ok_or("cargo metadata: package name")?;
        let manifest = relative(root, p["manifest_path"].as_str().unwrap_or_default())?;
        let dir = match manifest.strip_suffix("Cargo.toml") {
            Some(dir) if !dir.is_empty() => dir.to_string(),
            _ => return Err(format!("{name}: workspace root packages are not supported")),
        };
        let mut targets = Vec::new();
        for t in p["targets"].as_array().into_iter().flatten() {
            targets.push(Target {
                kind: t["kind"][0].as_str().unwrap_or_default().to_string(),
                name: t["name"].as_str().unwrap_or_default().to_string(),
                src: relative(root, t["src_path"].as_str().unwrap_or_default())?,
            });
        }
        let deps = p["dependencies"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|d| d.get("path").is_some_and(|p| !p.is_null()))
            .filter_map(|d| d["name"].as_str())
            .filter(|d| names.contains(d))
            .map(str::to_string)
            .collect();
        packages.push(Package {
            name: name.into(),
            owner: owner_for(name),
            dir,
            targets,
            features: p["features"]
                .as_object()
                .map(|f| f.keys().cloned().collect())
                .unwrap_or_default(),
            deps,
        });
    }
    if packages.is_empty() {
        return Err("cargo metadata listed no workspace packages".into());
    }
    Ok(packages)
}

fn owner_for(package: &str) -> Option<UnitOwner> {
    UnitOwner::ALL.into_iter().find(|o| o.package() == package)
}

/// Joins a relative specifier to the importing directory and normalizes `.`/`..`.
pub fn join(dir: &str, spec: &str) -> Option<String> {
    let mut parts: Vec<&str> = dir.split('/').filter(|p| !p.is_empty()).collect();
    for part in spec.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            other => parts.push(other),
        }
    }
    Some(parts.join("/"))
}

/// Quoted string after `marker` on one line, if any.
fn quoted_after<'a>(line: &'a str, marker: &str) -> Option<&'a str> {
    let rest = &line[line.find(marker)? + marker.len()..];
    let rest = rest.trim_start();
    let quote = rest.chars().next().filter(|c| *c == '\'' || *c == '"')?;
    let body = &rest[1..];
    Some(&body[..body.find(quote)?])
}

/// Statement-level `import … from '…'`, `export … from '…'` and bare `import '…'` specifiers.
/// Specifiers inside template strings (evaluated in a browser or bundler) are not imports.
pub fn specifiers(source: &str) -> Vec<String> {
    let mut found = Vec::new();
    for line in source.lines() {
        let line = line.trim_start();
        // Dynamic `import('./x.mjs')` with a literal relative specifier (phase-local modules).
        for (at, _) in line.match_indices("import(") {
            if let Some(spec) = quoted_after(&line[at..], "import(")
                && spec.starts_with("./")
            {
                found.push(spec.to_string());
            }
        }
        if !(line.starts_with("import") || line.starts_with("export") || line.starts_with('}')) {
            continue;
        }
        if let Some(spec) = quoted_after(line, " from").or_else(|| quoted_after(line, "import"))
            && spec.starts_with('.')
        {
            found.push(spec.to_string());
        }
    }
    found
}

fn module_imports(root: &Path, dir: &str) -> Result<BTreeMap<String, Vec<String>>, String> {
    let mut modules = BTreeMap::new();
    let entries = std::fs::read_dir(root.join(dir)).map_err(|e| format!("read {dir}: {e}"))?;
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.ends_with(".mjs") {
            continue;
        }
        let module = format!("{dir}{name}");
        let source = std::fs::read_to_string(entry.path()).map_err(|e| format!("{module}: {e}"))?;
        let imports = specifiers(&source)
            .into_iter()
            .filter_map(|spec| join(dir, &spec))
            .filter(|path| root.join(path).is_file())
            .collect();
        modules.insert(module, imports);
    }
    Ok(modules)
}

/// Follows local `@import` files and collects `@source` directories from a Tailwind entry.
fn stylesheet(root: &Path, entry: &str) -> Result<Stylesheet, String> {
    let mut sheet = Stylesheet {
        entry: entry.into(),
        files: BTreeSet::new(),
        sources: BTreeSet::new(),
    };
    let mut queue = vec![entry.to_string()];
    while let Some(file) = queue.pop() {
        if !sheet.files.insert(file.clone()) {
            continue;
        }
        let source =
            std::fs::read_to_string(root.join(&file)).map_err(|e| format!("{file}: {e}"))?;
        let dir = file.rsplit_once('/').map_or("", |(dir, _)| dir);
        for line in source.lines() {
            let line = line.trim_start();
            for (marker, is_import) in [("@import", true), ("@source", false)] {
                let Some(spec) = line
                    .strip_prefix(marker)
                    .and_then(|_| quoted_after(line, marker))
                else {
                    continue;
                };
                if !spec.starts_with('.') {
                    continue; // npm package inputs are pinned by the Tailwind package lock
                }
                let path = join(dir, spec).ok_or_else(|| format!("{file}: bad path {spec}"))?;
                if !root.join(&path).exists() {
                    return Err(format!("{file}: {marker} {spec} does not exist"));
                }
                if is_import {
                    queue.push(path);
                } else {
                    sheet.sources.insert(format!("{path}/"));
                }
            }
        }
    }
    Ok(sheet)
}

/// Inputs of each owner-local Kani manifest: the manifest directory, its `[lib] path`, the
/// production modules it mounts with `#[path]` and its path dependencies.
fn kani_inputs(root: &Path) -> Result<BTreeSet<String>, String> {
    let mut inputs = BTreeSet::new();
    for (manifest, harnesses) in crate::kani::MANIFESTS {
        let dir = manifest.trim_end_matches("Cargo.toml");
        inputs.insert(dir.to_string());
        inputs.extend(harnesses.iter().map(|h| h.to_string()));
        let toml: toml::Value = toml::from_str(
            &std::fs::read_to_string(root.join(manifest))
                .map_err(|e| format!("{manifest}: {e}"))?,
        )
        .map_err(|e| format!("{manifest}: {e}"))?;
        let lib = toml
            .get("lib")
            .and_then(|l| l.get("path"))
            .and_then(|p| p.as_str())
            .unwrap_or("src/lib.rs");
        let lib = join(dir, lib).ok_or_else(|| format!("{manifest}: bad lib path"))?;
        let lib_dir = lib.rsplit_once('/').map_or("", |(d, _)| d).to_string();
        let source = std::fs::read_to_string(root.join(&lib)).map_err(|e| format!("{lib}: {e}"))?;
        // A library outside the manifest directory is the production crate root itself.
        if !lib.starts_with(dir) {
            inputs.insert(format!("{lib_dir}/"));
        }
        for line in source.lines() {
            if let Some(module) = quoted_after(line.trim_start(), "#[path =") {
                let path = join(&lib_dir, module).ok_or_else(|| format!("{lib}: bad #[path]"))?;
                let module_dir = path.rsplit_once('/').map_or("", |(d, _)| d);
                inputs.insert(format!("{module_dir}/"));
            }
        }
        for (_, dep) in toml
            .get("dependencies")
            .and_then(|d| d.as_table())
            .into_iter()
            .flatten()
        {
            if let Some(path) = dep.get("path").and_then(|p| p.as_str()) {
                let dep_dir = join(dir, path).ok_or_else(|| format!("{manifest}: bad path dep"))?;
                inputs.insert(format!("{dep_dir}/"));
            }
        }
    }
    for input in &inputs {
        if !root.join(input).exists() {
            return Err(format!("Kani input {input} does not exist"));
        }
    }
    Ok(inputs)
}

/// The string literal starting at `text` (after optional whitespace), if any.
fn literal(text: &str) -> Option<&str> {
    let text = text.trim_start();
    let body = text.strip_prefix('"')?;
    let end = body.find('"')?;
    Some(&body[..end])
}

/// Embedded files of every workspace Rust source. Runtime reads (`fs::read` of a fixture
/// directory) are not visible here; the registry names those directories explicitly.
fn compile_inputs(
    root: &Path,
    packages: &[Package],
) -> Result<BTreeMap<String, BTreeSet<String>>, String> {
    let mut inputs: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for package in packages {
        let mut files = Vec::new();
        for sub in ["src", "tests", "examples", "benches"] {
            rust_files(root, &format!("{}{sub}", package.dir), &mut files)?;
        }
        if root.join(&package.dir).join("build.rs").is_file() {
            files.push(format!("{}build.rs", package.dir));
        }
        let crate_dir = package.dir.trim_end_matches('/');
        for file in files {
            let source =
                std::fs::read_to_string(root.join(&file)).map_err(|e| format!("{file}: {e}"))?;
            let file_dir = file.rsplit_once('/').map_or("", |(d, _)| d);
            let mut found = Vec::new();
            for marker in ["include_str!(", "include_bytes!("] {
                for (at, _) in source.match_indices(marker) {
                    let rest = &source[at + marker.len()..];
                    if let Some(lit) = literal(rest) {
                        found.push(join(file_dir, lit));
                    } else if let Some(after) = rest
                        .trim_start()
                        .strip_prefix("concat!(env!(\"CARGO_MANIFEST_DIR\"),")
                        && let Some(lit) = literal(after)
                    {
                        found.push(join(crate_dir, lit.trim_start_matches('/')));
                    }
                }
            }
            for (at, _) in source.match_indices("module = ") {
                if let Some(lit) = literal(&source[at + 9..]).filter(|l| l.starts_with('/')) {
                    found.push(join(crate_dir, &lit[1..]));
                }
            }
            for (at, _) in source.match_indices("migrate!(") {
                let rest = &source[at + 9..];
                let lit = literal(rest).unwrap_or("./migrations");
                found.push(join(crate_dir, lit));
            }
            for (at, _) in source.match_indices("#[path = ") {
                if let Some(lit) = literal(&source[at + 9..]) {
                    found.push(join(file_dir, lit));
                }
            }
            for path in found.into_iter().flatten() {
                let full = root.join(&path);
                let key = if full.is_dir() {
                    format!("{path}/")
                } else if full.is_file() {
                    path
                } else {
                    continue; // literal inside a test string or generated at build time
                };
                inputs.entry(key).or_default().insert(file.clone());
            }
        }
    }
    Ok(inputs)
}

fn rust_files(root: &Path, dir: &str, out: &mut Vec<String>) -> Result<(), String> {
    let Ok(entries) = std::fs::read_dir(root.join(dir)) else {
        return Ok(());
    };
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = format!("{dir}/{name}");
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        if kind.is_dir() && name != "node_modules" && name != "target" {
            rust_files(root, &path, out)?;
        } else if kind.is_file() && name.ends_with(".rs") {
            out.push(path);
        }
    }
    Ok(())
}
