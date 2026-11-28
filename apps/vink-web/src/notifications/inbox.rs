use leptos::prelude::*;
use leptos_meta::{Meta, Title};
use vink_patterns::{
    NotificationGroup, NotificationLeading, NotificationRow, NotificationRowFacts,
};
use vink_ui::{
    ActionState, Button, ButtonVariant, ExpressiveShape, HeadingLevel, IconButton, IconName,
    Person, ShapeMark, ShapeTone, TooltipIconButton,
};

use super::contract::{Item, Target, segment};
use super::state::Load;
use super::{INBOX_PATH, Notifications, PageAccess};
use crate::ui::empty_state::{EmptyState, NotFoundContent, ReadFailure};

#[component]
pub(crate) fn InboxPage() -> impl IntoView {
    crate::pages::ResponseStatus::capture().set_no_store();
    let owner = Notifications::current();
    view! {
        <Meta name="robots" content="noindex" />
        <Show when=move || owner.page_access.get() == PageAccess::Anonymous
            fallback=move || view! { <InboxPageContent owner /> }>
            <NotFoundContent />
        </Show>
    }
}

#[component]
fn InboxPageContent(owner: Notifications) -> impl IntoView {
    view! {
        <Title text="Thông báo — Vink" />
        <section class="vk-notif-page vk-state-page" aria-labelledby="notification-page-title">
            <BackButton />
            <header class="vk-notif-page__head">
                <div class="vk-notif-header-row">
                    <h1 id="notification-page-title" class="vk-ui-heading vk-notif-title" tabindex="-1">"Thông báo"</h1>
                    <ReadAll owner />
                </div>
                {move || owner.available().then(|| owner.inbox.with(|inbox| {
                    (inbox.snapshot.is_some() && !inbox.items.is_empty()).then(|| {
                        let count = inbox.items.iter().filter(|item| !item.is_read).count();
                        let suffix = if inbox.next.is_some() && count > 0 { "+" } else { "" };
                        (inbox.next.is_none() || count > 0).then(|| view! {
                            <span class="vk-notif-unread">{format!("{count}{suffix} chưa đọc")}</span>
                        })
                    }).flatten()
                })).flatten()}
            </header>
            <InboxBody owner />
        </section>
    }
}

#[component]
pub(super) fn BackButton() -> impl IntoView {
    #[cfg(feature = "browser")]
    let navigation = crate::navigation::use_navigation_guard();
    #[cfg(feature = "browser")]
    let came_from_app = Notifications::current().arrived_from_app;
    view! {
        <IconButton label="Quay lại" icon=IconName::ArrowBack on_press=move |_| {
            #[cfg(feature = "browser")]
            super::back_or_explore(navigation, came_from_app.get_untracked());
        } />
    }
}

#[component]
pub(crate) fn InboxPopover(open: Signal<bool>, on_close: Callback<()>) -> impl IntoView {
    let owner = Notifications::current();
    view! {
        <header class="vk-notif-header-row vk-notif-popover__head">
            <h2 id="notification-popover-title" class="vk-ui-heading vk-notif-title" tabindex="-1">"Thông báo"</h2>
            <Show when=move || open.get()><ReadAll owner /></Show>
            <IconButton label="Đóng thông báo" icon=IconName::Close on_press=move |_| on_close.run(()) />
        </header>
        <Show when=move || open.get()>
            <div class="vk-notif-popover__scroll"><InboxBody owner compact=true /></div>
        </Show>
        <Show when=move || open.get() && owner.available() && owner.inbox.with(|inbox| inbox.next.is_some())>
            <footer class="vk-notif-popover__foot">
                <a class=ButtonVariant::Text.classes() href=INBOX_PATH>"Mở trang Thông báo →"</a>
            </footer>
        </Show>
    }
}

#[component]
fn ReadAll(owner: Notifications) -> impl IntoView {
    view! {
        <Show when=move || owner.available() && owner.inbox.with(|inbox| inbox.items.iter().any(|row| !row.is_read))>
            <TooltipIconButton id="notification-read-all" icon=IconName::DoneAll
                label="Đánh dấu tất cả là đã đọc"
                state=Signal::derive(move || owner.inbox.with(|inbox| {
                    if inbox.mark == Load::Loading { ActionState::pending("Đang đánh dấu…") }
                    else if [inbox.first, inbox.more].contains(&Load::Loading) {
                        ActionState::Disabled { reason: None }
                    } else { ActionState::Enabled }
                }))
                on_press=move |_| {
                    #[cfg(feature = "browser")]
                    owner.mark_all();
                } />
        </Show>
    }
}

