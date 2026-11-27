mod handle;

use leptos::{prelude::*, task::spawn_local};
use unicode_normalization::UnicodeNormalization;
use vink_ui::{
    ActionState, Button, ButtonKind, ButtonVariant, FieldMode, FieldVariant, Heading, HeadingLevel,
    TextArea, TextField, TextInput, TypeRole,
};
use wasm_bindgen::JsCast;

use super::privacy::{PrivacySelect, visibility_choice};
use crate::account::Session;
use crate::api;
use crate::contract::{
    ApiError,
    profile::{OwnProfile, Policies, SaveProfile, Visibility},
};

#[component]
pub(super) fn OwnProfileHost(scope: String) -> impl IntoView {
    let session = Session::current();
    let attempt = RwSignal::new(0u64);
    let key = StoredValue::new(scope);
    let loaded = LocalResource::new(move || {
        attempt.track();
        async move { api::load_own_profile().await }
    });
    view! {
        <Suspense fallback=|| view! { <crate::ui::PendingRead label="Đang tải hồ sơ của bạn…" /> }>
            {move || loaded.get().map(|result| match result {
                Ok(profile) if same_owner(session, key, &profile) =>
                    view! { <ProfileForm scope=key initial=profile /> }.into_any(),
                _ => view! {
                    <p role="alert">"Chưa tải được hồ sơ của bạn."</p>
                    <Button variant=ButtonVariant::Tonal
                        on_press=Callback::new(move |()| attempt.update(|value| *value += 1))>"Thử lại"</Button>
                }.into_any(),
            })}
        </Suspense>
    }
}

fn same_owner(session: Session, scope: StoredValue<String>, profile: &OwnProfile) -> bool {
    scope.try_get_value().is_some_and(|scope| {
        session.profile_scope.get_untracked().as_deref() == Some(scope.as_str())
            && session.principal.get_untracked().as_deref() == Some(profile.principal_id.as_str())
    })
}

fn announce_change() {
    crate::public_reads::invalidate_author_attribution();
    if let Ok(channel) = web_sys::BroadcastChannel::new(super::CHANNEL) {
        let _ = channel.post_message(&wasm_bindgen::JsValue::NULL);
        channel.close();
    }
}

#[derive(Clone)]
struct Draft {
    scope: String,
    base: OwnProfile,
    name: String,
    bio: String,
    policies: Policies,
}

#[derive(Clone, Copy)]
struct DraftSlot(RwSignal<Option<Draft>>);

pub(crate) fn provide_draft(session: Session) {
    let slot = RwSignal::new(None::<Draft>);
    provide_context(DraftSlot(slot));
    Effect::new(move |_| {
        let scope = session.profile_scope.get();
        if slot.with_untracked(|value| {
            value
                .as_ref()
                .is_some_and(|draft| scope.as_deref() != Some(draft.scope.as_str()))
        }) {
            slot.set(None);
        }
    });
}

#[derive(Clone, Copy)]
struct Form {
    scope: StoredValue<String>,
    base: RwSignal<OwnProfile>,
    name: RwSignal<String>,
    bio: RwSignal<String>,
    policies: RwSignal<Policies>,
    busy: RwSignal<bool>,
    composing: RwSignal<bool>,
    message: RwSignal<&'static str>,
    bad_field: RwSignal<Option<&'static str>>,
    conflict: RwSignal<Option<OwnProfile>>,
    reconcile: RwSignal<Option<SaveProfile>>,
}

impl Form {
    fn adopt(self, profile: OwnProfile) {
        self.name.set(profile.pen_name.clone());
        self.bio.set(profile.bio.clone().unwrap_or_default());
        self.policies.set(profile.policies);
        self.base.set(profile);
    }

