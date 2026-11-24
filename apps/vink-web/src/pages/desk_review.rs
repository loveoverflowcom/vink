//! Private, on-demand review details for one Desk work.
use leptos::prelude::*;
use leptos_meta::{Meta, Title};
use leptos_router::hooks::{use_location, use_params_map};
use vink_ui::{
    ActionState, ActionSurface, Button, ButtonVariant, Dialog, DialogKind, Heading, HeadingLevel,
    IconButton, IconName,
};

use super::publication::decision_label;
use crate::account::Session;
use crate::contract::{
    ApiError,
    moderation::{Appeal, AuthorWorkDecision, AuthorWorkReview, Filed},
    publication::AuthorRevision,
};
use crate::{api, draft_list};

const CONTENT_CLASS: &str = "grid gap-vk-3 [min-inline-size:0] wrap-anywhere";
const HISTORY_CLASS: &str = "grid gap-vk-4 ps-vk-6";
#[cfg(feature = "browser")]
const ROW_ENTRY: &str = "vink-desk-review";

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ReviewTarget {
    work_id: String,
    decision: Option<(String, i64)>,
}

impl ReviewTarget {
    fn work(work_id: String) -> Self {
        Self {
            work_id,
            decision: None,
        }
    }

    pub(super) fn href(&self) -> String {
        match &self.decision {
            Some((revision, version)) => decision_review_href(&self.work_id, revision, *version),
            None => work_review_href(&self.work_id),
        }
    }
}

pub(super) fn work_review_href(work: &str) -> String {
    let mut query = url::form_urlencoded::Serializer::new(String::new());
    query.append_pair("review", work);
    format!("/ban-viet?{}", query.finish())
}

/// The future inbox uses this URL for a private work_moderation_decision target.
pub(crate) fn decision_review_href(work: &str, revision: &str, version: i64) -> String {
    let mut query = url::form_urlencoded::Serializer::new(String::new());
    query
        .append_pair("review", work)
        .append_pair("revision", revision)
        .append_pair("decision", &version.to_string());
    format!("/ban-viet?{}", query.finish())
}

fn canonical_id(value: &str) -> Option<String> {
    (value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        }))
    .then(|| value.to_ascii_lowercase())
}

fn parse_target(query: &str) -> Option<Result<ReviewTarget, ()>> {
    let (mut work, mut revision, mut decision) = (None, None, None);
    for (key, value) in url::form_urlencoded::parse(query.trim_start_matches('?').as_bytes()) {
        let field = match key.as_ref() {
            "review" => &mut work,
            "revision" => &mut revision,
            "decision" => &mut decision,
            _ => continue,
        };
        if field.replace(value).is_some() {
            return Some(Err(()));
        }
    }
    if work.is_none() && revision.is_none() && decision.is_none() {
        return None;
    }
    Some((|| {
        let work_id = work.as_deref().and_then(canonical_id).ok_or(())?;
        let decision = match (revision, decision) {
            (None, None) => None,
            (Some(revision), Some(version)) => {
                let revision = canonical_id(&revision).ok_or(())?;
                let version = version.parse::<i64>().ok().filter(|v| *v > 0).ok_or(())?;
                Some((revision, version))
            }
            _ => return Err(()),
        };
        Ok(ReviewTarget { work_id, decision })
    })())
}

pub(super) fn use_review_target() -> Memo<Option<Result<ReviewTarget, ()>>> {
    let search = use_location().search;
    Memo::new(move |_| search.with(|query| parse_target(query)))
}

#[cfg(feature = "browser")]
pub(super) fn desk_return_href(next: &str) -> Option<String> {
    if next == "/ban-viet" {
        return Some(next.into());
    }
    parse_target(next.strip_prefix("/ban-viet?")?)?
        .ok()
        .map(|target| target.href())
}

#[component]
pub fn LegacyDeskReview() -> impl IntoView {
    super::ResponseStatus::capture().set_no_store();
    let params = use_params_map();
    let href = Memo::new(move |_| work_review_href(&params.read().get("id").unwrap_or_default()));
    super::ResponseStatus::capture().redirect(&href.get_untracked());
    #[cfg(feature = "browser")]
    {
        let navigate = leptos_router::hooks::use_navigate();
        Effect::new(move |_| {
            navigate(
                &href.get(),
                leptos_router::NavigateOptions {
                    replace: true,
                    ..Default::default()
                },
            )
        });
    }
    view! {
        <Title text="Bàn viết — Vink" /><Meta name="robots" content="noindex" />
        <vink_ui::Link href=Signal::derive(move || href.get())>"Mở kiểm duyệt trong Bàn viết"</vink_ui::Link>
    }
}

