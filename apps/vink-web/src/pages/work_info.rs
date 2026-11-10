//! “Thông tin tác phẩm” in Bàn viết (TAG-PR-05): the app adapter between the draft-tag API and
//! the shared `TagSelector`. It loads the draft's tags and the catalog in the browser only (no
//! private data in SSR), owns the raw query and its request generation, and edits the
//! `TagSession` inside the editor's `DraftSession`. Saving goes through the editor's one “Lưu”,
//! so the toolbar status and the leave guard cover text and tags together.

use leptos::prelude::*;
use vink_ui::{
    Announce, Button, ButtonVariant, Heading, HeadingLevel, IconName, InlineNotice,
    SupportingDetails, Text, TextInput, TextTone, Tone, TypeRole,
};

use crate::contract::TagSuggestion;
use crate::desk_state::{DraftSession, Failure};
use crate::tag_state::{QueryState, TagSession, TagStatus};
use crate::ui::adapters::{self, TagLabels};
use crate::ui::patterns::{CatalogFacts, TagSelector};

/// The theme query field; “Ở lại” returns focus here when the author was choosing tags.
#[cfg_attr(not(feature = "hydrate"), allow(dead_code))]
pub const QUERY_ID: &str = "work-tags-query";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(feature = "hydrate"), allow(dead_code))]
enum TagLoad {
    Loading,
    Ready,
    Failed(Failure),
}