    fn command(self) -> SaveProfile {
        let base = self.base.get_untracked();
        let bio = self.bio.get_untracked().trim().replace("\r\n", "\n");
        SaveProfile {
            principal_id: base.principal_id,
            expected_version: base.profile_version,
            pen_name: self.name.get_untracked().trim().nfc().collect(),
            bio: (!bio.is_empty()).then_some(bio),
            policies: self.policies.get_untracked(),
        }
    }

    fn live(self, session: Session) -> bool {
        self.base
            .try_get_untracked()
            .is_some_and(|base| same_owner(session, self.scope, &base))
    }

    fn save(self, session: Session) {
        if self.busy.get_untracked()
            || self.composing.get_untracked()
            || self.reconcile.with_untracked(Option::is_some)
            || self.conflict.with_untracked(Option::is_some)
            || !self.live(session)
        {
            return;
        }
        let request = self.command();
        self.busy.set(true);
        self.message.set("Đang lưu hồ sơ…");
        self.bad_field.set(None);
        spawn_local(async move {
            let result = api::save_profile(&request).await;
            // The server can have committed after this form's route or session ended.
            if result.is_ok()
                || matches!(
                    result,
                    Err(ApiError::Network | ApiError::Unavailable | ApiError::Unexpected { .. })
                )
            {
                announce_change();
            }
            if !self.live(session) {
                return;
            }
            self.busy.set(false);
            match result {
                Ok(saved)
                    if same_owner(session, self.scope, &saved)
                        && request.matches_successor(&saved) =>
                {
                    self.adopt(saved);
                    self.message.set("Đã lưu hồ sơ.");
                    session.refresh();
                }
                Err(ApiError::Invalid { field: Some(field) }) if field == "policies" => {
                    self.message
                        .set("Chưa thể công khai lại trường đã đặt Chỉ mình tôi.");
                }
                Err(ApiError::Invalid { field }) => {
                    let (id, message) = match field.as_deref() {
                        Some("pen_name") => {
                            ("profile-pen-name", "Nhập bút danh hợp lệ, tối đa 80 ký tự.")
                        }
                        Some("bio") => ("profile-bio", "Nhập Giới thiệu hợp lệ, tối đa 500 ký tự."),
                        _ => ("edit-profile-heading", "Có trường hồ sơ chưa hợp lệ."),
                    };
                    self.bad_field.set(Some(id));
                    self.message.set(message);
                    super::super::focus_recovery(id);
                }
                Err(ApiError::Unauthenticated | ApiError::Forbidden | ApiError::NotFound) => {
                    session.fence_profile();
                }
                _ => {
                    self.reconcile.set(Some(request));
                    self.read_back(session);
                }
            }
        });
    }

    fn read_back(self, session: Session) {
        let Some(request) = self.reconcile.get_untracked() else {
            return;
        };
        self.busy.set(true);
        self.message.set("Đang đối chiếu với hồ sơ đã lưu…");
        spawn_local(async move {
            let result = api::load_own_profile().await;
            if !self.live(session) {
                return;
            }
            self.busy.set(false);
            match result {
                Ok(stored) if same_owner(session, self.scope, &stored) => {
                    self.reconcile.set(None);
                    if request.matches_successor(&stored) {
                        announce_change();
                        self.adopt(stored);
                        self.message
                            .set("Hồ sơ đã lưu khớp với thay đổi bạn vừa gửi.");
                        session.refresh();
                    } else if stored.profile_version != request.expected_version {
                        self.conflict.set(Some(stored));
                        self.message
                            .set("Hồ sơ đã thay đổi ở nơi khác. Bản nháp của bạn vẫn ở đây.");
                    } else {
                        self.message
                            .set("Chưa thấy thay đổi trên hồ sơ. Bạn có thể lưu lại.");
                    }
                }
                Err(ApiError::Unauthenticated | ApiError::Forbidden | ApiError::NotFound) => {
                    session.fence_profile();
                }
                _ => self
                    .message
                    .set("Chưa xác nhận được kết quả lưu. Hãy đối chiếu lại."),
            }
        });
    }
}

