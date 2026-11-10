//! Reader feature adapter. Patterns receive view facts/intents only; HTTP and request scope
//! stay here. SSR always reads anonymously, even when the diagnostic identity fixture runs.
use crate::contract::{CommentStatus, OwnPlacement};
use crate::discussion_reads::OwnState;
use std::collections::{BTreeMap, BTreeSet};

use leptos::prelude::*;
use vink_ui::{
    ActionState, Button, ButtonVariant, Heading, HeadingLevel, ListEnd, TextInput, TypeRole,
};

use super::endorsement::Endorsements;
use crate::api::{self, LoadError};
use crate::contract::{DiscussionPage, EndorsementSummaries, ReplyList};
use crate::discussion_state::{ComposerKey, Delivery, DiscussionController, Scope, Target};
use crate::endorsement_state::page_targets;
#[cfg(feature = "hydrate")]
use crate::endorsement_state::public_targets;
use crate::ui::{
    adapters,
    patterns::{DiscussionThread, FeedbackComposer, FeedbackIntent},
};

#[cfg(feature = "hydrate")]
#[derive(Clone)]
struct TransportOwner(leptos::reactive::owner::WeakOwner);

#[cfg(feature = "hydrate")]
fn spawn_transport(task: impl std::future::Future<Output = ()> + 'static) {
    // A button/composer owner may disappear on collapse or projection refresh. The feature
    // owner survives those presentation changes and cancels only when this reader ends.
    if let Some(owner) = use_context::<TransportOwner>().and_then(|owner| owner.0.upgrade()) {
        owner.with(|| leptos::task::spawn_local_scoped_with_cancellation(task));
    }
}

// Separate cancellable read owners. Replacing one drops its fetch future and deadline;
// writes stay under TransportOwner because aborting a write cannot undo a server commit.
#[cfg(feature = "hydrate")]
#[derive(Clone, Copy)]
struct PrivateTransports {
    own: StoredValue<Option<Owner>>,
    capability: StoredValue<Option<Owner>>,
}
#[cfg(feature = "hydrate")]
fn spawn_private(capability: bool, task: impl std::future::Future<Output = ()> + 'static) {
    let transports = expect_context::<PrivateTransports>();
    let slot = if capability {
        transports.capability
    } else {
        transports.own
    };
    if let Some(old) = slot.get_value() {
        old.cleanup();
    }
    if let Some(parent) = use_context::<TransportOwner>().and_then(|o| o.0.upgrade()) {
        let owner = parent.with(Owner::new);
        owner.with(|| leptos::task::spawn_local_scoped_with_cancellation(task));
        slot.set_value(Some(owner));
    }
}
#[cfg(feature = "hydrate")]
fn cancel_private() {
    let t = expect_context::<PrivateTransports>();
    for slot in [t.own, t.capability] {
        if let Some(old) = slot.get_value() {
            old.cleanup();
        }
        slot.set_value(None);
    }
}

#[derive(Clone)]
struct ViewState {
    page: DiscussionPage,
    own: OwnState,
    /// IDs of previously public rows fenced by a current private observation.
    redacted: BTreeSet<String>,
    replies: BTreeMap<String, ReplyList>,
    reply_loading: BTreeMap<String, bool>,
    reply_errors: BTreeMap<String, String>,
    loading: bool,
    #[cfg_attr(not(feature = "hydrate"), allow(dead_code))]
    refresh_queued: bool,
    error: Option<String>,
    /// A failed "load more roots" read, shown beside the control that started it.
    page_error: Option<String>,
    unavailable: bool,
    #[cfg_attr(not(feature = "hydrate"), allow(dead_code))]
    epoch: u64,
    mutation_busy: bool,
    mutation_message: Option<String>,
    mutation_needs_refresh: bool,
}

#[cfg_attr(not(any(feature = "hydrate", test)), allow(dead_code))]
impl ViewState {
    fn change_session(&mut self, scope: Scope) {
        self.own.change_scope(scope);
        // The public page is already scrubbed. Old reads are fenced below; discard even
        // ID-only private reconciliation knowledge before another principal starts.
        self.redacted.clear();
        self.epoch += 1;
        self.loading = false;
        self.refresh_queued = false;
        self.reply_loading.clear();
        self.mutation_busy = false;
        self.mutation_message = None;
        self.mutation_needs_refresh = false;
        self.page.viewer_is_work_author = false;
    }
    fn public_ids(&self) -> BTreeSet<String> {
        self.page
            .roots
            .iter()
            .flat_map(|r| {
                let root = match &r.root {
                    crate::contract::RootView::Comment { comment } => {
                        Some(comment.comment_id.clone())
                    }
                    _ => None,
                };
                root.into_iter()
                    .chain(r.preview.iter().map(|c| c.comment_id.clone()))
                    .chain(
                        self.replies
                            .get(adapters::root_id(r))
                            .into_iter()
                            .flat_map(|p| p.replies.iter().map(|c| c.comment_id.clone())),
                    )
            })
            .collect()
    }
    fn redact(&mut self) {
        let public_ids = self.public_ids();
        for c in self.own.comments.values() {
            if c.status == CommentStatus::Approved {
                self.redacted.remove(&c.comment_id);
            } else if public_ids.contains(&c.comment_id) || self.redacted.contains(&c.comment_id) {
                self.redacted.insert(c.comment_id.clone());
            }
        }
        let hidden = &self.redacted;
        let clean = |comments: &mut Vec<crate::contract::CommentDto>| {
            comments.retain(|c| !hidden.contains(&c.comment_id));
            for c in comments {
                if let Some(crate::contract::ReplyTo::Reply { comment_id, .. }) = &c.reply_to
                    && hidden.contains(comment_id)
                {
                    c.reply_to = Some(crate::contract::ReplyTo::Unavailable);
                }
            }
        };
        for r in &mut self.page.roots {
            if let crate::contract::RootView::Comment { comment } = &r.root
                && hidden.contains(&comment.comment_id)
            {
                r.root = crate::contract::RootView::Anchor {
                    comment_id: comment.comment_id.clone(),
                    created_at: comment.created_at.clone(),
                };
            }
            clean(&mut r.preview);
        }
        self.page
            .roots
            .retain(|r| !hidden.contains(adapters::root_id(r)) || r.public_replies > 0);
        for p in self.replies.values_mut() {
            clean(&mut p.replies);
        }
    }

    fn begin_mutation(&mut self) -> bool {
        if self.loading || self.mutation_busy || self.mutation_needs_refresh || self.unavailable {
            return false;
        }
        self.own.invalidate_reads();
        self.epoch += 1; // Fence every pre-mutation root/reply read, including delayed responses.
        self.reply_loading.clear();
        self.mutation_busy = true;
        self.mutation_message = None;
        true
    }

    fn accepts_read(&self, epoch: u64) -> bool {
        self.epoch == epoch && !self.mutation_busy
    }

