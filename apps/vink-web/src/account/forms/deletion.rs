use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DeletionStage {
    Accepted,
    Deleted,
}

fn operation_id() -> Option<String> {
    #[cfg(feature = "browser")]
    {
        let mut bytes = [0; 16];
        web_sys::window()?
            .crypto()
            .ok()?
            .get_random_values_with_u8_array(&mut bytes)
            .ok()?;
        Some(crate::endorsement_state::uuid_v4(bytes))
    }
    #[cfg(not(feature = "browser"))]
    None
}

fn focus(id: &str) {
    #[cfg(feature = "browser")]
    {
        use wasm_bindgen::JsCast;
        if let Some(node) = document()
            .get_element_by_id(id)
            .and_then(|node| node.dyn_into::<web_sys::HtmlElement>().ok())
        {
            let _ = node.focus();
        }
    }
    #[cfg(not(feature = "browser"))]
    let _ = id;
}

#[component]
pub(in crate::account) fn AccountDeletion() -> impl IntoView {
    let session = Session::current();
    let now = use_clock();
    let open = RwSignal::new(false);
    let receipt = RwSignal::<Option<String>>::new(None);
    let stage = RwSignal::<Option<DeletionStage>>::new(None);
    let error = RwSignal::<Option<AuthError>>::new(None);
    Effect::new(move |_| {
        if stage.with(Option::is_some)
            && matches!(
                session.standing.get(),
                crate::session::Standing::Active(_) | crate::session::Standing::NeedsPrincipal
            )
        {
            receipt.set(None);
            stage.set(None);
            open.set(false);
            error.set(None);
        }
    });
    let available = move || {
        !matches!(
            session.standing.get(),
            crate::session::Standing::Unknown | crate::session::Standing::Unavailable
        )
    };
    let idle = Signal::derive(move || {
        if session.auth.with(|gate| gate.active().is_some()) {
            ActionState::Disabled { reason: None }
        } else {
            ActionState::Enabled
        }
    });
    let status_action = session.action(Operation::DeletionStatus, now);
    let check = move |_| {
        let Some(id) = receipt.get_untracked() else {
            return;
        };
        session.dispatch(
            session.epoch.get_untracked(),
            Command::DeletionStatus(id),
            move |result| match result {
                Ok(Reply::Deletion(next)) => {
                    let _ = stage.try_set(Some(next));
                    let _ = error.try_set(None);
                }
                Err(AuthError::ChallengeClosed) if stage.try_get_untracked() == Some(None) => {
                    let _ = receipt.try_set(None);
                    let _ = error.try_set(Some(AuthError::ChallengeClosed));
                }
                Err(failure) => {
                    let _ = error.try_set(Some(failure));
                }
                _ => {}
            },
        );
    };
    view! {
        <div class="vk-account-setting">
            <Show when=move || available() && receipt.with(Option::is_none)>
                <Button id="open-account-deletion" variant=ButtonVariant::DangerText state=idle
                    expanded=Signal::derive(move || open.get())
                    on_press=move |_| { error.set(None); open.set(true); }>
                    "Xóa tài khoản"
                </Button>
            </Show>
            {move || {
                let epoch = session.epoch.get();
                (available() && open.get() && receipt.with(Option::is_none)).then(|| view! {
                    <DeletionForm epoch now receipt stage error
                        close=Callback::new(move |()| {
                            open.set(false);
                            error.set(None);
                            focus("open-account-deletion");
                        }) />
                })
            }}
            <Show when=move || receipt.with(Option::is_some)>
                <div class="vk-account-form">
                    <p id="account-deletion-result" role="status">{move || match stage.get() {
                        Some(DeletionStage::Deleted) => "Tài khoản đã được xóa vĩnh viễn.",
                        Some(DeletionStage::Accepted) => "Yêu cầu đã được tiếp nhận. Bạn đã đăng xuất và Vink đang dọn dữ liệu.",
                        None => "Chưa nhận được kết quả xóa tài khoản.",
                    }}</p>
                    <Show when=move || stage.get() != Some(DeletionStage::Deleted)>
                        {move || error.get().map(|failure| view! {
                            <p class="vk-account-error" role="alert">{failure.message()}</p>
                        })}
                        <Button id="check-account-deletion" variant=ButtonVariant::Tonal state=status_action
                            on_press=check>"Kiểm tra kết quả"</Button>
                    </Show>
                </div>
            </Show>
        </div>
    }
}

