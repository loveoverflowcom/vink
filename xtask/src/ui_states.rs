//! Golden vectors for the UIS-03 state contract (`packages/ui-system/shared/states.md`).
//!
//! `fixtures/states.json` states the lawful region combinations, recovery intents, notice
//! placement and `LoadingGate` timing once; this module validates its vocabulary, checks the gate
//! policy against the `duration.loading-*`/`duration.slow-notice` tokens and renders the same
//! vectors into Rust and Kotlin so each renderer's tests compare its own implementation with them.

use std::fmt::Write as _;

use serde_json::Value;

use crate::ui_tokens::{header, kotlin_str, rust_str};

pub const STATES: &str = "packages/ui-system/shared/fixtures/states.json";
pub const RUST_STATES_OUT: &str = "packages/ui-system/leptos/src/state_vectors.rs";
pub const KOTLIN_STATES_OUT: &str =
    "packages/ui-system/cmp/ui/src/commonMain/kotlin/vink/ui/VinkStateVectors.kt";

const KINDS: [&str; 12] = [
    "validation",
    "conflict",
    "rate-limited",
    "timeout",
    "network",
    "unavailable",
    "uncertain",
    "unknown",
    "auth-required",
    "session-expired",
    "forbidden",
    "not-found",
];
const CONTENTS: [&str; 7] = [
    "absent",
    "empty:first-use",
    "empty:no-results",
    "present",
    "present:low",
    "present:partial",
    "present:low+partial",
];
const INTENTS: [&str; 6] = [
    "retry",
    "reconcile",
    "reauthenticate",
    "return",
    "clear-filter",
    "export",
];
const PLACEMENTS: [&str; 6] = ["field", "inline", "region", "banner", "snackbar", "dialog"];
const TONES: [&str; 5] = ["neutral", "info", "success", "warning", "error"];
const ANNOUNCES: [&str; 3] = ["off", "polite", "assertive"];
const DISMISSES: [&str; 3] = ["state", "manual", "auto"];
const NOTICE_RESULTS: [&str; 4] = ["ok", "snackbar-tone", "announce", "dismiss"];
const UNLAWFUL: [&str; 3] = ["!not-requested", "!write-problem", "!revoked"];

/// Timing the gate vectors assume, read from the token source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GatePolicy {
    pub delay: u64,
    pub min_visible: u64,
    pub slow: u64,
}

struct GateCase {
    name: String,
    events: Vec<(String, u64)>,
    checks: Vec<(u64, bool, bool, Option<u64>)>,
}

struct States {
    version: String,
    regions: Vec<(String, String, String)>,
    intents: Vec<(String, Vec<String>)>,
    placements: Vec<(String, bool, bool, String)>,
    notices: Vec<[String; 5]>,
    gate: Vec<GateCase>,
}

fn phase_ok(phase: &str) -> bool {
    matches!(phase, "idle" | "loading" | "loading:slow" | "cancelled")
        || phase
            .strip_prefix("failed:")
            .is_some_and(|k| KINDS.contains(&k))
}

fn layout_ok(layout: &str) -> bool {
    if UNLAWFUL.contains(&layout) {
        return true;
    }
    let parts: Vec<&str> = layout.split('|').collect();
    let [body, progress, notice] = parts[..] else {
        return false;
    };
    let body_ok = matches!(body, "skeleton" | "content" | "panel:cancelled")
        || body
            .strip_prefix("panel:empty:")
            .is_some_and(|e| ["first-use", "no-results"].contains(&e))
        || body
            .strip_prefix("panel:error:")
            .is_some_and(|k| KINDS.contains(&k));
    let notice_ok = matches!(notice, "-" | "slow" | "partial" | "low-content")
        || notice
            .strip_prefix("refresh:")
            .is_some_and(|k| KINDS.contains(&k));
    body_ok && matches!(progress, "+" | "-") && notice_ok
}

fn strings(value: &Value) -> Option<Vec<String>> {
    value
        .as_array()?
        .iter()
        .map(|v| v.as_str().map(str::to_string))
        .collect()
}

