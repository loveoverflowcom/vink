use super::*;

#[component]
pub(super) fn RegistrationForm(
    epoch: u64,
    now: ReadSignal<Millis>,
    focus: bool,
    switch: Callback<Mode>,
) -> impl IntoView {
    let session = Session::current();
    let email = InputValue::new();
    email.focus_on_mount(focus);
    let error = RwSignal::new(None);
    let issued = RwSignal::new(None::<(String, Millis)>);
    let state = session.action(Operation::RegistrationChallenge, now);
    let send = move |address: String| {
        let address = address.trim().to_owned();
        if address.is_empty() {
            error.set(Some(AuthError::InvalidEmail));
            email.focus();
            return;
        }
        let previous_step = issued.get_untracked();
        session.dispatch(
            epoch,
            Command::RequestRegistrationCode(address.clone()),
            move |result| {
                if issued.try_get_untracked() != Some(previous_step) {
                    return;
                }
                match result {
                    Ok(Reply::Challenge(expires)) => {
                        error.set(None);
                        issued.set(Some((address, expires)));
                    }
                    Err(e) => {
                        if e.uncertain() {
                            issued.set(None);
                        }
                        error.set(Some(e));
                    }
                    _ => {}
                }
            },
        );
    };
    move || {
        let problem = move || {
            error.get().map(|e| view! { <p role="alert" class="vk-account-error">{action_error(Operation::RegistrationChallenge, &e)}</p> })
        };
        match issued.get() {
            Some(proof) => view! {
                <GuestPassword epoch now focus=true switch mode=Mode::Register registration=proof.clone() />
                {problem}
                <div class="vk-account-actions">
                    <Button id="registration-resend" variant=ButtonVariant::Text state=state
                        on_press=move |_| send(proof.0.clone())>"Gửi lại mã"</Button>
                    <Button id="registration-back" variant=ButtonVariant::Text
                        on_press=move |_| { issued.set(None); error.set(None); email.focus_on_mount(true); }>"Dùng email khác"</Button>
                </div>
            }.into_any(),
            None => view! {
                <AuthHeading title="Tạo tài khoản" />
                <form id="registration-email" class="vk-account-form" method="post" novalidate on:submit=move |ev| {
                    ev.prevent_default(); if let Some(address) = email.read() { send(address); }
                }>
                    <TextField id="registration-address" name="email" label="Email" autocomplete="email"
                        kind=TextKind::Email required=true max_length=254 variant=FieldVariant::FloatingTonal
                        input_ref=email.node value=empty_value() on_input=email.edit() error=field_error(error, |e| *e == AuthError::InvalidEmail) />
                    {problem}
                    <div class="vk-account-actions"><Button id="registration-send" variant=ButtonVariant::Filled kind=ButtonKind::Submit state=state>"Gửi mã xác minh"</Button></div>
                    <div class="vk-auth-account-switch mt-vk-1 flex min-w-0 flex-wrap items-center justify-center gap-x-vk-1 gap-y-0 text-center text-vk-label text-vk-on-surface-variant">
                        <span>"Đã có tài khoản?"</span>
                        <Button id="use-password" variant=ButtonVariant::Text attr:class="[&>button]:min-h-[48px] [&>button]:px-vk-2" on_press=move |_| switch.run(Mode::Login)>"Đăng nhập"</Button>
                    </div>
                </form>
            }.into_any(),
        }
    }
}

#[component]
pub(super) fn EmailSettings(epoch: u64) -> impl IntoView {
    let session = Session::current();
    let value = RwSignal::new(None::<Result<String, AuthError>>);
    let read = move || {
        value.set(None);
        session.verified_email(
            epoch,
            move || value.try_get_untracked().is_some(),
            move |answer| value.set(Some(answer.map(|status| status.verified_email))),
        );
    };
    let checking = Memo::new(move |_| session.auth.with(|gate| gate.needs_check()));
    Effect::new(move |_| {
        if checking.get() {
            value.set(None);
        } else {
            read();
        }
    });
    view! {
        <div id="account-email" class="vk-account-setting-row">
            <Icon name=IconName::Mail size=vink_ui::IconSize::Small />
            <span class="vk-account-setting-label min-w-0">"Email"
                {move || match value.get() {
                    None => view! { <span role="status">"Đang tải…"</span> }.into_any(),
                    Some(Ok(email)) => view! { <span class="block wrap-anywhere">{email}</span><span class="vk-ui-visually-hidden">"Đã xác minh"</span> }.into_any(),
                    Some(Err(_)) => view! { <span role="status">"Chưa tải được email."</span>
                        <Button variant=ButtonVariant::Text on_press=move |_| read()>"Thử lại"</Button> }.into_any(),
                }}
            </span>
            {move || matches!(value.get(), Some(Ok(_))).then(|| view! { <Icon name=IconName::VerifiedUser size=vink_ui::IconSize::Small /> })}
        </div>
    }
}