    /// A refresh failure belongs to the whole region; a failed next-page read stays by its
    /// button so the loaded list and the retry control keep their meaning.
    fn fail_roots_read(&mut self, append: bool, error: &crate::contract::ApiError) {
        if error == &crate::contract::ApiError::NotFound {
            self.error = Some(crate::discussion_state::error_message(error).into());
            self.unavailable = true;
            self.page.roots.clear();
            self.replies.clear();
            self.own.change_scope(self.own.scope.clone());
        } else if append {
            self.page_error = Some(crate::discussion_state::read_error_message(error, true).into());
        } else {
            self.error = Some(crate::discussion_state::read_error_message(error, false).into());
        }
    }
}

fn comments_mode_message(unavailable: bool, comments_open: bool) -> &'static str {
    if unavailable {
        "Thảo luận hiện không khả dụng. Bản đang soạn được giữ lại."
    } else if comments_open {
        "Bình luận đang mở"
    } else {
        "Tác giả đã tắt bình luận cho bài viết này"
    }
}

fn reply_needs_editor(draft: &crate::discussion_state::Draft) -> bool {
    !draft.body.is_empty() || !matches!(draft.delivery, Delivery::Editing)
}

fn should_recover_reply(draft: &crate::discussion_state::Draft) -> bool {
    reply_needs_editor(draft) || draft.receipt.is_some()
}

#[component]
pub fn Discussion(work_id: String) -> impl IntoView {
    let resource = api::discussion_resource(work_id);
    let fixture = api::local_discussion_fixture_scope();
    view! {
        <section id="feedback-region" class="vk-discussion" aria-labelledby="feedback-heading">
            <Heading level=HeadingLevel::H2 role=TypeRole::Title id="feedback-heading">"Nhận xét"</Heading>
            <p class="vk-muted">"Bình luận gốc và phản hồi theo thứ tự cũ đến mới."</p>
            <Suspense fallback=|| view! { <p role="status">"Đang tải nhận xét…"</p> }>
                {move || {
                    let fixture = fixture.clone();
                    Suspend::new(async move {
                        match resource.await {
                            Ok(load) => view! { <DiscussionLoaded page=load.page.into() endorsements=load.endorsements fixture /> }.into_any(),
                            Err(_) => view! {
                                <p role="status">"Chưa tải được nhận xét. Bạn vẫn có thể đọc tác phẩm."</p>
                                // A same-URL fragment link only scrolls and never re-runs the resource.
                                <Button id="discussion-load-retry" variant=ButtonVariant::Tonal on_press=move |_| resource.refetch()>"Tải lại nhận xét"</Button>
                            }.into_any(),
                        }
                    })
                }}
            </Suspense>
            <noscript><p>"Bật JavaScript để tải thêm phản hồi và soạn nhận xét. Các nhận xét công khai ở trên được máy chủ cung cấp."</p></noscript>
        </section>
    }
}

#[component]
fn DiscussionLoaded(
    page: DiscussionPage,
    endorsements: Result<EndorsementSummaries, LoadError>,
    fixture: Option<String>,
) -> impl IntoView {
    #[cfg(feature = "hydrate")]
    if let Some(owner) = Owner::current() {
        provide_context(TransportOwner(owner.downgrade()));
        provide_context(PrivateTransports {
            own: StoredValue::new(None),
            capability: StoredValue::new(None),
        });
    }
    // LVE #73: counts of the server-rendered page before any comment action renders them.
    let lve = Endorsements::current();
    if let Some(lve) = lve {
        lve.seed(page_targets(&page), endorsements.as_ref().ok());
        // A fixture session refreshes the page after hydration, which reads own states too.
        #[cfg(feature = "hydrate")]
        if fixture.is_none() && endorsements.is_err() {
            let targets = page_targets(&page);
            spawn_transport(async move { lve.read(targets, true) });
        }
    }
    let controller = RwSignal::new(DiscussionController::new(
        Scope {
            work_id: page.work_id.clone(),
            session: fixture.clone(),
        },
        page.settings.clone(),
    ));
    controller.update_untracked(|c| c.sign_in_open = api::sign_in_open());
    #[cfg(feature = "hydrate")]
    let work_id = page.work_id.clone();
    let state = RwSignal::new(ViewState {
        own: OwnState::new(controller.get_untracked().scope),
        redacted: BTreeSet::new(),
        page,
        replies: BTreeMap::new(),
        reply_loading: BTreeMap::new(),
        reply_errors: BTreeMap::new(),
        loading: false,
        refresh_queued: false,
        error: None,
        page_error: None,
        unavailable: false,
        epoch: 0,
        mutation_busy: false,
        mutation_message: None,
        mutation_needs_refresh: false,
    });
    let refresh = Callback::new(move |()| {
        read_roots(state, controller, lve, false);
        read_private(state, controller, false);
    });
    // Viewer-private projections only arrive after hydration through the no-store API.
    #[cfg(feature = "hydrate")]
    if fixture.is_some() {
        spawn_transport(async move {
            refresh.run(());
        });
    } else {
        // The tab's session owner decides who is acting. Each change re-reads the page for
        // that viewer; typed text stays only for the same person (`text_survives`).
        let session = crate::account::Session::current();
        Effect::new(move |previous: Option<(Option<String>, Option<String>)>| {
            let (scope, principal) = (session.scope.get(), session.principal.get());
            let (before, typed_as) = previous.unwrap_or_default();
            if before == scope {
                return (scope, typed_as);
            }
            controller.update(|c| {
                if crate::session::text_survives(typed_as.as_deref(), principal.as_deref()) {
                    c.change_session(scope.clone());
                } else {
                    c.change_scope(Scope {
                        work_id: work_id.clone(),
                        session: scope.clone(),
                    });
                }
            });
            cancel_private();
            state.update(|s| s.change_session(controller.get_untracked().scope));
            // Invalidate presentation context from the earlier viewer immediately.
            state.with_untracked(|s| revalidate_targets(s, controller));
            refresh.run(());
            (scope, principal.or(typed_as))
        });
    }
    let write_state = Signal::derive(move || {
        if state.read().mutation_busy {
            ActionState::pending("Đang cập nhật…")
        } else if state.read().loading || state.read().unavailable {
            ActionState::disabled("Chờ tải lại nhận xét trước khi thao tác.")
        } else if state.read().mutation_needs_refresh {
            ActionState::disabled("Tải lại để đối chiếu trước khi thao tác tiếp.")
        } else if !controller.read().write_ready() {
            ActionState::disabled(if controller.read().sign_in_open {
                "Đăng nhập ở mục Tôi để thao tác."
            } else {
                "Vink chưa mở đăng nhập."
            })
        } else {
            ActionState::Enabled
        }
    });
    view! {
        <div id="comments-mode" class="vk-discussion-mode">
            <p role="status">{move || comments_mode_message(state.read().unavailable, controller.read().settings.comments_open)}</p>
            {fixture.is_some().then(|| view! { <p class="vk-muted">"Định danh thử trên thiết bị này · Không phải tài khoản thật."</p> })}
            <Show when=move || !state.read().unavailable && state.read().own.author>
                <Button id="discussion-mode-toggle" variant=ButtonVariant::Tonal state=write_state on_press=move |_| change_mode(state, controller, lve)>
                    {move || if controller.read().settings.comments_open { "Tắt bình luận" } else { "Mở bình luận" }}
                </Button>
            </Show>
            <Button id="discussion-refresh" variant=ButtonVariant::Text state=Signal::derive(move || if state.read().loading || state.read().mutation_busy { ActionState::pending("Đang cập nhật nhận xét…") } else { ActionState::Enabled }) on_press=move |_| refresh.run(())>"Tải lại nhận xét"</Button>
            <p role="status">{move || state.read().error.clone().unwrap_or_default()}</p>
            <p role="status">{move || state.read().mutation_message.clone().unwrap_or_default()}</p>
        </div>
        <DiscussionComposer key=ComposerKey::Root controller state refresh />
        <OwnComments state controller lve />
        <p class="vk-muted vk-discussion-tools-note">"Bạn có thể dùng emoji từ bàn phím."</p>
        <Show when=move || !state.read().unavailable>
            <Show when=move || state.read().page.roots.is_empty()>
                <p>"Chưa có nhận xét nào được hiển thị."</p>
            </Show>
            <div id="root-list">
                <For each=move || { state.read().page.roots.iter().map(|root| adapters::root_id(root).to_owned()).collect::<Vec<_>>() } key=|id| id.clone()
                    children=move |root| view! { <RootThreadView root controller state refresh write_state lve /> } />
            </div>
            <Show when=move || state.read().page.next_cursor.is_some()>
                <div id="root-pagination" class="vk-discussion-pagination">
                    <p role="status">{move || state.read().page_error.clone().unwrap_or_default()}</p>
                    <Button variant=ButtonVariant::Tonal state=Signal::derive(move || if state.read().loading { ActionState::pending("Đang tải…") } else { ActionState::Enabled }) on_press=move |_| read_roots(state, controller, lve, true)>"Xem thêm nhận xét"</Button>
                </div>
            </Show>
            <Show when=move || state.read().page.next_cursor.is_none()>
                <ListEnd label="Đã hết danh sách nhận xét được cấp." />
            </Show>
        </Show>
        <For each=move || {
                let s = state.read();
                controller.read().drafts.iter().filter_map(|(key, draft)| match key {
                    ComposerKey::Replies(root) if should_recover_reply(draft) && !s.page.roots.iter().any(|thread| adapters::root_id(thread) == root) => Some(key.clone()),
                    _ => None,
                }).collect::<Vec<_>>()
            } key=|key| key.clone() children=move |key| {
                let key = StoredValue::new(key);
                view! {
                <section class="vk-discussion-recovered" aria-label="Bản trả lời được giữ lại">
                    <p>"Nhánh này chưa có trong danh sách đã tải. Trạng thái lần gửi và chữ đang soạn vẫn được giữ tại đây."</p>
                    <Show when=move || reply_needs_editor(&controller.read().draft(&key.get_value()))
                        fallback=move || view! { <p role="status">{move || controller.read().draft(&key.get_value()).receipt.unwrap_or_default()}</p> }>
                        <DiscussionComposer key=key.get_value() controller state refresh blocked=true />
                    </Show>
                </section>
                }
            } />
    }
}

