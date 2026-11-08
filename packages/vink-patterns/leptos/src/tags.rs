//! Work tags (TAG-PR-05): a controlled selector for one form and a few themes, and a removable
//! chip. Facts come in, intents go out. The host owns the catalog reads, suggestion requests,
//! versions, saving and every limit the server decides; nothing here fetches, stores or saves.

use leptos::prelude::*;
use vink_ui::*;

/// The most themes a work carries in the current contract; the host still enforces it.
pub const THEME_LIMIT: usize = 3;

/// A catalog tag reduced to what a person reads. `retired` marks a tag already on the work that
/// can no longer be added; `problem` is the server's refusal for this tag, in words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TagFacts {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
    pub retired: bool,
    pub successor: Option<TagLink>,
    pub problem: Option<String>,
}

impl TagFacts {
    pub fn active(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            description: None,
            retired: false,
            successor: None,
            problem: None,
        }
    }
}

/// Another tag named by ID and label (a curator's successor hint).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TagLink {
    pub id: String,
    pub label: String,
}

/// One candidate for the typed query; `via_alias` is the other name that matched.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TagOption {
    pub tag: TagFacts,
    pub via_alias: Option<String>,
}

/// Candidates for the current query, as the host's request owner knows them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum TagOptions {
    /// Blank query.
    #[default]
    Idle,
    /// An IME composition is open; nothing is looked up or announced yet.
    Composing,
    Loading,
    /// Answer to exactly the current query; empty means no match.
    Ready(Vec<TagOption>),
    /// The query was refused (e.g. no letter or digit, too long).
    Invalid,
    Unavailable,
}

/// The catalog the host loaded for choosing a form and browsing themes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum CatalogFacts {
    #[default]
    Loading,
    Ready {
        forms: Vec<TagFacts>,
        themes: Vec<TagFacts>,
        /// More active tags exist than were listed; search still finds them.
        truncated: bool,
    },
    Unavailable,
}

/// The polite status line under the query, in words; empty while idle or composing.
pub fn options_status(options: &TagOptions, query: &str, themes: usize) -> String {
    let query = query.trim();
    match options {
        TagOptions::Idle | TagOptions::Composing => String::new(),
        TagOptions::Loading => "Đang tìm chủ đề…".into(),
        TagOptions::Ready(found) if found.is_empty() => format!(
            "Không có chủ đề nào khớp “{query}”. Danh mục do ban biên tập quản lý; hiện chưa thể đề xuất thẻ mới."
        ),
        TagOptions::Ready(found) if themes >= THEME_LIMIT => format!(
            "{} gợi ý cho “{query}”. Đã đủ {THEME_LIMIT} chủ đề: bỏ một chủ đề để thêm chủ đề khác.",
            found.len()
        ),
        TagOptions::Ready(found) => format!("{} gợi ý cho “{query}”.", found.len()),
        TagOptions::Invalid => "Gõ chữ hoặc số để tìm, tối đa 40 ký tự.".into(),
        TagOptions::Unavailable => "Tạm thời không tìm được chủ đề. Gõ lại để thử lại.".into(),
    }
}

fn note_text(tag: &TagFacts) -> Option<String> {
    match (&tag.problem, tag.retired, &tag.successor) {
        (Some(problem), _, _) => Some(problem.clone()),
        (None, true, Some(successor)) => Some(format!(
            "Thẻ này đã ngừng dùng; vẫn được giữ trên bản nháp. Ban biên tập gợi ý “{}”.",
            successor.label
        )),
        (None, true, None) => Some("Thẻ này đã ngừng dùng; vẫn được giữ trên bản nháp.".into()),
        (None, false, _) => None,
    }
}

