use leptos::{prelude::*, task::spawn_local_scoped_with_cancellation};
use leptos_meta::{Meta, Title};
use vink_ui::{Icon, IconName};

use crate::follow_state::safe_profile_return_for_owner;
use crate::{account::Session, api, contract::profile::OwnProfile};

mod list;

#[derive(Clone, PartialEq, Eq)]
pub(super) struct VerifiedOwner {
    pub scope: String,
    pub profile: OwnProfile,
}

pub(super) fn verified_owner(handle: String) -> (Memo<Option<VerifiedOwner>>, Memo<bool>) {
    let session = Session::current();
    let handle = StoredValue::new(handle);
    let loaded = RwSignal::new(None::<(String, u64, Option<OwnProfile>)>);
    let revision = RwSignal::new(0u64);
    super::super::profile::observe_changes(move || revision.update(|revision| *revision += 1));
    Effect::new(move |_| {
        let stamp = revision.get();
        let scope = session.profile_scope.get();
        loaded.set(None);
        let Some(scope) = scope else {
            return;
        };
        spawn_local_scoped_with_cancellation(async move {
            let profile = api::load_own_profile().await.ok();
            if session.profile_scope.try_get_untracked().flatten().as_ref() == Some(&scope)
                && revision.try_get_untracked() == Some(stamp)
            {
                let profile = profile.filter(|profile| {
                    session.principal.get_untracked().as_ref() == Some(&profile.principal_id)
                        && handle.with_value(|handle| *handle == profile.handle)
                });
                loaded.set(Some((scope, stamp, profile)));
            }
        });
    });
    let verified = Memo::new(move |_| {
        let scope = session.profile_scope.get()?;
        let stamp = revision.get();
        loaded
            .get()
            .filter(|(key, checked, _)| *key == scope && *checked == stamp)
    });
    let owner = Memo::new(move |_| {
        let (scope, _, profile) = verified.get()?;
        Some(VerifiedOwner {
            scope,
            profile: profile?,
        })
    });
    (owner, Memo::new(move |_| verified.with(Option::is_some)))
}

pub(super) fn route_owner(segment: String) -> Memo<Option<VerifiedOwner>> {
    let path = format!("/{segment}/da-luu");
    let Some(handle) = segment
        .strip_prefix('@')
        .filter(|handle| safe_profile_return_for_owner(&path, Some(handle)).is_some())
    else {
        return Memo::new(|_| None);
    };
    let (owner, checked) = verified_owner(handle.to_owned());
    let session = Session::current();
    Effect::new(move |_| {
        if !session.checking()
            && !session.private_scope_fenced()
            && owner.with(Option::is_none)
            && (checked.get()
                || matches!(
                    session.standing.get(),
                    crate::session::Standing::Anonymous
                        | crate::session::Standing::NeedsPrincipal
                        | crate::session::Standing::Unavailable
                        | crate::session::Standing::Suspended
                ))
        {
            list::focus_denied_return(&path);
        }
    });
    owner
}

#[component]
pub(super) fn OwnSavedProfile(owner: VerifiedOwner) -> impl IntoView {
    use super::super::{follow, handle};
    let profile = owner.profile.clone();
    let own = RwSignal::new(None);
    let (public, on_summary) = handle::profile_summary(profile.handle.clone(), Default::default());
    let name = profile.pen_name;
    view! {
        <Title text=format!("Đã lưu · {name} · Vink") />
        <Meta name="robots" content="noindex" />
        <article class="vk-author-public">
            <handle::ProfileIdentity pen_name=name handle=profile.handle.clone() />
            <follow::ProfileActionRegion handle=profile.handle.clone() on_summary own_relationships=own />
            <follow::ProfileRelationships public=public.into() own=own.read_only()
                on_retry=Callback::new(move |()| on_summary.run(None)) />
            {profile.bio.map(|bio| view! { <p class="vk-author-bio">{bio}</p> })}
            <section class="vk-saved-section" aria-labelledby="saved-heading">
                <div id="profile-view-slot" class="vk-profile-view-slot">
                    <Modes handle=profile.handle saved=true />
                </div>
                <list::SavedList owner />
            </section>
        </article>
    }
}

#[component]
pub(super) fn Modes(handle: String, saved: bool) -> impl IntoView {
    let works_href = format!("/@{handle}");
    let saved_target = format!("{works_href}/da-luu");
    let saved_href = safe_profile_return_for_owner(&saved_target, Some(&handle))
        .expect("verified canonical profile handle");
    let links = move |suffix: &str| {
        [(false, "Tác phẩm", works_href.clone()), (true, "Đã lưu", saved_href.clone())]
            .into_iter()
            .map(|(is_saved, label, href)| {
                let id = if is_saved { "profile-mode-saved" } else { "profile-mode-works" };
                view! {
                    <a id=format!("{id}{suffix}") href=href aria-current=(is_saved == saved).then_some("page")>
                        {(is_saved == saved).then(|| view! { <Icon name=IconName::Check /> })}
                        {label}
                    </a>
                }
            }).collect_view()
    };
    view! {
        <nav id="profile-modes" class="vk-profile-modes" aria-label="Hồ sơ của tôi">
            <div class="vk-explore-modes vk-profile-modes__wide">{links("")}</div>
            <details class="vk-profile-modes__compact">
                <summary>{if saved { "Đã lưu" } else { "Tác phẩm" }}</summary>
                <div class="vk-explore-modes">{links("-compact")}</div>
            </details>
        </nav>
    }
}
