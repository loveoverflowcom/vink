//! Golden vectors for the UIS-04 navigation contract (`packages/ui-system/shared/navigation.md`).
//!
//! `fixtures/navigation.json` states the window classes, the adaptive layout table, the rail
//! toggle, the one-modal overlay layer and focus return once; this module validates their
//! vocabulary and completeness and renders the same vectors into Rust and Kotlin so each
//! renderer's tests compare its own implementation with them.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use serde_json::Value;

use crate::ui_tokens::{header, kotlin_str, rust_str};

pub const NAVIGATION: &str = "packages/ui-system/shared/fixtures/navigation.json";
pub const RUST_NAVIGATION_OUT: &str = "packages/ui-system/leptos/src/navigation_vectors.rs";
pub const KOTLIN_NAVIGATION_OUT: &str =
    "packages/ui-system/cmp/ui/src/commonMain/kotlin/vink/ui/VinkNavigationVectors.kt";

const WINDOWS: [&str; 3] = ["compact", "medium", "expanded"];
const CONTEXTS: [&str; 4] = ["-", "compact-rail", "hide-bar", "compact-rail+hide-bar"];
const PREFERENCES: [&str; 3] = ["-", "expanded", "compact"];
const LAYOUTS: [&str; 4] = ["rail:expanded", "rail:compact", "bar", "bar:hidden"];
const REASONS: [&str; 6] = ["dismiss", "escape", "back", "outside", "resize", "navigate"];
const FOCUS: [&str; 3] = ["trigger", "main", "leave"];

struct Navigation {
    version: String,
    css_px: (u64, u64),
    dp: (u64, u64),
    windows: Vec<(String, u64, String)>,
    layouts: Vec<[String; 4]>,
    toggles: Vec<(String, String)>,
    overlay: Vec<(String, Vec<(String, String)>)>,
    focus: Vec<(String, bool, String)>,
}

fn str_at(row: &Value, i: usize) -> Option<String> {
    row.get(i)?.as_str().map(str::to_string)
}

