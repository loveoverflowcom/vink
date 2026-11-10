//! Use-case wiring with a recording store: authentication precedes validation, raw input never
//! reaches storage, IDs are server-chosen and every store decision keeps a distinct outcome.
//! The store's own rules are covered by domain tests and the real PostgreSQL suite, not here.

use std::sync::Mutex;

use time::macros::datetime;
use uuid::Uuid;

use super::Vink;
use super::discussion::*;
use super::ports::{DiscussionStore, StoreError};
use crate::domain::discussion::admission::{PolicySelection, Refusal};
use crate::domain::discussion::body::{BodyError, CommentBody};
use crate::domain::discussion::lifecycle::{
    CommentState, LifecycleError, ModerationVersion, Review, Transition,
};
use crate::domain::discussion::moderation::{ModerationReason, ReasonError};
use crate::domain::discussion::page::{CursorScope, PageCursor, PageError, PageLimit};
use crate::domain::discussion::policy::Denied;
use crate::domain::discussion::settings::{
    CommentsMode, DiscussionSettings, ModeChange, SettingsError, SettingsGeneration,
};
use crate::domain::discussion::submission::PayloadDigest;
use crate::domain::discussion::visibility::{CommentView, ReplyContext};
use crate::domain::ids::{CommentId, PrincipalId, WorkId};

#[derive(Debug, Clone, PartialEq, Eq)]
enum Call {
    Submit(PrincipalId, NewComment),
    Remove(PrincipalId, WorkId, CommentId),
    Mode(PrincipalId, WorkId, SettingsGeneration, CommentsMode),
    Moderate(
        PrincipalId,
        CommentId,
        ModerationVersion,
        ModerationAction,
        Option<String>,
    ),
    Queue(PrincipalId),
    Roots(WorkId, Option<PrincipalId>, Option<PageCursor>, PageLimit),
    Own(WorkId, PrincipalId, OwnRead),
}