#[component]
fn OwnComments(
    state: RwSignal<ViewState>,
    controller: RwSignal<DiscussionController>,
    lve: Option<Endorsements>,
) -> impl IntoView {
    view! {
        <Show when=move || !state.read().unavailable && controller.read().scope.session.is_some()>
            <section id="own-comments" class="vk-discussion-recovered" aria-labelledby="own-comments-heading">
                <Heading level=HeadingLevel::H3 role=TypeRole::Title id="own-comments-heading">"Nhận xét của bạn"</Heading>
                <p class="vk-muted">"Những nhận xét chưa công khai và kết quả lần gửi của bạn."</p>
                <Button id="own-comments-refresh" variant=ButtonVariant::Text
                    state=Signal::derive(move || if state.read().own.loading { ActionState::pending("Đang tải nhận xét của bạn…") } else { ActionState::Enabled })
                    on_press=move |_| read_private(state, controller, false)>"Làm mới nhận xét của bạn"</Button>
                <p role="status">{move || state.read().own.error.as_ref().map(|e| crate::discussion_state::read_error_message(e, false)).unwrap_or_default()}</p>
                <p role="status">{move || state.read().own.capability_error.as_ref().map(|_| "Chưa xác nhận được quyền thay đổi bình luận. Hãy làm mới nhận xét của bạn.").unwrap_or_default()}</p>
                <Show when=move || !state.read().own.loading && state.read().own.error.is_none() && state.read().own.visible_items(&state.read().public_ids()).is_empty()>
                    <p>"Chưa có nhận xét riêng nào trong phần đã tải."</p>
                </Show>
                <For each=move || { state.read().own.visible_items(&state.read().public_ids()).into_iter().map(|c| c.comment_id).collect::<Vec<_>>() } key=|id| id.clone()
                    children=move |id| {
                        let id = StoredValue::new(id);
                        view! {
                            <article class="vk-discussion-delivery" data-own-comment=move || id.get_value()>
                                <p role="status">{move || state.read().own.comments.get(&id.get_value()).map(|c| match c.status {
                                    CommentStatus::Pending => "Chờ duyệt · Chưa công khai",
                                    CommentStatus::Rejected => "Không được duyệt · Chưa công khai",
                                    CommentStatus::Approved => "Đã được duyệt · Ngoài phần công khai đang tải",
                                    CommentStatus::Removed => "Nhận xét không còn hiển thị",
                                }).unwrap_or_default()}</p>
                                <p class="vk-muted">{move || state.read().own.comments.get(&id.get_value()).map(|c| match &c.placement {
                                    OwnPlacement::Root => "Nhận xét gốc của bạn",
                                    OwnPlacement::Reply { root_listed: false, .. } => "Phản hồi của bạn · Nhánh chưa có trong danh sách công khai",
                                    OwnPlacement::Reply { .. } => "Phản hồi của bạn",
                                }).unwrap_or_default()}</p>
                                <p class="vk-discussion-own-body">{move || state.read().own.comments.get(&id.get_value()).and_then(|c| (c.status != CommentStatus::Removed).then(|| c.body.clone()).flatten()).unwrap_or_default()}</p>
                                <p>{move || state.read().own.comments.get(&id.get_value()).and_then(|c| (c.status == CommentStatus::Rejected).then(|| c.rejection_reason.clone()).flatten()).unwrap_or_default()}</p>
                                <Show when=move || state.read().own.can_remove(&id.get_value())>
                                    <Button id=format!("remove-comment-{}", id.get_value()) variant=ButtonVariant::Text state=Signal::derive(move || if state.read().mutation_busy { ActionState::pending("Đang gỡ…") } else { ActionState::Enabled })
                                        on_press=move |_| remove_own(state, controller, lve, id.get_value())>"Gỡ nhận xét của tôi"</Button>
                                </Show>
                            </article>
                        }
                    } />
                <Show when=move || state.read().own.next_cursor.is_some()>
                    <Button id="own-comments-more" variant=ButtonVariant::Tonal
                        state=Signal::derive(move || if state.read().own.loading { ActionState::pending("Đang tải…") } else { ActionState::Enabled })
                        on_press=move |_| read_own(state, controller, true)>"Xem thêm nhận xét của bạn"</Button>
                </Show>
            </section>
        </Show>
    }
}

