//! Fixed mock responses from the Gateway HTTP v1 examples 2–4. The preview never calls the API.

use leptos::prelude::*;
use serde_json::Value;
use vink_patterns::{
    NotificationGroup, NotificationLeading, NotificationRow, NotificationRowFacts,
};
use vink_ui::{
    BadgeState, Button, Dialog, ExpressiveShape, Heading, HeadingLevel, Icon, IconName, NavUtility,
    NotificationBadge, Person, ShapeTone, TypeRole,
};

const EXAMPLES: &str = include_str!("../fixtures/notifications-examples.json");
const GATEWAY: &str = include_str!(
    "../../../../../services/vink-app/contracts/http/v1/fixtures/notifications-gateway.json"
);

pub fn specimen(id: &str, scenario: &str) -> Option<AnyView> {
    (id == "composition.notifications").then(|| {
        let examples: Value = serde_json::from_str(EXAMPLES).expect("notification examples");
        if scenario == "badges" {
            badges(&examples["counts"])
        } else {
            inbox(&examples)
        }
    })
}

fn badge_state(response: &Value) -> BadgeState {
    let freshness = &response["freshness"];
    let lower_bound = freshness["count_is_lower_bound"] == true;
    let count = |key: &str| {
        freshness[key]
            .as_u64()
            .unwrap_or_default()
            .min(u32::MAX.into()) as u32
    };
    match (freshness["status"].as_str(), lower_bound) {
        (Some("fresh"), false) => BadgeState::Count(count("unseen_group_count")),
        (Some("fresh"), true) => BadgeState::LowerBound,
        (Some("stale"), false) => BadgeState::Stale(count("last_known_count")),
        (Some("stale"), true) => BadgeState::StaleLowerBound,
        _ => BadgeState::Unknown,
    }
}

fn badges(counts: &Value) -> AnyView {
    let fresh = badge_state(&counts[0]);
    let stale = badge_state(&counts[1]);
    let unavailable = badge_state(&counts[2]);
    view! {
        <div class="grid gap-vk-4">
            <div class="vk-ui-rail vk-notif-preview-rail">
                <span class="vk-ui-nav-item">
                    <span class="vk-ui-nav-item__indicator"><Icon name=IconName::ExploreOutline /></span>
                    <span class="vk-ui-nav-item__label">"Khám phá"</span>
                </span>
                <span class="vk-ui-nav-item">
                    <span class="vk-ui-nav-item__indicator"><Icon name=IconName::EditOutline /></span>
                    <span class="vk-ui-nav-item__label">"Bàn viết"</span>
                </span>
                <Bell state=fresh />
            </div>
            <ul class="m-0 flex list-none flex-wrap gap-vk-3 p-0">
                {[BadgeState::Hidden, fresh, BadgeState::LowerBound, stale, BadgeState::Stale(0), unavailable]
                    .into_iter()
                    .map(|state| view! {
                        <li class="relative p-vk-2">
                            <span class="inline-flex items-center gap-vk-1" role="img" aria-label=state.trigger_name("Thông báo")>
                                <Icon name=IconName::NotificationsNone />
                                <NotificationBadge state=Signal::stored(state) />
                            </span>
                        </li>
                    })
                    .collect_view()}
            </ul>
        </div>
    }.into_any()
}

#[component]
fn Bell(state: BadgeState) -> impl IntoView {
    let open = RwSignal::new(false);
    view! {
        <NavUtility
            id="notification-preview-bell"
            label="Thông báo"
            icon=IconName::NotificationsNone
            active_icon=IconName::Notifications
            controls="notification-preview-dialog"
            expanded=open
            accessible_label=Signal::stored(state.trigger_name("Thông báo"))
            badge=move || view! { <NotificationBadge state=Signal::stored(state) /> }
            on_open=Callback::new(move |()| open.set(true))
        />
        <Dialog
            id="notification-preview-dialog"
            title=Signal::stored("Thông báo".to_string())
            open=open
            focus_title=true
            on_dismiss=Callback::new(move |_| open.set(false))
            actions=move || view! {
                <Button on_press=Callback::new(move |_| open.set(false))>"Đóng"</Button>
            }
        />
    }
}

