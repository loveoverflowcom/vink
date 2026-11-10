//! ToggleAction and the LVE EndorsementAction composition through public APIs. The "owner" in
//! the lifecycle scenario is an in-memory sample: nothing is fetched, written or confirmed by a
//! service, and every person, work and count is [Mẫu].

use leptos::prelude::*;
use vink_patterns::{EndorsementAction, EndorsementBlock, EndorsementFacts, EndorsementKind};
use vink_ui::{
    Align, Button, ButtonVariant, CountFact, Direction, RecoveryIntent, Space, Stack, Surface,
    SurfaceTone, Text, TextTag, TextTone, ToggleAction, ToggleCopy, TogglePhase, ToggleValue,
    TypeRole,
};

use crate::components::log;

pub fn specimen(id: &str, scenario: &str) -> Option<AnyView> {
    Some(match id {
        "action.toggle-action" => toggle(scenario),
        "composition.endorsement" => endorsement(scenario),
        _ => return None,
    })
}

fn sample_copy() -> ToggleCopy {
    ToggleCopy {
        count_unit: "lượt ghi nhận".into(),
        ..ToggleCopy::new("Ghi nhận", "Bỏ ghi nhận")
    }
}

fn logged_set(what: &'static str) -> Callback<bool> {
    Callback::new(move |desired: bool| {
        log(format!(
            "{what} → muốn {} (caller giữ phase)",
            if desired { "bật" } else { "tắt" }
        ));
    })
}

fn logged_recover(what: &'static str) -> Callback<RecoveryIntent> {
    Callback::new(move |intent: RecoveryIntent| log(format!("{what} → {}", intent.id())))
}

fn toggle(scenario: &str) -> AnyView {
    if scenario == "lifecycle" {
        return view! { <Lifecycle /> }.into_any();
    }
    let (value, phase, count, blocked, copy) = match scenario {
        "selected" => (
            ToggleValue::On,
            TogglePhase::Idle,
            CountFact::Known(13),
            None,
            sample_copy(),
        ),
        "pending" => (
            ToggleValue::Off,
            TogglePhase::Pending { desired: true },
            CountFact::Known(12),
            None,
            sample_copy(),
        ),
        "uncertain" => (
            ToggleValue::Off,
            TogglePhase::Uncertain { desired: true },
            CountFact::Known(12),
            None,
            sample_copy(),
        ),
        "failed" => (
            ToggleValue::On,
            TogglePhase::Failed,
            CountFact::Known(13),
            None,
            sample_copy(),
        ),
        "unknown" => (
            ToggleValue::Unknown,
            TogglePhase::Idle,
            CountFact::Known(12),
            None,
            sample_copy(),
        ),
        "disabled" => (
            ToggleValue::Off,
            TogglePhase::Idle,
            CountFact::Known(12),
            Some("Mẫu · Lý do do caller cấp, hiển thị thành chữ.".to_string()),
            sample_copy(),
        ),
        "count-unavailable" => (
            ToggleValue::Off,
            TogglePhase::Idle,
            CountFact::Unavailable,
            None,
            sample_copy(),
        ),
        "long-label" => (
            ToggleValue::On,
            TogglePhase::Idle,
            CountFact::Known(1_234_567),
            None,
            ToggleCopy {
                count_unit: "lượt ghi nhận".into(),
                ..ToggleCopy::new(
                    "Ghi nhận giá trị văn chương của truyện ngắn nhiều chương này",
                    "Bỏ ghi nhận giá trị văn chương của truyện ngắn nhiều chương này",
                )
            },
        ),
        _ => (
            ToggleValue::Off,
            TogglePhase::Idle,
            CountFact::Known(12),
            None,
            sample_copy(),
        ),
    };
    view! {
        <Stack direction=Direction::Column gap=Space::S3 align=Align::Start>
            <ToggleAction
                id="toggle-sample"
                copy
                value
                phase
                count
                blocked
                on_set=logged_set("toggle-action")
                on_recover=logged_recover("toggle-action")
            />
            <Text role=TypeRole::Caption tone=TextTone::Muted>
                "Mẫu · Chỉ owner xác nhận mới đổi dấu chọn; specimen không gọi dịch vụ."
            </Text>
        </Stack>
    }
    .into_any()
}

