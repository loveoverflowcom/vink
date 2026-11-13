//! Private moderation views. LocalResource never serializes staff snapshots into SSR. Results
//! and commands are fenced by the tab's session scope; server versions decide every write.
use crate::{
    api,
    contract::{ApiError, moderation::*, publication::AuthorPublicationState},
    document::DocumentView,
};
use leptos::prelude::*;
use leptos_meta::{Meta, Title};
use leptos_router::hooks::{use_location, use_params_map};
use serde_json::json;
use vink_ui::{ActionState, Button, ButtonVariant, Heading, HeadingLevel, Link, TextArea};

#[derive(Clone, Copy)]
struct Actions {
    busy: RwSignal<bool>,
    message: RwSignal<String>,
    reload: RwSignal<u64>,
    run: Callback<(String, serde_json::Value)>,
}
impl Actions {
    fn new() -> Self {
        super::ResponseStatus::capture().set_no_store();
        let session = crate::account::Session::current();
        let location = use_location();
        let busy = RwSignal::new(false);
        let message = RwSignal::new(String::new());
        let reload = RwSignal::new(0u64);
        Effect::new(move |_| {
            session.scope.track();
            location.pathname.track();
            message.set(String::new());
            busy.set(false);
        });
        let run = Callback::new(move |(path, body): (String, serde_json::Value)| {
            if busy.get_untracked() {
                return;
            }
            let scope = session.scope.get_untracked();
            let route = location.pathname.get_untracked();
            if scope.is_none() {
                message.set("Vui lòng đăng nhập trước khi thao tác.".into());
                return;
            }
            busy.set(true);
            leptos::task::spawn_local(async move {
                let result = api::moderation_post(&path, &body).await;
                if session.scope.try_get_untracked() != Some(scope)
                    || location.pathname.try_get_untracked() != Some(route)
                {
                    return;
                }
                busy.try_set(false);
                message.try_set(match result {
                    Ok(_) => "Đã ghi nhận. Đang tải lại trạng thái.".into(),
                    Err(error) => error_text(&error).into(),
                });
                reload.try_update(|n| *n += 1);
            });
        });
        Self {
            busy,
            message,
            reload,
            run,
        }
    }
    fn state(self) -> Signal<ActionState> {
        Signal::derive(move || {
            if self.busy.get() {
                ActionState::pending("Đang ghi nhận…")
            } else {
                ActionState::Enabled
            }
        })
    }
}
fn error_text(error: &ApiError) -> &'static str {
    match error {
        ApiError::Unauthenticated => "Vui lòng đăng nhập để mở trang này.",
        ApiError::Forbidden => {
            "Bạn không có quyền xử lý nội dung này hoặc phải tự né nội dung của mình."
        }
        ApiError::NotFound => "Hồ sơ không còn mở hoặc không thuộc phạm vi bạn được xem.",
        ApiError::Network | ApiError::Unavailable => {
            "Chưa xác nhận được kết quả. Tải lại để đối chiếu trước khi thử tiếp."
        }
        ApiError::Invalid { .. } => "Kiểm tra thông tin và nhập lý do trước khi tiếp tục.",
        _ => "Trạng thái hoặc quyền xử lý đã thay đổi. Tải lại hồ sơ trước khi thao tác tiếp.",
    }
}
// A valid cookie can make these reads succeed before the tab's initial account check.
// Never mount private controls under an unknown scope: confirming that scope would remount
// the inputs and discard text/focus. Anonymous and unavailable sessions still get a clear state.
fn loading_text(session: crate::account::Session, pending: &'static str) -> &'static str {
    use crate::session::Standing;
    match session.standing.get() {
        Standing::Anonymous | Standing::NeedsPrincipal => error_text(&ApiError::Unauthenticated),
        Standing::Suspended => error_text(&ApiError::Forbidden),
        Standing::Unavailable => error_text(&ApiError::Unavailable),
        Standing::Unknown | Standing::Active(_) => pending,
    }
}
fn state_label(state: &str) -> &'static str {
    match state {
        "queued" => "Chờ nhận",
        "claimed" => "Đã có người nhận",
        "escalated" => "Chờ quản trị viên",
        "concluded" => "Đã kết luận",
        "open" => "Đã ghi nhận, chờ quản trị viên",
        "upheld" => "Giữ quyết định",
        "reversed" => "Đảo quyết định",
        _ => "Đã đóng",
    }
}
#[component]
fn Feedback(actions: Actions) -> impl IntoView {
    view! { <div class="vk-moderation__feedback"><p role="status">{move || actions.message.get()}</p><Button variant=ButtonVariant::Text on_press=move |_| actions.reload.update(|n| *n += 1) state=actions.state()>"Tải lại"</Button></div> }
}
#[component]
pub fn ModerationHome() -> impl IntoView {
    let reason = RwSignal::new(String::new());
    let actions = Actions::new();
    let session = crate::account::Session::current();
    Effect::new(move |_| {
        session.scope.track();
        reason.set(String::new());
    });
    let data = LocalResource::new(move || {
        let scope = session.scope.get();
        actions.reload.track();
        async move {
            let cases = api::moderation_get::<Vec<Case>>("/api/v1/moderation/works/queue").await;
            let appeals = api::moderation_get::<Vec<Appeal>>("/api/v1/moderation/appeals").await;
            let automation =
                api::moderation_get::<Automation>("/api/v1/moderation/automation").await;
            (scope, cases, appeals, automation)
        }
    });
    view! {
        <Title text="Kiểm duyệt — Vink"/><Meta name="robots" content="noindex"/>
        <section class="vk-moderation" id="moderation-home">
            <Heading level=HeadingLevel::H1>"Kiểm duyệt"</Heading>
            <p>"Xem đúng bản đã gửi và ghi rõ căn cứ cho từng quyết định."</p>
            <Feedback actions/>
            {move || match data.get() {
                Some((scope,cases,appeals,automation)) if scope.is_some() && scope == session.scope.get() => view! {
                    <section aria-labelledby="case-list-title"><Heading id="case-list-title" level=HeadingLevel::H2>"Ca tác phẩm"</Heading>
                    {match cases {
                        Ok(cases) if cases.is_empty() => view! { <p>"Không có ca đang chờ trong phạm vi của bạn."</p> }.into_any(),
                        Ok(cases) => view! { <ul class="vk-moderation__list">{cases.into_iter().map(|c| view! { <li><Link href=format!("/kiem-duyet/ca/{}",c.id)>{format!("{} · {}",c.title.unwrap_or_else(|| "Bản đã gửi".into()),state_label(&c.state))}</Link></li> }).collect_view()}</ul> }.into_any(),
                        Err(e) => view! { <p>{error_text(&e)}</p> }.into_any(),
                    }}</section>
                    {appeals.as_ref().err().filter(|e| !matches!(e, ApiError::Forbidden)).map(|e| view! { <p>{format!("Phản đối: {}", error_text(e))}</p> })}
                    {appeals.ok().map(|appeals| view! { <section aria-labelledby="appeal-list-title"><Heading id="appeal-list-title" level=HeadingLevel::H2>"Phản đối cần xem xét"</Heading><p>"Hồ sơ được ghi nhận tại Vink. Chưa có thông báo nào được gửi."</p>
                        {if appeals.is_empty() { view! { <p>"Không có phản đối đang mở."</p> }.into_any() } else { view! { <ul class="vk-moderation__list">{appeals.into_iter().map(|a| view! { <li><Link href=format!("/kiem-duyet/phan-doi/{}",a.id)>"Xem hồ sơ phản đối"</Link></li> }).collect_view()}</ul> }.into_any() }}
                    </section> })}
                    {automation.as_ref().err().filter(|e| !matches!(e, ApiError::Forbidden)).map(|e| view! { <p>{format!("Duyệt tự động: {}", error_text(e))}</p> })}
                    {automation.ok().map(|automation| view! { <AutomationControls automation actions reason/> })}
                }.into_any(),
                _ => view! { <p role="status">{loading_text(session, "Đang kiểm tra quyền và tải hàng chờ…")}</p> }.into_any(),
            }}
        </section>
    }
}
#[component]
fn AutomationControls(
    automation: Automation,
    actions: Actions,
    reason: RwSignal<String>,
) -> impl IntoView {
    let paused = automation.mode == "paused";
    view! { <section aria-labelledby="automation-title"><Heading id="automation-title" level=HeadingLevel::H2>"Duyệt tự động"</Heading>
        <p>{if paused { "Đang dừng. Các lượt đã chuyển sang người duyệt vẫn ở hàng chờ." } else { "Đang bật cho lượt gửi mới. Khi không có kết quả đánh giá, người duyệt xử lý." }}</p>
        <TextArea id="automation-reason" label="Lý do thay đổi" value=reason on_input=move |i: vink_ui::TextInput| reason.set(i.value)/>
        <Button id="automation-toggle" state=actions.state() on_press=move |_| actions.run.run(("/api/v1/moderation/automation/mode".into(), json!({"expected_version":automation.version,"mode":if paused {"enabled"} else {"paused"},"reason":reason.get_untracked()})))>{if paused { "Bật cho lượt gửi mới" } else { "Dừng duyệt tự động" }}</Button>
    </section> }
}
#[component]
pub fn CasePage() -> impl IntoView {
    let reason = RwSignal::new(String::new());
    let note = RwSignal::new(String::new());
    let params = use_params_map();
    let actions = Actions::new();
    let session = crate::account::Session::current();
    Effect::new(move |_| {
        session.scope.track();
        params.track();
        reason.set(String::new());
        note.set(String::new());
    });
    let data = LocalResource::new(move || {
        let id = params.read().get("id").unwrap_or_default();
        let scope = session.scope.get();
        actions.reload.track();
        async move {
            let result =
                api::moderation_get::<Case>(&format!("/api/v1/moderation/works/cases/{id}")).await;
            (scope, id, result)
        }
    });
    view! { <Title text="Ca kiểm duyệt — Vink"/><Meta name="robots" content="noindex"/>
        <section class="vk-moderation" id="moderation-case"><Link href="/kiem-duyet">"Hàng chờ kiểm duyệt"</Link><Feedback actions/>
            {move || match data.get() {
                Some((scope,id,result)) if scope.is_some() && scope==session.scope.get() && Some(id.clone())==params.read().get("id") => match result {
                    Ok(case) => view! { <CaseContent case actions reason note/> }.into_any(),
                    Err(e) => view! { <p>{error_text(&e)}</p> }.into_any(),
                }, _ => view! { <p role="status">{loading_text(session, "Đang tải ca…")}</p> }.into_any(),
            }}
        </section>
    }
}
#[component]
fn CaseContent(
    case: Case,
    actions: Actions,
    reason: RwSignal<String>,
    note: RwSignal<String>,
) -> impl IntoView {
    let base = StoredValue::new(format!("/api/v1/moderation/works/cases/{}", case.id));
    let version = case.version;
    let holds = case.claimed_by.is_some()
        && case.claimed_by == crate::account::Session::current().principal.get_untracked();
    let admin = case.viewer_is_admin;
    let queued = case.state == "queued";
    let closed = matches!(case.state.as_str(), "closed" | "concluded");
    view! { <Heading level=HeadingLevel::H1>{case.title.unwrap_or_else(|| "Bản đã gửi".into())}</Heading>
        <p>{format!("{} · Phiên bản ca {} · Quyết định {}",state_label(&case.state),version,case.decision_version)}</p>
        {case.document.map(|document| view! { <article class="vk-moderation__snapshot"><DocumentView document/></article> })}
        {(!closed).then(|| view! { <div class="vk-moderation__actions">
            {queued.then(|| view! { <Button id="case-claim" state=actions.state() on_press=move |_| actions.run.run((format!("{}/claim",base.get_value()),json!({"expected_version":version})))>"Nhận ca"</Button> })}
            {holds.then(|| view! { <Button state=actions.state() on_press=move |_| actions.run.run((format!("{}/release",base.get_value()),json!({"expected_version":version})))>"Trả ca"</Button>
            <Button state=actions.state() on_press=move |_| actions.run.run((format!("{}/escalate",base.get_value()),json!({"expected_version":version})))>"Chuyển quản trị viên"</Button> })}
        </div>
        <TextArea id="case-reason" label="Lý do quyết định gửi người viết" value=reason on_input=move |i:vink_ui::TextInput| reason.set(i.value)/>
        <div class="vk-moderation__actions">
            {holds.then(|| view! { <Button id="case-approve" variant=ButtonVariant::Filled state=actions.state() on_press=move |_| actions.run.run((format!("{}/conclude",base.get_value()),json!({"expected_version":version,"verdict":"approved","reason":reason.get_untracked()})))>"Duyệt bản này"</Button>
            <Button id="case-reject" state=actions.state() on_press=move |_| actions.run.run((format!("{}/conclude",base.get_value()),json!({"expected_version":version,"verdict":"rejected","reason":reason.get_untracked()})))>"Bác bản này"</Button> })}
            {(admin && !holds).then(|| view! { <Button state=actions.state() on_press=move |_| actions.run.run((format!("{}/takeover",base.get_value()),json!({"expected_version":version,"reason":reason.get_untracked()})))>"Quản trị viên nhận ca"</Button> })}
        </div>
        {(admin || holds).then(|| view! { <TextArea id="case-note" label="Ghi chú nội bộ" value=note on_input=move |i:vink_ui::TextInput| note.set(i.value)/>
        <Button state=actions.state() on_press=move |_| actions.run.run((format!("{}/note",base.get_value()),json!({"expected_version":version,"note":note.get_untracked()})))>"Lưu ghi chú"</Button> })} })}
        <Heading level=HeadingLevel::H2>"Ghi chú nội bộ"</Heading><ul>{case.notes.into_iter().map(|n|view! { <li>{n.note}</li> }).collect_view()}</ul>
    }
}
#[component]
pub fn AppealPage() -> impl IntoView {
    let reason = RwSignal::new(String::new());
    let params = use_params_map();
    let actions = Actions::new();
    let session = crate::account::Session::current();
    Effect::new(move |_| {
        session.scope.track();
        params.track();
        reason.set(String::new());
    });
    let data = LocalResource::new(move || {
        let id = params.read().get("id").unwrap_or_default();
        let scope = session.scope.get();
        actions.reload.track();
        async move {
            let result =
                api::moderation_get::<AppealDetail>(&format!("/api/v1/moderation/appeals/{id}"))
                    .await;
            (scope, id, result)
        }
    });
    view! { <Title text="Xem xét phản đối — Vink"/><Meta name="robots" content="noindex"/>
        <section class="vk-moderation" id="moderation-appeal"><Link href="/kiem-duyet">"Hàng chờ kiểm duyệt"</Link><Feedback actions/>
            {move || match data.get() {
                Some((scope,id,result)) if scope.is_some() && scope==session.scope.get() && Some(id.clone())==params.read().get("id") => match result {
                    Ok(detail) => view! { <AppealContent detail actions reason/> }.into_any(), Err(e) => view! { <p>{error_text(&e)}</p> }.into_any(),
                }, _ => view! { <p role="status">{loading_text(session, "Đang tải hồ sơ…")}</p> }.into_any(),
            }}
        </section>
    }
}
#[component]
fn AppealContent(
    detail: AppealDetail,
    actions: Actions,
    reason: RwSignal<String>,
) -> impl IntoView {
    let id = StoredValue::new(detail.appeal.id);
    let version = detail.appeal.decision_version;
    view! { <Heading level=HeadingLevel::H1>{detail.title}</Heading><p>"Kết luận của quản trị viên là quyết định cuối cùng. Chưa có thông báo được gửi."</p>
        <article class="vk-moderation__snapshot"><DocumentView document=detail.document/></article>
        <TextArea id="appeal-reason" label="Căn cứ kết luận" value=reason on_input=move |i:vink_ui::TextInput| reason.set(i.value)/>
        <div class="vk-moderation__actions">
            <Button id="appeal-uphold" state=actions.state() on_press=move |_| actions.run.run((format!("/api/v1/moderation/appeals/{}/conclude",id.get_value()),json!({"expected_decision_version":version,"verdict":"uphold","reason":reason.get_untracked()})))>"Giữ quyết định"</Button>
            <Button id="appeal-reverse" variant=ButtonVariant::Filled state=actions.state() on_press=move |_| actions.run.run((format!("/api/v1/moderation/appeals/{}/conclude",id.get_value()),json!({"expected_decision_version":version,"verdict":"reverse","reason":reason.get_untracked()})))>"Đảo quyết định"</Button>
        </div>
    }
}
#[component]
pub fn AuthorReviews() -> impl IntoView {
    let params = use_params_map();
    let actions = Actions::new();
    let session = crate::account::Session::current();
    let data = LocalResource::new(move || {
        let id = params.read().get("id").unwrap_or_default();
        let scope = session.scope.get();
        actions.reload.track();
        async move {
            let state =
                api::moderation_get::<AuthorPublicationState>(&api::publication_state_path(&id))
                    .await;
            let appeals =
                api::moderation_get::<Vec<Appeal>>(&format!("/api/v1/works/{id}/appeals")).await;
            (scope, id, state, appeals)
        }
    });
    view! { <Title text="Kiểm duyệt bài của tôi — Vink"/><Meta name="robots" content="noindex"/>
        <section class="vk-moderation" id="author-reviews"><Heading level=HeadingLevel::H1>"Kiểm duyệt bài của tôi"</Heading>
            <Link href=move || format!("/ban-viet/{}",params.read().get("id").unwrap_or_default())>"Về bản nháp"</Link><Feedback actions/>
            <p>"Yêu cầu được ghi nhận tại Vink để quản trị viên xem xét. Chưa có thông báo nào được gửi."</p>
            {move || match data.get() {
                Some((scope,id,Ok(state),Ok(appeals))) if scope.is_some() && scope==session.scope.get() && Some(id.clone())==params.read().get("id") => view! {
                    <ul class="vk-moderation__list">{state.revisions.into_iter().map(|r| {
                        let path=format!("/api/v1/works/{}/revisions/{}/appeals",state.work_id,r.revision_id);
                        let version=r.decision_version;
                        let already_filed=appeals.iter().any(|a|a.revision_id==r.revision_id && Some(a.decision_version)==version);
                        let can_appeal=r.can_appeal && version.is_some() && !already_filed;
                        // One wording of a decision (shared with the editor's publication region);
                        // "Đang công khai" comes only from the public pointer, never from a decision.
                        let public = state.pointer.as_ref().is_some_and(|p| p.revision_id == r.revision_id);
                        view! { <li><Heading level=HeadingLevel::H2>{r.title}</Heading><p>{super::publication::decision_label(r.moderation)}</p>
                            {public.then(|| view! { <p>"Đang công khai."</p> })}
                            {r.decision_reason.map(|r| view! { <p>{r}</p> })}
                            {can_appeal.then(|| view! { <Button id=format!("file-appeal-{}",r.revision_id) state=actions.state() on_press=move |_| actions.run.run((path.clone(),json!({"expected_decision_version":version})))>"Yêu cầu quản trị viên xem xét"</Button> })}
                        </li> }
                    }).collect_view()}</ul>
                    <Heading level=HeadingLevel::H2>"Hồ sơ đã ghi nhận"</Heading><ul>{appeals.into_iter().map(|a|view! { <li>{state_label(&a.state)}{a.resolution_reason.map(|r|view! { <p>{r}</p> })}</li> }).collect_view()}</ul>
                }.into_any(),
                Some((scope,_,Err(e),_)) | Some((scope,_,_,Err(e))) if scope.is_some() && scope==session.scope.get() => view! { <p>{error_text(&e)}</p> }.into_any(),
                _ => view! { <p role="status">{loading_text(session, "Đang tải trạng thái kiểm duyệt…")}</p> }.into_any(),
            }}
        </section>
    }
}