/// A selected tag: its label, a written retired/problem state (never colour alone) and a
/// remove action with its own name. `on_replace` offers the curator's successor when there is
/// one. The remove button's id is `{id}-remove`, so a host can return focus to it.
#[component]
pub fn TagChip(
    #[prop(into)] id: String,
    tag: TagFacts,
    /// Accessible name of the remove action, e.g. “Bỏ chủ đề Ký ức”.
    #[prop(into)]
    remove_label: String,
    #[prop(optional, into)] on_remove: Option<Callback<()>>,
    #[prop(optional, into)] on_replace: Option<Callback<String>>,
) -> impl IntoView {
    let note = note_text(&tag);
    let note_id = format!("{id}-note");
    let status = if tag.problem.is_some() {
        "problem"
    } else if tag.retired {
        "retired"
    } else {
        "active"
    };
    let replace = tag
        .successor
        .clone()
        .zip(on_replace)
        .filter(|_| tag.retired || tag.problem.is_some());
    view! {
        <span class="vk-tag-chip" data-status=status id=id.clone()>
            <span class="vk-tag-chip__body">
                <span class="vk-tag-chip__label">{tag.label.clone()}</span>
                {tag.retired.then(|| view! { <span class="vk-tag-chip__badge">"Đã ngừng dùng"</span> })}
                {on_remove.map(|remove| {
                    let button = format!("{id}-remove");
                    if note.is_some() {
                        view! { <IconButton icon=IconName::Close label=remove_label id=button
                            described_by=note_id.clone() on_press=move |_| remove.run(()) /> }
                        .into_any()
                    } else {
                        view! { <IconButton icon=IconName::Close label=remove_label id=button
                            on_press=move |_| remove.run(()) /> }
                        .into_any()
                    }
                })}
            </span>
            {note.map(|text| view! {
                <span class="vk-tag-chip__note" id=note_id.clone()>
                    <Icon name=if tag.problem.is_some() { IconName::Error } else { IconName::Info } size=IconSize::Small />
                    <span>{text}</span>
                </span>
            })}
            {replace.map(|(successor, replace)| {
                let target = successor.id.clone();
                view! {
                    <span class="vk-tag-chip__replace">
                        <Button variant=ButtonVariant::Text id=format!("{id}-replace")
                            on_press=move |_| replace.run(target.clone())>
                            {format!("Thay bằng “{}”", successor.label)}
                        </Button>
                    </span>
                }
            })}
        </span>
    }
}

/// Focuses the element with `id`, if it is still in the document.
fn focus_id(id: &str) -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        use leptos::wasm_bindgen::JsCast;
        if let Some(element) = document()
            .get_element_by_id(id)
            .and_then(|e| e.dyn_into::<leptos::web_sys::HtmlElement>().ok())
        {
            return element.focus().is_ok();
        }
    }
    let _ = id;
    false
}

