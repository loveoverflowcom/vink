//! Pure Web Build DAG & Artifact Inputs Contract for `build` and `#501` diff-aware planning.
//!
//! Declares every artifact node, its transitive dependencies, file/directory inputs, relevant
//! environment variables, tool provenance, profile/feature sensitivity, and reuse strategy.
//! Contains no I/O or environment reads so `--plan` and `verify changed` can inspect it safely.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WebArtifactId {
    Tokens,
    WebBoundary,
    RustWasm,
    WasmBindgen,
    TailwindCss,
    EditorJs,
    StaticFonts,
    StaticBrand,
    StaticEmpty,
    SpaIndex,
}

impl WebArtifactId {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Tokens => "tokens",
            Self::WebBoundary => "web-boundary",
            Self::RustWasm => "rust-wasm",
            Self::WasmBindgen => "wasm-bindgen",
            Self::TailwindCss => "tailwind-css",
            Self::EditorJs => "editor-js",
            Self::StaticFonts => "static-fonts",
            Self::StaticBrand => "static-brand",
            Self::StaticEmpty => "static-empty",
            Self::SpaIndex => "spa-index",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WebArtifactSpec {
    pub id: WebArtifactId,
    pub depends_on: &'static [WebArtifactId],
    pub input_files: &'static [&'static str],
    pub input_dirs: &'static [&'static str],
    pub relevant_env: &'static [&'static str],
    pub tools: &'static [&'static str],
    pub profile_sensitive: bool,
    pub feature_sensitive: bool,
    pub prefix_sensitive: bool,
    pub outputs: &'static [&'static str],
    pub reuse_strategy: &'static str,
}

pub(crate) const TOKENS_SPEC: WebArtifactSpec = WebArtifactSpec {
    id: WebArtifactId::Tokens,
    depends_on: &[],
    input_files: &[
        "packages/ui-system/shared/tokens.json",
        "packages/ui-system/shared/preview-catalog.json",
        "packages/ui-system/shared/icons.json",
        "packages/ui-system/shared/fixtures/foundation.json",
        "packages/ui-system/shared/fixtures/identity.json",
        "packages/ui-system/shared/fixtures/states.json",
        "packages/ui-system/shared/fixtures/navigation.json",
        "packages/ui-system/shared/fixtures/toggle.json",
        "packages/ui-system/leptos/style/tokens.css",
        "packages/ui-system/leptos/style/tailwind-theme.css",
        "packages/ui-system/leptos/src/tokens.rs",
        "packages/ui-system/leptos/src/catalog.rs",
        "packages/ui-system/leptos/src/icons.rs",
        "packages/ui-system/leptos/src/identity_vectors.rs",
        "packages/ui-system/leptos/src/state_vectors.rs",
        "packages/ui-system/leptos/src/navigation_vectors.rs",
        "packages/ui-system/leptos/src/toggle_vectors.rs",
        crate::ui_tokens::KOTLIN_TOKENS_OUT,
        crate::ui_tokens::KOTLIN_CATALOG_OUT,
        crate::ui_tokens::KOTLIN_ICONS_OUT,
        crate::ui_tokens::KOTLIN_IDENTITY_OUT,
        crate::ui_states::KOTLIN_STATES_OUT,
        crate::ui_navigation::KOTLIN_NAVIGATION_OUT,
        crate::ui_toggle::KOTLIN_TOGGLE_OUT,
    ],
    input_dirs: &[
        "packages/ui-system/shared/icons/",
        "apps/vink-web/assets/fonts/",
    ],
    relevant_env: &[],
    tools: &["xtask"],
    profile_sensitive: false,
    feature_sensitive: false,
    prefix_sensitive: false,
    outputs: &[
        "packages/ui-system/leptos/style/tokens.css",
        "packages/ui-system/leptos/style/tailwind-theme.css",
        "packages/ui-system/leptos/src/tokens.rs",
        "packages/ui-system/leptos/src/catalog.rs",
        "packages/ui-system/leptos/src/icons.rs",
        "packages/ui-system/leptos/src/identity_vectors.rs",
        "packages/ui-system/leptos/src/state_vectors.rs",
        "packages/ui-system/leptos/src/navigation_vectors.rs",
        "packages/ui-system/leptos/src/toggle_vectors.rs",
        crate::ui_tokens::KOTLIN_TOKENS_OUT,
        crate::ui_tokens::KOTLIN_CATALOG_OUT,
        crate::ui_tokens::KOTLIN_ICONS_OUT,
        crate::ui_tokens::KOTLIN_IDENTITY_OUT,
        crate::ui_states::KOTLIN_STATES_OUT,
        crate::ui_navigation::KOTLIN_NAVIGATION_OUT,
        crate::ui_toggle::KOTLIN_TOGGLE_OUT,
    ],
    reuse_strategy: "validated-every-run",
};