#[component]
fn ProfileForm(scope: StoredValue<String>, initial: OwnProfile) -> impl IntoView {
    let session = Session::current();
    let DraftSlot(slot) = expect_context();
    let key = scope.get_value();
    let saved_draft = slot.get_untracked().filter(|draft| draft.scope == key);
    let (base, name, bio, policies, conflict) = match saved_draft {
        Some(draft) => {
            let changed = draft.base.profile_version != initial.profile_version;
            (
                draft.base,
                draft.name,
                draft.bio,
                draft.policies,
                changed.then_some(initial),
            )
        }
        None => (
            initial.clone(),
            initial.pen_name,
            initial.bio.unwrap_or_default(),
            initial.policies,
            None,
        ),
    };
    let form = Form {
        scope,
        name: RwSignal::new(name),
        bio: RwSignal::new(bio),
        policies: RwSignal::new(policies),
        base: RwSignal::new(base),
        busy: RwSignal::new(false),
        composing: RwSignal::new(false),
        message: RwSignal::new(""),
        bad_field: RwSignal::new(None),
        conflict: RwSignal::new(conflict),
        reconcile: RwSignal::new(None),
    };
    Effect::new(move |_| {
        let (base, name, bio, policies) = (
            form.base.get(),
            form.name.get(),
            form.bio.get(),
            form.policies.get(),
        );
        if session.profile_scope.get_untracked().as_deref() == Some(key.as_str()) {
            let changed = name != base.pen_name
                || bio != base.bio.as_deref().unwrap_or_default()
                || policies != base.policies;
            slot.set(changed.then(|| Draft {
                scope: key.clone(),
                base,
                name,
                bio,
                policies,
            }));
        }
    });
    let handle = handle::Controller::new(
        session,
        scope,
        form.base.with_untracked(|base| base.principal_id.clone()),
    );
    // A successful profile save may carry an old Handle snapshot or race a Handle write.
    // Refresh the dedicated Handle endpoint without adopting any of its fields into this draft.
    let profile_version = Memo::new(move |_| form.base.with(|base| base.profile_version));
    Effect::new(move |_| {
        profile_version.track();
        handle.refresh();
    });
    super::observe_changes(move || handle.refresh());
    let blocked = move || form.busy.get() || form.reconcile.with(Option::is_some);
    let field_mode = Signal::derive(move || {
        if blocked() {
            FieldMode::ReadOnly
        } else {
            FieldMode::Editable
        }
    });
    let narrows_privacy = move || {
        let mut baseline = form.base.with(|base| base.policies);
        let mut draft = form.policies.get();
        POLICY_FIELDS.iter().any(|field| {
            *field(&mut baseline) == Visibility::Everyone && *field(&mut draft) == Visibility::Owner
        })
    };
    let has_locked_privacy = move || {
        let mut stored = form.base.with(|base| base.policies);
        POLICY_FIELDS
            .iter()
            .any(|field| *field(&mut stored) == Visibility::Owner)
    };
    let error_for = move |id: &'static str| {
        Signal::derive(move || {
            (form.bad_field.get() == Some(id)).then(|| form.message.get().to_string())
        })
    };
    let typed = move |target: RwSignal<String>| {
        Callback::new(move |input: TextInput| {
            form.composing.set(input.composing);
            if !blocked() {
                target.set(input.value);
            }
        })
    };
    let save_state = Signal::derive(move || {
        if form.busy.get() {
            ActionState::Pending {
                status: "Đang xử lý".into(),
            }
        } else if form.reconcile.with(Option::is_some) || form.conflict.with(Option::is_some) {
            ActionState::Disabled { reason: None }
        } else {
            ActionState::Enabled
        }
    });
    let privacy = NodeRef::<leptos::html::Div>::new();
    Effect::new(move |_| {
        if let Some(region) = privacy.get()
            && window()
                .location()
                .hash()
                .is_ok_and(|hash| hash == "#quyen-rieng-tu")
            && document().active_element().is_none_or(|active| {
                active.tag_name() == "BODY"
                    || matches!(active.id().as_str(), "noi-dung" | "edit-profile-heading")
            })
        {
            region.scroll_into_view();
            let _ = region.focus();
        }
    });
    view! {
        <form class="vk-profile-form grid min-w-0 gap-vk-4" autocomplete="off" novalidate
            on:compositionstart=move |_| form.composing.set(true)
            on:compositionend=move |_| form.composing.set(false)
            on:submit=move |event| { event.prevent_default(); form.save(session); }>
            <div class="flex min-w-0 items-start gap-vk-3">
                <div class="shrink-0"><crate::appearance::AccountAvatar /></div>
                <div class="min-w-0 flex-1">
                    <TextField id="profile-pen-name" label="Bút danh" value=form.name
                        variant=FieldVariant::FloatingTonal mode=field_mode
                        on_input=typed(form.name) error=error_for("profile-pen-name")
                        help="Luôn công khai, có thể trùng tên." />
                </div>
            </div>
            <handle::HandleRow controller=handle />
            <TextArea id="profile-bio" label="Giới thiệu" value=form.bio rows=3 grow=true
                variant=FieldVariant::FloatingTonal mode=field_mode
                on_input=typed(form.bio) error=error_for("profile-bio") />
            <div id="quyen-rieng-tu" node_ref=privacy tabindex="-1" role="group"
                aria-labelledby="profile-privacy-heading"
                class="grid min-w-0 gap-vk-2 rounded-vk-lg focus-visible:outline-2 focus-visible:[outline-color:var(--vk-focus-ring)]">
                <Heading id="profile-privacy-heading" level=HeadingLevel::H2 role=TypeRole::Title
                    attr:class="m-0">"Quyền riêng tư"</Heading>
                <PolicyField form label="Ảnh đại diện" id="profile-avatar-policy" access=|p| &mut p.avatar />
                <PolicyField form label="Giới thiệu" id="profile-bio-policy" access=|p| &mut p.bio />
                <PolicyField form label="Danh sách người theo dõi" id="profile-followers-list" access=|p| &mut p.followers_list />
                <PolicyField form label="Số người theo dõi" id="profile-followers-count" access=|p| &mut p.followers_count />
                <PolicyField form label="Danh sách đang theo dõi" id="profile-following-list" access=|p| &mut p.following_list />
                <PolicyField form label="Số người đang theo dõi" id="profile-following-count" access=|p| &mut p.following_count />
                <Show when=has_locked_privacy>
                    <p id="profile-privacy-lock-note" class="m-0 text-vk-caption text-vk-on-surface-variant">
                        "Trường đã đặt Chỉ mình tôi hiện chưa thể công khai lại."
                    </p>
                </Show>
                <div role="status" aria-atomic="true">
                    {move || narrows_privacy().then(|| view! {
                        <p id="profile-privacy-change-note" class="m-0 rounded-vk-md bg-vk-warning-container p-vk-3 text-vk-caption text-vk-on-warning-container">
                            "Sau khi lưu ở chế độ Chỉ mình tôi, trường vừa đổi hiện chưa thể công khai lại."
                        </p>
                    })}
                </div>
            </div>
            {move || form.conflict.get().map(|current| view! {
                <Conflict form current />
            })}
            <div class="vk-profile-actions grid gap-vk-2">
                <p id="profile-message" role="status" aria-atomic="true">{move || form.message.get()}</p>
                <div class="flex flex-wrap items-center gap-vk-3">
                    <Button id="profile-save" kind=ButtonKind::Submit state=save_state
                        described_by="profile-message">"Lưu"</Button>
                    <Show when=move || form.reconcile.with(Option::is_some) && !form.busy.get()>
                        <Button variant=ButtonVariant::Tonal
                            on_press=Callback::new(move |()| form.read_back(session))>"Đối chiếu lại"</Button>
                    </Show>
                </div>
            </div>
        </form>
        <handle::HandleDialog controller=handle />
    }
}

