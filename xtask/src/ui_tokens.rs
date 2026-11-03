//! `cargo xtask ui-tokens [--check]` — the Vink UI token source and its generated renderer files.
//!
//! `packages/ui-system/shared/tokens.json` is the only authoring place for colour, type, shape,
//! spacing, density and motion roles; `preview-catalog.json` and `fixtures/foundation.json` own
//! the preview IDs and presentation samples. This module validates all three (types, units,
//! aliases, cycles, theme completeness, contrast) before anything is written, then renders the
//! committed CSS, Rust and Kotlin files. Output is a pure function of the inputs: maps iterate in
//! key order and numbers print without locale or float noise, so regenerating never produces a
//! diff. `--check` writes nothing and fails on a missing or drifted output.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use serde_json::{Map, Value};

pub const TOKENS: &str = "packages/ui-system/shared/tokens.json";
pub const CATALOG: &str = "packages/ui-system/shared/preview-catalog.json";
pub const FIXTURES: &str = "packages/ui-system/shared/fixtures/foundation.json";
pub const ICONS: &str = "packages/ui-system/shared/icons.json";
pub const IDENTITY: &str = "packages/ui-system/shared/fixtures/identity.json";

pub const CSS_OUT: &str = "packages/ui-system/leptos/style/tokens.css";
pub const RUST_TOKENS_OUT: &str = "packages/ui-system/leptos/src/tokens.rs";
pub const RUST_CATALOG_OUT: &str = "packages/ui-system/leptos/src/catalog.rs";
pub const KOTLIN_TOKENS_OUT: &str =
    "packages/ui-system/cmp/ui/src/commonMain/kotlin/vink/ui/VinkTokens.kt";
pub const KOTLIN_CATALOG_OUT: &str =
    "packages/ui-system/cmp/ui/src/commonMain/kotlin/vink/ui/VinkCatalog.kt";
pub const RUST_ICONS_OUT: &str = "packages/ui-system/leptos/src/icons.rs";
pub const RUST_IDENTITY_OUT: &str = "packages/ui-system/leptos/src/identity_vectors.rs";
pub const KOTLIN_ICONS_OUT: &str =
    "packages/ui-system/cmp/ui/src/commonMain/kotlin/vink/ui/VinkIcons.kt";
pub const KOTLIN_IDENTITY_OUT: &str =
    "packages/ui-system/cmp/ui/src/commonMain/kotlin/vink/ui/VinkIdentityVectors.kt";

/// The renderers every catalog entry must declare a status for.
const RENDERERS: [&str; 2] = ["leptos", "cmp"];
/// Web root font size the rem conversion assumes (browser default; user scaling stays relative).
const ROOT_PX: f64 = 16.0;

pub fn run(root: &Path, check: bool) -> Result<(), String> {
    let read = |path: &str| {
        std::fs::read_to_string(root.join(path)).map_err(|e| format!("read {path}: {e}"))
    };
    let mut outputs = generate(&read(TOKENS)?, &read(CATALOG)?, &read(FIXTURES)?)
        .map_err(|problems| invalid(&problems))?;
    let source = parse_tokens(&read(TOKENS)?).map_err(|problems| invalid(&problems))?;
    let license = root.join("packages/ui-system/shared");
    outputs.extend(
        generate_assets(&source.version, &read(ICONS)?, &read(IDENTITY)?, |file| {
            license.join(file).is_file()
        })
        .map_err(|problems| invalid(&problems))?,
    );
    let duration = |id: &str| match source.tokens.get(id).map(|t| &t.def) {
        Some(Def::Duration { ms, .. }) => Ok(u64::from(*ms)),
        _ => Err(invalid(&[format!("tokens: {id} is a duration")])),
    };
    let policy = crate::ui_states::GatePolicy {
        delay: duration("duration.loading-delay")?,
        min_visible: duration("duration.loading-min-visible")?,
        slow: duration("duration.slow-notice")?,
    };
    outputs.extend(
        crate::ui_states::generate(&source.version, &read(crate::ui_states::STATES)?, policy)
            .map_err(|problems| invalid(&problems))?,
    );
    outputs.extend(
        crate::ui_navigation::generate(&source.version, &read(crate::ui_navigation::NAVIGATION)?)
            .map_err(|problems| invalid(&problems))?,
    );
    let fonts_dir = root.join(crate::checks::FONTS_DIR);
    let missing: Vec<String> = source
        .fonts
        .iter()
        .filter(|face| !fonts_dir.join(&face.file).is_file())
        .map(|face| {
            format!(
                "fonts: {} is not in {}",
                face.file,
                crate::checks::FONTS_DIR
            )
        })
        .collect();
    if !missing.is_empty() {
        return Err(invalid(&missing));
    }
    let mut drift = Vec::new();
    for (path, text) in &outputs {
        let current = std::fs::read_to_string(root.join(path)).ok();
        if current.as_deref() == Some(text.as_str()) {
            continue;
        }
        if check {
            drift.push(match current {
                None => format!("{path}: missing"),
                Some(_) => format!("{path}: differs from the token source"),
            });
        } else {
            let target = root.join(path);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("create {}: {e}", parent.display()))?;
            }
            std::fs::write(&target, text).map_err(|e| format!("write {path}: {e}"))?;
            println!("==> wrote {path}");
        }
    }
    if drift.is_empty() {
        if check {
            println!("==> ui tokens: {} generated files match", outputs.len());
        }
        Ok(())
    } else {
        Err(format!(
            "generated UI files are stale; run `cargo xtask ui-tokens`:\n  {}",
            drift.join("\n  ")
        ))
    }
}

fn invalid(problems: &[String]) -> String {
    format!("invalid UI token source:\n  {}", problems.join("\n  "))
}

/// Validates the three sources and renders every output, in a fixed order.
pub fn generate(
    tokens: &str,
    catalog: &str,
    fixtures: &str,
) -> Result<Vec<(&'static str, String)>, Vec<String>> {
    let source = parse_tokens(tokens)?;
    let catalog = parse_catalog(catalog)?;
    let fixtures = parse_fixtures(fixtures)?;
    Ok(vec![
        (CSS_OUT, css(&source)),
        (RUST_TOKENS_OUT, rust_tokens(&source)),
        (RUST_CATALOG_OUT, rust_catalog(&source, &catalog, &fixtures)),
        (KOTLIN_TOKENS_OUT, kotlin_tokens(&source)),
        (
            KOTLIN_CATALOG_OUT,
            kotlin_catalog(&source, &catalog, &fixtures),
        ),
    ])
}

// ---------------------------------------------------------------------------------------------
// Model

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgba {
    r: u8,
    g: u8,
    b: u8,
    a: f64,
}

impl Rgba {
    fn css(self) -> String {
        if self.a >= 1.0 {
            format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
        } else {
            format!("rgb({} {} {} / {})", self.r, self.g, self.b, num(self.a))
        }
    }