fn read_private(
    state: RwSignal<ViewState>,
    controller: RwSignal<DiscussionController>,
    append: bool,
) {
    read_own(state, controller, append);
    #[cfg(feature = "hydrate")]
    {
        let scope = controller.get_untracked().scope;
        if scope.session.is_none() {
            return;
        }
        state.update(|s| {
            s.own.author = false;
            s.own.capability_error = None;
        });
        spawn_private(true, async move {
            let result = api::load_discussion_capability(&scope.work_id).await;
            if controller
                .try_get_untracked()
                .is_none_or(|c| c.scope != scope)
            {
                return;
            }
            state.try_update(|s| match result {
                Ok(c)
                    if c.work_id == scope.work_id
                        && !matches!(
                            s.own.error,
                            Some(
                                crate::contract::ApiError::Unauthenticated
                                    | crate::contract::ApiError::Forbidden
                                    | crate::contract::ApiError::NotFound
                            )
                        ) =>
                {
                    s.own.author = c.viewer_is_work_author
                }
                _ => {
                    s.own.author = false;
                    s.own.capability_error = Some(crate::contract::ApiError::Unavailable);
                }
            });
        });
    }
}

fn read_own(state: RwSignal<ViewState>, controller: RwSignal<DiscussionController>, append: bool) {
    #[cfg(feature = "hydrate")]
    {
        let cursor = if append {
            state.read_untracked().own.next_cursor.clone()
        } else {
            None
        };
        if append && (cursor.is_none() || state.read_untracked().own.loading) {
            return;
        }
        let Some(Some(ticket)) = state.try_update(|s| s.own.begin()) else {
            return;
        };
        spawn_private(false, async move {
            let result =
                api::load_own_comments(&ticket.scope.work_id, cursor.as_deref(), None).await;
            if state.try_with_untracked(|s| s.own.accepts(&ticket)) != Some(true) {
                return;
            }
            let result = match result {
                Ok(page) if page.work_id == ticket.scope.work_id => Ok(page),
                Ok(_) => Err(crate::contract::ApiError::Unexpected { status: 200 }),
                Err(e) => Err(e),
            };
            match result {
                Ok(page) => {
                    state.update(|s| {
                        s.own.apply_list(&ticket, page, append);
                        s.redact();
                        revalidate_targets(s, controller);
                    });
                }
                Err(e) => {
                    state.update(|s| s.own.finish(&ticket, Some(e)));
                    return;
                }
            }
            let mut ids = state.with_untracked(|s| {
                s.public_ids()
                    .into_iter()
                    .chain(s.own.comments.keys().cloned())
                    .collect::<Vec<_>>()
            });
            ids.extend(controller.with_untracked(|c| {
                c.drafts
                    .values()
                    .filter_map(|d| d.acknowledged.as_ref().map(|ack| ack.comment_id.clone()))
                    .collect::<Vec<_>>()
            }));
            for batch in crate::discussion_reads::lookup_batches(ids) {
                if state.try_with_untracked(|s| s.own.accepts(&ticket)) != Some(true) {
                    return;
                }
                match api::load_own_comments(&ticket.scope.work_id, None, Some(&batch)).await {
                    Ok(page) if page.work_id == ticket.scope.work_id => {
                        state.try_update(|s| {
                            if s.own.apply_lookup(&ticket, &batch, page) {
                                s.redact();
                                revalidate_targets(s, controller);
                            }
                        });
                    }
                    result => {
                        let error = result
                            .err()
                            .unwrap_or(crate::contract::ApiError::Unexpected { status: 200 });
                        state.try_update(|s| s.own.finish(&ticket, Some(error)));
                        return;
                    }
                }
            }
            state.try_update(|s| s.own.finish(&ticket, None));
        });
    }
    #[cfg(not(feature = "hydrate"))]
    let _ = (state, controller, append);
}

#[component]
fn RootThreadView(
    root: String,
    controller: RwSignal<DiscussionController>,
    state: RwSignal<ViewState>,
    refresh: Callback<()>,
    write_state: Signal<ActionState>,
    lve: Option<Endorsements>,
) -> impl IntoView {
    let root = StoredValue::new(root);
    let facts = Signal::derive(move || {
        let state = state.read();
        let Some(thread) = state
            .page
            .roots
            .iter()
            .find(|thread| adapters::root_id(thread) == root.get_value())
        else {
            return crate::ui::patterns::DiscussionThreadFacts {
                root_id: root.get_value(),
                root: None,
                replies: vec![],
            };
        };
        let replies = state
            .replies
            .get(&root.get_value())
            .map(|page| page.replies.as_slice())
            .unwrap_or(&thread.preview);
        let mut facts = adapters::discussion_thread(thread, replies);
        for c in facts.root.iter_mut().chain(facts.replies.iter_mut()) {
            c.can_remove = state.own.can_remove(&c.id);
        }
        facts
    });
    let key = StoredValue::new(ComposerKey::Replies(root.get_value()));
    let on_reply = Callback::new(move |id: String| {
        let facts = facts.get_untracked();
        let target = facts
            .root
            .iter()
            .chain(facts.replies.iter())
            .find(|comment| comment.id == id && comment.can_reply)
            .map(|comment| Target {
                id: comment.id.clone(),
                label: comment.author.name.clone(),
            });
        if let Some(target) = target {
            controller.update(|c| c.reply(root.get_value(), target));
            #[cfg(feature = "hydrate")]
            focus_discussion_control(format!("reply-composer-{}", root.get_value()));
        }
    });
    let on_remove = Callback::new(move |id: String| remove_own(state, controller, lve, id));
    view! {
        <section class="vk-discussion-root" aria-label="Nhánh nhận xét">
            <DiscussionThread facts on_reply on_remove action_state=write_state endorsements=lve.map(Endorsements::comments)
                has_more_replies=Signal::derive(move || next_reply_cursor(&state.read(), &root.get_value()).is_some())
                reply_footer=std::sync::Arc::new(move || view! {
            <div class="vk-discussion-pagination" hidden=move || next_reply_cursor(&state.read(), &root.get_value()).is_none() && !state.read().reply_errors.contains_key(&root.get_value())>
            <p role="status">{move || state.read().reply_errors.get(&root.get_value()).cloned().unwrap_or_default()}</p>
            <Show when=move || next_reply_cursor(&state.read(), &root.get_value()).is_some()>
                <Button variant=ButtonVariant::Text state=Signal::derive(move || if state.read().reply_loading.get(&root.get_value()) == Some(&true) { ActionState::pending("Đang tải phản hồi…") } else { ActionState::Enabled })
                    on_press=move |_| read_replies(state, controller, lve, root.get_value())>"Xem thêm phản hồi"</Button>
            </Show>
            </div>
                }.into_any()) />
            <Show when=move || controller.read().draft(&key.get_value()).open>
                <div class="vk-discussion-reply-composer">
                    <DiscussionComposer key=key.get_value() controller state refresh />
                </div>
            </Show>
        </section>
    }
}