pub(super) fn use_open_review() -> Callback<ReviewTarget> {
    #[cfg(feature = "browser")]
    let navigate = leptos_router::hooks::use_navigate();
    Callback::new(move |target: ReviewTarget| {
        #[cfg(feature = "browser")]
        navigate(
            &target.href(),
            leptos_router::NavigateOptions {
                scroll: false,
                state: ROW_ENTRY.into(),
                ..Default::default()
            },
        );
        #[cfg(not(feature = "browser"))]
        let _ = target;
    })
}

fn trigger_id(work_id: &str) -> String {
    format!("desk-review-trigger-{work_id}")
}

#[component]
pub(super) fn ReviewTrigger(
    work_id: String,
    title: Signal<String>,
    open: Callback<ReviewTarget>,
) -> impl IntoView {
    let target = StoredValue::new(ReviewTarget::work(work_id.clone()));
    view! {
        <div class="vk-desk-review-action">
            <Button
                id=trigger_id(&work_id)
                variant=ButtonVariant::Text
                has_popup="dialog"
                accessible_label=Signal::derive(move || format!("Xem kiểm duyệt của {}", title.get()))
                on_press=Callback::new(move |()| open.run(target.get_value()))
            >
                "Xem kiểm duyệt"
            </Button>
        </div>
    }
}

#[component]
pub(super) fn DeskReview(session: Session) -> impl IntoView {
    let target = use_review_target();
    let open = Signal::derive(move || target.with(Option::is_some));
    #[cfg(feature = "browser")]
    let closing = StoredValue::new(false);
    let close = Callback::new({
        #[cfg(feature = "browser")]
        let navigate = leptos_router::hooks::use_navigate();
        move |()| {
            #[cfg(feature = "browser")]
            {
                if closing.get_value() {
                    return;
                }
                // History traversal is asynchronous. Repeated dismissals must consume one entry.
                if row_history_entry()
                    && let Ok(history) = window().history()
                {
                    closing.set_value(true);
                    if history.back().is_ok() {
                        return;
                    }
                    closing.set_value(false);
                }
                navigate(
                    "/ban-viet",
                    leptos_router::NavigateOptions {
                        replace: true,
                        scroll: false,
                        ..Default::default()
                    },
                );
            }
        }
    });

    #[cfg(feature = "browser")]
    Effect::new(move |previous: Option<(bool, Option<String>)>| {
        let current = target.get();
        closing.set_value(false);
        if current.is_none()
            && let Some((true, trigger)) = previous
        {
            recover_desk_focus(trigger, target);
        }
        let trigger = match &current {
            Some(Ok(target)) if row_history_entry() => Some(trigger_id(&target.work_id)),
            _ => None,
        };
        (current.is_some(), trigger)
    });

    view! {
        <Dialog id="desk-review" open title="Kết quả kiểm duyệt" kind=DialogKind::Sheet focus_title=true
            on_dismiss=move |_| close.run(())
            header_action=move || view! {
                <IconButton id="desk-review-close" label="Đóng chi tiết kiểm duyệt" icon=IconName::Close
                    surface=ActionSurface::Tonal on_press=move |_| close.run(()) />
            }>
            <Show when=move || target.with(|target| matches!(target, Some(Err(()))))>
                <p role="alert">"Đường dẫn kiểm duyệt chưa hợp lệ."</p>
            </Show>
            <For each=move || target.get().and_then(Result::ok) key=Clone::clone
                children=move |target| view! { <ReviewDetails target session /> } />
        </Dialog>
    }
}

#[cfg(feature = "browser")]
fn row_history_entry() -> bool {
    window()
        .history()
        .ok()
        .and_then(|history| history.state().ok())
        .and_then(|state| state.as_string())
        .as_deref()
        == Some(ROW_ENTRY)
}

