//! TagSelector and TagChip (TAG-PR-05) through the public `vink-patterns` API. Every tag, label
//! and description is [Mẫu]; the interactive owner is in memory: nothing is fetched, saved or
//! confirmed by a service.

use leptos::prelude::*;
use vink_patterns::{CatalogFacts, TagFacts, TagLink, TagOption, TagOptions, TagSelector};
use vink_ui::{Text, TextInput, TextTone, TypeRole};

use crate::components::log;

pub fn specimen(id: &str, scenario: &str) -> Option<AnyView> {
    (id == "composition.tag-selector").then(|| selector(scenario))
}

fn tag(id: &str, label: &str, description: Option<&str>) -> TagFacts {
    TagFacts {
        description: description.map(str::to_string),
        ..TagFacts::active(id, label)
    }
}

fn retired_theme() -> TagFacts {
    TagFacts {
        retired: true,
        successor: Some(TagLink {
            id: "mua".into(),
            label: "Mùa".into(),
        }),
        ..TagFacts::active("do-thi", "Đô thị")
    }
}

fn forms() -> Vec<TagFacts> {
    vec![
        tag(
            "tan-van",
            "Tản văn",
            Some("Văn xuôi ngắn viết từ trải nghiệm và cảm xúc riêng."),
        ),
        tag("tho", "Thơ", None),
        tag("truyen-ngan", "Truyện ngắn", None),
    ]
}

fn themes() -> Vec<TagFacts> {
    vec![
        tag(
            "ky-uc",
            "Ký ức",
            Some("Những điều đã qua còn ở lại trong người viết."),
        ),
        tag("gia-dinh", "Gia đình", None),
        tag("mua", "Mùa", None),
        tag("hoi-hoa", "Hội họa", None),
    ]
}

fn hoi() -> TagOptions {
    TagOptions::Ready(vec![
        TagOption {
            tag: tag("hoi-hoa", "Hội họa", None),
            via_alias: None,
        },
        TagOption {
            tag: tag(
                "ky-uc",
                "Ký ức",
                Some("Những điều đã qua còn ở lại trong người viết."),
            ),
            via_alias: Some("Hồi ức".into()),
        },
    ])
}

struct Sample {
    form: Option<TagFacts>,
    themes: Vec<TagFacts>,
    catalog: CatalogFacts,
    query: &'static str,
    options: TagOptions,
}

fn sample(scenario: &str) -> Sample {
    let ready = CatalogFacts::Ready {
        forms: forms(),
        themes: themes(),
        truncated: false,
    };
    let base = Sample {
        form: Some(forms()[0].clone()),
        themes: vec![themes()[0].clone()],
        catalog: ready.clone(),
        query: "hồi",
        options: hoi(),
    };
    match scenario {
        "empty" => Sample {
            form: None,
            themes: Vec::new(),
            query: "",
            options: TagOptions::Idle,
            ..base
        },
        "full" => Sample {
            themes: vec![
                themes()[0].clone(),
                themes()[1].clone(),
                themes()[2].clone(),
            ],
            ..base
        },
        "retired" => Sample {
            themes: vec![themes()[0].clone(), retired_theme()],
            query: "",
            options: TagOptions::Idle,
            ..base
        },
        "refused" => Sample {
            themes: vec![
                themes()[0].clone(),
                TagFacts {
                    problem: Some(
                        "Thẻ này vừa ngừng dùng nên không thể thêm. Chưa lưu thay đổi nào; bỏ hoặc thay thẻ này."
                            .into(),
                    ),
                    ..retired_theme()
                },
            ],
            query: "",
            options: TagOptions::Idle,
            ..base
        },
        "no-results" => Sample {
            query: "xyz",
            options: TagOptions::Ready(Vec::new()),
            ..base
        },
        "unavailable" => Sample {
            catalog: CatalogFacts::Unavailable,
            options: TagOptions::Unavailable,
            ..base
        },
        "long-label" => Sample {
            form: Some(tag(
                "ky",
                "Ký sự hành trình qua những miền đất phương Nam",
                Some("Ghi chép dài về những chuyến đi, người gặp và những đổi thay của làng quê, phố thị và sông nước."),
            )),
            themes: vec![
                tag("a", "Những người đàn bà gánh nước sông", None),
                TagFacts {
                    successor: Some(TagLink {
                        id: "b".into(),
                        label: "Ký ức tuổi thơ ở quê ngoại".into(),
                    }),
                    ..TagFacts {
                        retired: true,
                        ..TagFacts::active("c", "Tuổi thơ dưới mái hiên nhà bà ngoại")
                    }
                },
            ],
            query: "những",
            options: TagOptions::Ready(vec![TagOption {
                tag: tag(
                    "d",
                    "Những đổi thay của làng quê sau mùa nước nổi",
                    Some("Mô tả dài của ban biên tập để kiểm tra chữ xuống dòng ở màn hẹp và chữ 200%."),
                ),
                via_alias: Some("Làng quê đổi thay".into()),
            }]),
            ..base
        },
        _ => base,
    }
}

