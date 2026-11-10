//! The tab's one source of session facts, and the sign-in flow in front of it.
//!
//! Pure decisions only (no HTTP, no reactivity): [`SessionOwner`] records what the server last
//! confirmed about the presented session; [`SignInFlow`] is the email → code exchange. The
//! reactive handle and the transports live in [`crate::account`].
//!
//! Rules:
//! - Only the server decides. A standing is replaced by an answer of the newest check; an older
//!   check that answers late is dropped, so a slow "anonymous" can never undo a sign-in.
//! - `generation` advances each time the confirmed principal changes (A → nobody, nobody → B,
//!   A → B) after the first confirmation. Private views key on it; a reply fetched for an earlier
//!   generation must not be shown. Returning to the same account is still a new generation.
//! - A temporary failure keeps the last confirmed standing: the server authorizes every request
//!   on its own, and a blip must not sign the page out or drop what the person is writing.
//! - Nothing here is stored: no credential, address or code reaches browser storage or SSR.

use crate::contract::CurrentAccount;

/// What the page may assume about the session.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Standing {
    /// Not asked yet (server render and the first moments after hydration).
    #[default]
    Unknown,
    Anonymous,
    /// Authenticated, no Vink principal yet: a pen name is needed.
    NeedsPrincipal,
    Suspended,
    Active(CurrentAccount),
    /// The first check could not be answered; nothing is known.
    Unavailable,
}

/// One answer of the server about the presented session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Observation {
    Anonymous,
    NeedsPrincipal,
    Suspended,
    Active(CurrentAccount),
    Unavailable,
}

/// Identifies one check. Only the newest may settle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckTicket(u64);

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SessionOwner {
    standing: Standing,
    generation: u64,
    confirmed: bool,
    checks: u64,
}

impl SessionOwner {
    pub fn standing(&self) -> &Standing {
        &self.standing
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// The principal private requests currently act as, if the server confirmed one.
    pub fn principal(&self) -> Option<&str> {
        match &self.standing {
            Standing::Active(account) => Some(&account.principal_id),
            _ => None,
        }
    }

    /// Scope of viewer-private state (own comments, endorsements): principal and generation.
    pub fn scope(&self) -> Option<String> {
        self.principal()
            .map(|principal| format!("{principal}/{}", self.generation))
    }

    pub fn begin_check(&mut self) -> CheckTicket {
        self.checks += 1;
        CheckTicket(self.checks)
    }

    /// Applies the newest check's answer. Returns whether the confirmed principal changed.
    pub fn settle(&mut self, ticket: CheckTicket, observation: Observation) -> bool {
        if ticket.0 != self.checks {
            return false;
        }
        let next = match observation {
            Observation::Unavailable => {
                if !self.confirmed {
                    self.standing = Standing::Unavailable;
                }
                return false;
            }
            Observation::Anonymous => Standing::Anonymous,
            Observation::NeedsPrincipal => Standing::NeedsPrincipal,
            Observation::Suspended => Standing::Suspended,
            Observation::Active(account) => Standing::Active(account),
        };
        let before = self.principal().map(str::to_owned);
        self.standing = next;
        let changed = self.confirmed && before.as_deref() != self.principal();
        if changed {
            self.generation += 1;
        }
        self.confirmed = true;
        changed
    }
}

/// Whether text typed on a page may stay there when the session changes. `typed_as` is the last
/// principal confirmed while the page was open (`None`: only ever a guest).
///
/// Text stays for the same person: a guest who signs in, a session that ends, the same account
/// signing back in. It never crosses to another account, also not through a signed-out moment.
pub fn text_survives(typed_as: Option<&str>, now: Option<&str>) -> bool {
    typed_as.is_none() || now.is_none() || typed_as == now
}

/// Why a sign-in request did not succeed, as the person needs to tell them apart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthError {
    /// `422` on `email`: not a mailbox address.
    InvalidEmail,
    /// `422` on `code`: not six digits (also decided locally before sending).
    InvalidCode,
    /// `422 code_rejected`: wrong code; the same challenge accepts another attempt.
    CodeRejected,
    /// `409 challenge_closed`: expired, used, exhausted or missing. Ask for a new code.
    ChallengeClosed,
    /// `403 account_disabled`.
    AccountDisabled,
    /// `403 principal_suspended`.
    Suspended,
    /// `429`, with the server's `Retry-After` seconds when it sent one.
    RateLimited {
        retry_after_seconds: Option<u32>,
    },
    /// `501 authority_not_configured`: sign-in is not open on this deployment.
    NotOpen,
    /// `503`: Identity or its directory cannot answer right now.
    Unavailable,
    /// `401`: the session ended before this request.
    Unauthenticated,
    /// `422` on `pen_name`.
    InvalidPenName,
    /// No answer. A request that changes the session may or may not have been applied.
    Network,
    Unexpected {
        status: u16,
    },
}

