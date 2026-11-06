//! Golden vectors for the `ToggleAction` contract (`packages/ui-system/shared/components.md`).
//!
//! `fixtures/toggle.json` states once how a confirmed value, the caller's latest-intent phase and
//! availability resolve to what the control shows and may dispatch, and how a count fact reads.
//! This module validates the vocabulary, requires every value × phase × availability exactly
//! once, rejects rows that would dispatch blindly (while busy, uncertain, disabled or unknown, or
//! the state already shown) or turn an unavailable count into a number, and renders the same
//! vectors into Rust and Kotlin so each renderer's tests compare its own implementation.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use serde_json::Value;

use crate::ui_tokens::{header, kotlin_str, rust_str};

pub const TOGGLE: &str = "packages/ui-system/shared/fixtures/toggle.json";
pub const RUST_TOGGLE_OUT: &str = "packages/ui-system/leptos/src/toggle_vectors.rs";
pub const KOTLIN_TOGGLE_OUT: &str =
    "packages/ui-system/cmp/ui/src/commonMain/kotlin/vink/ui/VinkToggleVectors.kt";

const VALUES: [&str; 3] = ["off", "on", "unknown"];
const PHASES: [&str; 6] = [
    "idle",
    "pending:on",
    "pending:off",
    "uncertain:on",
    "uncertain:off",
    "failed",
];
const AVAILABILITY: [&str; 2] = ["enabled", "disabled"];
const PRESSED: [&str; 3] = ["off", "on", "-"];
const NOTES: [&str; 7] = [
    "-",
    "pending-on",
    "pending-off",
    "uncertain",
    "failed",
    "unknown",
    "disabled",
];
const RECOVER: [&str; 2] = ["reconcile", "-"];
const COLUMNS: [&str; 8] = [
    "value",
    "phase",
    "availability",
    "pressed",
    "busy",
    "dispatch",
    "note",
    "recover",
];

struct View {
    value: String,
    phase: String,
    availability: String,
    pressed: String,
    busy: bool,
    dispatch: String,
    note: String,
    recover: String,
}

struct Toggle {
    version: String,
    views: Vec<View>,
    counts: Vec<(String, Option<String>)>,
}

fn text(row: &[Value], i: usize) -> Option<String> {
    row.get(i)?.as_str().map(str::to_string)
}

/// `hidden`, `unavailable` or `known:<n>` with `n` within a PostgreSQL `bigint`.
fn count_value(fact: &str) -> Option<Option<u64>> {
    match fact {
        "hidden" | "unavailable" => Some(None),
        _ => {
            let digits = fact.strip_prefix("known:")?;
            let n: u64 = digits.parse().ok()?;
            (n <= i64::MAX as u64 && digits == n.to_string()).then_some(Some(n))
        }
    }
}

/// The row's own safety: a press is a desired state, never a blind toggle or replay.
fn row_problems(v: &View) -> Vec<String> {
    let mut out = Vec::new();
    let expected_pressed = if v.value == "unknown" { "-" } else { &v.value };
    if v.pressed != expected_pressed {
        out.push("pressed must show the confirmed value (`-` when unknown)".to_string());
    }
    if v.dispatch != "-" {
        if v.busy || v.phase.starts_with("uncertain") || v.availability == "disabled" {
            out.push("busy, uncertain or disabled controls never dispatch".to_string());
        }
        if v.pressed == "-" || v.dispatch == v.pressed {
            out.push("dispatch is the opposite of a known confirmed value".to_string());
        }
    }
    if v.phase.starts_with("pending") != v.busy {
        out.push("busy exactly while pending".to_string());
    }
    if v.phase.starts_with("uncertain") && (v.note != "uncertain" || v.recover != "reconcile") {
        out.push("an uncertain write shows its note and offers reconcile".to_string());
    }
    out
}