/// The author's form and themes for one work.
///
/// Anatomy (LayoutSpec `#tag-selector`): a “Dạng bài” group of single-choice chips, then a
/// “Chủ đề” group with removable chips, a search field, a polite status line, the candidates and
/// a native disclosure listing the themes the host loaded. Enter while composing never chooses
/// (the field blocks it); outside composition Enter adds only an exact match of the current
/// query. ↓ moves from the field to the candidates, ↑/↓ between them, Escape back to the field.
#[component]
pub fn TagSelector(
    /// Prefix of every element id inside.
    #[prop(into)]
    id: String,
    #[prop(into)] form: Signal<Option<TagFacts>>,
    #[prop(into)] themes: Signal<Vec<TagFacts>>,
    #[prop(into)] catalog: Signal<CatalogFacts>,
    /// The raw query, exactly as typed (composition included).
    #[prop(into)]
    query: Signal<String>,
    #[prop(into)] options: Signal<TagOptions>,
    /// Exact match of the current query (`match: exact`), offered to Enter.
    #[prop(optional, into)]
    exact: Signal<Option<String>>,
    #[prop(into)] on_form: Callback<Option<String>>,
    #[prop(into)] on_add_theme: Callback<String>,
    #[prop(into)] on_remove_theme: Callback<String>,
    #[prop(into)] on_replace_theme: Callback<(String, String)>,
    #[prop(into)] on_query: Callback<TextInput>,
) -> impl IntoView {
    let input = NodeRef::<leptos::html::Input>::new();
    let query_id = format!("{id}-query");
    let status_id = format!("{id}-status");
    let options_id = format!("{id}-options");
    let option_id = {
        let id = id.clone();
        move |index: usize| format!("{id}-option-{index}")
    };
    let focus_query = move || {
        request_animation_frame(move || {
            if let Some(input) = input.get_untracked() {
                let _ = input.focus();
            }
        });
    };
    let add = move |tag_id: String| {
        on_add_theme.run(tag_id);
        focus_query();
    };
    let full = move || themes.with(|t| t.len() >= THEME_LIMIT);
    let selected = move |tag_id: &str| themes.with(|t| t.iter().any(|x| x.id == tag_id));

    // Retired or unlisted current form: kept as a chip, since its own choice chip is gone.
    let form_chip = {
        let id = id.clone();
        move || {
            let current = form.get()?;
            let listed = catalog.with(|c| match c {
                CatalogFacts::Ready { forms, .. } => forms.iter().any(|f| f.id == current.id),
                _ => false,
            });
            (current.retired || current.problem.is_some() || !listed).then(|| {
                let label = format!("Bỏ dạng bài {}", current.label);
                view! {
                    <TagChip id=format!("{id}-form-current") tag=current remove_label=label
                        on_remove=move |_| on_form.run(None)
                        on_replace=move |successor: String| on_form.run(Some(successor)) />
                }
            })
        }
    };
    let form_choices = {
        let id = id.clone();
        move || {
            match catalog.get() {
            CatalogFacts::Loading => view! {
                <Text role=TypeRole::Caption tone=TextTone::Muted>"Đang tải danh mục…"</Text>
            }
            .into_any(),
            CatalogFacts::Unavailable => view! {
                <Text role=TypeRole::Caption tone=TextTone::Muted>
                    "Chưa tải được danh mục dạng bài. Thẻ đã chọn vẫn được giữ."
                </Text>
            }
            .into_any(),
            CatalogFacts::Ready { forms, .. } if forms.is_empty() => view! {
                <Text role=TypeRole::Caption tone=TextTone::Muted>"Danh mục chưa có dạng bài nào."</Text>
            }
            .into_any(),
            CatalogFacts::Ready { forms, .. } => view! {
                <div class="vk-tag-chips">
                    {forms
                        .into_iter()
                        .map(|tag| {
                            let picked = tag.id.clone();
                            let current = tag.id.clone();
                            view! {
                                <FilterChip id=format!("{id}-form-{}", tag.id) label=tag.label
                                    selected=Signal::derive(move || form.with(|f| f.as_ref().is_some_and(|f| f.id == current)))
                                    on_toggle=move |on: bool| on_form.run(on.then(|| picked.clone())) />
                            }
                        })
                        .collect_view()}
                </div>
            }
            .into_any(),
        }
        }
    };
    let form_description = move || {
        form.get().and_then(|f| {
            f.description
                .map(|d| view! { <p class="vk-tag-group__help">{format!("{}: {d}", f.label)}</p> })
        })
    };

    let theme_chips = {
        let id = id.clone();
        let query_id = query_id.clone();
        move || {
            let list = themes.get();
            if list.is_empty() {
                return view! {
                    <Text role=TypeRole::Caption tone=TextTone::Muted>"Chưa chọn chủ đề nào."</Text>
                }
                .into_any();
            }
            let count = list.len();
            view! {
                <ul class="vk-tag-chips" aria-label="Chủ đề đã chọn">
                    {list
                        .into_iter()
                        .enumerate()
                        .map(|(index, tag)| {
                            let removed = tag.id.clone();
                            let retired = tag.id.clone();
                            let chip = format!("{id}-theme-{index}");
                            let label = format!("Bỏ chủ đề {}", tag.label);
                            let id = id.clone();
                            let query_id = query_id.clone();
                            view! {
                                <li>
                                    <TagChip id=chip tag remove_label=label
                                        on_remove=move |_| {
                                            on_remove_theme.run(removed.clone());
                                            // The next chip takes this index; else the previous; else the field.
                                            let id = id.clone();
                                            let query_id = query_id.clone();
                                            request_animation_frame(move || {
                                                let next = format!("{id}-theme-{index}-remove");
                                                let previous = index
                                                    .checked_sub(1)
                                                    .map(|i| format!("{id}-theme-{i}-remove"));
                                                let _ = (index + 1 < count && focus_id(&next))
                                                    || previous.is_some_and(|p| focus_id(&p))
                                                    || focus_id(&query_id);
                                            });
                                        }
                                        on_replace=move |successor: String| on_replace_theme.run((retired.clone(), successor)) />
                                </li>
                            }
                        })
                        .collect_view()}
                </ul>
            }
            .into_any()
        }
    };

    // One candidate button; its description is supporting text linked to the same target.
    let option_view = move |button: String, option: TagOption| {
        let tag = option.tag;
        let chosen = selected(&tag.id);
        let state = if chosen || full() {
            ActionState::Disabled { reason: None }
        } else {
            ActionState::Enabled
        };
        let description_id = format!("{button}-description");
        let target = tag.id.clone();
        let label = view! {
            <span class="vk-tag-option__label">{tag.label.clone()}</span>
            {option.via_alias.map(|alias| view! { <span class="vk-tag-option__alias">{format!("qua tên “{alias}”")}</span> })}
            {chosen.then(|| view! { <span class="vk-tag-option__alias">"Đã chọn"</span> })}
        };
        let action = if tag.description.is_some() {
            view! {
                <Button variant=ButtonVariant::Text leading=IconName::Add id=button
                    described_by=description_id.clone() state on_press=move |_| add(target.clone())>
                    {label}
                </Button>
            }
            .into_any()
        } else {
            view! {
                <Button variant=ButtonVariant::Text leading=IconName::Add id=button
                    state on_press=move |_| add(target.clone())>
                    {label}
                </Button>
            }
            .into_any()
        };
        view! {
            <li class="vk-tag-option">
                {action}
                {tag.description.map(|d| view! { <p class="vk-tag-option__description" id=description_id>{d}</p> })}
            </li>
        }
    };
    let candidates = {
        let option_id = option_id.clone();
        let options_id = options_id.clone();
        move || match options.get() {
            TagOptions::Ready(found) if !found.is_empty() => Some(view! {
                <ul class="vk-tag-options" id=options_id.clone() aria-label="Gợi ý chủ đề">
                    {found
                        .into_iter()
                        .enumerate()
                        .map(|(index, option)| option_view(option_id(index), option))
                        .collect_view()}
                </ul>
            }),
            _ => None,
        }
    };
    let browse = {
        let id = id.clone();
        move || match catalog.get() {
            CatalogFacts::Ready {
                themes: all,
                truncated,
                ..
            } if !all.is_empty() => {
                let count = all.len();
                let id = id.clone();
                Some(view! {
                    <div class="vk-tag-browse">
                        <SupportingDetails id=format!("{id}-browse") summary=format!("Xem các chủ đề có sẵn ({count})")>
                            <ul class="vk-tag-options" aria-label="Các chủ đề có sẵn">
                                {all
                                    .into_iter()
                                    .enumerate()
                                    .map(|(index, tag)| option_view(format!("{id}-browse-{index}"), TagOption { tag, via_alias: None }))
                                    .collect_view()}
                            </ul>
                            {truncated.then(|| view! {
                                <p class="vk-tag-group__help">"Danh mục còn nhiều chủ đề hơn; gõ để tìm."</p>
                            })}
                        </SupportingDetails>
                    </div>
                })
            }
            _ => None,
        }
    };

    let on_keydown = {
        let option_id = option_id.clone();
        let query_id = query_id.clone();
        move |ev: leptos::ev::KeyboardEvent| {
            if ev.is_composing() || ev.key_code() == 229 {
                return;
            }
            let count = options.with(|o| match o {
                TagOptions::Ready(found) => found.len(),
                _ => 0,
            });
            let target = event_target::<leptos::web_sys::Element>(&ev).id();
            let current = (0..count).find(|i| option_id(*i) == target);
            let next = match (ev.key().as_str(), current) {
                ("ArrowDown", None) if target == query_id && count > 0 => Some(option_id(0)),
                ("ArrowDown", Some(i)) if i + 1 < count => Some(option_id(i + 1)),
                ("ArrowUp", Some(0)) => Some(query_id.clone()),
                ("ArrowUp", Some(i)) => Some(option_id(i - 1)),
                ("Escape", Some(_)) => Some(query_id.clone()),
                _ => None,
            };
            if let Some(next) = next {
                ev.prevent_default();
                focus_id(&next);
            }
        }
    };
    let submit = move |_value: String| {
        // The field already refused Enter during composition. Only an exact match is chosen.
        if let Some(tag_id) = exact.get_untracked()
            && !full()
            && !selected(&tag_id)
        {
            add(tag_id);
        }
    };

    view! {
        <div class="vk-tag-selector" id=id.clone()>
            <fieldset class="vk-tag-group" id=format!("{id}-form")>
                <legend class="vk-tag-group__legend">
                    "Dạng bài" <span class="vk-tag-group__count">" · chọn 1"</span>
                </legend>
                {form_chip}
                {form_choices}
                {form_description}
            </fieldset>
            <fieldset class="vk-tag-group" id=format!("{id}-themes")>
                <legend class="vk-tag-group__legend">
                    "Chủ đề" <span class="vk-tag-group__count">{move || format!(" · {}/{THEME_LIMIT}", themes.with(Vec::len))}</span>
                </legend>
                {theme_chips}
                <div class="vk-tag-query" on:keydown=on_keydown>
                    <SearchField id=query_id.clone() label="Tìm chủ đề" value=query on_input=on_query
                        on_submit=submit input_ref=input
                        help="Gõ tên chủ đề hoặc tên gọi khác, rồi chọn trong gợi ý. Enter chỉ chọn khi gõ đúng tên." />
                    <p class="vk-tag-query__status" id=status_id role="status" aria-live="polite">
                        {move || options_status(&options.get(), &query.get(), themes.with(Vec::len))}
                    </p>
                    {candidates}
                </div>
                {browse}
            </fieldset>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(view: impl FnOnce() -> AnyView + 'static) -> String {
        Owner::new().with(|| view().to_html())
    }

    fn retired() -> TagFacts {
        TagFacts {
            retired: true,
            successor: Some(TagLink {
                id: "mua".into(),
                label: "Mùa".into(),
            }),
            ..TagFacts::active("do-thi", "Đô thị")
        }
    }

    #[test]
    fn a_chip_names_its_removal_and_writes_its_state() {
        let html = render(|| {
            view! { <TagChip id="t" tag=retired() remove_label="Bỏ chủ đề Đô thị"
            on_remove=|_| {} on_replace=|_: String| {} /> }
            .into_any()
        });
        assert!(html.contains("aria-label=\"Bỏ chủ đề Đô thị\""), "{html}");
        assert!(html.contains("Đã ngừng dùng"));
        assert!(html.contains("Thay bằng “Mùa”"));
        assert!(html.contains("aria-describedby=\"t-note\""));
        assert!(html.contains("data-status=\"retired\""));
        let active = render(|| {
            view! { <TagChip id="a" tag=TagFacts::active("ky-uc", "Ký ức") remove_label="Bỏ chủ đề Ký ức"
                on_remove=|_| {} /> }
            .into_any()
        });
        assert!(!active.contains("a-note") && !active.contains("Thay bằng"));
    }

    #[test]
    fn the_selector_renders_groups_counts_and_candidates_without_any_effect() {
        let html = render(|| {
            view! {
                <TagSelector id="tags"
                    form=Signal::stored(Some(TagFacts::active("tan-van", "Tản văn")))
                    themes=Signal::stored(vec![TagFacts::active("ky-uc", "Ký ức"), retired()])
                    catalog=Signal::stored(CatalogFacts::Ready {
                        forms: vec![TagFacts::active("tan-van", "Tản văn"), TagFacts::active("tho", "Thơ")],
                        themes: vec![TagFacts::active("ky-uc", "Ký ức")],
                        truncated: false,
                    })
                    query=Signal::stored("hồi".to_string())
                    options=Signal::stored(TagOptions::Ready(vec![TagOption {
                        tag: TagFacts { description: Some("Những điều đã qua.".into()), ..TagFacts::active("ky-uc", "Ký ức") },
                        via_alias: Some("Hồi ức".into()),
                    }]))
                    on_form=|_| {} on_add_theme=|_| {} on_remove_theme=|_| {}
                    on_replace_theme=|_| {} on_query=|_| {} />
            }
            .into_any()
        });
        for needle in [
            "Dạng bài",
            " · 2/3",
            "aria-pressed=\"true\"",
            "Bỏ chủ đề Ký ức",
            "Bỏ chủ đề Đô thị",
            "qua tên “Hồi ức”",
            "Đã chọn",
            "1 gợi ý cho “hồi”.",
            "Xem các chủ đề có sẵn (1)",
            "role=\"status\"",
        ] {
            assert!(html.contains(needle), "missing {needle}: {html}");
        }
    }

    #[test]
    fn status_words_never_claim_a_proposal_or_a_result_for_other_input() {
        assert_eq!(options_status(&TagOptions::Composing, "hô", 0), "");
        assert!(
            options_status(&TagOptions::Ready(Vec::new()), " xyz ", 0)
                .contains("chưa thể đề xuất thẻ mới")
        );
        let one = TagOptions::Ready(vec![TagOption {
            tag: TagFacts::active("a", "A"),
            via_alias: None,
        }]);
        assert!(options_status(&one, "a", THEME_LIMIT).contains("Đã đủ 3 chủ đề"));
        assert!(options_status(&TagOptions::Unavailable, "a", 0).contains("Tạm thời"));
    }
}
