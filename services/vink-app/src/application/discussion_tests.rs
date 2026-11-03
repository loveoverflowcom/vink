//! Use-case wiring with a recording store: authentication precedes validation, raw input never
//! reaches storage, IDs are server-chosen and every store decision keeps a distinct outcome.
//! The store's own rules are covered by domain tests and the real PostgreSQL suite, not here.

use std::sync::Mutex;

use time::macros::datetime;
use uuid::Uuid;

use super::Vink;
use super::discussion::*;
use super::ports::{DiscussionStore, StoreError};
use crate::domain::discussion::body::{BodyError, CommentBody};
use crate::domain::discussion::lifecycle::{
    CommentState, LifecycleError, ModerationVersion, Review, Transition,
};
use crate::domain::discussion::page::{PageCursor, PageError, PageLimit};
use crate::domain::discussion::policy::Denied;
use crate::domain::discussion::settings::{
    CommentsMode, DiscussionSettings, ModeChange, SettingsError, SettingsGeneration,
};
use crate::domain::discussion::submission::PayloadDigest;
use crate::domain::ids::{CommentId, PrincipalId, WorkId};

#[derive(Debug, Clone, PartialEq, Eq)]
enum Call {
    Submit(PrincipalId, NewComment),
    Remove(PrincipalId, WorkId, CommentId),
    Mode(PrincipalId, WorkId, SettingsGeneration, CommentsMode),
    Moderate(PrincipalId, CommentId, ModerationVersion, ModerationAction),
    Roots(WorkId, Option<PrincipalId>, Option<PageCursor>, PageLimit),
}

#[derive(Default)]
struct Script {
    calls: Mutex<Vec<Call>>,
    submit: Option<SubmitOutcome>,
    removal: Option<Guarded<Transition, LifecycleError>>,
    mode: Option<Guarded<ModeChange, SettingsError>>,
    moderation: Option<Guarded<Transition, LifecycleError>>,
    unavailable: bool,
}

impl Script {
    fn record<T>(&self, call: Call, answer: Option<T>) -> Result<T, StoreError> {
        self.calls.lock().unwrap().push(call);
        if self.unavailable {
            return Err(StoreError::Unavailable("down".into()));
        }
        Ok(answer.expect("this use case must not reach the store"))
    }
}

impl DiscussionStore for Script {
    async fn submit_comment(
        &self,
        actor: PrincipalId,
        comment: &NewComment,
    ) -> Result<SubmitOutcome, StoreError> {
        self.record(Call::Submit(actor, comment.clone()), self.submit)
    }

    async fn remove_own_comment(
        &self,
        actor: PrincipalId,
        work_id: WorkId,
        comment_id: CommentId,
    ) -> Result<Guarded<Transition, LifecycleError>, StoreError> {
        self.record(Call::Remove(actor, work_id, comment_id), self.removal)
    }

    async fn set_comments_mode(
        &self,
        actor: PrincipalId,
        work_id: WorkId,
        expected: SettingsGeneration,
        mode: CommentsMode,
    ) -> Result<Guarded<ModeChange, SettingsError>, StoreError> {
        self.record(Call::Mode(actor, work_id, expected, mode), self.mode)
    }

    async fn moderate_comment(
        &self,
        moderator: PrincipalId,
        comment_id: CommentId,
        expected: ModerationVersion,
        action: ModerationAction,
    ) -> Result<Guarded<Transition, LifecycleError>, StoreError> {
        self.record(
            Call::Moderate(moderator, comment_id, expected, action),
            self.moderation,
        )
    }

    async fn root_page(
        &self,
        work_id: WorkId,
        viewer: Option<PrincipalId>,
        after: Option<PageCursor>,
        limit: PageLimit,
    ) -> Result<Option<RootPage>, StoreError> {
        self.record(Call::Roots(work_id, viewer, after, limit), Some(None))
    }

    async fn reply_page(
        &self,
        _: WorkId,
        _: CommentId,
        _: Option<PrincipalId>,
        _: Option<PageCursor>,
        _: PageLimit,
    ) -> Result<Option<ReplyPage>, StoreError> {
        panic!("not used")
    }
}

fn actor() -> PrincipalId {
    PrincipalId::from_uuid(Uuid::from_u128(1))
}

fn work() -> WorkId {
    WorkId::from_uuid(Uuid::from_u128(2))
}

fn comment() -> CommentId {
    CommentId::from_uuid(Uuid::from_u128(3))
}

