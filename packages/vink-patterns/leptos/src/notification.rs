//! Presentation for Notifications R4. The host resolves access, destinations and read intents.

use leptos::ev::MouseEvent;
use leptos::prelude::*;
use vink_ui::{
    Avatar, AvatarGroup, AvatarSize, ExpressiveShape, IconName, Person, ShapeMark, ShapeTone,
};

/// The visible actors and their type glyph, or a mark for a system decision.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
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

/// Product rows carry an instant. Preview can supply its fixed specimen caption.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum NotificationTime {
    Instant(String),
    Caption {
        label: String,
        instant: Option<String>,
    },
}

impl NotificationTime {
    fn render(self, mode: crate::TimeMode, id: String, context: &'static str) -> AnyView {
        match self {
            Self::Instant(instant) => {
                view! { <crate::Timestamp instant mode description_id=id context /> }.into_any()
            }
            Self::Caption { label, instant } => {
                view! { <time datetime=instant>{label}</time> }.into_any()
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct NotificationRowFacts {
    pub href: String,
    pub leading: NotificationLeading,
    pub actors: Option<String>,
    pub action: String,
    pub title: String,
    /// The authorized root/reply text and whether the adapter truncated it.
    pub excerpt: Option<(String, bool)>,
    pub revision: Option<NotificationTime>,
    pub updated: NotificationTime,
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
        if let Some(NotificationTime::Caption { label, .. }) = &self.revision {
            parts.push(format!("Bản {label}"));
        }
        if let NotificationTime::Caption { label, .. } = &self.updated {
            parts.push(label.clone());
        }
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
    #[prop(optional, into)] show_heading_when: Option<Signal<bool>>,
    children: Children,
) -> impl IntoView {
    let title = label.clone();
    view! {
        <div class="vk-notif-group">
            <h3 class="vk-notif-group__heading" hidden=move || !show_heading_when.map_or(show_heading, |when| when.get())>{title}</h3>
            <ul class="vk-notif-group__list" aria-label=label>{children()}</ul>
        </div>
    }
}

/// One link per row. The host may handle activation to record Read for the observed version.
#[component]
pub fn NotificationRow(
    facts: NotificationRowFacts,
    #[prop(optional)] description_id: Option<String>,
    #[prop(optional)] on_activate: Option<Callback<MouseEvent>>,
) -> impl IntoView {
    let excerpt = facts
        .excerpt
        .as_ref()
        .map(|(text, truncated)| format!("“{text}{}”", if *truncated { "…" } else { "" }));
    let name = facts.accessible_name(excerpt.as_deref());
    let time_id = description_id.clone().unwrap_or_default();
    let revision_id = description_id
        .as_ref()
        .filter(|_| facts.revision.is_some())
        .map(|id| format!("{id}-revision"));
    let described_by = description_id.map(|id| {
        revision_id
            .as_ref()
            .map_or_else(|| id.clone(), |revision| format!("{revision} {id}"))
    });
    let revision = facts.revision.map(|stamp| {
        stamp.render(
            crate::TimeMode::Date,
            revision_id.unwrap_or_default(),
            "Bản gửi",
        )
    });
    let updated = facts
        .updated
        .render(crate::TimeMode::Social, time_id, "Cập nhật");
    let leading = match facts.leading {
        NotificationLeading::People { people, glyph } => {
            let actors = if people.len() > 1 {
                view! { <AvatarGroup people label="Người đọc" max_visible=2 /> }.into_any()
            } else {
                let person = people.into_iter().next().unwrap_or_default();
                view! {
                    <Avatar
                        name=person.name.unwrap_or_default()
                        identity_key=person.key.unwrap_or_default()
                        image=Signal::stored(person.image)
                        size=AvatarSize::Reader
                        decorative=true
                    />
                }
                .into_any()
            };
            view! {
                <div class="vk-notif-row__person">
                    {actors}
                    <ShapeMark shape=ExpressiveShape::Cookie icon=glyph glyph=true />
                </div>
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
                aria-describedby=described_by
                data-vink-managed-navigation=on_activate.is_some().then_some("")
                on:click:capture=move |event| {
                    if let Some(activate) = on_activate {
                        activate.run(event);
                    }
                }
            >
                <div class="vk-notif-row__leading" aria-hidden="true">{leading}</div>
                <span class="vk-notif-row__text">
                    <span class="vk-notif-row__sentence">
                        {facts.actors.map(|actors| view! { <strong>{actors}</strong>" " })}
                        {facts.action}
                    </span>
                    <span class="vk-notif-row__title">{facts.title}</span>
                    {excerpt.map(|text| view! { <span class="vk-notif-row__excerpt">{text}</span> })}
                    <span class="vk-notif-row__meta">
                        {revision.map(|date| view! { <span>"Bản " {date} " ·"</span> })}
                        {updated}
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
