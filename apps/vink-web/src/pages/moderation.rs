//! Private moderation views. LocalResource never serializes staff snapshots into SSR. Results
//! and commands are fenced by the tab's session scope; server versions decide every write.
use crate::{
    api,
    contract::{ApiError, moderation::*},
    document::ReadingDocument,
};
use leptos::prelude::*;
use leptos_meta::{Meta, Title};
use leptos_router::hooks::{use_location, use_params_map};
use serde_json::json;
use vink_ui::{ActionState, Button, ButtonVariant, Heading, HeadingLevel, Link, TextArea};

const MODERATION_PAGE_CLASS: &str =
    "mx-auto grid gap-vk-6 [max-inline-size:54rem] [min-inline-size:0]";
const MODERATION_LIST_CLASS: &str = "grid gap-vk-4 ps-vk-6";
const MODERATION_LIST_ITEM_CLASS: &str = "[min-inline-size:0] wrap-anywhere";
const MODERATION_ACTIONS_CLASS: &str = "flex flex-wrap items-center gap-vk-3";
const MODERATION_SNAPSHOT_CLASS: &str =
    "vk-moderation__snapshot [min-inline-size:0] py-vk-4 wrap-anywhere";

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
    view! { <div class=MODERATION_ACTIONS_CLASS><p role="status">{move || actions.message.get()}</p><Button variant=ButtonVariant::Text on_press=move |_| actions.reload.update(|n| *n += 1) state=actions.state()>"Tải lại"</Button></div> }
}
#[component]
pub fn ModerationHome() -> impl IntoView {
    let admin_allowed = RwSignal::new(true);
    let reason = RwSignal::new(String::new());
    let actions = Actions::new();
    let session = crate::account::Session::current();
    Effect::new(move |_| {
        session.scope.track();
        admin_allowed.set(true);
        reason.set(String::new());
    });
    Effect::new(move |_| {
        if !admin_allowed.get() {
            reason.set(String::new());
        }
    });
    // Paging never re-reads automation or remounts its unsent reason field.
    let data = LocalResource::new(move || {
        let scope = session.scope.get();
        actions.reload.track();
        async move {
            (
                scope,
                api::moderation_get::<Automation>("/api/v1/moderation/automation").await,
            )
        }
    });
    view! {
        <Title text="Kiểm duyệt — Vink"/><Meta name="robots" content="noindex"/>
        <section class=MODERATION_PAGE_CLASS id="moderation-home">
            <Heading level=HeadingLevel::H1>"Kiểm duyệt"</Heading>
            <p>"Xem đúng bản đã gửi và ghi rõ căn cứ cho từng quyết định."</p>
            <QueuePanel appeals=false reload=actions.reload admin_allowed/>
            <QueuePanel appeals=true reload=actions.reload admin_allowed/>
            <Feedback actions/>
            {move || match data.get() {
                Some((scope,result)) if scope.is_some() && scope == session.scope.get() => match result {
                    Ok(automation) if admin_allowed.get() => view! { <AutomationControls automation actions reason/> }.into_any(),
                    Ok(_) => ().into_any(),
                    Err(ApiError::Forbidden) => ().into_any(),
                    Err(error) => view! { <p>{format!("Duyệt tự động: {}",error_text(&error))}</p> }.into_any(),
                },
                _ => ().into_any(),
            }}
        </section>
    }
}

