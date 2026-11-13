//! Auth writes use one lane per tab, even when a form goes away. UI ownership and the
//! request's ability to set a cookie are separate: invalidating a form never releases the lane.

use crate::session::{AuthError, Millis, RESEND_COOLDOWN_MS};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Operation {
    PasswordLogin,
    PasswordRegister,
    EmailChallenge,
    EmailCode,
    EnrollmentChallenge,
    Enrollment,
    PenName,
    SignOut,
}

#[cfg_attr(not(feature = "hydrate"), allow(dead_code))]
impl Operation {
    pub(super) fn status(self) -> &'static str {
        match self {
            Self::PasswordLogin => "Đang đăng nhập…",
            Self::PasswordRegister => "Đang tạo tài khoản…",
            Self::EmailChallenge | Self::EnrollmentChallenge => "Đang xin mã…",
            Self::EmailCode => "Đang kiểm tra mã…",
            Self::Enrollment => "Đang thêm mật khẩu…",
            Self::PenName => "Đang lưu bút danh…",
            Self::SignOut => "Đang đăng xuất…",
        }
    }

    pub(super) fn unavailable(self) -> &'static str {
        match self {
            Self::PasswordLogin | Self::PasswordRegister => {
                "Vink chưa mở đăng nhập và đăng ký bằng mật khẩu trên hệ thống này."
            }
            Self::EmailChallenge | Self::EmailCode => {
                "Vink chưa mở đăng nhập bằng mã email trên hệ thống này."
            }
            Self::EnrollmentChallenge | Self::Enrollment => {
                "Vink chưa hỗ trợ thêm mật khẩu trên hệ thống này."
            }
            Self::PenName | Self::SignOut => "Thao tác này hiện chưa khả dụng.",
        }
    }

    /// A success or uncertain answer may already have changed the session (or its binding).
    pub(super) fn changes_session(self) -> bool {
        matches!(
            self,
            Self::PasswordLogin
                | Self::PasswordRegister
                | Self::EmailCode
                | Self::PenName
                | Self::SignOut
        )
    }

    pub(super) fn reread_session(self, error: Option<&AuthError>) -> bool {
        self.changes_session() && error.is_none_or(AuthError::uncertain)
            || matches!(
                error,
                Some(AuthError::AlreadySignedIn | AuthError::Unauthenticated)
            )
            || (!matches!(
                self,
                Self::PasswordLogin
                    | Self::PasswordRegister
                    | Self::EmailCode
                    | Self::EmailChallenge
            ) && matches!(
                error,
                Some(AuthError::Suspended | AuthError::AccountDisabled)
            ))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Ticket {
    serial: u64,
    epoch: u64,
    operation: Operation,
}

#[cfg_attr(not(feature = "hydrate"), allow(dead_code))]
#[derive(Default)]
pub(super) struct AuthGate {
    epoch: u64,
    serial: u64,
    active: Option<Ticket>,
    // Retry-After is indexed by App endpoint. The local resend hint belongs to the
    // current flow, so a different account doesn't inherit it; server cooldowns do.
    retry_until: [Millis; Operation::SignOut as usize + 1],
    resend_until: [Millis; 2],
    needs_check: bool,
}

#[cfg_attr(not(feature = "hydrate"), allow(dead_code))]
impl AuthGate {
    pub(super) fn epoch(&self) -> u64 {
        self.epoch
    }

    pub(super) fn active(&self) -> Option<Operation> {
        self.active.map(|ticket| ticket.operation)
    }

    pub(super) fn needs_check(&self) -> bool {
        self.needs_check
    }

    pub(super) fn invalidate(&mut self, needs_check: bool) {
        self.epoch += 1;
        self.resend_until = [0; 2];
        self.needs_check |= needs_check;
    }

    pub(super) fn require_check(&mut self) {
        self.needs_check = true;
    }

    pub(super) fn checked(&mut self) {
        self.needs_check = false;
    }

    pub(super) fn wait(&self, op: Operation, now: Millis) -> Option<u32> {
        let hint = match op {
            Operation::EmailChallenge => self.resend_until[0],
            Operation::EnrollmentChallenge => self.resend_until[1],
            _ => 0,
        };
        let remaining = self.retry_until[op as usize].max(hint) - now;
        (remaining > 0).then(|| u32::try_from((remaining + 999) / 1_000).unwrap_or(u32::MAX))
    }

    pub(super) fn begin(&mut self, op: Operation, epoch: u64, now: Millis) -> Option<Ticket> {
        if epoch != self.epoch
            || self.needs_check
            || self.active.is_some()
            || self.wait(op, now).is_some()
        {
            return None;
        }
        self.serial += 1;
        let ticket = Ticket {
            serial: self.serial,
            epoch,
            operation: op,
        };
        self.active = Some(ticket);
        Some(ticket)
    }

    pub(super) fn owns(&self, ticket: Ticket) -> bool {
        self.active == Some(ticket) && self.epoch == ticket.epoch
    }

    /// Ends the HTTP lane regardless of whether the initiating form still owns the result.
    pub(super) fn finish(
        &mut self,
        ticket: Ticket,
        error: Option<&AuthError>,
        now: Millis,
    ) -> bool {
        if self.active != Some(ticket) {
            return false;
        }
        use Operation::*;
        let operation = ticket.operation;
        match error {
            Some(AuthError::RateLimited {
                retry_after_seconds: Some(seconds),
            }) => {
                self.retry_until[operation as usize] = now + Millis::from(*seconds) * 1_000;
            }
            None if self.epoch == ticket.epoch && operation == EmailChallenge => {
                self.resend_until[0] = now + RESEND_COOLDOWN_MS;
            }
            None if self.epoch == ticket.epoch && operation == EnrollmentChallenge => {
                self.resend_until[1] = now + RESEND_COOLDOWN_MS;
            }
            Some(AuthError::ChallengeClosed)
                if self.epoch == ticket.epoch && operation == EmailCode =>
            {
                self.resend_until[0] = 0;
            }
            Some(AuthError::ChallengeClosed)
                if self.epoch == ticket.epoch && operation == Enrollment =>
            {
                self.resend_until[1] = 0;
            }
            _ => {}
        }
        self.active = None;
        self.epoch == ticket.epoch
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_change_fences_even_unbound_or_same_account_and_keeps_the_http_lane() {
        let mut gate = AuthGate::default();
        let first = gate
            .begin(Operation::PasswordRegister, gate.epoch(), 0)
            .unwrap();
        assert!(
            gate.begin(Operation::PasswordRegister, gate.epoch(), 0)
                .is_none()
        );
        assert!(gate.begin(Operation::EmailCode, gate.epoch(), 0).is_none());
        // No principal/scope is involved in this invalidation.
        gate.invalidate(true);
        assert!(!gate.owns(first));
        assert!(gate.begin(Operation::PenName, gate.epoch(), 0).is_none());
        assert!(!gate.finish(first, None, 0), "discard the old form result");
        assert!(
            gate.begin(Operation::PenName, gate.epoch(), 0).is_none(),
            "await fresh standing"
        );
        gate.checked();
        assert!(
            gate.begin(Operation::PenName, 0, 0).is_none(),
            "old view cannot send"
        );
        let next = gate.begin(Operation::PenName, gate.epoch(), 0).unwrap();
        assert!(!gate.finish(first, Some(&AuthError::Network), 0));
        assert!(gate.owns(next), "late release never clears a newer write");
    }

    #[test]
    fn retry_after_survives_form_switch_but_applies_to_its_own_operation() {
        let mut gate = AuthGate::default();
        let send = gate.begin(Operation::EnrollmentChallenge, 0, 0).unwrap();
        assert!(gate.finish(
            send,
            Some(&AuthError::RateLimited {
                retry_after_seconds: Some(61)
            }),
            0
        ));
        assert_eq!(gate.wait(Operation::EnrollmentChallenge, 60_001), Some(1));
        assert!(
            gate.begin(Operation::EnrollmentChallenge, 0, 60_001)
                .is_none()
        );
        let exit = gate.begin(Operation::SignOut, 0, 60_001).unwrap();
        gate.finish(exit, None, 60_001);
        assert!(
            gate.begin(Operation::EnrollmentChallenge, 0, 61_000)
                .is_some()
        );
    }

    #[test]
    fn a_new_code_waits_after_success_and_a_closed_challenge_lifts_only_the_hint() {
        let mut gate = AuthGate::default();
        let send = gate.begin(Operation::EmailChallenge, 0, 0).unwrap();
        gate.finish(send, None, 0);
        assert_eq!(gate.wait(Operation::EmailChallenge, 59_001), Some(1));
        let complete = gate.begin(Operation::EmailCode, 0, 1).unwrap();
        gate.finish(complete, Some(&AuthError::ChallengeClosed), 2);
        assert!(gate.begin(Operation::EmailChallenge, 0, 2).is_some());
    }

    #[test]
    fn uncertain_session_writes_require_readback_but_invalid_credentials_do_not() {
        for op in [
            Operation::PasswordLogin,
            Operation::PasswordRegister,
            Operation::EmailCode,
            Operation::SignOut,
        ] {
            assert!(op.reread_session(Some(&AuthError::Network)));
            assert!(op.reread_session(Some(&AuthError::Unexpected { status: 200 })));
            assert!(op.reread_session(Some(&AuthError::Unavailable)));
        }
        assert!(!Operation::PasswordLogin.reread_session(Some(&AuthError::InvalidCredentials)));
        assert!(Operation::PasswordRegister.reread_session(Some(&AuthError::AlreadySignedIn)));
        assert!(
            !Operation::Enrollment.reread_session(None),
            "read credential status, not a new session"
        );
    }
}