#[component]
pub fn WorkInfo(
    work_id: String,
    session: RwSignal<DraftSession>,
    labels: RwSignal<TagLabels>,
    #[prop(into)] on_save: Callback<()>,
) -> impl IntoView {
    let work_id = StoredValue::new(work_id);
    let load = RwSignal::new(TagLoad::Loading);
    let catalog = RwSignal::new(CatalogFacts::Loading);
    let query = RwSignal::new(QueryState::<TagSuggestion>::default());

    // Never replaces a loaded session: retrying is offered only while no tags are loaded.
    let load_tags = move || {
        load.set(TagLoad::Loading);
        #[cfg(feature = "hydrate")]
        leptos::task::spawn_local(async move {
            match crate::api::load_draft_tags(&work_id.get_value()).await {
                Ok(tags) => {
                    labels.try_update(|l| l.learn(tags.form.iter().chain(tags.themes.iter())));
                    session.try_update(|s| {
                        if s.tags.is_none() {
                            s.tags = Some(TagSession::loaded(
                                tags.version,
                                adapters::tag_selection(&tags),
                            ));
                        }
                    });
                    load.try_set(TagLoad::Ready);
                }
                Err(error) => {
                    load.try_set(TagLoad::Failed(super::desk::failure(&error)));
                }
            }
        });
    };
    let load_catalog = move || {
        catalog.set(CatalogFacts::Loading);
        #[cfg(feature = "hydrate")]
        leptos::task::spawn_local(async move {
            let next = match crate::api::load_tag_catalog().await {
                Ok(list) => {
                    labels.try_update(|l| l.learn(list.tags.iter()));
                    adapters::tag_catalog(&list)
                }
                Err(_) => CatalogFacts::Unavailable,
            };
            catalog.try_set(next);
        });
    };
    #[cfg(not(feature = "hydrate"))]
    let _ = work_id;
    load_tags();
    load_catalog();

    let on_query = Callback::new(move |input: TextInput| {
        let request = query
            .try_update(|q| q.input(&input.value, input.composing))
            .flatten();
        #[cfg(feature = "hydrate")]
        if let Some(request) = request {
            leptos::task::spawn_local(async move {
                use crate::tag_state::QueryFailure;
                let answer = match crate::api::suggest_tags(&request.q, "theme").await {
                    Ok(answer) => {
                        labels.try_update(|l| l.learn(answer.suggestions.iter().map(|s| &s.tag)));
                        Ok((answer.query, answer.suggestions))
                    }
                    Err(crate::contract::ApiError::Invalid { .. }) => Err(QueryFailure::Invalid),
                    Err(_) => Err(QueryFailure::Unavailable),
                };
                query.try_update(|q| q.settle(&request, answer));
            });
        }
        #[cfg(not(feature = "hydrate"))]
        let _ = request;
    });

    let edit = move |change: &dyn Fn(&mut TagSession)| {
        session.update(|s| {
            if let Some(tags) = s.tags.as_mut() {
                change(tags);
            }
        });
    };
    // Memos: typing in the body updates the session on every keystroke; the chips re-render only
    // when the selection, a refusal or a label actually changes.
    let selection =
        Memo::new(move |_| session.with(|s| s.tags.as_ref().map(|t| t.current().clone())));
    let refusal = Memo::new(move |_| {
        session.with(|s| s.tags.as_ref().and_then(|t| t.refused_tag().cloned()))
    });
    let form = Memo::new(move |_| {
        let refusal = refusal.get();
        selection
            .get()
            .and_then(|s| s.form)
            .map(|id| labels.with(|l| l.facts(&id, refusal.as_ref())))
    });
    let themes = Memo::new(move |_| {
        let refusal = refusal.get();
        selection
            .get()
            .map(|s| s.themes)
            .unwrap_or_default()
            .iter()
            .map(|id| labels.with(|l| l.facts(id, refusal.as_ref())))
            .collect::<Vec<_>>()
    });
    let options = Signal::derive(move || query.with(|q| adapters::tag_options(q.phase())));
    let exact = Signal::derive(move || query.with(|q| adapters::exact_option(q.phase())));
    let raw = Signal::derive(move || query.with(|q| q.raw().to_string()));
    let status = Memo::new(move |_| session.with(DraftSession::tag_status));
    let loaded = move || session.with(|s| s.tags.is_some());

    let notice = move || {
        let status = status.get()?;
        let view = match status {
            TagStatus::Conflict { stored, .. } => {
                let stored_names = labels.with(|l| l.names(&stored));
                let mine = labels.with(|l| selection.get().map(|s| l.names(&s)).unwrap_or_default());
                view! {
                    <section class="vk-editor-recovery" data-tone="warning" data-testid="tag-conflict" aria-label="Xung đột thẻ">
                        <InlineNotice tone=Tone::Warning announce=Announce::Off
                            title="Thẻ đã được lưu khác ở nơi khác."
                            message=format!("Đang lưu: {stored_names}. Trên trang này: {mine}. Chưa ghi đè lựa chọn nào.") />
                        <div class="vk-notice__actions">
                            <Button variant=ButtonVariant::Text id="tags-use-stored"
                                on_press=move |_| edit(&TagSession::use_stored)>"Dùng thẻ đang lưu"</Button>
                            <Button variant=ButtonVariant::Text id="tags-keep-mine"
                                on_press=move |_| { edit(&TagSession::keep_mine); on_save.run(()); }>
                                "Lưu đè bằng lựa chọn trên trang"
                            </Button>
                        </div>
                    </section>
                }
                .into_any()
            }
            TagStatus::Refused(refusal) => view! {
                <section class="vk-editor-recovery" data-tone="error" data-testid="tag-refused" aria-label="Thẻ bị từ chối">
                    <InlineNotice tone=Tone::Error announce=Announce::Off title="Thẻ chưa lưu được."
                        message=adapters::tag_refusal_text(&refusal) />
                </section>
            }
            .into_any(),
            TagStatus::Failed(failure) => view! {
                <section class="vk-editor-recovery" data-tone="error" data-testid="tag-failed" aria-label="Phục hồi lưu thẻ">
                    <InlineNotice tone=Tone::Error announce=Announce::Off
                        title=super::desk::tag_status_label(&TagStatus::Failed(failure))
                        message=super::desk::tag_status_detail(&TagStatus::Failed(failure)) />
                    <div class="vk-notice__actions">
                        <Button variant=ButtonVariant::Text id="tags-retry" leading=IconName::Refresh
                            on_press=move |_| on_save.run(())>"Thử lại"</Button>
                    </div>
                </section>
            }
            .into_any(),
            _ => return None,
        };
        Some(view)
    };

    view! {
        <section class="vk-work-info" aria-labelledby="work-info-heading" data-testid="work-info">
            <div class="vk-work-info__heading">
                <Heading level=HeadingLevel::H2 role=TypeRole::Title id="work-info-heading">"Thông tin tác phẩm"</Heading>
                {move || status.get().map(|status| view! {
                    <span class="vk-work-info__status" data-testid="tag-status" data-status=status.token()>
                        {super::desk::tag_status_label(&status)}
                    </span>
                })}
            </div>
            <Text role=TypeRole::Caption tone=TextTone::Muted>
                "Chỉ bạn thấy. Lưu thẻ chỉ lưu vào bản nháp, không công khai tác phẩm."
            </Text>
            <Show when=loaded fallback=move || match load.get() {
                TagLoad::Failed(failure) => view! {
                    <section class="vk-editor-recovery" data-tone="error" data-testid="tags-unavailable" aria-label="Không tải được thẻ">
                        <InlineNotice tone=Tone::Error announce=Announce::Polite title="Chưa tải được thẻ của bản nháp."
                            message=format!("{} Chữ vẫn sửa và lưu được.", super::desk::failure_reason(failure)) />
                        <div class="vk-notice__actions">
                            <Button variant=ButtonVariant::Text id="tags-reload" leading=IconName::Refresh
                                on_press=move |_| load_tags()>"Tải lại thẻ"</Button>
                        </div>
                    </section>
                }
                .into_any(),
                _ => view! { <Text role=TypeRole::Caption tone=TextTone::Muted>"Đang tải thẻ…"</Text> }.into_any(),
            }>
                <Text role=TypeRole::Caption class:vk-work-info__selection=true attr:data-testid="tag-selection-summary">
                    {move || selection.get().map(|s| format!("Thẻ: {}.", labels.with(|l| l.names(&s))))}
                </Text>
                // Native disclosure keeps the same mounted selector/query and EditorView.
                // Save failures and conflict actions remain visible even while it is closed.
                <SupportingDetails id="work-tags-edit" summary="Chỉnh thẻ">
                <TagSelector id="work-tags" form themes catalog query=raw options exact
                    on_form=Callback::new(move |id: Option<String>| edit(&|t| t.choose_form(id.clone())))
                    on_add_theme=Callback::new(move |id: String| {
                        edit(&|t| { let _ = t.add_theme(&id); });
                        let added = session.with_untracked(|s| s.tags.as_ref().is_some_and(|t| t.current().themes.contains(&id)));
                        if added { query.update(QueryState::clear); }
                    })
                    on_remove_theme=Callback::new(move |id: String| edit(&|t| t.remove_theme(&id)))
                    on_replace_theme=Callback::new(move |(old, new): (String, String)| edit(&|t| t.replace_theme(&old, &new)))
                    on_query />
                </SupportingDetails>
                {notice}
            </Show>
        </section>
    }
}