#[derive(Clone, Debug)]
struct QueueRow {
    id: String,
    label: String,
    href: String,
}
#[component]
fn QueuePanel(
    appeals: bool,
    reload: RwSignal<u64>,
    admin_allowed: RwSignal<bool>,
) -> impl IntoView {
    use crate::read_lifecycle::{Intent, PageChain, ReadOwner};
    let session = crate::account::Session::current();
    let prefix = if appeals {
        "appeal-queue"
    } else {
        "case-queue"
    };
    let endpoint = if appeals {
        "/api/v1/moderation/appeals/page"
    } else {
        "/api/v1/moderation/works/queue/page"
    };
    let rows = RwSignal::new(Vec::<QueueRow>::new());
    let chain = StoredValue::new(PageChain::default());
    let authority = StoredValue::new(None::<String>);
    let reads = StoredValue::new(ReadOwner::new(None::<String>));
    let listed_scope = StoredValue::new(None::<Option<String>>);
    let busy = RwSignal::new(false);
    let appending = RwSignal::new(false);
    let ready = RwSignal::new(false);
    let more = RwSignal::new(false);
    let error = RwSignal::new(None::<ApiError>);
    let restarted = RwSignal::new(false);
    let wake = StoredValue::new(crate::read_events::WakePolicy::default());
    #[cfg(feature = "hydrate")]
    let transport = StoredValue::new(Owner::current().map(|o| o.downgrade()));
    let load = Callback::new(move |intent: Intent| {
        #[cfg(feature = "hydrate")]
        {
            if session.scope.get_untracked().is_none() {
                return;
            }
            let cursor = if intent == Intent::Append {
                chain.with_value(|c| c.next_cursor.clone())
            } else {
                None
            };
            if intent == Intent::Append && cursor.is_none() {
                return;
            }
            let Some(Some(ticket)) = reads.try_update_value(|r| r.begin(intent, cursor)) else {
                return;
            };
            let Some(owner) = transport.get_value().and_then(|o| o.upgrade()) else {
                return;
            };
            busy.set(true);
            appending.set(intent == Intent::Append);
            error.set(None);
            wake.update_value(|w| w.started());
            owner.with(|| {
                leptos::task::spawn_local_scoped_with_cancellation(async move {
                    let path = format!(
                        "{endpoint}?limit=50{}",
                        ticket
                            .cursor
                            .as_ref()
                            .map(|c| format!(
                                "&cursor={}",
                                percent_encoding::utf8_percent_encode(
                                    c,
                                    percent_encoding::NON_ALPHANUMERIC
                                )
                            ))
                            .unwrap_or_default()
                    );
                    let result: Result<QueuePage<QueueRow>, ApiError> = if appeals {
                        api::moderation_get::<QueuePage<Appeal>>(&path)
                            .await
                            .map(|p| QueuePage {
                                authority_key: p.authority_key,
                                next_cursor: p.next_cursor,
                                items: p
                                    .items
                                    .into_iter()
                                    .map(|a| QueueRow {
                                        href: format!("/kiem-duyet/phan-doi/{}", a.id),
                                        id: a.id,
                                        label: "Xem hồ sơ phản đối".into(),
                                    })
                                    .collect(),
                            })
                    } else {
                        api::moderation_get::<QueuePage<Case>>(&path)
                            .await
                            .map(|p| QueuePage {
                                authority_key: p.authority_key,
                                next_cursor: p.next_cursor,
                                items: p
                                    .items
                                    .into_iter()
                                    .map(|c| QueueRow {
                                        href: format!("/kiem-duyet/ca/{}", c.id),
                                        id: c.id,
                                        label: format!(
                                            "{} · {}",
                                            c.title.unwrap_or_else(|| "Bản đã gửi".into()),
                                            state_label(&c.state)
                                        ),
                                    })
                                    .collect(),
                            })
                    };
                    if reads.try_with_value(|r| r.accepts(&ticket)) != Some(true)
                        || session.scope.try_get_untracked() != Some(ticket.scope.clone())
                    {
                        return;
                    }
                    let old_tail = chain.with_value(|c| c.pages > 1);
                    let had_more_focus = document()
                        .active_element()
                        .is_some_and(|e| e.id() == format!("{prefix}-more"));
                    let result = result.and_then(|page| {
                        if authority.with_value(|a| {
                            a.as_ref().is_some_and(|key| key != &page.authority_key)
                        }) {
                            rows.set(Vec::new());
                            chain.set_value(PageChain::default());
                            ready.set(false);
                            more.set(false);
                            if intent == Intent::Append {
                                return Err(ApiError::Invalid {
                                    field: Some("cursor".into()),
                                });
                            }
                        }
                        authority.set_value(Some(page.authority_key.clone()));

                        if page.items.len() > 50
                            || chain.try_update_value(|c| {
                                reads.with_value(|r| {
                                    r.accept_page(
                                        &ticket,
                                        c,
                                        page.items.iter().map(|r| r.id.clone()),
                                        page.next_cursor,
                                    )
                                })
                            }) != Some(true)
                        {
                            return Err(ApiError::Invalid {
                                field: Some("cursor".into()),
                            });
                        }
                        if intent == Intent::Append {
                            rows.update(|r| r.extend(page.items));
                        } else {
                            rows.set(page.items);
                        }
                        Ok(())
                    });
                    if reads.try_update_value(|r| r.finish(&ticket)) != Some(true) {
                        return;
                    }
                    let success = result.is_ok();
                    match result {
                        Ok(()) => {
                            if appeals && !admin_allowed.get_untracked() {
                                admin_allowed.set(true);
                            }
                            ready.set(true);
                            more.set(chain.with_value(|c| c.next_cursor.is_some()));
                            restarted.set(intent != Intent::Append && old_tail);
                        }
                        Err(e) => {
                            if matches!(
                                e,
                                ApiError::Unauthenticated
                                    | ApiError::Forbidden
                                    | ApiError::NotFound
                            ) {
                                rows.set(Vec::new());
                                chain.set_value(PageChain::default());
                                ready.set(false);
                                more.set(false);
                                if appeals && admin_allowed.get_untracked() {
                                    admin_allowed.set(false);
                                }
                            }
                            error.set(Some(e));
                        }
                    }
                    busy.set(false);
                    appending.set(false);
                    if had_more_focus && !more.get_untracked() {
                        super::focus_recovery(if appeals {
                            "appeal-queue-refresh"
                        } else {
                            "case-queue-refresh"
                        });
                    }
                    wake.update_value(|w| w.settled(crate::read_events::now(), success));
                })
            });
        }
        #[cfg(not(feature = "hydrate"))]
        let _ = intent;
    });
    #[cfg(feature = "hydrate")]
    {
        Effect::new(move |_| {
            let scope = session.scope.get();
            if listed_scope.get_value() != Some(scope.clone()) {
                listed_scope.set_value(Some(scope.clone()));
                reads.update_value(|r| r.change_scope(scope.clone()));
                rows.set(Vec::new());
                chain.set_value(PageChain::default());
                busy.set(false);
                appending.set(false);
                ready.set(false);
                more.set(false);
                error.set(None);
                if scope.is_some() {
                    load.run(Intent::Initial);
                }
            }
        });
        Effect::new(move |_| {
            let n = reload.get();
            if n > 0 {
                load.run(Intent::Refresh);
            }
        });
        crate::read_events::observe(move || {
            if crate::read_events::visible()
                && wake.with_value(|w| w.due(crate::read_events::now()))
            {
                load.run(Intent::Refresh);
            }
        });
        on_cleanup(move || {
            reads.try_update_value(|r| r.dispose());
        });
    }
    #[cfg(not(feature = "hydrate"))]
    let _ = admin_allowed;
    #[cfg(not(feature = "hydrate"))]
    let _ = (
        chain,
        authority,
        reads,
        listed_scope,
        wake,
        reload,
        endpoint,
    );
    let state = Signal::derive(move || {
        if busy.get() {
            ActionState::pending("Đang tải…")
        } else {
            ActionState::Enabled
        }
    });
    let refresh_state = Signal::derive(move || {
        if busy.get() && !appending.get() {
            ActionState::pending("Đang tải…")
        } else {
            ActionState::Enabled
        }
    });
    view! {
        <section class="vk-moderation__queue grid gap-vk-4" id=prefix aria-busy=move ||busy.get().to_string()>
            <Heading id=format!("{prefix}-title") level=HeadingLevel::H2>{if appeals {"Phản đối cần xem xét"} else {"Ca tác phẩm"}}</Heading>
            <Button id=format!("{prefix}-refresh") state=refresh_state on_press=move |_|load.run(Intent::Refresh)>{if appeals {"Tải lại phản đối"} else {"Tải lại ca tác phẩm"}}</Button>
            <p role="status">{move || if busy.get() {"Đang tải hàng chờ…"} else if !ready.get() && error.get().is_none() {loading_text(session,"Đang kiểm tra quyền…")} else if restarted.get() {"Đã tải lại từ đầu. Các trang sau được bỏ để đối chiếu trạng thái mới."} else {""}}</p>
            {move || error.get().map(|e|view! {<p role="alert">{if matches!(e,ApiError::Invalid{..}) {"Vị trí hoặc phạm vi hàng chờ đã thay đổi. Bắt đầu lại để đối chiếu."} else {error_text(&e)}}</p>})}
            <ul class=MODERATION_LIST_CLASS><For each=move ||rows.get() key=|r|r.id.clone() children=move |r| {
                // The key preserves the link/focus; refreshed facts must remain reactive.
                let label = Signal::derive(move || rows.with(|current| current.iter()
                    .find(|row| row.id == r.id).map(|row| row.label.clone())
                    .unwrap_or_else(|| r.label.clone())));
                view! {<li class=MODERATION_LIST_ITEM_CLASS><Link href=r.href>{move || label.get()}</Link></li>}
            }/></ul>
            {move || (ready.get() && rows.with(|r|r.is_empty())).then(||view!{<p>"Không có hồ sơ đang chờ trong phạm vi của bạn."</p>})}
            <Show when=move ||more.get()><Button id=format!("{prefix}-more") state on_press=move |_|load.run(Intent::Append)>{if appeals {"Tải thêm phản đối"} else {"Tải thêm ca tác phẩm"}}</Button></Show>
            <Show when=move ||error.with(|e|matches!(e,Some(ApiError::Invalid{..})))><Button id=format!("{prefix}-restart") state on_press=move |_|load.run(Intent::Restart)>"Bắt đầu lại hàng chờ"</Button></Show>
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
    view! { <section class="grid gap-vk-4" aria-labelledby="automation-title"><Heading id="automation-title" level=HeadingLevel::H2>"Duyệt tự động"</Heading>
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
        <section class=MODERATION_PAGE_CLASS id="moderation-case"><Link href="/kiem-duyet">"Hàng chờ kiểm duyệt"</Link><Feedback actions/>
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
    let title = case.title.unwrap_or_else(|| "Bản đã gửi".into());
    let document = case.document.as_ref().map(ReadingDocument::new);
    let title_in_body = document
        .as_ref()
        .is_some_and(ReadingDocument::title_in_body);
    let body = document.map(|document| document.render(&title));
    view! { {(!title_in_body).then(|| view! { <Heading level=HeadingLevel::H1>{title}</Heading> })}
        <p>{format!("{} · Phiên bản ca {} · Quyết định {}",state_label(&case.state),version,case.decision_version)}</p>
        {body.map(|body| view! { <article class=MODERATION_SNAPSHOT_CLASS>{body}</article> })}
        {(!closed).then(|| view! { <div class=MODERATION_ACTIONS_CLASS>
            {queued.then(|| view! { <Button id="case-claim" state=actions.state() on_press=move |_| actions.run.run((format!("{}/claim",base.get_value()),json!({"expected_version":version})))>"Nhận ca"</Button> })}
            {holds.then(|| view! { <Button state=actions.state() on_press=move |_| actions.run.run((format!("{}/release",base.get_value()),json!({"expected_version":version})))>"Trả ca"</Button>
            <Button state=actions.state() on_press=move |_| actions.run.run((format!("{}/escalate",base.get_value()),json!({"expected_version":version})))>"Chuyển quản trị viên"</Button> })}
        </div>
        <TextArea id="case-reason" label="Lý do quyết định gửi người viết" value=reason on_input=move |i:vink_ui::TextInput| reason.set(i.value)/>
        <div class=MODERATION_ACTIONS_CLASS>
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
        <section class=MODERATION_PAGE_CLASS id="moderation-appeal"><Link href="/kiem-duyet">"Hàng chờ kiểm duyệt"</Link><Feedback actions/>
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
    let document = ReadingDocument::new(&detail.document);
    let title_in_body = document.title_in_body();
    let body = document.render(&detail.title);
    view! { {(!title_in_body).then(|| view! { <Heading level=HeadingLevel::H1>{detail.title}</Heading> })}
        <p>"Kết luận của quản trị viên là quyết định cuối cùng. Chưa có thông báo được gửi."</p>
        <article class=MODERATION_SNAPSHOT_CLASS>{body}</article>
        <TextArea id="appeal-reason" label="Căn cứ kết luận" value=reason on_input=move |i:vink_ui::TextInput| reason.set(i.value)/>
        <div class=MODERATION_ACTIONS_CLASS>
            <Button id="appeal-uphold" state=actions.state() on_press=move |_| actions.run.run((format!("/api/v1/moderation/appeals/{}/conclude",id.get_value()),json!({"expected_decision_version":version,"verdict":"uphold","reason":reason.get_untracked()})))>"Giữ quyết định"</Button>
            <Button id="appeal-reverse" variant=ButtonVariant::Filled state=actions.state() on_press=move |_| actions.run.run((format!("/api/v1/moderation/appeals/{}/conclude",id.get_value()),json!({"expected_decision_version":version,"verdict":"reverse","reason":reason.get_untracked()})))>"Đảo quyết định"</Button>
        </div>
    }
}
