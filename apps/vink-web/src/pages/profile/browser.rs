mod driver;
mod handle;

use super::{
    inline::{Editor, Field, Policy, Status},
    privacy::PrivacySelect,
};
use crate::{
    account::Session,
    api,
    contract::profile::{OwnProfile, Visibility},
};
pub(crate) use driver::provide_draft;
use driver::{Driver, Form};
use leptos::prelude::*;
use vink_ui::{
    Button, ButtonVariant, Dialog, DismissPolicy, FieldVariant, Heading, HeadingLevel, TextArea,
    TextField, TextInput, TypeRole,
};
use wasm_bindgen::JsCast;

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

#[component]
fn ProfileForm(scope: StoredValue<String>, initial: OwnProfile) -> impl IntoView {
    let session = Session::current();
    let handle = handle::Controller::new(session, scope, initial.principal_id.clone());
    let d = Driver::mount(session, scope.get_value(), initial);
    let form = Form(StoredValue::new(d.clone()));
    handle.refresh();
    super::observe_changes(move || {
        handle.refresh();
        d.refresh();
    });
    let privacy = NodeRef::<leptos::html::Div>::new();
    Effect::new(move |_| {
        if let Some(region) = privacy.get()
            && window()
                .location()
                .hash()
                .is_ok_and(|h| h == "#quyen-rieng-tu")
            && document().active_element().is_none_or(|a| {
                a.tag_name() == "BODY"
                    || matches!(a.id().as_str(), "noi-dung" | "edit-profile-heading")
            })
        {
            region.scroll_into_view();
            let _ = region.focus();
        }
    });
    let navigating = StoredValue::new(false);
    let down = window_event_listener(leptos::ev::mousedown, move |ev| {
        navigating.set_value(
            ev.target()
                .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
                .and_then(|el| el.closest("a[href]").ok().flatten())
                .is_some(),
        );
    });
    let up = window_event_listener(leptos::ev::mouseup, move |_| navigating.set_value(false));
    let unload = window_event_listener(leptos::ev::beforeunload, move |ev| {
        if form.read(Editor::unsaved) {
            ev.prevent_default();
        }
    });
    on_cleanup(move || {
        down.remove();
        up.remove();
        unload.remove();
    });
    let guard = crate::navigation::use_navigation_guard();
    let destination = RwSignal::new(None::<String>);
    let skip_blur =
        Signal::derive(move || navigating.get_value() || destination.with(Option::is_some));
    let source_link = StoredValue::new_local(None::<web_sys::HtmlAnchorElement>);
    let released = StoredValue::new(false);
    let registration = guard.register(move |to| {
        if released.get_value() || !form.read(Editor::unsaved) {
            return false;
        }
        let link = document()
            .active_element()
            .and_then(|el| el.closest("a[href]").ok().flatten())
            .and_then(|el| el.dyn_into::<web_sys::HtmlAnchorElement>().ok())
            .filter(|a| link_destination(a) == to);
        source_link.set_value(link);
        destination.set(Some(to.to_string()));
        true
    });
    let leave = move |_| {
        let Some(to) = destination.get_untracked() else {
            return;
        };
        let link = source_link.get_value();
        navigating.set_value(true);
        destination.set(None);
        // Let Dialog restore focus before the link's own return/focus handler runs.
        request_animation_frame(move || {
            if released.try_set_value(true).is_some() {
                return;
            }
            if let Some(a) = link.filter(|a| a.is_connected() && link_destination(a) == to) {
                a.click();
            } else {
                guard.go(&to);
            }
            released.try_set_value(false);
            navigating.try_set_value(false);
        });
    };
    on_cleanup(move || guard.unregister(registration));
    let announcement = Memo::new(move |_| {
        form.read(|e| {
            [Field::Name, Field::Bio]
                .into_iter()
                .chain(Policy::ALL.map(Field::Policy))
                .filter_map(|field| {
                    let msg = feedback(e.status(field), field);
                    (!msg.is_empty()).then(|| format!("{}. {}", field.label(), msg))
                })
                .collect::<Vec<_>>()
                .join(" ")
        })
    });
    view! {
        <div class="vk-profile-form grid min-w-0 gap-vk-4">
            <div class="flex min-w-0 items-start gap-vk-3">
                <div class="shrink-0"><crate::appearance::AccountAvatar /></div>
                <div class="min-w-0 flex-1"><InlineText form field=Field::Name skip_blur /></div>
            </div>
            <handle::HandleRow controller=handle />
            <InlineText form field=Field::Bio skip_blur />
            <div id="quyen-rieng-tu" node_ref=privacy tabindex="-1" role="group"
                aria-labelledby="profile-privacy-heading"
                class="grid min-w-0 gap-vk-2 rounded-vk-lg focus-visible:outline-2 focus-visible:[outline-color:var(--vk-focus-ring)]">
                <Heading id="profile-privacy-heading" level=HeadingLevel::H2 role=TypeRole::Title
                    attr:class="m-0">"Quyền riêng tư"</Heading>
                {Policy::ALL.into_iter().map(|policy|view!{<PolicyField form policy />}).collect_view()}
                <Show when=move ||form.read(|e|Policy::ALL.iter().any(|p|*p.access(&mut e.acked.policies.clone())==Visibility::Owner))>
                    <p id="profile-privacy-lock-note" class="m-0 text-vk-caption text-vk-on-surface-variant">
                        "Trường đã đặt Chỉ mình tôi hiện chưa thể công khai lại."
                    </p>
                </Show>
            </div>
            <p id="profile-message" role="status" aria-atomic="true" class="sr-only">{announcement}</p>
        </div>
        <handle::HandleDialog controller=handle />
        <Dialog id="profile-leave" open=Signal::derive(move ||destination.with(Option::is_some))
            title=String::from("Rời Chỉnh hồ sơ?") dismiss=DismissPolicy::EscapeOnly
            on_dismiss=move |_|destination.set(None)
            actions=move ||view!{
                <Button id="profile-stay" variant=ButtonVariant::Text on_press=move |_|destination.set(None)>"Ở lại"</Button>
                <Button id="profile-leave-keep" on_press=leave>"Giữ bản nháp và rời trang"</Button>
            }>
            <p>"Bạn có thay đổi chưa được xác nhận. Bản nháp được giữ trong phiên này."</p>
        </Dialog>
    }
}