fn rows(doc: &Value, key: &str) -> Vec<Value> {
    doc.get(key)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

fn pair(doc: &Value, key: &str) -> Option<(u64, u64)> {
    let list = doc.get("breakpoints")?.get(key)?.as_array()?;
    match list[..] {
        [ref a, ref b] => Some((a.as_u64()?, b.as_u64()?)),
        _ => None,
    }
}

/// An overlay step is `open:<id>`, `close:<id>:<reason>`, `back` or `escape`.
fn op_ok(op: &str) -> bool {
    let id_ok =
        |id: &str| !id.is_empty() && id.bytes().all(|b| b.is_ascii_lowercase() || b == b'-');
    match op.split(':').collect::<Vec<_>>()[..] {
        ["back"] | ["escape"] => true,
        ["open", id] => id_ok(id),
        ["close", id, reason] => id_ok(id) && REASONS.contains(&reason),
        _ => false,
    }
}

/// An outcome is `ok`, `busy:<id>`, `closed:<id>:<reason>`, `stale` or `pass`.
fn outcome_ok(outcome: &str) -> bool {
    match outcome.split(':').collect::<Vec<_>>()[..] {
        ["ok"] | ["stale"] | ["pass"] => true,
        ["busy", id] => !id.is_empty(),
        ["closed", id, reason] => !id.is_empty() && REASONS.contains(&reason),
        _ => false,
    }
}

fn parse(text: &str) -> Result<Navigation, Vec<String>> {
    let doc: Value = serde_json::from_str(text).map_err(|e| vec![format!("{NAVIGATION}: {e}")])?;
    let mut problems = Vec::new();
    let mut bad = |what: String| problems.push(format!("{NAVIGATION}: {what}"));
    if doc.get("format") != Some(&Value::from(1)) {
        bad("`format` is 1".into());
    }
    let version = doc
        .get("version")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    if version.is_empty() {
        bad("`version` names the contract".into());
    }
    let (Some(css_px), Some(dp)) = (pair(&doc, "cssPx"), pair(&doc, "dp")) else {
        bad("`breakpoints.cssPx` and `breakpoints.dp` are [medium, expanded]".into());
        return Err(problems);
    };
    if css_px.0 >= css_px.1 || dp.0 >= dp.1 {
        bad("each breakpoint pair increases".into());
    }

    let mut windows = Vec::new();
    for row in rows(&doc, "windows") {
        let (Some(unit), Some(width), Some(class)) = (
            str_at(&row, 0),
            row.get(1).and_then(Value::as_u64),
            str_at(&row, 2),
        ) else {
            bad(format!("window row {row} is [unit, width, class]"));
            continue;
        };
        let (medium, expanded) = match unit.as_str() {
            "px" => css_px,
            "dp" => dp,
            _ => {
                bad(format!("window unit {unit} is px or dp"));
                continue;
            }
        };
        let expected = if width < medium {
            "compact"
        } else if width < expanded {
            "medium"
        } else {
            "expanded"
        };
        if class != expected {
            bad(format!(
                "{unit} {width} is {expected} under the breakpoints, not {class}"
            ));
        }
        windows.push((unit, width, class));
    }
    for (unit, (medium, expanded)) in [("px", css_px), ("dp", dp)] {
        for edge in [medium - 1, medium, expanded - 1, expanded] {
            if !windows.iter().any(|(u, w, _)| u == unit && *w == edge) {
                bad(format!("windows cover the {unit} edge {edge}"));
            }
        }
    }

    let mut layouts = Vec::new();
    let mut seen = BTreeSet::new();
    for row in rows(&doc, "layouts") {
        let cells: Option<Vec<String>> = (0..4).map(|i| str_at(&row, i)).collect();
        let cells: Option<[String; 4]> = cells.and_then(|c| c.try_into().ok());
        let Some([window, context, preference, layout]) = cells else {
            bad(format!("layout row {row} has four strings"));
            continue;
        };
        if !WINDOWS.contains(&window.as_str())
            || !CONTEXTS.contains(&context.as_str())
            || !PREFERENCES.contains(&preference.as_str())
            || !LAYOUTS.contains(&layout.as_str())
        {
            bad(format!(
                "unknown window, context, preference or layout in {row}"
            ));
        }
        if !seen.insert((window.clone(), context.clone(), preference.clone())) {
            bad(format!(
                "layout ({window}, {context}, {preference}) appears twice"
            ));
        }
        layouts.push([window, context, preference, layout]);
    }
    if seen.len() != WINDOWS.len() * CONTEXTS.len() * PREFERENCES.len() {
        bad("every window × context × preference appears exactly once".into());
    }

    let mut toggles = Vec::new();
    for row in rows(&doc, "toggles") {
        match (str_at(&row, 0), str_at(&row, 1)) {
            (Some(shown), Some(next))
                if LAYOUTS.contains(&shown.as_str()) && PREFERENCES.contains(&next.as_str()) =>
            {
                toggles.push((shown, next));
            }
            _ => bad(format!("toggle row {row} is [layout, preference or -]")),
        }
    }
    if toggles.len() != LAYOUTS.len() {
        bad("toggles cover every layout once".into());
    }

    let mut overlay = Vec::new();
    for case in rows(&doc, "overlay") {
        let name = case
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let mut steps = Vec::new();
        for step in case
            .get("steps")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            match (str_at(step, 0), str_at(step, 1)) {
                (Some(op), Some(outcome)) if op_ok(&op) && outcome_ok(&outcome) => {
                    steps.push((op, outcome));
                }
                _ => bad(format!(
                    "overlay {name}: step {step} is [operation, outcome]"
                )),
            }
        }
        if name.is_empty() || steps.is_empty() {
            bad("each overlay case has a name and steps".into());
        }
        overlay.push((name, steps));
    }

    let mut focus = Vec::new();
    for row in rows(&doc, "focus") {
        match (
            str_at(&row, 0),
            row.get(1).and_then(Value::as_bool),
            str_at(&row, 2),
        ) {
            (Some(reason), Some(visible), Some(to))
                if REASONS.contains(&reason.as_str()) && FOCUS.contains(&to.as_str()) =>
            {
                focus.push((reason, visible, to));
            }
            _ => bad(format!(
                "focus row {row} is [reason, trigger visible, target]"
            )),
        }
    }
    if focus.len() != REASONS.len() * 2 {
        bad("focus covers every close reason with and without a visible trigger".into());
    }

    if problems.is_empty() {
        Ok(Navigation {
            version,
            css_px,
            dp,
            windows,
            layouts,
            toggles,
            overlay,
            focus,
        })
    } else {
        Err(problems)
    }
}