#[component]
fn DiscussionComposer(
    key: ComposerKey,
    controller: RwSignal<DiscussionController>,
    state: RwSignal<ViewState>,
    refresh: Callback<()>,
    #[prop(optional)] blocked: bool,
) -> impl IntoView {
    let key = StoredValue::new(key);
    let id = match key.get_value() {
        ComposerKey::Root => "root-composer".into(),
        ComposerKey::Replies(root) => format!("reply-composer-{root}"),
    };
    let value = Signal::derive(move || controller.read().draft(&key.get_value()).body);
    let action = Signal::derive(move || {
        let c = controller.read();
        if matches!(c.draft(&key.get_value()).delivery, Delivery::Sending(_)) {
            return ActionState::pending("Đang gửi…");
        }
        if blocked
            || state.read().unavailable
            || state.read().loading
            || state.read().mutation_busy
            || state.read().mutation_needs_refresh
        {
            return ActionState::disabled("Chờ tải lại nhận xét và chọn ngữ cảnh trước khi gửi.");
        }
        if let Some(reason) = c.reason(&key.get_value()) {
            ActionState::disabled(reason)
        } else {
            ActionState::Enabled
        }
    });
    view! {
        <div class="vk-discussion-composer">
            <FeedbackComposer id intent=FeedbackIntent::Unspecified value state=action tonal=true
                comments_closed=Signal::derive(move || !state.read().unavailable && !controller.read().settings.comments_open)
                on_input=move |input: TextInput| controller.update(|c| c.edit(&key.get_value(), input.value, input.composing))
                on_submit=move |_| { if !blocked { submit(controller, state, key.get_value(), refresh, false); } } >
                {matches!(key.get_value(), ComposerKey::Replies(_)).then(|| view! {
                    <div class="vk-discussion-composer-heading">
                        <p>{move || {
                            let draft = controller.read().draft(&key.get_value());
                            draft.target.map(|target| if draft.target_available { format!("Đang trả lời @{}", target.label) } else { target.label }).unwrap_or_default()
                        }}</p>
                        <Button variant=ButtonVariant::Text on_press=move |_| {
                            #[cfg(feature = "hydrate")]
                            let target = controller.read().draft(&key.get_value()).target;
                            controller.update(|c| c.close_reply(&key.get_value()));
                            #[cfg(feature = "hydrate")]
                            if let Some(target) = target { focus_discussion_control(format!("reply-to-{}", target.id)); }
                        }>"Thu ô trả lời · Giữ chữ"</Button>
                    </div>
                })}
            </FeedbackComposer>
            <div class="vk-discussion-delivery" hidden=move || controller.read().draft(&key.get_value()).receipt.is_none() && !matches!(controller.read().draft(&key.get_value()).delivery, Delivery::Uncertain(_))>
            <p role="status" data-testid="discussion-receipt">{move || {
                let draft = controller.read().draft(&key.get_value());
                if matches!(draft.delivery, Delivery::Uncertain(_)) { "Chưa rõ kết quả gửi".into() } else { draft.receipt.unwrap_or_default() }
            }}</p>
            <Show when=move || matches!(controller.read().draft(&key.get_value()).delivery, Delivery::Uncertain(_))>
                <p>"Dùng lại nội dung lần trước, giữ nháp."</p>
                <Button variant=ButtonVariant::Filled state=Signal::derive(move || {
                    if !controller.read().write_ready() { ActionState::disabled("Máy chủ chưa xác nhận phiên đăng nhập; tải lại để đối chiếu. Chữ vẫn được giữ.") }
                    else if state.read().loading || state.read().mutation_busy || state.read().mutation_needs_refresh { ActionState::disabled("Chờ tải lại nhận xét trước khi đối soát.") }
                    else { ActionState::Enabled }
                }) on_press=move |_| submit(controller, state, key.get_value(), refresh, true)>"Kiểm tra lần gửi"</Button>
            </Show>
            </div>
        </div>
    }
}

#[cfg(feature = "hydrate")]
fn focus_discussion_control(id: String) {
    use wasm_bindgen::JsCast;
    request_animation_frame(move || {
        if let Some(control) = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id(&id))
            .and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok())
        {
            let _ = control.focus();
        }
    });
}

fn next_reply_cursor(state: &ViewState, root: &str) -> Option<String> {
    if let Some(replies) = state.replies.get(root) {
        return replies.next_cursor.clone();
    }
    state
        .page
        .roots
        .iter()
        .find(|thread| adapters::root_id(thread) == root)
        .and_then(|thread| thread.preview_next_cursor.clone())
}

#[cfg(any(feature = "hydrate", test))]
fn merge_comments(
    current: &mut Vec<crate::contract::CommentDto>,
    incoming: Vec<crate::contract::CommentDto>,
) {
    for comment in incoming {
        if let Some(existing) = current
            .iter_mut()
            .find(|c| c.comment_id == comment.comment_id)
        {
            *existing = comment;
        } else {
            current.push(comment);
        }
    }
    // The provider already ordered the keyset page. RFC3339 text is not an instant sort
    // key when fractional precision varies; keep its exact order and update duplicates.
}

#[cfg(feature = "hydrate")]
fn revalidate_targets(state: &ViewState, controller: RwSignal<DiscussionController>) {
    let mut visible = Vec::new();
    for thread in &state.page.roots {
        let root = adapters::root_id(thread);
        let root_comment = match &thread.root {
            crate::contract::RootView::Comment { comment } => Some(comment),
            _ => None,
        };
        let replies = state
            .replies
            .get(root)
            .map(|page| page.replies.as_slice())
            .unwrap_or(&thread.preview);
        for comment in root_comment
            .into_iter()
            .chain(replies.iter())
            .filter(|comment| comment.visibility == "public")
        {
            visible.push((
                root.to_owned(),
                comment.comment_id.clone(),
                comment.author_display_name.clone(),
            ));
        }
    }
    controller.update(|c| c.revalidate_targets(&visible));
}