#[cfg(feature = "browser")]
fn recover_desk_focus(trigger: Option<String>, target: Memo<Option<Result<ReviewTarget, ()>>>) {
    use wasm_bindgen::JsCast;
    request_animation_frame(move || {
        if target.try_get_untracked() != Some(None)
            || window().location().pathname().as_deref() != Ok("/ban-viet")
        {
            return;
        }
        let document = document();
        if document.active_element().is_some_and(|element| {
            !matches!(element.tag_name().as_str(), "BODY" | "HTML") && element.id() != "noi-dung"
        }) {
            return;
        }
        let control = trigger
            .as_deref()
            .and_then(|id| document.get_element_by_id(id))
            .or_else(|| document.get_element_by_id("desk-title"));
        if let Some(control) = control.and_then(|el| el.dyn_into::<web_sys::HtmlElement>().ok()) {
            let options = web_sys::FocusOptions::new();
            options.set_prevent_scroll(true);
            let _ = control.focus_with_options(&options);
        }
    });
}

async fn fetch_review(
    work: &str,
    still_current: impl Fn() -> bool,
) -> Result<AuthorWorkReview, ApiError> {
    if !still_current() {
        return Err(ApiError::Unauthenticated);
    }
    let review: AuthorWorkReview = api::moderation_get(&api::author_work_review_path(work)).await?;
    if !still_current() {
        return Err(ApiError::Unauthenticated);
    }
    if review.state.work_id != work {
        return Err(ApiError::NotFound);
    }
    Ok(review)
}

#[derive(Clone, Copy)]
struct AppealActions {
    submit: Callback<(String, i64)>,
    busy: RwSignal<bool>,
    issue: RwSignal<Option<ApiError>>,
    receipts: RwSignal<Vec<Appeal>>,
}