fn submission() -> SubmitComment {
    SubmitComment {
        work_id: work(),
        target: Some(comment()),
        body: "  Ca\u{0300} phê  ".into(),
        command_key: "01J9-retry_key".into(),
    }
}

fn app(script: Script) -> Vink<Script> {
    Vink::new(script)
}

fn calls(vink: &Vink<Script>) -> Vec<Call> {
    vink.store().calls.lock().unwrap().clone()
}

fn version(raw: i64) -> ModerationVersion {
    ModerationVersion::new(raw).unwrap()
}

#[tokio::test]
async fn a_submission_reaches_the_store_validated_with_server_chosen_identities() {
    let vink = app(Script {
        submit: Some(SubmitOutcome::Created {
            comment_id: comment(),
            created_at: datetime!(2026-11-03 10:00 UTC),
        }),
        ..Script::default()
    });
    let submitted = vink.submit_comment(Some(actor()), submission()).await;
    assert_eq!(
        submitted,
        Ok(SubmittedComment {
            comment_id: comment(),
            status: OwnStatus::Pending,
            replayed: false,
        })
    );
    let recorded: [Call; 1] = calls(&vink).try_into().unwrap();
    let [Call::Submit(who, new)] = recorded else {
        panic!("one submission expected")
    };
    assert_eq!(who, actor());
    assert_eq!(new.body.as_str(), "Cà phê");
    assert_eq!(new.target, Some(comment()));
    assert_eq!(new.key.as_str(), "01J9-retry_key");
    let body = CommentBody::parse("Cà phê").unwrap();
    assert_eq!(
        new.digest,
        PayloadDigest::of_submission(work(), Some(comment()), &body)
    );
    assert_eq!(new.comment_id.as_uuid().get_version_num(), 7);
    assert_ne!(new.comment_id, comment(), "the client never chooses an ID");
}

#[tokio::test]
async fn authentication_precedes_validation_and_no_raw_input_reaches_storage() {
    let vink = app(Script::default());
    let invalid = SubmitComment {
        body: String::new(),
        command_key: "x".into(),
        ..submission()
    };
    assert_eq!(
        vink.submit_comment(None, invalid.clone()).await,
        Err(DiscussionError::Unauthenticated)
    );
    assert_eq!(
        vink.remove_own_comment(None, work(), comment()).await,
        Err(DiscussionError::Unauthenticated)
    );
    let mode = SetCommentsMode {
        work_id: work(),
        expected_generation: 0,
        mode: CommentsMode::Closed,
    };
    assert_eq!(
        vink.set_comments_mode(None, mode).await,
        Err(DiscussionError::Unauthenticated)
    );

    let key_error = vink
        .submit_comment(Some(actor()), invalid)
        .await
        .unwrap_err();
    let DiscussionError::Invalid(input) = key_error else {
        panic!("{key_error:?}")
    };
    assert_eq!(input.field(), "command_key");
    let body_error = SubmitComment {
        body: "a\u{202E}b".into(),
        ..submission()
    };
    assert_eq!(
        vink.submit_comment(Some(actor()), body_error).await,
        Err(DiscussionError::Invalid(DiscussionInput::Body(
            BodyError::BidiControl
        )))
    );
    assert_eq!(
        vink.set_comments_mode(Some(actor()), mode).await,
        Err(DiscussionError::Invalid(DiscussionInput::ExpectedVersion))
    );
    assert!(calls(&vink).is_empty());
}

#[tokio::test]
async fn submission_outcomes_stay_distinct() {
    let approved = CommentState::restore(Review::Approved, None, version(2));
    for (outcome, expected) in [
        (
            SubmitOutcome::Replayed {
                comment_id: comment(),
                state: approved,
            },
            Ok(SubmittedComment {
                comment_id: comment(),
                status: OwnStatus::Approved,
                replayed: true,
            }),
        ),
        (SubmitOutcome::KeyReused, Err(DiscussionError::KeyReused)),
        (SubmitOutcome::WorkNotFound, Err(DiscussionError::NotFound)),
        (
            SubmitOutcome::CommentsClosed,
            Err(DiscussionError::CommentsClosed),
        ),
        (
            SubmitOutcome::TargetUnavailable,
            Err(DiscussionError::TargetUnavailable),
        ),
    ] {
        let vink = app(Script {
            submit: Some(outcome),
            ..Script::default()
        });
        assert_eq!(
            vink.submit_comment(Some(actor()), submission()).await,
            expected
        );
    }
    let down = app(Script {
        unavailable: true,
        ..Script::default()
    });
    assert_eq!(
        down.submit_comment(Some(actor()), submission()).await,
        Err(DiscussionError::Unavailable)
    );
}

