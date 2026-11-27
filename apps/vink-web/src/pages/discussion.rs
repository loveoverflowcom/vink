//! Reader feature adapter. Patterns receive view facts/intents only; HTTP and request scope
//! stay here. SSR always reads anonymously, even when the diagnostic identity fixture runs.
mod auth;

use crate::account::Session;
use crate::contract::{CommentStatus, OwnPlacement};
use crate::discussion_reads::{OwnChip, OwnState};
pub(crate) use auth::{ReaderAuth, ReaderAuthDialog};
#[cfg(feature = "browser")]
pub(crate) use auth::{ReaderAuthPurpose, ReaderAuthReady, ReaderAuthSettled};
use std::collections::{BTreeMap, BTreeSet};

use leptos::prelude::*;
use vink_ui::{
    ActionState, Button, ButtonVariant, FieldMode, Heading, HeadingLevel, Icon, IconButton,
    IconName, IconSize, LoadingIndicator, Skeleton, SkeletonShape, TextInput, TypeRole,
};
#[cfg(feature = "browser")]
use vink_ui::{Clock, use_loading_gate};

use super::endorsement::Endorsements;
use crate::api::{self, LoadError};
use crate::contract::{ApiError, DiscussionPage, EndorsementSummaries, ReplyList};
use crate::discussion_state::{
    ComposerKey, ComposerStatus, Delivery, DiscussionController, NoticeTone, ReplyTarget, Scope,
    SendEmphasis, SendIntent, Target,
};
use crate::endorsement_state::page_targets;
#[cfg(feature = "browser")]
use crate::endorsement_state::public_targets;
use crate::ui::{
    adapters,
    empty_state::EmptyState,
    patterns::{
        DiscussionThread, FeedbackComposer, FeedbackIntent, OwnRemoveAction, RemoveConfirmation,
    },
};

#[cfg(feature = "browser")]
#[derive(Clone)]
struct TransportOwner(leptos::reactive::owner::WeakOwner);

#[cfg(feature = "browser")]
fn spawn_transport(task: impl std::future::Future<Output = ()> + 'static) {
    // A button/composer owner may disappear on collapse or projection refresh. The feature
    // owner survives those presentation changes and cancels only when this reader ends.
    if let Some(owner) = use_context::<TransportOwner>().and_then(|owner| owner.0.upgrade()) {
        owner.with(|| leptos::task::spawn_local_scoped_with_cancellation(task));
    }
}

// Separate cancellable read owners. Replacing one drops its fetch future and deadline;
// writes stay under TransportOwner because aborting a write cannot undo a server commit.
#[cfg(feature = "browser")]
#[derive(Clone, Copy)]
struct PrivateTransports {
    own: StoredValue<Option<Owner>>,
    capability: StoredValue<Option<Owner>>,
}
#[cfg(feature = "browser")]
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
#[cfg(feature = "browser")]
fn cancel_private() {
    let t = expect_context::<PrivateTransports>();
    for slot in [t.own, t.capability] {
        if let Some(old) = slot.get_value() {
            old.cleanup();
        }
        slot.set_value(None);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DiscussionMutation {
    ChangeMode,
    #[cfg_attr(not(any(feature = "browser", test)), allow(dead_code))]
    RemoveOwn,
}

#[derive(Clone)]
struct PageReadFailure {
    message: &'static str,
    restart: bool,
}

impl PageReadFailure {
    const RESTART: Self = Self {
        message: "Danh sách nhận xét đã thay đổi. Phần đã tải vẫn được giữ.",
        restart: true,
    };

    fn from_read(error: &ApiError) -> Self {
        if matches!(error, ApiError::Invalid { field: Some(field) } if field == "cursor") {
            Self::RESTART
        } else {
            Self {
                message: crate::discussion_state::read_error_message(error, true),
                restart: false,
            }
        }
    }

    fn action(&self) -> &'static str {
        if self.restart {
            "Tải từ đầu"
        } else {
            "Thử lại"
        }
    }
}

#[derive(Clone)]
struct ViewState {
    page: DiscussionPage,
    roots_read: crate::read_lifecycle::ReadOwner<Scope>,
    #[cfg_attr(not(feature = "browser"), allow(dead_code))]
    roots_chain: crate::read_lifecycle::PageChain,
    restarted_tail: bool,
    #[cfg_attr(not(feature = "browser"), allow(dead_code))]
    wake: crate::read_events::WakePolicy,
    own: OwnState,
    /// IDs of previously public rows fenced by a current private observation.
    redacted: BTreeSet<String>,
    replies: BTreeMap<String, ReplyList>,
    reply_loading: BTreeMap<String, bool>,
    #[cfg_attr(not(feature = "browser"), allow(dead_code))]
    reply_chains: BTreeMap<String, crate::read_lifecycle::PageChain>,
    reply_errors: BTreeMap<String, PageReadFailure>,
    loading: bool,
    #[cfg_attr(not(feature = "browser"), allow(dead_code))]
    refresh_queued: bool,
    error: Option<String>,
    /// A failed "load more roots" read, shown beside the control that started it.
    page_error: Option<PageReadFailure>,
    unavailable: bool,
    #[cfg_attr(not(feature = "browser"), allow(dead_code))]
    epoch: u64,
    local_fixture: bool,
    mutation: Option<DiscussionMutation>,
    mutation_message: Option<String>,
    mutation_needs_refresh: bool,
}

#[cfg_attr(not(any(feature = "browser", test)), allow(dead_code))]
impl ViewState {
    #[cfg(feature = "browser")]
    fn has_paged_tail(&self) -> bool {
        self.roots_chain.pages > 1 || self.reply_chains.values().any(|chain| chain.pages > 1)
    }

    fn change_session(&mut self, scope: Scope) {
        self.roots_read.change_scope(scope.clone());
        self.own.change_scope(scope);
        // The public page is already scrubbed. Old reads are fenced below; discard even
        // ID-only private reconciliation knowledge before another principal starts.
        self.redacted.clear();
        self.epoch += 1;
        self.loading = false;
        self.refresh_queued = false;
        self.reply_loading.clear();
        self.mutation = None;
        self.mutation_message = None;
        self.mutation_needs_refresh = false;
        self.page.viewer_is_work_author = false;
    }
    #[cfg(feature = "browser")]
    fn invalidate_attribution(&mut self) {
        use crate::contract::{AuthorProjection, CommentDto, OwnPlacement, ReplyTo, RootView};
        fn context(reply_to: &mut ReplyTo) {
            if let ReplyTo::Reply {
                author_display_name,
                author,
                ..
            } = reply_to
            {
                author_display_name.clear();
                *author = Some(AuthorProjection::Unsupported);
            }
        }
        fn comment(row: &mut CommentDto) {
            row.author_display_name.clear();
            row.author = Some(AuthorProjection::Unsupported);
            if let Some(reply_to) = &mut row.reply_to {
                context(reply_to);
            }
        }
        for root in &mut self.page.roots {
            if let RootView::Comment { comment: row } = &mut root.root {
                comment(row);
            }
            root.preview.iter_mut().for_each(comment);
        }
        for replies in self.replies.values_mut() {
            replies.replies.iter_mut().for_each(comment);
        }
        for row in self.own.comments.values_mut() {
            if let OwnPlacement::Reply { reply_to, .. } = &mut row.placement {
                context(reply_to);
            }
        }
        self.roots_read.invalidate();
        self.own.invalidate_reads();
        self.epoch += 1;
        self.loading = false;
        self.reply_loading.clear();
        self.wake = crate::read_events::WakePolicy::default();
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

    fn begin_mutation(&mut self, mutation: DiscussionMutation) -> bool {
        if self.loading
            || self.mutation.is_some()
            || self.mutation_needs_refresh
            || self.unavailable
        {
            return false;
        }
        self.roots_read.invalidate();
        self.own.invalidate_reads();
        self.epoch += 1; // Fence every pre-mutation root/reply read, including delayed responses.
        self.reply_loading.clear();
        self.mutation = Some(mutation);
        self.mutation_message = None;
        true
    }

    fn accepts_read(&self, epoch: u64) -> bool {
        self.epoch == epoch && self.mutation.is_none()
    }

    /// A refresh failure belongs to the whole region; a failed next-page read stays by its
    /// button so the loaded list and the retry control keep their meaning.
    fn fail_roots_read(&mut self, append: bool, error: &crate::contract::ApiError) {
        if !can_retry_discussion_read(error) {
            self.error = Some(crate::discussion_state::error_message(error).into());
            self.unavailable = true;
            self.page.roots.clear();
            self.replies.clear();
            self.own.change_scope(self.own.scope.clone());
        } else if append {
            self.page_error = Some(PageReadFailure::from_read(error));
        } else {
            self.error = Some(crate::discussion_state::read_error_message(error, false).into());
        }
    }
}

fn initial_chain(page: &DiscussionPage) -> crate::read_lifecycle::PageChain {
    use crate::read_lifecycle::{Intent, PageChain, ReadOwner};
    let mut owner = ReadOwner::new(());
    let ticket = owner.begin(Intent::Initial, None).unwrap();
    let mut chain = PageChain::default();
    chain.accept(
        &ticket,
        page.roots.iter().map(|r| adapters::root_id(r).to_owned()),
        page.next_cursor.clone(),
    );
    chain
}

/// `viewer_is_author` is the server's `viewer_is_work_author` (VDR-DISCUSSION-R4@r1 §4).
fn closed_comments_message(viewer_is_author: bool) -> &'static str {
    if viewer_is_author {
        "Bạn đã tắt bình luận cho bài viết này"
    } else {
        "Tác giả đã tắt bình luận cho bài viết này"
    }
}

/// “Chưa có nhận xét…” only for a list the server returned empty, never for a failed read.
fn shows_empty(state: &ViewState) -> bool {
    !state.unavailable && state.error.is_none() && state.page.roots.is_empty()
}

/// Both pagination and its DOM metadata read the server's continuation.
fn has_more_roots(state: &ViewState) -> bool {
    state.page.next_cursor.is_some()
}

/// Decorative icon of an own-comment chip; the label carries the meaning.
fn own_chip_icon(chip: OwnChip) -> IconName {
    match chip {
        OwnChip::Pending => IconName::Schedule,
        OwnChip::Rejected => IconName::Cancel,
        OwnChip::ApprovedOffPage => IconName::Check,
        OwnChip::Unavailable => IconName::VisibilityOff,
    }
}

fn notice_icon(tone: NoticeTone) -> IconName {
    match tone {
        NoticeTone::Waiting => IconName::Schedule,
        NoticeTone::Unknown => IconName::Info,
        NoticeTone::Adverse => IconName::Error,
        NoticeTone::Success => IconName::Check,
        NoticeTone::Neutral => IconName::Info,
    }
}

fn comments_mode_kind(
    state: &ViewState,
    controller: &DiscussionController,
    read_only: bool,
) -> &'static str {
    if state.mutation == Some(DiscussionMutation::ChangeMode) {
        "changing"
    } else if state.unavailable || state.error.is_some() || state.mutation_needs_refresh {
        "conflict"
    } else if read_only {
        "system-locked"
    } else if !controller.settings.comments_open {
        "author-closed"
    } else {
        "open"
    }
}

