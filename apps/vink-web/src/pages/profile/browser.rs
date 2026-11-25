mod handle;

use leptos::{prelude::*, task::spawn_local};
use unicode_normalization::UnicodeNormalization;
use vink_ui::{
    ActionState, Button, ButtonKind, ButtonVariant, ChoiceOption, Icon, IconName, Link, Select,
    TextArea, TextField, TextInput,
};
use wasm_bindgen::JsCast;

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
        <Suspense fallback=|| view! { <p role="status">"Đang tải hồ sơ của bạn…"</p> }>
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
                        Some("bio") => ("profile-bio", "Nhập Bio hợp lệ, tối đa 500 ký tự."),
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
    // Shared fields keep a stable native control; the in-flight lock is applied to it here so
    // the text that is being saved cannot change under the request.
    Effect::new(move |_| {
        let readonly = blocked();
        for id in ["profile-pen-name", "profile-bio"] {
            if let Some(element) = document().get_element_by_id(id) {
                let _ = element.toggle_attribute_with_force("readonly", readonly);
            }
        }
    });
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
        <form class="vk-profile-form grid [min-inline-size:0] gap-vk-6 [margin-block-start:var(--vk-space-6)]" autocomplete="off" novalidate
            on:compositionstart=move |_| form.composing.set(true)
            on:compositionend=move |_| form.composing.set(false)
            on:submit=move |event| { event.prevent_default(); form.save(session); }>
            <TextField id="profile-pen-name" label="Bút danh" value=form.name
                on_input=typed(form.name) error=error_for("profile-pen-name")
                help="Luôn công khai. Bút danh có thể trùng với người khác." />
            <handle::HandleRow controller=handle />
            <div id="quyen-rieng-tu" node_ref=privacy tabindex="-1" role="group" aria-label="Quyền riêng tư"
                class="grid min-w-0 gap-vk-6">
                <PolicyField form label="Quyền xem ảnh đại diện" id="profile-avatar-policy" access=|p| &mut p.avatar />
                <div class="vk-profile-field grid [min-inline-size:0] gap-vk-3">
                    <TextArea id="profile-bio" label="Bio" value=form.bio rows=4
                        on_input=typed(form.bio) error=error_for("profile-bio") />
                    <PolicyField form label="Quyền xem Bio" id="profile-bio-policy" access=|p| &mut p.bio />
                </div>
                <fieldset class="grid [min-inline-size:0] gap-vk-1 m-0 p-0 [border:0]">
                    <legend>"Quyền xem quan hệ"</legend>
                    <PolicyField form label="Danh sách người theo dõi" id="profile-followers-list" access=|p| &mut p.followers_list />
                    <PolicyField form label="Số người theo dõi" id="profile-followers-count" access=|p| &mut p.followers_count />
                    <PolicyField form label="Danh sách đang theo dõi" id="profile-following-list" access=|p| &mut p.following_list />
                    <PolicyField form label="Số người đang theo dõi" id="profile-following-count" access=|p| &mut p.following_count />
                </fieldset>
            </div>
            <p class="vk-muted vk-profile-note">"Trường đã đặt Chỉ mình tôi hiện chưa thể công khai lại."</p>
            {move || form.conflict.get().map(|current| view! {
                <Conflict form current />
            })}
            <div class="vk-profile-actions grid gap-vk-2">
                <p id="profile-message" role="status" aria-atomic="true">{move || form.message.get()}</p>
                <div class="flex flex-wrap items-center gap-vk-3">
                    <Button id="profile-save" kind=ButtonKind::Submit state=save_state
                        described_by="profile-message">"Lưu hồ sơ"</Button>
                    <Show when=move || form.reconcile.with(Option::is_some) && !form.busy.get()>
                        <Button variant=ButtonVariant::Tonal
                            on_press=Callback::new(move |()| form.read_back(session))>"Đối chiếu lại"</Button>
                    </Show>
                    <Link attr:id="cancel-profile-edit" attr:data-profile-return="true" href="/toi#edit-profile-link">"Về Tôi"</Link>
                </div>
            </div>
        </form>
        <handle::HandleDialog controller=handle />
    }
}

type PolicyAccess = fn(&mut Policies) -> &mut Visibility;

fn policy_text(policy: Visibility) -> &'static str {
    match policy {
        Visibility::Everyone => "Công khai",
        Visibility::Owner => "Chỉ mình tôi",
    }
}

#[component]
fn PolicyField(
    form: Form,
    label: &'static str,
    id: &'static str,
    access: PolicyAccess,
) -> impl IntoView {
    // While SOC-OPEN-01 is open a stored "Chỉ mình tôi" cannot be widened again.
    let stored_private =
        Memo::new(move |_| *access(&mut form.base.get().policies) == Visibility::Owner);
    let value = Signal::derive(move || {
        Some(
            match *access(&mut form.policies.get()) {
                Visibility::Everyone => "everyone",
                Visibility::Owner => "self",
            }
            .to_string(),
        )
    });
    let disabled = Signal::derive(move || form.busy.get() || form.reconcile.with(Option::is_some));
    let on_change = Callback::new(move |picked: Option<String>| {
        let visibility = match picked.as_deref() {
            Some("everyone") => Visibility::Everyone,
            Some("self") => Visibility::Owner,
            _ => return,
        };
        form.policies
            .update(|policies| *access(policies) = visibility);
    });
    move || {
        // AC-02 (#336): a locked value is a fact, not a control with a dead option.
        if stored_private.get() {
            let label_id = format!("{id}-label");
            let group_label = label_id.clone();
            return view! {
                <div class="vk-profile-policy vk-profile-policy--locked" role="group" aria-labelledby=group_label>
                    <div class="vk-ui-field">
                        <span class="vk-ui-field__label" id=label_id>{label}</span>
                        <p class="vk-profile-policy__value" id=id>
                            <Icon name=IconName::Lock />
                            <span>{policy_text(Visibility::Owner)}</span>
                        </p>
                    </div>
                </div>
            }
            .into_any();
        }
        let options = vec![
            ChoiceOption::new("everyone", "Công khai"),
            ChoiceOption::new("self", "Chỉ mình tôi"),
        ];
        view! { <div class="vk-profile-policy"><Select id label options value on_change disabled /></div> }
            .into_any()
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
                <dt>"Bio"</dt><dd>{current.bio.unwrap_or_else(|| "Chưa điền".into())}</dd>
                <dt>"Ảnh đại diện"</dt><dd>{policy_text(p.avatar)}</dd>
                <dt>"Bio hiển thị"</dt><dd>{policy_text(p.bio)}</dd>
                <dt>"Danh sách người theo dõi"</dt><dd>{policy_text(p.followers_list)}</dd>
                <dt>"Số người theo dõi"</dt><dd>{policy_text(p.followers_count)}</dd>
                <dt>"Danh sách đang theo dõi"</dt><dd>{policy_text(p.following_list)}</dd>
                <dt>"Số người đang theo dõi"</dt><dd>{policy_text(p.following_count)}</dd>
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
                        for field in [
                            (|p: &mut Policies| &mut p.avatar) as PolicyAccess,
                            |p| &mut p.bio, |p| &mut p.followers_list, |p| &mut p.followers_count,
                            |p| &mut p.following_list, |p| &mut p.following_count,
                        ] {
                            if *field(&mut current.policies.clone()) == Visibility::Owner {
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