#[derive(Default)]
struct Script {
    calls: Mutex<Vec<Call>>,
    submit: Option<SubmitOutcome>,
    removal: Option<Guarded<Transition, LifecycleError>>,
    mode: Option<Guarded<ModeChange, SettingsError>>,
    moderation: Option<Guarded<Transition, LifecycleError>>,
    /// What `root_page` returns; `None` reads as a work that is not readable.
    roots: Option<RootPage>,
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
        actor: PrincipalId,
        comment_id: CommentId,
        expected: ModerationVersion,
        action: ModerationAction,
        reason: Option<&ModerationReason>,
    ) -> Result<Guarded<Transition, LifecycleError>, StoreError> {
        let reason = reason.map(|reason| reason.as_str().to_string());
        self.record(
            Call::Moderate(actor, comment_id, expected, action, reason),
            self.moderation,
        )
    }

    async fn moderation_queue(
        &self,
        actor: PrincipalId,
        _: Option<PageCursor>,
        _: PageLimit,
    ) -> Result<Option<ModerationQueue>, StoreError> {
        // `None`: the store found no active grant for this actor.
        self.record(Call::Queue(actor), Some(None))
    }

    async fn root_page(
        &self,
        work_id: WorkId,
        viewer: Option<PrincipalId>,
        after: Option<PageCursor>,
        limit: PageLimit,
    ) -> Result<Option<RootPage>, StoreError> {
        self.record(
            Call::Roots(work_id, viewer, after, limit),
            Some(self.roots.clone()),
        )
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

    async fn own_comments(
        &self,
        work_id: WorkId,
        viewer: PrincipalId,
        read: &OwnRead,
    ) -> Result<Option<OwnPage>, StoreError> {
        self.record(
            Call::Own(work_id, viewer, read.clone()),
            Some(Some(OwnPage {
                items: Vec::new(),
                next: None,
            })),
        )
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
            state: CommentState::SUBMITTED,
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
    assert_eq!(new.selection, PolicySelection::PREMODERATION);
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
        // The status is the stored state, never a constant: a store that wrote another initial
        // state (a future policy) reports it unchanged.
        (
            SubmitOutcome::Created {
                comment_id: comment(),
                created_at: datetime!(2026-11-03 10:00 UTC),
                state: CommentState::restore(Review::Approved, None, version(1)),
            },
            Ok(SubmittedComment {
                comment_id: comment(),
                status: OwnStatus::Approved,
                replayed: false,
            }),
        ),
        (
            SubmitOutcome::Refused(Refusal::Suspended),
            Err(DiscussionError::Suspended),
        ),
        (
            SubmitOutcome::Refused(Refusal::StandingUnknown),
            Err(DiscussionError::Unauthenticated),
        ),
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
    // Moderation needs an actor and a reason before the store is asked anything; the store then
    // answers whether this actor holds a grant or must recuse.
    let vink = app(Script::default());
    let remove = ModerateComment {
        comment_id: comment(),
        expected_version: 1,
        action: ModerationAction::Remove,
        reason: Some("Spam".into()),
    };
    assert_eq!(
        vink.moderate_comment(None, remove.clone()).await,
        Err(DiscussionError::Unauthenticated)
    );
    let unexplained = ModerateComment {
        reason: None,
        ..remove.clone()
    };
    assert_eq!(
        vink.moderate_comment(Some(actor()), unexplained).await,
        Err(DiscussionError::Invalid(DiscussionInput::Reason(
            ReasonError::Required
        )))
    );
    assert!(calls(&vink).is_empty());
    for (denied, expected) in [
        (Denied::NotModerator, DiscussionError::NotModerator),
        (Denied::Recused, DiscussionError::Recused),
    ] {
        let vink = app(Script {
            moderation: Some(Guarded::Denied(denied)),
            ..Script::default()
        });
        assert_eq!(
            vink.moderate_comment(Some(actor()), remove.clone()).await,
            Err(expected)
        );
    }
    assert_eq!(
        vink.moderation_queue(Some(actor()), None, None).await,
        Err(DiscussionError::NotModerator)
    );
    assert_eq!(calls(&vink), vec![Call::Queue(actor())]);
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

    let moderator = PrincipalId::from_uuid(Uuid::from_u128(9));
    let moderate = ModerateComment {
        comment_id: comment(),
        expected_version: 1,
        action: ModerationAction::Approve,
        reason: None,
    };
    for (decided, expected) in [
        (
            Err(LifecycleError::Stale {
                current: version(4),
            }),
            Err(DiscussionError::ModerationConflict { current: 4 }),
        ),
        (Err(LifecycleError::Removed), Err(DiscussionError::Removed)),
    ] {
        let vink = app(Script {
            moderation: Some(Guarded::Decided(decided)),
            ..Script::default()
        });
        assert_eq!(
            vink.moderate_comment(Some(moderator), moderate.clone())
                .await,
            expected
        );
        assert_eq!(
            calls(&vink),
            vec![Call::Moderate(
                moderator,
                comment(),
                version(1),
                ModerationAction::Approve,
                None
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

fn page_with(view: CommentView, viewer_is_work_author: bool) -> RootPage {
    RootPage {
        settings: DiscussionSettings::INITIAL,
        viewer_is_work_author,
        roots: vec![RootItem {
            entry: RootEntry::Comment(CommentItem {
                id: comment(),
                view,
                author: AuthorName {
                    id: actor(),
                    display_name: "Bạn đọc".into(),
                },
                body: "Câu thơ".into(),
                created_at: datetime!(2026-11-03 10:00 UTC),
                context: ReplyContext::Root,
                context_author: None,
                rejection_reason: None,
            }),
            public_replies: 0,
            preview: Vec::new(),
            preview_next: None,
        }],
        next: None,
    }
}

#[tokio::test]
async fn the_public_projection_reads_without_an_actor_and_fails_closed_on_viewer_state() {
    let vink = app(Script {
        roots: Some(page_with(CommentView::Public, false)),
        ..Script::default()
    });
    let public = vink.public_discussion(work(), None, None).await.unwrap();
    assert_eq!(public.page().roots.len(), 1);
    assert_eq!(
        calls(&vink),
        vec![Call::Roots(work(), None, None, PageLimit::DEFAULT)]
    );
    // Negative controls: if a store ever answered an anonymous read with private facts, the
    // public type refuses to exist rather than serialize them.
    for page in [
        page_with(CommentView::OwnPending, false),
        page_with(CommentView::OwnRejected, false),
        page_with(CommentView::Public, true),
    ] {
        let vink = app(Script {
            roots: Some(page),
            ..Script::default()
        });
        assert_eq!(
            vink.public_discussion(work(), None, None).await,
            Err(DiscussionError::Internal)
        );
    }
    let missing = app(Script::default());
    assert_eq!(
        missing.public_discussion(work(), None, None).await,
        Err(DiscussionError::NotFound)
    );
}

#[tokio::test]
async fn the_overlay_needs_an_actor_first_and_is_scoped_to_it() {
    let vink = app(Script::default());
    let invalid = OwnCommentsQuery {
        ids: Some("not-an-id".into()),
        ..OwnCommentsQuery::default()
    };
    assert_eq!(
        vink.own_comments(None, work(), &invalid).await,
        Err(DiscussionError::Unauthenticated)
    );
    assert!(calls(&vink).is_empty());
    let page = vink
        .own_comments(Some(actor()), work(), &OwnCommentsQuery::default())
        .await
        .unwrap();
    assert!(page.items.is_empty());
    assert_eq!(
        calls(&vink),
        vec![Call::Own(
            work(),
            actor(),
            OwnRead::Unpublished {
                after: None,
                limit: PageLimit::DEFAULT
            }
        )]
    );
}

#[test]
fn overlay_queries_are_bounded_and_scoped() {
    let id = |n: u128| CommentId::from_uuid(Uuid::from_u128(n));
    let ids = |raw: &str| OwnCommentsQuery {
        ids: Some(raw.into()),
        ..OwnCommentsQuery::default()
    };
    let position = PageCursor::after(datetime!(2026-11-03 10:00 UTC), id(5));
    let own = OwnCommentsQuery {
        cursor: Some(position.encode_in(CursorScope::Own)),
        limit: Some(50),
        ids: None,
    };
    assert_eq!(
        OwnRead::parse(&own),
        Ok(OwnRead::Unpublished {
            after: Some(position),
            limit: PageLimit::new(Some(50)).unwrap()
        })
    );
    // A public list's cursor is not an overlay position.
    let foreign = OwnCommentsQuery {
        cursor: Some(position.encode()),
        ..OwnCommentsQuery::default()
    };
    assert_eq!(
        OwnRead::parse(&foreign),
        Err(DiscussionInput::Page(PageError::Cursor))
    );
    let over = OwnCommentsQuery {
        limit: Some(51),
        ..OwnCommentsQuery::default()
    };
    assert_eq!(
        OwnRead::parse(&over),
        Err(DiscussionInput::Page(PageError::Limit))
    );
    let a = id(1).to_string();
    let b = id(2).to_string();
    assert_eq!(
        OwnRead::parse(&ids(&format!("{a},{b},{a}"))),
        Ok(OwnRead::Exact(vec![id(1), id(2)]))
    );
    let twenty: Vec<String> = (1..=20).map(|n| id(n).to_string()).collect();
    assert!(matches!(
        OwnRead::parse(&ids(&twenty.join(","))),
        Ok(OwnRead::Exact(found)) if found.len() == MAX_OWN_LOOKUP
    ));
    let twenty_one: Vec<String> = (1..=21).map(|n| id(n).to_string()).collect();
    let mixed = OwnCommentsQuery {
        cursor: Some(position.encode_in(CursorScope::Own)),
        ..ids(&a)
    };
    let limited = OwnCommentsQuery {
        limit: Some(5),
        ..ids(&a)
    };
    for bad in [
        ids(""),
        ids(","),
        ids(&format!("{a},")),
        ids(&format!("{a} ")),
        ids("not-an-id"),
        ids(&twenty_one.join(",")),
        ids(&"a".repeat(1 << 16)),
        mixed,
        limited,
    ] {
        assert_eq!(
            OwnRead::parse(&bad),
            Err(DiscussionInput::OwnIds),
            "{bad:?}"
        );
        assert_eq!(DiscussionInput::OwnIds.field(), "ids");
    }
}