fn read_roots(
    state: RwSignal<ViewState>,
    controller: RwSignal<DiscussionController>,
    lve: Option<Endorsements>,
    append: bool,
) {
    #[cfg(feature = "hydrate")]
    {
        if state.read_untracked().mutation_busy {
            return;
        }
        if state.read_untracked().loading {
            if !append {
                state.update(|s| s.refresh_queued = true);
            }
            return;
        }
        let scope = controller.get_untracked().scope;
        let cursor = if append {
            state.get_untracked().page.next_cursor
        } else {
            None
        };
        if append && cursor.is_none() {
            return;
        }
        let mut epoch = 0;
        state.update(|s| {
            s.loading = true;
            s.refresh_queued = false;
            s.reply_loading.clear();
            s.error = None;
            s.page_error = None;
            s.epoch += 1;
            epoch = s.epoch;
        });
        spawn_transport(async move {
            let result = api::load_discussion(&scope.work_id, cursor.as_deref()).await;
            if controller
                .try_get_untracked()
                .is_none_or(|c| c.scope != scope)
            {
                return;
            }
            let mut refresh_again = false;
            let mut endorsable = None;
            state.try_update(|s| {
                if !s.accepts_read(epoch) {
                    return;
                }
                s.loading = false;
                refresh_again = s.refresh_queued;
                s.refresh_queued = false;
                match result {
                    Ok(page) if page.work_id == scope.work_id => {
                        endorsable = Some(page_targets(&page));
                        controller.update(|c| c.settings = page.settings.clone());
                        s.unavailable = false;
                        s.mutation_needs_refresh = false;
                        // Invalidate old per-root reads and context projections on a refresh.
                        s.reply_loading.clear();
                        if append {
                            for thread in page.roots {
                                let id = adapters::root_id(&thread);
                                if let Some(old) = s
                                    .page
                                    .roots
                                    .iter_mut()
                                    .find(|old| adapters::root_id(old) == id)
                                {
                                    *old = thread;
                                } else {
                                    s.page.roots.push(thread);
                                }
                            }
                            s.page.next_cursor = page.next_cursor;
                            s.page.viewer_is_work_author = page.viewer_is_work_author;
                            s.page.settings = page.settings;
                        } else {
                            s.page = page;
                            s.replies.clear();
                            s.reply_errors.clear();
                        }
                        s.redact();
                        revalidate_targets(s, controller);
                    }
                    Err(error) => {
                        s.fail_roots_read(append, &error);
                        if error == crate::contract::ApiError::NotFound {
                            revalidate_targets(s, controller);
                        }
                    }
                    _ => {
                        let message = "Phản hồi không khớp tác phẩm; chưa cập nhật.".to_owned();
                        if append {
                            s.page_error = Some(message);
                        } else {
                            s.error = Some(message);
                        }
                    }
                }
            });
            // Fresh counts and own states for the comments this read returned.
            let public_loaded = endorsable.is_some();
            if let (Some(lve), Some(targets)) = (lve, endorsable) {
                lve.read(targets, true);
            }
            if public_loaded {
                read_own(state, controller, false);
            }
            if refresh_again {
                read_roots(state, controller, lve, false);
            }
        });
    }
    #[cfg(not(feature = "hydrate"))]
    {
        let _ = (state, controller, lve, append);
    }
}

fn read_replies(
    state: RwSignal<ViewState>,
    controller: RwSignal<DiscussionController>,
    lve: Option<Endorsements>,
    root: String,
) {
    #[cfg(feature = "hydrate")]
    {
        let before = state.get_untracked();
        if before.loading || before.mutation_busy || before.reply_loading.get(&root) == Some(&true)
        {
            return;
        }
        let Some(cursor) = next_reply_cursor(&before, &root) else {
            return;
        };
        let epoch = before.epoch;
        let scope = controller.get_untracked().scope;
        state.update(|s| {
            s.reply_loading.insert(root.clone(), true);
            s.reply_errors.remove(&root);
        });
        spawn_transport(async move {
            let result = api::load_replies(&scope.work_id, &root, Some(&cursor)).await;
            if controller
                .try_get_untracked()
                .is_none_or(|c| c.scope != scope)
            {
                return;
            }
            let mut endorsable = None;
            state.try_update(|s| {
                if !s.accepts_read(epoch) {
                    return;
                }
                s.reply_loading.insert(root.clone(), false);
                match result {
                    Ok(page) => {
                        endorsable = Some(public_targets(&page.replies));
                        let initial = s
                            .page
                            .roots
                            .iter()
                            .find(|thread| adapters::root_id(thread) == root)
                            .map(|thread| thread.preview.clone())
                            .unwrap_or_default();
                        let current = s.replies.entry(root).or_insert(ReplyList {
                            replies: initial,
                            next_cursor: None,
                        });
                        merge_comments(&mut current.replies, page.replies);
                        current.next_cursor = page.next_cursor;
                        s.redact();
                        revalidate_targets(s, controller);
                    }
                    Err(error) => {
                        if error == crate::contract::ApiError::NotFound {
                            s.replies.remove(&root);
                            s.page
                                .roots
                                .retain(|thread| adapters::root_id(thread) != root);
                            revalidate_targets(s, controller);
                        }
                        s.reply_errors.insert(
                            root,
                            crate::discussion_state::read_error_message(&error, true).into(),
                        );
                    }
                }
            });
            let public_loaded = endorsable.is_some();
            if let (Some(lve), Some(targets)) = (lve, endorsable) {
                lve.read(targets, true);
            }
            if public_loaded {
                read_own(state, controller, false);
            }
        });
    }
    #[cfg(not(feature = "hydrate"))]
    {
        let _ = (state, controller, lve, root);
    }
}

fn submit(
    controller: RwSignal<DiscussionController>,
    state: RwSignal<ViewState>,
    key: ComposerKey,
    refresh: Callback<()>,
    reconcile: bool,
) {
    #[cfg(feature = "hydrate")]
    {
        if state.read_untracked().unavailable
            || state.read_untracked().loading
            || state.read_untracked().mutation_busy
            || state.read_untracked().mutation_needs_refresh
        {
            return;
        }
        let command_key = || {
            let mut random = [0_u8; 24];
            web_sys::window()?
                .crypto()
                .ok()?
                .get_random_values_with_u8_array(&mut random)
                .ok()?;
            Some(
                random
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>(),
            )
        };
        let mut submission = None;
        controller.update(|c| {
            submission = if reconcile {
                c.reconcile(&key)
            } else {
                command_key().and_then(|command_key| c.begin(&key, command_key))
            }
        });
        let Some(submission) = submission else {
            return;
        };
        spawn_transport(async move {
            let result = api::submit_comment(&submission.scope.work_id, &submission.request).await;
            let mut accepted = false;
            controller.try_update(|c| accepted = c.complete(&submission, result));
            if accepted {
                refresh.run(());
            }
        });
    }
    #[cfg(not(feature = "hydrate"))]
    {
        let _ = (controller, state, key, refresh, reconcile);
    }
}

