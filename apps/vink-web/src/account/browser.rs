use std::cell::RefCell;

use leptos::{prelude::*, task::spawn_local};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};

use super::{Command, Reply, Session, flow::Operation};
use crate::api;
use crate::contract::SessionStateKind as Kind;
use crate::session::{AuthError, Millis, Observation, SessionOwner, Standing};

const CHANNEL: &str = "vink-session";

struct Broadcast {
    channel: web_sys::BroadcastChannel,
    _heard: Closure<dyn FnMut(web_sys::MessageEvent)>,
}

impl Drop for Broadcast {
    fn drop(&mut self) {
        self.channel.set_onmessage(None);
        self.channel.close();
    }
}

thread_local! {
    // The sender is also the receiver; BroadcastChannel excludes the source object, not all
    // objects in the source tab. Creating a second channel would echo into this tab.
    static BROADCAST: RefCell<Option<Broadcast>> = const { RefCell::new(None) };
}

pub(super) fn now() -> Millis {
    js_sys::Date::now() as Millis
}

pub(super) fn watch(session: Session) {
    let focus = window_event_listener(leptos::ev::focus, move |_| refresh(session));
    on_cleanup(move || focus.remove());
    api::on_unauthenticated(move || refresh(session));
    if let Ok(channel) = web_sys::BroadcastChannel::new(CHANNEL) {
        let heard = Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |_| {
            session.auth.update(|gate| gate.invalidate(true));
            session.owner.update(SessionOwner::expect_session_change);
            refresh(session);
        });
        channel.set_onmessage(Some(heard.as_ref().unchecked_ref()));
        BROADCAST.with_borrow_mut(|value| {
            *value = Some(Broadcast {
                channel,
                _heard: heard,
            })
        });
        on_cleanup(|| {
            BROADCAST.with_borrow_mut(|value| {
                value.take();
            })
        });
    }
    Effect::new(move |_| refresh(session));
}

fn announce() {
    BROADCAST.with_borrow(|broadcast| {
        if let Some(broadcast) = broadcast {
            let _ = broadcast
                .channel
                .post_message(&JsValue::from_str("changed"));
        }
    });
}

async fn observe() -> Observation {
    // The cookie can change between the two reads. A profile from another principal is
    // never combined with the first answer. A second read can establish the new standing.
    for _ in 0..2 {
        let Ok(state) = api::session_state().await else {
            return Observation::Unavailable;
        };
        match (state.state, state.principal_id) {
            (Kind::Anonymous, _) => return Observation::Anonymous,
            (Kind::NeedsPrincipal, _) => return Observation::NeedsPrincipal,
            (Kind::Suspended, _) => return Observation::Suspended,
            (Kind::Active, Some(id)) => match api::current_account().await {
                Ok(account) if account.principal_id == id => return Observation::Active(account),
                Ok(_) | Err(AuthError::Unauthenticated) => {}
                Err(_) => return Observation::Unavailable,
            },
            (Kind::Active, None) => return Observation::Unavailable,
        }
    }
    Observation::Unavailable
}

pub(super) fn refresh(session: Session) {
    let Some(ticket) = session.owner.try_update(SessionOwner::begin_check) else {
        return;
    };
    session.checking.set(true);
    spawn_local(async move {
        let observation = observe().await;
        let confirmed = observation != Observation::Unavailable;
        let changed = session
            .owner
            .try_update(|owner| {
                if !owner.is_latest(ticket) {
                    return None;
                }
                let before = owner.standing().clone();
                owner.settle(ticket, observation);
                Some(before != *owner.standing())
            })
            .flatten();
        if let Some(changed) = changed {
            session.auth.update(|gate| {
                if changed {
                    gate.invalidate(false);
                }
                if confirmed {
                    gate.checked();
                }
            });
            session.checking.set(false);
        }
    });
}

fn challenge_expiry(response: crate::contract::ChallengeSent) -> Result<Reply, AuthError> {
    let at = js_sys::Date::parse(&response.expires_at);
    if at.is_finite() {
        Ok(Reply::Challenge(at as Millis))
    } else {
        Err(AuthError::Unexpected { status: 202 })
    }
}