#[tokio::test]
async fn work_ownership_grants_no_power_over_other_peoples_comments() {
    for (guarded, expected) in [
        (
            Guarded::Denied(Denied::NotCommentWriter),
            Err(DiscussionError::NotCommentWriter),
        ),
        (
            Guarded::Denied(Denied::Hidden),
            Err(DiscussionError::NotFound),
        ),
        (Guarded::NotFound, Err(DiscussionError::NotFound)),
        (
            Guarded::Decided(Ok(Transition::Unchanged(CommentState::restore(
                Review::Pending,
                Some(crate::domain::discussion::lifecycle::Removal::ByAuthor),
                version(2),
            )))),
            Ok(OwnStatus::Removed),
        ),
    ] {
        let vink = app(Script {
            removal: Some(guarded),
            ..Script::default()
        });
        assert_eq!(
            vink.remove_own_comment(Some(actor()), work(), comment())
                .await,
            expected
        );
    }
    // No grant, no moderation: the work's author has no path to review comments.
    let vink = app(Script::default());
    let command = ModerateComment {
        comment_id: comment(),
        expected_version: 1,
        action: ModerationAction::Remove,
    };
    assert_eq!(
        vink.moderate_comment(None, command).await,
        Err(DiscussionError::NotModerator)
    );
    assert!(calls(&vink).is_empty());
}

#[tokio::test]
async fn mode_and_moderation_conflicts_report_the_current_version() {
    let current =
        DiscussionSettings::restore(CommentsMode::Open, SettingsGeneration::new(3).unwrap());
    let vink = app(Script {
        mode: Some(Guarded::Decided(Err(SettingsError::Stale { current }))),
        ..Script::default()
    });
    let command = SetCommentsMode {
        work_id: work(),
        expected_generation: 1,
        mode: CommentsMode::Closed,
    };
    assert_eq!(
        vink.set_comments_mode(Some(actor()), command).await,
        Err(DiscussionError::Conflict { current: 3 })
    );
    let not_author = app(Script {
        mode: Some(Guarded::Denied(Denied::NotWorkAuthor)),
        ..Script::default()
    });
    assert_eq!(
        not_author.set_comments_mode(Some(actor()), command).await,
        Err(DiscussionError::NotFound)
    );

    let grant = ModeratorGrant::from_authority(PrincipalId::from_uuid(Uuid::from_u128(9)));
    let moderate = ModerateComment {
        comment_id: comment(),
        expected_version: 1,
        action: ModerationAction::Approve,
    };
    for (decided, expected) in [
        (
            Err(LifecycleError::Stale {
                current: version(4),
            }),
            Err(DiscussionError::Conflict { current: 4 }),
        ),
        (Err(LifecycleError::Removed), Err(DiscussionError::Removed)),
    ] {
        let vink = app(Script {
            moderation: Some(Guarded::Decided(decided)),
            ..Script::default()
        });
        assert_eq!(vink.moderate_comment(Some(grant), moderate).await, expected);
        assert_eq!(
            calls(&vink),
            vec![Call::Moderate(
                grant.moderator(),
                comment(),
                version(1),
                ModerationAction::Approve
            )]
        );
    }
}

#[tokio::test]
async fn paging_input_is_validated_before_reading() {
    let vink = app(Script::default());
    for (after, limit, field) in [(Some("c9.x"), None, "cursor"), (None, Some(0), "limit")] {
        let error = vink
            .discussion(None, work(), after, limit)
            .await
            .unwrap_err();
        let DiscussionError::Invalid(input) = error else {
            panic!("{error:?}")
        };
        assert_eq!(input.field(), field);
        assert!(matches!(
            input,
            DiscussionInput::Page(PageError::Cursor | PageError::Limit)
        ));
    }
    assert!(calls(&vink).is_empty());
    // A work that is not readable is NotFound, not an empty discussion.
    assert_eq!(
        vink.discussion(Some(actor()), work(), None, None).await,
        Err(DiscussionError::NotFound)
    );
    assert_eq!(
        calls(&vink),
        vec![Call::Roots(work(), Some(actor()), None, PageLimit::DEFAULT)]
    );
}