fn change_mode(
    state: RwSignal<ViewState>,
    controller: RwSignal<DiscussionController>,
    lve: Option<Endorsements>,
) {
    #[cfg(feature = "hydrate")]
    {
        let c = controller.get_untracked();
        let s = state.get_untracked();
        if !c.write_ready()
            || !s.own.author
            || s.mutation_busy
            || s.mutation_needs_refresh
            || s.loading
        {
            return;
        }
        let request = crate::contract::CommentsSettingsRequest {
            expected_generation: c.settings.generation,
            comments_open: !c.settings.comments_open,
        };
        let mut started = false;
        state.update(|s| started = s.begin_mutation());
        if !started {
            return;
        }
        spawn_transport(async move {
            let result = api::set_comments_mode(&c.scope.work_id, &request).await;
            if controller
                .try_get_untracked()
                .is_none_or(|current| current.scope != c.scope)
            {
                return;
            }
            state.try_update(|s| {
                s.mutation_busy = false;
                match result {
                    Ok(settings) => {
                        controller.update(|c| c.settings = settings);
                        s.mutation_message = Some("Đã cập nhật thiết lập bình luận.".into());
                    }
                    Err(error) => {
                        s.mutation_message =
                            Some(crate::discussion_state::error_message(&error).into());
                        s.mutation_needs_refresh = true;
                    }
                }
            });
            read_roots(state, controller, lve, false);
            read_private(state, controller, false);
        });
    }
    #[cfg(not(feature = "hydrate"))]
    {
        let _ = (state, controller, lve);
    }
}