fn rust(version: &str, n: &Navigation) -> String {
    let mut out = header("//!", version);
    out.push_str("//!\n//! Golden vectors for `crate::navigation` shared with CMP (shared/navigation.md). Strings use\n//! the contract's ids; the tests in `navigation.rs` parse them into the renderer's own types.\n\n");
    let _ = writeln!(out, "pub const VERSION: &str = {};\n", rust_str(&n.version));
    let _ = writeln!(
        out,
        "/// (medium, expanded) lower bounds: Web CSS px of the #7 shell and Material dp.\npub const CSS_PX: (u32, u32) = ({}, {});\npub const DP: (u32, u32) = ({}, {});\n",
        n.css_px.0, n.css_px.1, n.dp.0, n.dp.1
    );
    out.push_str(
        "/// (unit, available width, window class)\npub const WINDOWS: &[(&str, u32, &str)] = &[\n",
    );
    for (unit, width, class) in &n.windows {
        let _ = writeln!(
            out,
            "    ({}, {width}, {}),",
            rust_str(unit),
            rust_str(class)
        );
    }
    out.push_str("];\n\n/// (window class, context, preference or `-`, layout)\npub const LAYOUTS: &[(&str, &str, &str, &str)] = &[\n");
    for row in &n.layouts {
        let row: Vec<String> = row.iter().map(|v| rust_str(v)).collect();
        let _ = writeln!(out, "    ({}),", row.join(", "));
    }
    out.push_str("];\n\n/// (layout shown, preference the toggle stores or `-`)\npub const TOGGLES: &[(&str, &str)] = &[\n");
    for (shown, next) in &n.toggles {
        let _ = writeln!(out, "    ({}, {}),", rust_str(shown), rust_str(next));
    }
    out.push_str("];\n\n/// (case, [(operation, outcome)])\npub const OVERLAY: &[(&str, &[(&str, &str)])] = &[\n");
    for (name, steps) in &n.overlay {
        let steps: Vec<String> = steps
            .iter()
            .map(|(op, outcome)| format!("({}, {})", rust_str(op), rust_str(outcome)))
            .collect();
        let _ = writeln!(
            out,
            "    (\n        {},\n        &[{}],\n    ),",
            rust_str(name),
            steps.join(", ")
        );
    }
    out.push_str("];\n\n/// (close reason, trigger still visible, focus target)\npub const FOCUS: &[(&str, bool, &str)] = &[\n");
    for (reason, visible, to) in &n.focus {
        let _ = writeln!(
            out,
            "    ({}, {visible}, {}),",
            rust_str(reason),
            rust_str(to)
        );
    }
    out.push_str("];\n");
    out
}

