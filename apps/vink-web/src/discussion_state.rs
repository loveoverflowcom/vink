//! Browser communication state, not discussion authority. The server owns permissions,
//! visibility, target placement and idempotency. No text or identity is persisted here.
use std::collections::BTreeMap;

use crate::contract::{ApiError, CommentsSettings, SubmitCommentRequest, SubmittedComment};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scope {
    pub work_id: String,
    /// A trusted host's account/session generation. Currently only the local fixture supplies
    /// one; it is an opaque marker, never a credential or a client-selected API actor.
    pub session: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ComposerKey {
    Root,
    Replies(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    pub id: String,
    pub label: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Draft {
    pub body: String,
    pub composing: bool,
    pub target: Option<Target>,
    pub target_available: bool,
    pub open: bool,
    pub receipt: Option<Receipt>,
    pub delivery: Delivery,
    /// Typed ACK identity retained for current-state lookup, never inferred from body text.
    pub acknowledged: Option<AcknowledgedComment>,
}

impl Draft {
    /// Typed text or a command whose outcome is still being tracked: collapsing or scrolling
    /// the branch away must keep a place where this stays visible.
    pub fn holds_work(&self) -> bool {
        !self.body.is_empty() || !matches!(self.delivery, Delivery::Editing)
    }

    /// `holds_work`, or a receipt the writer has not seen settle yet.
    pub fn needs_recovery(&self) -> bool {
        self.holds_work() || self.receipt.is_some()
    }
}

/// What the last send of one composer came back with. Typed so each kind gets its own tonal
/// notice (R4); never derived from the body text or from wording.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Receipt {
    /// Stored and waiting for review: every new comment under `premoderation.v1`.
    Pending,
    /// Stored and already public; the loaded public list may not show it yet.
    Approved,
    /// Stored and not approved; the reason is only the writer's, in “Bạn đã gửi”.
    Rejected,
    /// Stored, then no longer shown.
    Removed,
    /// No answer proves either outcome; reconciling replays the same key and body.
    Uncertain,
    /// Refused before anything was stored, by contract code. Typed text stays.
    Refused(ApiError),
    /// The session changed before the earlier command was confirmed; never replayed.
    SessionChanged,
}

/// One accent per notice: amber waiting, blue unknown, red adverse, green success.
/// Colour is never the only signal; the message names the state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NoticeTone {
    Neutral,
    Waiting,
    Unknown,
    Adverse,
    Success,
}

impl NoticeTone {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Neutral => "neutral",
            Self::Waiting => "waiting",
            Self::Unknown => "unknown",
            Self::Adverse => "adverse",
            Self::Success => "success",
        }
    }
}

impl Receipt {
    pub fn tone(&self) -> NoticeTone {
        match self {
            Self::Pending => NoticeTone::Waiting,
            Self::Uncertain | Self::SessionChanged => NoticeTone::Unknown,
            Self::Rejected | Self::Refused(_) => NoticeTone::Adverse,
            Self::Approved | Self::Removed => NoticeTone::Neutral,
        }
    }

    pub fn message(&self) -> &'static str {
        match self {
            Self::Pending => "Chờ duyệt · Chưa công khai",
            Self::Approved => "Lần gửi này đã được duyệt. Tải lại để xem trạng thái hiện tại.",
            Self::Rejected => "Lần gửi này không được duyệt. Tải lại để xem lý do được cấp.",
            Self::Removed => "Nhận xét của lần gửi này không còn hiển thị.",
            Self::Uncertain => {
                "Chưa rõ máy chủ đã nhận hay chưa. Đối soát dùng lại đúng nội dung và mã của lần gửi trước; chữ mới vẫn được giữ."
            }
            Self::Refused(error) => error_message(error),
            Self::SessionChanged => {
                "Phiên đăng nhập đã thay đổi trước khi lần gửi trước được xác nhận. Tải lại nhận xét để kiểm tra trước khi gửi lại; chữ vẫn được giữ."
            }
        }
    }
}

/// The one primary state a composer shows (R4 image 04). Focus is presentation only and the
/// lock reason (`reason`) is separate, so a guest's typed text is never mistaken for a send.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ComposerStatus {
    Draft,
    Sending,
    CommittedPending,
    Uncertain,
    Refused,
    SignInAvailable,
    SignInClosed,
    Closed,
}

impl ComposerStatus {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Sending => "sending",
            Self::CommittedPending => "committed-pending",
            Self::Uncertain => "uncertain",
            Self::Refused => "refused",
            Self::SignInAvailable => "sign-in",
            Self::SignInClosed => "sign-in-closed",
            Self::Closed => "closed",
        }
    }
}

/// Emphasis of “Gửi”: tonal while empty, filled once there is text, progress while sending.
/// Whether it may be pressed is `reason`, never the emphasis.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SendEmphasis {
    Tonal,
    Filled,
    Progress,
}