#[component]
fn ReviewDetails(target: ReviewTarget, session: Session) -> impl IntoView {
    let target = StoredValue::new(target);
    let scope = StoredValue::new(session.scope.get_untracked());
    let generation = RwSignal::new(0u64);
    let data = LocalResource::new(move || {
        let work = target.with_value(|target| target.work_id.clone());
        let attempt = generation.get();
        let expected_scope = scope.get_value();
        async move {
            let still_current = || {
                generation.try_get_untracked() == Some(attempt)
                    && session.scope.try_get_untracked() == Some(expected_scope.clone())
            };
            let result = fetch_review(&work, still_current).await;
            (attempt, result)
        }
    });
    let feedback = RwSignal::new("");
    let busy = RwSignal::new(false);
    let issue = RwSignal::new(None);
    let receipts = RwSignal::new(Vec::<Appeal>::new());
    let submit = Callback::new(move |(revision, version): (String, i64)| {
        if busy.get_untracked()
            || issue.with_untracked(Option::is_some)
            || session.scope.get_untracked() != scope.get_value()
        {
            return;
        }
        if receipts.with_untracked(|rows| {
            rows.iter()
                .any(|a| a.revision_id == revision && a.decision_version == version)
        }) {
            return;
        }
        let work = target.with_value(|target| target.work_id.clone());
        let expected_scope = scope.get_value();
        let generation_at_start = generation.get_untracked();
        let path = api::file_appeal_path(&work, &revision);
        busy.set(true);
        feedback.set("");
        leptos::task::spawn_local(async move {
            let result = api::moderation_post(
                &path,
                &serde_json::json!({"expected_decision_version": version}),
            )
            .await
            .and_then(|body| {
                serde_json::from_value::<Filed>(body)
                    .map_err(|_| ApiError::Unexpected { status: 200 })
            });
            if session.scope.try_get_untracked() != Some(expected_scope)
                || generation.try_get_untracked() != Some(generation_at_start)
            {
                return;
            }
            match result {
                Ok(filed)
                    if filed.appeal.work_id == work
                        && filed.appeal.revision_id == revision
                        && filed.appeal.decision_version == version =>
                {
                    #[cfg(feature = "browser")]
                    let held_focus = document().active_element().is_some_and(|element| {
                        element.id() == format!("file-appeal-{revision}-{version}")
                    });
                    receipts.update(|rows| rows.push(filed.appeal));
                    feedback.set(if filed.replayed {
                        "Yêu cầu xem xét đã có hồ sơ."
                    } else {
                        "Vink đã ghi nhận yêu cầu xem xét."
                    });
                    #[cfg(feature = "browser")]
                    if held_focus {
                        let feedback = document().get_element_by_id("desk-review-feedback");
                        request_animation_frame(move || {
                            use wasm_bindgen::JsCast;
                            if generation.try_get_untracked() == Some(generation_at_start)
                                && document()
                                    .active_element()
                                    .is_some_and(|el| el.tag_name() == "BODY")
                                && let Some(feedback) = feedback
                                    .filter(|el| el.is_connected())
                                    .and_then(|el| el.dyn_into::<web_sys::HtmlElement>().ok())
                            {
                                let options = web_sys::FocusOptions::new();
                                options.set_prevent_scroll(true);
                                let _ = feedback.focus_with_options(&options);
                            }
                        });
                    }
                }
                Ok(_) => issue.set(Some(ApiError::Unexpected { status: 200 })),
                Err(error) => issue.set(Some(error)),
            }
            busy.set(false);
        });
    });

    let actions = AppealActions {
        submit,
        busy,
        issue,
        receipts,
    };
    let denied = Memo::new(move |_| issue.get().filter(access_error));
    view! {
        <div id="desk-review-content" class=CONTENT_CLASS>
            <p id="desk-review-feedback" tabindex="-1" role="status">{move || feedback.get()}</p>
            {move || issue.get().filter(|e| !access_error(e)).map(|e| {
                let message = match e {
                    ApiError::Network | ApiError::Unavailable | ApiError::Unexpected { status: 200 | 500..=599 } =>
                        "Chưa xác nhận được kết quả. Tải lại chi tiết để đối chiếu hồ sơ.",
                    _ => "Quyết định hoặc quyền phản đối đã thay đổi. Tải lại chi tiết trước khi thử tiếp.",
                };
                view! { <p role="alert">{message}</p> }
            })}
            <Suspense fallback=|| view! { <p role="status">"Đang tải chi tiết kiểm duyệt…"</p> }>
                {move || {
                    if let Some(error) = denied.get() {
                        return view! { <p role="alert">{read_error(&error)}</p> }.into_any();
                    }
                    let current = generation.get();
                    match data.get() {
                        Some((attempt, Ok(data))) if current == attempt =>
                            review_content(data, &target.get_value(), actions),
                        Some((attempt, Err(error))) if current == attempt =>
                            view! { <p role="alert">{read_error(&error)}</p> }.into_any(),
                        _ => ().into_any(),
                    }
                }}
            </Suspense>
            <Button id="desk-review-reload" variant=ButtonVariant::Text
                state=Signal::derive(move || if busy.get() { ActionState::pending("Đang ghi nhận…") } else { ActionState::Enabled })
                on_press=move |_| {
                    if !busy.get_untracked() {
                        generation.update(|n| *n += 1);
                        receipts.set(Vec::new());
                        issue.set(None);
                        feedback.set("");
                    }
                }>"Tải lại chi tiết"</Button>
        </div>
    }
}

fn access_error(error: &ApiError) -> bool {
    matches!(
        error,
        ApiError::Unauthenticated | ApiError::Forbidden | ApiError::NotFound
    )
}

fn read_error(error: &ApiError) -> &'static str {
    match error {
        ApiError::Unauthenticated => {
            "Phiên đăng nhập đã thay đổi. Hãy mở Tài khoản để kiểm tra lại."
        }
        ApiError::Forbidden | ApiError::NotFound => "Không mở được tác phẩm hoặc quyết định này.",
        _ => "Chưa tải được chi tiết kiểm duyệt. Hãy thử tải lại.",
    }
}