fn link_destination(link: &web_sys::HtmlAnchorElement) -> String {
    format!("{}{}{}", link.pathname(), link.search(), link.hash())
}

#[component]
fn InlineText(form: Form, field: Field, skip_blur: Signal<bool>) -> impl IntoView {
    let composing = StoredValue::new(false);
    let pending_blur = StoredValue::new(false);
    let value = Memo::new(move |_| form.read(|e| e.text(field)));
    let error = Signal::derive(move || {
        form.read(|e| {
            e.invalid(field)
                .then(|| feedback(Status::Invalid, field).to_owned())
        })
    });
    let typed = Callback::new(move |input: TextInput| {
        composing.set_value(input.composing);
        form.change(|e| e.input(field, input.value));
        if !input.composing && pending_blur.get_value() {
            // Some IMEs deliver the final input after compositionend in this frame.
            request_animation_frame(move || {
                if pending_blur.try_get_value() == Some(true) && !composing.get_value() {
                    pending_blur.set_value(false);
                    if !skip_blur.get_untracked() {
                        form.commit(field);
                    }
                }
            });
        }
    });
    let is_field = move |target: Option<web_sys::EventTarget>| {
        target
            .and_then(|node| node.dyn_into::<web_sys::Element>().ok())
            .is_some_and(|node| node.id() == field.id())
    };
    let blur = move |event: web_sys::FocusEvent| {
        if !is_field(event.target()) || skip_blur.get_untracked() {
            return;
        }
        if composing.get_value() {
            pending_blur.set_value(true);
        } else {
            form.commit(field);
        }
    };
    let keydown = move |event: web_sys::KeyboardEvent| {
        if !is_field(event.target())
            || composing.get_value()
            || event.is_composing()
            || event.key_code() == 229
            || event.default_prevented()
        {
            return;
        }
        match event.key().as_str() {
            "Escape" => {
                event.prevent_default();
                form.change(|e| e.escape(field));
            }
            "Enter" if field == Field::Name || event.ctrl_key() || event.meta_key() => {
                event.prevent_default();
                form.commit(field);
            }
            _ => {}
        }
    };
    view! {
        <div class="grid min-w-0 gap-vk-1"
            on:compositionstart=move |_| composing.set_value(true)
            on:focusin=move |_| pending_blur.set_value(false) on:focusout=blur on:keydown=keydown>
            {if field==Field::Name {
                view!{<TextField id=field.id() label=field.label() value on_input=typed error required=true
                    variant=FieldVariant::FloatingTonal help="Luôn công khai, có thể trùng tên." />}.into_any()
            } else {
                view!{<TextArea id=field.id() label=field.label() value on_input=typed error rows=3 grow=true
                    variant=FieldVariant::FloatingTonal />}.into_any()
            }}
            <Feedback form field />
        </div>
    }
}

fn feedback(status: Status, field: Field) -> &'static str {
    match status {
        Status::Idle => "",
        Status::Queued => "Chờ lưu…",
        Status::Saving => "Đang lưu…",
        Status::Checking => "Đang đối chiếu…",
        Status::CheckFailed => "Chưa xác nhận được kết quả lưu.",
        Status::Retry => "Chưa lưu được.",
        Status::Saved => "Đã lưu.",
        Status::Observed => "Đã thấy giá trị này trên Vink.",
        Status::Conflict => "Trường này đã thay đổi ở nơi khác.",
        Status::Invalid if field == Field::Name => "Nhập bút danh hợp lệ, tối đa 80 ký tự.",
        Status::Invalid => "Nhập Giới thiệu hợp lệ, tối đa 500 ký tự.",
    }
}

