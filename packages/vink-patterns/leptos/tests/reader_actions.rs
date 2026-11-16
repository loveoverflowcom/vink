//! #226 regression baseline at the composition boundary. These exercise the real pattern
//! and ToggleAction together; a future presentation prop must preserve the supplied facts.
use leptos::prelude::*;
use vink_patterns::{
    Author, DiscussionComment, DiscussionThread, DiscussionThreadFacts, EndorsementAction,
    EndorsementBlock, EndorsementFacts, EndorsementKind,
};
use vink_ui::{CountFact, RecoveryIntent, TogglePhase, ToggleValue};

fn render_endorsement(kind: EndorsementKind, facts: EndorsementFacts) -> String {
    Owner::new().with(|| {
        view! {
            <EndorsementAction id="target" kind facts on_set=|_: bool| {}
                on_recover=|_: RecoveryIntent| {} />
        }
        .to_html()
    })
}

fn opening_tag<'a>(html: &'a str, marker: &str) -> &'a str {
    let at = html.find(marker).unwrap();
    let start = html[..at].rfind('<').unwrap();
    let end = at + html[at..].find('>').unwrap();
    &html[start..=end]
}

#[test]
fn reader_actions_keep_confirmed_values_and_counts_in_every_mutation_phase() {
    for kind in [EndorsementKind::Work, EndorsementKind::Comment] {
        for (value, phase, count, pressed, disabled, recovery) in [
            (
                ToggleValue::Off,
                TogglePhase::Pending { desired: true },
                CountFact::Known(0),
                "false",
                true,
                false,
            ),
            (
                ToggleValue::On,
                TogglePhase::Pending { desired: false },
                CountFact::Known(999_999),
                "true",
                true,
                false,
            ),
            (
                ToggleValue::On,
                TogglePhase::Uncertain { desired: false },
                CountFact::Known(7),
                "true",
                true,
                true,
            ),
            (
                ToggleValue::Off,
                TogglePhase::Failed,
                CountFact::Known(0),
                "false",
                false,
                false,
            ),
            (
                ToggleValue::Unknown,
                TogglePhase::Idle,
                CountFact::Unavailable,
                "unknown",
                true,
                true,
            ),
            (
                ToggleValue::Unknown,
                TogglePhase::Idle,
                CountFact::Hidden,
                "unknown",
                true,
                true,
            ),
        ] {
            let html = render_endorsement(
                kind,
                EndorsementFacts {
                    value,
                    phase,
                    count,
                    block: None,
                },
            );
            let control = opening_tag(&html, "id=\"target\"");
            assert!(
                control.contains(&format!("data-pressed=\"{pressed}\"")),
                "{html}"
            );
            assert_eq!(
                control.contains("aria-disabled=\"true\""),
                disabled,
                "{html}"
            );
            assert_eq!(html.contains("Kiểm tra lại"), recovery, "{html}");
            assert_eq!(
                html.matches("data-icon=\"pen-flight\"").count(),
                1,
                "{html}"
            );
            assert!(
                opening_tag(&html, "data-icon=\"pen-flight\"").contains("aria-hidden=\"true\""),
                "{html}"
            );
            match count {
                CountFact::Known(999_999) => assert!(html.contains(">999.999</span>"), "{html}"),
                CountFact::Known(n) => assert!(html.contains(&format!(">{n}</span>")), "{html}"),
                CountFact::Unavailable => assert!(
                    html.contains("data-count=\"unavailable\"") && !html.contains(">0</span>"),
                    "{html}"
                ),
                CountFact::Hidden => assert!(!html.contains("target-count"), "{html}"),
            }
            // Count belongs to the one endorsement control; recovery is a sibling command.
            let control_body = html
                .split("id=\"target\"")
                .nth(1)
                .unwrap()
                .split("</button>")
                .next()
                .unwrap();
            assert!(
                !control_body.contains("<button"),
                "nested interactive count: {html}"
            );
            assert_eq!(
                html.matches("<button").count(),
                if recovery { 2 } else { 1 },
                "{html}"
            );
        }
    }
}

#[test]
fn reader_actions_explain_every_block_without_inventing_own_state_or_zero() {
    for kind in [EndorsementKind::Work, EndorsementKind::Comment] {
        for block in [
            EndorsementBlock::OwnContent,
            EndorsementBlock::SignedOut,
            EndorsementBlock::NotPermitted,
            EndorsementBlock::Unavailable,
        ] {
            let html = render_endorsement(
                kind,
                EndorsementFacts {
                    value: ToggleValue::Unknown,
                    phase: TogglePhase::Idle,
                    count: CountFact::Unavailable,
                    block: Some(block),
                },
            );
            assert!(html.contains(kind.reason(block)), "visible reason: {html}");
            assert!(
                html.contains("aria-describedby=\"target-count target-note\""),
                "{html}"
            );
            assert!(
                html.contains("data-pressed=\"unknown\"")
                    && html.contains("aria-disabled=\"true\""),
                "{html}"
            );
            assert!(
                !html.contains(">0</span>") && !html.contains("Kiểm tra lại"),
                "{html}"
            );
        }
    }
}

#[test]
fn reader_actions_render_only_the_supplied_reply_and_removal_capabilities() {
    for (can_reply, can_remove) in [(false, false), (true, false), (false, true), (true, true)] {
        let html = Owner::new().with(|| {
            let comment = |id: &str| DiscussionComment {
                id: id.into(),
                author: Author {
                    name: "Người đọc".into(),
                    key: None,
                    published_at: "2026-11-16T00:00:00Z".into(),
                },
                body: "Giữ nguyên chữ Việt và thứ tự nhà cung cấp.".into(),
                status: None,
                context: None,
                can_reply,
                can_remove,
            };
            let facts = DiscussionThreadFacts {
                root_id: "root".into(),
                root: Some(comment("root")),
                replies: vec![comment("z-first"), comment("a-second")],
            };
            view! { <DiscussionThread facts on_reply=|_: String| {} on_remove=|_: String| {} /> }
                .to_html()
        });
        for id in ["root", "z-first", "a-second"] {
            assert_eq!(
                html.contains(&format!("id=\"reply-to-{id}\"")),
                can_reply,
                "{html}"
            );
            assert_eq!(
                html.contains(&format!("id=\"remove-comment-{id}\"")),
                can_remove,
                "{html}"
            );
        }
        assert!(
            html.find("id=\"comment-z-first\"").unwrap()
                < html.find("id=\"comment-a-second\"").unwrap(),
            "provider order: {html}"
        );
        assert_eq!(
            html.matches("<ul").count(),
            1,
            "one flat reply list: {html}"
        );
        assert!(
            !html.contains("endorse-comment"),
            "no supplied endorsement target: {html}"
        );
    }
}