/// A sample owner: the press only asks; one of the step buttons decides what the "owner" did.
#[component]
fn Lifecycle() -> impl IntoView {
    let value = RwSignal::new(ToggleValue::Off);
    let phase = RwSignal::new(TogglePhase::Idle);
    let count = RwSignal::new(12_u64);
    // What the sample owner holds; Reconcile reads it, a lost answer does not reveal it.
    let owner = RwSignal::new((false, 12_u64));
    let presses = RwSignal::new(0_u32);
    let desired = move || match phase.get_untracked() {
        TogglePhase::Pending { desired } | TogglePhase::Uncertain { desired } => Some(desired),
        _ => None,
    };
    let apply = move |selected: bool| {
        let (was, n) = owner.get_untracked();
        let n = match (was, selected) {
            (false, true) => n + 1,
            (true, false) => n - 1,
            _ => n,
        };
        owner.set((selected, n));
    };
    let pending = Signal::derive(move || matches!(phase.get(), TogglePhase::Pending { .. }));
    let step_state = move || {
        if pending.get() {
            vink_ui::ActionState::Enabled
        } else {
            vink_ui::ActionState::disabled("Bấm Ghi nhận trước.")
        }
    };
    view! {
        <Stack direction=Direction::Column gap=Space::S4 align=Align::Start>
            <ToggleAction
                id="toggle-lifecycle"
                copy=sample_copy()
                value
                phase
                count=Signal::derive(move || CountFact::Known(count.get()))
                on_set=move |next: bool| {
                    // The caller moves to Pending before returning: a second click is ignored.
                    phase.set(TogglePhase::Pending { desired: next });
                    presses.update(|n| *n += 1);
                    log(format!("toggle-action → muốn {} (lần {})", if next { "bật" } else { "tắt" }, presses.get_untracked()));
                }
                on_recover=move |intent: RecoveryIntent| {
                    let (selected, n) = owner.get_untracked();
                    value.set(ToggleValue::from_selected(selected));
                    count.set(n);
                    phase.set(TogglePhase::Idle);
                    log(format!("toggle-action → {} → owner đang {}", intent.id(), if selected { "bật" } else { "tắt" }));
                }
            />
            <Surface tone=SurfaceTone::Container padding=Space::S3>
                <Stack direction=Direction::Column gap=Space::S2>
                    <Text role=TypeRole::Caption tone=TextTone::Muted>"Mẫu · Bước của owner giả lập"</Text>
                    <Stack direction=Direction::Row gap=Space::S2 wrap=true>
                        <Button variant=ButtonVariant::Tonal state=Signal::derive(step_state) on_press=move |_| {
                            if let Some(d) = desired() {
                                apply(d);
                                let (selected, n) = owner.get_untracked();
                                value.set(ToggleValue::from_selected(selected));
                                count.set(n);
                                phase.set(TogglePhase::Idle);
                                log("owner → chấp nhận, phản hồi tới");
                            }
                        }>"Chấp nhận"</Button>
                        <Button variant=ButtonVariant::Tonal state=Signal::derive(step_state) on_press=move |_| {
                            if let Some(d) = desired() {
                                apply(d);
                                phase.set(TogglePhase::Uncertain { desired: d });
                                log("owner → chấp nhận, phản hồi bị mất");
                            }
                        }>"Mất phản hồi"</Button>
                        <Button variant=ButtonVariant::Tonal state=Signal::derive(step_state) on_press=move |_| {
                            if desired().is_some() {
                                phase.set(TogglePhase::Failed);
                                log("owner → từ chối, không đổi gì");
                            }
                        }>"Từ chối"</Button>
                    </Stack>
                    <Text tag=TextTag::Span role=TypeRole::Caption tone=TextTone::Muted>
                        {move || {
                            let (selected, n) = owner.get();
                            format!("Owner (ẩn với người dùng): {} · {n}", if selected { "bật" } else { "tắt" })
                        }}
                    </Text>
                </Stack>
            </Surface>
        </Stack>
    }
}