/// The reply composer's target chip (R4 image 05). An unavailable context is not inferred as
/// deleted and offers another target; sending stays blocked until a visible one is chosen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReplyTarget {
    /// “Trả lời bình luận gốc”: the branch's root, nothing to clear.
    Root,
    /// “Đang trả lời {name}” with a control back to the root when the root accepts replies.
    Reply { name: String },
    /// “Chọn bình luận khác” + “Sao chép nháp”.
    Unavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AcknowledgedComment {
    pub comment_id: String,
    pub status: crate::contract::CommentStatus,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Delivery {
    #[default]
    Editing,
    Sending(Submission),
    Uncertain(Submission),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Submission {
    pub scope: Scope,
    pub composer: ComposerKey,
    pub request: SubmitCommentRequest,
    sequence: u64,
    edit_generation: u64,
}

#[derive(Clone, Debug)]
pub struct DiscussionController {
    pub scope: Scope,
    pub settings: CommentsSettings,
    pub drafts: BTreeMap<ComposerKey, Draft>,
    /// Whether this deployment offers sign-in; only chooses the guest's reason text.
    pub sign_in_open: bool,
    /// The server's `viewer_is_work_author`; only chooses “Bạn đã tắt…” wording, never a
    /// permission. Every write is decided again by the server.
    pub viewer_is_author: bool,
    session_invalid: bool,
    sequence: u64,
    edits: BTreeMap<ComposerKey, u64>,
}

impl DiscussionController {
    pub fn new(scope: Scope, settings: CommentsSettings) -> Self {
        Self {
            scope,
            settings,
            drafts: BTreeMap::new(),
            sign_in_open: false,
            viewer_is_author: false,
            session_invalid: false,
            sequence: 0,
            edits: BTreeMap::new(),
        }
    }

    /// A different actor/session/work never receives another scope's text or in-flight state.
    pub fn change_scope(&mut self, scope: Scope) {
        if self.scope != scope {
            self.scope = scope;
            self.drafts.clear();
            self.edits.clear();
            self.session_invalid = false;
            self.viewer_is_author = false;
        }
    }

    /// The same person on the same work under another session: signed in after composing as a
    /// guest, lost the session, or signed back in. Typed text and reply context stay on the
    /// page. What belonged to the earlier session does not: a command in flight or uncertain
    /// is no longer tracked (its late answer cannot settle, and it is never replayed under
    /// another session) and receipts are dropped.
    pub fn change_session(&mut self, session: Option<String>) {
        if self.scope.session == session {
            return;
        }
        self.scope.session = session;
        self.session_invalid = false;
        self.viewer_is_author = false;
        for draft in self.drafts.values_mut() {
            draft.acknowledged = None;
            draft.receipt = match std::mem::take(&mut draft.delivery) {
                Delivery::Editing => None,
                Delivery::Sending(_) | Delivery::Uncertain(_) => Some(Receipt::SessionChanged),
            };
        }
    }

    pub fn write_ready(&self) -> bool {
        self.scope.session.is_some() && !self.session_invalid
    }

    pub fn draft(&self, key: &ComposerKey) -> Draft {
        self.drafts.get(key).cloned().unwrap_or_default()
    }

    pub fn edit(&mut self, key: &ComposerKey, body: String, composing: bool) {
        let draft = self.drafts.entry(key.clone()).or_default();
        if draft.body != body || draft.composing != composing {
            *self.edits.entry(key.clone()).or_default() += 1;
        }
        draft.body = body;
        draft.composing = composing;
    }

    pub fn reply(&mut self, root: String, target: Target) {
        let key = ComposerKey::Replies(root);
        self.drafts.entry(key.clone()).or_default().open = true;
        let draft = self.drafts.get_mut(&key).expect("inserted draft");
        if draft.target.as_ref() != Some(&target) {
            *self.edits.entry(key).or_default() += 1;
        }
        draft.target = Some(target);
        draft.target_available = true;
    }

    /// Revoke cached identity/context labels with the latest authorized projection. User
    /// text and the immutable submitted snapshot remain intact, including on off-page loss.
    pub fn revalidate_targets(&mut self, visible: &[(String, String, String)]) {
        for (key, draft) in &mut self.drafts {
            let ComposerKey::Replies(root) = key else {
                continue;
            };
            let Some(target) = &mut draft.target else {
                continue;
            };
            if let Some((_, _, name)) = visible
                .iter()
                .find(|(r, id, _)| r == root && *id == target.id)
            {
                target.label = name.clone();
                draft.target_available = true;
            } else {
                target.label =
                    "Bình luận được trả lời không còn hiển thị trong danh sách đã tải".into();
                draft.target_available = false;
            }
        }
    }

    pub fn close_reply(&mut self, key: &ComposerKey) {
        self.drafts.entry(key.clone()).or_default().open = false;
    }

    /// “Mở lại” on a collapsed reply: the same text, target and in-flight state come back.
    pub fn reopen(&mut self, key: &ComposerKey) {
        if let Some(draft) = self.drafts.get_mut(key) {
            draft.open = true;
        }
    }

    /// A reply composer that was collapsed while it still holds text or a tracked send.
    pub fn collapsed_with_work(&self, key: &ComposerKey) -> bool {
        matches!(key, ComposerKey::Replies(_))
            && self
                .drafts
                .get(key)
                .is_some_and(|d| !d.open && d.holds_work())
    }

    pub fn reply_target(&self, key: &ComposerKey) -> Option<ReplyTarget> {
        let ComposerKey::Replies(root) = key else {
            return None;
        };
        let draft = self.drafts.get(key)?;
        let target = draft.target.as_ref()?;
        Some(if !draft.target_available {
            ReplyTarget::Unavailable
        } else if &target.id == root {
            ReplyTarget::Root
        } else {
            ReplyTarget::Reply {
                name: target.label.clone(),
            }
        })
    }

    /// The one primary state for `key`. In flight wins, then the work's policy, then who is
    /// writing, then the last receipt; a peer composer blocked by another composer's refusal
    /// stays a draft whose lock reason explains why.
    pub fn status(&self, key: &ComposerKey) -> ComposerStatus {
        let draft = self.drafts.get(key);
        match draft.map(|d| &d.delivery) {
            Some(Delivery::Sending(_)) => return ComposerStatus::Sending,
            Some(Delivery::Uncertain(_)) => return ComposerStatus::Uncertain,
            _ => {}
        }
        if !self.settings.comments_open {
            return ComposerStatus::Closed;
        }
        if self.scope.session.is_none() {
            return if self.sign_in_open {
                ComposerStatus::SignInAvailable
            } else {
                ComposerStatus::SignInClosed
            };
        }
        match draft.and_then(|d| d.receipt.as_ref()) {
            Some(Receipt::Pending) => ComposerStatus::CommittedPending,
            Some(receipt) if receipt.tone() == NoticeTone::Adverse => ComposerStatus::Refused,
            _ => ComposerStatus::Draft,
        }
    }

    pub fn send_emphasis(&self, key: &ComposerKey) -> SendEmphasis {
        match self.drafts.get(key) {
            Some(draft) if matches!(draft.delivery, Delivery::Sending(_)) => SendEmphasis::Progress,
            Some(draft) if !draft.body.trim().is_empty() => SendEmphasis::Filled,
            _ => SendEmphasis::Tonal,
        }
    }

    pub fn reason(&self, key: &ComposerKey) -> Option<&'static str> {
        let draft = self.drafts.get(key);
        if draft.is_some_and(|d| matches!(d.delivery, Delivery::Sending(_))) {
            return Some("Đang gửi nhận xét…");
        }
        if draft.is_some_and(|d| matches!(d.delivery, Delivery::Uncertain(_))) {
            return Some("Kiểm tra lần gửi trước khi gửi tiếp.");
        }
        if !self.settings.comments_open {
            return Some(if self.viewer_is_author {
                "Bạn đã tắt bình luận cho bài viết này. Bản đang soạn được giữ lại."
            } else {
                "Tác giả đã tắt bình luận cho bài viết này. Bản đang soạn được giữ lại."
            });
        }
        if self.session_invalid {
            return Some(
                "Máy chủ chưa xác nhận phiên đăng nhập. Tải lại để đối chiếu; chữ vẫn được giữ.",
            );
        }
        if !self.write_ready() {
            return Some(if self.sign_in_open {
                "Cần phiên đăng nhập hợp lệ để gửi. Chữ bạn soạn tại đây được giữ lại."
            } else {
                "Vink chưa mở đăng nhập; chưa thể gửi nhận xét. Bạn có thể soạn tại đây."
            });
        }
        if draft.is_some_and(|d| d.target.is_some() && !d.target_available) {
            return Some(
                "Chọn lại bình luận được phép trả lời; ngữ cảnh cũ chưa khả dụng. Chữ được giữ lại.",
            );
        }
        if draft.is_some_and(|d| d.composing) {
            return Some("Hoàn tất nhập chữ trước khi gửi.");
        }
        if draft.is_none_or(|d| d.body.trim().is_empty()) {
            return Some("Viết nhận xét trước khi gửi.");
        }
        None
    }

    pub fn begin(&mut self, key: &ComposerKey, command_key: String) -> Option<Submission> {
        if self.reason(key).is_some() {
            return None;
        }
        self.sequence += 1;
        let draft = self.drafts.entry(key.clone()).or_default();
        let submission = Submission {
            scope: self.scope.clone(),
            composer: key.clone(),
            sequence: self.sequence,
            edit_generation: *self.edits.get(key).unwrap_or(&0),
            request: SubmitCommentRequest {
                body: draft.body.clone(),
                command_key,
                target_id: draft.target.as_ref().map(|target| target.id.clone()),
            },
        };
        draft.receipt = None;
        draft.acknowledged = None;
        draft.delivery = Delivery::Sending(submission.clone());
        Some(submission)
    }

    /// The API has no receipt GET. Its documented reconciliation is an explicit replay of
    /// this exact keyed command, even if settings closed or the target disappeared later.
    pub fn reconcile(&mut self, key: &ComposerKey) -> Option<Submission> {
        let draft = self.drafts.get_mut(key)?;
        let Delivery::Uncertain(submission) = &draft.delivery else {
            return None;
        };
        if submission.scope != self.scope || self.scope.session.is_none() || self.session_invalid {
            return None;
        }
        let mut submission = submission.clone();
        self.sequence += 1;
        submission.sequence = self.sequence;
        draft.delivery = Delivery::Sending(submission.clone());
        Some(submission)
    }

    /// Returns false for a completion that no longer belongs to the active scope/request.
    pub fn complete(
        &mut self,
        submission: &Submission,
        result: Result<SubmittedComment, ApiError>,
    ) -> bool {
        if self.scope != submission.scope {
            return false;
        }
        let Some(draft) = self.drafts.get_mut(&submission.composer) else {
            return false;
        };
        if draft.delivery != Delivery::Sending(submission.clone()) {
            return false;
        }
        match result {
            Ok(receipt)
                if matches!(
                    receipt.status.as_str(),
                    "pending" | "approved" | "rejected" | "removed"
                ) =>
            {
                // Generation also catches type → undo to the same string and target changes.
                if !draft.composing
                    && self.edits.get(&submission.composer).copied().unwrap_or(0)
                        == submission.edit_generation
                {
                    draft.body.clear();
                }
                draft.acknowledged = Some(AcknowledgedComment {
                    comment_id: receipt.comment_id.clone(),
                    status: match receipt.status.as_str() {
                        "pending" => crate::contract::CommentStatus::Pending,
                        "approved" => crate::contract::CommentStatus::Approved,
                        "rejected" => crate::contract::CommentStatus::Rejected,
                        _ => crate::contract::CommentStatus::Removed,
                    },
                });
                draft.receipt = Some(match receipt.status.as_str() {
                    "pending" => Receipt::Pending,
                    "approved" => Receipt::Approved,
                    "rejected" => Receipt::Rejected,
                    _ => Receipt::Removed,
                });
                draft.delivery = Delivery::Editing;
            }
            Ok(_)
            | Err(ApiError::Network | ApiError::Unavailable | ApiError::Unexpected { .. }) => {
                draft.delivery = Delivery::Uncertain(submission.clone());
                draft.receipt = Some(Receipt::Uncertain);
            }
            Err(error) => {
                draft.delivery = Delivery::Editing;
                draft.receipt = Some(Receipt::Refused(error.clone()));
                if error == ApiError::TargetUnavailable {
                    draft.target_available = false;
                    if let Some(target) = &mut draft.target {
                        target.label = "Bình luận được trả lời không còn hiển thị".into();
                    }
                }
                if error == ApiError::CommentsClosed {
                    self.settings.comments_open = false;
                }
                if error == ApiError::Unauthenticated {
                    self.session_invalid = true;
                }
            }
        }
        true
    }
}

/// Failed reads keep the loaded projection and never imply an unknown write outcome.
pub fn read_error_message(error: &ApiError, append: bool) -> &'static str {
    match error {
        ApiError::Unauthenticated | ApiError::Forbidden | ApiError::NotFound => {
            error_message(error)
        }
        _ if append => "Chưa tải thêm được. Phần đã tải vẫn giữ nguyên; bạn có thể thử lại.",
        _ => {
            "Chưa làm mới được nhận xét. Phần đã tải và bản đang soạn vẫn giữ nguyên; bạn có thể thử lại."
        }
    }
}