fn review_content(
    data: AuthorWorkReview,
    target: &ReviewTarget,
    actions: AppealActions,
) -> AnyView {
    let revisions = &data.state.revisions;
    let selected = if let Some((revision_id, version)) = &target.decision {
        let index = revisions
            .iter()
            .position(|revision| &revision.revision_id == revision_id);
        let decision = data.decisions.iter().find(|decision| {
            &decision.revision_id == revision_id && decision.decision_version == *version
        });
        match (index, decision) {
            (Some(index), Some(decision)) => Some((index, Some(decision))),
            _ => return view! { <p role="alert">"Không tìm thấy đúng phiên bản quyết định trong liên kết này."</p> }.into_any(),
        }
    } else {
        let intent = data.state.intent.as_ref().map(|item| &item.revision_id);
        let public = data.state.pointer.as_ref().map(|item| &item.revision_id);
        intent
            .and_then(|id| {
                revisions
                    .iter()
                    .position(|revision| &revision.revision_id == id)
            })
            .or_else(|| {
                public.and_then(|id| {
                    revisions
                        .iter()
                        .position(|revision| &revision.revision_id == id)
                })
            })
            .or_else(|| revisions.len().checked_sub(1))
            .map(|index| (index, None))
    };
    let Some((index, exact)) = selected else {
        return view! { <p>"Tác phẩm này chưa có bản gửi để xem."</p> }.into_any();
    };
    let selected_revision = &revisions[index];
    let main_decision = exact.or_else(|| {
        data.decisions.iter().find(|decision| {
            decision.revision_id == selected_revision.revision_id
                && Some(decision.decision_version) == selected_revision.decision_version
        })
    });

    let main = revision_review(&data, index, main_decision, actions);
    let history = revisions
        .iter()
        .enumerate()
        .rev()
        .flat_map(|(position, revision)| {
            let mut items = Vec::new();
            if position != index
                || exact.is_some_and(|decision| {
                    Some(decision.decision_version) != revision.decision_version
                })
            {
                let current = data.decisions.iter().find(|item| {
                    item.revision_id == revision.revision_id
                        && Some(item.decision_version) == revision.decision_version
                });
                items.push(revision_review(&data, position, current, actions));
            }
            items.extend(
                data.decisions
                    .iter()
                    .filter(|decision| {
                        decision.revision_id == revision.revision_id
                            && Some(decision.decision_version) != revision.decision_version
                            && !exact.is_some_and(|selected| {
                                selected.revision_id == decision.revision_id
                                    && selected.decision_version == decision.decision_version
                            })
                    })
                    .map(|decision| revision_review(&data, position, Some(decision), actions)),
            );
            items
        })
        .map(|item| view! { <li>{item}</li> })
        .collect_view();
    let public_is_other = data
        .state
        .pointer
        .as_ref()
        .is_some_and(|pointer| pointer.revision_id != selected_revision.revision_id);
    view! {
        {public_is_other.then(|| view! { <p>"Một phiên bản khác của tác phẩm vẫn đang công khai."</p> })}
        {main}
        <details>
            <summary>"Lịch sử phiên bản và quyết định"</summary>
            {if history.is_empty() {
                view! { <p>"Chưa có phiên bản hoặc quyết định khác."</p> }.into_any()
            } else {
                view! { <ol class=HISTORY_CLASS>{history}</ol> }.into_any()
            }}
        </details>
    }.into_any()
}