fn parse(text: &str, policy: GatePolicy) -> Result<States, Vec<String>> {
    let doc: Value = serde_json::from_str(text).map_err(|e| vec![format!("{STATES}: {e}")])?;
    let mut problems = Vec::new();
    let mut bad = |what: String| problems.push(format!("{STATES}: {what}"));
    let rows = |key: &str| {
        doc.get(key)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    };
    let version = doc
        .get("version")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    if doc.get("format") != Some(&Value::from(1)) || !version.starts_with("state-contract-v") {
        bad("`format` is 1 and `version` names state-contract-vN".into());
    }
    let declared = doc.get("gatePolicy").map(|p| {
        let n = |k: &str| p.get(k).and_then(Value::as_u64).unwrap_or_default();
        GatePolicy {
            delay: n("delay"),
            min_visible: n("minVisible"),
            slow: n("slow"),
        }
    });
    if declared != Some(policy) {
        bad(format!(
            "`gatePolicy` must equal the duration.loading-delay/-min-visible/slow-notice tokens {policy:?}"
        ));
    }

    let mut regions = Vec::new();
    for row in rows("regions") {
        match strings(&row).as_deref() {
            Some([c, p, l]) if CONTENTS.contains(&c.as_str()) && phase_ok(p) && layout_ok(l) => {
                regions.push((c.clone(), p.clone(), l.clone()));
            }
            _ => bad(format!(
                "region vector {row} uses an unknown content, phase or layout"
            )),
        }
    }
    let expected = CONTENTS.len() * (4 + KINDS.len());
    let mut pairs: Vec<_> = regions.iter().map(|(c, p, _)| (c, p)).collect();
    pairs.sort();
    pairs.dedup();
    if pairs.len() != regions.len() || regions.len() != expected {
        bad(format!(
            "`regions` lists every content × phase pair exactly once ({expected}), found {}",
            regions.len()
        ));
    }

    let mut intents = Vec::new();
    for row in rows("intents") {
        let kind = row.get(0).and_then(Value::as_str);
        let allowed = row.get(1).and_then(strings);
        match (kind, allowed) {
            (Some(k), Some(list))
                if KINDS.contains(&k) && list.iter().all(|i| INTENTS.contains(&i.as_str())) =>
            {
                intents.push((k.to_string(), list));
            }
            _ => bad(format!(
                "intent vector {row} uses an unknown kind or intent"
            )),
        }
    }
    if intents.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>() != KINDS {
        bad("`intents` lists every problem kind once, in contract order".into());
    }

    let mut placements = Vec::new();
    for row in rows("placements") {
        let kind = row.get(0).and_then(Value::as_str);
        let flags = (
            row.get(1).and_then(Value::as_bool),
            row.get(2).and_then(Value::as_bool),
        );
        let expect = row.get(3).and_then(Value::as_str);
        let known = expect
            .and_then(|e| e.split_once('/'))
            .is_some_and(|(p, a)| PLACEMENTS.contains(&p) && ANNOUNCES.contains(&a));
        match (kind, flags, expect) {
            (Some(k), (Some(h), Some(w)), Some(e)) if KINDS.contains(&k) && known => {
                placements.push((k.to_string(), h, w, e.to_string()));
            }
            _ => bad(format!(
                "placement vector {row} is not [kind, bool, bool, placement/announce]"
            )),
        }
    }
    if placements.len() != KINDS.len() * 4 {
        bad("`placements` covers every kind × hasData × writing".into());
    }

    let mut notices = Vec::new();
    for row in rows("notices") {
        match strings(&row) {
            Some(v)
                if v.len() == 5
                    && PLACEMENTS.contains(&v[0].as_str())
                    && TONES.contains(&v[1].as_str())
                    && ANNOUNCES.contains(&v[2].as_str())
                    && DISMISSES.contains(&v[3].as_str())
                    && NOTICE_RESULTS.contains(&v[4].as_str()) =>
            {
                notices.push([
                    v[0].clone(),
                    v[1].clone(),
                    v[2].clone(),
                    v[3].clone(),
                    v[4].clone(),
                ]);
            }
            _ => bad(format!("notice vector {row} uses an unknown value")),
        }
    }

    let mut gate = Vec::new();
    for case in rows("gate") {
        let name = case
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let events: Option<Vec<(String, u64)>> = case
            .get("events")
            .and_then(Value::as_array)
            .map(|events| {
                events
                    .iter()
                    .map(|e| {
                        let what = e
                            .get(0)?
                            .as_str()
                            .filter(|w| ["start", "finish"].contains(w))?;
                        Some((what.to_string(), e.get(1)?.as_u64()?))
                    })
                    .collect::<Option<Vec<_>>>()
            })
            .unwrap_or(None);
        let checks: Option<Vec<_>> = case
            .get("checks")
            .and_then(Value::as_array)
            .map(|checks| {
                checks
                    .iter()
                    .map(|c| {
                        let next = c.get(3)?;
                        let next = if next.is_null() {
                            None
                        } else {
                            Some(next.as_u64()?)
                        };
                        Some((
                            c.get(0)?.as_u64()?,
                            c.get(1)?.as_bool()?,
                            c.get(2)?.as_bool()?,
                            next,
                        ))
                    })
                    .collect::<Option<Vec<_>>>()
            })
            .unwrap_or(None);
        match (events, checks) {
            (Some(events), Some(checks)) if !name.is_empty() && !checks.is_empty() => {
                gate.push(GateCase {
                    name,
                    events,
                    checks,
                });
            }
            _ => bad(format!(
                "gate case {case} needs a name, [start|finish, ms] events and [ms, bool, bool, ms|null] checks"
            )),
        }
    }
    if regions.is_empty() || notices.is_empty() || gate.is_empty() {
        bad("needs `regions`, `notices` and `gate`".into());
    }
    if problems.is_empty() {
        Ok(States {
            version,
            regions,
            intents,
            placements,
            notices,
            gate,
        })
    } else {
        Err(problems)
    }
}