impl AuthError {
    /// Classifies a non-success answer of an `/auth` route.
    pub fn from_response(
        status: u16,
        code: Option<&str>,
        field: Option<&str>,
        retry_after: Option<&str>,
    ) -> Self {
        match (status, code, field) {
            (401, ..) => Self::Unauthenticated,
            (403, Some("account_disabled"), _) => Self::AccountDisabled,
            (403, Some("principal_suspended"), _) => Self::Suspended,
            (409, Some("challenge_closed"), _) => Self::ChallengeClosed,
            (422, Some("code_rejected"), _) => Self::CodeRejected,
            (422, _, Some("email")) => Self::InvalidEmail,
            (422, _, Some("code")) => Self::InvalidCode,
            (422, _, Some("pen_name")) => Self::InvalidPenName,
            (429, ..) => Self::RateLimited {
                retry_after_seconds: retry_after.and_then(retry_after_seconds),
            },
            (501, ..) => Self::NotOpen,
            (503, ..) => Self::Unavailable,
            _ => Self::Unexpected { status },
        }
    }

    /// What the person can do next, in their language. Never echoes server text.
    pub fn message(&self) -> String {
        match self {
            Self::InvalidEmail => "Địa chỉ email chưa đúng. Hãy kiểm tra rồi thử lại.".into(),
            Self::InvalidCode => "Mã gồm đúng 6 chữ số trong thư.".into(),
            Self::CodeRejected => "Mã chưa đúng. Hãy kiểm tra thư và nhập lại.".into(),
            Self::ChallengeClosed => {
                "Mã đã hết hạn hoặc không còn dùng được. Hãy xin mã mới.".into()
            }
            Self::AccountDisabled => {
                "Tài khoản của email này đang bị khóa đăng nhập. Hãy liên hệ Vink để được hỗ trợ."
                    .into()
            }
            Self::Suspended => {
                "Tài khoản đang bị tạm khóa trên Vink nên chưa thể đăng nhập.".into()
            }
            Self::RateLimited {
                retry_after_seconds: Some(seconds),
            } => format!(
                "Có quá nhiều yêu cầu. Hãy thử lại sau {}.",
                wait_text(*seconds)
            ),
            Self::RateLimited {
                retry_after_seconds: None,
            } => "Có quá nhiều yêu cầu. Hãy chờ ít phút rồi thử lại.".into(),
            Self::NotOpen => "Vink chưa mở đăng nhập.".into(),
            Self::Unavailable => {
                "Dịch vụ đăng nhập tạm thời chưa phản hồi. Chữ bạn đã nhập được giữ; hãy thử lại sau ít phút."
                    .into()
            }
            Self::Unauthenticated => "Phiên đăng nhập đã kết thúc. Hãy đăng nhập lại.".into(),
            Self::InvalidPenName => {
                "Bút danh cần từ 1 đến 80 ký tự và không chỉ gồm khoảng trắng.".into()
            }
            Self::Network => "Chưa nhận được phản hồi từ máy chủ. Hãy kiểm tra kết nối rồi thử lại.".into(),
            Self::Unexpected { .. } => "Có lỗi chưa xác định. Hãy thử lại.".into(),
        }
    }
}