fn parse(source: &str) -> Result<Toggle, Vec<String>> {
    let doc: Value = serde_json::from_str(source).map_err(|e| vec![format!("{TOGGLE}: {e}")])?;
    let mut problems = Vec::new();
    let mut bad = |what: String| problems.push(format!("{TOGGLE}: {what}"));
    if doc.get("format").and_then(Value::as_u64) != Some(1) {
        bad("`format` must be 1".into());
    }
    let version = doc
        .get("version")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    if version.is_empty()
        || !version
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        bad("`version` is a lowercase contract id".into());
    }
    let columns: Option<Vec<&str>> = doc
        .get("columns")
        .and_then(Value::as_array)
        .and_then(|c| c.iter().map(Value::as_str).collect());
    if columns.as_deref() != Some(&COLUMNS[..]) {
        bad(format!("`columns` must be {COLUMNS:?}"));
    }

    let mut views = Vec::new();
    let mut seen = BTreeSet::new();
    let rows = doc
        .get("views")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for (i, row) in rows.iter().enumerate() {
        let at = format!("views[{i}]");
        let Some(row) = row.as_array().filter(|r| r.len() == COLUMNS.len()) else {
            bad(format!("{at} has {} columns", COLUMNS.len()));
            continue;
        };
        let (Some(value), Some(phase), Some(availability), Some(pressed)) =
            (text(row, 0), text(row, 1), text(row, 2), text(row, 3))
        else {
            bad(format!("{at}: the first four columns are strings"));
            continue;
        };
        let (Some(busy), Some(dispatch), Some(note), Some(recover)) =
            (row[4].as_bool(), text(row, 5), text(row, 6), text(row, 7))
        else {
            bad(format!("{at}: busy is a boolean, the rest strings"));
            continue;
        };
        let vocabulary = [
            (VALUES.contains(&value.as_str()), "value"),
            (PHASES.contains(&phase.as_str()), "phase"),
            (
                AVAILABILITY.contains(&availability.as_str()),
                "availability",
            ),
            (PRESSED.contains(&pressed.as_str()), "pressed"),
            (PRESSED.contains(&dispatch.as_str()), "dispatch"),
            (NOTES.contains(&note.as_str()), "note"),
            (RECOVER.contains(&recover.as_str()), "recover"),
        ];
        let unknown: Vec<&str> = vocabulary
            .iter()
            .filter(|(ok, _)| !ok)
            .map(|(_, name)| *name)
            .collect();
        if !unknown.is_empty() {
            bad(format!("{at}: unknown {}", unknown.join(", ")));
            continue;
        }
        if !seen.insert((value.clone(), phase.clone(), availability.clone())) {
            bad(format!(
                "{at}: {value} × {phase} × {availability} appears twice"
            ));
        }
        let view = View {
            value,
            phase,
            availability,
            pressed,
            busy,
            dispatch,
            note,
            recover,
        };
        for problem in row_problems(&view) {
            bad(format!("{at}: {problem}"));
        }
        views.push(view);
    }
    let expected = VALUES.len() * PHASES.len() * AVAILABILITY.len();
    if seen.len() != expected {
        bad(format!(
            "views must list every value × phase × availability exactly once ({} of {expected})",
            seen.len()
        ));
    }

    let mut counts = Vec::new();
    let mut facts = BTreeSet::new();
    let rows = doc
        .get("counts")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for (i, row) in rows.iter().enumerate() {
        let at = format!("counts[{i}]");
        let Some([fact, visible]) = row.as_array().map(Vec::as_slice) else {
            bad(format!("{at} is [fact, visible]"));
            continue;
        };
        let Some(fact) = fact.as_str() else {
            bad(format!("{at}: fact is a string"));
            continue;
        };
        let visible = visible.as_str().map(str::to_string);
        match (count_value(fact), visible.as_deref()) {
            (None, _) => bad(format!(
                "{at}: `{fact}` is hidden, unavailable or known:<n ≤ i64::MAX>"
            )),
            (Some(Some(n)), Some(shown)) => {
                if shown.replace('.', "") != n.to_string() {
                    bad(format!("{at}: `{shown}` does not read as {n}"));
                }
            }
            (Some(None), shown) => {
                let ok = match fact {
                    "hidden" => shown.is_none(),
                    _ => shown
                        .is_some_and(|s| !s.is_empty() && !s.chars().any(|c| c.is_ascii_digit())),
                };
                if !ok {
                    bad(format!(
                        "{at}: hidden shows nothing, unavailable shows a mark that is not a number"
                    ));
                }
            }
            (Some(Some(_)), None) => bad(format!("{at}: a known count is shown")),
        }
        facts.insert(fact.split(':').next().unwrap_or_default().to_string());
        counts.push((fact.to_string(), visible));
    }
    for kind in ["hidden", "unavailable", "known"] {
        if !facts.contains(kind) {
            bad(format!("counts need a `{kind}` case"));
        }
    }
    if !counts.iter().any(|(f, _)| f == "known:0") {
        bad("counts need `known:0`: zero is only a known count".into());
    }

    if problems.is_empty() {
        Ok(Toggle {
            version,
            views,
            counts,
        })
    } else {
        Err(problems)
    }
}

fn rust(version: &str, t: &Toggle) -> String {
    let mut out = header("//!", version);
    out.push_str("//!\n//! Golden vectors for `crate::toggle` shared with CMP (shared/components.md). Strings use\n//! the contract's ids; the tests in `toggle.rs` parse them into the renderer's own types.\n\n");
    let _ = writeln!(out, "pub const VERSION: &str = {};\n", rust_str(&t.version));
    out.push_str("/// (value, phase, availability, pressed, busy, dispatch, note, recover)\n#[allow(clippy::type_complexity)]\npub const VIEWS: &[(&str, &str, &str, &str, bool, &str, &str, &str)] = &[\n");
    for v in &t.views {
        let _ = writeln!(
            out,
            "    ({}, {}, {}, {}, {}, {}, {}, {}),",
            rust_str(&v.value),
            rust_str(&v.phase),
            rust_str(&v.availability),
            rust_str(&v.pressed),
            v.busy,
            rust_str(&v.dispatch),
            rust_str(&v.note),
            rust_str(&v.recover)
        );
    }
    out.push_str("];\n\n/// (count fact, visible text; `None` shows nothing)\npub const COUNTS: &[(&str, Option<&str>)] = &[\n");
    for (fact, visible) in &t.counts {
        let visible = visible
            .as_deref()
            .map_or_else(|| "None".to_string(), |s| format!("Some({})", rust_str(s)));
        let _ = writeln!(out, "    ({}, {visible}),", rust_str(fact));
    }
    out.push_str("];\n");
    out
}