    fn argb(self) -> String {
        let alpha = (self.a * 255.0).round() as u8;
        format!("0x{alpha:02X}{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }

    /// `self` drawn over an opaque `below`.
    fn over(self, below: Rgba) -> Rgba {
        let mix = |top: u8, bottom: u8| {
            (f64::from(top) * self.a + f64::from(bottom) * (1.0 - self.a)).round() as u8
        };
        Rgba {
            r: mix(self.r, below.r),
            g: mix(self.g, below.g),
            b: mix(self.b, below.b),
            a: 1.0,
        }
    }

    fn luminance(self) -> f64 {
        let channel = |c: u8| {
            let c = f64::from(c) / 255.0;
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(self.r) + 0.7152 * channel(self.g) + 0.0722 * channel(self.b)
    }
}

/// WCAG 2.x contrast ratio of a foreground (composited over the background) on an opaque one.
pub fn contrast(fg: Rgba, bg: Rgba) -> f64 {
    let fg = fg.over(bg);
    let (a, b) = (fg.luminance(), bg.luminance());
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WebUnit {
    Rem,
    Px,
}

#[derive(Clone, Debug, PartialEq)]
struct Dim {
    value: f64,
    /// Fluid size: `clamp(value, base + vw, max)` on Web; CMP uses `value`.
    fluid: Option<(f64, f64, f64)>,
    compact: Option<f64>,
    web: WebUnit,
}

#[derive(Clone, Debug, PartialEq)]
struct Shadow {
    x: f64,
    y: f64,
    blur: f64,
    spread: f64,
    color: Rgba,
}

#[derive(Clone, Debug, PartialEq)]
enum Size {
    Own(Dim),
    Alias(String),
}

#[derive(Clone, Debug, PartialEq)]
enum LineHeight {
    Ratio(f64),
    Alias(String),
}

#[derive(Clone, Debug, PartialEq)]
struct TypeRole {
    family: String,
    size: Size,
    weight: u16,
    line_height: LineHeight,
    tracking: f64,
}

#[derive(Clone, Debug, PartialEq)]
enum Def {
    /// A themed semantic colour: each side references a primitive.
    Color {
        light: Rgba,
        dark: Rgba,
    },
    /// A recipe alias of another colour token; resolved per theme and per scope.
    ColorAlias(String),
    Shadow {
        light: Shadow,
        dark: Shadow,
    },
    Font {
        stack: Vec<String>,
        native: String,
    },
    Dimension(Dim),
    Number(f64),
    Type(TypeRole),
    Duration {
        ms: u32,
        reduced: Option<u32>,
    },
    Iterations,
    Bezier([f64; 4]),
}

#[derive(Clone, Debug)]
struct Token {
    id: String,
    css: String,
    rationale: Option<String>,
    def: Def,
}

#[derive(Clone, Debug)]
struct FontFace {
    family: String,
    file: String,
    weight: (u16, u16),
}

#[derive(Clone, Debug)]
struct Contrast {
    fg: String,
    bg: String,
    min: f64,
    usage: String,
}

#[derive(Debug)]
struct Source {
    version: String,
    fonts: Vec<FontFace>,
    tokens: BTreeMap<String, Token>,
    contrast: Vec<Contrast>,
}

impl Source {
    fn of<'a>(&'a self, pred: impl Fn(&Def) -> bool + 'a) -> impl Iterator<Item = &'a Token> + 'a {
        self.tokens.values().filter(move |token| pred(&token.def))
    }

    fn colors(&self) -> impl Iterator<Item = &Token> {
        self.of(|def| matches!(def, Def::Color { .. } | Def::ColorAlias(_)))
    }

    /// Light and dark values of a colour token, following recipe aliases.
    fn resolve(&self, id: &str) -> (Rgba, Rgba) {
        match &self.tokens[id].def {
            Def::Color { light, dark } => (*light, *dark),
            Def::ColorAlias(target) => self.resolve(target),
            other => unreachable!("validated colour token, found {other:?}"),
        }
    }

    fn dim(&self, size: &Size) -> Dim {
        match size {
            Size::Own(dim) => dim.clone(),
            Size::Alias(id) => match &self.tokens[id].def {
                Def::Dimension(dim) => dim.clone(),
                other => unreachable!("validated dimension, found {other:?}"),
            },
        }
    }

    fn ratio(&self, line: &LineHeight) -> f64 {
        match line {
            LineHeight::Ratio(ratio) => *ratio,
            LineHeight::Alias(id) => match self.tokens[id].def {
                Def::Number(ratio) => ratio,
                ref other => unreachable!("validated number, found {other:?}"),
            },
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Parsing and validation

struct Problems(Vec<String>);

impl Problems {
    fn push(&mut self, at: &str, message: impl std::fmt::Display) {
        self.0.push(format!("{at}: {message}"));
    }
}

fn object<'a>(
    value: &'a Value,
    at: &str,
    problems: &mut Problems,
) -> Option<&'a Map<String, Value>> {
    let object = value.as_object();
    if object.is_none() {
        problems.push(at, "must be an object");
    }
    object
}

fn only(object: &Map<String, Value>, allowed: &[&str], at: &str, problems: &mut Problems) {
    for key in object.keys() {
        if !allowed.contains(&key.as_str()) {
            problems.push(at, format!("unknown field `{key}`"));
        }
    }
}

fn number(value: Option<&Value>, at: &str, field: &str, problems: &mut Problems) -> Option<f64> {
    match value.and_then(Value::as_f64) {
        Some(n) if n.is_finite() => Some(n),
        _ => {
            problems.push(at, format!("`{field}` must be a number"));
            None
        }
    }
}

fn alias(value: &str) -> Option<&str> {
    value
        .strip_prefix('{')
        .and_then(|rest| rest.strip_suffix('}'))
        .filter(|inner| !inner.is_empty())
}

fn hex(value: &str) -> Option<Rgba> {
    let digits = value.strip_prefix('#')?;
    if digits.len() != 6
        || !digits
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
    {
        return None;
    }
    let channel = |i: usize| u8::from_str_radix(&digits[i..i + 2], 16).ok();
    Some(Rgba {
        r: channel(0)?,
        g: channel(2)?,
        b: channel(4)?,
        a: 1.0,
    })
}

fn is_semver(version: &str) -> bool {
    let parts: Vec<&str> = version.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
}

/// CSS custom property for a token id: `color.` is dropped (`color.surface` → `--vk-surface`);
/// every other group keeps its name (`space.4` → `--vk-space-4`). A `css` field overrides it.
fn css_name(id: &str) -> String {
    let name = id.strip_prefix("color.").unwrap_or(id);
    format!("--vk-{}", name.replace('.', "-"))
}

fn parse_tokens(text: &str) -> Result<Source, Vec<String>> {
    let mut problems = Problems(Vec::new());
    let doc: Value = serde_json::from_str(text).map_err(|e| vec![format!("{TOKENS}: {e}")])?;
    let Some(root) = object(&doc, TOKENS, &mut problems) else {
        return Err(problems.0);
    };
    only(
        root,
        &[
            "$schema",
            "format",
            "name",
            "version",
            "fonts",
            "primitives",
            "tokens",
            "contrast",
        ],
        TOKENS,
        &mut problems,
    );
    if root.get("format") != Some(&Value::from(1)) {
        problems.push(TOKENS, "`format` must be 1");
    }
    if root.get("name").and_then(Value::as_str) != Some("vink-ui") {
        problems.push(TOKENS, "`name` must be \"vink-ui\"");
    }
    let version = root
        .get("version")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    if !is_semver(&version) {
        problems.push(TOKENS, "`version` must be MAJOR.MINOR.PATCH");
    }

    let mut primitives = BTreeMap::new();
    match root.get("primitives").and_then(Value::as_object) {
        Some(entries) => {
            for (name, entry) in entries {
                let at = format!("primitives.{name}");
                let Some(entry) = object(entry, &at, &mut problems) else {
                    continue;
                };
                only(entry, &["type", "value"], &at, &mut problems);
                if entry.get("type").and_then(Value::as_str) != Some("color") {
                    problems.push(&at, "primitives are colours (`type`: \"color\")");
                }
                match entry.get("value").and_then(Value::as_str).and_then(hex) {
                    Some(rgba) => {
                        primitives.insert(name.clone(), rgba);
                    }
                    None => problems.push(&at, "`value` must be a lowercase #rrggbb colour"),
                }
            }
        }
        None => problems.push(TOKENS, "`primitives` must be an object"),
    }

    let mut tokens = BTreeMap::new();
    match root.get("tokens").and_then(Value::as_object) {
        Some(entries) => {
            for (id, entry) in entries {
                if let Some(token) = parse_token(id, entry, &primitives, &mut problems) {
                    tokens.insert(id.clone(), token);
                }
            }
        }
        None => problems.push(TOKENS, "`tokens` must be an object"),
    }
    check_references(&tokens, &mut problems);

    let mut css_names = BTreeMap::new();
    for token in tokens.values() {
        if let Some(other) = css_names.insert(token.css.clone(), token.id.clone()) {
            problems.push(
                &token.id,
                format!("CSS name {} is also used by {other}", token.css),
            );
        }
    }

    let fonts = parse_fonts(root.get("fonts"), &tokens, &mut problems);
    let contrast = parse_contrast(root.get("contrast"), &mut problems);
    if !problems.0.is_empty() {
        return Err(problems.0);
    }
    let source = Source {
        version,
        fonts,
        tokens,
        contrast,
    };
    check_contrast(&source, &mut problems);
    if problems.0.is_empty() {
        Ok(source)
    } else {
        Err(problems.0)
    }
}

fn color_value(
    value: Option<&Value>,
    at: &str,
    side: &str,
    primitives: &BTreeMap<String, Rgba>,
    problems: &mut Problems,
) -> Option<Rgba> {
    let Some(value) = value else {
        problems.push(at, format!("missing `{side}` theme value"));
        return None;
    };
    let (reference, opacity) = match value {
        Value::String(reference) => (reference.as_str(), 1.0),
        Value::Object(fields) => {
            only(fields, &["ref", "alpha"], at, problems);
            let reference = fields
                .get("ref")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let opacity = number(fields.get("alpha"), at, "alpha", problems)?;
            if !(opacity > 0.0 && opacity < 1.0) {
                problems.push(at, "`alpha` must be between 0 and 1 (exclusive)");
                return None;
            }
            (reference, opacity)
        }
        _ => {
            problems.push(at, format!("`{side}` must be a primitive alias"));
            return None;
        }
    };
    let Some(name) = alias(reference) else {
        problems.push(
            at,
            format!("`{side}` must be an alias like {{lavender.38}}"),
        );
        return None;
    };
    match primitives.get(name) {
        Some(rgba) => Some(Rgba {
            a: opacity,
            ..*rgba
        }),
        None => {
            problems.push(
                at,
                format!("`{side}` references unknown primitive {{{name}}}"),
            );
            None
        }
    }
}

fn shadow_value(
    value: Option<&Value>,
    at: &str,
    side: &str,
    primitives: &BTreeMap<String, Rgba>,
    problems: &mut Problems,
) -> Option<Shadow> {
    let Some(fields) = value.and_then(Value::as_object) else {
        problems.push(at, format!("missing `{side}` shadow"));
        return None;
    };
    only(fields, &["x", "y", "blur", "spread", "color"], at, problems);
    Some(Shadow {
        x: number(fields.get("x"), at, "x", problems)?,
        y: number(fields.get("y"), at, "y", problems)?,
        blur: number(fields.get("blur"), at, "blur", problems)?,
        spread: number(fields.get("spread"), at, "spread", problems)?,
        color: color_value(fields.get("color"), at, side, primitives, problems)?,
    })
}

fn dimension(fields: &Map<String, Value>, at: &str, problems: &mut Problems) -> Option<Dim> {
    let value = number(fields.get("value"), at, "value", problems)?;
    if value < 0.0 {
        problems.push(at, "`value` must not be negative");
    }
    let web = match fields.get("web").and_then(Value::as_str) {
        Some("rem") => WebUnit::Rem,
        Some("px") => WebUnit::Px,
        Some(other) => {
            problems.push(at, format!("invalid unit `{other}` (expected rem or px)"));
            return None;
        }
        None => WebUnit::Rem,
    };
    let fluid = match (fields.get("max"), fields.get("fluid")) {
        (None, None) => None,
        (Some(max), Some(Value::Object(fluid))) => {
            only(fluid, &["base", "vw"], at, problems);
            let max = number(Some(max), at, "max", problems)?;
            let base = number(fluid.get("base"), at, "fluid.base", problems)?;
            let vw = number(fluid.get("vw"), at, "fluid.vw", problems)?;
            if max <= value || vw <= 0.0 {
                problems.push(
                    at,
                    "a fluid size needs `max` above `value` and a positive `vw`",
                );
            }
            Some((base, vw, max))
        }
        _ => {
            problems.push(at, "`max` and `fluid` must be given together");
            None
        }
    };
    let compact = match fields.get("compact") {
        None => None,
        some => Some(number(some, at, "compact", problems)?),
    };
    Some(Dim {
        value,
        fluid,
        compact,
        web,
    })
}

fn parse_token(
    id: &str,
    entry: &Value,
    primitives: &BTreeMap<String, Rgba>,
    problems: &mut Problems,
) -> Option<Token> {
    let at = id;
    if id.is_empty()
        || !id.split('.').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'-'))
        })
    {
        problems.push(at, "token ids are dot-separated lowercase names");
        return None;
    }
    let fields = object(entry, at, problems)?;
    let kind = fields
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let common = ["type", "rationale", "css"];
    let allowed =
        |extra: &[&'static str]| -> Vec<&str> { common.iter().chain(extra).copied().collect() };
    let def = match kind {
        "color" if fields.contains_key("value") => {
            only(fields, &allowed(&["value"]), at, problems);
            let target = fields.get("value").and_then(Value::as_str).and_then(alias);
            match target {
                Some(target) => Def::ColorAlias(target.to_string()),
                None => {
                    problems.push(at, "a colour recipe `value` must be a token alias");
                    return None;
                }
            }
        }
        "color" => {
            only(fields, &allowed(&["light", "dark"]), at, problems);
            let light = color_value(fields.get("light"), at, "light", primitives, problems);
            let dark = color_value(fields.get("dark"), at, "dark", primitives, problems);
            Def::Color {
                light: light?,
                dark: dark?,
            }
        }
        "shadow" => {
            only(fields, &allowed(&["light", "dark"]), at, problems);
            let light = shadow_value(fields.get("light"), at, "light", primitives, problems);
            let dark = shadow_value(fields.get("dark"), at, "dark", primitives, problems);
            Def::Shadow {
                light: light?,
                dark: dark?,
            }
        }
        "fontFamily" => {
            only(fields, &allowed(&["value", "native"]), at, problems);
            let stack: Vec<String> = fields
                .get("value")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .map(String::from)
                        .collect()
                })
                .unwrap_or_default();
            let native = fields
                .get("native")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if stack.len() < 2 {
                problems.push(at, "a font stack needs a face and a fallback");
            }
            if !["sansSerif", "serif", "monospace"].contains(&native) {
                problems.push(at, "`native` must be sansSerif, serif or monospace");
            }
            Def::Font {
                stack,
                native: native.to_string(),
            }
        }
        "dimension" => {
            only(
                fields,
                &allowed(&["value", "max", "fluid", "compact", "web"]),
                at,
                problems,
            );
            if !fields.contains_key("web") {
                problems.push(at, "a dimension names its Web unit (`web`: rem or px)");
            }
            Def::Dimension(dimension(fields, at, problems)?)
        }
        "number" => {
            only(fields, &allowed(&["value"]), at, problems);
            let value = number(fields.get("value"), at, "value", problems)?;
            if value <= 0.0 {
                problems.push(at, "`value` must be positive");
            }
            Def::Number(value)
        }
        "typography" => {
            only(fields, &allowed(&["value"]), at, problems);
            let value = object(fields.get("value").unwrap_or(&Value::Null), at, problems)?;
            only(
                value,
                &["family", "size", "weight", "lineHeight", "tracking"],
                at,
                problems,
            );
            let family = value.get("family").and_then(Value::as_str).and_then(alias);
            let Some(family) = family else {
                problems.push(at, "`family` must be a fontFamily alias");
                return None;
            };
            let size = match value.get("size") {
                Some(Value::String(reference)) => match alias(reference) {
                    Some(target) => Size::Alias(target.to_string()),
                    None => {
                        problems.push(at, "`size` alias must look like {text.reading}");
                        return None;
                    }
                },
                Some(Value::Object(fluid)) => {
                    only(fluid, &["value", "max", "fluid"], at, problems);
                    Size::Own(dimension(fluid, at, problems)?)
                }
                other => Size::Own(Dim {
                    value: number(other, at, "size", problems)?,
                    fluid: None,
                    compact: None,
                    web: WebUnit::Rem,
                }),
            };
            let weight = value.get("weight").and_then(Value::as_u64).unwrap_or(0);
            if !(100..=900).contains(&weight) || weight % 100 != 0 {
                problems.push(at, "`weight` must be 100–900 in steps of 100");
            }
            let line_height = match value.get("lineHeight") {
                Some(Value::String(reference)) => match alias(reference) {
                    Some(target) => LineHeight::Alias(target.to_string()),
                    None => {
                        problems.push(at, "`lineHeight` alias must look like {leading.reading}");
                        return None;
                    }
                },
                other => {
                    let ratio = number(other, at, "lineHeight", problems)?;
                    if !(1.0..=3.0).contains(&ratio) {
                        problems.push(at, "`lineHeight` is a unitless ratio between 1 and 3");
                    }
                    LineHeight::Ratio(ratio)
                }
            };
            let tracking = number(value.get("tracking"), at, "tracking", problems)?;
            Def::Type(TypeRole {
                family: family.to_string(),
                size,
                weight: weight as u16,
                line_height,
                tracking,
            })
        }
        "duration" => {
            only(fields, &allowed(&["value", "reduced"]), at, problems);
            let ms = fields
                .get("value")
                .and_then(Value::as_u64)
                .filter(|ms| *ms > 0);
            let Some(ms) = ms else {
                problems.push(at, "a duration is a positive whole number of milliseconds");
                return None;
            };
            let reduced = match fields.get("reduced") {
                None => None,
                Some(value) if value.as_u64() == Some(0) => Some(0),
                Some(_) => {
                    problems.push(
                        at,
                        "`reduced` may only be 0 (finite transitions become instant)",
                    );
                    return None;
                }
            };
            Def::Duration {
                ms: ms as u32,
                reduced,
            }
        }
        "iterations" => {
            only(fields, &allowed(&["value", "reduced"]), at, problems);
            if fields.get("value").and_then(Value::as_str) != Some("infinite")
                || fields.get("reduced").and_then(Value::as_u64) != Some(0)
            {
                problems.push(
                    at,
                    "loop iterations are \"infinite\" and stop (`reduced`: 0)",
                );
            }
            Def::Iterations
        }
        "cubicBezier" => {
            only(fields, &allowed(&["value"]), at, problems);
            let points: Vec<f64> = fields
                .get("value")
                .and_then(Value::as_array)
                .map(|items| items.iter().filter_map(Value::as_f64).collect())
                .unwrap_or_default();
            let Ok(points) = <[f64; 4]>::try_from(points) else {
                problems.push(at, "a cubic Bézier has four numbers");
                return None;
            };
            if !(0.0..=1.0).contains(&points[0]) || !(0.0..=1.0).contains(&points[2]) {
                problems.push(at, "Bézier x coordinates must be within 0–1");
            }
            Def::Bezier(points)
        }
        other => {
            problems.push(at, format!("unknown token type `{other}`"));
            return None;
        }
    };
    let css = match fields.get("css").and_then(Value::as_str) {
        Some(name) if name.starts_with("--vk-") => name.to_string(),
        Some(name) => {
            problems.push(at, format!("CSS name `{name}` must start with --vk-"));
            return None;
        }
        None => css_name(id),
    };
    Some(Token {
        id: id.to_string(),
        css,
        rationale: fields
            .get("rationale")
            .and_then(Value::as_str)
            .map(String::from),
        def,
    })
}

