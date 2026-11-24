//! Presentation for Notifications R4. The host resolves access, destinations and read intents.

use leptos::ev::MouseEvent;
use leptos::prelude::*;
use vink_ui::{Avatar, AvatarSize, ExpressiveShape, IconName, Person, ShapeMark, ShapeTone};

/// The visible actors and their type glyph, or a mark for a system decision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NotificationLeading {
    People {
        people: Vec<Person>,
        glyph: IconName,
    },
    System {
        shape: ExpressiveShape,
        tone: ShapeTone,
        icon: IconName,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotificationRowFacts {
    pub href: String,
    pub leading: NotificationLeading,
    pub actors: Option<String>,
    pub action: String,
    pub title: String,
    /// The authorized root/reply text and whether the adapter truncated it.
    pub excerpt: Option<(String, bool)>,
    pub revision_label: Option<String>,
    pub time_label: String,
    pub time_instant: Option<String>,
    pub action_required: bool,
    pub unread: bool,
}

impl NotificationRowFacts {
    fn accessible_name(&self, excerpt: Option<&str>) -> String {
        let mut parts = Vec::new();
        if self.unread {
            parts.push("Chưa đọc".to_string());
        }
        parts.push(match &self.actors {
            Some(actors) => format!("{actors} {}", self.action),
            None => self.action.clone(),
        });
        parts.push(self.title.clone());
        if let Some(excerpt) = excerpt {
            parts.push(format!("Trích: {excerpt}"));
        }
        if let Some(revision) = &self.revision_label {
            parts.push(revision.clone());
        }
        parts.push(self.time_label.clone());
        if self.action_required {
            parts.push("Cần xử lý".to_string());
        }
        format!("{}.", parts.join(". "))
    }
}

/// A segmented list with an optional visible date heading. The host supplies rows in server
/// order and can omit the heading when only one group is present.
#[component]
pub fn NotificationGroup(
    #[prop(into)] label: String,
    #[prop(optional)] show_heading: bool,
    children: Children,
) -> impl IntoView {
    view! {
        <div class="vk-notif-group">
            {show_heading.then(|| view! {
                <h3 class="vk-notif-group__heading">{label.clone()}</h3>
            })}
            <ul class="vk-notif-group__list" aria-label=label>{children()}</ul>
        </div>
    }
}

/// One link per row. The host may handle activation to record Read for the observed version.
#[component]
pub fn NotificationRow(
    facts: NotificationRowFacts,
    #[prop(optional)] on_activate: Option<Callback<MouseEvent>>,
) -> impl IntoView {
    let excerpt = facts
        .excerpt
        .as_ref()
        .map(|(text, truncated)| format!("“{text}{}”", if *truncated { "…" } else { "" }));
    let name = facts.accessible_name(excerpt.as_deref());
    let leading = match facts.leading {
        NotificationLeading::People { people, glyph } => {
            let grouped = people.len() > 1;
            let size = if grouped {
                AvatarSize::Small
            } else {
                AvatarSize::Reader
            };
            view! {
                <span class="vk-notif-row__person" class:vk-notif-row__person--group=grouped>
                    <span class="vk-notif-row__avatars">
                        {people.into_iter().take(2).map(|person| view! {
                            <Avatar
                                name=person.name.unwrap_or_default()
                                identity_key=person.key.unwrap_or_default()
                                image=Signal::stored(person.image)
                                size=size
                                decorative=true
                            />
                        }).collect_view()}
                    </span>
                    <ShapeMark shape=ExpressiveShape::Cookie icon=glyph glyph=true />
                </span>
            }
            .into_any()
        }
        NotificationLeading::System { shape, tone, icon } => {
            view! { <ShapeMark shape=shape tone=tone icon=icon /> }.into_any()
        }
    };
    view! {
        <li class="vk-notif-row" class:vk-notif-row--unread=facts.unread>
            <a
                class="vk-notif-row__link"
                href=facts.href
                aria-label=name
                on:click=move |event| {
                    if let Some(activate) = on_activate {
                        activate.run(event);
                    }
                }
            >
                <span class="vk-notif-row__leading" aria-hidden="true">{leading}</span>
                <span class="vk-notif-row__text">
                    <span class="vk-notif-row__sentence">
                        {facts.actors.map(|actors| view! { <strong>{actors}</strong>" " })}
                        {facts.action}
                    </span>
                    <span class="vk-notif-row__title">{facts.title}</span>
                    {excerpt.map(|text| view! { <span class="vk-notif-row__excerpt">{text}</span> })}
                    <span class="vk-notif-row__meta">
                        {facts.revision_label.map(|revision| view! { <span>{revision}" ·"</span> })}
                        <time datetime=facts.time_instant>{facts.time_label}</time>
                        {facts.action_required.then(|| view! {
                            <span class="vk-notif-row__chip">"Cần xử lý"</span>
                        })}
                    </span>
                </span>
                <span class="vk-notif-row__dot" aria-hidden="true"></span>
            </a>
        </li>
    }
}
