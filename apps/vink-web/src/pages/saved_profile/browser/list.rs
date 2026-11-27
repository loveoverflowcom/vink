use std::collections::HashMap;

use leptos::{prelude::*, task::spawn_local};
use vink_ui::{
    ActionState, ActionSurface, Button, ButtonVariant, Icon, IconButton, IconName, Skeleton,
};
use wasm_bindgen::{JsCast, prelude::*};

use crate::{
    account::Session,
    api,
    contract::{ApiError, SavedWorkRow, SavedWorkState, SavedWorksPage},
    read_lifecycle::{Intent, PageChain, ReadOwner, ReadTicket},
    ui::{adapters::author_name, empty_state::EmptyState},
};

use super::VerifiedOwner;

const HEADING: &str = "saved-heading";
const MAX_PAGES: usize = 500;

#[wasm_bindgen(module = "/profile/navigation.js")]
extern "C" {
    #[wasm_bindgen(js_name = rememberSavedReturn)]
    fn remember_return(path: &str, scope: &str, work: &str, pages: usize);
    #[wasm_bindgen(js_name = savedReturnPages)]
    fn return_pages(path: &str, scope: &str) -> usize;
    #[wasm_bindgen(js_name = focusSavedReturn)]
    fn focus_return(path: &str, scope: Option<String>);
}