fn kotlin(version: &str, t: &Toggle) -> String {
    let mut out = header("//", version);
    out.push_str("//\n// Golden vectors for VinkToggle.kt shared with Leptos (shared/components.md). Strings use\n// the contract's ids; VinkToggleTest parses them into the renderer's own types.\n\npackage vink.ui\n\n");
    out.push_str("/** One resolved row: value × phase × availability → what the control shows and may dispatch. */\ndata class ToggleVector(\n    val value: String,\n    val phase: String,\n    val availability: String,\n    val pressed: String,\n    val busy: Boolean,\n    val dispatch: String,\n    val note: String,\n    val recover: String,\n)\n\n");
    let _ = writeln!(
        out,
        "object VinkToggleVectors {{\n    const val VERSION: String = {}\n",
        kotlin_str(&t.version)
    );
    let views: Vec<String> = t
        .views
        .iter()
        .map(|v| {
            format!(
                "ToggleVector({}, {}, {}, {}, {}, {}, {}, {})",
                kotlin_str(&v.value),
                kotlin_str(&v.phase),
                kotlin_str(&v.availability),
                kotlin_str(&v.pressed),
                v.busy,
                kotlin_str(&v.dispatch),
                kotlin_str(&v.note),
                kotlin_str(&v.recover)
            )
        })
        .collect();
    let _ = writeln!(
        out,
        "    /** (value, phase, availability, pressed, busy, dispatch, note, recover) */\n    val views: List<ToggleVector> = listOf(\n        {},\n    )\n",
        views.join(",\n        ")
    );
    let counts: Vec<String> = t
        .counts
        .iter()
        .map(|(fact, visible)| {
            let visible = visible
                .as_deref()
                .map_or_else(|| "null".to_string(), kotlin_str);
            format!("{} to {visible}", kotlin_str(fact))
        })
        .collect();
    let _ = writeln!(
        out,
        "    /** (count fact, visible text; null shows nothing) */\n    val counts: List<Pair<String, String?>> = listOf(\n        {},\n    )\n}}",
        counts.join(",\n        ")
    );
    out
}

/// Validates the toggle vectors and renders their two outputs.
pub fn generate(version: &str, text: &str) -> Result<Vec<(&'static str, String)>, Vec<String>> {
    let toggle = parse(text)?;
    Ok(vec![
        (RUST_TOGGLE_OUT, rust(version, &toggle)),
        (KOTLIN_TOGGLE_OUT, kotlin(version, &toggle)),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source() -> Value {
        let text = std::fs::read_to_string(crate::root().join(TOGGLE)).expect(TOGGLE);
        serde_json::from_str(&text).unwrap()
    }

    fn rejected(doc: &Value, needle: &str) {
        let problems = generate("0", &doc.to_string()).unwrap_err();
        assert!(
            problems.iter().any(|p| p.contains(needle)),
            "{needle}: {problems:?}"
        );
    }

    #[test]
    fn the_committed_vectors_are_valid_and_deterministic() {
        let text = source().to_string();
        let first = generate("0", &text).expect("valid vectors");
        assert_eq!(first, generate("0", &text).unwrap());
        assert_eq!(first.len(), 2);
    }

    #[test]
    fn blind_dispatch_missing_rows_and_zeroed_unavailable_counts_are_rejected() {
        // A pending control that would send again.
        let mut doc = source();
        let row = doc["views"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|r| r[1] == "pending:on" && r[2] == "enabled" && r[0] == "off")
            .unwrap();
        row[5] = "on".into();
        rejected(&doc, "never dispatch");

        // An uncertain write that offers a press instead of reconcile.
        let mut doc = source();
        let row = doc["views"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|r| r[1] == "uncertain:on" && r[0] == "on" && r[2] == "enabled")
            .unwrap();
        row[7] = "-".into();
        rejected(&doc, "offers reconcile");

        // A press that sends the state already confirmed.
        let mut doc = source();
        let row = doc["views"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|r| r[1] == "idle" && r[0] == "on" && r[2] == "enabled")
            .unwrap();
        row[5] = "on".into();
        rejected(&doc, "opposite of a known");

        let mut doc = source();
        doc["views"].as_array_mut().unwrap().pop();
        rejected(&doc, "exactly once");

        let mut doc = source();
        doc["counts"][1][1] = "0".into();
        rejected(&doc, "not a number");

        let mut doc = source();
        doc["counts"][2][1] = "1".into();
        rejected(&doc, "does not read as");
    }
}