#[component]
fn Feedback(form: Form, field: Field) -> impl IntoView {
    let status = Memo::new(move |_| form.read(|e| e.status(field)));
    let refocus = move || super::super::focus_recovery(field.id());
    let retry_text = move || {
        if status.get() == Status::CheckFailed {
            "Đối chiếu lại"
        } else {
            "Thử lại"
        }
    };
    view! {
        <Show when=move ||!matches!(status.get(),Status::Idle|Status::Invalid)>
            <div id=format!("{}-feedback",field.id()) class="grid min-w-0 gap-vk-1 text-vk-caption text-vk-on-surface-variant wrap-anywhere">
                <span>{move ||feedback(status.get(),field)}</span>
                <Show when=move ||matches!(status.get(),Status::Retry|Status::CheckFailed)>
                    <Button variant=ButtonVariant::Text accessible_label=Signal::derive(move ||format!("{}: {}",retry_text(),field.label()))
                        on_press=move |_|{form.retry(field);refocus();}>
                        {retry_text}
                    </Button>
                </Show>
                <Show when=move ||status.get()==Status::Conflict>
                    <div class="whitespace-pre-wrap">{move ||format!("Đang lưu trên Vink: {}",form.read(|e|e.stored_text(field)))}</div>
                    <div class="flex flex-wrap gap-vk-2">
                        <Button variant=ButtonVariant::Text accessible_label=format!("Dùng bản trên Vink: {}",field.label()) on_press=move |_|{form.change(|e|e.resolve(field,false));refocus();}>"Dùng bản trên Vink"</Button>
                        <Button variant=ButtonVariant::Tonal accessible_label=format!("Giữ thay đổi của tôi: {}",field.label()) on_press=move |_|{form.retry(field);refocus();}>"Giữ thay đổi của tôi"</Button>
                    </div>
                </Show>
            </div>
        </Show>
    }
}

#[component]
fn PolicyField(form: Form, policy: Policy) -> impl IntoView {
    let field = Field::Policy(policy);
    let confirming = RwSignal::new(false);
    let locked = Memo::new(move |_| {
        form.read(|e| *policy.access(&mut e.acked.policies.clone()) == Visibility::Owner)
    });
    let outstanding = Memo::new(move |_| {
        form.read(|e| {
            !matches!(
                e.status(field),
                Status::Idle | Status::Saved | Status::Observed
            )
        })
    });
    let value = Signal::derive(move || {
        if locked.get() || confirming.get() || outstanding.get() {
            Visibility::Owner
        } else {
            Visibility::Everyone
        }
    });
    let cancel = move || {
        confirming.set(false);
        super::super::focus_recovery(field.id());
    };
    let cancel_id = format!("{}-cancel", field.id());
    let focus_id = cancel_id.clone();
    view! {
        <div class="grid min-w-0 gap-vk-1" on:keydown=move |ev|{
            if ev.key()=="Escape" && confirming.get_untracked(){ev.prevent_default();cancel();}
        }>
            <PrivacySelect id=field.id() label=field.label() value locked disabled=Signal::derive(move ||outstanding.get()||confirming.get())
                on_change=move |v|{
                    if v==Visibility::Owner && !locked.get_untracked(){
                        confirming.set(true);super::super::focus_recovery(focus_id.clone());
                    }
                } lock_description_id="profile-privacy-lock-note" />
            <Show when=move ||confirming.get() && !locked.get()>
                <div class="grid min-w-0 gap-vk-2 rounded-vk-md bg-vk-warning-container p-vk-3 text-vk-on-warning-container">
                    <p id=format!("{}-consequence",field.id()) class="m-0 wrap-anywhere">{format!("Chuyển {} sang Chỉ mình tôi? Sau khi lưu, bạn chưa thể công khai lại trường này.",field.label())}</p>
                    <div class="flex flex-wrap gap-vk-2">
                        <Button id=cancel_id.clone() described_by=format!("{}-consequence",field.id()) variant=ButtonVariant::Text on_press=move |_|cancel()>"Hủy"</Button>
                        <Button id=format!("{}-confirm",field.id()) described_by=format!("{}-consequence",field.id()) variant=ButtonVariant::Tonal on_press=move |_|{
                            confirming.set(false);form.change(|e|e.narrow(policy));form.pump();
                            super::super::focus_recovery("quyen-rieng-tu");
                        }>"Chuyển sang Chỉ mình tôi"</Button>
                    </div>
                </div>
            </Show>
            <Feedback form field />
        </div>
    }
}