pub fn error_message(error: &ApiError) -> &'static str {
    match error {
        ApiError::Unauthenticated => {
            "Máy chủ chưa xác nhận phiên đăng nhập. Bản đang soạn được giữ lại."
        }
        // Endorsement refusals never reach the composer; keep the generic refusal wording.
        ApiError::Forbidden | ApiError::SelfEndorsement | ApiError::EndorsementNotPermitted => {
            "Máy chủ từ chối thao tác này. Bản đang soạn được giữ lại."
        }
        ApiError::NotFound => {
            "Tác phẩm hoặc nhận xét không còn khả dụng. Bản đang soạn được giữ lại."
        }
        ApiError::CommentsClosed => "Tác giả đã tắt bình luận. Bản đang soạn được giữ lại.",
        ApiError::TargetUnavailable => {
            "Bình luận được trả lời không còn hiển thị. Chọn lại nơi trả lời; chữ vẫn được giữ."
        }
        ApiError::KeyReused => {
            "Mã lần gửi không khớp nội dung. Chưa gửi lại; bản đang soạn được giữ lại."
        }
        ApiError::Invalid { .. } | ApiError::InvalidSubmission { .. } => {
            "Nội dung chưa hợp lệ (tối đa 4.000 ký tự, 200 dòng). Sửa rồi gửi lại; chữ vẫn được giữ."
        }
        // Draft-tag answers never reach the composer; a conflict-shaped answer reads as one.
        ApiError::VersionConflict { .. }
        | ApiError::EndorsementConflict { .. }
        | ApiError::RevisionChanged { .. }
        | ApiError::TagConflict { .. }
        | ApiError::TagRefused(_)
        // Publication answers never reach the composer either.
        | ApiError::SnapshotConflict { .. }
        | ApiError::GenerationConflict { .. }
        | ApiError::RevisionRejected
        | ApiError::UnknownRevision
        | ApiError::WorkUnavailable
        | ApiError::SelfFollow
        | ApiError::RelationshipListPrivate
        | ApiError::ProfileGone
        | ApiError::FollowActorUnavailable
        | ApiError::FollowConflict { .. }
        | ApiError::HandleRefused(_)
        | ApiError::FollowKeyReused => {
            "Thiết lập đã thay đổi ở nơi khác. Tải lại trước khi đổi tiếp."
        }
        ApiError::CommentRemoved => "Nhận xét không còn hiển thị.",
        ApiError::Network => {
            "Mất kết nối; chưa rõ kết quả. Tải lại để đối chiếu trước khi thao tác tiếp."
        }
        ApiError::Unavailable | ApiError::Unexpected { .. } => {
            "Chưa rõ kết quả từ máy chủ. Tải lại để đối chiếu trước khi thao tác tiếp."
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn controller() -> DiscussionController {
        DiscussionController::new(
            Scope {
                work_id: "work-a".into(),
                session: Some("actor-a/session-1".into()),
            },
            CommentsSettings {
                comments_open: true,
                generation: 1,
            },
        )
    }
    fn ack() -> Result<SubmittedComment, ApiError> {
        Ok(SubmittedComment {
            comment_id: "c1".into(),
            status: "pending".into(),
            replayed: false,
        })
    }

    #[test]
    fn d2t_03_new_typing_and_target_changes_survive_late_ack() {
        let mut c = controller();
        let key = ComposerKey::Replies("root".into());
        c.reply(
            "root".into(),
            Target {
                id: "reply-a".into(),
                label: "A".into(),
            },
        );
        c.edit(&key, "Khổ thơ\n\n👩‍💻".into(), false);
        let sent = c.begin(&key, "command-1".into()).unwrap();
        assert!(c.begin(&key, "command-2".into()).is_none());
        c.edit(&key, "Chữ mới".into(), false);
        c.reply(
            "root".into(),
            Target {
                id: "reply-b".into(),
                label: "B".into(),
            },
        );
        assert!(c.complete(&sent, ack()));
        assert_eq!(c.draft(&key).body, "Chữ mới");
        assert_eq!(c.draft(&key).target.unwrap().id, "reply-b");
        assert_eq!(c.draft(&key).receipt, Some(Receipt::Pending));
        assert!(Receipt::Pending.message().contains("Chưa công khai"));
        assert!(!c.complete(&sent, ack()));
    }
    #[test]
    fn d2t_03_lost_ack_replays_original_key_body_target_after_close() {
        let mut c = controller();
        let key = ComposerKey::Root;
        c.edit(&key, "Một lần gửi".into(), false);
        let sent = c.begin(&key, "unique-key".into()).unwrap();
        c.complete(&sent, Err(ApiError::Network));
        c.settings.comments_open = false;
        c.edit(&key, "Gõ tiếp sau mất mạng".into(), false);
        assert!(c.begin(&key, "another-key".into()).is_none());
        let replay = c.reconcile(&key).unwrap();
        assert_eq!(replay.request, sent.request);
        assert!(!c.complete(&sent, Err(ApiError::CommentsClosed)));
        assert!(matches!(c.draft(&key).delivery, Delivery::Sending(_)));
        assert!(c.begin(&key, "new-during-replay".into()).is_none());
        c.complete(&replay, ack());
        assert_eq!(c.draft(&key).body, "Gõ tiếp sau mất mạng");
        assert!(!c.settings.comments_open);
    }
    #[test]
    fn d2t_03_original_late_ack_cannot_finish_a_new_physical_attempt() {
        let mut c = controller();
        let key = ComposerKey::Root;
        c.edit(&key, "Một nhận xét".into(), false);
        let first = c.begin(&key, "same-logical-command".into()).unwrap();
        c.complete(&first, Err(ApiError::Network));
        let replay = c.reconcile(&key).unwrap();
        assert_eq!(first.request, replay.request);
        assert!(!c.complete(&first, ack()));
        assert!(matches!(c.draft(&key).delivery, Delivery::Sending(_)));
        assert!(c.complete(&replay, ack()));
    }
    #[test]
    fn d2t_03_scope_change_clears_drafts_and_fences_completion_even_after_return() {
        let mut c = controller();
        let original = c.scope.clone();
        let key = ComposerKey::Root;
        c.edit(&key, "Riêng của A".into(), false);
        let old = c.begin(&key, "old-key".into()).unwrap();
        c.change_scope(Scope {
            work_id: "work-b".into(),
            session: Some("actor-b/session-2".into()),
        });
        assert_eq!(c.draft(&key), Draft::default());
        assert!(!c.complete(&old, ack()));
        c.change_scope(original);
        c.edit(&key, "A viết lại".into(), false);
        let new = c.begin(&key, "new-key".into()).unwrap();
        assert!(!c.complete(&old, ack()));
        assert!(c.complete(&new, ack()));
        assert!(c.draft(&key).body.is_empty());
    }
    #[test]
    fn d2t_03_ime_policy_and_branch_collapse_preserve_text() {
        let mut c = controller();
        let key = ComposerKey::Replies("root".into());
        c.reply(
            "root".into(),
            Target {
                id: "root".into(),
                label: "gốc".into(),
            },
        );
        c.edit(&key, "Tiếng Việt".into(), true);
        assert!(c.begin(&key, "ime-key".into()).is_none());
        c.close_reply(&key);
        c.settings.comments_open = false;
        c.reply(
            "root".into(),
            Target {
                id: "reply".into(),
                label: "người đọc".into(),
            },
        );
        assert_eq!(c.draft(&key).body, "Tiếng Việt");
        assert!(c.begin(&key, "closed-key".into()).is_none());
        c.settings.comments_open = true;
        c.edit(&key, "Tiếng Việt".into(), false);
        assert_eq!(
            c.begin(&key, "open-key".into())
                .unwrap()
                .request
                .target_id
                .as_deref(),
            Some("reply")
        );
    }
    #[test]
    fn d2t_03_guest_never_sends_and_rejection_keeps_draft() {
        let mut c = controller();
        let key = ComposerKey::Replies("root".into());
        c.reply(
            "root".into(),
            Target {
                id: "target".into(),
                label: "Tên bị thu hồi".into(),
            },
        );
        c.edit(&key, "Giữ chữ".into(), false);
        c.scope.session = None;
        assert!(c.begin(&key, "guest-key".into()).is_none());
        c.scope.session = Some("fixture".into());
        let sent = c.begin(&key, "valid-key".into()).unwrap();
        c.complete(&sent, Err(ApiError::TargetUnavailable));
        assert_eq!(c.draft(&key).body, "Giữ chữ");
        assert!(!c.draft(&key).target_available);
        assert!(
            !c.draft(&key)
                .target
                .unwrap()
                .label
                .contains("Tên bị thu hồi")
        );
        assert!(c.begin(&key, "no-replay-stale-target".into()).is_none());
    }

    #[test]
    fn auth_web_session_change_keeps_typed_text_and_fences_the_earlier_session() {
        let mut c = DiscussionController::new(
            Scope {
                work_id: "w".into(),
                session: None,
            },
            CommentsSettings {
                comments_open: true,
                generation: 1,
            },
        );
        // Composed as a guest, then signed in: the text is still there and can be sent.
        c.edit(
            &ComposerKey::Root,
            "Chữ soạn khi chưa đăng nhập".into(),
            false,
        );
        assert!(
            c.reason(&ComposerKey::Root)
                .unwrap()
                .contains("Vink chưa mở")
        );
        c.sign_in_open = true;
        assert!(
            c.reason(&ComposerKey::Root)
                .unwrap()
                .contains("Cần phiên đăng nhập")
        );
        c.change_session(Some("a/0".into()));
        assert_eq!(
            c.draft(&ComposerKey::Root).body,
            "Chữ soạn khi chưa đăng nhập"
        );
        let sent = c.begin(&ComposerKey::Root, "key-1".into()).expect("ready");
        // The session ends while the command is in flight: text stays, the answer is dropped.
        c.change_session(None);
        let draft = c.draft(&ComposerKey::Root);
        assert_eq!(draft.body, "Chữ soạn khi chưa đăng nhập");
        assert_eq!(draft.delivery, Delivery::Editing);
        assert_eq!(draft.receipt, Some(Receipt::SessionChanged));
        assert!(
            Receipt::SessionChanged
                .message()
                .contains("Phiên đăng nhập đã thay đổi")
        );
        assert!(!c.complete(&sent, Err(ApiError::Unauthenticated)));
        assert!(c.reconcile(&ComposerKey::Root).is_none());
        // Signed back in: a new command, never a replay of the earlier session's key.
        c.change_session(Some("a/2".into()));
        let again = c
            .begin(&ComposerKey::Root, "key-2".into())
            .expect("ready again");
        assert_ne!(again.scope, sent.scope);
        // Another actor is a different scope: nothing typed before reaches it.
        c.change_scope(Scope {
            work_id: "w".into(),
            session: Some("b/3".into()),
        });
        assert_eq!(c.draft(&ComposerKey::Root).body, "");
    }

    #[test]
    fn d2t_03_session_invalid_blocks_new_commands_without_losing_draft_or_stranding_peers() {
        let mut c = controller();
        let root = ComposerKey::Root;
        let reply = ComposerKey::Replies("root".into());
        c.edit(&root, "Giữ nhận xét gốc".into(), false);
        c.reply(
            "root".into(),
            Target {
                id: "root".into(),
                label: "Tác giả".into(),
            },
        );
        c.edit(&reply, "Giữ trả lời".into(), false);
        let first = c.begin(&root, "first-command".into()).unwrap();
        let peer = c.begin(&reply, "peer-command".into()).unwrap();
        assert!(c.complete(&first, Err(ApiError::Unauthenticated)));
        assert_eq!(c.draft(&root).body, "Giữ nhận xét gốc");
        assert!(!c.write_ready());
        assert!(c.begin(&root, "blocked-command".into()).is_none());
        assert!(c.complete(&peer, Err(ApiError::Unauthenticated)));
        assert!(matches!(c.draft(&reply).delivery, Delivery::Editing));
        assert_eq!(c.draft(&reply).body, "Giữ trả lời");
    }
    #[test]
    fn d2t_03_context_revocation_removes_cached_name_but_preserves_text_and_peers() {
        let mut c = controller();
        let key = ComposerKey::Replies("root".into());
        c.reply(
            "root".into(),
            Target {
                id: "hidden-context".into(),
                label: "Tên đã thu hồi".into(),
            },
        );
        c.edit(&key, "Lời đang viết".into(), false);
        let visible = vec![("root".into(), "peer".into(), "Người vẫn được thấy".into())];
        c.revalidate_targets(&visible);
        assert!(
            !c.draft(&key)
                .target
                .unwrap()
                .label
                .contains("Tên đã thu hồi")
        );
        assert_eq!(c.draft(&key).body, "Lời đang viết");
        assert!(c.begin(&key, "blocked-target".into()).is_none());
        c.reply(
            "root".into(),
            Target {
                id: "peer".into(),
                label: "Người vẫn được thấy".into(),
            },
        );
        assert_eq!(
            c.begin(&key, "peer-target".into())
                .unwrap()
                .request
                .target_id
                .as_deref(),
            Some("peer")
        );
    }

    #[test]
    fn r4_composer_shows_one_primary_status_in_a_fixed_precedence() {
        let mut c = controller();
        let key = ComposerKey::Root;
        assert_eq!(c.status(&key), ComposerStatus::Draft);
        assert_eq!(c.send_emphasis(&key), SendEmphasis::Tonal);
        c.edit(&key, "   ".into(), false);
        assert_eq!(c.send_emphasis(&key), SendEmphasis::Tonal, "blank is empty");
        c.edit(&key, "Câu thơ\nthứ hai".into(), false);
        assert_eq!(c.send_emphasis(&key), SendEmphasis::Filled);
        let sent = c.begin(&key, "k1".into()).unwrap();
        assert_eq!(c.status(&key), ComposerStatus::Sending);
        assert_eq!(c.send_emphasis(&key), SendEmphasis::Progress);
        // Closing comments while a send is in flight never hides the send.
        c.settings.comments_open = false;
        assert_eq!(c.status(&key), ComposerStatus::Sending);
        c.settings.comments_open = true;
        assert!(c.complete(&sent, Err(ApiError::Network)));
        assert_eq!(c.status(&key), ComposerStatus::Uncertain);
        assert_eq!(c.draft(&key).receipt.unwrap().tone(), NoticeTone::Unknown);
        let replay = c.reconcile(&key).unwrap();
        assert!(c.complete(&replay, ack()));
        assert_eq!(c.status(&key), ComposerStatus::CommittedPending);
        assert_eq!(c.draft(&key).receipt.unwrap().tone(), NoticeTone::Waiting);
        // A new draft after the pending receipt keeps the receipt until the next send.
        c.edit(&key, "Ý mới".into(), false);
        assert_eq!(c.status(&key), ComposerStatus::CommittedPending);
        let next = c.begin(&key, "k2".into()).unwrap();
        assert_eq!(c.draft(&key).receipt, None);
        assert!(c.complete(&next, Err(ApiError::Invalid { field: None })));
        assert_eq!(c.status(&key), ComposerStatus::Refused);
        assert_eq!(c.draft(&key).body, "Ý mới", "a refusal keeps the text");
        assert_eq!(c.draft(&key).receipt.unwrap().tone(), NoticeTone::Adverse);
        // The work's policy outranks who is writing and the last receipt.
        c.settings.comments_open = false;
        assert_eq!(c.status(&key), ComposerStatus::Closed);
        c.settings.comments_open = true;
        c.scope.session = None;
        assert_eq!(c.status(&key), ComposerStatus::SignInClosed);
        c.sign_in_open = true;
        assert_eq!(c.status(&key), ComposerStatus::SignInAvailable);
        assert!(c.reason(&key).is_some(), "a guest never sends");
    }

    #[test]
    fn r4_a_peer_composer_blocked_by_an_unconfirmed_session_stays_a_draft_with_a_reason() {
        let mut c = controller();
        let (root, reply) = (ComposerKey::Root, ComposerKey::Replies("root".into()));
        c.edit(&root, "Gốc".into(), false);
        c.reply(
            "root".into(),
            Target {
                id: "root".into(),
                label: "Tác giả".into(),
            },
        );
        c.edit(&reply, "Trả lời".into(), false);
        let sent = c.begin(&root, "k".into()).unwrap();
        c.complete(&sent, Err(ApiError::Unauthenticated));
        assert_eq!(c.status(&root), ComposerStatus::Refused);
        assert_eq!(c.status(&reply), ComposerStatus::Draft);
        assert!(c.reason(&reply).unwrap().contains("phiên đăng nhập"));
    }

    #[test]
    fn r4_reply_target_chip_returns_to_the_root_and_never_infers_deletion() {
        let mut c = controller();
        let key = ComposerKey::Replies("root".into());
        assert_eq!(c.reply_target(&ComposerKey::Root), None);
        c.reply(
            "root".into(),
            Target {
                id: "root".into(),
                label: "Tác giả gốc".into(),
            },
        );
        assert_eq!(c.reply_target(&key), Some(ReplyTarget::Root));
        c.edit(&key, "Đang viết dở".into(), false);
        c.reply(
            "root".into(),
            Target {
                id: "reply-b".into(),
                label: "Bạn đọc B".into(),
            },
        );
        assert_eq!(
            c.reply_target(&key),
            Some(ReplyTarget::Reply {
                name: "Bạn đọc B".into()
            })
        );
        let late = c.begin(&key, "to-b".into()).unwrap();
        // × back to the root keeps the text; the late ACK of the earlier target cannot clear it.
        c.reply(
            "root".into(),
            Target {
                id: "root".into(),
                label: "Tác giả gốc".into(),
            },
        );
        assert_eq!(c.reply_target(&key), Some(ReplyTarget::Root));
        assert!(c.complete(&late, ack()));
        assert_eq!(c.draft(&key).body, "Đang viết dở");
        c.revalidate_targets(&[]);
        assert_eq!(c.reply_target(&key), Some(ReplyTarget::Unavailable));
        let label = c.draft(&key).target.unwrap().label;
        assert!(
            !label.contains("xóa") && !label.contains("Tác giả gốc"),
            "{label}"
        );
        assert!(c.begin(&key, "blocked".into()).is_none());
    }

    #[test]
    fn r4_collapsing_a_reply_keeps_it_reopenable_with_the_same_target_and_text() {
        let mut c = controller();
        let key = ComposerKey::Replies("root".into());
        c.reopen(&key);
        assert!(
            !c.drafts.contains_key(&key),
            "reopen never invents a composer"
        );
        c.reply(
            "root".into(),
            Target {
                id: "reply-b".into(),
                label: "B".into(),
            },
        );
        assert!(!c.collapsed_with_work(&key));
        c.close_reply(&key);
        assert!(!c.collapsed_with_work(&key), "nothing typed, nothing kept");
        c.reply(
            "root".into(),
            Target {
                id: "reply-b".into(),
                label: "B".into(),
            },
        );
        c.edit(&key, "Chữ đang gõ".into(), true);
        c.close_reply(&key);
        assert!(c.collapsed_with_work(&key));
        assert!(!c.collapsed_with_work(&ComposerKey::Root));
        c.reopen(&key);
        let draft = c.draft(&key);
        assert!(draft.open && draft.body == "Chữ đang gõ" && draft.composing);
        assert_eq!(draft.target.unwrap().id, "reply-b");
        assert!(!c.collapsed_with_work(&key));
    }

    #[test]
    fn r4_closed_wording_follows_the_servers_author_fact_and_resets_with_scope() {
        let mut c = controller();
        let key = ComposerKey::Root;
        c.settings.comments_open = false;
        assert!(c.reason(&key).unwrap().starts_with("Tác giả đã tắt"));
        c.viewer_is_author = true;
        assert!(c.reason(&key).unwrap().starts_with("Bạn đã tắt"));
        c.change_session(Some("actor-a/session-2".into()));
        assert!(c.reason(&key).unwrap().starts_with("Tác giả đã tắt"));
        c.viewer_is_author = true;
        c.change_scope(Scope {
            work_id: "work-b".into(),
            session: Some("actor-a/session-2".into()),
        });
        assert!(!c.viewer_is_author);
    }
}