fn revision_review(
    data: &AuthorWorkReview,
    index: usize,
    decision: Option<&AuthorWorkDecision>,
    actions: AppealActions,
) -> AnyView {
    let revision: &AuthorRevision = &data.state.revisions[index];
    let version = decision
        .map(|decision| decision.decision_version)
        .or(revision.decision_version);
    let historical = decision.is_some_and(|decision| !decision.is_current)
        || version != revision.decision_version;
    let status = decision.map_or(revision.moderation, |decision| decision.outcome);
    let date = decision
        .map_or(&revision.created_at, |decision| &decision.decided_at)
        .clone();
    let reason = match decision {
        Some(decision) => decision.reason.clone(),
        None => revision.decision_reason.clone(),
    };
    let title = draft_list::display_title(&revision.title);
    let public = data
        .state
        .pointer
        .as_ref()
        .is_some_and(|pointer| pointer.revision_id == revision.revision_id);
    let appeal = data
        .appeals
        .iter()
        .find(|appeal| {
            appeal.revision_id == revision.revision_id && Some(appeal.decision_version) == version
        })
        .cloned();
    let appeal_target = version.map(|version| (revision.revision_id.clone(), version));
    let can_submit = revision.can_appeal
        && decision.is_none_or(|decision| decision.can_appeal)
        && !historical
        && appeal.is_none()
        && appeal_target.is_some();
    let appeal_button_id = StoredValue::new(
        appeal_target
            .as_ref()
            .map(|(id, version)| format!("file-appeal-{id}-{version}"))
            .unwrap_or_default(),
    );
    let appeal = StoredValue::new(appeal);
    let appeal_target = StoredValue::new(appeal_target);
    let display_appeal = Memo::new(move |_| {
        if let Some((revision, version)) = appeal_target.get_value() {
            actions
                .receipts
                .with(|rows| {
                    rows.iter()
                        .find(|item| {
                            item.revision_id == revision && item.decision_version == version
                        })
                        .cloned()
                })
                .or_else(|| appeal.get_value())
        } else {
            None
        }
    });

    view! {
        <article class=CONTENT_CLASS>
            <Heading level=HeadingLevel::H3>{title}</Heading>
            <p>
                {format!("Bản {} · ", index + 1)}
                {version.map(|version| format!("Quyết định {version} · "))}
                <time datetime=date.clone()>{vink_patterns::updated_at_label(&date)}</time>
            </p>
            <p>{decision_label(status)}{historical.then_some(" · Quyết định trước đây")}</p>
            {public.then(|| view! {
                <p>{if historical { "Bản này đang công khai theo quyết định hiện hành." } else { "Đang công khai." }}</p>
            })}
            {reason.filter(|reason| !reason.trim().is_empty()).map(|reason| view! { <p>"Lý do: " {reason}</p> })}
            {move || display_appeal.get().map(appeal_summary)}
            <Show when=move || can_submit && display_appeal.with(Option::is_none)>
                <Button id=appeal_button_id.get_value()
                    variant=ButtonVariant::Tonal
                    state=Signal::derive(move || {
                        if actions.busy.get() { ActionState::pending("Đang ghi nhận…") }
                        else if actions.issue.with(Option::is_some) {
                            ActionState::Disabled { reason: Some("Tải lại chi tiết để đối chiếu.".into()) }
                        } else { ActionState::Enabled }
                    })
                    on_press=move |_| {
                        if let Some(target) = appeal_target.get_value() {
                            actions.submit.run(target);
                        }
                    }>"Yêu cầu quản trị viên xem xét"</Button>
            </Show>
        </article>
    }.into_any()
}

fn appeal_summary(appeal: Appeal) -> impl IntoView {
    let status = match appeal.state.as_str() {
        "open" => "Đã ghi nhận, chờ quản trị viên",
        "upheld" => "Quản trị viên giữ quyết định",
        "reversed" => "Quản trị viên đảo quyết định",
        "closed" => "Hồ sơ đã đóng",
        _ => "Trạng thái hồ sơ chưa nhận diện được",
    };
    view! {
        <div class=CONTENT_CLASS>
            <p>{status}</p>
            {(appeal.state == "open" && appeal.notice == "not_sent").then(|| view! {
                <p>"Chưa có thông báo nào được gửi về hồ sơ này."</p>
            })}
            {appeal.resolution_reason.map(|reason| view! { <p>"Lý do kết luận: " {reason}</p> })}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORK: &str = "0192f3a1-0000-7000-8000-0000000000a1";
    const REVISION: &str = "0192f3a1-0000-7000-8000-0000000000b1";

    fn from_href(href: &str) -> Option<Result<ReviewTarget, ()>> {
        let (_, query) = href.split_once('?').unwrap();
        parse_target(query)
    }

    #[test]
    fn exact_notification_targets_round_trip_without_rounding() {
        let href = decision_review_href(WORK, REVISION, i64::MAX);
        let target = from_href(&href).unwrap().unwrap();
        assert_eq!(target.work_id, WORK);
        assert_eq!(
            target
                .decision
                .as_ref()
                .map(|(id, version)| (id.as_str(), *version)),
            Some((REVISION, i64::MAX))
        );
        assert_eq!(target.href(), href);
    }

    #[test]
    fn invalid_exact_selector_never_falls_back_to_the_works_current_decision() {
        let work = work_review_href(WORK);
        assert_eq!(from_href(&work), Some(Ok(ReviewTarget::work(WORK.into()))));
        for suffix in [
            "&decision=2".to_owned(),
            format!("&revision={REVISION}&decision=0"),
            format!("&revision={REVISION}&decision=1&decision=2"),
            format!("&review={REVISION}"),
        ] {
            assert_eq!(from_href(&format!("{work}{suffix}")), Some(Err(())));
        }
    }
}