fn inbox(examples: &Value) -> AnyView {
    let gateway: Value = serde_json::from_str(GATEWAY).expect("notification Gateway fixture");
    let root = &gateway["response"]["items"][0];
    let items = &examples["inbox"]["items"];
    let decisions = &examples["decisions"];
    let comment = people_row(
        root,
        IconName::ChatBubbleOutline,
        "đã nhận xét tác phẩm của bạn",
    );
    let tham = people_row(&items[0], IconName::PenFlight, "đã Thấm tác phẩm của bạn");
    let rejected = decision_row(
        &decisions[0],
        !items[1]["is_read"].as_bool().unwrap_or(true),
    );
    let approved = decision_row(&decisions[1], true);
    let removed = decision_row(&decisions[2], false);
    let ung = NotificationRowFacts {
        leading: NotificationLeading::People {
            people: vec![Person {
                key: Some("ngoc-mai".into()),
                name: Some("Ngọc Mai".into()),
                ..Default::default()
            }],
            glyph: IconName::Heart,
        },
        actors: Some("Ngọc Mai".into()),
        action: "đã Ưng nhận xét của bạn".into(),
        excerpt: None,
        time_label: "18 tháng 11".into(),
        time_instant: None,
        unread: false,
        ..comment.clone()
    };

    view! {
        <section id="notification-preview" class="vk-notif-preview-inbox grid w-full min-w-0 gap-vk-4 px-vk-2 py-vk-2">
            <Heading level=HeadingLevel::H2 role=TypeRole::Headline>"Thông báo"</Heading>
            <p class="m-0 text-vk-on-surface-variant">"Dữ liệu mẫu từ hợp đồng Gateway v1."</p>
            <NotificationGroup label="Hôm nay" show_heading=true>
                <NotificationRow facts=comment />
                <NotificationRow facts=tham />
                <NotificationRow facts=approved />
                <NotificationRow facts=removed />
                <NotificationRow facts=rejected />
            </NotificationGroup>
            <NotificationGroup label="7 ngày qua" show_heading=true>
                <NotificationRow facts=ung />
            </NotificationGroup>
        </section>
    }.into_any()
}

fn people_row(item: &Value, glyph: IconName, action: &str) -> NotificationRowFacts {
    let people: Vec<Person> = item["actors_preview"]
        .as_array()
        .expect("example actors")
        .iter()
        .map(|actor| Person {
            key: string(&actor["handle"]),
            name: string(&actor["pen_name"]).or_else(|| string(&actor["label"])),
            ..Default::default()
        })
        .collect();
    let names: Vec<&str> = people
        .iter()
        .filter_map(|person| person.name.as_deref())
        .collect();
    let count = item["actor_count"].as_u64().unwrap_or_default();
    let actors = match names.as_slice() {
        [a, b, ..] if count > 3 => Some(format!("{a}, {b} và {} người khác", count - 2)),
        [a, b, c] => Some(format!("{a}, {b} và {c}")),
        [a, b] => Some(format!("{a} và {b}")),
        [a] => Some((*a).to_string()),
        _ => None,
    };

    NotificationRowFacts {
        href: "#composition.notifications/default".into(),
        leading: NotificationLeading::People { people, glyph },
        actors,
        action: action.into(),
        title: string(&item["target"]["work_title"]).unwrap_or_default(),
        excerpt: string(&item["excerpt"]["text"])
            .map(|text| (text, item["excerpt"]["truncated"] == true)),
        revision_label: None,
        time_label: "2 giờ".into(),
        time_instant: string(&item["updated_at"]),
        action_required: item["summary"]["action_required"] == true,
        unread: item["is_read"] == false,
    }
}

fn decision_row(item: &Value, unread: bool) -> NotificationRowFacts {
    let (shape, tone, icon, action) = match item["outcome"].as_str() {
        Some("approved") => (
            ExpressiveShape::Cookie,
            ShapeTone::PrimaryFixed,
            IconName::Check,
            "Tác phẩm của bạn đã được duyệt",
        ),
        Some("rejected") => (
            ExpressiveShape::Burst,
            ShapeTone::Tertiary,
            IconName::Edit,
            "Tác phẩm của bạn bị từ chối",
        ),
        _ => (
            ExpressiveShape::Cookie,
            ShapeTone::Neutral,
            IconName::Block,
            "Nhận xét của bạn đã bị gỡ",
        ),
    };

    NotificationRowFacts {
        href: "#composition.notifications/default".into(),
        leading: NotificationLeading::System { shape, tone, icon },
        actors: None,
        action: action.into(),
        title: string(&item["work_title"]).unwrap_or_else(|| "Tác phẩm không còn hiển thị".into()),
        excerpt: None,
        revision_label: (item["kind"] == "work_moderation_decision")
            .then(|| "Bản 20 tháng 11".into()),
        time_label: "2 giờ".into(),
        time_instant: string(&item["decided_at"]),
        action_required: item["action_required"] == true,
        unread,
    }
}

fn string(value: &Value) -> Option<String> {
    value.as_str().map(str::to_owned)
}