fn accepted_session(state: crate::contract::SessionState) -> Result<Reply, AuthError> {
    match (state.state, state.principal_id) {
        (Kind::Active, Some(_)) | (Kind::NeedsPrincipal, None) => Ok(Reply::Done),
        _ => Err(AuthError::Unexpected { status: 200 }),
    }
}

async fn send(command: Command) -> Result<Reply, AuthError> {
    match command {
        Command::Login(input) => accepted_session(api::login_password(input).await?),
        Command::Register(input) => accepted_session(api::register_password(input).await?),
        Command::RequestEmailCode(email) => {
            challenge_expiry(api::request_sign_in_code(email).await?)
        }
        Command::EmailCode(code) => accepted_session(api::complete_sign_in(code).await?),
        Command::RequestEnrollmentCode => challenge_expiry(api::request_enrollment_code().await?),
        Command::Enroll(proof) => {
            if api::enroll_password(proof).await?.enrolled {
                Ok(Reply::Done)
            } else {
                Err(AuthError::Unexpected { status: 200 })
            }
        }
        Command::PenName(name) => api::provision_principal(name).await.map(|_| Reply::Done),
        Command::SignOut => api::sign_out().await.map(|()| Reply::Done),
    }
}

pub(super) fn dispatch(
    session: Session,
    epoch: u64,
    command: Command,
    complete: impl FnOnce(Result<Reply, AuthError>) + 'static,
) {
    let operation = command.operation();
    let allowed = session.standing.with_untracked(|standing| match operation {
        Operation::PasswordLogin
        | Operation::PasswordRegister
        | Operation::EmailChallenge
        | Operation::EmailCode => matches!(standing, Standing::Anonymous),
        Operation::Enrollment | Operation::EnrollmentChallenge => {
            matches!(standing, Standing::Active(_))
        }
        Operation::PenName => matches!(standing, Standing::NeedsPrincipal),
        Operation::SignOut => matches!(
            standing,
            Standing::NeedsPrincipal | Standing::Active(_) | Standing::Suspended
        ),
    });
    if !allowed {
        return;
    }
    let Some(ticket) = session
        .auth
        .try_update(|gate| gate.begin(operation, epoch, now()))
        .flatten()
    else {
        return;
    };
    spawn_local(async move {
        let outcome = send(command).await;
        let error = outcome.as_ref().err().cloned();
        let reread = operation.reread_session(error.as_ref());
        let changed_cookie =
            operation.changes_session() && error.as_ref().is_none_or(AuthError::uncertain);
        let revoked = operation == Operation::SignOut && outcome.is_ok();
        let owns = session.auth.with_untracked(|gate| gate.owns(ticket));
        session.auth.update(|gate| {
            if reread && (operation != Operation::SignOut || revoked) {
                gate.require_check();
            }
            gate.finish(ticket, error.as_ref(), now());
        });
        if owns {
            complete(outcome);
        }
        if revoked && owns {
            // An exact 204 confirms revocation. Still read the actual cookie afterwards:
            // another tab may also have a response on its way.
            session.owner.update(|owner| {
                let ticket = owner.begin_check();
                owner.settle(ticket, Observation::Anonymous);
            });
            session.auth.update(|gate| gate.invalidate(false));
        }
        if changed_cookie {
            if operation != Operation::SignOut || revoked {
                session.owner.update(SessionOwner::expect_session_change);
            }
            announce();
        }
        if reread {
            refresh(session);
        }
    });
}

pub(super) fn enrollment_status(
    session: Session,
    epoch: u64,
    live: impl Fn() -> bool + 'static,
    complete: impl FnOnce(Result<bool, AuthError>) + 'static,
) {
    let owns = move || {
        session
            .auth
            .with_untracked(|gate| gate.epoch() == epoch && !gate.needs_check())
    };
    if !owns()
        || !session
            .standing
            .with_untracked(|s| matches!(s, Standing::Active(_)))
    {
        return;
    }
    spawn_local(async move {
        let outcome = api::password_enrollment_status()
            .await
            .map(|status| status.enrolled);
        if !owns() || !live() {
            return;
        }
        let reread = matches!(
            outcome,
            Err(AuthError::Unauthenticated | AuthError::Suspended | AuthError::AccountDisabled)
        );
        complete(outcome);
        if reread {
            session.auth.update(|gate| gate.invalidate(true));
            refresh(session);
        }
    });
}