fn rust(version: &str, policy: GatePolicy, s: &States) -> String {
    let mut out = header("//!", version);
    out.push_str("//!\n//! Golden vectors for `crate::state` shared with CMP (shared/states.md). Strings use the\n//! contract's ids; the tests in `state.rs` parse them into the renderer's own types.\n\n");
    let _ = writeln!(out, "pub const VERSION: &str = {};\n", rust_str(&s.version));
    let _ = writeln!(
        out,
        "/// (delay, min visible, slow) in ms, equal to the duration tokens.\npub const GATE_POLICY: (u64, u64, u64) = ({}, {}, {});\n",
        policy.delay, policy.min_visible, policy.slow
    );
    out.push_str("/// (content, phase, `body|progress|notice` or `!reason`)\npub const REGIONS: &[(&str, &str, &str)] = &[\n");
    for (c, p, l) in &s.regions {
        let _ = writeln!(
            out,
            "    ({}, {}, {}),",
            rust_str(c),
            rust_str(p),
            rust_str(l)
        );
    }
    out.push_str("];\n\n/// (problem kind, allowed recovery intents)\npub const INTENTS: &[(&str, &[&str])] = &[\n");
    for (k, list) in &s.intents {
        let list: Vec<String> = list.iter().map(|i| rust_str(i)).collect();
        let _ = writeln!(out, "    ({}, &[{}]),", rust_str(k), list.join(", "));
    }
    out.push_str("];\n\n/// (problem kind, has data, writing, `placement/announce`)\npub const PLACEMENTS: &[(&str, bool, bool, &str)] = &[\n");
    for (k, h, w, e) in &s.placements {
        let _ = writeln!(out, "    ({}, {h}, {w}, {}),", rust_str(k), rust_str(e));
    }
    out.push_str("];\n\n/// (placement, tone, announce, dismiss, `ok` or the rejection)\npub const NOTICES: &[(&str, &str, &str, &str, &str)] = &[\n");
    for n in &s.notices {
        let n: Vec<String> = n.iter().map(|v| rust_str(v)).collect();
        let _ = writeln!(out, "    ({}),", n.join(", "));
    }
    out.push_str("];\n\npub struct GateCase {\n    pub name: &'static str,\n    /// (`start` | `finish`, at ms)\n    pub events: &'static [(&'static str, u64)],\n    /// (at ms, visible, slow, next change)\n    pub checks: &'static [(u64, bool, bool, Option<u64>)],\n}\n\npub const GATE: &[GateCase] = &[\n");
    for case in &s.gate {
        let events: Vec<String> = case
            .events
            .iter()
            .map(|(w, t)| format!("({}, {t})", rust_str(w)))
            .collect();
        let checks: Vec<String> = case
            .checks
            .iter()
            .map(|(t, v, sl, n)| {
                let n = n.map_or("None".into(), |n| format!("Some({n})"));
                format!("({t}, {v}, {sl}, {n})")
            })
            .collect();
        let _ = writeln!(
            out,
            "    GateCase {{\n        name: {},\n        events: &[{}],\n        checks: &[{}],\n    }},",
            rust_str(&case.name),
            events.join(", "),
            checks.join(", ")
        );
    }
    out.push_str("];\n");
    out
}