/// `Retry-After` as whole seconds (the App never sends the date form).
pub fn retry_after_seconds(value: &str) -> Option<u32> {
    let value = value.trim();
    (!value.is_empty() && value.len() <= 6 && value.bytes().all(|b| b.is_ascii_digit()))
        .then(|| value.parse().ok())
        .flatten()
}

/// "45 giây", "2 phút": rounded up, so the person never retries a moment too early.
pub fn wait_text(seconds: u32) -> String {
    if seconds < 60 {
        format!("{} giây", seconds.max(1))
    } else {
        format!("{} phút", seconds.div_ceil(60))
    }
}

/// Milliseconds since the epoch (the browser clock; only used for waiting hints).
pub type Millis = i64;

/// The client's own minimum gap between two code requests; the server's limits still apply.
pub const RESEND_COOLDOWN_MS: Millis = 60_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Step {
    #[default]
    Email,
    Code,
}

/// Identifies one sign-in request. A reply for an older attempt is dropped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Attempt(u64);

/// Email → code. The pen-name step follows from the session's standing, not from this flow,
/// so a reload in the middle resumes at the right place.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SignInFlow {
    step: Step,
    /// The address the code was requested for, as typed. Memory only.
    email: String,
    busy: bool,
    error: Option<AuthError>,
    resend_at: Option<Millis>,
    expires_at: Option<Millis>,
    attempts: u64,
}

impl SignInFlow {
    pub fn step(&self) -> Step {
        self.step
    }

    pub fn email(&self) -> &str {
        &self.email
    }

    pub fn busy(&self) -> bool {
        self.busy
    }

    pub fn error(&self) -> Option<&AuthError> {
        self.error.as_ref()
    }

    pub fn expires_at(&self) -> Option<Millis> {
        self.expires_at
    }

    /// Seconds until another code may be requested; `None` when it may be requested now.
    pub fn resend_wait(&self, now: Millis) -> Option<u32> {
        let remaining = self.resend_at? - now;
        (remaining > 0).then(|| u32::try_from((remaining + 999) / 1_000).unwrap_or(u32::MAX))
    }

    fn begin(&mut self) -> Attempt {
        self.busy = true;
        self.error = None;
        self.attempts += 1;
        Attempt(self.attempts)
    }

    fn current(&self, attempt: Attempt) -> bool {
        self.busy && attempt.0 == self.attempts
    }

    /// Asks for a code (first request or a resend). `None`: nothing is sent.
    pub fn request_code(&mut self, email: &str, now: Millis) -> Option<(Attempt, String)> {
        if self.busy || self.resend_wait(now).is_some() {
            return None;
        }
        let email = email.trim();
        if email.is_empty() || !email.contains('@') {
            self.error = Some(AuthError::InvalidEmail);
            return None;
        }
        self.email = email.to_owned();
        Some((self.begin(), self.email.clone()))
    }

    pub fn code_sent(&mut self, attempt: Attempt, now: Millis, expires_at: Option<Millis>) {
        if !self.current(attempt) {
            return;
        }
        self.busy = false;
        self.step = Step::Code;
        self.expires_at = expires_at;
        self.resend_at = Some(now + RESEND_COOLDOWN_MS);
    }

    /// Submits the typed code. Spaces are dropped; anything but six digits is refused here.
    pub fn submit_code(&mut self, code: &str) -> Option<(Attempt, String)> {
        if self.busy || self.step != Step::Code {
            return None;
        }
        let code: String = code.chars().filter(|c| !c.is_whitespace()).collect();
        if code.len() != 6 || !code.bytes().all(|b| b.is_ascii_digit()) {
            self.error = Some(AuthError::InvalidCode);
            return None;
        }
        Some((self.begin(), code))
    }