fn kotlin(version: &str, n: &Navigation) -> String {
    let mut out = header("//", version);
    out.push_str("//\n// Golden vectors for VinkNavigation.kt shared with Leptos (shared/navigation.md). Strings use\n// the contract's ids; VinkNavigationTest parses them into the renderer's own types.\n\npackage vink.ui\n\n");
    let _ = writeln!(
        out,
        "object VinkNavigationVectors {{\n    const val VERSION: String = {}\n",
        kotlin_str(&n.version)
    );
    let _ = writeln!(
        out,
        "    /** (medium, expanded) lower bounds: Web CSS px of the #7 shell and Material dp. */\n    val cssPx: Pair<Int, Int> = {} to {}\n    val dp: Pair<Int, Int> = {} to {}\n",
        n.css_px.0, n.css_px.1, n.dp.0, n.dp.1
    );
    let join = |rows: Vec<String>| rows.join(",\n        ");
    let _ = writeln!(
        out,
        "    /** (unit, available width, window class) */\n    val windows: List<Triple<String, Int, String>> = listOf(\n        {},\n    )\n",
        join(
            n.windows
                .iter()
                .map(|(u, w, c)| format!("Triple({}, {w}, {})", kotlin_str(u), kotlin_str(c)))
                .collect()
        )
    );
    let _ = writeln!(
        out,
        "    /** (window class, context, preference or `-`, layout) */\n    val layouts: List<List<String>> = listOf(\n        {},\n    )\n",
        join(
            n.layouts
                .iter()
                .map(|row| {
                    let row: Vec<String> = row.iter().map(|v| kotlin_str(v)).collect();
                    format!("listOf({})", row.join(", "))
                })
                .collect()
        )
    );
    let _ = writeln!(
        out,
        "    /** (layout shown, preference the toggle stores or `-`) */\n    val toggles: List<Pair<String, String>> = listOf(\n        {},\n    )\n",
        join(
            n.toggles
                .iter()
                .map(|(s, t)| format!("{} to {}", kotlin_str(s), kotlin_str(t)))
                .collect()
        )
    );
    let _ = writeln!(
        out,
        "    /** (case, [(operation, outcome)]) */\n    val overlay: List<Pair<String, List<Pair<String, String>>>> = listOf(\n        {},\n    )\n",
        join(
            n.overlay
                .iter()
                .map(|(name, steps)| {
                    let steps: Vec<String> = steps
                        .iter()
                        .map(|(op, o)| format!("{} to {}", kotlin_str(op), kotlin_str(o)))
                        .collect();
                    format!(
                        "{} to listOf(\n            {},\n        )",
                        kotlin_str(name),
                        steps.join(",\n            ")
                    )
                })
                .collect()
        )
    );
    let _ = writeln!(
        out,
        "    /** (close reason, trigger still visible, focus target) */\n    val focus: List<Triple<String, Boolean, String>> = listOf(\n        {},\n    )\n}}",
        join(
            n.focus
                .iter()
                .map(|(r, v, t)| format!("Triple({}, {v}, {})", kotlin_str(r), kotlin_str(t)))
                .collect()
        )
    );
    out
}

/// Validates the navigation vectors and renders their two outputs.
pub fn generate(version: &str, text: &str) -> Result<Vec<(&'static str, String)>, Vec<String>> {
    let navigation = parse(text)?;
    Ok(vec![
        (RUST_NAVIGATION_OUT, rust(version, &navigation)),
        (KOTLIN_NAVIGATION_OUT, kotlin(version, &navigation)),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source() -> Value {
        let text = std::fs::read_to_string(crate::root().join(NAVIGATION)).expect(NAVIGATION);
        serde_json::from_str(&text).unwrap()
    }

    #[test]
    fn the_committed_vectors_are_valid_and_deterministic() {
        let text = source().to_string();
        let first = generate("0", &text).expect("valid vectors");
        assert_eq!(first, generate("0", &text).unwrap());
    }

    #[test]
    fn breakpoint_drift_missing_combinations_and_unknown_steps_are_rejected() {
        let mut doc = source();
        doc["breakpoints"]["cssPx"] = serde_json::json!([768, 1100]);
        let problems = generate("0", &doc.to_string()).unwrap_err();
        assert!(
            problems.iter().any(|p| p.contains("under the breakpoints")),
            "{problems:?}"
        );

        let mut doc = source();
        doc["layouts"].as_array_mut().unwrap().pop();
        let problems = generate("0", &doc.to_string()).unwrap_err();
        assert!(
            problems.iter().any(|p| p.contains("exactly once")),
            "{problems:?}"
        );

        let mut doc = source();
        doc["overlay"][0]["steps"][0][0] = "open:Profile".into();
        let problems = generate("0", &doc.to_string()).unwrap_err();
        assert!(
            problems.iter().any(|p| p.contains("[operation, outcome]")),
            "{problems:?}"
        );

        let mut doc = source();
        doc["focus"][0][0] = "swipe".into();
        let problems = generate("0", &doc.to_string()).unwrap_err();
        assert!(
            problems.iter().any(|p| p.contains("focus row")),
            "{problems:?}"
        );
    }
}