fn endorsement(scenario: &str) -> AnyView {
    let (work, root, replies): (EndorsementFacts, EndorsementFacts, [EndorsementFacts; 2]) =
        match scenario {
            "selected" => (
                facts(
                    ToggleValue::On,
                    TogglePhase::Idle,
                    CountFact::Known(13),
                    None,
                ),
                facts(
                    ToggleValue::On,
                    TogglePhase::Idle,
                    CountFact::Known(4),
                    None,
                ),
                [
                    facts(
                        ToggleValue::On,
                        TogglePhase::Idle,
                        CountFact::Known(1),
                        None,
                    ),
                    facts(
                        ToggleValue::Off,
                        TogglePhase::Idle,
                        CountFact::Known(0),
                        None,
                    ),
                ],
            ),
            "pending" => (
                facts(
                    ToggleValue::Off,
                    TogglePhase::Pending { desired: true },
                    CountFact::Known(12),
                    None,
                ),
                facts(
                    ToggleValue::On,
                    TogglePhase::Pending { desired: false },
                    CountFact::Known(4),
                    None,
                ),
                [
                    facts(
                        ToggleValue::Off,
                        TogglePhase::Idle,
                        CountFact::Known(1),
                        None,
                    ),
                    facts(
                        ToggleValue::Off,
                        TogglePhase::Idle,
                        CountFact::Known(0),
                        None,
                    ),
                ],
            ),
            "uncertain" => (
                facts(
                    ToggleValue::Off,
                    TogglePhase::Uncertain { desired: true },
                    CountFact::Known(12),
                    None,
                ),
                facts(
                    ToggleValue::Off,
                    TogglePhase::Idle,
                    CountFact::Known(3),
                    None,
                ),
                [
                    facts(
                        ToggleValue::On,
                        TogglePhase::Uncertain { desired: false },
                        CountFact::Known(1),
                        None,
                    ),
                    facts(
                        ToggleValue::Off,
                        TogglePhase::Idle,
                        CountFact::Known(0),
                        None,
                    ),
                ],
            ),
            "own-content" => (
                facts(
                    ToggleValue::Off,
                    TogglePhase::Idle,
                    CountFact::Known(12),
                    Some(EndorsementBlock::OwnContent),
                ),
                facts(
                    ToggleValue::Off,
                    TogglePhase::Idle,
                    CountFact::Known(3),
                    None,
                ),
                [
                    facts(
                        ToggleValue::Off,
                        TogglePhase::Idle,
                        CountFact::Known(1),
                        Some(EndorsementBlock::OwnContent),
                    ),
                    facts(
                        ToggleValue::Off,
                        TogglePhase::Idle,
                        CountFact::Known(0),
                        None,
                    ),
                ],
            ),
            "signed-out" => {
                let guest = |n| {
                    facts(
                        ToggleValue::Unknown,
                        TogglePhase::Idle,
                        CountFact::Known(n),
                        Some(EndorsementBlock::SignedOut),
                    )
                };
                (guest(12), guest(3), [guest(1), guest(0)])
            }
            "unavailable" => {
                let gone = facts(
                    ToggleValue::Unknown,
                    TogglePhase::Idle,
                    CountFact::Unavailable,
                    Some(EndorsementBlock::Unavailable),
                );
                (
                    gone,
                    facts(
                        ToggleValue::Off,
                        TogglePhase::Idle,
                        CountFact::Known(3),
                        None,
                    ),
                    [
                        gone,
                        facts(
                            ToggleValue::Off,
                            TogglePhase::Idle,
                            CountFact::Unavailable,
                            None,
                        ),
                    ],
                )
            }
            _ => (
                facts(
                    ToggleValue::Off,
                    TogglePhase::Idle,
                    CountFact::Known(12),
                    None,
                ),
                facts(
                    ToggleValue::Off,
                    TogglePhase::Idle,
                    CountFact::Known(3),
                    None,
                ),
                [
                    facts(
                        ToggleValue::On,
                        TogglePhase::Idle,
                        CountFact::Known(1),
                        None,
                    ),
                    facts(
                        ToggleValue::Off,
                        TogglePhase::Idle,
                        CountFact::Known(0),
                        None,
                    ),
                ],
            ),
        };
    view! {
        <Stack direction=Direction::Column gap=Space::S4>
            <Text role=TypeRole::Caption tone=TextTone::Muted>
                "Mẫu · Người, tác phẩm và số đếm là dữ liệu trình bày; không gọi dịch vụ, Pen Flight R2 A · Mềm (CHOICE-07)."
            </Text>
            <Surface tone=SurfaceTone::Container padding=Space::S4>
                <Stack direction=Direction::Column gap=Space::S2>
                    <Text role=TypeRole::Title tag=TextTag::Strong>"Mùa hạ cuối cùng — Mẫu"</Text>
                    <Text role=TypeRole::Caption tone=TextTone::Muted>"An Nhiên · bản công khai"</Text>
                    <EndorsementAction id="endorse-work" kind=EndorsementKind::Work facts=work
                        on_set=logged_set("endorse-work") on_recover=logged_recover("endorse-work") />
                </Stack>
            </Surface>
            <Surface tone=SurfaceTone::Low padding=Space::S4>
                <Stack direction=Direction::Column gap=Space::S3>
                    <Text tag=TextTag::Strong>"Trần Linh — nhận xét gốc (Mẫu)"</Text>
                    <Text>"Đoạn mở đầu giữ nhịp rất tốt; câu thứ ba hơi dài."</Text>
                    <EndorsementAction id="endorse-root" kind=EndorsementKind::Comment facts=root
                        on_set=logged_set("endorse-root") on_recover=logged_recover("endorse-root") />
                    <ul class="vkp-endorsement-replies" aria-label="Phản hồi theo thứ tự cũ đến mới">
                        <li>
                            <Text tag=TextTag::Strong>"An Nhiên — phản hồi (Mẫu)"</Text>
                            <Text>"Cảm ơn, mình sẽ ngắt câu đó."</Text>
                            <EndorsementAction id="endorse-reply-1" kind=EndorsementKind::Comment facts=replies[0]
                                on_set=logged_set("endorse-reply-1") on_recover=logged_recover("endorse-reply-1") />
                        </li>
                        <li>
                            <Text tag=TextTag::Strong>"Nguyễn Hoài Thương — phản hồi (Mẫu)"</Text>
                            <Text>"Mình thấy câu dài lại hợp với cảnh mưa."</Text>
                            <EndorsementAction id="endorse-reply-2" kind=EndorsementKind::Comment facts=replies[1]
                                on_set=logged_set("endorse-reply-2") on_recover=logged_recover("endorse-reply-2") />
                        </li>
                    </ul>
                </Stack>
            </Surface>
        </Stack>
    }
    .into_any()
}

const fn facts(
    value: ToggleValue,
    phase: TogglePhase,
    count: CountFact,
    block: Option<EndorsementBlock>,
) -> EndorsementFacts {
    EndorsementFacts {
        value,
        phase,
        count,
        block,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_toggle_and_endorsement_scenario_names_its_state() {
        let html = Owner::new().with(|| toggle("pending").to_html());
        assert!(
            html.contains("aria-busy=\"true\"") && html.contains("Đang gửi…"),
            "{html}"
        );
        let html = Owner::new().with(|| toggle("count-unavailable").to_html());
        assert!(
            html.contains("data-count=\"unavailable\"") && !html.contains(">0<"),
            "{html}"
        );
        let html = Owner::new().with(|| endorsement("default").to_html());
        assert_eq!(
            html.matches("vk-ui-toggle__control").count(),
            4,
            "one action per target: {html}"
        );
        assert!(html.contains(">0<"), "a known zero on a reply: {html}");
        let html = Owner::new().with(|| endorsement("signed-out").to_html());
        assert!(
            html.contains("Cần đăng nhập để đánh giá.")
                && html.contains("Cần đăng nhập để đánh dấu hữu ích."),
            "{html}"
        );
    }
}