fn check_references(tokens: &BTreeMap<String, Token>, problems: &mut Problems) {
    let expect =
        |from: &str, target: &str, what: &str, ok: fn(&Def) -> bool, problems: &mut Problems| {
            match tokens.get(target) {
                None => problems.push(from, format!("dangling alias {{{target}}}")),
                Some(token) if !ok(&token.def) => {
                    problems.push(
                        from,
                        format!("alias {{{target}}} is not a {what} (type mismatch)"),
                    );
                }
                Some(_) => {}
            }
        };
    for token in tokens.values() {
        match &token.def {
            Def::ColorAlias(target) => {
                expect(
                    &token.id,
                    target,
                    "colour",
                    |d| matches!(d, Def::Color { .. } | Def::ColorAlias(_)),
                    problems,
                );
                // Follow the chain: a cycle never reaches a themed colour.
                let mut seen = BTreeSet::from([token.id.as_str()]);
                let mut next = target.as_str();
                while let Some(Def::ColorAlias(after)) = tokens.get(next).map(|t| &t.def) {
                    if !seen.insert(next) {
                        problems.push(&token.id, format!("alias cycle through {{{next}}}"));
                        break;
                    }
                    next = after;
                }
            }
            Def::Type(role) => {
                expect(
                    &token.id,
                    &role.family,
                    "fontFamily",
                    |d| matches!(d, Def::Font { .. }),
                    problems,
                );
                if let Size::Alias(target) = &role.size {
                    expect(
                        &token.id,
                        target,
                        "dimension",
                        |d| matches!(d, Def::Dimension(_)),
                        problems,
                    );
                }
                if let LineHeight::Alias(target) = &role.line_height {
                    expect(
                        &token.id,
                        target,
                        "number",
                        |d| matches!(d, Def::Number(_)),
                        problems,
                    );
                }
            }
            _ => {}
        }
    }
}

fn parse_fonts(
    value: Option<&Value>,
    tokens: &BTreeMap<String, Token>,
    problems: &mut Problems,
) -> Vec<FontFace> {
    let mut faces = Vec::new();
    let Some(items) = value.and_then(Value::as_array) else {
        problems.push(TOKENS, "`fonts` must be an array");
        return faces;
    };
    let families: BTreeSet<&str> = tokens
        .values()
        .filter_map(|token| match &token.def {
            Def::Font { stack, .. } => stack.first().map(String::as_str),
            _ => None,
        })
        .collect();
    for (index, item) in items.iter().enumerate() {
        let at = format!("fonts[{index}]");
        let Some(fields) = object(item, &at, problems) else {
            continue;
        };
        only(fields, &["family", "file", "weight"], &at, problems);
        let family = fields
            .get("family")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let file = fields
            .get("file")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if !families.contains(family) {
            problems.push(
                &at,
                format!("`{family}` is not the first face of any font token"),
            );
        }
        if !file.ends_with(".woff2") || file.contains('/') {
            problems.push(
                &at,
                "`file` is a .woff2 name inside the font asset directory",
            );
        }
        let weight = match fields.get("weight") {
            Some(Value::Number(n)) => n.as_u64().map(|w| (w, w)),
            Some(Value::Array(range)) if range.len() == 2 => {
                match (range[0].as_u64(), range[1].as_u64()) {
                    (Some(low), Some(high)) if low < high => Some((low, high)),
                    _ => None,
                }
            }
            _ => None,
        };
        let Some((low, high)) = weight.filter(|(l, h)| *l >= 100 && *h <= 900) else {
            problems.push(&at, "`weight` is 100–900 or an ascending [low, high] range");
            continue;
        };
        faces.push(FontFace {
            family: family.to_string(),
            file: file.to_string(),
            weight: (low as u16, high as u16),
        });
    }
    faces
}

fn parse_contrast(value: Option<&Value>, problems: &mut Problems) -> Vec<Contrast> {
    let Some(items) = value.and_then(Value::as_array) else {
        problems.push(TOKENS, "`contrast` must be an array");
        return Vec::new();
    };
    items
        .iter()
        .enumerate()
        .filter_map(|(index, item)| {
            let at = format!("contrast[{index}]");
            let fields = object(item, &at, problems)?;
            only(fields, &["fg", "bg", "min", "use"], &at, problems);
            let text = |key: &str| fields.get(key).and_then(Value::as_str).map(String::from);
            let min = number(fields.get("min"), &at, "min", problems)?;
            if ![3.0, 4.5, 7.0].contains(&min) {
                problems.push(&at, "`min` is 3, 4.5 or 7");
            }
            Some(Contrast {
                fg: text("fg")?,
                bg: text("bg")?,
                min,
                usage: text("use").unwrap_or_default(),
            })
        })
        .collect()
}

fn check_contrast(source: &Source, problems: &mut Problems) {
    for pair in &source.contrast {
        let at = format!("contrast {} on {}", pair.fg, pair.bg);
        let is_color = |id: &str| {
            source
                .tokens
                .get(id)
                .is_some_and(|t| matches!(t.def, Def::Color { .. } | Def::ColorAlias(_)))
        };
        if !is_color(&pair.fg) || !is_color(&pair.bg) {
            problems.push(&at, "both sides must be colour tokens");
            continue;
        }
        let (fg_light, fg_dark) = source.resolve(&pair.fg);
        let (bg_light, bg_dark) = source.resolve(&pair.bg);
        for (theme, fg, bg) in [("light", fg_light, bg_light), ("dark", fg_dark, bg_dark)] {
            if bg.a < 1.0 {
                problems.push(&at, format!("{theme} background must be opaque"));
                continue;
            }
            let ratio = contrast(fg, bg);
            if ratio + 1e-9 < pair.min {
                problems.push(
                    &at,
                    format!(
                        "{theme} ratio {ratio:.2} is below {} ({})",
                        num(pair.min),
                        pair.usage
                    ),
                );
            }
        }
    }
}

struct Catalog {
    version: String,
    statuses: Vec<String>,
    families: Vec<(String, String)>,
    entries: Vec<Entry>,
}

struct Entry {
    id: String,
    family: String,
    label: String,
    summary: String,
    scenarios: Vec<(String, String)>,
    status: BTreeMap<String, String>,
}

fn parse_catalog(text: &str) -> Result<Catalog, Vec<String>> {
    let mut problems = Problems(Vec::new());
    let doc: Value = serde_json::from_str(text).map_err(|e| vec![format!("{CATALOG}: {e}")])?;
    let at = CATALOG;
    let Some(root) = object(&doc, at, &mut problems) else {
        return Err(problems.0);
    };
    only(
        root,
        &["format", "version", "statuses", "families", "entries"],
        at,
        &mut problems,
    );
    if root.get("format") != Some(&Value::from(1)) {
        problems.push(at, "`format` must be 1");
    }
    let version = root
        .get("version")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    if !is_semver(&version) {
        problems.push(at, "`version` must be MAJOR.MINOR.PATCH");
    }
    let statuses: Vec<String> = root
        .get("statuses")
        .and_then(Value::as_object)
        .map(|s| s.keys().cloned().collect())
        .unwrap_or_default();
    if !statuses.iter().any(|s| s == "implemented-unverified") {
        problems.push(at, "`statuses` must define implemented-unverified");
    }
    let pairs =
        |value: Option<&Value>, at: &str, problems: &mut Problems| -> Vec<(String, String)> {
            let mut seen = BTreeSet::new();
            value
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|item| {
                    let id = item.get("id").and_then(Value::as_str)?;
                    let label = item.get("label").and_then(Value::as_str)?;
                    if !seen.insert(id.to_string()) {
                        problems.push(at, format!("duplicate id `{id}`"));
                    }
                    Some((id.to_string(), label.to_string()))
                })
                .collect()
        };
    let families = pairs(root.get("families"), at, &mut problems);
    let mut ids = BTreeSet::new();
    let mut entries = Vec::new();
    for item in root
        .get("entries")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let text = |key: &str| {
            item.get(key)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string()
        };
        let id = text("id");
        let at = format!("{CATALOG} {id}");
        if !ids.insert(id.clone()) || id.is_empty() {
            problems.push(&at, "entry ids are unique and non-empty");
        }
        let family = text("family");
        if !families.iter().any(|(f, _)| *f == family) || !id.starts_with(&format!("{family}.")) {
            problems.push(
                &at,
                format!("family `{family}` must be declared and prefix the id"),
            );
        }
        let scenarios = pairs(item.get("scenarios"), &at, &mut problems);
        if scenarios.first().map(|(id, _)| id.as_str()) != Some("default") {
            problems.push(&at, "the first scenario is `default`");
        }
        let status: BTreeMap<String, String> = item
            .get("status")
            .and_then(Value::as_object)
            .map(|s| {
                s.iter()
                    .map(|(k, v)| (k.clone(), v.as_str().unwrap_or_default().to_string()))
                    .collect()
            })
            .unwrap_or_default();
        for renderer in RENDERERS {
            match status.get(renderer) {
                Some(state) if statuses.contains(state) => {}
                _ => problems.push(&at, format!("needs a declared status for {renderer}")),
            }
            if status.get(renderer).is_some_and(|s| s == "unsupported")
                && item["unsupported"][renderer]
                    .as_str()
                    .is_none_or(str::is_empty)
            {
                problems.push(
                    &at,
                    format!("unsupported {renderer} needs an explicit reason"),
                );
            }
        }
        if status.len() != RENDERERS.len() {
            problems.push(&at, "status names exactly leptos and cmp");
        }
        entries.push(Entry {
            id,
            family,
            label: text("label"),
            summary: text("summary"),
            scenarios,
            status,
        });
    }
    if entries.is_empty() {
        problems.push(at, "the catalog has no entries");
    }
    if problems.0.is_empty() {
        Ok(Catalog {
            version,
            statuses,
            families,
            entries,
        })
    } else {
        Err(problems.0)
    }
}