pub(super) fn focus_denied_return(path: &str) {
    focus_return(path, None);
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RowAction {
    Writing,
    Checking,
    Retry,
    Uncertain,
}

struct List {
    reads: ReadOwner<String>,
    chain: PageChain,
    rows: Vec<SavedWorkRow>,
    actions: HashMap<String, RowAction>,
    initial_error: bool,
    append_error: bool,
    notice: Option<String>,
    notice_revision: u64,
}

impl List {
    fn new(scope: String) -> Self {
        Self {
            reads: ReadOwner::new(scope),
            chain: PageChain::default(),
            rows: Vec::new(),
            actions: HashMap::new(),
            initial_error: false,
            append_error: false,
            notice: None,
            notice_revision: 0,
        }
    }

    fn start_page(&mut self, intent: Intent) -> Option<ReadTicket<String>> {
        let cursor = (intent == Intent::Append)
            .then(|| self.chain.next_cursor.clone())
            .flatten();
        self.reads.begin(intent, cursor)
    }

    fn accept_page(&mut self, ticket: &ReadTicket<String>, page: SavedWorksPage) -> bool {
        let ids = page.items.iter().map(|item| item.work_id().to_owned());
        let valid = page.items.len() <= 20
            && page.items.iter().all(|row| row.generation() > 0)
            && self
                .reads
                .accept_page(ticket, &mut self.chain, ids, page.next_cursor);
        if valid {
            self.rows.extend(page.items);
        }
        self.reads.finish(ticket);
        valid
    }
}

#[derive(Clone, Copy)]
struct ListOwner {
    state: RwSignal<List>,
    actor: StoredValue<VerifiedOwner>,
    path: StoredValue<String>,
    session: Session,
}

impl ListOwner {
    fn scope(self) -> Option<VerifiedOwner> {
        let owner = self.actor.try_get_value()?;
        (self
            .session
            .profile_scope
            .try_get_untracked()
            .flatten()
            .as_ref()
            == Some(&owner.scope))
        .then_some(owner)
    }

    fn live(self) -> bool {
        self.scope().is_some() && self.state.try_with_untracked(|_| ()).is_some()
    }

    fn fence(self, error: &ApiError) -> bool {
        if matches!(error, ApiError::Unauthenticated | ApiError::Forbidden) {
            self.session.fence_profile();
            return true;
        }
        false
    }

    fn read(self, intent: Intent) {
        let Some(owner) = self.scope() else {
            return;
        };
        let Some(ticket) = self
            .state
            .try_update(|list| {
                let ticket = list.start_page(intent)?;
                list.append_error = false;
                list.initial_error = false;
                Some(ticket)
            })
            .flatten()
        else {
            return;
        };
        let path = self.path.get_value();
        let append = intent == Intent::Append;
        let depth = if append {
            1
        } else {
            return_pages(&path, &owner.scope).clamp(1, MAX_PAGES)
        };
        spawn_local(async move {
            let principal = &owner.profile.principal_id;
            let first = api::load_saved_works(ticket.cursor.as_deref(), principal).await;
            if !self.live()
                || !self
                    .state
                    .with_untracked(|list| list.reads.accepts(&ticket))
            {
                return;
            }
            if first.as_ref().is_err_and(|error| self.fence(error)) {
                return;
            }
            if append {
                self.state.update(|list| {
                    list.append_error = match first {
                        Ok(page) => !list.accept_page(&ticket, page),
                        Err(_) => {
                            list.reads.finish(&ticket);
                            true
                        }
                    };
                });
                if self
                    .state
                    .with_untracked(|list| list.chain.next_cursor.is_none())
                {
                    keep_tail_focus(self);
                }
                return;
            }

            // Rebuild the already visited depth with new, session-bound cursors. None of the
            // old row projections or cursors are eligible to render, even while waiting.
            let mut fresh = List::new(owner.scope.clone());
            let head = fresh.start_page(Intent::Initial).expect("empty read owner");
            let mut failed = !first.is_ok_and(|page| fresh.accept_page(&head, page));
            for _ in 1..depth {
                let Some(next) = fresh.start_page(Intent::Append).filter(|_| !failed) else {
                    break;
                };
                let result = api::load_saved_works(next.cursor.as_deref(), principal).await;
                if !self.live()
                    || !self
                        .state
                        .with_untracked(|list| list.reads.accepts(&ticket))
                {
                    return;
                }
                if result.as_ref().is_err_and(|error| self.fence(error)) {
                    return;
                }
                if !result.is_ok_and(|page| fresh.accept_page(&next, page)) {
                    failed = true;
                }
            }
            fresh.initial_error = failed && fresh.chain.pages == 0;
            fresh.append_error = failed && fresh.chain.pages > 0;
            fresh.reads.invalidate();
            self.state.set(fresh);
            focus_return(&path, Some(owner.scope));
        });
    }

    fn operate(self, work: String, check_only: bool) {
        let Some(owner) = self.scope() else {
            return;
        };
        let Some((generation, readable)) = self
            .state
            .try_update(|list| {
                match (check_only, list.actions.get(&work)) {
                    (true, Some(RowAction::Uncertain)) | (false, None | Some(RowAction::Retry)) => {
                    }
                    _ => return None,
                }
                let row = list.rows.iter().find(|row| row.work_id() == work)?;
                let generation = row.generation();
                let readable = matches!(row, SavedWorkRow::Readable { .. });
                list.actions.insert(
                    work.clone(),
                    if check_only {
                        RowAction::Checking
                    } else {
                        RowAction::Writing
                    },
                );
                Some((generation, readable))
            })
            .flatten()
        else {
            return;
        };
        if check_only
            || document().active_element().is_some_and(|active| {
                document()
                    .get_element_by_id(&format!("saved-row-{work}"))
                    .is_some_and(|row| row.contains(Some(&active)))
            })
        {
            // Recovery controls disappear while checking; park focus at the stable action
            // before replacing the row state. A later ACK can then move it to a neighbor.
            if let Some(button) = document()
                .get_element_by_id(&format!("saved-remove-{work}"))
                .and_then(|element| element.dyn_into::<web_sys::HtmlElement>().ok())
            {
                let _ = button.focus();
            }
        }
        spawn_local(async move {
            let principal = &owner.profile.principal_id;
            let confirmed = if check_only {
                api::reconcile_saved_work(&work, principal).await
            } else {
                let write = api::set_saved_work(&work, false, generation, principal).await;
                if !self.live() {
                    return;
                }
                match write {
                    Ok(ack) if !ack.saved && ack.generation > generation => Ok(ack),
                    Err(ApiError::GenerationConflict { .. }) => {
                        self.state.update(|list| {
                            list.actions.insert(work.clone(), RowAction::Checking);
                        });
                        api::reconcile_saved_work(&work, principal).await
                    }
                    Ok(_)
                    | Err(
                        ApiError::Network
                        | ApiError::Unavailable
                        | ApiError::Unexpected {
                            status: 0 | 500..=599,
                        },
                    ) => {
                        self.set_action(work, RowAction::Uncertain);
                        return;
                    }
                    Err(error) => {
                        if !self.fence(&error) {
                            self.set_action(work, RowAction::Retry);
                        }
                        return;
                    }
                }
            };
            if !self.live() {
                return;
            }
            match confirmed {
                Ok(SavedWorkState {
                    saved: false,
                    generation: current,
                }) if current > generation => {
                    self.remove_confirmed(work, readable);
                }
                Ok(SavedWorkState {
                    saved: true,
                    generation: current,
                }) if current >= generation => {
                    // Keep the membership, advance only this pair's generation. The next
                    // DELETE still needs a deliberate press by the user.
                    self.state.update(|list| {
                        if let Some(row) = list.rows.iter_mut().find(|row| row.work_id() == work) {
                            match row {
                                SavedWorkRow::Readable { generation, .. }
                                | SavedWorkRow::Hidden { generation, .. }
                                | SavedWorkRow::AuthorDeleted { generation, .. } => {
                                    *generation = current
                                }
                            }
                        }
                        list.actions.insert(work, RowAction::Retry);
                    });
                }
                Err(error) if self.fence(&error) => {}
                _ => self.set_action(work, RowAction::Uncertain),
            }
        });
    }

    fn set_action(self, work: String, action: RowAction) {
        self.state.update(|list| {
            list.actions.insert(work, action);
        });
    }

    fn remove_confirmed(self, work: String, readable: bool) {
        let active = document().active_element();
        let row_id = format!("saved-row-{work}");
        let focus_lost = active.as_ref().is_some_and(|active| {
            document()
                .get_element_by_id(&row_id)
                .is_some_and(|row| row.contains(Some(active)))
        });
        let Some((target, revision)) = self
            .state
            .try_update(|list| {
                let index = list.rows.iter().position(|row| row.work_id() == work)?;
                list.rows.remove(index);
                list.actions.remove(&work);
                let target = list
                    .rows
                    .get(index)
                    .or_else(|| index.checked_sub(1).and_then(|n| list.rows.get(n)));
                let target = target.map(|row| {
                    let prefix = if matches!(row, SavedWorkRow::Readable { .. }) {
                        "saved-open"
                    } else {
                        "saved-remove"
                    };
                    format!("{prefix}-{}", row.work_id())
                });
                list.notice_revision += 1;
                list.notice = Some("Đã bỏ lưu.".into());
                Some((target, list.notice_revision))
            })
            .flatten()
        else {
            return;
        };
        if focus_lost {
            let fallback = if target.is_none()
                && self
                    .state
                    .with_untracked(|list| list.chain.next_cursor.is_none())
            {
                "saved-empty-heading"
            } else {
                HEADING
            };
            focus_after_render(self, target.unwrap_or_else(|| fallback.into()), active);
        }
        if readable {
            spawn_local(async move {
                let title = api::readable_saved_title(&work).await;
                if self.live()
                    && let Some(title) = title
                {
                    self.state.update(|list| {
                        if list.notice_revision == revision && list.notice.is_some() {
                            list.notice = Some(format!("Đã bỏ lưu “{title}”."));
                        }
                    });
                }
            });
        }
        set_timeout(
            move || {
                self.state.try_update(|list| {
                    if list.notice_revision == revision {
                        list.notice = None;
                    }
                });
            },
            std::time::Duration::from_secs(6),
        );
    }
}

fn focus_after_render(owner: ListOwner, target: String, source: Option<web_sys::Element>) {
    request_animation_frame(move || {
        if !owner.live() || window().location().pathname().ok() != owner.path.try_get_value() {
            return;
        }
        let active = document().active_element();
        if active
            .as_ref()
            .is_some_and(|el| el.tag_name() == "BODY" || source.as_ref() == Some(el))
            && let Some(el) = document()
                .get_element_by_id(&target)
                .or_else(|| document().get_element_by_id(HEADING))
                .and_then(|el| el.dyn_into::<web_sys::HtmlElement>().ok())
        {
            let _ = el.focus();
        }
    });
}

fn keep_tail_focus(owner: ListOwner) {
    let focused = document()
        .active_element()
        .filter(|el| matches!(el.id().as_str(), "saved-more" | "saved-append-retry"));
    if focused.is_some() {
        focus_after_render(owner, "saved-tail".into(), focused);
    }
}

#[component]
pub(super) fn SavedList(owner: VerifiedOwner) -> impl IntoView {
    let path = format!("/@{}/da-luu", owner.profile.handle);
    let list = ListOwner {
        state: RwSignal::new(List::new(owner.scope.clone())),
        actor: StoredValue::new(owner),
        path: StoredValue::new(path),
        session: Session::current(),
    };
    let state = list.state;
    let initial = Memo::new(move |_| state.with(|list| list.chain.pages == 0));
    let gate = vink_ui::use_loading_gate(
        Signal::derive(move || initial.get() && !state.with(|s| s.initial_error)),
        vink_ui::Clock::Browser,
    );
    let empty = Memo::new(move |_| {
        state.with(|list| {
            list.chain.pages > 0 && list.rows.is_empty() && list.chain.next_cursor.is_none()
        })
    });
    let more = Memo::new(move |_| state.with(|list| list.chain.next_cursor.is_some()));
    Effect::new(move |_| list.read(Intent::Initial));
    view! {
        <h2 id=HEADING class="vk-saved-heading" tabindex="-1">"Đã lưu"</h2>
        <p id="saved-privacy-note"><Icon name=IconName::Lock />"Chỉ mình bạn thấy danh sách này."</p>
        <Show when=move || initial.get()>
            <div id="saved-initial">
                {move || if state.with(|list| list.initial_error) {
                    view! {
                        <p role="alert">"Chưa tải được danh sách Đã lưu."</p>
                        <Button id="saved-retry" variant=ButtonVariant::Tonal on_press=move |_| list.read(Intent::Initial)>"Thử lại"</Button>
                    }.into_any()
                } else {
                    view! {
                        <p role="status" class:vk-ui-visually-hidden=move || !gate.slow.get()>{move || if gate.slow.get() { "Vẫn đang tải…" } else { "Đang tải danh sách…" }}</p>
                        <Show when=move || gate.visible.get()>
                            <div class="vk-saved-skeleton" aria-hidden="true"><Skeleton /><Skeleton /><Skeleton /></div>
                        </Show>
                    }.into_any()
                }}
            </div>
        </Show>
        <Show when=move || empty.get()>
            <EmptyState compact=true icon=IconName::BookmarkBorder title="Chưa lưu bài nào."
                title_id="saved-empty-heading" message="Bấm Lưu ở cuối một tác phẩm để đọc lại sau ở đây.">
                <a class=ButtonVariant::Tonal.classes() href="/">"Khám phá tác phẩm"</a>
            </EmptyState>
        </Show>
        <ul id="saved-list" class="vk-saved-list">
            <For each=move || state.with(|list| list.rows.iter().map(|row| row.work_id().to_owned()).collect::<Vec<_>>())
                key=Clone::clone
                children=move |work| view! { <Row list work /> }
            />
        </ul>
        <div id="saved-tail" tabindex="-1">
            <Show when=move || state.with(|list| list.append_error)>
                <p role="alert">"Chưa tải thêm được. Các bài đã tải vẫn ở nguyên."</p>
            </Show>
            <Show when=move || more.get()>
                <Button id="saved-more" variant=ButtonVariant::Tonal
                    state=Signal::derive(move || if state.with(|list| list.reads.in_flight_intent() == Some(Intent::Append)) {
                        ActionState::pending("Đang tải thêm…")
                    } else { ActionState::Enabled })
                    on_press=move |_| list.read(Intent::Append)>
                    {move || if state.with(|list| list.append_error) { "Thử lại" } else { "Xem thêm" }}
                </Button>
            </Show>
            <Show when=move || !more.get() && !initial.get() && !empty.get()>
                <p>"Đã hết danh sách"</p>
            </Show>
        </div>
        <p id="saved-snack" role="status">{move || state.with(|list| list.notice.clone())}</p>
    }
}

#[component]
fn Row(list: ListOwner, work: String) -> impl IntoView {
    let id = StoredValue::new(work.clone());
    let state = list.state;
    let row = Memo::new(move |_| {
        state.with(|list| {
            list.rows
                .iter()
                .find(|row| row.work_id() == id.get_value())
                .cloned()
        })
    });
    let action = Memo::new(move |_| state.with(|list| list.actions.get(&id.get_value()).copied()));
    let label = Signal::derive(move || {
        row.with(|row| match row {
            Some(SavedWorkRow::Readable { title, .. }) => format!("Bỏ lưu: {title}"),
            Some(SavedWorkRow::AuthorDeleted { .. }) => {
                "Bỏ lưu nội dung đã được tác giả xoá".into()
            }
            Some(SavedWorkRow::Hidden { .. }) => "Bỏ lưu nội dung đã ẩn".into(),
            None => "Bỏ lưu".into(),
        })
    });
    let phase = Signal::derive(move || match action.get() {
        Some(RowAction::Writing) => ActionState::pending("Đang bỏ lưu…"),
        Some(RowAction::Checking) => ActionState::pending("Đang kiểm tra…"),
        Some(RowAction::Uncertain | RowAction::Retry) => ActionState::Disabled { reason: None },
        None => ActionState::Enabled,
    });
    view! {
        <li id=format!("saved-row-{work}") class="vk-public-row vk-saved-row wrap-anywhere">
            <div class="vk-public-row__card vk-saved-card">
                <div class="vk-saved-row__body">
                    {move || match row.get() {
                        Some(SavedWorkRow::Readable { title, form, saved_at, author, .. }) => {
                            let work = id.get_value();
                            let date = vink_patterns::work_view_date_hcm(&saved_at);
                            let (byline, profile) = author_name("", Some(&author));
                            let handle = profile.map(|profile| format!("@{}", profile.handle));
                            let initials = (!crate::ui::adapters::neutral_label(Some(&author), false)).then(|| byline.clone());
                            view! {
                                <h3 class="vk-public-row__title m-0 wrap-anywhere">
                                    <a id=format!("saved-open-{work}") class="vk-public-row__link"
                                        href=format!("/tac-pham/{work}") on:click=move |event| {
                                            if super::super::super::is_primary_link_click(&event)
                                                && let Some(actor) = list.scope()
                                            {
                                                let pages = state.with_untracked(|list| list.chain.pages);
                                                remember_return(&list.path.get_value(), &actor.scope, &work, pages);
                                            }
                                        }>{title}</a>
                                </h3>
                                <p class="vk-public-row__eyebrow -order-1 m-0">
                                    {form.map(|form| view! { <span class="vk-public-row__tag">{form}" · "</span> })}
                                    {date.map(|day| view! { <time datetime=saved_at>"lưu "{day}</time> })}
                                </p>
                                <p class="vk-public-row__byline vk-saved-byline m-0">
                                    <vink_ui::Avatar name=initials.unwrap_or_default() size=vink_ui::AvatarSize::Small decorative=true />
                                    <span class="vk-saved-byline__text">
                                        <span>{byline}</span>
                                        {handle.map(|handle| view! { <span class="vk-saved-byline__handle">{handle}</span> })}
                                    </span>
                                </p>
                            }.into_any()
                        }
                        Some(SavedWorkRow::Hidden { .. }) => view! { <NeutralRow icon=IconName::VisibilityOff message="Nội dung đã được ẩn" /> }.into_any(),
                        Some(SavedWorkRow::AuthorDeleted { .. }) => view! { <NeutralRow icon=IconName::Block message="Nội dung đã được tác giả xoá" /> }.into_any(),
                        None => ().into_any(),
                    }}
                </div>
                <div class="vk-saved-row__action">
                    <IconButton id=format!("saved-remove-{work}") icon=IconName::Bookmark label=label
                        surface=ActionSurface::Tonal state=phase on_press=move |_| list.operate(id.get_value(), false) />
                    <Show when=move || matches!(action.get(), Some(RowAction::Writing | RowAction::Checking))>
                        <span class="vk-saved-row__progress"><vink_ui::LoadingIndicator /></span>
                    </Show>
                </div>
                {move || match action.get() {
                    Some(RowAction::Writing) => view! {
                        <p class="vk-ui-visually-hidden" role="status">"Đang bỏ lưu…"</p>
                    }.into_any(),
                    Some(RowAction::Checking) => view! {
                        <p class="vk-ui-visually-hidden" role="status">"Đang kiểm tra…"</p>
                    }.into_any(),
                    Some(RowAction::Retry) => view! {
                        <div class="vk-saved-row__error"><p role="alert">"Chưa bỏ lưu được. Bài vẫn nằm trong danh sách."</p>
                            <Button variant=ButtonVariant::Text on_press=move |_| list.operate(id.get_value(), false)>"Thử lại"</Button>
                        </div>
                    }.into_any(),
                    Some(RowAction::Uncertain) => view! {
                        <div class="vk-saved-row__error"><p role="alert">"Chưa xác nhận được."</p>
                            <Button variant=ButtonVariant::Text on_press=move |_| list.operate(id.get_value(), true)>"Kiểm tra lại"</Button>
                        </div>
                    }.into_any(),
                    _ => ().into_any(),
                }}
            </div>
        </li>
    }
}

#[component]
fn NeutralRow(icon: IconName, message: &'static str) -> impl IntoView {
    view! { <p class="vk-saved-row__neutral"><Icon name=icon />{message}</p> }
}