#[component]
fn InboxBody(owner: Notifications, #[prop(optional)] compact: bool) -> impl IntoView {
    let empty_heading = if compact {
        HeadingLevel::H3
    } else {
        HeadingLevel::H2
    };
    #[cfg(feature = "browser")]
    let navigation = crate::navigation::use_navigation_guard();
    let date_group = vink_patterns::inbox_day_group();
    // A single keyed sequence keeps the focused link mounted when midnight moves it to
    // another heading. A changed acknowledged version can still replace that row.
    let entries = Memo::new(move |_| {
        owner.inbox.with(|inbox| {
            let mut entries = Vec::new();
            let mut previous = None;
            let mut headings = 0;
            for item in &inbox.items {
                let Some(facts) = row_facts(item) else {
                    continue;
                };
                let day = date_group(&item.updated_at);
                if previous != Some(day) {
                    entries.push(Entry::Heading(headings, day));
                    headings += 1;
                    previous = Some(day);
                }
                entries.push(Entry::Item(Row {
                    facts,
                    group_id: item.group_id.clone(),
                    version: item.group_version,
                }));
            }
            if headings == 1 {
                entries.remove(0);
            }
            entries
        })
    });
    let offline = owner.offline;
    let refresh = Callback::new(move |()| {
        #[cfg(feature = "browser")]
        {
            // The retry goes away while loading. Keep keyboard focus in this same surface.
            let body = document()
                .active_element()
                .and_then(|control| control.closest(".vk-notif-body").ok().flatten());
            crate::shell::focus_without_scroll(body);
            owner.load(false);
        }
    });
    view! {
        <div class="vk-notif-body vk-state-slot" tabindex="-1" aria-busy=move || (owner.page_access.get() == PageAccess::Pending
            || owner.available() && owner.inbox.with(|inbox| inbox.first == Load::Loading)).to_string()>
            <Show when=move || offline.get()>
                <p class="vk-notif-note" role="status">"Đang ngoại tuyến. Danh sách có thể đã cũ."</p>
            </Show>
            <Show when=move || !offline.get()
                && owner.available()
                && owner.count_failed.get()
                && owner.inbox.with(|inbox| inbox.snapshot.is_some() && inbox.first == Load::Idle)>
                <div class="vk-notif-note">
                    <p role="status">{move || if owner.count.get() == vink_ui::BadgeState::Unknown {
                        "Chưa tải được số thông báo mới."
                    } else {
                        "Số thông báo mới có thể chưa cập nhật."
                    }}</p>
                    <Button id="notification-count-retry" variant=ButtonVariant::Text on_press=move |_| {
                        #[cfg(feature = "browser")]
                        owner.poll_now();
                    }>"Thử lại"</Button>
                </div>
            </Show>
            {move || {
                match owner.page_access.get() {
                    PageAccess::Pending => return view! { <Loading /> }.into_any(),
                    PageAccess::SessionUnavailable => return view! {
                        <ReadFailure compact message="Chưa kiểm tra được phiên đăng nhập." retry=Callback::new(move |()| owner.session.refresh()) />
                    }.into_any(),
                    // The full page selects NotFound above this body. A closing popover
                    // never renders a full-page state.
                    PageAccess::Anonymous => return ().into_any(),
                    PageAccess::UnavailableToAccount => return view! {
                        <EmptyState compact hook="vk-notif-empty" icon=IconName::Lock message="Đăng nhập để xem thông báo của bạn.">
                            <a href="/toi" class=ButtonVariant::Tonal.classes()>"Mở Tài khoản"</a>
                        </EmptyState>
                    }.into_any(),
                    PageAccess::Available => {}
                }
                owner.inbox.with(|inbox| match (inbox.snapshot.is_some(), inbox.first, inbox.next.is_some()) {
                    (false, Load::Failed, _) => view! { <ReadFailure compact message="Chưa tải được thông báo." retry=refresh /> }.into_any(),
                    (false, _, _) => view! { <Loading /> }.into_any(),
                    (true, Load::Failed, _) => view! { <ReadFailure inline=true message="Chưa cập nhật được thông báo." retry=refresh /> }.into_any(),
                    (true, Load::Idle, false) if inbox.items.is_empty() => view! {
                        <EmptyState compact hook="vk-notif-empty" heading=empty_heading title="Chưa có thông báo."
                            mark=Box::new(|| view! { <ShapeMark shape=ExpressiveShape::Cookie
                                tone=ShapeTone::PrimaryFixed icon=IconName::NotificationsNone /> }.into_any())
                            message="Tương tác và kết quả kiểm duyệt sẽ xuất hiện ở đây." />
                    }.into_any(),
                    _ => ().into_any(),
                })
            }}
            <Show when=move || owner.available()>
                <vink_patterns::TimeList show=Signal::derive(move || entries.with(|g| !g.is_empty()))>
                    <NotificationGroup label="Thông báo">
                        <For each=move || entries.get() key=Clone::clone children=move |entry| match entry {
                            Entry::Heading(_, day) => view! { <li class="vk-notif-day" role="presentation"><h3 class="vk-notif-group__heading">{day}</h3></li> }.into_any(),
                            Entry::Item(row) => {
                                #[cfg(feature = "browser")]
                                let href = row.facts.href.clone();
                                let description_id = format!("notice-time-{}-{}", if compact {"popover"} else {"page"}, row.group_id);
                                view! { <NotificationRow facts=row.facts description_id on_activate=Callback::new(move |event: leptos::ev::MouseEvent| {
                                    #[cfg(feature = "browser")]
                                    if event.button() == 0 && !event.default_prevented() {
                                        owner.read(row.group_id.clone(), row.version);
                                        if !(event.meta_key() || event.ctrl_key() || event.shift_key() || event.alt_key()) {
                                            event.prevent_default();
                                            navigation.navigate(&href);
                                        }
                                    }
                                    #[cfg(not(feature = "browser"))]
                                    let _ = event;
                                }) /> }.into_any()
                            }
                        } />
                    </NotificationGroup>
                </vink_patterns::TimeList>
            </Show>
            <Show when=move || owner.available() && owner.inbox.with(|inbox| inbox.mark == Load::Failed)>
                <ReadFailure message="Chưa đánh dấu được thông báo." inline=true retry=Callback::new(move |()| {
                    #[cfg(feature = "browser")]
                    {
                        // Retry disappears when the request starts. Keep its focus in this inbox.
                        crate::shell::focus_without_scroll(document().get_element_by_id("notification-read-all"));
                        owner.mark_all();
                    }
                }) />
            </Show>
            <Show when=move || owner.available() && owner.inbox.with(|inbox| inbox.next.is_some())>
                <div class="vk-notif-more">
                    <Show when=move || owner.inbox.with(|inbox| inbox.more == Load::Failed)
                        fallback=move || view! { <Button id="notification-load-more" variant=ButtonVariant::Tonal
                            state=Signal::derive(move || if owner.inbox.with(|inbox| inbox.more == Load::Loading) {
                                ActionState::pending("Đang tải thêm…")
                            } else { ActionState::Enabled })
                            on_press=move |_| {
                                #[cfg(feature = "browser")]
                                owner.load(true);
                            }>"Xem thêm"</Button> }>
                        <ReadFailure message="Chưa tải thêm được." inline=true retry=Callback::new(move |()| {
                            #[cfg(feature = "browser")]
                            owner.load(true);
                        }) />
                    </Show>
                </div>
            </Show>
        </div>
    }
}