#[component]
fn DeletionForm(
    epoch: u64,
    now: ReadSignal<Millis>,
    receipt: RwSignal<Option<String>>,
    stage: RwSignal<Option<DeletionStage>>,
    error: RwSignal<Option<AuthError>>,
    close: Callback<()>,
) -> impl IntoView {
    let session = Session::current();
    let email_mode = RwSignal::new(false);
    let challenge = RwSignal::<Option<String>>::new(None);
    let username = InputValue::new();
    let password = InputValue::new();
    let email = InputValue::new();
    let code = InputValue::new();
    let confirm_action = session.action(Operation::ConfirmDeletion, now);
    let challenge_action = session.action(Operation::DeletionChallenge, now);
    let idle = Signal::derive(move || {
        if session.auth.with(|gate| gate.active().is_some()) {
            ActionState::Disabled { reason: None }
        } else {
            ActionState::Enabled
        }
    });
    let heading = NodeRef::<leptos::html::H2>::new();
    Effect::new(move |_| {
        if heading.get().is_some() {
            focus("account-deletion-title");
        }
    });
    let confirm = move |proof: serde_json::Value| {
        let Some(id) = operation_id() else {
            error.set(Some(AuthError::Network));
            return;
        };
        let mut proof = proof;
        proof["operation_id"] = serde_json::Value::String(id.clone());
        session.dispatch(
            epoch,
            Command::ConfirmDeletion(proof),
            move |result| match result {
                Ok(Reply::Deletion(next)) => {
                    let _ = stage.try_set(Some(next));
                    let _ = error.try_set(None);
                    let _ = receipt.try_set(Some(id));
                }
                Err(failure) if failure.uncertain() => {
                    let _ = error.try_set(None);
                    let _ = receipt.try_set(Some(id));
                }
                Err(failure) => {
                    let _ = error.try_set(Some(failure));
                }
                _ => {}
            },
        );
    };
    let send = move || {
        let Some(value) = email.read() else { return };
        if !value.contains('@') {
            error.set(Some(AuthError::InvalidEmail));
            email.focus();
            return;
        }
        session.dispatch(
            epoch,
            Command::StartDeletion(value.trim().into()),
            move |result| match result {
                Ok(Reply::DeletionChallenge(id)) => {
                    let _ = error.try_set(None);
                    let _ = challenge.try_set(Some(id));
                }
                Err(failure) => {
                    let _ = error.try_set(Some(failure));
                }
                _ => {}
            },
        );
    };
    view! {
        <section id="delete-account-form" class="vk-account-form" aria-labelledby="account-deletion-title">
            <h2 id="account-deletion-title" node_ref=heading tabindex="-1">"Xóa vĩnh viễn tài khoản"</h2>
            <p>"Tác phẩm và bản nháp của bạn sẽ bị xóa. Bạn sẽ đăng xuất trên mọi thiết bị. Không thể khôi phục tài khoản."</p>
            <div class="vk-account-actions">
                <Button id="account-delete-with-password" variant=ButtonVariant::Text state=idle
                    on_press=move |_| { error.set(None); email_mode.set(false); }>"Dùng mật khẩu"</Button>
                <Button id="account-delete-with-email" variant=ButtonVariant::Text state=idle
                    on_press=move |_| { error.set(None); email_mode.set(true); }>"Dùng mã email"</Button>
            </div>
            {move || if !email_mode.get() {
                view! {
                    <form class="vk-account-form" method="post" on:submit=move |event| {
                        event.prevent_default();
                        if let (Some(username), Some(value)) = (username.read(), password.read()) {
                            password.clear();
                            confirm(serde_json::json!({ "method": "password", "username": username, "password": value }));
                        }
                    }>
                        <PasswordFields purpose=PasswordUse::Deletion username password error />
                        <Button id="confirm-account-deletion-password" variant=ButtonVariant::Danger kind=ButtonKind::Submit
                            state=confirm_action>"Xóa vĩnh viễn"</Button>
                    </form>
                }.into_any()
            } else if let Some(challenge_id) = challenge.get() {
                code.focus_on_mount(true);
                view! {
                    <form class="vk-account-form" method="post" on:submit=move |event| {
                        event.prevent_default();
                        if let Some(value) = code_value(code, error) {
                            confirm(serde_json::json!({ "method": "email_code", "challenge_id": challenge_id, "code": value }));
                        }
                    }>
                        <p role="status">"Nếu email đã được xác minh trên tài khoản, Vink sẽ gửi mã đến địa chỉ đó."</p>
                        <TextField id="account-delete-code" name="code" label="Mã 6 chữ số" required=true
                            autocomplete="one-time-code" variant=FieldVariant::FloatingTonal
                            input_ref=code.node value=empty_value() on_input=code.edit()
                            error=field_error(error, |e| matches!(e, AuthError::CodeRejected | AuthError::InvalidCode)) />
                        <div class="vk-account-actions">
                            <Button id="confirm-account-deletion-code" variant=ButtonVariant::Danger kind=ButtonKind::Submit
                                state=confirm_action>"Xóa vĩnh viễn"</Button>
                            <Button variant=ButtonVariant::Text state=idle on_press=move |_| {
                                code.clear(); error.set(None); challenge.set(None);
                            }>"Xin mã mới"</Button>
                        </div>
                    </form>
                }.into_any()
            } else {
                view! {
                    <form class="vk-account-form" method="post" on:submit=move |event| { event.prevent_default(); send(); }>
                        <TextField id="account-delete-email" name="email" label="Email đã xác minh"
                            kind=TextKind::Email required=true autocomplete="email" max_length=254
                            variant=FieldVariant::FloatingTonal input_ref=email.node value=empty_value()
                            on_input=email.edit() error=field_error(error, |e| *e == AuthError::InvalidEmail) />
                        <Button id="send-account-deletion-code" variant=ButtonVariant::Tonal kind=ButtonKind::Submit
                            state=challenge_action>"Gửi mã xác minh"</Button>
                    </form>
                }.into_any()
            }}
            {move || error.get().map(|failure| view! {
                <p class="vk-account-error" role="alert">{action_error(Operation::ConfirmDeletion, &failure)}</p>
            })}
            <Button variant=ButtonVariant::Text state=idle on_press=move |_| close.run(())>"Hủy"</Button>
        </section>
    }
}