const WEB_BOUNDARY_SPEC: WebArtifactSpec = WebArtifactSpec {
    id: WebArtifactId::WebBoundary,
    depends_on: &[],
    input_files: &[
        "Cargo.toml",
        "Cargo.lock",
        "rust-toolchain.toml",
        ".cargo/config.toml",
        "apps/vink-web/src/lib.rs",
        "apps/vink-web/Cargo.toml",
        "packages/expressions/Cargo.toml",
        "packages/markdown-text/Cargo.toml",
        "packages/store-postgres/Cargo.toml",
        "packages/ui-system/leptos/Cargo.toml",
        "packages/ui-system/leptos/preview/Cargo.toml",
        "packages/vink-patterns/leptos/Cargo.toml",
        "packages/work-view-policy/Cargo.toml",
        "services/vink-app/Cargo.toml",
        "services/vink-identity/Cargo.toml",
        "services/vink-notifications/Cargo.toml",
        "xtask/Cargo.toml",
    ],
    input_dirs: &[],
    relevant_env: &["DYLD_FALLBACK_LIBRARY_PATH", "LD_LIBRARY_PATH"],
    tools: &["cargo", "rustc"],
    profile_sensitive: false,
    feature_sensitive: false,
    prefix_sensitive: false,
    outputs: &["route-and-normal-graph-admission"],
    reuse_strategy: "routes-and-cargo-trees-every-run",
};

pub(crate) const RUST_WASM_SPEC: WebArtifactSpec = WebArtifactSpec {
    id: WebArtifactId::RustWasm,
    depends_on: &[WebArtifactId::Tokens, WebArtifactId::WebBoundary],
    input_files: &[
        "Cargo.toml",
        "Cargo.lock",
        "rust-toolchain.toml",
        ".cargo/config.toml",
        "apps/vink-web/Cargo.toml",
        "apps/vink-web/appearance.js",
        "apps/vink-web/editor/bridge.js",
        "apps/vink-web/profile/navigation.js",
        "apps/vink-web/public-cache/indexeddb.js",
        "packages/ui-system/leptos/Cargo.toml",
        "packages/vink-patterns/leptos/Cargo.toml",
        "packages/markdown-text/Cargo.toml",
        "packages/work-view-policy/Cargo.toml",
    ],
    input_dirs: &[
        "apps/vink-web/src/",
        "packages/ui-system/leptos/src/",
        "packages/vink-patterns/leptos/src/",
        "packages/markdown-text/src/",
        "packages/work-view-policy/src/",
    ],
    relevant_env: &[
        "VINK_ASSET_PREFIX",
        "RUSTFLAGS",
        "CARGO_ENCODED_RUSTFLAGS",
        "CARGO_PROFILE_*",
        "CARGO_BUILD_*",
        "RUSTUP_TOOLCHAIN",
    ],
    tools: &["cargo", "rustc", "wasm32-unknown-unknown"],
    profile_sensitive: true,
    feature_sensitive: true,
    prefix_sensitive: true,
    outputs: &["target/wasm32-unknown-unknown/{debug,wasm-release}/vink_web.wasm"],
    reuse_strategy: "cargo-freshness",
};

