//! Admission checks for the migrated UI owners, run by the existing `verify quick` lane.
//! Public component discovery is deliberately bounded to the repository's declaration syntax;
//! registration is independent of the catalog so adding an API cannot silently bypass preview.
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde_json::Value;

const COVERAGE: &str = "packages/ui-system/shared/api-coverage.json";

fn files(dir: &Path, extension: &str) -> Vec<PathBuf> {
    let mut result = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                result.extend(files(&path, extension));
            } else if path.extension().is_some_and(|e| e == extension) {
                result.push(path);
            }
        }
    }
    result.sort();
    result
}

fn components(text: &str, renderer: &str) -> BTreeSet<String> {
    let marker = if renderer == "leptos" {
        "#[component"
    } else {
        "@Composable"
    };
    text.split(marker)
        .skip(1)
        .filter_map(|part| {
            let declaration = part.split('{').next()?;
            let prefix = if renderer == "leptos" {
                "pub fn "
            } else {
                "fun "
            };
            declaration.lines().find_map(|line| {
                let name = line.strip_prefix(prefix)?.split('(').next()?;
                (name.chars().next()?.is_uppercase() && !name.contains('.'))
                    .then(|| name.to_string())
            })
        })
        .collect()
}

pub fn check(root: &Path, problems: &mut Vec<String>) {
    let read = |path: &str| std::fs::read_to_string(root.join(path)).unwrap_or_default();
    let coverage: Value = serde_json::from_str(&read(COVERAGE)).unwrap_or(Value::Null);
    if coverage["format"] != 1 || coverage["components"].as_array().is_none_or(Vec::is_empty) {
        problems.push(format!("{COVERAGE}: missing or invalid API admission map"));
    }
    let catalog: Value =
        serde_json::from_str(&read(crate::ui_tokens::CATALOG)).unwrap_or(Value::Null);
    let entries: BTreeSet<&str> = catalog["entries"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|e| e["id"].as_str())
        .collect();
    let mut declared = BTreeSet::new();
    for (renderer, dirs, extension) in [
        (
            "leptos",
            vec![
                "packages/ui-system/leptos/src",
                "packages/vink-patterns/leptos/src",
            ],
            "rs",
        ),
        (
            "cmp",
            vec![
                "packages/ui-system/cmp/ui/src/commonMain",
                "packages/vink-patterns/cmp/src/commonMain",
            ],
            "kt",
        ),
    ] {
        for file in dirs
            .iter()
            .flat_map(|dir| files(&root.join(dir), extension))
        {
            let path = file
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .to_string();
            let text = std::fs::read_to_string(&file).unwrap_or_default();
            for name in components(&text, renderer) {
                declared.insert((renderer.to_string(), path.clone(), name));
            }
            // Production sources cannot depend on diagnostic fixtures, effect adapters or hosts.
            for forbidden in [
                "vink.ui.preview",
                "VinkFixture",
                "VinkIdentityVectors",
                "::catalog::fixture",
                "vink_app::",
                "vink_web::",
                "import retrofit",
                "import io.ktor",
                "import java.sql",
            ] {
                if !file
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .contains("Vectors")
                    && !path.ends_with("catalog.rs")
                    && !path.ends_with("identity_vectors.rs")
                    && !path.ends_with("VinkCatalog.kt")
                    && text.lines().any(|l| {
                        let l = l.trim();
                        (l.starts_with("use ") || l.starts_with("import ")) && l.contains(forbidden)
                    })
                {
                    problems.push(format!("{path}: production UI imports `{forbidden}`"));
                }
            }
        }
    }
    let mut admitted = BTreeSet::new();
    for component in coverage["components"].as_array().into_iter().flatten() {
        let get = |k: &str| component[k].as_str().unwrap_or_default().to_string();
        let key = (get("renderer"), get("source"), get("symbol"));
        if !admitted.insert(key.clone())
            || !declared.contains(&key)
            || !entries.contains(get("entry").as_str())
        {
            problems.push(format!(
                "{COVERAGE}: duplicate/stale API or missing catalog entry: {key:?}"
            ));
        }
    }
    for key in declared.difference(&admitted) {
        problems.push(format!(
            "{COVERAGE}: public component lacks scenario mapping: {key:?}"
        ));
    }
    for manifest in [
        "apps/vink-web/Cargo.toml",
        "packages/vink-patterns/leptos/Cargo.toml",
        "packages/ui-system/leptos/Cargo.toml",
        "packages/ui-system/cmp/ui/build.gradle.kts",
        "packages/vink-patterns/cmp/build.gradle.kts",
    ] {
        for forbidden in [
            "vink-ui-preview",
            "project(\":preview\")",
            "project(\":diagnostics\")",
            "vink-app",
            "vink-web",
            "sqlx",
        ] {
            if read(manifest).lines().any(|l| {
                !l.trim_start().starts_with('#')
                    && (l.trim_start().starts_with(forbidden)
                        || forbidden.starts_with("project(") && l.contains(forbidden))
            }) {
                problems.push(format!("{manifest}: production dependency on {forbidden}"));
            }
        }
    }
    for file in files(&root.join("apps/vink-web/src"), "rs") {
        let text = std::fs::read_to_string(&file).unwrap_or_default();
        for (i, line) in text.lines().enumerate() {
            if ["<button", "<input", "<select", "<textarea"]
                .iter()
                .any(|tag| line.contains(tag))
            {
                problems.push(format!(
                    "{}:{}: migrated app must use shared controls",
                    file.display(),
                    i + 1
                ));
            }
        }
    }
    let exceptions: Vec<(&str, &str)> = coverage["style_exceptions"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|e| {
            let path = e["path"].as_str()?;
            let declaration = e["declaration"].as_str()?;
            if e["owner"].as_str().is_none_or(str::is_empty)
                || e["reason"].as_str().is_none_or(str::is_empty)
                || !read(path).contains(declaration)
            {
                problems.push(format!(
                    "{COVERAGE}: stale or unexplained style exception: {path}: {declaration}"
                ));
            }
            Some((path, declaration))
        })
        .collect();
    for path in [
        "apps/vink-web/style/vink.css",
        "packages/ui-system/leptos/style/vink-ui.css",
        "packages/vink-patterns/leptos/patterns.css",
        "apps/vink-preview/leptos/style/preview.css",
        "packages/ui-system/leptos/tailwind/web.css",
        "packages/ui-system/leptos/tailwind/preview.css",
    ] {
        for chunk in read(path).split(';') {
            let declaration = chunk.rsplit(['{', '}']).next().unwrap_or("").trim();
            if style_literal(declaration) && !exceptions.contains(&(path, declaration)) {
                problems.push(format!(
                    "{path}: authored UI value outside tokens: {declaration}"
                ));
            }
        }
    }
    // Authored Web/Preview Rust sources: block Tailwind default palette/type/shadow/motion and
    // arbitrary color/type/motion literals outside `--vk-*` tokens (CHOICE-04 B).
    for dir in [
        "apps/vink-web/src",
        "packages/ui-system/leptos/src",
        "packages/vink-patterns/leptos/src",
        "apps/vink-preview/leptos/src",
        "apps/vink-preview/leptos/examples",
    ] {
        for file in files(&root.join(dir), "rs") {
            let rel = file
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            let text = std::fs::read_to_string(&file).unwrap_or_default();
            if text.starts_with("//! GENERATED") || text.starts_with("// GENERATED") {
                continue;
            }
            for (line_no, token) in rust_utility_candidates(&text) {
                if tailwind_literal_utility(&token)
                    && !exceptions.contains(&(rel.as_str(), token.as_str()))
                {
                    problems.push(format!(
                        "{rel}:{line_no}: authored Tailwind utility outside Vink tokens: `{token}`"
                    ));
                }
            }
        }
    }
    // Native literal colour/type/motion values; geometry and colour arithmetic have other owners.
    for dir in [
        "packages/ui-system/cmp/ui/src/commonMain",
        "packages/vink-patterns/cmp/src/commonMain",
    ] {
        for file in files(&root.join(dir), "kt") {
            if file.file_name().is_some_and(|n| n == "VinkTokens.kt") {
                continue;
            }
            for (i, line) in std::fs::read_to_string(&file)
                .unwrap_or_default()
                .lines()
                .enumerate()
            {
                if line.contains("Color(0x")
                    || [
                        "FontWeight.SemiBold",
                        "FontWeight.Bold",
                        "FontWeight.Normal",
                        "FontWeight.Medium",
                    ]
                    .iter()
                    .any(|weight| line.contains(weight))
                    || ["tween(", "delay(", "durationMillis = "]
                        .iter()
                        .any(|marker| {
                            line.split(marker).skip(1).any(|after| {
                                after.trim_start().starts_with(|c: char| c.is_ascii_digit())
                            })
                        })
                    || line
                        .split(".sp")
                        .take(line.matches(".sp").count())
                        .any(|prefix| prefix.chars().last().is_some_and(|c| c.is_ascii_digit()))
                {
                    problems.push(format!(
                        "{}:{}: native colour/type literal outside tokens",
                        file.display(),
                        i + 1
                    ));
                }
            }
        }
    }
}