fn remove_own(
    state: RwSignal<ViewState>,
    controller: RwSignal<DiscussionController>,
    lve: Option<Endorsements>,
    id: String,
) {
    #[cfg(feature = "hydrate")]
    {
        let c = controller.get_untracked();
        let s = state.get_untracked();
        let allowed = s.own.can_remove(&id);
        if !allowed || !c.write_ready() || s.mutation_busy || s.mutation_needs_refresh || s.loading
        {
            return;
        }
        let mut started = false;
        state.update(|s| started = s.begin_mutation());
        if !started {
            return;
        }
        spawn_transport(async move {
            let result = api::remove_comment(&c.scope.work_id, &id).await;
            if controller
                .try_get_untracked()
                .is_none_or(|current| current.scope != c.scope)
            {
                return;
            }
            state.try_update(|s| {
                s.mutation_busy = false;
                s.mutation_message = Some(match result {
                    Ok(_) => {
                        if s.public_ids().contains(&id) {
                            s.redacted.insert(id.clone());
                        }
                        if let Some(c) = s.own.comments.get_mut(&id) {
                            c.status = CommentStatus::Removed;
                            c.body = None;
                            c.can_remove = false;
                            c.rejection_reason = None;
                        }
                        s.redact();
                        "Đã gỡ nhận xét của bạn.".into()
                    }
                    Err(error) => {
                        s.mutation_needs_refresh = true;
                        crate::discussion_state::error_message(&error).into()
                    }
                });
            });
            read_roots(state, controller, lve, false);
            read_private(state, controller, false);
        });
    }
    #[cfg(not(feature = "hydrate"))]
    {
        let _ = (state, controller, lve, id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepted_off_page_reply_keeps_its_receipt_after_the_submitted_text_is_cleared() {
        let mut controller = DiscussionController::new(
            Scope {
                work_id: "work".into(),
                session: Some("fixture-session".into()),
            },
            crate::contract::CommentsSettings {
                comments_open: true,
                generation: 1,
            },
        );
        let key = ComposerKey::Replies("page-two-root".into());
        assert!(!should_recover_reply(&controller.draft(&key)));
        controller.reply(
            "page-two-root".into(),
            Target {
                id: "page-two-root".into(),
                label: "Tác giả mẫu".into(),
            },
        );
        controller.edit(&key, "Nhận xét ở trang thứ hai".into(), false);
        let request = controller.begin(&key, "off-page-command".into()).unwrap();
        assert!(should_recover_reply(&controller.draft(&key)));
        assert!(reply_needs_editor(&controller.draft(&key)));
        assert!(controller.complete(
            &request,
            Ok(crate::contract::SubmittedComment {
                comment_id: "pending-comment".into(),
                status: "pending".into(),
                replayed: false
            })
        ));
        let retained = controller.draft(&key);
        assert!(retained.body.is_empty());
        assert!(retained.receipt.as_deref().unwrap().contains("Chờ duyệt"));
        assert!(should_recover_reply(&retained));
        assert!(!reply_needs_editor(&retained));
        controller.edit(&key, "Một lần gửi khác".into(), false);
        assert!(
            controller
                .begin(&key, "second-off-page-command".into())
                .is_some()
        );
        controller.edit(&key, String::new(), false);
        let unsettled = controller.draft(&key);
        assert!(unsettled.receipt.is_none());
        assert!(should_recover_reply(&unsettled));
        assert!(reply_needs_editor(&unsettled));
    }
    #[test]
    fn loss_of_public_access_never_claims_the_author_closed_comments() {
        for prior_mode in [true, false] {
            assert_eq!(
                comments_mode_message(true, prior_mode),
                "Thảo luận hiện không khả dụng. Bản đang soạn được giữ lại."
            );
            assert!(!comments_mode_message(true, prior_mode).contains("Tác giả"));
        }
        assert_eq!(
            comments_mode_message(false, false),
            "Tác giả đã tắt bình luận cho bài viết này"
        );
        assert_eq!(comments_mode_message(false, true), "Bình luận đang mở");
    }
    #[test]
    fn a_failed_next_page_stays_local_while_a_refresh_failure_or_loss_of_access_keeps_its_scope() {
        use crate::contract::ApiError;
        let page: DiscussionPage = serde_json::from_str(include_str!(
            "../../../../services/vink-app/contracts/http/v1/fixtures/discussion.get.json"
        ))
        .unwrap();
        let mut state = ViewState {
            own: OwnState::new(Scope {
                work_id: page.work_id.clone(),
                session: None,
            }),
            redacted: BTreeSet::new(),
            page,
            replies: BTreeMap::new(),
            reply_loading: BTreeMap::new(),
            reply_errors: BTreeMap::new(),
            loading: false,
            refresh_queued: false,
            error: None,
            page_error: None,
            unavailable: false,
            epoch: 0,
            mutation_busy: false,
            mutation_message: None,
            mutation_needs_refresh: false,
        };
        let loaded = state.page.roots.len();
        state.fail_roots_read(true, &ApiError::Unavailable);
        assert!(
            state.error.is_none(),
            "region status must not carry a next-page failure"
        );
        let local = state.page_error.clone().unwrap();
        assert!(local.contains("thử lại") && !local.contains("chưa rõ kết quả"));
        assert_eq!(state.page.roots.len(), loaded);
        assert!(!state.unavailable);
        state.fail_roots_read(false, &ApiError::Network);
        let refresh_error = state.error.as_deref().unwrap();
        assert!(refresh_error.contains("Chưa làm mới được") && !refresh_error.contains("chưa rõ"));
        state.fail_roots_read(true, &ApiError::NotFound);
        assert!(state.unavailable && state.page.roots.is_empty());
        assert!(
            state
                .error
                .as_deref()
                .unwrap()
                .contains("không còn khả dụng")
        );
    }
    #[test]
    fn loaded_page_read_copy_never_describes_a_read_as_an_unknown_write() {
        for error in [
            crate::contract::ApiError::Network,
            crate::contract::ApiError::Unavailable,
            crate::contract::ApiError::Unexpected { status: 500 },
        ] {
            assert_eq!(
                crate::discussion_state::read_error_message(&error, true),
                "Chưa tải thêm được. Phần đã tải vẫn giữ nguyên; bạn có thể thử lại."
            );
            let refresh = crate::discussion_state::read_error_message(&error, false);
            assert!(refresh.contains("Chưa làm mới được") && !refresh.contains("chưa rõ"));
        }
        for error in [
            crate::contract::ApiError::Unauthenticated,
            crate::contract::ApiError::Forbidden,
            crate::contract::ApiError::NotFound,
        ] {
            assert_eq!(
                crate::discussion_state::read_error_message(&error, true),
                crate::discussion_state::error_message(&error)
            );
        }
    }
    #[test]
    fn cursors_are_authoritative_even_when_preview_is_short_or_count_differs() {
        let page: DiscussionPage = serde_json::from_str(include_str!(
            "../../../../services/vink-app/contracts/http/v1/fixtures/discussion.get.json"
        ))
        .unwrap();
        let mut state = ViewState {
            own: OwnState::new(Scope {
                work_id: page.work_id.clone(),
                session: None,
            }),
            redacted: BTreeSet::new(),
            page,
            replies: BTreeMap::new(),
            reply_loading: BTreeMap::new(),
            reply_errors: BTreeMap::new(),
            loading: false,
            refresh_queued: false,
            error: None,
            page_error: None,
            unavailable: false,
            epoch: 0,
            mutation_busy: false,
            mutation_message: None,
            mutation_needs_refresh: false,
        };
        let root = adapters::root_id(&state.page.roots[0]).to_owned();
        assert!(next_reply_cursor(&state, &root).is_some());
        state.replies.insert(
            root.clone(),
            ReplyList {
                replies: vec![],
                next_cursor: None,
            },
        );
        assert!(next_reply_cursor(&state, &root).is_none());
        let mut comments = state.page.roots[0].preview.clone();
        let old_len = comments.len();
        let mut updated = comments[0].clone();
        updated.body = "Đã cập nhật".into();
        merge_comments(&mut comments, vec![updated]);
        assert_eq!(comments.len(), old_len);
        assert_eq!(comments[0].body, "Đã cập nhật");
        let mut whole = comments[0].clone();
        whole.comment_id = "whole-second".into();
        whole.created_at = "2026-11-03T08:00:09Z".into();
        let mut fractional = whole.clone();
        fractional.comment_id = "fractional-second".into();
        fractional.created_at = "2026-11-03T08:00:09.500Z".into();
        let mut ordered = vec![whole];
        merge_comments(&mut ordered, vec![fractional]);
        assert_eq!(
            ordered
                .iter()
                .map(|c| c.comment_id.as_str())
                .collect::<Vec<_>>(),
            ["whole-second", "fractional-second"]
        );
        let before_mutation = state.epoch;
        assert!(state.begin_mutation());
        assert!(!state.accepts_read(before_mutation));
        assert!(!state.begin_mutation());
        state.mutation_busy = false;
        assert!(
            !state.accepts_read(before_mutation),
            "old GET cannot resurrect a removed comment or overwrite a committed mode"
        );
        assert!(state.accepts_read(state.epoch));
    }
}

#[cfg(test)]
mod separated_read_regressions {
    use super::*;
    fn state() -> ViewState {
        let public: crate::contract::PublicDiscussionPage = serde_json::from_str(include_str!(
            "../../../../services/vink-app/contracts/http/v1/fixtures/discussion.public.get.json"
        ))
        .unwrap();
        ViewState {
            own: OwnState::new(Scope {
                work_id: public.work_id.clone(),
                session: Some("a/0".into()),
            }),
            redacted: BTreeSet::new(),
            page: public.into(),
            replies: BTreeMap::new(),
            reply_loading: BTreeMap::new(),
            reply_errors: BTreeMap::new(),
            loading: false,
            refresh_queued: false,
            error: None,
            page_error: None,
            unavailable: false,
            epoch: 0,
            mutation_busy: false,
            mutation_message: None,
            mutation_needs_refresh: false,
        }
    }
    #[test]
    fn a_scope_change_releases_both_read_and_mutation_owners_and_fences_old_reads() {
        let mut s = state();
        s.loading = true;
        s.refresh_queued = true;
        let old = s.epoch;
        let public = s.page.clone();
        s.own.author = true;
        s.change_session(Scope {
            work_id: s.page.work_id.clone(),
            session: Some("b/1".into()),
        });
        assert!(!s.loading && !s.refresh_queued && !s.mutation_busy && !s.own.author);
        assert!(!s.accepts_read(old));
        assert_eq!(s.page, public, "actor-free data survives account switching");
        assert!(
            s.begin_mutation(),
            "new account is not blocked by old loading"
        );
        let old = s.epoch;
        s.change_session(Scope {
            work_id: s.page.work_id.clone(),
            session: Some("a/2".into()),
        });
        assert!(!s.mutation_busy && s.mutation_message.is_none());
        assert!(!s.accepts_read(old));
        assert!(
            s.begin_mutation(),
            "old mutation cannot strand the new generation"
        );
    }
    #[test]
    fn removed_or_held_lookup_facts_fence_public_resurrection_and_context_names() {
        let mut s = state();
        let public = s.page.clone();
        let crate::contract::RootView::Comment { comment } = &s.page.roots[0].root else {
            panic!()
        };
        let id = comment.comment_id.clone();
        let reply_id = s.page.roots[0].preview[0].comment_id.clone();
        let own: crate::contract::OwnCommentList = serde_json::from_str(include_str!(
            "../../../../services/vink-app/contracts/http/v1/fixtures/discussion.me.get.json"
        ))
        .unwrap();
        let mut held = own.comments[0].clone();
        held.comment_id = id.clone();
        s.own.comments.insert(id.clone(), held);
        s.redact();
        assert!(matches!(
            s.page.roots[0].root,
            crate::contract::RootView::Anchor { .. }
        ));
        s.redacted.insert(reply_id.clone());
        s.page = public.clone(); // a delayed public snapshot has all the old bodies and names
        s.redact();
        assert!(!s.public_ids().contains(&id) && !s.public_ids().contains(&reply_id));
        assert_eq!(
            s.page.roots[0].preview[0].reply_to,
            Some(crate::contract::ReplyTo::Unavailable)
        );
        let count = public.roots[0].public_replies;
        assert_eq!(
            s.page.roots[0].public_replies, count,
            "never replace the public count with own entries"
        );
        let old_epoch = s.epoch;
        s.change_session(Scope {
            work_id: s.page.work_id.clone(),
            session: None,
        });
        assert!(
            !s.public_ids().contains(&id),
            "the scrubbed public page stays scrubbed"
        );
        assert!(
            !s.accepts_read(old_epoch),
            "old public data cannot enter after a scope change"
        );
        assert!(
            s.redacted.is_empty(),
            "even reconciliation IDs leave with the private scope"
        );
    }
}