const WASM_BINDGEN_SPEC: WebArtifactSpec = WebArtifactSpec {
    id: WebArtifactId::WasmBindgen,
    depends_on: &[WebArtifactId::RustWasm],
    input_files: &["Cargo.lock"],
    input_dirs: &[],
    relevant_env: &[
        "VINK_ASSET_PREFIX",
        "WASM_BINDGEN_*",
        "DYLD_FALLBACK_LIBRARY_PATH",
        "LD_LIBRARY_PATH",
    ],
    tools: &["wasm-bindgen"],
    profile_sensitive: true,
    feature_sensitive: true,
    prefix_sensitive: true,
    outputs: &["vink.js", "vink_bg.wasm", "snippets/**"],
    reuse_strategy: "per-artifact-receipt",
};

const TAILWIND_CSS_SPEC: WebArtifactSpec = WebArtifactSpec {
    id: WebArtifactId::TailwindCss,
    depends_on: &[WebArtifactId::Tokens],
    input_files: &[
        "packages/ui-system/leptos/tailwind/web.css",
        "packages/ui-system/leptos/tailwind/package.json",
        "packages/ui-system/leptos/tailwind/package-lock.json",
        "packages/ui-system/leptos/style/tailwind-theme.css",
        "packages/ui-system/leptos/style/tokens.css",
        "packages/ui-system/leptos/style/vink-ui.css",
        "packages/vink-patterns/leptos/patterns.css",
        "apps/vink-web/style/vink.css",
    ],
    input_dirs: &[
        "apps/vink-web/src/",
        "packages/ui-system/leptos/src/",
        "packages/vink-patterns/leptos/src/",
    ],
    relevant_env: &[
        "BROWSERSLIST*",
        "TAILWIND_*",
        "OXIDE_*",
        "NODE_OPTIONS",
        "NODE_PATH",
        "DYLD_FALLBACK_LIBRARY_PATH",
        "LD_LIBRARY_PATH",
    ],
    tools: &["node", "npm", "@tailwindcss/cli"],
    profile_sensitive: true,
    feature_sensitive: false,
    prefix_sensitive: false,
    outputs: &["vink.css"],
    reuse_strategy: "per-artifact-receipt",
};

const EDITOR_JS_SPEC: WebArtifactSpec = WebArtifactSpec {
    id: WebArtifactId::EditorJs,
    depends_on: &[],
    input_files: &[
        "apps/vink-web/editor/package.json",
        "apps/vink-web/editor/package-lock.json",
        "apps/vink-web/editor/tsconfig.json",
        "services/vink-app/tests/fixtures/title-authority/markdown.json",
        "services/vink-app/tests/fixtures/title-authority/render.json",
    ],
    input_dirs: &["apps/vink-web/editor/src/", "apps/vink-web/editor/scripts/"],
    relevant_env: &[
        "NODE_ENV",
        "NODE_OPTIONS",
        "NODE_PATH",
        "ESBUILD_BINARY_PATH",
        "DYLD_FALLBACK_LIBRARY_PATH",
        "LD_LIBRARY_PATH",
    ],
    tools: &["node", "npm", "typescript", "esbuild"],
    profile_sensitive: false,
    feature_sensitive: false,
    prefix_sensitive: false,
    outputs: &["vink-editor.js"],
    reuse_strategy: "per-artifact-receipt",
};

const STATIC_FONTS_SPEC: WebArtifactSpec = WebArtifactSpec {
    id: WebArtifactId::StaticFonts,
    depends_on: &[],
    input_files: &[],
    input_dirs: &["apps/vink-web/assets/fonts/"],
    relevant_env: &[],
    tools: &[],
    profile_sensitive: false,
    feature_sensitive: false,
    prefix_sensitive: false,
    outputs: &["fonts/**"],
    reuse_strategy: "direct-source-digest",
};