fn style_literal(declaration: &str) -> bool {
    let Some((property, value)) = declaration.split_once(':') else {
        return false;
    };
    let property = property.trim().to_ascii_lowercase();
    let property = property.as_str();
    let value = value.trim().trim_end_matches("!important").trim();
    if matches!(
        property,
        "color"
            | "background-color"
            | "border-color"
            | "outline-color"
            | "caret-color"
            | "fill"
            | "stroke"
            | "text-decoration-color"
    ) && is_css_named_color(value)
        || shorthand_has_named_color(property, value)
    {
        return true;
    }
    if matches!(
        property,
        "transition" | "animation" | "transition-delay" | "animation-delay"
    ) && value
        .split(|ch: char| ch.is_ascii_whitespace() || ch == ',')
        .any(nonzero_css_time)
    {
        return true;
    }
    if property == "font"
        && value
            .split_whitespace()
            .any(|part| part.starts_with(|c: char| c.is_ascii_digit()))
    {
        return true;
    }
    if property.starts_with("font-") && property != "font-style" {
        return !value.contains("var(") && !["inherit", "normal", "bold"].contains(&value);
    }
    value.contains("rgb(")
        || value.contains("rgba(")
        || value.contains("hsl(")
        || value.contains("hsla(")
        || value.contains("oklch(")
        || value.contains("oklab(")
        || value
            .split('#')
            .skip(1)
            .any(|hex| hex.chars().take_while(char::is_ascii_hexdigit).count() >= 3)
        || (property.ends_with("duration")
            && !value.contains("var(")
            && value != "0s"
            && value != "0ms")
}