fn copy_draft_text(
    editor_id: String,
    text: String,
    status: RwSignal<Option<(bool, &'static str)>>,
) {
    #[cfg(feature = "browser")]
    {
        use wasm_bindgen::JsCast;
        // Invoke writeText during the user gesture. navigator.clipboard can be absent
        // in a non-secure context; a present clipboard can also reject permission.
        let promise = web_sys::window()
            .map(|window| window.navigator().clipboard())
            .filter(|clipboard| !clipboard.is_undefined() && !clipboard.is_null())
            .map(|clipboard| clipboard.write_text(&text));
        spawn_transport(async move {
            let copied = match promise {
                Some(promise) => wasm_bindgen_futures::JsFuture::from(promise).await.is_ok(),
                None => false,
            };
            let message = if copied {
                "Đã sao chép nháp."
            } else {
                "Không thể tự động sao chép. Hãy chọn chữ trong ô soạn và sao chép."
            };
            if status.try_set(Some((copied, message))).is_none()
                && !copied
                && let Some(field) = document()
                    .get_element_by_id(&editor_id)
                    .and_then(|node| node.dyn_into::<web_sys::HtmlTextAreaElement>().ok())
                && field.value() == text
            {
                let _ = field.focus();
                field.select();
            }
        });
    }
    #[cfg(not(feature = "browser"))]
    {
        let _ = (editor_id, text, status);
    }
}

fn discussion_skeleton_visibility(loading: Signal<bool>) -> Signal<bool> {
    #[cfg(feature = "browser")]
    {
        use_loading_gate(loading, Clock::Browser).visible
    }
    #[cfg(not(feature = "browser"))]
    {
        let _ = loading;
        Signal::stored(false)
    }
}

#[component]
fn DiscussionSkeleton() -> impl IntoView {
    view! {
        <div class="vk-discussion-loading">
            <p role="status" class="vk-ui-visually-hidden">"Đang tải nhận xét…"</p>
            <div class="vk-discussion-skeleton-list grid gap-vk-3" aria-hidden="true">
                <Skeleton shape=SkeletonShape::Card />
                <Skeleton shape=SkeletonShape::Card />
            </div>
        </div>
    }
}

fn ask_remove(state: RwSignal<ViewState>, id: String) {
    let mut asked = false;
    state.update(|s| asked = s.own.ask_remove(&id));
    #[cfg(feature = "browser")]
    if asked {
        // The least destructive choice takes focus; Enter keeps the comment.
        focus_discussion_control(format!("cancel-remove-comment-{id}"));
    }
    #[cfg(not(feature = "browser"))]
    let _ = asked;
}

fn cancel_remove(state: RwSignal<ViewState>, id: String) {
    state.update(|s| s.own.cancel_remove());
    #[cfg(feature = "browser")]
    focus_discussion_control(format!("remove-comment-{id}"));
    #[cfg(not(feature = "browser"))]
    let _ = id;
}

/// A notification opens the exact published comment in its thread, including on later pages.
#[component]
pub(super) fn ReaderDiscussion(
    work_id: String,
    #[prop(optional)] read_only: bool,
) -> impl IntoView {
    let search = leptos_router::hooks::use_location().search;
    let target = Memo::new(move |_| {
        search.with(|search| {
            url::form_urlencoded::parse(search.trim_start_matches('?').as_bytes())
                .find(|(key, _)| key == "nhan-xet")
                .map(|(_, value)| value.into_owned())
        })
    });
    move || {
        if let Some(comment) = target.get() {
            view! { <LinkedComment work_id=work_id.clone() comment /> }.into_any()
        } else {
            view! { <Discussion work_id=work_id.clone() read_only /> }.into_any()
        }
    }
}

#[component]
fn LinkedComment(work_id: String, comment: String) -> impl IntoView {
    let show_all = format!("/tac-pham/{work_id}#feedback-heading");
    let data = LocalResource::new({
        let comment = comment.clone();
        move || {
            let (work, comment) = (work_id.clone(), comment.clone());
            async move {
                #[cfg(feature = "browser")]
                {
                    api::linked_public_comment(&work, &comment).await
                }
                #[cfg(not(feature = "browser"))]
                {
                    let _ = (work, comment);
                    Err::<crate::contract::PublicRootThread, _>(ApiError::NotFound)
                }
            }
        }
    });
    let anchor = format!("comment-{comment}");
    view! {
        <section id="feedback-region" class="vk-discussion grid min-w-0 gap-vk-3 [margin-block:var(--vk-space-8)]" aria-labelledby="feedback-heading">
            <Heading level=HeadingLevel::H2 role=TypeRole::Title id="feedback-heading" attr:tabindex="-1">"Nhận xét"</Heading>
            <Suspense fallback=|| view! { <crate::ui::PendingRead label="Đang mở nhận xét…" /> }>
                {move || {
                    let anchor = anchor.clone();
                    data.get().map(|result| match result {
                        Ok(root) => {
                            let root = crate::contract::RootThread::from(root);
                            let facts = adapters::discussion_thread(&root, &root.preview);
                            #[cfg(feature = "browser")]
                            super::focus_recovery(anchor);
                            #[cfg(not(feature = "browser"))]
                            let _ = anchor;
                            view! {
                                <DiscussionThread facts=Signal::stored(facts) read_only=Signal::stored(true)
                                    on_reply=Callback::new(|_: String| {}) on_remove=Callback::new(|_: String| {}) />
                            }.into_any()
                        }
                        Err(error) if !can_retry_discussion_read(&error) => {
                            #[cfg(feature = "browser")]
                            super::focus_recovery("feedback-heading");
                            view! { <p role="status">"Nhận xét này không còn khả dụng."</p> }.into_any()
                        }
                        Err(_) => view! {
                            <p role="alert">"Chưa mở được nhận xét."</p>
                            <DiscussionLoadRetry on_retry=Callback::new(move |()| data.refetch()) />
                        }.into_any(),
                    })
                }}
            </Suspense>
            <a class=ButtonVariant::Tonal.classes() href=show_all>"Xem toàn bộ nhận xét"</a>
        </section>
    }
}

#[component]
pub fn Discussion(work_id: String, #[prop(optional)] read_only: bool) -> impl IntoView {
    let resource = api::discussion_resource(work_id);
    let fixture = StoredValue::new(api::local_discussion_fixture_scope());
    // Reading the resource outside the nested Suspense also suspends the entire reader.
    // Keep its future mounted while the discussion-only gate shows its minimum interval.
    let initial_loading = RwSignal::new(true);
    let initial_skeleton = discussion_skeleton_visibility(initial_loading.into());
    view! {
        <section id="feedback-region" class="vk-discussion grid min-w-0 gap-vk-3 [margin-block:var(--vk-space-8)] [padding-block-start:var(--vk-space-3)]" aria-labelledby="feedback-heading"
            data-readonly=read_only.then_some("true")>
            <div class="vk-discussion-head flex flex-wrap items-baseline justify-between gap-x-vk-4 gap-y-vk-2 [&_.vk-ui-heading]:m-0">
                <Heading level=HeadingLevel::H2 role=TypeRole::Title id="feedback-heading" attr:tabindex="-1">"Nhận xét"</Heading>
            </div>
            {read_only.then(|| view! {
                <div class="vk-discussion-readonly-notice flex items-start gap-[12px] [padding:14px_16px] rounded-vk-lg bg-vk-surface-container-high text-vk-on-surface [&>.vk-ui-icon]:shrink-0 [&>.vk-ui-icon]:[margin-block-start:2px] [&>.vk-ui-icon]:text-vk-on-surface-variant"
                    role="note" data-testid="discussion-readonly-notice">
                    <Icon name=IconName::Lock size=IconSize::Small />
                    <div>
                        <strong class="block text-[length:var(--vk-type-label-size)] font-vk-role-section">"Trang này chỉ đọc"</strong>
                        <p class="[margin:2px_0_0] text-[length:var(--vk-type-caption-size)] text-vk-on-surface-variant">"Không gửi nhận xét mới hoặc bấm Ưng tại đây. Bạn vẫn đọc được nhận xét cũ và gỡ nhận xét của chính mình nếu còn quyền."</p>
                    </div>
                </div>
            })}
            <Show when=move || initial_skeleton.get()><DiscussionSkeleton /></Show>
            <div class="vk-discussion-load" hidden=move || initial_skeleton.get()>
            <Suspense fallback=|| view! { <p role="status" class="vk-ui-visually-hidden">"Đang tải nhận xét…"</p> }>
                {move || {
                    let fixture = fixture.get_value();
                    Suspend::new(async move {
                        let result = resource.await;
                        initial_loading.try_set(false);
                        match result {
                            Ok(load) => view! { <DiscussionLoaded page=load.page.into() endorsements=load.endorsements fixture read_only /> }.into_any(),
                            Err(LoadError::NotFound | LoadError::Invalid(_)) => view! {
                                <p role="status">"Nhận xét không còn khả dụng."</p>
                            }.into_any(),
                            Err(LoadError::Unavailable) => view! {
                                <div class="vk-discussion-state-card" data-tone="adverse" role="region" aria-label="Lỗi tải nhận xét">
                                    <Icon name=IconName::Error size=IconSize::Small />
                                    <div class="vk-discussion-state-card__body">
                                        <strong>"Lỗi tải nhận xét"</strong>
                                        <p role="status">"Chưa tải được nhận xét. Bạn vẫn có thể đọc tác phẩm."</p>
                                    </div>
                                    // A same-URL fragment link only scrolls and never re-runs the resource.
                                    <DiscussionLoadRetry on_retry=move |_| {
                                        if !initial_loading.get_untracked() {
                                            initial_loading.set(true);
                                            resource.refetch();
                                        }
                                    } />
                                </div>
                            }.into_any(),
                        }
                    })
                }}
            </Suspense>
            </div>
            <noscript><p>"Bật JavaScript để tải thêm phản hồi và soạn nhận xét. Các nhận xét công khai ở trên được máy chủ cung cấp."</p></noscript>
        </section>
    }
}

// Start false on both SSR and hydration; only a mounted client control can dispatch.
// Each control owner schedules its effect after rendering, preserving hydration parity.
fn client_action_ready() -> RwSignal<bool> {
    super::client_read_action_ready()
}

fn discussion_retry_state(ready: bool, busy: bool) -> ActionState {
    if busy && ready {
        ActionState::pending("Đang kiểm tra nhận xét…")
    } else {
        super::read_retry_state(ready)
    }
}

fn can_retry_discussion_read(error: &ApiError) -> bool {
    !matches!(
        error,
        ApiError::Unauthenticated | ApiError::Forbidden | ApiError::NotFound
    )
}

#[component]
fn DiscussionLoadRetry(#[prop(into)] on_retry: Callback<()>) -> impl IntoView {
    let ready = client_action_ready();
    let state = Signal::derive(move || discussion_retry_state(ready.get(), false));
    view! {
        <Button id="discussion-load-retry" variant=ButtonVariant::Tonal state on_press=on_retry>"Thử lại"</Button>
    }
}

#[component]
fn DiscussionLoaded(
    page: DiscussionPage,
    endorsements: Result<EndorsementSummaries, LoadError>,
    fixture: Option<String>,
    read_only: bool,
) -> impl IntoView {
    let client_ready = client_action_ready();
    #[cfg(feature = "browser")]
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
        #[cfg(feature = "browser")]
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
    // The Reader page provides the sign-in dialog for its composers and “Lưu” (#388).
    #[cfg(feature = "browser")]
    let reader_auth = ReaderAuth::current();
    #[cfg(feature = "browser")]
    let work_id = page.work_id.clone();
    #[cfg(feature = "browser")]
    let wake = crate::read_events::WakePolicy::confirmed(crate::read_events::now());
    #[cfg(not(feature = "browser"))]
    let wake = crate::read_events::WakePolicy::confirmed(0.0);
    let state = RwSignal::new(ViewState {
        own: OwnState::new(controller.get_untracked().scope),
        roots_read: crate::read_lifecycle::ReadOwner::new(controller.get_untracked().scope),
        roots_chain: initial_chain(&page),
        restarted_tail: false,
        wake,
        redacted: BTreeSet::new(),
        page,
        replies: BTreeMap::new(),
        reply_loading: BTreeMap::new(),
        reply_chains: BTreeMap::new(),
        reply_errors: BTreeMap::new(),
        loading: false,
        refresh_queued: false,
        error: None,
        page_error: None,
        unavailable: false,
        epoch: 0,
        local_fixture: fixture.is_some(),
        mutation: None,
        mutation_message: None,
        mutation_needs_refresh: false,
    });
    let refresh = Callback::new(move |()| {
        read_roots(state, controller, lve, false);
    });
    #[cfg(feature = "browser")]
    let fence_attribution = move || {
        state.update(ViewState::invalidate_attribution);
        state.with_untracked(|s| revalidate_targets(s, controller));
    };
    #[cfg(feature = "browser")]
    crate::read_events::observe_lifecycle(move |hint| {
        if hint == crate::read_events::Hint::HistoryRestored {
            fence_attribution();
        }
        if crate::read_events::visible()
            && state.try_with_untracked(|s| {
                s.wake.due(crate::read_events::now())
                    // The next Reader entry starts with a fresh head. A passive wake must
                    // not throw away a continuation the reader is still using.
                    && !s.has_paged_tail()
                    && s.mutation.is_none()
                    && (!s.loading
                        || s.roots_read.in_flight_intent()
                            == Some(crate::read_lifecycle::Intent::Append))
            }) == Some(true)
        {
            read_roots(state, controller, lve, false);
        }
    });
    #[cfg(feature = "browser")]
    super::profile::observe_changes(move || {
        fence_attribution();
        refresh.run(());
    });
    // Viewer-private projections only arrive after hydration through the no-store API.
    #[cfg(feature = "browser")]
    if fixture.is_some() {
        spawn_transport(async move {
            refresh.run(());
        });
    } else {
        // The tab's session owner decides who is acting. Each change re-reads the page for
        // that viewer; typed text stays only for the same person (`text_survives`).
        let session = Session::current();
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
            if scope.is_some() && reader_auth.is_some_and(ReaderAuth::is_open) {
                // Public roots/replies are actor-free. Keeping the current pagination chain
                // avoids remounting an open composer on page 2+ simply to read the new viewer.
                // Reconcile settings and private facts without restarting the public list.
                state.update(|s| s.wake.settled(crate::read_events::now(), true));
                read_private(state, controller, false);
            } else {
                refresh.run(());
            }
            (scope, principal.or(typed_as))
        });
    }
    // Wording only: the controller mirrors the server's author fact for “Bạn đã tắt…”.
    Effect::new(move |_| {
        let author = state.read().own.author;
        if controller.with_untracked(|c| c.viewer_is_author != author) {
            controller.update(|c| c.viewer_is_author = author);
        }
    });
    let write_state = Signal::derive(move || {
        if state.read().mutation.is_some() {
            ActionState::pending("Đang cập nhật…")
        } else if state.read().loading || state.read().unavailable {
            ActionState::disabled("Chờ nhận xét khả dụng trước khi thao tác.")
        } else if state.read().mutation_needs_refresh {
            ActionState::disabled("Kiểm tra trạng thái bình luận trước khi thao tác tiếp.")
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
    let mode_kind =
        Memo::new(move |_| comments_mode_kind(&state.read(), &controller.read(), read_only));
    let show_mode_surface = move || {
        let s = state.read();
        mode_kind.get() != "open" || s.own.author || s.local_fixture || s.mutation_message.is_some()
    };
    let retry_skeleton = discussion_skeleton_visibility(Signal::derive(move || {
        let state = state.read();
        state.loading && state.page.roots.is_empty()
    }));
    view! {
        <div id="comments-mode" class="vk-discussion-mode gap-vk-2 min-w-0"
            hidden=move || !show_mode_surface()
            data-mode-kind=move || mode_kind.get()
            aria-busy=move || (state.read().loading || state.read().mutation.is_some()).to_string()
            data-tone=move || (state.read().unavailable || state.read().error.is_some()).then_some("adverse")>
            <Show when=move || mode_kind.get() != "open" || (!read_only && state.read().own.author && !state.read().unavailable)>
            <div class="vk-discussion-mode__row flex flex-wrap items-center justify-between gap-x-vk-3 gap-y-vk-2 min-w-0">
                <Show when=move || mode_kind.get() != "open">
                <div class="vk-discussion-mode__status flex items-start gap-[10px] flex-[1_1_14rem] min-w-0 [&>.vk-ui-icon]:shrink-0 [&>.vk-ui-icon]:[margin-block-start:3px]">
                    {move || match mode_kind.get() {
                        "changing" => view! { <LoadingIndicator /> }.into_any(),
                        "conflict" => view! { <Icon name=IconName::Info size=IconSize::Small /> }.into_any(),
                        "author-closed" => view! { <Icon name=IconName::Lock size=IconSize::Small /> }.into_any(),
                        "system-locked" => view! { <Icon name=IconName::VerifiedUser size=IconSize::Small /> }.into_any(),
                        _ => ().into_any(),
                    }}
                    <div class="vk-discussion-mode__copy min-w-0">
                        <p role="status" class="m-0 wrap-anywhere [font-weight:var(--vk-type-label-weight)]">{move || match mode_kind.get() {
                            "changing" => "Đang cập nhật thiết lập bình luận…",
                            "system-locked" => "Hệ thống đang khóa bình luận",
                            "conflict" if !state.read().unavailable => "Chưa xác nhận được trạng thái bình luận",
                            "conflict" => "Thảo luận hiện không khả dụng. Bản đang soạn được giữ lại.",
                            "author-closed" => closed_comments_message(state.read().own.author),
                            _ => "",
                        }}</p>
                        {move || {
                            let s = state.read();
                            let c = controller.read();
                            if s.mutation_needs_refresh {
                                Some("Kiểm tra trạng thái mới nhất trước khi thao tác tiếp.".to_owned())
                            } else if (read_only || !c.settings.comments_open) && !s.unavailable {
                                Some("Nhận xét cũ vẫn hiển thị bên dưới.".to_owned())
                            } else {
                                None
                            }
                        }.map(|detail| view! { <p class="vk-muted m-0 wrap-anywhere text-[length:var(--vk-type-caption-size)]">{detail}</p> })}
                    </div>
                </div>
                </Show>
                <Show when=move || !read_only && !state.read().unavailable && state.read().own.author>
                    <Button id="discussion-mode-toggle" variant=ButtonVariant::Tonal state=write_state on_press=move |_| change_mode(state, controller, lve)>
                        {move || if controller.read().settings.comments_open { "Tắt bình luận" } else { "Mở bình luận" }}
                    </Button>
                </Show>
            </div>
            </Show>
            {fixture.is_some().then(|| view! { <p class="vk-muted vk-discussion-fixture-note">"Định danh thử trên thiết bị này · Không phải tài khoản thật."</p> })}
            <Show when=move || !state.read().unavailable && state.read().error.is_some()>
                <p role="status" class="vk-discussion-mode__alert">{move || state.read().error.clone().unwrap_or_default()}</p>
            </Show>
            <p role="status" class="vk-discussion-mode__alert vk-discussion-live"
                class:vk-ui-visually-hidden=move || state.read().mutation_message.is_none()>
                {move || state.read().mutation_message.clone().unwrap_or_default()}
            </p>
            <Show when=move || !state.read().unavailable && (state.read().error.is_some() || state.read().mutation_needs_refresh)>
                <Button id="discussion-read-retry" variant=ButtonVariant::Tonal
                    state=Signal::derive(move || discussion_retry_state(client_ready.get(), state.read().loading || state.read().mutation.is_some()))
                    on_press=move |_| refresh.run(())>
                    {move || if state.read().mutation_needs_refresh { "Kiểm tra trạng thái bình luận" } else { "Thử lại" }}
                </Button>
            </Show>
        </div>
        <Show when=move || !read_only>
            <DiscussionComposer key=ComposerKey::Root controller state refresh />
        </Show>
        <OwnComments state controller lve client_ready />
        <Show when=move || state.read().unavailable && !retry_skeleton.get()>
            <div class="vk-discussion-state-card" data-tone="adverse" role="region" aria-label="Lỗi tải nhận xét">
                <Icon name=IconName::Error size=IconSize::Small />
                <div class="vk-discussion-state-card__body">
                    <strong>"Nhận xét không còn khả dụng"</strong>
                    <p role="status">{move || state.read().error.clone().unwrap_or_default()}</p>
                </div>
            </div>
        </Show>
        <Show when=move || retry_skeleton.get()>
            <DiscussionSkeleton />
        </Show>
        <Show when=move || !state.read().unavailable && !retry_skeleton.get()>
            <Show when=move || !state.read().loading && shows_empty(&state.read())>
                <EmptyState compact=true hook="vk-discussion-empty" icon=IconName::ChatBubbleOutline announce=true
                    title="Chưa có nhận xét nào được hiển thị." />
            </Show>
            <Show when=move || state.read().restarted_tail>
                <p role="status" class="vk-muted" data-testid="discussion-chain-restarted">"Đã làm mới nhận xét. Các trang sau cần được tải lại."</p>
            </Show>
            <div id="root-list" data-order="oldest-first"
                data-has-more=move || has_more_roots(&state.read()).to_string()>
                <For each=move || { state.read().page.roots.iter().map(|root| adapters::root_id(root).to_owned()).collect::<Vec<_>>() } key=|id| id.clone()
                    children=move |root| view! { <RootThreadView root controller state refresh write_state lve read_only /> } />
            </div>
            <Show when=move || has_more_roots(&state.read())>
                <div id="root-pagination" class="vk-discussion-pagination justify-items-center gap-vk-2 my-vk-4 min-w-0">
                    <Show when=move || state.read().page_error.is_some()>
                        <div class="vk-discussion-inline-error" data-tone="adverse">
                            <Icon name=IconName::Error size=IconSize::Small />
                            <p role="status">{move || state.read().page_error.as_ref().map(|e| e.message).unwrap_or_default()}</p>
                        </div>
                    </Show>
                    <Button variant=ButtonVariant::Tonal state=Signal::derive(move || {
                        let s = state.read();
                        if s.loading { ActionState::pending("Đang tải…") }
                        else if s.mutation_needs_refresh { ActionState::disabled("Kiểm tra trạng thái bình luận trước.") }
                        else { ActionState::Enabled }
                    }) on_press=move |_| {
                        let append = !state.with_untracked(|s| s.page_error.as_ref().is_some_and(|e| e.restart));
                        read_roots(state, controller, lve, append);
                    }>{move || state.read().page_error.as_ref().map_or("Xem thêm nhận xét", PageReadFailure::action)}</Button>
                </div>
            </Show>
        </Show>
        <Show when=move || !read_only>
        <For each=move || {
                let s = state.read();
                controller.read().drafts.iter().filter_map(|(key, draft)| match key {
                    ComposerKey::Replies(root) if draft.needs_recovery() && !s.page.roots.iter().any(|thread| adapters::root_id(thread) == root) => Some(key.clone()),
                    _ => None,
                }).collect::<Vec<_>>()
            } key=|key| key.clone() children=move |key| {
                let key = StoredValue::new(key);
                view! {
                <section class="vk-discussion-recovered" aria-label="Bản trả lời được giữ lại">
                    <p>"Nhánh này chưa có trong danh sách đã tải. Trạng thái lần gửi và chữ đang soạn vẫn được giữ tại đây."</p>
                    <Show when=move || controller.read().draft(&key.get_value()).holds_work()
                        fallback=move || view! { <p role="status">{move || controller.read().draft(&key.get_value()).receipt.map(|r| r.message()).unwrap_or_default()}</p> }>
                        <DiscussionComposer key=key.get_value() controller state refresh blocked=true />
                    </Show>
                </section>
                }
            } />
        </Show>
    }
}

#[component]
fn OwnComments(
    state: RwSignal<ViewState>,
    controller: RwSignal<DiscussionController>,
    lve: Option<Endorsements>,
    client_ready: RwSignal<bool>,
) -> impl IntoView {
    let busy = Memo::new(move |_| {
        let s = state.read();
        s.loading || s.own.loading || s.own.capability_loading || s.mutation.is_some()
    });
    let visible_ids = Memo::new(move |_| {
        let s = state.read();
        s.own
            .visible_items(&s.public_ids())
            .into_iter()
            .map(|comment| comment.comment_id)
            .collect::<Vec<_>>()
    });
    let show_own = move || {
        let s = state.read();
        !s.unavailable
            && controller.read().scope.session.is_some()
            && (s.own.loading
                || s.own.error.is_some()
                || s.own.capability_error.is_some()
                || s.own.next_cursor.is_some()
                || visible_ids.with(|ids| !ids.is_empty()))
    };
    view! {
        <Show when=show_own>
            <section id="own-comments" class="vk-discussion-recovered" aria-labelledby="own-comments-heading"
                aria-busy=move || busy.get().to_string()>
                <div class="vk-discussion-own-head">
                    <Heading level=HeadingLevel::H3 role=TypeRole::Title id="own-comments-heading" attr:tabindex="-1">"Bạn đã gửi"</Heading>
                    <p class="vk-muted">"Chỉ bạn thấy trước khi được duyệt."</p>
                </div>
                <Show when=move || state.read().own.error.is_some() || state.read().own.capability_error.is_some()>
                    <div class="vk-discussion-inline-error flex flex-wrap items-center gap-vk-2" data-tone="adverse">
                        <p role="status">{move || {
                            let s = state.read();
                            s.own.error.as_ref().map_or("Chưa xác nhận được quyền thay đổi bình luận.", |e| crate::discussion_state::read_error_message(e, false))
                        }}</p>
                        <Show when=move || {
                            let s = state.read();
                            s.own.error.as_ref().or(s.own.capability_error.as_ref()).is_some_and(can_retry_discussion_read)
                        }>
                            <Button id="own-comments-retry" variant=ButtonVariant::Tonal
                                state=Signal::derive(move || discussion_retry_state(client_ready.get(), busy.get()))
                                on_press=move |_| read_private(state, controller, false)>"Thử lại"</Button>
                        </Show>
                    </div>
                </Show>
                <Show when=move || state.read().own.loading && visible_ids.with(Vec::is_empty)>
                    <crate::ui::PendingRead label="Đang tải nhận xét của bạn…" />
                </Show>
                <For each=move || visible_ids.get() key=|id| id.clone()
                    children=move |id| {
                        let id = StoredValue::new(id);
                        view! {
                            <article class="vk-discussion-delivery vk-discussion-own-card justify-items-start gap-vk-2 [padding:14px_16px] rounded-vk-lg min-w-0" data-own-comment=move || id.get_value()>
                                <div class="vk-discussion-own-card__meta flex flex-wrap items-center justify-between gap-vk-2 [inline-size:100%]">
                                    {move || state.read().own.comments.get(&id.get_value()).map(|c| OwnChip::of(c.status)).map(|chip| view! {
                                        <p role="status" class="vk-discussion-own-chip items-center gap-[6px] m-0 [padding:4px_10px_4px_8px] rounded-vk-full" data-tone=chip.tone().id()>
                                            <Icon name=own_chip_icon(chip) size=IconSize::Small />
                                            <span>{chip.label()}</span>
                                        </p>
                                    })}
                                    <p class="vk-muted">{move || state.read().own.comments.get(&id.get_value()).map(|c| match &c.placement {
                                        OwnPlacement::Root => "Nhận xét gốc của bạn",
                                        OwnPlacement::Reply { root_listed: false, .. } => "Phản hồi của bạn · Nhánh chưa có trong danh sách công khai",
                                        OwnPlacement::Reply { .. } => "Phản hồi của bạn",
                                    }).unwrap_or_default()}</p>
                                </div>
                                <p class="vk-discussion-own-body">{move || state.read().own.comments.get(&id.get_value()).and_then(|c| (c.status != CommentStatus::Removed).then(|| c.body.clone()).flatten()).unwrap_or_default()}</p>
                                {move || state.read().own.comments.get(&id.get_value())
                                    .and_then(|c| (c.status == CommentStatus::Rejected).then(|| c.rejection_reason.clone()).flatten())
                                    .map(|reason| view! {
                                        <div class="vk-discussion-own-rejection [inline-size:100%] box-border [padding:10px_12px] rounded-vk-md" data-tone="adverse">
                                            <span class="vk-discussion-own-rejection__label">"Lý do chỉ bạn thấy"</span>
                                            <p>{reason}</p>
                                        </div>
                                    })}
                                <Show when=move || state.read().own.can_remove(&id.get_value())>
                                    <div class="vk-discussion-own-card__actions flex flex-wrap items-center gap-vk-2 [inline-size:100%]">
                                        <OwnRemoveAction comment_id=id.get_value()
                                            confirming=Signal::derive(move || state.read().own.confirming(&id.get_value()))
                                            state=Signal::derive(move || if state.read().mutation.is_some() { ActionState::pending("Đang gỡ…") } else { ActionState::Enabled })
                                            on_ask=move |_| ask_remove(state, id.get_value())
                                            on_confirm=move |_| remove_own(state, controller, lve, id.get_value())
                                            on_cancel=move |_| cancel_remove(state, id.get_value()) />
                                    </div>
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
    #[cfg(feature = "browser")]
    {
        let scope = controller.get_untracked().scope;
        if scope.session.is_none() || state.read_untracked().own.capability_loading {
            return;
        }
        state.update(|s| {
            s.own.author = false;
            s.own.capability_loading = true;
        });
        spawn_private(true, async move {
            let result = api::load_discussion_capability(&scope.work_id).await;
            if controller
                .try_get_untracked()
                .is_none_or(|c| c.scope != scope)
            {
                return;
            }
            state.try_update(|s| {
                s.own.capability_loading = false;
                s.own.capability_error = None;
                match result {
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
                        s.own.author = c.viewer_is_work_author;
                        if let Some(settings) = c.settings
                            && settings.generation >= s.page.settings.generation
                        {
                            s.page.settings = settings.clone();
                            controller.update(|c| c.settings = settings);
                        }
                    }
                    Ok(c) if c.work_id != scope.work_id => {
                        s.own.capability_error = Some(ApiError::Unexpected { status: 200 });
                    }
                    Ok(_) => {}
                    Err(error) => {
                        s.own.capability_error = Some(error);
                    }
                }
            });
        });
    }
}

fn read_own(state: RwSignal<ViewState>, controller: RwSignal<DiscussionController>, append: bool) {
    #[cfg(feature = "browser")]
    {
        let cursor = if append {
            state.read_untracked().own.next_cursor.clone()
        } else {
            None
        };
        if append && (cursor.is_none() || state.read_untracked().own.loading) {
            return;
        }
        let Some(Some(ticket)) = state.try_update(|s| s.own.begin_page(append)) else {
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
                        if !s.own.apply_list(&ticket, page, append) {
                            s.own.finish(
                                &ticket,
                                Some(crate::contract::ApiError::Unexpected { status: 200 }),
                            );
                            return;
                        }
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
    #[cfg(not(feature = "browser"))]
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
    read_only: bool,
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
            #[cfg(feature = "browser")]
            focus_discussion_control(format!("reply-composer-{}", root.get_value()));
        }
    });
    let on_remove = Callback::new(move |id: String| ask_remove(state, id));
    let removal = RemoveConfirmation {
        confirming: Callback::new(move |id: String| state.read().own.confirming(&id)),
        on_confirm: Callback::new(move |id: String| remove_own(state, controller, lve, id)),
        on_cancel: Callback::new(move |id: String| cancel_remove(state, id)),
    };
    // The root as a reply target, only while the provider lets this viewer reply to it.
    let root_target = Signal::derive(move || {
        facts
            .get()
            .root
            .filter(|root| root.can_reply)
            .map(|root| Target {
                id: root.id,
                label: root.author.name,
            })
    });
    let reselect_id = Memo::new(move |_| {
        let thread = facts.get();
        thread
            .root
            .into_iter()
            .chain(thread.replies)
            .find(|comment| comment.can_reply)
            .map(|comment| comment.id)
    });
    let reply_expanded = Signal::derive(move || {
        !read_only
            && controller
                .read()
                .drafts
                .get(&key.get_value())
                .is_some_and(|draft| draft.open)
    });
    let reply_target_id = Signal::derive(move || {
        if !reply_expanded.get() {
            return None;
        }
        controller
            .read()
            .drafts
            .get(&key.get_value())?
            .target
            .as_ref()
            .map(|target| target.id.clone())
    });
    view! {
        <section id=format!("thread-{}", root.get_value()) class="vk-discussion-root" aria-label="Nhánh nhận xét" tabindex="-1">
            <DiscussionThread facts on_reply on_remove removal action_state=write_state endorsements=lve.map(Endorsements::comments)
                read_only
                reply_expanded
                reply_target_id
                has_more_replies=Signal::derive(move || next_reply_cursor(&state.read(), &root.get_value()).is_some())
                reply_footer=std::sync::Arc::new(move || view! {
            <div class="vk-discussion-pagination justify-items-center gap-vk-2 my-vk-4 min-w-0" hidden=move || next_reply_cursor(&state.read(), &root.get_value()).is_none() && !state.read().reply_errors.contains_key(&root.get_value())>
            <Show when=move || state.read().reply_errors.contains_key(&root.get_value())>
                <div class="vk-discussion-inline-error" data-tone="adverse">
                    <Icon name=IconName::Error size=IconSize::Small />
                    <p role="status">{move || state.read().reply_errors.get(&root.get_value()).map(|e| e.message).unwrap_or_default()}</p>
                </div>
            </Show>
            <Show when=move || next_reply_cursor(&state.read(), &root.get_value()).is_some()>
                <Button variant=ButtonVariant::Text state=Signal::derive(move || if state.read().reply_loading.get(&root.get_value()) == Some(&true) { ActionState::pending("Đang tải phản hồi…") } else { ActionState::Enabled })
                    on_press=move |_| {
                        let root = root.get_value();
                        if state.with_untracked(|s| s.reply_errors.get(&root).is_some_and(|e| e.restart)) {
                            refresh.run(());
                        } else {
                            read_replies(state, controller, lve, root);
                        }
                    }>{move || state.read().reply_errors.get(&root.get_value()).map_or("Xem thêm phản hồi", PageReadFailure::action)}</Button>
            </Show>
            </div>
                }.into_any()) />
            <Show when=move || !read_only && controller.read().collapsed_with_work(&key.get_value())>
                <div class="vk-discussion-reply-kept">
                    <p role="status">"Bản trả lời được giữ lại"</p>
                    <Button id=format!("reopen-reply-{}", root.get_value()) variant=ButtonVariant::Text
                        on_press=move |_| {
                        controller.update(|c| c.reopen(&key.get_value()));
                        #[cfg(feature = "browser")]
                        focus_discussion_control(format!("reply-composer-{}", root.get_value()));
                    }>"Mở lại"<span class="vk-ui-visually-hidden">" bình luận"</span></Button>
                </div>
            </Show>
            <Show when=move || !read_only && controller.read().drafts.contains_key(&key.get_value())>
                <div class="vk-discussion-reply-composer" hidden=move || !reply_expanded.get()>
                    <DiscussionComposer key=key.get_value() controller state refresh root_target reselect_id />
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
    /// The branch root as a target, when it accepts replies and the reader selects it again.
    #[prop(optional, into)]
    root_target: Signal<Option<Target>>,
    /// First visible, eligible reply action in this branch (an anchor has no root action).
    #[prop(optional, into)]
    reselect_id: Signal<Option<String>>,
) -> impl IntoView {
    let key = StoredValue::new(key);
    let id = match key.get_value() {
        ComposerKey::Root => "root-composer".into(),
        ComposerKey::Replies(root) => format!("reply-composer-{root}"),
    };
    let value = Memo::new(move |_| controller.read().draft(&key.get_value()).body);
    let editor_id = StoredValue::new(id.clone());
    let session = Session::current();
    let host = expect_context::<ReaderAuth>();
    let send_intent = move || {
        let s = state.read();
        let c = controller.read();
        let owner_ready = s.local_fixture
            || session
                .profile_scope
                .with(|scope| scope.is_some() && *scope == c.scope.session);
        let intent = session
            .standing
            .with(|standing| c.send_intent(&key.get_value(), standing, owner_ready));
        if intent != SendIntent::Pending
            && (blocked
                || s.unavailable
                || s.loading
                || s.mutation.is_some()
                || s.mutation_needs_refresh)
        {
            SendIntent::Blocked("Chờ tải lại nhận xét và chọn ngữ cảnh trước khi gửi.")
        } else {
            intent
        }
    };
    let copy_status = RwSignal::new(Option::<(bool, &'static str)>::None);
    let is_closed = Memo::new(move |_| !controller.read().settings.comments_open);
    let closed_actions = ViewFn::from(move || {
        view! {
        <div class="vk-discussion-closed-actions">
            <span class="vk-muted">"Chỉ đọc lúc này"</span>
            <Show when=move || !value.get().is_empty()>
                <Button variant=ButtonVariant::Tonal leading=IconName::ContentCopy
                    on_press=move |_| copy_draft_text(editor_id.get_value(), value.get_untracked(), copy_status)>
                    "Sao chép nháp"
                </Button>
            </Show>
            {move || copy_status.get().map(|(ok, msg)| view! {
                <span role="status" class="vk-discussion-copy-status" data-copied=ok.then_some("true")>{msg}</span>
            })}
        </div>
    }.into_any()
    });
    let field_mode = Signal::derive(move || {
        if is_closed.get() {
            FieldMode::ReadOnly
        } else {
            FieldMode::Editable
        }
    });
    let action = Signal::derive(move || match send_intent() {
        SendIntent::Pending => ActionState::pending("Đang gửi…"),
        SendIntent::Empty => ActionState::Disabled { reason: None },
        SendIntent::Blocked(reason) => ActionState::disabled(reason),
        SendIntent::Authenticate | SendIntent::Submit => ActionState::Enabled,
    });
    // R4: tonal while empty, filled with text; progress is the pending state of the button.
    let send_variant =
        Signal::derive(
            move || match controller.read().send_emphasis(&key.get_value()) {
                SendEmphasis::Tonal => ButtonVariant::Tonal,
                SendEmphasis::Filled | SendEmphasis::Progress => ButtonVariant::Filled,
            },
        );
    let send_icon = Signal::derive(move || {
        if controller.read().status(&key.get_value()) == ComposerStatus::SignInClosed {
            IconName::Lock
        } else {
            IconName::Send
        }
    });
    let is_reply = matches!(key.get_value(), ComposerKey::Replies(_));
    let reply_target = Memo::new(move |_| {
        let c = controller.read();
        let target = c.drafts.get(&key.get_value())?;
        target
            .target
            .as_ref()
            .filter(|_| target.target_available)
            .map(|target| target.label.clone())
    });
    let close_id = format!("{id}-close");
    let context_id = format!("{id}-target");
    view! {
        // The closed reason comes from the controller, which words it for the work's author.
        <div class="vk-discussion-composer" data-status=move || controller.read().status(&key.get_value()).id()>
            <FeedbackComposer id intent=FeedbackIntent::Unspecified value state=action tonal=true
                field_label=if is_reply { "Viết phản hồi" } else { "Viết nhận xét" }
                field_description=is_reply.then_some(context_id.clone())
                comments_closed=is_closed field_mode
                show_alternative=is_closed alternative=closed_actions send_variant send_icon
                on_input=move |input: TextInput| {
                    copy_status.set(None);
                    controller.update(|c| c.edit(&key.get_value(), input.value, input.composing));
                }
                on_submit=move |_| match untrack(send_intent) {
                    SendIntent::Authenticate => host.open_for_comment(editor_id.get_value(), controller, state),
                    SendIntent::Submit => submit(controller, state, key.get_value(), refresh, false),
                    _ => {}
                } >
                {is_reply.then(|| {
                    let recovery_context_id = context_id.clone();
                    view! {
                    <div class="vk-discussion-composer-heading grid grid-cols-[minmax(0,1fr)_auto] items-center gap-x-vk-2 min-w-0">
                        <div class="vk-discussion-reply-target" data-target=move || match controller.read().reply_target(&key.get_value()) {
                            Some(ReplyTarget::Root) => "root",
                            Some(ReplyTarget::Reply { .. }) => "reply",
                            _ => "unavailable",
                        }>
                            <Show when=move || reply_target.with(Option::is_some)>
                                <p id=context_id.clone()>{move || reply_target.get().map(|name| format!("Trả lời {name}"))}</p>
                            </Show>
                            <Show when=move || matches!(controller.read().reply_target(&key.get_value()), Some(ReplyTarget::Reply { .. })) && root_target.get().is_some()>
                                <Button variant=ButtonVariant::Text on_press=move |_| {
                                    let ComposerKey::Replies(root) = key.get_value() else { return };
                                    if let Some(target) = root_target.get_untracked() {
                                        controller.update(|c| c.reply(root.clone(), target));
                                        #[cfg(feature = "browser")]
                                        focus_discussion_control(format!("reply-composer-{root}"));
                                    }
                                }>"Về nhận xét gốc"</Button>
                            </Show>
                        </div>
                        {(!blocked).then(|| view! {
                            <IconButton id=close_id icon=IconName::Close label="Đóng ô trả lời, giữ nháp" on_press=move |_| {
                                controller.update(|c| c.close_reply(&key.get_value()));
                                #[cfg(feature = "browser")]
                                if let ComposerKey::Replies(root) = key.get_value() {
                                    let c = controller.read_untracked();
                                    let draft = c.drafts.get(&key.get_value());
                                    let focus = draft.and_then(|draft| draft.target.as_ref().filter(|_| draft.target_available))
                                        .map(|target| format!("reply-to-{}", target.id))
                                        .or_else(|| c.collapsed_with_work(&key.get_value()).then(|| format!("reopen-reply-{root}")))
                                        .or_else(|| reselect_id.get_untracked().map(|id| format!("reply-to-{id}")))
                                        .unwrap_or_else(|| format!("thread-{root}"));
                                    focus_discussion_control(focus);
                                }
                            } />
                        })}
                    </div>
                    <Show when=move || reply_target.with(Option::is_none)>
                        <div class="vk-discussion-target-lost" data-tone="waiting">
                            <div class="vk-discussion-target-lost__head">
                                <Icon name=IconName::Warning size=IconSize::Small />
                                <div role="status">
                                    <strong id=recovery_context_id.clone()>"Bình luận được trả lời không còn hiển thị"</strong>
                                    <p>"Nháp của bạn vẫn còn bên dưới. Bạn có thể sao chép nháp trong khi chưa gửi được vào bình luận cũ."</p>
                                </div>
                            </div>
                            <div class="vk-discussion-target-lost__actions">
                                <Button variant=ButtonVariant::Tonal leading=IconName::ContentCopy
                                    on_press=move |_| copy_draft_text(editor_id.get_value(), value.get_untracked(), copy_status)>"Sao chép nháp"</Button>
                                <Show when=move || reselect_id.get().is_some()>
                                    <Button variant=ButtonVariant::Filled on_press=move |_| {
                                        #[cfg(feature = "browser")]
                                        if let Some(id) = reselect_id.get_untracked() {
                                            focus_discussion_control(format!("reply-to-{id}"));
                                        }
                                    }>"Chọn bình luận khác"</Button>
                                </Show>
                                {move || copy_status.get().map(|(ok, msg)| view! {
                                    <span role="status" class="vk-discussion-copy-status" data-copied=ok.then_some("true")>{msg}</span>
                                })}
                            </div>
                        </div>
                    </Show>
                    }
                })}
                <Show when=move || is_closed.get() && matches!(key.get_value(), ComposerKey::Replies(_))>
                    <div class="vk-discussion-closed-banner" data-tone="neutral" role="status">
                        <Icon name=IconName::Lock size=IconSize::Small />
                        <div>
                            <strong>{move || closed_comments_message(controller.read().viewer_is_author)}</strong>
                            <p>"Nhận xét cũ vẫn hiển thị bên dưới. Bạn vẫn có thể sao chép đoạn đã soạn để giữ lại."</p>
                        </div>
                    </div>
                </Show>
            </FeedbackComposer>
            <div class="vk-discussion-delivery justify-items-start gap-vk-2 [padding:14px_16px] rounded-vk-lg min-w-0" hidden=move || controller.read().draft(&key.get_value()).receipt.is_none() && !matches!(controller.read().draft(&key.get_value()).delivery, Delivery::Uncertain(_))
                data-tone=move || {
                    let draft = controller.read().draft(&key.get_value());
                    if matches!(draft.delivery, Delivery::Uncertain(_)) { NoticeTone::Unknown.id() } else { draft.receipt.map_or(NoticeTone::Neutral, |r| r.tone()).id() }
                }>
            <div class="vk-discussion-delivery__head flex items-start gap-[8px] min-w-0">
                {move || {
                    let draft = controller.read().draft(&key.get_value());
                    let tone = if matches!(draft.delivery, Delivery::Uncertain(_)) {
                        NoticeTone::Unknown
                    } else {
                        draft.receipt.map_or(NoticeTone::Neutral, |r| r.tone())
                    };
                    view! { <Icon name=notice_icon(tone) size=IconSize::Small /> }
                }}
                <p role="status" data-testid="discussion-receipt">{move || {
                    let draft = controller.read().draft(&key.get_value());
                    if matches!(draft.delivery, Delivery::Uncertain(_)) { "Chưa rõ kết quả gửi" } else { draft.receipt.map(|r| r.message()).unwrap_or_default() }
                }}</p>
            </div>
            <Show when=move || matches!(controller.read().draft(&key.get_value()).delivery, Delivery::Uncertain(_))>
                <p>"Dùng lại nội dung lần trước, giữ nháp."</p>
                <Button variant=ButtonVariant::Filled state=Signal::derive(move || {
                    if !controller.read().write_ready() { ActionState::disabled("Máy chủ chưa xác nhận phiên đăng nhập; tải lại để đối chiếu. Chữ vẫn được giữ.") }
                    else if state.read().loading || state.read().mutation.is_some() || state.read().mutation_needs_refresh { ActionState::disabled("Chờ tải lại nhận xét trước khi đối soát.") }
                    else { ActionState::Enabled }
                }) on_press=move |_| submit(controller, state, key.get_value(), refresh, true)>"Kiểm tra lần gửi"</Button>
            </Show>
            </div>
        </div>
    }
}

#[cfg(feature = "browser")]
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

#[cfg(any(feature = "browser", test))]
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

#[cfg(feature = "browser")]
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
    #[cfg(feature = "browser")]
    {
        if state.read_untracked().mutation.is_some() {
            return;
        }
        let scope = controller.get_untracked().scope;
        let cursor = if append {
            state.get_untracked().page.next_cursor
        } else {
            None
        };
        if append && (cursor.is_none() || state.read_untracked().mutation_needs_refresh) {
            return;
        }
        let Some(Some(ticket)) = state.try_update(|s| {
            s.roots_read.begin(
                if append {
                    crate::read_lifecycle::Intent::Append
                } else {
                    crate::read_lifecycle::Intent::Refresh
                },
                cursor.clone(),
            )
        }) else {
            return;
        };
        let mut epoch = 0;
        state.update(|s| {
            if !append {
                s.wake.started();
            }
            s.loading = true;
            s.refresh_queued = false;
            s.reply_loading.clear();
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
                if !s.accepts_read(epoch) || !s.roots_read.finish(&ticket) {
                    return;
                }
                s.loading = false;
                refresh_again = s.refresh_queued;
                s.refresh_queued = false;
                let had_tail = s.has_paged_tail();
                match result {
                    Ok(page)
                        if page.work_id == scope.work_id
                            && s.roots_chain.accept(
                                &ticket,
                                page.roots.iter().map(|r| adapters::root_id(r).to_owned()),
                                page.next_cursor.clone(),
                            ) =>
                    {
                        s.restarted_tail = !append && had_tail;
                        endorsable = Some(page_targets(&page));
                        controller.update(|c| c.settings = page.settings.clone());
                        s.unavailable = false;
                        if !append {
                            s.error = None;
                        }
                        s.mutation_needs_refresh = false;
                        // Invalidate old per-root reads and context projections on a refresh.
                        s.reply_loading.clear();
                        if append {
                            s.page.roots.extend(page.roots);
                            s.page.next_cursor = page.next_cursor;
                            s.page.viewer_is_work_author = page.viewer_is_work_author;
                            s.page.settings = page.settings;
                        } else {
                            s.page = page;
                            s.replies.clear();
                            s.reply_chains.clear();
                            s.reply_errors.clear();
                        }
                        s.redact();
                        revalidate_targets(s, controller);
                    }
                    Err(error) => {
                        s.fail_roots_read(append, &error);
                        if s.unavailable {
                            revalidate_targets(s, controller);
                        }
                    }
                    _ => {
                        let message = "Phản hồi không khớp tác phẩm; chưa cập nhật.".to_owned();
                        if append {
                            s.page_error = Some(PageReadFailure::RESTART);
                        } else {
                            s.error = Some(message);
                        }
                    }
                }
                if !append {
                    s.wake
                        .settled(crate::read_events::now(), endorsable.is_some());
                }
            });
            // Fresh counts and own states for the comments this read returned.
            if let (Some(lve), Some(targets)) = (lve, endorsable) {
                lve.read(targets, true);
            }
            if state.try_with_untracked(|s| !s.unavailable && s.accepts_read(epoch)) == Some(true) {
                read_private(state, controller, false);
            }
            if refresh_again {
                read_roots(state, controller, lve, false);
            }
        });
    }
    #[cfg(not(feature = "browser"))]
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
    #[cfg(feature = "browser")]
    {
        let before = state.get_untracked();
        if before.loading
            || before.mutation.is_some()
            || before.reply_loading.get(&root) == Some(&true)
        {
            return;
        }
        let Some(cursor) = next_reply_cursor(&before, &root) else {
            return;
        };
        let epoch = before.epoch;
        let scope = controller.get_untracked().scope;
        // Per-root single-flight is guarded by reply_loading; the page ticket additionally
        // binds the input cursor and scope, while epoch fences a head refresh or mutation.
        let mut read = crate::read_lifecycle::ReadOwner::new((scope.clone(), root.clone()));
        let ticket = read
            .begin(crate::read_lifecycle::Intent::Append, Some(cursor.clone()))
            .unwrap();
        state.update(|s| {
            s.reply_chains.entry(root.clone()).or_insert_with(|| {
                let mut chain = crate::read_lifecycle::PageChain::default();
                let head = crate::read_lifecycle::ReadOwner::new(())
                    .begin(crate::read_lifecycle::Intent::Initial, None)
                    .unwrap();
                let ids = s
                    .page
                    .roots
                    .iter()
                    .find(|r| adapters::root_id(r) == root)
                    .into_iter()
                    .flat_map(|r| r.preview.iter().map(|c| c.comment_id.clone()));
                chain.accept(&head, ids, Some(cursor.clone()));
                chain
            });
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
                        if !s.reply_chains.get_mut(&root).is_some_and(|chain| {
                            chain.accept(
                                &ticket,
                                page.replies.iter().map(|c| c.comment_id.clone()),
                                page.next_cursor.clone(),
                            )
                        }) {
                            s.reply_errors
                                .insert(root.clone(), PageReadFailure::RESTART);
                            return;
                        }
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
                        s.reply_errors
                            .insert(root, PageReadFailure::from_read(&error));
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
    #[cfg(not(feature = "browser"))]
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
    #[cfg(feature = "browser")]
    {
        if state.read_untracked().unavailable
            || state.read_untracked().loading
            || state.read_untracked().mutation.is_some()
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
        let session = Session::current();
        spawn_transport(async move {
            let result = api::submit_comment(&submission.scope.work_id, &submission.request).await;
            let unauthenticated = matches!(result, Err(ApiError::Unauthenticated));
            let mut accepted = false;
            controller.try_update(|c| accepted = c.complete(&submission, result));
            if accepted {
                if unauthenticated && !state.read_untracked().local_fixture {
                    session.fence_profile();
                }
                refresh.run(());
            }
        });
    }
    #[cfg(not(feature = "browser"))]
    {
        let _ = (controller, state, key, refresh, reconcile);
    }
}

fn change_mode(
    state: RwSignal<ViewState>,
    controller: RwSignal<DiscussionController>,
    lve: Option<Endorsements>,
) {
    #[cfg(feature = "browser")]
    {
        let c = controller.get_untracked();
        let s = state.get_untracked();
        if !c.write_ready()
            || !s.own.author
            || s.mutation.is_some()
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
        state.update(|s| started = s.begin_mutation(DiscussionMutation::ChangeMode));
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
                s.mutation = None;
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
    #[cfg(not(feature = "browser"))]
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
    #[cfg(feature = "browser")]
    {
        let c = controller.get_untracked();
        let s = state.get_untracked();
        let allowed = s.own.confirming(&id);
        if !allowed
            || !c.write_ready()
            || s.mutation.is_some()
            || s.mutation_needs_refresh
            || s.loading
        {
            return;
        }
        // Only a confirmed, still removable comment is sent, once (VDR-DISCUSSION-R4@r1 §2).
        let mut started = false;
        state.update(|s| {
            started = s.own.take_confirmed(&id) && s.begin_mutation(DiscussionMutation::RemoveOwn)
        });
        if !started {
            return;
        }
        focus_discussion_control(
            if document()
                .get_element_by_id("own-comments-heading")
                .is_some()
            {
                "own-comments-heading"
            } else {
                "feedback-heading"
            }
            .into(),
        );
        spawn_transport(async move {
            let result = api::remove_comment(&c.scope.work_id, &id).await;
            if controller
                .try_get_untracked()
                .is_none_or(|current| current.scope != c.scope)
            {
                return;
            }
            state.try_update(|s| {
                s.mutation = None;
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
    #[cfg(not(feature = "browser"))]
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
        assert!(!controller.draft(&key).needs_recovery());
        controller.reply(
            "page-two-root".into(),
            Target {
                id: "page-two-root".into(),
                label: "Tác giả mẫu".into(),
            },
        );
        controller.edit(&key, "Nhận xét ở trang thứ hai".into(), false);
        let request = controller.begin(&key, "off-page-command".into()).unwrap();
        assert!(controller.draft(&key).needs_recovery());
        assert!(controller.draft(&key).holds_work());
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
        assert_eq!(
            retained.receipt,
            Some(crate::discussion_state::Receipt::Pending)
        );
        assert!(retained.needs_recovery());
        assert!(!retained.holds_work());
        controller.edit(&key, "Một lần gửi khác".into(), false);
        assert!(
            controller
                .begin(&key, "second-off-page-command".into())
                .is_some()
        );
        controller.edit(&key, String::new(), false);
        let unsettled = controller.draft(&key);
        assert!(unsettled.receipt.is_none());
        assert!(unsettled.needs_recovery());
        assert!(unsettled.holds_work());
    }
    #[test]
    fn closed_copy_follows_the_servers_author_fact() {
        assert_eq!(
            closed_comments_message(false),
            "Tác giả đã tắt bình luận cho bài viết này"
        );
        assert_eq!(
            closed_comments_message(true),
            "Bạn đã tắt bình luận cho bài viết này"
        );
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
            roots_read: crate::read_lifecycle::ReadOwner::new(Scope {
                work_id: page.work_id.clone(),
                session: None,
            }),
            roots_chain: initial_chain(&page),
            restarted_tail: false,
            wake: crate::read_events::WakePolicy::default(),
            redacted: BTreeSet::new(),
            page,
            replies: BTreeMap::new(),
            reply_loading: BTreeMap::new(),
            reply_chains: BTreeMap::new(),
            reply_errors: BTreeMap::new(),
            loading: false,
            refresh_queued: false,
            error: None,
            page_error: None,
            unavailable: false,
            epoch: 0,
            local_fixture: false,
            mutation: None,
            mutation_message: None,
            mutation_needs_refresh: false,
        };
        let loaded = state.page.roots.len();
        state.fail_roots_read(true, &ApiError::Unavailable);
        assert!(
            state.error.is_none(),
            "region status must not carry a next-page failure"
        );
        let local = state.page_error.as_ref().unwrap().message;
        assert!(local.contains("thử lại") && !local.contains("chưa rõ kết quả"));
        state.fail_roots_read(
            true,
            &ApiError::Invalid {
                field: Some("cursor".into()),
            },
        );
        assert!(state.page_error.as_ref().unwrap().restart);
        assert_eq!(state.page_error.as_ref().unwrap().action(), "Tải từ đầu");
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
            roots_read: crate::read_lifecycle::ReadOwner::new(Scope {
                work_id: page.work_id.clone(),
                session: None,
            }),
            roots_chain: initial_chain(&page),
            restarted_tail: false,
            wake: crate::read_events::WakePolicy::default(),
            redacted: BTreeSet::new(),
            page,
            replies: BTreeMap::new(),
            reply_loading: BTreeMap::new(),
            reply_chains: BTreeMap::new(),
            reply_errors: BTreeMap::new(),
            loading: false,
            refresh_queued: false,
            error: None,
            page_error: None,
            unavailable: false,
            epoch: 0,
            local_fixture: false,
            mutation: None,
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
        assert!(state.begin_mutation(DiscussionMutation::ChangeMode));
        assert!(!state.accepts_read(before_mutation));
        assert!(!state.begin_mutation(DiscussionMutation::ChangeMode));
        state.mutation = None;
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
            roots_read: crate::read_lifecycle::ReadOwner::new(Scope {
                work_id: public.work_id.clone(),
                session: Some("a/0".into()),
            }),
            roots_chain: initial_chain(&public.clone().into()),
            restarted_tail: false,
            wake: crate::read_events::WakePolicy::default(),
            redacted: BTreeSet::new(),
            page: public.into(),
            replies: BTreeMap::new(),
            reply_loading: BTreeMap::new(),
            reply_chains: BTreeMap::new(),
            reply_errors: BTreeMap::new(),
            loading: false,
            refresh_queued: false,
            error: None,
            page_error: None,
            unavailable: false,
            epoch: 0,
            local_fixture: false,
            mutation: None,
            mutation_message: None,
            mutation_needs_refresh: false,
        }
    }
    #[test]
    fn mode_only_reports_the_confirmed_policy_or_operation() {
        let mut state = state();
        let mut controller =
            DiscussionController::new(state.own.scope.clone(), state.page.settings.clone());
        controller.settings.comments_open = true;
        for author in [false, true] {
            state.own.author = author;
            assert_eq!(comments_mode_kind(&state, &controller, false), "open");
        }
        state.mutation = Some(DiscussionMutation::RemoveOwn);
        assert_eq!(comments_mode_kind(&state, &controller, false), "open");
        state.mutation = Some(DiscussionMutation::ChangeMode);
        assert_eq!(comments_mode_kind(&state, &controller, false), "changing");
        state.mutation = None;
        state.mutation_needs_refresh = true;
        assert_eq!(comments_mode_kind(&state, &controller, false), "conflict");
        state.mutation_needs_refresh = false;
        state.unavailable = true;
        assert_eq!(comments_mode_kind(&state, &controller, false), "conflict");
        state.unavailable = false;
        controller.settings.comments_open = false;
        for author in [true, false] {
            state.own.author = author;
            assert_eq!(
                comments_mode_kind(&state, &controller, false),
                "author-closed"
            );
            assert_eq!(
                comments_mode_kind(&state, &controller, true),
                "system-locked"
            );
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
        assert!(!s.loading && !s.refresh_queued && s.mutation.is_none() && !s.own.author);
        assert!(!s.accepts_read(old));
        assert_eq!(s.page, public, "actor-free data survives account switching");
        assert!(
            s.begin_mutation(DiscussionMutation::ChangeMode),
            "new account is not blocked by old loading"
        );
        let old = s.epoch;
        s.change_session(Scope {
            work_id: s.page.work_id.clone(),
            session: Some("a/2".into()),
        });
        assert!(s.mutation.is_none() && s.mutation_message.is_none());
        assert!(!s.accepts_read(old));
        assert!(
            s.begin_mutation(DiscussionMutation::ChangeMode),
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
    #[test]
    fn r4_a_failed_read_is_never_shown_as_empty_and_pagination_follows_the_cursor() {
        use crate::contract::ApiError;
        let mut s = state();
        assert!(!s.page.roots.is_empty());
        s.page.next_cursor = Some("next".into());
        assert!(!shows_empty(&s) && has_more_roots(&s), "more pages remain");
        s.page.next_cursor = None;
        assert!(!has_more_roots(&s) && !shows_empty(&s));
        s.page.roots.clear();
        assert!(
            shows_empty(&s) && !has_more_roots(&s),
            "empty is not an end marker"
        );
        s.fail_roots_read(false, &ApiError::Network);
        assert!(!shows_empty(&s), "a failed refresh is not “no comments”");
        s.error = None;
        s.fail_roots_read(true, &ApiError::NotFound);
        assert!(s.unavailable && !shows_empty(&s) && !has_more_roots(&s));
    }
}