fn kotlin(version: &str, policy: GatePolicy, s: &States) -> String {
    let mut out = header("//", version);
    out.push_str("//\n// Golden vectors for VinkState.kt shared with Leptos (shared/states.md). Strings use the\n// contract's ids; VinkStateTest parses them into the renderer's own types.\n\npackage vink.ui\n\n");
    out.push_str("class GateCase(val name: String, val events: List<Pair<String, Long>>, val checks: List<GateCheck>)\n\n");
    out.push_str("data class GateCheck(val at: Long, val visible: Boolean, val slow: Boolean, val next: Long?)\n\n");
    let _ = writeln!(
        out,
        "object VinkStateVectors {{\n    const val VERSION: String = {}\n",
        kotlin_str(&s.version)
    );
    let _ = writeln!(
        out,
        "    /** (delay, min visible, slow) in ms, equal to the duration tokens. */\n    val gatePolicy: Triple<Long, Long, Long> = Triple({}L, {}L, {}L)\n",
        policy.delay, policy.min_visible, policy.slow
    );
    let join = |rows: Vec<String>| rows.join(",\n        ");
    let _ = writeln!(
        out,
        "    /** (content, phase, `body|progress|notice` or `!reason`) */\n    val regions: List<Triple<String, String, String>> = listOf(\n        {},\n    )\n",
        join(
            s.regions
                .iter()
                .map(|(c, p, l)| format!(
                    "Triple({}, {}, {})",
                    kotlin_str(c),
                    kotlin_str(p),
                    kotlin_str(l)
                ))
                .collect()
        )
    );
    let _ = writeln!(
        out,
        "    /** (problem kind, allowed recovery intents) */\n    val intents: List<Pair<String, List<String>>> = listOf(\n        {},\n    )\n",
        join(
            s.intents
                .iter()
                .map(|(k, list)| {
                    let list: Vec<String> = list.iter().map(|i| kotlin_str(i)).collect();
                    format!("{} to listOf({})", kotlin_str(k), list.join(", "))
                })
                .collect()
        )
    );
    let _ = writeln!(
        out,
        "    /** (problem kind, has data, writing, `placement/announce`) */\n    val placements: List<List<Any>> = listOf(\n        {},\n    )\n",
        join(
            s.placements
                .iter()
                .map(|(k, h, w, e)| format!(
                    "listOf({}, {h}, {w}, {})",
                    kotlin_str(k),
                    kotlin_str(e)
                ))
                .collect()
        )
    );
    let _ = writeln!(
        out,
        "    /** (placement, tone, announce, dismiss, `ok` or the rejection) */\n    val notices: List<List<String>> = listOf(\n        {},\n    )\n",
        join(
            s.notices
                .iter()
                .map(|n| {
                    let n: Vec<String> = n.iter().map(|v| kotlin_str(v)).collect();
                    format!("listOf({})", n.join(", "))
                })
                .collect()
        )
    );
    let _ = writeln!(
        out,
        "    val gate: List<GateCase> = listOf(\n        {},\n    )\n}}",
        join(s.gate.iter().map(|case| {
            let events: Vec<String> = case.events.iter().map(|(w, t)| format!("{} to {t}L", kotlin_str(w))).collect();
            let checks: Vec<String> = case.checks.iter().map(|(t, v, sl, n)| {
                let n = n.map_or("null".into(), |n| format!("{n}L"));
                format!("GateCheck({t}L, {v}, {sl}, {n})")
            }).collect();
            format!(
                "GateCase(\n            {},\n            listOf({}),\n            listOf({}),\n        )",
                kotlin_str(&case.name),
                events.join(", "),
                checks.join(", ")
            )
        }).collect())
    );
    out
}

/// Validates the state vectors and renders their two outputs.
pub fn generate(
    version: &str,
    text: &str,
    policy: GatePolicy,
) -> Result<Vec<(&'static str, String)>, Vec<String>> {
    let states = parse(text, policy)?;
    Ok(vec![
        (RUST_STATES_OUT, rust(version, policy, &states)),
        (KOTLIN_STATES_OUT, kotlin(version, policy, &states)),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    const POLICY: GatePolicy = GatePolicy {
        delay: 300,
        min_visible: 500,
        slow: 8000,
    };

    fn source() -> Value {
        let text = std::fs::read_to_string(crate::root().join(STATES)).expect(STATES);
        serde_json::from_str(&text).unwrap()
    }

    #[test]
    fn the_committed_vectors_are_valid_and_deterministic() {
        let text = source().to_string();
        let first = generate("0", &text, POLICY).expect("valid vectors");
        assert_eq!(first, generate("0", &text, POLICY).unwrap());
    }

    #[test]
    fn unknown_vocabulary_missing_pairs_and_policy_drift_are_rejected() {
        let mut doc = source();
        doc["regions"][0][1] = "failed:teapot".into();
        let problems = generate("0", &doc.to_string(), POLICY).unwrap_err();
        assert!(
            problems
                .iter()
                .any(|p| p.contains("unknown content, phase")),
            "{problems:?}"
        );

        let mut doc = source();
        doc["regions"].as_array_mut().unwrap().pop();
        let problems = generate("0", &doc.to_string(), POLICY).unwrap_err();
        assert!(
            problems.iter().any(|p| p.contains("exactly once")),
            "{problems:?}"
        );

        let mut doc = source();
        doc["intents"][2][1] = serde_json::json!(["retry", "replay"]);
        let problems = generate("0", &doc.to_string(), POLICY).unwrap_err();
        assert!(
            problems
                .iter()
                .any(|p| p.contains("unknown kind or intent")),
            "{problems:?}"
        );

        let problems = generate(
            "0",
            &source().to_string(),
            GatePolicy {
                delay: 250,
                ..POLICY
            },
        )
        .unwrap_err();
        assert!(
            problems.iter().any(|p| p.contains("gatePolicy")),
            "{problems:?}"
        );
    }
}