fn shorthand_has_named_color(property: &str, value: &str) -> bool {
    let structural: &[&str] = match property {
        "background" => &[
            "bottom",
            "center",
            "left",
            "right",
            "top",
            "contain",
            "cover",
            "repeat",
            "repeat-x",
            "repeat-y",
            "no-repeat",
            "round",
            "space",
            "fixed",
            "local",
            "scroll",
            "border-box",
            "border-area",
            "content-box",
            "padding-box",
            "text",
        ],
        "border"
        | "border-top"
        | "border-right"
        | "border-bottom"
        | "border-left"
        | "border-block"
        | "border-block-start"
        | "border-block-end"
        | "border-inline"
        | "border-inline-start"
        | "border-inline-end" => &[
            "thin", "medium", "thick", "hidden", "dotted", "dashed", "solid", "double", "groove",
            "ridge", "inset", "outset",
        ],
        _ => return false,
    };
    let is_literal = |part: &str| {
        is_css_named_color(part)
            && !structural
                .iter()
                .any(|token| part.eq_ignore_ascii_case(token))
    };

    // Only inspect top-level shorthand components. Functions, quoted URLs and comments
    // can contain colour-like words that are not the declaration's named colour.
    let mut chars = value.char_indices().peekable();
    let mut start = 0;
    let mut depth = 0usize;
    let mut quote = None;
    while let Some((end, ch)) = chars.next() {
        match ch {
            '\\' => {
                chars.next();
            }
            _ if quote == Some(ch) => quote = None,
            _ if quote.is_some() => {}
            '"' | '\'' => quote = Some(ch),
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            '/' if chars.next_if(|&(_, next)| next == '*').is_some() => {
                if depth == 0 && is_literal(&value[start..end]) {
                    return true;
                }
                while let Some((_, part)) = chars.next() {
                    if part == '*'
                        && let Some((close, _)) = chars.next_if(|&(_, next)| next == '/')
                    {
                        if depth == 0 {
                            start = close + 1;
                        }
                        break;
                    }
                }
            }
            ch if depth == 0 && (ch.is_ascii_whitespace() || matches!(ch, ',' | '/')) => {
                if is_literal(&value[start..end]) {
                    return true;
                }
                start = end + ch.len_utf8();
            }
            _ => {}
        }
    }
    is_literal(&value[start..])
}

fn nonzero_css_time(token: &str) -> bool {
    token
        .strip_suffix("ms")
        .or_else(|| token.strip_suffix('s'))
        .and_then(|time| time.parse::<f64>().ok())
        .is_some_and(|time| time != 0.0)
}