const STATIC_BRAND_SPEC: WebArtifactSpec = WebArtifactSpec {
    id: WebArtifactId::StaticBrand,
    depends_on: &[],
    input_files: &[
        "docs/design/brand/vpen-r1/asset-manifest.json",
        "docs/design/brand/vpen-r1/Vink_Brand_LightDark_VPen_R1_2026-11-08.zip",
    ],
    input_dirs: &["apps/vink-web/assets/brand/"],
    relevant_env: &[],
    tools: &[],
    profile_sensitive: false,
    feature_sensitive: false,
    prefix_sensitive: false,
    outputs: &["brand/**"],
    reuse_strategy: "direct-source-digest + provenance-check",
};

const STATIC_EMPTY_SPEC: WebArtifactSpec = WebArtifactSpec {
    id: WebArtifactId::StaticEmpty,
    depends_on: &[],
    input_files: &[],
    input_dirs: &["apps/vink-web/assets/empty/"],
    relevant_env: &[],
    tools: &["node"],
    profile_sensitive: false,
    feature_sensitive: false,
    prefix_sensitive: false,
    outputs: &["empty/**"],
    reuse_strategy: "direct-source-digest + webp-check",
};

const SPA_INDEX_SPEC: WebArtifactSpec = WebArtifactSpec {
    id: WebArtifactId::SpaIndex,
    depends_on: &[
        WebArtifactId::WasmBindgen,
        WebArtifactId::TailwindCss,
        WebArtifactId::EditorJs,
        WebArtifactId::StaticFonts,
        WebArtifactId::StaticBrand,
        WebArtifactId::StaticEmpty,
    ],
    input_files: &["apps/vink-web/index.html", "apps/vink-web/appearance.js"],
    input_dirs: &[],
    relevant_env: &[
        "VINK_SPA_REVISION",
        "VINK_SPA_SIGN_IN",
        "VINK_SPA_FOLLOWING_FEED",
    ],
    tools: &["git"],
    profile_sensitive: false,
    feature_sensitive: false,
    prefix_sensitive: true,
    outputs: &["index.html"],
    reuse_strategy: "atomic-write-after-pkg-publish",
};

const WEB_DAG: &[WebArtifactSpec] = &[
    TOKENS_SPEC,
    WEB_BOUNDARY_SPEC,
    RUST_WASM_SPEC,
    WASM_BINDGEN_SPEC,
    TAILWIND_CSS_SPEC,
    EDITOR_JS_SPEC,
    STATIC_FONTS_SPEC,
    STATIC_BRAND_SPEC,
    STATIC_EMPTY_SPEC,
];

const SPA_DAG: &[WebArtifactSpec] = &[
    TOKENS_SPEC,
    WEB_BOUNDARY_SPEC,
    RUST_WASM_SPEC,
    WASM_BINDGEN_SPEC,
    TAILWIND_CSS_SPEC,
    EDITOR_JS_SPEC,
    STATIC_FONTS_SPEC,
    STATIC_BRAND_SPEC,
    STATIC_EMPTY_SPEC,
    SPA_INDEX_SPEC,
];

pub fn specs(spa: bool) -> &'static [WebArtifactSpec] {
    if spa { SPA_DAG } else { WEB_DAG }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WebImpact {
    pub direct: BTreeMap<WebArtifactId, BTreeSet<String>>,
    pub closure: BTreeSet<WebArtifactId>,
    pub unmapped_web_paths: BTreeSet<String>,
}

impl WebImpact {
    pub fn is_empty(&self) -> bool {
        self.direct.is_empty() && self.unmapped_web_paths.is_empty()
    }
}