    /// A refused or unanswered request. Typed text is never cleared by a failure.
    pub fn failed(&mut self, attempt: Attempt, error: AuthError, now: Millis) {
        if !self.current(attempt) {
            return;
        }
        self.busy = false;
        match &error {
            AuthError::RateLimited {
                retry_after_seconds: Some(seconds),
            } => self.resend_at = Some(now + Millis::from(*seconds) * 1_000),
            // The challenge is gone or the account cannot sign in: only a new request helps.
            AuthError::ChallengeClosed | AuthError::AccountDisabled | AuthError::Suspended => {
                self.step = Step::Email;
                self.expires_at = None;
                self.resend_at = None;
            }
            _ => {}
        }
        self.error = Some(error);
    }

    /// The session exists now; the flow forgets the address.
    pub fn signed_in(&mut self, attempt: Attempt) {
        if self.current(attempt) {
            *self = Self {
                attempts: self.attempts,
                ..Self::default()
            };
        }
    }

    /// Back to the address field, keeping what was typed.
    pub fn change_email(&mut self) {
        if !self.busy {
            self.step = Step::Email;
            self.error = None;
            self.expires_at = None;
        }
    }

    /// The session changed elsewhere (another tab signed in): this flow is over.
    pub fn reset(&mut self) {
        *self = Self {
            attempts: self.attempts + 1,
            ..Self::default()
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn account(id: &str) -> CurrentAccount {
        CurrentAccount {
            principal_id: id.into(),
            display_name: format!("Bút danh {id}"),
        }
    }

    fn settle(owner: &mut SessionOwner, observation: Observation) -> bool {
        let ticket = owner.begin_check();
        owner.settle(ticket, observation)
    }

    #[test]
    fn auth_web_first_confirmation_sets_the_standing_without_a_new_generation() {
        let mut owner = SessionOwner::default();
        assert_eq!(owner.standing(), &Standing::Unknown);
        assert!(!settle(&mut owner, Observation::Active(account("a"))));
        assert_eq!(owner.generation(), 0);
        assert_eq!(owner.principal(), Some("a"));
        assert_eq!(owner.scope().as_deref(), Some("a/0"));
        // The same account confirmed again (focus, another tab) changes nothing.
        assert!(!settle(&mut owner, Observation::Active(account("a"))));
        assert_eq!(owner.generation(), 0);
    }

    #[test]
    fn auth_web_account_switch_and_return_are_new_generations() {
        let mut owner = SessionOwner::default();
        settle(&mut owner, Observation::Active(account("a")));
        assert!(settle(&mut owner, Observation::Active(account("b"))));
        assert_eq!((owner.generation(), owner.principal()), (1, Some("b")));
        assert!(settle(&mut owner, Observation::Anonymous));
        assert_eq!((owner.generation(), owner.scope()), (2, None));
        // Back to A: what was read as A before must still not be trusted.
        assert!(settle(&mut owner, Observation::Active(account("a"))));
        assert_eq!(owner.scope().as_deref(), Some("a/3"));
        // No principal on either side: the same (absent) private scope.
        let mut guest = SessionOwner::default();
        settle(&mut guest, Observation::Anonymous);
        assert!(!settle(&mut guest, Observation::NeedsPrincipal));
        assert!(!settle(&mut guest, Observation::Suspended));
        assert_eq!(guest.generation(), 0);
    }

    #[test]
    fn auth_web_a_late_answer_of_an_older_check_never_replaces_a_newer_one() {
        let mut owner = SessionOwner::default();
        let slow = owner.begin_check();
        let fast = owner.begin_check();
        assert!(!owner.settle(fast, Observation::Active(account("b"))));
        assert!(!owner.settle(slow, Observation::Active(account("a"))));
        assert!(!owner.settle(slow, Observation::Anonymous));
        assert_eq!(owner.principal(), Some("b"));
        assert_eq!(owner.generation(), 0);
    }

    #[test]
    fn auth_web_a_temporary_failure_keeps_the_confirmed_standing() {
        let mut owner = SessionOwner::default();
        settle(&mut owner, Observation::Unavailable);
        assert_eq!(owner.standing(), &Standing::Unavailable);
        // Nothing was confirmed before, so the first real answer is not a "change".
        assert!(!settle(&mut owner, Observation::Active(account("a"))));
        assert!(!settle(&mut owner, Observation::Unavailable));
        assert_eq!(owner.principal(), Some("a"));
        assert_eq!(owner.generation(), 0);
    }

    #[test]
    fn auth_web_typed_text_stays_with_its_person_and_never_reaches_another_account() {
        assert!(text_survives(None, Some("a")), "a guest signs in");
        assert!(text_survives(Some("a"), None), "the session ends");
        assert!(
            text_survives(Some("a"), Some("a")),
            "the same account returns"
        );
        assert!(
            !text_survives(Some("a"), Some("b")),
            "A → B, also after a signed-out moment"
        );
        assert!(text_survives(None, None));
    }

    #[test]
    fn auth_errors_follow_the_contract_fixtures() {
        const FIXTURES: &str = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../services/vink-app/contracts/http/v1/fixtures"
        );
        let classify = |name: &str, status: u16, retry: Option<&str>| {
            let raw = std::fs::read_to_string(format!("{FIXTURES}/{name}")).unwrap();
            let envelope: crate::contract::ErrorEnvelope = serde_json::from_str(&raw).unwrap();
            AuthError::from_response(
                status,
                Some(&envelope.error.code),
                envelope.error.field.as_deref(),
                retry,
            )
        };
        assert_eq!(
            classify("error.code_rejected.json", 422, None),
            AuthError::CodeRejected
        );
        assert_eq!(
            classify("error.challenge_closed.json", 409, None),
            AuthError::ChallengeClosed
        );
        assert_eq!(
            classify("error.account_disabled.json", 403, None),
            AuthError::AccountDisabled
        );
        assert_eq!(
            classify("error.principal_suspended.json", 403, None),
            AuthError::Suspended
        );
        assert_eq!(
            classify("error.rate_limited.json", 429, Some("60")),
            AuthError::RateLimited {
                retry_after_seconds: Some(60)
            }
        );
        // Identity's own ceilings send no window: still a rate limit, without a time.
        assert_eq!(
            classify("error.rate_limited.json", 429, None),
            AuthError::RateLimited {
                retry_after_seconds: None
            }
        );
        assert_eq!(
            classify(
                "error.rate_limited.json",
                429,
                Some("Wed, 21 Oct 2026 07:28:00 GMT")
            ),
            AuthError::RateLimited {
                retry_after_seconds: None
            }
        );
        assert_eq!(
            classify("error.authority_not_configured.json", 501, None),
            AuthError::NotOpen
        );
        assert_eq!(
            classify("error.dependency_unavailable.json", 503, None),
            AuthError::Unavailable
        );
        assert_eq!(
            classify("error.unauthenticated.json", 401, None),
            AuthError::Unauthenticated
        );
        for (field, expected) in [
            ("email", AuthError::InvalidEmail),
            ("code", AuthError::InvalidCode),
            ("pen_name", AuthError::InvalidPenName),
        ] {
            assert_eq!(
                AuthError::from_response(422, Some("validation_failed"), Some(field), None),
                expected
            );
        }
        assert_eq!(
            AuthError::from_response(403, Some("forbidden"), None, None),
            AuthError::Unexpected { status: 403 }
        );
        assert_eq!(wait_text(0), "1 giây");
        assert_eq!(wait_text(59), "59 giây");
        assert_eq!(wait_text(61), "2 phút");
    }

    #[test]
    fn sign_in_flow_requests_once_and_waits_before_a_resend() {
        let mut flow = SignInFlow::default();
        assert!(flow.request_code("   ", 0).is_none());
        assert_eq!(flow.error(), Some(&AuthError::InvalidEmail));
        let (attempt, email) = flow.request_code("  an@vi-du.vn ", 1_000).unwrap();
        assert_eq!(email, "an@vi-du.vn");
        // A double press while the first request is outstanding sends nothing.
        assert!(flow.request_code("an@vi-du.vn", 1_000).is_none());
        flow.code_sent(attempt, 2_000, Some(602_000));
        assert_eq!(flow.step(), Step::Code);
        assert_eq!(flow.resend_wait(2_000), Some(60));
        assert_eq!(flow.resend_wait(61_001), Some(1));
        assert!(flow.request_code("an@vi-du.vn", 30_000).is_none());
        assert_eq!(flow.resend_wait(62_000), None);
        assert!(flow.request_code("an@vi-du.vn", 62_000).is_some());
    }

    #[test]
    fn sign_in_flow_keeps_the_step_on_a_wrong_code_and_restarts_on_a_closed_challenge() {
        let mut flow = SignInFlow::default();
        let (attempt, _) = flow.request_code("an@vi-du.vn", 0).unwrap();
        flow.code_sent(attempt, 0, None);
        assert!(flow.submit_code("12 34").is_none());
        assert_eq!(flow.error(), Some(&AuthError::InvalidCode));
        let (attempt, code) = flow.submit_code(" 123 456 ").unwrap();
        assert_eq!(code, "123456");
        assert!(
            flow.submit_code("123456").is_none(),
            "one request at a time"
        );
        flow.failed(attempt, AuthError::CodeRejected, 0);
        assert_eq!(flow.step(), Step::Code);
        let (attempt, _) = flow.submit_code("654321").unwrap();
        flow.failed(attempt, AuthError::ChallengeClosed, 0);
        assert_eq!((flow.step(), flow.email()), (Step::Email, "an@vi-du.vn"));
        assert_eq!(flow.error(), Some(&AuthError::ChallengeClosed));
        // A closed challenge lifts the client's own wait: a new code is the only way on.
        assert!(flow.request_code("an@vi-du.vn", 1).is_some());
    }

    #[test]
    fn sign_in_flow_honours_retry_after_and_drops_stale_replies() {
        let mut flow = SignInFlow::default();
        let (attempt, _) = flow.request_code("an@vi-du.vn", 0).unwrap();
        flow.failed(
            attempt,
            AuthError::RateLimited {
                retry_after_seconds: Some(120),
            },
            10_000,
        );
        assert_eq!(flow.step(), Step::Email);
        assert_eq!(flow.resend_wait(10_000), Some(120));
        assert!(flow.request_code("an@vi-du.vn", 100_000).is_none());
        let (old, _) = flow.request_code("an@vi-du.vn", 130_000).unwrap();
        // Another tab signed in meanwhile: the reply of this attempt changes nothing.
        flow.reset();
        flow.code_sent(old, 130_500, None);
        flow.failed(old, AuthError::Unavailable, 130_500);
        assert_eq!(
            (flow.step(), flow.error(), flow.busy()),
            (Step::Email, None, false)
        );
        // Outage and lost answers keep the step and the typed address.
        let (attempt, _) = flow.request_code("an@vi-du.vn", 200_000).unwrap();
        flow.code_sent(attempt, 200_000, None);
        let (attempt, _) = flow.submit_code("123456").unwrap();
        flow.failed(attempt, AuthError::Unavailable, 200_000);
        assert_eq!((flow.step(), flow.email()), (Step::Code, "an@vi-du.vn"));
        let (attempt, _) = flow.submit_code("123456").unwrap();
        flow.signed_in(attempt);
        assert_eq!((flow.step(), flow.email()), (Step::Email, ""));
    }
}