// CSS named colours are bare identifiers. Admit inherited/transparent and platform colour roles.
fn is_css_named_color(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|b| b.is_ascii_alphabetic() || b == b'-')
        && !matches!(
            value.to_ascii_lowercase().as_str(),
            "auto"
                | "none"
                | "currentcolor"
                | "transparent"
                | "inherit"
                | "initial"
                | "unset"
                | "revert"
                | "revert-layer"
                | "accentcolor"
                | "accentcolortext"
                | "activetext"
                | "buttonborder"
                | "buttonface"
                | "buttontext"
                | "canvas"
                | "canvastext"
                | "field"
                | "fieldtext"
                | "graytext"
                | "highlight"
                | "highlighttext"
                | "linktext"
                | "mark"
                | "marktext"
                | "selecteditem"
                | "selecteditemtext"
                | "visitedtext"
        )
}

const TEST_ONLY: &str = "#[cfg(test)]";

fn test_module_body(source: &str, start: usize) -> Option<usize> {
    let after = source.get(start..)?.strip_prefix(TEST_ONLY)?.trim_start();
    let header = after
        .strip_prefix("pub ")
        .unwrap_or(after)
        .strip_prefix("mod ")?;
    let (name, body) = header.split_once('{')?;
    let name = name.trim();
    if name.is_empty() || !name.chars().all(|ch| ch.is_alphanumeric() || ch == '_') {
        return None;
    }
    Some(source.len() - body.len())
}

fn char_literal_end(source: &str, start: usize) -> Option<usize> {
    let body = source.get(start + 1..)?;
    let value_len = if let Some(escaped) = body.strip_prefix('\\') {
        if escaped.starts_with("u{") {
            escaped.find('}')? + 2
        } else if escaped.starts_with('x') {
            4
        } else {
            2
        }
    } else {
        body.chars().next()?.len_utf8()
    };
    (body.as_bytes().get(value_len) == Some(&b'\'')).then_some(start + value_len + 2)
}