type PolicyAccess = fn(&mut Policies) -> &mut Visibility;

const POLICY_FIELDS: [PolicyAccess; 6] = [
    |p| &mut p.avatar,
    |p| &mut p.bio,
    |p| &mut p.followers_list,
    |p| &mut p.followers_count,
    |p| &mut p.following_list,
    |p| &mut p.following_count,
];

#[component]
fn PolicyField(
    form: Form,
    label: &'static str,
    id: &'static str,
    access: PolicyAccess,
) -> impl IntoView {
    // SOC-OPEN-01: the stored policy determines whether the field can be widened.
    let locked =
        Signal::derive(move || *access(&mut form.base.get().policies) == Visibility::Owner);
    let value = Signal::derive(move || *access(&mut form.policies.get()));
    let disabled = Signal::derive(move || form.busy.get() || form.reconcile.with(Option::is_some));
    let on_change = move |picked| {
        if !locked.get_untracked() && !disabled.get_untracked() {
            form.policies.update(|policies| *access(policies) = picked);
        }
    };
    view! {
        <PrivacySelect id label value locked disabled on_change
            lock_description_id="profile-privacy-lock-note" />
    }
}

#[component]
fn Conflict(form: Form, current: OwnProfile) -> impl IntoView {
    let stored = current.clone();
    let p = current.policies;
    view! {
        <section class="vk-profile-conflict wrap-anywhere p-vk-3 rounded-vk-md bg-vk-surface-container" aria-labelledby="profile-conflict-heading">
            <h2 id="profile-conflict-heading">"Bản đang lưu trên Vink"</h2>
            <dl>
                <dt>"Bút danh"</dt><dd>{current.pen_name}</dd>
                <dt>"Giới thiệu"</dt><dd>{current.bio.unwrap_or_else(|| "Chưa điền".into())}</dd>
                <dt>"Ảnh đại diện"</dt><dd>{visibility_choice(p.avatar).label}</dd>
                <dt>"Quyền xem giới thiệu"</dt><dd>{visibility_choice(p.bio).label}</dd>
                <dt>"Danh sách người theo dõi"</dt><dd>{visibility_choice(p.followers_list).label}</dd>
                <dt>"Số người theo dõi"</dt><dd>{visibility_choice(p.followers_count).label}</dd>
                <dt>"Danh sách đang theo dõi"</dt><dd>{visibility_choice(p.following_list).label}</dd>
                <dt>"Số người đang theo dõi"</dt><dd>{visibility_choice(p.following_count).label}</dd>
            </dl>
            <Button variant=ButtonVariant::Tonal on_press=Callback::new(move |()| {
                form.adopt(stored.clone());
                form.conflict.set(None);
                form.message.set("Đang chỉnh từ bản đã lưu.");
                focus_name();
            })>"Dùng bản đã lưu"</Button>
            <Button variant=ButtonVariant::Text on_press=Callback::new(move |()| {
                if let Some(current) = form.conflict.get_untracked() {
                    // Keep the draft text. Adopt any newer policy that already narrowed.
                    form.policies.update(|draft| {
                        let mut stored = current.policies;
                        for field in POLICY_FIELDS {
                            if *field(&mut stored) == Visibility::Owner {
                                *field(draft) = Visibility::Owner;
                            }
                        }
                    });
                    form.base.set(current);
                    form.conflict.set(None);
                    form.message.set("Đã giữ chữ trong nháp. Hãy xem lại quyền trước khi lưu.");
                    focus_name();
                }
            })>"Giữ bản nháp"</Button>
        </section>
    }
}

fn focus_name() {
    if let Some(element) = document()
        .get_element_by_id("profile-pen-name")
        .and_then(|element| element.dyn_into::<web_sys::HtmlElement>().ok())
    {
        let _ = element.focus();
    }
}