struct Fixtures {
    clock: String,
    seed: u64,
    texts: BTreeMap<String, String>,
}

fn parse_fixtures(text: &str) -> Result<Fixtures, Vec<String>> {
    let doc: Value = serde_json::from_str(text).map_err(|e| vec![format!("{FIXTURES}: {e}")])?;
    let mut problems = Vec::new();
    let clock = doc
        .get("clock")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    if clock.len() != 25 || !clock.contains('T') {
        problems.push(format!(
            "{FIXTURES}: `clock` is a fixed RFC 3339 instant with offset"
        ));
    }
    let seed = doc.get("seed").and_then(Value::as_u64);
    let texts: BTreeMap<String, String> = doc
        .get("texts")
        .and_then(Value::as_object)
        .map(|t| {
            t.iter()
                .filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_string())))
                .collect()
        })
        .unwrap_or_default();
    for key in texts.keys() {
        if !key.bytes().all(|b| b.is_ascii_lowercase() || b == b'_') {
            problems.push(format!("{FIXTURES}: text key `{key}` is snake_case"));
        }
    }
    if texts.is_empty() || seed.is_none() {
        problems.push(format!("{FIXTURES}: needs `seed` and `texts`"));
    }
    if problems.is_empty() {
        Ok(Fixtures {
            clock,
            seed: seed.unwrap_or_default(),
            texts,
        })
    } else {
        Err(problems)
    }
}

// ---------------------------------------------------------------------------------------------
// Rendering helpers