/// Extracts candidate utility tokens from production Rust UI source (string literals and Leptos
/// `class:<token>=` directives), skipping comments and `#[cfg(test)]` module bodies.
fn rust_utility_candidates(source: &str) -> Vec<(usize, String)> {
    let bytes = source.as_bytes();
    let len = bytes.len();
    let mut i = 0usize;
    let mut line = 1usize;
    let mut out = Vec::new();
    let mut test_depth = 0;

    while i < len {
        if test_depth == 0
            && bytes[i..].starts_with(TEST_ONLY.as_bytes())
            && let Some(body) = test_module_body(source, i)
        {
            line += bytes[i..body].iter().filter(|&&byte| byte == b'\n').count();
            i = body;
            test_depth = 1;
            continue;
        }
        if bytes[i] == b'\''
            && let Some(end) = char_literal_end(source, i)
        {
            i = end;
            continue;
        }

        // Line comment `//...`
        if bytes[i] == b'/' && i + 1 < len && bytes[i + 1] == b'/' {
            while i < len && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        // Block comment `/* ... */` (supports nesting in Rust)
        if bytes[i] == b'/' && i + 1 < len && bytes[i + 1] == b'*' {
            let mut depth = 1usize;
            i += 2;
            while i < len && depth > 0 {
                if bytes[i] == b'\n' {
                    line += 1;
                    i += 1;
                } else if bytes[i] == b'/' && i + 1 < len && bytes[i + 1] == b'*' {
                    depth += 1;
                    i += 2;
                } else if bytes[i] == b'*' && i + 1 < len && bytes[i + 1] == b'/' {
                    depth -= 1;
                    i += 2;
                } else {
                    i += 1;
                }
            }
            continue;
        }
        // Raw string literal `r"..."` or `r#"..."#`
        if bytes[i] == b'r'
            && (i == 0 || !(bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_'))
        {
            let mut j = i + 1;
            let mut hashes = 0usize;
            while j < len && bytes[j] == b'#' {
                hashes += 1;
                j += 1;
            }
            if j < len && bytes[j] == b'"' {
                let start_line = line;
                j += 1;
                let content_start = j;
                while j < len {
                    if bytes[j] == b'\n' {
                        line += 1;
                    }
                    if bytes[j] == b'"'
                        && j + hashes < len
                        && bytes[j + 1..=j + hashes].iter().all(|&b| b == b'#')
                    {
                        if test_depth == 0 {
                            push_tokens_from_literal(
                                &source[content_start..j],
                                start_line,
                                &mut out,
                            );
                        }
                        j += 1 + hashes;
                        break;
                    }
                    j += 1;
                }
                i = j;
                continue;
            }
        }
        // Normal string literal `"..."`
        if bytes[i] == b'"' {
            let start_line = line;
            i += 1;
            let content_start = i;
            while i < len {
                if bytes[i] == b'\\' && i + 1 < len {
                    if bytes[i + 1] == b'\n' {
                        line += 1;
                    }
                    i += 2;
                    continue;
                }
                if bytes[i] == b'\n' {
                    line += 1;
                }
                if bytes[i] == b'"' {
                    if test_depth == 0 {
                        push_tokens_from_literal(&source[content_start..i], start_line, &mut out);
                    }
                    i += 1;
                    break;
                }
                i += 1;
            }
            continue;
        }
        // Leptos `class:<token>=` attribute syntax outside strings.
        if test_depth == 0
            && bytes[i..].starts_with(b"class:")
            && (i == 0 || !(bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_'))
        {
            let start = i + "class:".len();
            let mut j = start;
            while j < len
                && !bytes[j].is_ascii_whitespace()
                && !matches!(bytes[j], b'=' | b'>' | b'/' | b')' | b'"')
            {
                j += 1;
            }
            if j > start {
                out.push((line, source[start..j].to_string()));
            }
            i = j;
            continue;
        }
        match bytes[i] {
            b'\n' => line += 1,
            b'{' if test_depth > 0 => test_depth += 1,
            b'}' if test_depth > 0 => test_depth -= 1,
            _ => {}
        }
        i += 1;
    }
    out
}

fn push_tokens_from_literal(literal: &str, start_line: usize, out: &mut Vec<(usize, String)>) {
    for (rel_line, line_text) in literal.lines().enumerate() {
        for raw in line_text.split_whitespace() {
            let token = raw.trim_matches(|c| matches!(c, ',' | ';'));
            if !token.is_empty() {
                out.push((start_line + rel_line, token.to_string()));
            }
        }
    }
}

/// Returns `true` when `token` is a Tailwind utility that hardcodes colour, typography, shadow or
/// motion values outside Vink's `--vk-*` semantic tokens (CHOICE-04 B).
fn tailwind_literal_utility(token: &str) -> bool {
    let base = strip_tailwind_variants(token);
    let base = base.strip_prefix('!').unwrap_or(base);
    let base = base.strip_suffix('!').unwrap_or(base);
    if base.is_empty() || base.contains("://") {
        return false;
    }

    // Arbitrary CSS property syntax: `[color:#fff]`, `[font-size:14px]`, `[transition-duration:123ms]`
    if let Some(inner) = base.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
        if inner.contains(':') {
            let normalized = inner.replace('_', " ");
            return style_literal(&normalized);
        }
        return false;
    }

    // Colour utilities across Tailwind v4 namespaces.
    const COLOR_PREFIXES: &[&str] = &[
        "bg-",
        "text-",
        "border-",
        "border-t-",
        "border-r-",
        "border-b-",
        "border-l-",
        "border-x-",
        "border-y-",
        "border-s-",
        "border-e-",
        "outline-",
        "ring-",
        "ring-offset-",
        "fill-",
        "stroke-",
        "decoration-",
        "accent-",
        "caret-",
        "shadow-",
        "from-",
        "via-",
        "to-",
        "divide-",
        "placeholder-",
    ];
    for prefix in COLOR_PREFIXES {
        if let Some(rest) = base.strip_prefix(prefix) {
            let val = strip_opacity_modifier(rest);
            if is_default_tailwind_color(val) || is_arbitrary_color_literal(val) {
                return true;
            }
        }
    }

    // Typography: default Tailwind sizes (`text-xs`..`text-9xl`) and arbitrary font sizes (`text-[14px]`).
    if let Some(rest) = base.strip_prefix("text-") {
        let val = strip_opacity_modifier(rest);
        if matches!(
            val,
            "xs" | "sm"
                | "base"
                | "lg"
                | "xl"
                | "2xl"
                | "3xl"
                | "4xl"
                | "5xl"
                | "6xl"
                | "7xl"
                | "8xl"
                | "9xl"
        ) {
            return true;
        }
        if let Some(arb) = val.strip_prefix('[').and_then(|s| s.strip_suffix(']'))
            && !arb.contains("var(--vk-")
            && (arb.starts_with(|c: char| c.is_ascii_digit() || c == '.')
                || arb.starts_with("length:")
                || arb.starts_with("calc(")
                || arb.starts_with("clamp(")
                || arb.starts_with("min(")
                || arb.starts_with("max("))
        {
            return true;
        }
    }

    // Typography: default Tailwind font families/weights and arbitrary `font-[...]`.
    if let Some(rest) = base.strip_prefix("font-") {
        if matches!(
            rest,
            "sans"
                | "serif"
                | "mono"
                | "thin"
                | "extralight"
                | "light"
                | "medium"
                | "semibold"
                | "extrabold"
                | "black"
        ) {
            return true;
        }
        if let Some(arb) = rest.strip_prefix('[').and_then(|s| s.strip_suffix(']'))
            && !arb.contains("var(--vk-")
        {
            return true;
        }
    }

    // Motion: literal `duration-150`, `duration-[123ms]`, `delay-*`, default `ease-in*`/`ease-out`/`ease-[...]`.
    if let Some(rest) = base.strip_prefix("duration-")
        && rest != "0"
        && !rest.starts_with("vk-")
        && !rest.starts_with("(--vk-")
        && (rest.chars().all(|c| c.is_ascii_digit())
            || rest
                .strip_prefix('[')
                .and_then(|s| s.strip_suffix(']'))
                .is_some_and(|arb| !arb.contains("var(--vk-")))
    {
        return true;
    }
    if let Some(rest) = base.strip_prefix("delay-")
        && rest != "0"
        && !rest.starts_with("vk-")
        && !rest.starts_with("(--vk-")
        && (rest.chars().all(|c| c.is_ascii_digit())
            || rest
                .strip_prefix('[')
                .and_then(|s| s.strip_suffix(']'))
                .is_some_and(|arb| !arb.contains("var(--vk-")))
    {
        return true;
    }
    if let Some(rest) = base.strip_prefix("ease-")
        && (matches!(rest, "in" | "out" | "in-out")
            || rest
                .strip_prefix('[')
                .and_then(|s| s.strip_suffix(']'))
                .is_some_and(|arb| !arb.contains("var(--vk-")))
    {
        return true;
    }

    // Shadows: default Tailwind shadow scale and arbitrary shadows outside `--vk-*`.
    if let Some(rest) = base.strip_prefix("shadow-")
        && (matches!(
            rest,
            "2xs" | "xs" | "sm" | "md" | "lg" | "xl" | "2xl" | "inner"
        ) || rest
            .strip_prefix('[')
            .and_then(|s| s.strip_suffix(']'))
            .is_some_and(|arb| !arb.contains("var(--vk-")))
    {
        return true;
    }

    false
}

/// Strips variant prefixes (`sm:`, `hover:`, `[&>.vk-ui-notice]:`) while respecting `[...]` groups.
fn strip_tailwind_variants(token: &str) -> &str {
    let mut bracket_depth = 0usize;
    let mut paren_depth = 0usize;
    let mut last_colon = None;
    for (idx, ch) in token.char_indices() {
        match ch {
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            '(' => paren_depth += 1,
            ')' => paren_depth = paren_depth.saturating_sub(1),
            ':' if bracket_depth == 0 && paren_depth == 0 => last_colon = Some(idx),
            _ => {}
        }
    }
    match last_colon {
        Some(idx) => &token[idx + 1..],
        None => token,
    }
}

fn strip_opacity_modifier(value: &str) -> &str {
    let mut bracket_depth = 0usize;
    let mut paren_depth = 0usize;
    for (idx, ch) in value.char_indices() {
        match ch {
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            '(' => paren_depth += 1,
            ')' => paren_depth = paren_depth.saturating_sub(1),
            '/' if bracket_depth == 0 && paren_depth == 0 => return &value[..idx],
            _ => {}
        }
    }
    value
}

fn is_default_tailwind_color(val: &str) -> bool {
    if matches!(val, "white" | "black") {
        return true;
    }
    let Some((family, shade)) = val.rsplit_once('-') else {
        return false;
    };
    const FAMILIES: &[&str] = &[
        "slate", "gray", "zinc", "neutral", "stone", "red", "orange", "amber", "yellow", "lime",
        "green", "emerald", "teal", "cyan", "sky", "blue", "indigo", "violet", "purple", "fuchsia",
        "pink", "rose",
    ];
    FAMILIES.contains(&family) && !shade.is_empty() && shade.chars().all(|c| c.is_ascii_digit())
}

fn is_arbitrary_color_literal(val: &str) -> bool {
    let Some(arb) = val.strip_prefix('[').and_then(|s| s.strip_suffix(']')) else {
        return false;
    };
    if arb.contains("var(--vk-") {
        return false;
    }
    let lower = arb.to_ascii_lowercase();
    lower.starts_with('#')
        || lower.starts_with("rgb(")
        || lower.starts_with("rgba(")
        || lower.starts_with("hsl(")
        || lower.starts_with("hsla(")
        || lower.starts_with("oklch(")
        || lower.starts_with("oklab(")
        || lower.starts_with("lab(")
        || lower.starts_with("lch(")
        || lower.starts_with("color:")
        || lower.starts_with("color-mix(")
        || is_css_named_color(arb)
            && !matches!(
                lower.as_str(),
                "auto"
                    | "center"
                    | "cover"
                    | "contain"
                    | "left"
                    | "right"
                    | "top"
                    | "bottom"
                    | "inset"
                    | "min-content"
                    | "max-content"
                    | "fit-content"
            )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn discovers_unregistered_public_components_independently_of_catalog() {
        assert_eq!(
            components(
                "#[component]\npub fn NewControl(x: String) -> impl IntoView {}\n#[component]\nfn Private() {}",
                "leptos"
            ),
            BTreeSet::from(["NewControl".into()])
        );
        assert_eq!(
            components(
                "@Composable\nfun NewNative() {}\n@Composable\nprivate fun Hidden() {}",
                "cmp"
            ),
            BTreeSet::from(["NewNative".into()])
        );
    }
    #[test]
    fn rejects_authored_colour_type_and_duration_but_allows_layout() {
        for s in [
            "color: #ffeeff",
            "background: rgb(0 0 0 / .5)",
            "font-size: 14px",
            "transition-duration: 123ms",
            "transition: opacity 150ms",
            "color: red",
            "background: red url(cover.svg) center / cover no-repeat",
            "background: url(layer.svg) no-repeat, rebeccapurple",
            "border: RED solid 1px",
            "border-inline-start: 1px solid/* side */red",
        ] {
            assert!(style_literal(s), "{s}");
        }
        for s in [
            "font-size: var(--vk-type-body-size)",
            "width: 320px",
            "background: var(--vk-surface)",
            "background: /* red is only a comment */ var(--vk-surface)",
            "background: color-mix(in srgb, var(--vk-primary) 25%, transparent)",
            "background: center / cover url(cover.svg) no-repeat",
            "border-block: Thin Dashed ButtonText",
            "color: CanvasText",
            "transition: opacity var(--vk-duration-short) var(--vk-ease-emphasized)",
        ] {
            assert!(!style_literal(s), "{s}");
        }
    }

    #[test]
    fn admission_fails_on_missing_apis_stale_entries_and_preview_dependencies() {
        let root = std::env::temp_dir().join(format!("vink-ui-admission-{}", std::process::id()));
        for dir in [
            "packages/ui-system/shared",
            "packages/ui-system/leptos/src",
            "apps/vink-web/style",
        ] {
            std::fs::create_dir_all(root.join(dir)).unwrap();
        }
        std::fs::write(
            root.join("packages/ui-system/leptos/src/new.rs"),
            r#"#[cfg(test)]
mod fixtures { const CLASSES: &str = "[color:blue]"; }
#[component]
pub fn Unregistered() -> impl IntoView {
    let tiếng = 'đ';
    view! { <p class="[color:red] [background:red] [border:1px_solid_red]
        [background:var(--vk-surface)] [border:1px_solid_CanvasText]
        text-vk-caption w-[320px]" /> }
}
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("apps/vink-web/style/vink.css"),
            r#".card { transition: opacity 150ms; background: red; border: 1px solid red; }
                .token { background: var(--vk-surface); border: 1px solid var(--vk-outline); }
                .system { background: Highlight; border: 1px solid CanvasText; }
                .image { background: url("red (and blue).svg") center / contain no-repeat; }
            "#,
        )
        .unwrap();
        std::fs::write(root.join(COVERAGE), r#"{"format":1,"components":[{"renderer":"leptos","source":"missing.rs","symbol":"Stale","entry":"missing"}]}"#).unwrap();
        std::fs::write(
            root.join("apps/vink-web/Cargo.toml"),
            "[dependencies]\nvink-ui-preview = { path = \"preview\" }\n",
        )
        .unwrap();
        let mut problems = Vec::new();
        check(&root, &mut problems);
        std::fs::remove_dir_all(root).unwrap();
        assert!(problems.iter().any(|p| p.contains("Unregistered")));
        assert!(problems.iter().any(|p| p.contains("Stale")));
        for rejected in [
            "[color:red]",
            "[background:red]",
            "[border:1px_solid_red]",
            "background: red",
            "border: 1px solid red",
        ] {
            assert!(
                problems.iter().any(|problem| problem.contains(rejected)),
                "{rejected} missing from {problems:?}"
            );
        }
        for accepted in [
            "[background:var(--vk-surface)]",
            "[border:1px_solid_CanvasText]",
            "background: var(--vk-surface)",
            "border: 1px solid var(--vk-outline)",
            "background: Highlight",
            "border: 1px solid CanvasText",
            "url(",
        ] {
            assert!(
                problems.iter().all(|problem| !problem.contains(accepted)),
                "{accepted} rejected in {problems:?}"
            );
        }
        assert!(
            problems
                .iter()
                .any(|p| p.contains("transition: opacity 150ms"))
        );
        assert!(!problems.iter().any(|p| p.contains("[color:blue]")
            || p.contains("text-vk-caption")
            || p.contains("w-[320px]")));
        assert!(
            problems
                .iter()
                .any(|p| p.contains("production dependency on vink-ui-preview"))
        );
    }

    #[test]
    fn rejects_tailwind_default_palette_type_shadow_and_motion_but_allows_vink_tokens_and_layout() {
        for forbidden in [
            "bg-red-500",
            "hover:text-slate-600/80",
            "bg-white",
            "text-black",
            "bg-[#fff]",
            "text-[rgb(10,20,30)]",
            "border-[hsl(210_40%_50%)]",
            "bg-[oklch(0.6_0.2_250)]",
            "text-sm",
            "sm:text-lg/6",
            "text-[14px]",
            "text-[1.125rem]",
            "font-sans",
            "font-mono",
            "font-semibold",
            "font-[600]",
            "duration-150",
            "duration-[123ms]",
            "delay-100",
            "ease-in-out",
            "ease-[cubic-bezier(0.2,0,0,1)]",
            "shadow-md",
            "shadow-[0_2px_8px_rgba(0,0,0,0.2)]",
            "[color:#fff]",
            "[color:red]",
            "bg-[red]",
            "[transition:opacity_150ms]",
            "[font-size:14px]",
            "[transition-duration:123ms]",
            "[&>.vk-ui-notice]:bg-red-500",
        ] {
            assert!(
                tailwind_literal_utility(forbidden),
                "expected `{forbidden}` to be rejected"
            );
        }
        for allowed in [
            "bg-vk-surface",
            "bg-vk-error-container",
            "hover:bg-vk-state-hover",
            "text-vk-on-surface-variant",
            "text-vk-label",
            "text-vk-caption",
            "text-vk-size-draft-title",
            "font-vk-ui",
            "font-vk-reading",
            "font-vk-code",
            "font-vk-role-body",
            "font-normal",
            "font-bold",
            "duration-vk-short",
            "duration-vk-medium",
            "duration-0",
            "ease-vk-emphasized",
            "ease-linear",
            "shadow-vk-raised",
            "shadow-none",
            "bg-transparent",
            "text-inherit",
            "text-center",
            "forced-colors:border-[CanvasText]",
            "forced-colors:outline-[Highlight]",
            "w-[320px]",
            "min-h-[48px]",
            "gap-[6px]",
            "bg-[var(--vk-surface)]",
            "text-(--vk-on-surface)",
            "[&>.vk-ui-notice]:p-0",
            "[&>.vk-ui-notice]:bg-transparent",
            "[&_.vk-ui-notice\\_\\_icon]:hidden",
        ] {
            assert!(
                !tailwind_literal_utility(allowed),
                "expected `{allowed}` to be allowed"
            );
        }
    }

    #[test]
    fn rust_utility_guard_flags_literals_in_strings_and_class_directives_while_skipping_tests() {
        let sample = r#"
            fn view_fn(err: bool) {
                let tiếng = 'đ';
                // bg-red-500 inside comment must be ignored
                let _ = "flex bg-vk-surface text-vk-body";
                let _ = "bg-red-500 text-sm duration-150";
                let _ = view! { <div class:bg-blue-500=err class:rounded-none=err /> };
            }
            #[cfg(test)]
            mod tests {
                const IGNORED: &str = "bg-green-500 text-lg }";
                const BRACE: char = '}';
            }
            fn after_tests() {
                let _ = view! { <div class:text-red-500=true /> };
            }
        "#;
        let flagged: Vec<String> = rust_utility_candidates(sample)
            .into_iter()
            .filter(|(_, t)| tailwind_literal_utility(t))
            .map(|(_, t)| t)
            .collect();
        assert_eq!(
            flagged,
            vec![
                "bg-red-500",
                "text-sm",
                "duration-150",
                "bg-blue-500",
                "text-red-500"
            ]
        );
    }
}