#[component]
pub(super) fn Loading() -> impl IntoView {
    view! {
        <div class="vk-notif-loading" role="status">
            <span class="vk-ui-visually-hidden">"Đang tải thông báo…"</span>
            <span></span><span></span><span></span>
        </div>
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Entry {
    Heading(usize, &'static str),
    Item(Row),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Row {
    facts: NotificationRowFacts,
    group_id: String,
    version: i64,
}

fn row_facts(item: &Item) -> Option<NotificationRowFacts> {
    use ExpressiveShape::{Burst, Cookie};
    let link = |work: &str, comment: &str| {
        format!(
            "/tac-pham/{}?nhan-xet={}#comment-{}",
            segment(work),
            segment(comment),
            segment(comment)
        )
    };
    let actors = item
        .actors_preview
        .iter()
        .map(|actor| actor.label())
        .collect::<Vec<_>>();
    let names = match actors.as_slice() {
        [a, b, ..] if item.actor_count > 3 => {
            format!("{a}, {b} và {} người khác", item.actor_count - 2)
        }
        [a, b, c, ..] => format!("{a}, {b} và {c}"),
        [a, b] => format!("{a} và {b}"),
        [a] => (*a).into(),
        [] => "Người đọc Vink".into(),
    };
    let people = |glyph| NotificationLeading::People {
        people: item
            .actors_preview
            .iter()
            .map(|actor| Person {
                key: actor.handle().map(str::to_owned),
                name: Some(actor.label().into()),
                ..Person::default()
            })
            .collect(),
        glyph,
    };
    let system = |shape, tone, icon| NotificationLeading::System { shape, tone, icon };
    let (href, title, leading, action, excerpt, revision_instant) = match &item.target {
        Target::WorkEndorsement {
            work_id,
            revision_id,
            work_title,
        } => (
            format!(
                "/tac-pham/{}?ban-tham={}#work-endorsement",
                segment(work_id),
                segment(revision_id)
            ),
            work_title.as_str(),
            people(IconName::PenFlight),
            "đã Thấm tác phẩm của bạn",
            false,
            None,
        ),
        Target::WorkCommentThread {
            work_id,
            comment_id,
            work_title,
        } => (
            link(work_id, comment_id),
            work_title.as_str(),
            people(IconName::ChatBubbleOutline),
            "đã nhận xét tác phẩm của bạn",
            true,
            None,
        ),
        Target::WorkCommentReply {
            work_id,
            comment_id,
            work_title,
        } => (
            link(work_id, comment_id),
            work_title.as_str(),
            people(IconName::Reply),
            if item.summary.reply_to_viewer == Some(true) {
                "đã phản hồi nhận xét của bạn"
            } else {
                "đã phản hồi trong chuỗi nhận xét của bạn"
            },
            true,
            None,
        ),
        Target::WorkCommentEndorsement {
            work_id,
            comment_id,
            work_title,
        } => (
            link(work_id, comment_id),
            work_title.as_str(),
            people(IconName::Heart),
            "đã Ưng nhận xét của bạn",
            false,
            None,
        ),
        Target::WorkModerationDecision {
            work_id,
            revision_id,
            decision_version,
            work_title,
            revision_created_at,
        } => {
            let (leading, action) = if item.summary.kind == "work_moderation_rejected" {
                (
                    system(Burst, ShapeTone::Tertiary, IconName::Edit),
                    "Tác phẩm của bạn bị từ chối",
                )
            } else {
                (
                    system(Cookie, ShapeTone::PrimaryFixed, IconName::Check),
                    "Tác phẩm của bạn đã được duyệt",
                )
            };
            (
                crate::pages::desk_review::decision_review_href(
                    work_id,
                    revision_id,
                    *decision_version,
                ),
                work_title.as_str(),
                leading,
                action,
                false,
                revision_created_at.clone(),
            )
        }
        Target::CommentModerationDecision {
            comment_id,
            moderation_version,
            work_title,
        } => (
            format!(
                "/thong-bao/nhan-xet/{}/{moderation_version}",
                segment(comment_id)
            ),
            work_title
                .as_deref()
                .unwrap_or("Tác phẩm không còn hiển thị"),
            system(Cookie, ShapeTone::Neutral, IconName::Block),
            if item.summary.kind == "comment_moderation_removed" {
                "Nhận xét của bạn đã bị gỡ"
            } else {
                "Nhận xét của bạn bị từ chối"
            },
            false,
            None,
        ),
        Target::Unknown => return None,
    };
    let actors = matches!(leading, NotificationLeading::People { .. }).then_some(names);
    Some(NotificationRowFacts {
        href,
        title: title.into(),
        leading,
        actors,
        action: action.into(),
        excerpt: item
            .excerpt
            .as_ref()
            .filter(|_| excerpt)
            .map(|x| (x.text.clone(), x.truncated)),
        revision: revision_instant.map(vink_patterns::NotificationTime::Instant),
        updated: vink_patterns::NotificationTime::Instant(item.updated_at.clone()),
        action_required: item.summary.action_required,
        unread: !item.is_read,
    })
}