/// Shortest exact decimal for the precision tokens use (four places), never exponent notation.
fn num(value: f64) -> String {
    let text = format!("{value:.4}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" {
        "0".into()
    } else {
        text.into()
    }
}

fn rem(px: f64) -> String {
    format!("{}rem", num(px / ROOT_PX))
}

fn web_dim(value: f64, unit: WebUnit) -> String {
    match unit {
        WebUnit::Rem => rem(value),
        WebUnit::Px => format!("{}px", num(value)),
    }
}

fn web_size(dim: &Dim, value: f64) -> String {
    match dim.fluid {
        Some((base, vw, max)) => format!(
            "clamp({}, {} + {}vw, {})",
            rem(value),
            rem(base),
            num(vw),
            rem(max)
        ),
        None => web_dim(value, dim.web),
    }
}

/// Upper camel case of a token id without its group: `color.on-primary` → `OnPrimary`.
fn camel(id: &str, drop_group: bool) -> String {
    let name = if drop_group {
        id.split_once('.').map_or(id, |(_, rest)| rest)
    } else {
        id
    };
    name.split(['.', '-', '_'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            chars
                .next()
                .map(|first| first.to_uppercase().chain(chars).collect::<String>())
                .unwrap_or_default()
        })
        .collect()
}

fn screaming(id: &str) -> String {
    id.replace(['.', '-'], "_").to_uppercase()
}

pub(crate) fn rust_str(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c if c.is_ascii() => out.push(c),
            // Combining marks and joiners are invisible on their own; escape them so NFD and
            // emoji sequences stay reviewable.
            c if is_combining(c) || is_invisible(c) => {
                let _ = write!(out, "\\u{{{:04x}}}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

pub(crate) fn kotlin_str(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '$' => out.push_str("\\$"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c if is_combining(c) || is_invisible(c) => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn is_combining(c: char) -> bool {
    matches!(c as u32, 0x0300..=0x036f | 0x1dc0..=0x1dff | 0x20d0..=0x20ff | 0xfe20..=0xfe2f)
}

fn is_invisible(c: char) -> bool {
    matches!(c as u32, 0x200b..=0x200f | 0x2060 | 0xfeff)
}

pub(crate) fn header(comment: &str, version: &str) -> String {
    format!(
        "{comment} GENERATED by `cargo xtask ui-tokens` from packages/ui-system/shared (vink-ui {version}).\n\
         {comment} Do not edit: change tokens.json, preview-catalog.json, icons.json or fixtures/ and regenerate.\n"
    )
}

// ---------------------------------------------------------------------------------------------
// CSS

const GENERIC_FAMILIES: [&str; 6] = [
    "sans-serif",
    "serif",
    "monospace",
    "ui-monospace",
    "system-ui",
    "cursive",
];

fn css_stack(stack: &[String]) -> String {
    stack
        .iter()
        .map(|face| {
            if GENERIC_FAMILIES.contains(&face.as_str()) {
                face.clone()
            } else {
                format!("\"{face}\"")
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn css_shadow(shadow: &Shadow) -> String {
    let length = |value: f64| {
        if value == 0.0 {
            "0".to_string()
        } else {
            format!("{}px", num(value))
        }
    };
    let spread = if shadow.spread == 0.0 {
        String::new()
    } else {
        format!(" {}", length(shadow.spread))
    };
    format!(
        "{} {} {}{spread} {}",
        length(shadow.x),
        length(shadow.y),
        length(shadow.blur),
        shadow.color.css()
    )
}

fn block(out: &mut String, selector: &str, lines: &[String]) {
    let _ = writeln!(out, "{selector} {{");
    for line in lines {
        let _ = writeln!(out, "  {line}");
    }
    out.push_str("}\n\n");
}

fn theme_lines(source: &Source, dark: bool) -> Vec<String> {
    let mut lines = Vec::new();
    for token in source.tokens.values() {
        match &token.def {
            Def::Color { light, dark: night } => {
                let value = if dark { night } else { light };
                lines.push(format!("{}: {};", token.css, value.css()));
            }
            Def::Shadow { light, dark: night } => {
                let value = if dark { night } else { light };
                lines.push(format!("{}: {};", token.css, css_shadow(value)));
            }
            _ => {}
        }
    }
    lines
}

fn css(source: &Source) -> String {
    let mut out = String::new();
    let _ = write!(
        out,
        "/*\n{}{} */\n\n",
        header(" *", &source.version),
        [
            " *",
            " * Theme: light by default; dark follows the system unless `data-theme=\"light|dark\"` on",
            " * <html> chooses. A `.vk-scope` element re-themes its subtree (`data-theme` light, dark or",
            " * system), sets density (`data-density` comfortable|compact) and may only add reduced",
            " * motion (`data-motion=\"reduce\"`); the system reduced-motion setting always applies.",
            " * Recipe aliases are re-declared on every scope so they resolve against the scope's roles.",
            "",
        ]
        .join("\n")
    );
    for face in &source.fonts {
        let weight = if face.weight.0 == face.weight.1 {
            face.weight.0.to_string()
        } else {
            format!("{} {}", face.weight.0, face.weight.1)
        };
        let _ = write!(
            out,
            "@font-face {{\n  font-family: \"{}\";\n  src: url(\"fonts/{}\") format(\"woff2\");\n  \
             font-style: normal;\n  font-weight: {weight};\n  font-display: swap;\n}}\n\n",
            face.family, face.file
        );
    }

    let mut root = vec!["color-scheme: light dark;".to_string()];
    let mut comfortable = Vec::new();
    let mut compact = Vec::new();
    let mut reduced = Vec::new();
    for token in source.tokens.values() {
        let declare = |value: String| format!("{}: {value};", token.css);
        match &token.def {
            Def::Font { stack, .. } => root.push(declare(css_stack(stack))),
            Def::Dimension(dim) => {
                root.push(declare(web_size(dim, dim.value)));
                if let Some(small) = dim.compact {
                    comfortable.push(declare(web_dim(dim.value, dim.web)));
                    compact.push(declare(web_dim(small, dim.web)));
                }
            }
            Def::Number(value) => root.push(declare(num(*value))),
            Def::Duration { ms, reduced: less } => {
                root.push(declare(format!("{ms}ms")));
                if let Some(less) = less {
                    reduced.push(declare(format!("{less}ms")));
                }
            }
            Def::Iterations => {
                root.push(declare("infinite".into()));
                reduced.push(declare("0".into()));
            }
            Def::Bezier(p) => root.push(declare(format!(
                "cubic-bezier({}, {}, {}, {})",
                num(p[0]),
                num(p[1]),
                num(p[2]),
                num(p[3])
            ))),
            Def::Type(role) => {
                let var = |id: &str| format!("var({})", source.tokens[id].css);
                root.push(format!("{}-font: {};", token.css, var(&role.family)));
                let size = match &role.size {
                    Size::Own(dim) => web_size(dim, dim.value),
                    Size::Alias(id) => var(id),
                };
                root.push(format!("{}-size: {size};", token.css));
                root.push(format!("{}-weight: {};", token.css, role.weight));
                let line = match &role.line_height {
                    LineHeight::Ratio(ratio) => num(*ratio),
                    LineHeight::Alias(id) => var(id),
                };
                root.push(format!("{}-line-height: {line};", token.css));
                let tracking = if role.tracking == 0.0 {
                    "0".to_string()
                } else {
                    format!("{}em", num(role.tracking))
                };
                root.push(format!("{}-tracking: {tracking};", token.css));
            }
            Def::Color { .. } | Def::ColorAlias(_) | Def::Shadow { .. } => {}
        }
    }
    let light = theme_lines(source, false);
    let dark = theme_lines(source, true);
    root.extend(light.iter().cloned());
    block(&mut out, ":root", &root);
    block(
        &mut out,
        ":root[data-theme=\"light\"]",
        &["color-scheme: light;".into()],
    );
    let mut scoped_light = vec!["color-scheme: light;".to_string()];
    scoped_light.extend(light.iter().cloned());
    block(&mut out, ".vk-scope[data-theme=\"light\"]", &scoped_light);
    let mut scoped_dark = vec!["color-scheme: dark;".to_string()];
    scoped_dark.extend(dark.iter().cloned());
    block(
        &mut out,
        ":root[data-theme=\"dark\"],\n.vk-scope[data-theme=\"dark\"]",
        &scoped_dark,
    );
    let mut scoped_system = vec!["color-scheme: light dark;".to_string()];
    scoped_system.extend(light.iter().cloned());
    block(&mut out, ".vk-scope[data-theme=\"system\"]", &scoped_system);
    out.push_str("@media (prefers-color-scheme: dark) {\n");
    let mut inner = String::new();
    block(
        &mut inner,
        ":root:not([data-theme=\"light\"]),\n.vk-scope[data-theme=\"system\"]",
        &dark,
    );
    for line in inner.trim_end().lines() {
        if line.is_empty() {
            out.push('\n');
        } else {
            let _ = writeln!(out, "  {line}");
        }
    }
    out.push_str("}\n\n");

    let recipes: Vec<String> = source
        .of(|def| matches!(def, Def::ColorAlias(_)))
        .map(|token| match &token.def {
            Def::ColorAlias(target) => {
                format!("{}: var({});", token.css, source.tokens[target].css)
            }
            _ => unreachable!(),
        })
        .collect();
    block(&mut out, ":root,\n.vk-scope", &recipes);
    block(
        &mut out,
        ".vk-scope[data-theme]",
        &[
            "background-color: var(--vk-surface);".into(),
            "color: var(--vk-on-surface);".into(),
        ],
    );
    block(
        &mut out,
        ".vk-scope[data-density=\"comfortable\"]",
        &comfortable,
    );
    block(
        &mut out,
        ":root[data-density=\"compact\"],\n.vk-scope[data-density=\"compact\"]",
        &compact,
    );
    block(
        &mut out,
        ":root[data-motion=\"reduce\"],\n.vk-scope[data-motion=\"reduce\"]",
        &reduced,
    );
    out.push_str("@media (prefers-reduced-motion: reduce) {\n  :root {\n");
    for line in &reduced {
        let _ = writeln!(out, "    {line}");
    }
    out.push_str("  }\n}\n");
    out
}

// ---------------------------------------------------------------------------------------------
// Rust

fn rust_rgba(c: Rgba) -> String {
    format!("Rgba::new({}, {}, {}, {})", c.r, c.g, c.b, rust_f64(c.a))
}

fn rust_f64(value: f64) -> String {
    let text = num(value);
    if text.contains('.') {
        text
    } else {
        format!("{text}.0")
    }
}

fn rust_tokens(source: &Source) -> String {
    let mut out = header("//!", &source.version);
    out.push_str(
        "//!\n//! Typed accessors for every token. Colours carry resolved light/dark values for previews\n\
         //! and checks; components style through `css_var()` so themes, scopes and overrides apply.\n\n\
         use crate::values::{\n    ContrastPair, Dimension, Duration, Easing, FontFace, FontFamily, NativeFamily, Rgba, TextRole,\n    TextSize, WebUnit,\n};\n\n",
    );
    let _ = writeln!(
        out,
        "pub const VERSION: &str = {};",
        rust_str(&source.version)
    );
    let _ = writeln!(out, "pub const SOURCE: &str = \"{TOKENS}\";\n");

    let colors: Vec<&Token> = source.colors().collect();
    out.push_str(
        "/// Semantic colour roles and recipe aliases, in token-id order.\n\
         #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]\npub enum ColorRole {\n",
    );
    for token in &colors {
        if let Some(why) = &token.rationale {
            let _ = writeln!(out, "    /// {why}");
        }
        let _ = writeln!(out, "    {},", color_variant(&token.id));
    }
    out.push_str("}\n\nimpl ColorRole {\n");
    let _ = writeln!(out, "    pub const ALL: [ColorRole; {}] = [", colors.len());
    for token in &colors {
        let _ = writeln!(out, "        ColorRole::{},", color_variant(&token.id));
    }
    out.push_str("    ];\n\n");
    let arms = |out: &mut String, name: &str, ret: &str, value: &dyn Fn(&Token) -> String| {
        let _ = writeln!(
            out,
            "    pub const fn {name}(self) -> {ret} {{\n        match self {{"
        );
        for token in &colors {
            let _ = writeln!(
                out,
                "            ColorRole::{} => {},",
                color_variant(&token.id),
                value(token)
            );
        }
        out.push_str("        }\n    }\n\n");
    };
    arms(&mut out, "token", "&'static str", &|t| rust_str(&t.id));
    arms(&mut out, "css_var", "&'static str", &|t| rust_str(&t.css));
    arms(&mut out, "light", "Rgba", &|t| {
        rust_rgba(source.resolve(&t.id).0)
    });
    arms(&mut out, "dark", "Rgba", &|t| {
        rust_rgba(source.resolve(&t.id).1)
    });
    arms(&mut out, "alias", "Option<ColorRole>", &|t| match &t.def {
        Def::ColorAlias(target) => format!("Some(ColorRole::{})", color_variant(target)),
        _ => "None".into(),
    });
    out.truncate(out.trim_end().len());
    out.push_str("\n}\n\n");

    out.push_str("/// Declared contrast pairs; the generator rejects a source where one falls below `min`.\npub const CONTRAST: &[ContrastPair] = &[\n");
    for pair in &source.contrast {
        let _ = writeln!(
            out,
            "    ContrastPair {{\n        fg: ColorRole::{},\n        bg: ColorRole::{},\n        min: {},\n        usage: {},\n    }},",
            color_variant(&pair.fg),
            color_variant(&pair.bg),
            rust_f64(pair.min),
            rust_str(&pair.usage)
        );
    }
    out.push_str("];\n\n/// Shadows: CSS value per theme.\npub mod shadow {\n");
    for token in source.of(|def| matches!(def, Def::Shadow { .. })) {
        if let Def::Shadow { light, dark } = &token.def {
            let _ = writeln!(
                out,
                "    pub const {}: (&str, &str, &str) = ({}, {}, {});",
                screaming(token.id.split_once('.').map_or(&token.id, |(_, r)| r)),
                rust_str(&token.css),
                rust_str(&css_shadow(light)),
                rust_str(&css_shadow(dark))
            );
        }
    }
    out.push_str("}\n\n/// Font stacks.\npub mod font {\n    use super::*;\n\n");
    for token in source.of(|def| matches!(def, Def::Font { .. })) {
        if let Def::Font { stack, native } = &token.def {
            let faces: Vec<String> = stack.iter().map(|s| rust_str(s)).collect();
            let _ = writeln!(
                out,
                "    pub const {}: FontFamily = FontFamily {{\n        token: {},\n        css_var: {},\n        stack: &[{}],\n        native: NativeFamily::{},\n    }};",
                screaming(&token.id[5..]),
                rust_str(&token.id),
                rust_str(&token.css),
                faces.join(", "),
                camel(native, false)
            );
        }
    }
    out.push_str("}\n\n/// Self-hosted faces (files live in apps/vink-web/assets/fonts).\npub const FONT_FACES: &[FontFace] = &[\n");
    for face in &source.fonts {
        let _ = writeln!(
            out,
            "    FontFace {{\n        family: {},\n        file: {},\n        weight: ({}, {}),\n    }},",
            rust_str(&face.family),
            rust_str(&face.file),
            face.weight.0,
            face.weight.1
        );
    }
    out.push_str("];\n\n/// Geometry in CSS px / dp; `web` says how CSS prints it.\npub mod dim {\n    use super::*;\n\n");
    let dims: Vec<&Token> = source.of(|def| matches!(def, Def::Dimension(_))).collect();
    for token in &dims {
        if let Def::Dimension(dim) = &token.def {
            let compact = dim
                .compact
                .map_or("None".into(), |v| format!("Some({})", rust_f64(v)));
            let max = dim.fluid.map_or("None".into(), |(_, _, max)| {
                format!("Some({})", rust_f64(max))
            });
            let _ = writeln!(
                out,
                "    pub const {}: Dimension = Dimension {{\n        token: {},\n        css_var: {},\n        value: {},\n        max: {max},\n        compact: {compact},\n        web: WebUnit::{:?},\n    }};",
                screaming(&token.id),
                rust_str(&token.id),
                rust_str(&token.css),
                rust_f64(dim.value),
                dim.web
            );
        }
    }
    let _ = writeln!(out, "\n    pub const ALL: [Dimension; {}] = [", dims.len());
    for token in &dims {
        let _ = writeln!(out, "        {},", screaming(&token.id));
    }
    out.push_str("    ];\n}\n\n/// Unitless numbers.\npub mod number {\n");
    for token in source.of(|def| matches!(def, Def::Number(_))) {
        if let Def::Number(value) = &token.def {
            let _ = writeln!(
                out,
                "    pub const {}: f64 = {};",
                screaming(&token.id),
                rust_f64(*value)
            );
        }
    }
    out.push_str("}\n\n/// Text roles; CSS consumers read `--vk-type-<role>-{font,size,weight,line-height,tracking}`.\npub mod text {\n    use super::*;\n\n");
    let roles: Vec<&Token> = source.of(|def| matches!(def, Def::Type(_))).collect();
    for token in &roles {
        if let Def::Type(role) = &token.def {
            let dim = source.dim(&role.size);
            let size = match dim.fluid {
                Some((_, _, max)) => format!(
                    "TextSize::Fluid {{\n            min: {},\n            max: {},\n        }}",
                    rust_f64(dim.value),
                    rust_f64(max)
                ),
                None => format!("TextSize::Fixed({})", rust_f64(dim.value)),
            };
            let _ = writeln!(
                out,
                "    pub const {}: TextRole = TextRole {{\n        token: {},\n        css_prefix: {},\n        family: font::{},\n        size: {size},\n        weight: {},\n        line_height: {},\n        tracking_em: {},\n    }};",
                screaming(&token.id[5..]),
                rust_str(&token.id),
                rust_str(&token.css),
                screaming(&role.family[5..]),
                role.weight,
                rust_f64(source.ratio(&role.line_height)),
                rust_f64(role.tracking)
            );
        }
    }
    let _ = writeln!(out, "\n    pub const ALL: [TextRole; {}] = [", roles.len());
    for token in &roles {
        let _ = writeln!(out, "        {},", screaming(&token.id[5..]));
    }
    out.push_str("    ];\n}\n\n/// Motion: finite transitions, the loading loop and easing curves.\npub mod motion {\n    use super::*;\n\n");
    for token in source.tokens.values() {
        match &token.def {
            Def::Duration { ms, reduced } => {
                let reduced = reduced.map_or("None".into(), |r| format!("Some({r})"));
                let _ = writeln!(
                    out,
                    "    pub const {}: Duration = Duration {{\n        token: {},\n        css_var: {},\n        ms: {ms},\n        reduced_ms: {reduced},\n    }};",
                    screaming(&token.id),
                    rust_str(&token.id),
                    rust_str(&token.css)
                );
            }
            Def::Bezier(p) => {
                let _ = writeln!(
                    out,
                    "    pub const {}: Easing = Easing {{\n        token: {},\n        css_var: {},\n        points: [{}, {}, {}, {}],\n    }};",
                    screaming(&token.id),
                    rust_str(&token.id),
                    rust_str(&token.css),
                    rust_f64(p[0]),
                    rust_f64(p[1]),
                    rust_f64(p[2]),
                    rust_f64(p[3])
                );
            }
            Def::Iterations => {
                let _ = writeln!(
                    out,
                    "    /// `infinite`, or `0` (stopped) under reduced motion.\n    pub const {}: &str = {};",
                    screaming(&token.id),
                    rust_str(&token.css)
                );
            }
            _ => {}
        }
    }
    out.push_str("}\n");
    out
}

fn color_variant(id: &str) -> String {
    match id.strip_prefix("color.") {
        Some(rest) => camel(rest, false),
        None => camel(id, false),
    }
}

fn rust_catalog(source: &Source, catalog: &Catalog, fixtures: &Fixtures) -> String {
    let mut out = header("//!", &source.version);
    out.push_str("//!\n//! Preview catalog IDs, renderer statuses and presentation fixtures shared with CMP.\n\n");
    out.push_str("use crate::values::{CatalogEntry, Scenario};\n\n");
    let _ = writeln!(
        out,
        "pub const CATALOG_VERSION: &str = {};",
        rust_str(&catalog.version)
    );
    let statuses: Vec<String> = catalog.statuses.iter().map(|s| rust_str(s)).collect();
    let _ = writeln!(
        out,
        "pub const STATUSES: &[&str] = &[{}];",
        statuses.join(", ")
    );
    let families: Vec<String> = catalog
        .families
        .iter()
        .map(|(id, label)| format!("({}, {})", rust_str(id), rust_str(label)))
        .collect();
    let _ = writeln!(
        out,
        "pub const FAMILIES: &[(&str, &str)] = &[{}];\n",
        families.join(", ")
    );
    out.push_str("pub const ENTRIES: &[CatalogEntry] = &[\n");
    for entry in &catalog.entries {
        let _ = writeln!(
            out,
            "    CatalogEntry {{\n        id: {},\n        family: {},\n        label: {},\n        summary: {},\n        scenarios: &[",
            rust_str(&entry.id),
            rust_str(&entry.family),
            rust_str(&entry.label),
            rust_str(&entry.summary)
        );
        for (id, label) in &entry.scenarios {
            let _ = writeln!(
                out,
                "            Scenario {{\n                id: {},\n                label: {},\n            }},",
                rust_str(id),
                rust_str(label)
            );
        }
        let _ = writeln!(
            out,
            "        ],\n        leptos: {},\n        cmp: {},\n    }},",
            rust_str(&entry.status["leptos"]),
            rust_str(&entry.status["cmp"])
        );
    }
    out.push_str("];\n\n/// Fixed presentation inputs: the same clock, seed and Vietnamese samples in both previews.\npub mod fixture {\n");
    let _ = writeln!(
        out,
        "    pub const CLOCK: &str = {};",
        rust_str(&fixtures.clock)
    );
    let _ = writeln!(out, "    pub const SEED: u64 = {};", fixtures.seed);
    for (key, text) in &fixtures.texts {
        let _ = writeln!(
            out,
            "    pub const {}: &str = {};",
            key.to_uppercase(),
            rust_str(text)
        );
    }
    out.push_str("}\n");
    out
}

// ---------------------------------------------------------------------------------------------
// Kotlin

fn kotlin_float(value: f64) -> String {
    let text = num(value);
    if text.contains('.') {
        format!("{text}f")
    } else {
        format!("{text}.0f")
    }
}

fn kotlin_double(value: f64) -> String {
    let text = num(value);
    if text.contains('.') {
        text
    } else {
        format!("{text}.0")
    }
}

fn kotlin_unit(value: f64, suffix: &str) -> String {
    format!("{}.{suffix}", num(value))
}

fn kotlin_tokens(source: &Source) -> String {
    let mut out = header("//", &source.version);
    out.push_str(
        "//\n// Common (all targets) token values. Geometry is dp, type is sp with line height in sp\n\
         // (size × ratio), so platform font scale applies exactly once. Colours are ARGB.\n\n\
         package vink.ui\n\n\
         import androidx.compose.animation.core.CubicBezierEasing\n\
         import androidx.compose.ui.unit.dp\n\
         import androidx.compose.ui.unit.sp\n\n",
    );
    let _ = writeln!(
        out,
        "object VinkTokens {{\n    const val VERSION: String = {}\n    const val SOURCE: String = \"{TOKENS}\"\n}}\n",
        kotlin_str(&source.version)
    );
    let colors: Vec<&Token> = source.colors().collect();
    out.push_str(
        "/** Semantic colour roles and recipe aliases; `light`/`dark` are resolved ARGB values. */\n\
         enum class ColorRole(val token: String, val cssVar: String, val light: Long, val dark: Long) {\n",
    );
    for (index, token) in colors.iter().enumerate() {
        if let Some(why) = &token.rationale {
            let _ = writeln!(out, "    /** {why} */");
        }
        let (light, dark) = source.resolve(&token.id);
        let end = if index + 1 == colors.len() { ";" } else { "," };
        let _ = writeln!(
            out,
            "    {}({}, {}, {}, {}){end}",
            color_variant(&token.id),
            kotlin_str(&token.id),
            kotlin_str(&token.css),
            light.argb(),
            dark.argb()
        );
    }
    out.push_str("\n    /** The semantic role a recipe follows, so scope overrides reach recipes too. */\n    val alias: ColorRole?\n        get() = when (this) {\n");
    for token in &colors {
        if let Def::ColorAlias(target) = &token.def {
            let _ = writeln!(
                out,
                "            {} -> {}",
                color_variant(&token.id),
                color_variant(target)
            );
        }
    }
    out.push_str("            else -> null\n        }\n}\n\n");
    out.push_str("/** Declared contrast pairs; the generator rejects a source where one falls below `min`. */\nval VinkContrastPairs: List<ContrastPair> = listOf(\n");
    for pair in &source.contrast {
        let _ = writeln!(
            out,
            "    ContrastPair(ColorRole.{}, ColorRole.{}, {}, {}),",
            color_variant(&pair.fg),
            color_variant(&pair.bg),
            kotlin_double(pair.min),
            kotlin_str(&pair.usage)
        );
    }
    out.push_str(")\n\n");

    out.push_str("/** Font stacks; native renderers map `native` until a TTF/OTF pipeline exists. */\nenum class FontRole(val token: String, val stack: List<String>, val native: NativeFamily) {\n");
    let fonts: Vec<&Token> = source.of(|def| matches!(def, Def::Font { .. })).collect();
    for (index, token) in fonts.iter().enumerate() {
        if let Def::Font { stack, native } = &token.def {
            let end = if index + 1 == fonts.len() { ";" } else { "," };
            let faces: Vec<String> = stack.iter().map(|s| kotlin_str(s)).collect();
            let _ = writeln!(
                out,
                "    {}({}, listOf({}), NativeFamily.{}){end}",
                camel(&token.id, true),
                kotlin_str(&token.id),
                faces.join(", "),
                camel(native, false)
            );
        }
    }
    out.push_str("}\n\n/** Self-hosted faces (apps/vink-web/assets/fonts); WOFF2 is not loaded natively. */\nval VinkFontFaces: List<FontFace> = listOf(\n");
    for face in &source.fonts {
        let _ = writeln!(
            out,
            "    FontFace({}, {}, {}, {}),",
            kotlin_str(&face.family),
            kotlin_str(&face.file),
            face.weight.0,
            face.weight.1
        );
    }
    out.push_str(")\n\n/** Geometry in dp. Density-dependent roles are in [VinkDensityDimens]. */\nobject VinkDimens {\n");
    for token in source.of(|def| matches!(def, Def::Dimension(_))) {
        if let Def::Dimension(dim) = &token.def {
            let _ = writeln!(
                out,
                "    val {} = {}",
                camel(&token.id, false),
                kotlin_unit(dim.value, "dp")
            );
        }
    }
    out.push_str("}\n\n/** Roles whose value depends on density: comfortable / compact. */\nobject VinkDensityDimens {\n");
    for token in source.of(|def| matches!(def, Def::Dimension(d) if d.compact.is_some())) {
        if let Def::Dimension(dim) = &token.def {
            let _ = writeln!(
                out,
                "    val {} = DensityDimen({}, {})",
                camel(&token.id, true),
                kotlin_unit(dim.value, "dp"),
                kotlin_unit(dim.compact.unwrap_or(dim.value), "dp")
            );
        }
    }
    out.push_str("}\n\n/** Text roles. Fluid Web sizes use their minimum here; `maxSize` records the Web cap. */\nobject VinkTextRoles {\n");
    let roles: Vec<&Token> = source.of(|def| matches!(def, Def::Type(_))).collect();
    for token in &roles {
        if let Def::Type(role) = &token.def {
            let dim = source.dim(&role.size);
            let ratio = source.ratio(&role.line_height);
            let max = dim
                .fluid
                .map_or("null".into(), |(_, _, max)| kotlin_unit(max, "sp"));
            let _ = writeln!(
                out,
                "    val {} = VinkTextRole({}, FontRole.{}, size = {}, maxSize = {max}, weight = {}, lineHeight = {}, trackingEm = {})",
                camel(&token.id, true),
                kotlin_str(&token.id),
                camel(&role.family, true),
                kotlin_unit(dim.value, "sp"),
                role.weight,
                kotlin_unit(dim.value * ratio, "sp"),
                kotlin_float(role.tracking)
            );
        }
    }
    let names: Vec<String> = roles.iter().map(|t| camel(&t.id, true)).collect();
    let _ = writeln!(
        out,
        "\n    val all: List<VinkTextRole> = listOf({})\n}}\n",
        names.join(", ")
    );
    out.push_str("/** Motion tokens. Reduced finite transitions become instant; loops stop, never spin at 0 ms. */\nobject VinkMotionTokens {\n");
    for token in source.tokens.values() {
        match &token.def {
            Def::Duration { ms, reduced } => {
                let name = camel(&token.id, false);
                let _ = writeln!(out, "    const val {name}Ms: Int = {ms}");
                if let Some(r) = reduced {
                    let _ = writeln!(out, "    const val {name}ReducedMs: Int = {r}");
                }
            }
            Def::Bezier(p) => {
                let _ = writeln!(
                    out,
                    "    val {} = CubicBezierEasing({}, {}, {}, {})",
                    camel(&token.id, false),
                    kotlin_float(p[0]),
                    kotlin_float(p[1]),
                    kotlin_float(p[2]),
                    kotlin_float(p[3])
                );
            }
            _ => {}
        }
    }
    out.push_str("}\n");
    out
}

fn kotlin_catalog(source: &Source, catalog: &Catalog, fixtures: &Fixtures) -> String {
    let mut out = header("//", &source.version);
    out.push_str("//\n// Preview catalog IDs, renderer statuses and presentation fixtures shared with Leptos.\n\npackage vink.ui\n\n");
    let _ = writeln!(
        out,
        "object VinkCatalog {{\n    const val VERSION: String = {}",
        kotlin_str(&catalog.version)
    );
    let statuses: Vec<String> = catalog.statuses.iter().map(|s| kotlin_str(s)).collect();
    let _ = writeln!(
        out,
        "    val statuses: List<String> = listOf({})",
        statuses.join(", ")
    );
    let families: Vec<String> = catalog
        .families
        .iter()
        .map(|(id, label)| format!("{} to {}", kotlin_str(id), kotlin_str(label)))
        .collect();
    let _ = writeln!(
        out,
        "    val families: List<Pair<String, String>> = listOf({})\n",
        families.join(", ")
    );
    out.push_str("    val entries: List<CatalogEntry> = listOf(\n");
    for entry in &catalog.entries {
        let scenarios: Vec<String> = entry
            .scenarios
            .iter()
            .map(|(id, label)| format!("Scenario({}, {})", kotlin_str(id), kotlin_str(label)))
            .collect();
        let _ = writeln!(
            out,
            "        CatalogEntry(\n            id = {},\n            family = {},\n            label = {},\n            summary = {},\n            scenarios = listOf({}),\n            leptos = {},\n            cmp = {},\n        ),",
            kotlin_str(&entry.id),
            kotlin_str(&entry.family),
            kotlin_str(&entry.label),
            kotlin_str(&entry.summary),
            scenarios.join(", "),
            kotlin_str(&entry.status["leptos"]),
            kotlin_str(&entry.status["cmp"])
        );
    }
    out.push_str("    )\n}\n\n/** Fixed presentation inputs: the same clock, seed and Vietnamese samples in both previews. */\nobject VinkFixture {\n");
    let _ = writeln!(
        out,
        "    const val CLOCK: String = {}",
        kotlin_str(&fixtures.clock)
    );
    let _ = writeln!(out, "    const val SEED: Long = {}", fixtures.seed);
    for (key, text) in &fixtures.texts {
        let _ = writeln!(
            out,
            "    const val {}: String = {}",
            key.to_uppercase(),
            kotlin_str(text)
        );
    }
    out.push_str("}\n");
    out
}

// ---------------------------------------------------------------------------------------------
// Icon registry and identity vectors

/// One registry icon: its path is copied verbatim from the pinned upstream file.
struct Icon {
    id: String,
    upstream: String,
    usage: String,
    path: String,
}

struct Icons {
    repository: String,
    commit: String,
    license: String,
    icons: Vec<Icon>,
}

/// The transparent frame most Material Icons files start with.
const DEFAULT_FRAME: &str = "M0 0h24v24H0z";
/// The only other frame spelling an upstream file in the registry uses (e.g. `menu_open`).
const FRAMES: [&str; 2] = [DEFAULT_FRAME, "M0 0h24v24H0V0z"];

/// Material Icons files share this exact wrapper; rebuilding it lets the generator compare the
/// recorded upstream SHA-256 without vendoring the SVG files.
fn upstream_svg(frame: &str, path: &str) -> String {
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" height=\"24\" viewBox=\"0 0 24 24\" width=\"24\"><path d=\"{frame}\" fill=\"none\"/><path d=\"{path}\"/></svg>"
    )
}

fn parse_icons(text: &str, license_exists: impl Fn(&str) -> bool) -> Result<Icons, Vec<String>> {
    use sha2::{Digest, Sha256};
    let doc: Value = serde_json::from_str(text).map_err(|e| vec![format!("{ICONS}: {e}")])?;
    let mut problems = Vec::new();
    let field = |value: &Value, key: &str| {
        value
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let source = doc.get("source").cloned().unwrap_or(Value::Null);
    let (repository, commit, license) = (
        field(&source, "repository"),
        field(&source, "commit"),
        field(&source, "license"),
    );
    if doc.get("format") != Some(&Value::from(1)) || field(&doc, "viewBox") != "0 0 24 24" {
        problems.push(format!(
            "{ICONS}: `format` is 1 and `viewBox` is \"0 0 24 24\""
        ));
    }
    if commit.len() != 40 || !commit.bytes().all(|b| b.is_ascii_hexdigit()) {
        problems.push(format!("{ICONS}: `source.commit` pins a full commit SHA"));
    }
    if license != "Apache-2.0" || !license_exists(&field(&source, "licenseFile")) {
        problems.push(format!(
            "{ICONS}: `source.license` is Apache-2.0 and `source.licenseFile` exists"
        ));
    }
    let mut icons = Vec::new();
    for (id, item) in doc
        .get("icons")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
    {
        let at = format!("{ICONS} {id}");
        if id.is_empty() || !id.bytes().all(|b| b.is_ascii_lowercase() || b == b'-') {
            problems.push(format!("{at}: ids are kebab-case"));
        }
        let path = field(item, "path");
        if path.is_empty()
            || !path
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || " .,-".contains(c))
        {
            problems.push(format!("{at}: `path` is SVG path data"));
        }
        // An optional `frame` records a differently spelled transparent frame in the upstream
        // file; it never reaches the rendered icon.
        let frame = item
            .get("frame")
            .and_then(Value::as_str)
            .unwrap_or(DEFAULT_FRAME);
        if !FRAMES.contains(&frame) {
            problems.push(format!("{at}: `frame` is a known Material Icons frame"));
        }
        let digest: String = Sha256::digest(upstream_svg(frame, &path).as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        if digest != field(item, "sha256") {
            problems.push(format!(
                "{at}: path does not match the recorded upstream sha256"
            ));
        }
        let usage = field(item, "use");
        if usage.is_empty() {
            problems.push(format!("{at}: `use` names the action the icon stands for"));
        }
        icons.push(Icon {
            id: id.clone(),
            upstream: field(item, "upstream"),
            usage,
            path,
        });
    }
    if icons.is_empty() {
        problems.push(format!("{ICONS}: the registry has no icons"));
    }
    if problems.is_empty() {
        Ok(Icons {
            repository,
            commit,
            license,
            icons,
        })
    } else {
        Err(problems)
    }
}

struct Identity {
    version: String,
    tones: u64,
    initials: Vec<(Option<String>, Option<String>)>,
    tone_vectors: Vec<(Option<String>, Option<u64>)>,
    people: Vec<(Option<String>, Option<String>)>,
    sample_png: String,
    broken_avif: String,
}

fn parse_identity(text: &str) -> Result<Identity, Vec<String>> {
    let doc: Value = serde_json::from_str(text).map_err(|e| vec![format!("{IDENTITY}: {e}")])?;
    let mut problems = Vec::new();
    let list = |key: &str| -> Vec<Value> {
        doc.get(key)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    };
    let text_of = |v: &Value, key: &str| v.get(key).and_then(Value::as_str).map(str::to_string);
    let version = doc
        .get("version")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let tones = doc.get("tones").and_then(Value::as_u64).unwrap_or_default();
    if doc.get("format") != Some(&Value::from(1)) || !version.starts_with("avatar-identity-v") {
        problems.push(format!(
            "{IDENTITY}: `format` is 1 and `version` names avatar-identity-vN"
        ));
    }
    if !(2..=12).contains(&tones) {
        problems.push(format!("{IDENTITY}: `tones` is between 2 and 12"));
    }
    let initials: Vec<_> = list("initials")
        .iter()
        .map(|v| (text_of(v, "name"), text_of(v, "expected")))
        .collect();
    let tone_vectors: Vec<_> = list("tones_vectors")
        .iter()
        .map(|v| (text_of(v, "key"), v.get("expected").and_then(Value::as_u64)))
        .collect();
    if tone_vectors
        .iter()
        .any(|(_, t)| t.is_some_and(|t| t == 0 || t > tones))
    {
        problems.push(format!("{IDENTITY}: expected tones are 1..=tones or null"));
    }
    let people: Vec<_> = list("people")
        .iter()
        .map(|v| (text_of(v, "key"), text_of(v, "name")))
        .collect();
    let base64 = |section: &str, key: &str| {
        doc.get(section)
            .and_then(|s| s.get(key))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let sample_png = base64("sample_image", "png_base64");
    let broken_avif = base64("broken_image", "avif_base64");
    for (name, text) in [
        ("sample_image", &sample_png),
        ("broken_image", &broken_avif),
    ] {
        let valid = !text.is_empty()
            && text
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"+/=".contains(&b));
        if !valid {
            problems.push(format!("{IDENTITY}: `{name}` holds base64 bytes"));
        }
    }
    if !sample_png.starts_with("iVBORw0KGgo") {
        problems.push(format!("{IDENTITY}: `sample_image` is a PNG"));
    }
    if initials.is_empty() || tone_vectors.is_empty() || people.is_empty() {
        problems.push(format!(
            "{IDENTITY}: needs `initials`, `tones_vectors` and `people`"
        ));
    }
    if problems.is_empty() {
        Ok(Identity {
            version,
            tones,
            initials,
            tone_vectors,
            people,
            sample_png,
            broken_avif,
        })
    } else {
        Err(problems)
    }
}

fn pascal(id: &str) -> String {
    id.split('-')
        .map(|part| {
            let mut chars = part.chars();
            chars
                .next()
                .map(|c| c.to_ascii_uppercase().to_string() + chars.as_str())
                .unwrap_or_default()
        })
        .collect()
}

fn rust_opt(value: &Option<String>) -> String {
    value
        .as_deref()
        .map_or("None".into(), |v| format!("Some({})", rust_str(v)))
}

fn kotlin_opt(value: &Option<String>) -> String {
    value.as_deref().map_or("null".into(), kotlin_str)
}

fn rust_icons(version: &str, icons: &Icons) -> String {
    let mut out = header("//!", version);
    let _ = writeln!(
        out,
        "//!\n//! Icon registry shared with CMP: {}@{} (Material Icons filled, 24px), {}.\n//! See packages/ui-system/shared/icons.json for upstream paths and SHA-256.\n",
        icons.repository, icons.commit, icons.license
    );
    out.push_str("/// The viewBox every registry path is drawn in.\npub const VIEW_BOX: &str = \"0 0 24 24\";\n");
    let _ = writeln!(
        out,
        "pub const SOURCE: &str = {};\n",
        rust_str(&format!(
            "{}@{} ({})",
            icons.repository, icons.commit, icons.license
        ))
    );
    out.push_str("#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]\npub enum IconName {\n");
    for icon in &icons.icons {
        let _ = writeln!(out, "    /// {}\n    {},", icon.usage, pascal(&icon.id));
    }
    let names: Vec<String> = icons
        .icons
        .iter()
        .map(|i| format!("Self::{}", pascal(&i.id)))
        .collect();
    let _ = writeln!(
        out,
        "}}\n\nimpl IconName {{\n    pub const ALL: [IconName; {}] = [{}];\n",
        icons.icons.len(),
        names.join(", ")
    );
    for (name, value) in [("id", 0), ("path", 1), ("upstream", 2)] {
        let _ = writeln!(
            out,
            "    pub const fn {name}(self) -> &'static str {{\n        match self {{"
        );
        for icon in &icons.icons {
            let text = match value {
                0 => &icon.id,
                1 => &icon.path,
                _ => &icon.upstream,
            };
            let _ = writeln!(
                out,
                "            Self::{} => {},",
                pascal(&icon.id),
                rust_str(text)
            );
        }
        out.push_str("        }\n    }\n");
        if value < 2 {
            out.push('\n');
        }
    }
    out.push_str("}\n");
    out
}

fn kotlin_icons(version: &str, icons: &Icons) -> String {
    let mut out = header("//", version);
    let _ = writeln!(
        out,
        "//\n// Icon registry shared with Leptos: {}@{} (Material Icons filled, 24px), {}.\n// See packages/ui-system/shared/icons.json for upstream paths and SHA-256.\n\npackage vink.ui\n",
        icons.repository, icons.commit, icons.license
    );
    let _ = writeln!(
        out,
        "object VinkIconSource {{\n    const val VIEW_BOX: String = \"0 0 24 24\"\n    const val VIEWPORT: Float = 24f\n    const val SOURCE: String = {}\n}}\n",
        kotlin_str(&format!(
            "{}@{} ({})",
            icons.repository, icons.commit, icons.license
        ))
    );
    out.push_str("enum class IconName(val id: String, val path: String) {\n");
    let last = icons.icons.len().saturating_sub(1);
    for (i, icon) in icons.icons.iter().enumerate() {
        let _ = writeln!(
            out,
            "    /** {} */\n    {}({}, {}){}",
            icon.usage,
            pascal(&icon.id),
            kotlin_str(&icon.id),
            kotlin_str(&icon.path),
            if i == last { ";" } else { "," }
        );
    }
    out.push_str("}\n");
    out
}

fn rust_identity(version: &str, identity: &Identity) -> String {
    let mut out = header("//!", version);
    out.push_str("//!\n//! Golden vectors for `crate::identity` shared with CMP, and [Mẫu] people for previews.\n//! `None` expects the neutral fallback.\n\n");
    let _ = writeln!(
        out,
        "pub const VERSION: &str = {};\npub const TONES: u8 = {};\n",
        rust_str(&identity.version),
        identity.tones
    );
    out.push_str("/// (display name, expected initials)\npub const INITIALS: &[(Option<&str>, Option<&str>)] = &[\n");
    for (name, expected) in &identity.initials {
        let _ = writeln!(out, "    ({}, {}),", rust_opt(name), rust_opt(expected));
    }
    out.push_str("];\n\n/// (identity key, expected tone)\npub const TONE_VECTORS: &[(Option<&str>, Option<u8>)] = &[\n");
    for (key, expected) in &identity.tone_vectors {
        let tone = expected.map_or("None".into(), |t| format!("Some({t})"));
        let _ = writeln!(out, "    ({}, {tone}),", rust_opt(key));
    }
    out.push_str("];\n\n/// [Mẫu] (identity key, display name) for previews; not real accounts.\npub const PEOPLE: &[(Option<&str>, Option<&str>)] = &[\n");
    for (key, name) in &identity.people {
        let _ = writeln!(out, "    ({}, {}),", rust_opt(key), rust_opt(name));
    }
    let _ = writeln!(
        out,
        "];\n\n/// [Mẫu] PNG made by Vink (not a person), base64.\npub const SAMPLE_PNG_BASE64: &str = {};\n\n/// Undecodable bytes labelled AVIF, base64: exercises the decode-failure fallback.\npub const BROKEN_AVIF_BASE64: &str = {};",
        rust_str(&identity.sample_png),
        rust_str(&identity.broken_avif)
    );
    out
}

fn kotlin_identity(version: &str, identity: &Identity) -> String {
    let mut out = header("//", version);
    out.push_str("//\n// Golden vectors for VinkIdentity.kt shared with Leptos, and [Mẫu] people for previews.\n// null expects the neutral fallback.\n\npackage vink.ui\n\n");
    let _ = writeln!(
        out,
        "object VinkIdentityVectors {{\n    const val VERSION: String = {}\n    const val TONES: Int = {}\n",
        kotlin_str(&identity.version),
        identity.tones
    );
    let pairs = |rows: Vec<String>| rows.join(",\n        ");
    let _ = writeln!(
        out,
        "    /** (display name, expected initials) */\n    val initials: List<Pair<String?, String?>> = listOf(\n        {},\n    )\n",
        pairs(
            identity
                .initials
                .iter()
                .map(|(n, e)| format!("{} to {}", kotlin_opt(n), kotlin_opt(e)))
                .collect()
        )
    );
    let _ = writeln!(
        out,
        "    /** (identity key, expected tone) */\n    val tones: List<Pair<String?, Int?>> = listOf(\n        {},\n    )\n",
        pairs(
            identity
                .tone_vectors
                .iter()
                .map(|(k, t)| format!(
                    "{} to {}",
                    kotlin_opt(k),
                    t.map_or("null".into(), |t| t.to_string())
                ))
                .collect()
        )
    );
    let _ = writeln!(
        out,
        "    /** [Mẫu] (identity key, display name) for previews; not real accounts. */\n    val people: List<Pair<String?, String?>> = listOf(\n        {},\n    )\n}}",
        pairs(
            identity
                .people
                .iter()
                .map(|(k, n)| format!("{} to {}", kotlin_opt(k), kotlin_opt(n)))
                .collect()
        )
    );
    out.truncate(out.trim_end_matches(['}', '\n']).len());
    let _ = writeln!(
        out,
        "\n\n    /** [Mẫu] PNG made by Vink (not a person), base64. */\n    const val SAMPLE_PNG_BASE64: String = {}\n\n    /** Undecodable bytes labelled AVIF, base64: exercises the decode-failure fallback. */\n    const val BROKEN_AVIF_BASE64: String = {}\n}}",
        kotlin_str(&identity.sample_png),
        kotlin_str(&identity.broken_avif)
    );
    out
}

/// Validates the icon registry and identity vectors and renders their four outputs.
pub fn generate_assets(
    version: &str,
    icons: &str,
    identity: &str,
    license_exists: impl Fn(&str) -> bool,
) -> Result<Vec<(&'static str, String)>, Vec<String>> {
    let icons = parse_icons(icons, license_exists);
    let identity = parse_identity(identity);
    let (icons, identity) = match (icons, identity) {
        (Ok(i), Ok(d)) => (i, d),
        (a, b) => {
            return Err(a
                .err()
                .unwrap_or_default()
                .into_iter()
                .chain(b.err().unwrap_or_default())
                .collect());
        }
    };
    Ok(vec![
        (RUST_ICONS_OUT, rust_icons(version, &icons)),
        (RUST_IDENTITY_OUT, rust_identity(version, &identity)),
        (KOTLIN_ICONS_OUT, kotlin_icons(version, &icons)),
        (KOTLIN_IDENTITY_OUT, kotlin_identity(version, &identity)),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sources() -> (String, String, String) {
        let root = crate::root();
        let read = |p: &str| std::fs::read_to_string(root.join(p)).expect(p);
        (read(TOKENS), read(CATALOG), read(FIXTURES))
    }

    fn with_token(edit: impl FnOnce(&mut Map<String, Value>)) -> Vec<String> {
        let (tokens, catalog, fixtures) = sources();
        let mut doc: Value = serde_json::from_str(&tokens).unwrap();
        edit(doc["tokens"].as_object_mut().unwrap());
        generate(&doc.to_string(), &catalog, &fixtures).expect_err("must be rejected")
    }

    fn mentions(problems: &[String], needle: &str) -> bool {
        problems.iter().any(|p| p.contains(needle))
    }

    fn asset_sources() -> (String, String) {
        let root = crate::root();
        let read = |p: &str| std::fs::read_to_string(root.join(p)).expect(p);
        (read(ICONS), read(IDENTITY))
    }

    #[test]
    fn icon_registry_matches_upstream_and_generates_stably() {
        let (icons, identity) = asset_sources();
        let first = generate_assets("0", &icons, &identity, |_| true).expect("valid assets");
        assert_eq!(
            first,
            generate_assets("0", &icons, &identity, |_| true).unwrap()
        );
        assert_eq!(first.len(), 4);
        let mut doc: Value = serde_json::from_str(&icons).unwrap();
        doc["icons"]["add"]["path"] = Value::from("M19 13h-6v6h-2v-6H5v-2h6V5h2v6h6v3z");
        let problems = generate_assets("0", &doc.to_string(), &identity, |_| true).unwrap_err();
        assert!(mentions(&problems, "upstream sha256"), "{problems:?}");
        let problems = generate_assets("0", &icons, &identity, |_| false).unwrap_err();
        assert!(mentions(&problems, "licenseFile"), "{problems:?}");
    }

    #[test]
    fn identity_vectors_reject_out_of_range_tones() {
        let (icons, identity) = asset_sources();
        let mut doc: Value = serde_json::from_str(&identity).unwrap();
        doc["tones_vectors"][0]["expected"] = Value::from(7);
        let problems = generate_assets("0", &icons, &doc.to_string(), |_| true).unwrap_err();
        assert!(mentions(&problems, "1..=tones"), "{problems:?}");
        let outputs = generate_assets("0", &icons, &identity, |_| true).unwrap();
        let rust = &outputs[1].1;
        assert!(
            rust.contains("Nguye\\u{0302}\\u{0303}n"),
            "NFD stays escaped"
        );
        assert!(rust.contains("\\u{200d}"), "joiners stay escaped");
    }

    #[test]
    fn repository_source_generates_twice_without_a_difference() {
        let (tokens, catalog, fixtures) = sources();
        let first = generate(&tokens, &catalog, &fixtures).expect("valid source");
        let second = generate(&tokens, &catalog, &fixtures).expect("valid source");
        assert_eq!(first, second);
        assert_eq!(first.len(), 5);
    }

    #[test]
    fn committed_outputs_match_the_source() {
        let root = crate::root();
        let (tokens, catalog, fixtures) = sources();
        let (icons, identity) = asset_sources();
        let assets = generate_assets("0.4.0", &icons, &identity, |_| true).unwrap();
        let version = parse_tokens(&tokens).ok().map(|s| s.version);
        assert_eq!(
            version.as_deref(),
            Some("0.4.0"),
            "update the asset version above"
        );
        let states = std::fs::read_to_string(root.join(crate::ui_states::STATES)).unwrap();
        let policy = crate::ui_states::GatePolicy {
            delay: 300,
            min_visible: 500,
            slow: 8000,
        };
        let states = crate::ui_states::generate("0.4.0", &states, policy).unwrap();
        let navigation =
            std::fs::read_to_string(root.join(crate::ui_navigation::NAVIGATION)).unwrap();
        let navigation = crate::ui_navigation::generate("0.4.0", &navigation).unwrap();
        for (path, text) in generate(&tokens, &catalog, &fixtures)
            .unwrap()
            .into_iter()
            .chain(assets)
            .chain(states)
            .chain(navigation)
        {
            let current = std::fs::read_to_string(root.join(path)).unwrap_or_default();
            assert!(
                current == text,
                "{path} is stale; run `cargo xtask ui-tokens`"
            );
        }
    }

    #[test]
    fn alias_cycles_are_rejected() {
        let problems = with_token(|tokens| {
            tokens.insert(
                "loop.a".into(),
                serde_json::json!({"type": "color", "value": "{loop.b}"}),
            );
            tokens.insert(
                "loop.b".into(),
                serde_json::json!({"type": "color", "value": "{loop.a}"}),
            );
        });
        assert!(mentions(&problems, "alias cycle"), "{problems:?}");
    }

    #[test]
    fn dangling_aliases_and_type_mismatches_are_rejected() {
        let problems = with_token(|tokens| {
            tokens.insert(
                "bad.dangling".into(),
                serde_json::json!({"type": "color", "value": "{color.nope}"}),
            );
            tokens.insert(
                "bad.mismatch".into(),
                serde_json::json!({"type": "color", "value": "{space.4}"}),
            );
        });
        assert!(
            mentions(&problems, "dangling alias {color.nope}"),
            "{problems:?}"
        );
        assert!(mentions(&problems, "type mismatch"), "{problems:?}");
    }

    #[test]
    fn a_role_missing_a_theme_is_rejected() {
        let problems = with_token(|tokens| {
            tokens["color.primary"]
                .as_object_mut()
                .unwrap()
                .remove("dark");
        });
        assert!(
            mentions(&problems, "color.primary: missing `dark` theme value"),
            "{problems:?}"
        );
    }

    #[test]
    fn invalid_units_and_values_are_rejected() {
        let problems = with_token(|tokens| {
            tokens["space.4"]["web"] = "em".into();
            tokens["duration.short"]["value"] = 0.into();
            tokens["duration.medium"]["reduced"] = 1.into();
            tokens["loop.iterations"]["reduced"] = 1.into();
            tokens.insert(
                "primary.unknown".into(),
                serde_json::json!({"type": "color", "light": "#123456", "dark": "{neutral.6}"}),
            );
        });
        assert!(mentions(&problems, "invalid unit `em`"), "{problems:?}");
        assert!(
            mentions(&problems, "positive whole number of milliseconds"),
            "{problems:?}"
        );
        assert!(
            mentions(&problems, "`reduced` may only be 0"),
            "{problems:?}"
        );
        assert!(mentions(&problems, "loop iterations"), "{problems:?}");
        assert!(mentions(&problems, "must be an alias"), "{problems:?}");
    }

    #[test]
    fn unknown_fields_and_types_are_rejected() {
        let problems = with_token(|tokens| {
            tokens["space.4"]["colour"] = "x".into();
            tokens.insert("odd.kind".into(), serde_json::json!({"type": "gradient"}));
        });
        assert!(
            mentions(&problems, "unknown field `colour`"),
            "{problems:?}"
        );
        assert!(
            mentions(&problems, "unknown token type `gradient`"),
            "{problems:?}"
        );
    }

    #[test]
    fn insufficient_contrast_is_rejected() {
        let problems = with_token(|tokens| {
            tokens["color.on-surface"]["light"] = "{neutral.96}".into();
        });
        assert!(mentions(&problems, "light ratio"), "{problems:?}");
    }

    #[test]
    fn catalog_requires_both_renderers_and_a_default_scenario() {
        let (tokens, catalog, fixtures) = sources();
        let mut doc: Value = serde_json::from_str(&catalog).unwrap();
        doc["entries"][0]["status"]
            .as_object_mut()
            .unwrap()
            .remove("cmp");
        doc["entries"][1]["scenarios"][0]["id"] = "other".into();
        let problems = generate(&tokens, &doc.to_string(), &fixtures).unwrap_err();
        assert!(
            mentions(&problems, "needs a declared status for cmp"),
            "{problems:?}"
        );
        assert!(
            mentions(&problems, "first scenario is `default`"),
            "{problems:?}"
        );
    }

    #[test]
    fn unsupported_catalog_registration_requires_a_reason() {
        let (tokens, catalog, fixtures) = sources();
        let mut doc: Value = serde_json::from_str(&catalog).unwrap();
        doc["entries"][0]["status"]["cmp"] = "unsupported".into();
        assert!(mentions(
            &generate(&tokens, &doc.to_string(), &fixtures).unwrap_err(),
            "unsupported cmp needs an explicit reason"
        ));
        doc["entries"][0]["unsupported"]["cmp"] =
            "Platform-specific semantics; see target contract".into();
        assert!(generate(&tokens, &doc.to_string(), &fixtures).is_ok());
    }

    #[test]
    fn contrast_uses_the_wcag_formula() {
        let white = hex("#ffffff").unwrap();
        let black = hex("#000000").unwrap();
        assert!((contrast(black, white) - 21.0).abs() < 1e-9);
        // The design baseline measured primary on surface at 6.77 (light).
        let ratio = contrast(hex("#6846a5").unwrap(), hex("#fffaff").unwrap());
        assert!((ratio - 6.77).abs() < 0.01, "{ratio}");
    }

    #[test]
    fn numbers_print_without_float_noise() {
        assert_eq!(num(0.1 + 0.2), "0.3");
        assert_eq!(rem(17.0), "1.0625rem");
        assert_eq!(rem(672.0), "42rem");
        assert_eq!(num(-0.0), "0");
        assert_eq!(num(-0.025), "-0.025");
    }

    #[test]
    fn generated_css_keeps_the_web_role_names_and_scopes_recipes() {
        let (tokens, catalog, fixtures) = sources();
        let outputs = generate(&tokens, &catalog, &fixtures).unwrap();
        let css = &outputs[0].1;
        for needle in [
            "--vk-surface: #fffaff;",
            "--vk-primary: #d4bbff;",
            "--vk-state-hover: rgb(104 70 165 / 0.08);",
            "--vk-shadow: 0 12px 40px rgb(36 29 45 / 0.14);",
            "--vk-text-reading: clamp(1.0625rem, 1rem + 0.25vw, 1.25rem);",
            "--vk-space-4: 1rem;",
            "--vk-radius-md: 12px;",
            "--vk-loop-iterations: 0;",
            ":root,\n.vk-scope {\n  --vk-action-primary-container: var(--vk-primary);",
        ] {
            assert!(css.contains(needle), "missing {needle}");
        }
        assert!(
            !css.lines()
                .any(|line| line.starts_with(|c: char| c.is_ascii_lowercase())
                    && line.trim_end().ends_with('{')),
            "tokens never style global tags"
        );
    }
}