/// Computes direct and transitive Web Build DAG impact for a list of repository-relative paths.
pub fn affected_artifacts(paths: &[&str], spa: bool) -> WebImpact {
    let dag = specs(spa);
    let mut impact = WebImpact::default();
    for &raw in paths {
        let path = raw.replace('\\', "/");
        let mut matched = false;
        for spec in dag {
            let file_hit = spec.input_files.iter().any(|f| *f == path);
            let dir_hit = spec.input_dirs.iter().any(|d| path.starts_with(d));
            if file_hit || dir_hit {
                impact
                    .direct
                    .entry(spec.id)
                    .or_default()
                    .insert(path.clone());
                matched = true;
            }
        }
        if !matched && is_unmapped_web_path(&path) {
            impact.unmapped_web_paths.insert(path);
        }
    }
    impact.closure = impact.direct.keys().copied().collect();
    loop {
        let mut changed = false;
        for spec in dag {
            if !impact.closure.contains(&spec.id)
                && spec
                    .depends_on
                    .iter()
                    .any(|dep| impact.closure.contains(dep))
            {
                impact.closure.insert(spec.id);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    impact
}

fn is_unmapped_web_path(path: &str) -> bool {
    let in_web_owner = path.starts_with("apps/vink-web/")
        || path.starts_with("packages/ui-system/leptos/tailwind/");
    if !in_web_owner {
        return false;
    }
    !path.starts_with("apps/vink-web/e2e/")
        && !path.starts_with("apps/vink-web/behavior/")
        && path != "apps/vink-web/dev-server.mjs"
        && path != "apps/vink-web/README.md"
        && path != "apps/vink-web/editor/README.md"
}

/// Pure formatting of the Web Build DAG for `cargo xtask ... --plan`.
pub fn describe_dag(spa: bool, profile: &str, feature: &str) -> String {
    let mut out = format!("web build DAG (profile={profile}, feature={feature}):\n");
    for spec in specs(spa) {
        let deps: Vec<_> = spec.depends_on.iter().map(|d| d.name()).collect();
        let deps_display = if deps.is_empty() {
            "none".to_string()
        } else {
            deps.join(",")
        };
        let mut sensitivity = Vec::new();
        if spec.profile_sensitive {
            sensitivity.push("profile");
        }
        if spec.feature_sensitive {
            sensitivity.push("feature");
        }
        if spec.prefix_sensitive {
            sensitivity.push("prefix");
        }
        let sens_display = if sensitivity.is_empty() {
            "none".to_string()
        } else {
            sensitivity.join(",")
        };
        writeln!(
            out,
            "  - {}: deps=[{}] files={:?} roots={:?} outputs={:?} reuse={} sensitivity=[{}] tools={:?} env={:?}",
            spec.id.name(),
            deps_display,
            spec.input_files,
            spec.input_dirs,
            spec.outputs,
            spec.reuse_strategy,
            sens_display,
            spec.tools,
            spec.relevant_env
        )
        .unwrap();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dag_is_acyclic_unique_and_matches_owner_constants() {
        for spa in [false, true] {
            let dag = specs(spa);
            let mut seen = BTreeSet::new();
            for spec in dag {
                for dep in spec.depends_on {
                    assert!(
                        seen.contains(dep),
                        "dependency {:?} must precede {:?}",
                        dep,
                        spec.id
                    );
                }
                assert!(seen.insert(spec.id), "duplicate node {:?}", spec.id);
            }
        }
        let tw_dirs: BTreeSet<_> = TAILWIND_CSS_SPEC
            .input_dirs
            .iter()
            .map(|d| d.trim_end_matches('/'))
            .collect();
        let expected_tw: BTreeSet<_> = crate::tailwind::WEB_SOURCE_ROOTS.iter().copied().collect();
        assert_eq!(tw_dirs, expected_tw);
        assert_eq!(
            STATIC_FONTS_SPEC.input_dirs,
            &[&format!("{}/", crate::checks::FONTS_DIR)]
        );
        assert_eq!(
            STATIC_BRAND_SPEC.input_dirs,
            &[&format!("{}/", crate::checks::BRAND_DIR)]
        );
        for token_file in [
            crate::ui_tokens::TOKENS,
            crate::ui_tokens::CATALOG,
            crate::ui_tokens::FIXTURES,
            crate::ui_tokens::ICONS,
            crate::ui_tokens::IDENTITY,
            crate::ui_tokens::CSS_OUT,
            crate::ui_tokens::TAILWIND_THEME_OUT,
            crate::ui_tokens::RUST_TOKENS_OUT,
            crate::ui_tokens::RUST_CATALOG_OUT,
            crate::ui_tokens::RUST_ICONS_OUT,
            crate::ui_tokens::RUST_IDENTITY_OUT,
            crate::ui_states::STATES,
            crate::ui_states::RUST_STATES_OUT,
            crate::ui_navigation::NAVIGATION,
            crate::ui_navigation::RUST_NAVIGATION_OUT,
            crate::ui_toggle::TOGGLE,
            crate::ui_toggle::RUST_TOGGLE_OUT,
        ] {
            assert!(
                TOKENS_SPEC.input_files.contains(&token_file),
                "missing token contract file {token_file}"
            );
        }
    }

    #[test]
    fn affected_closures_isolate_css_js_rust_ui_and_non_ui_changes() {
        let css = affected_artifacts(&["apps/vink-web/style/vink.css"], false);
        assert_eq!(css.closure, BTreeSet::from([WebArtifactId::TailwindCss]));

        let editor = affected_artifacts(&["apps/vink-web/editor/src/editor.ts"], false);
        assert_eq!(editor.closure, BTreeSet::from([WebArtifactId::EditorJs]));

        let corpus = affected_artifacts(
            &["services/vink-app/tests/fixtures/title-authority/markdown.json"],
            false,
        );
        assert_eq!(corpus.closure, BTreeSet::from([WebArtifactId::EditorJs]));

        let bridge = affected_artifacts(&["apps/vink-web/editor/bridge.js"], false);
        assert_eq!(
            bridge.closure,
            BTreeSet::from([WebArtifactId::RustWasm, WebArtifactId::WasmBindgen])
        );

        let rust_ui = affected_artifacts(&["apps/vink-web/src/pages/desk.rs"], false);
        assert_eq!(
            rust_ui.closure,
            BTreeSet::from([
                WebArtifactId::RustWasm,
                WebArtifactId::WasmBindgen,
                WebArtifactId::TailwindCss,
            ])
        );

        let non_ui_rust = affected_artifacts(&["packages/markdown-text/src/lib.rs"], false);
        assert_eq!(
            non_ui_rust.closure,
            BTreeSet::from([WebArtifactId::RustWasm, WebArtifactId::WasmBindgen])
        );

        let tokens = affected_artifacts(&["packages/ui-system/shared/tokens.json"], false);
        assert_eq!(
            tokens.closure,
            BTreeSet::from([
                WebArtifactId::Tokens,
                WebArtifactId::RustWasm,
                WebArtifactId::WasmBindgen,
                WebArtifactId::TailwindCss,
            ])
        );

        let spa_html = affected_artifacts(&["apps/vink-web/index.html"], true);
        assert_eq!(spa_html.closure, BTreeSet::from([WebArtifactId::SpaIndex]));

        let lock = affected_artifacts(&["Cargo.lock"], false);
        assert_eq!(
            lock.closure,
            BTreeSet::from([
                WebArtifactId::WebBoundary,
                WebArtifactId::RustWasm,
                WebArtifactId::WasmBindgen,
            ])
        );
    }

    #[test]
    fn unmapped_web_paths_are_surfaced_while_e2e_and_docs_are_ignored() {
        let e2e = affected_artifacts(
            &[
                "apps/vink-web/e2e/browser-smoke.mjs",
                "apps/vink-web/README.md",
            ],
            false,
        );
        assert!(e2e.is_empty());

        let unknown = affected_artifacts(&["apps/vink-web/style/new-theme.css"], false);
        assert_eq!(
            unknown.unmapped_web_paths,
            BTreeSet::from(["apps/vink-web/style/new-theme.css".to_string()])
        );
    }
}