fn selector(scenario: &str) -> AnyView {
    if scenario == "interactive" {
        return view! { <Interactive /> }.into_any();
    }
    let s = sample(scenario);
    let query = RwSignal::new(s.query.to_string());
    view! {
        <div class="vkp-tag-specimen">
            <Text role=TypeRole::Caption tone=TextTone::Muted>"Mẫu · không gọi dịch vụ, không lưu"</Text>
            <TagSelector id="sample-tags"
                form=Signal::stored(s.form) themes=Signal::stored(s.themes)
                catalog=Signal::stored(s.catalog) query=query options=Signal::stored(s.options)
                on_form=Callback::new(|id: Option<String>| log(format!("dạng bài → {}", id.unwrap_or_else(|| "bỏ chọn".into()))))
                on_add_theme=Callback::new(|id: String| log(format!("thêm chủ đề → {id}")))
                on_remove_theme=Callback::new(|id: String| log(format!("bỏ chủ đề → {id}")))
                on_replace_theme=Callback::new(|(old, new): (String, String)| log(format!("thay {old} → {new}")))
                on_query=Callback::new(move |input: TextInput| {
                    log(format!("query “{}”{}", input.value, if input.composing { " (đang gõ dấu)" } else { "" }));
                    query.set(input.value);
                }) />
        </div>
    }
    .into_any()
}

/// An in-memory owner: chooses, removes and replaces, and filters candidates locally by a plain
/// prefix of the sample labels. It never claims a save.
#[component]
fn Interactive() -> impl IntoView {
    let form = RwSignal::new(None::<String>);
    let chosen = RwSignal::new(vec![retired_theme()]);
    let query = RwSignal::new(String::new());
    let composing = RwSignal::new(false);
    let catalog = CatalogFacts::Ready {
        forms: forms(),
        themes: themes(),
        truncated: false,
    };
    let options = Signal::derive(move || {
        let q = query.get().trim().to_lowercase();
        if composing.get() {
            TagOptions::Composing
        } else if q.is_empty() {
            TagOptions::Idle
        } else {
            TagOptions::Ready(
                themes()
                    .into_iter()
                    .filter(|t| t.label.to_lowercase().starts_with(&q))
                    .map(|tag| TagOption {
                        tag,
                        via_alias: None,
                    })
                    .collect(),
            )
        }
    });
    let exact = Signal::derive(move || {
        let q = query.get().trim().to_lowercase();
        themes()
            .into_iter()
            .find(|t| t.label.to_lowercase() == q)
            .map(|t| t.id)
    });
    let find = |id: &str| themes().into_iter().find(|t| t.id == id);
    view! {
        <div class="vkp-tag-specimen">
            <Text role=TypeRole::Caption tone=TextTone::Muted>"Mẫu · owner trong bộ nhớ, không lưu"</Text>
            <TagSelector id="sample-tags"
                form=Signal::derive(move || form.get().and_then(|id| forms().into_iter().find(|f| f.id == id)))
                themes=chosen catalog=Signal::stored(catalog) query options exact
                on_form=Callback::new(move |id: Option<String>| { log(format!("dạng bài → {id:?}")); form.set(id); })
                on_add_theme=Callback::new(move |id: String| {
                    if let Some(tag) = find(&id) {
                        chosen.update(|c| if c.len() < vink_patterns::THEME_LIMIT && !c.iter().any(|t| t.id == id) { c.push(tag) });
                        query.set(String::new());
                        log(format!("thêm chủ đề → {id}"));
                    }
                })
                on_remove_theme=Callback::new(move |id: String| { chosen.update(|c| c.retain(|t| t.id != id)); log(format!("bỏ chủ đề → {id}")); })
                on_replace_theme=Callback::new(move |(old, new): (String, String)| {
                    if let Some(tag) = find(&new) {
                        chosen.update(|c| if let Some(slot) = c.iter_mut().find(|t| t.id == old) { *slot = tag });
                        log(format!("thay {old} → {new}"));
                    }
                })
                on_query=Callback::new(move |input: TextInput| { composing.set(input.composing); query.set(input.value); }) />
        </div>
    }
}
