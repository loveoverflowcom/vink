//! Pure Follow UI state machine and presentation helpers (#294).
//!
//! No DOM, browser storage or HTTP I/O runs here; the hydrate adapter translates UI intents into
//! HTTP calls and feeds scoped answers back into [`FollowController`].

use crate::contract::{
    ApiError,
    follow::{
        FollowObserved, FollowReceipt, OwnRelationshipsSummary, PublicRelationships,
        SetFollowRequest, SetFollowResponse, ViewerRelationship, valid_canonical_handle,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FollowScope {
    pub session_gen: u64,
    pub target_handle: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FollowCommand {
    pub command_id: String,
    pub target_handle: String,
    pub expected_order: i64,
    pub following: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FollowNotice {
    pub text: &'static str,
    pub is_error: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FollowInvalidation {
    pub target_handle: String,
    pub following: bool,
    pub accepted_order: i64,
    pub relationships: Option<PublicRelationships>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FollowEffect {
    pub should_focus_heading: bool,
    pub invalidation: Option<FollowInvalidation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FollowPhase {
    Loading,
    Error,
    SelfProfile(OwnRelationshipsSummary),
    Unavailable,
    Gone,
    Ready {
        state: FollowObserved,
        can_follow: bool,
        notice: Option<FollowNotice>,
    },
    Pending {
        shown: FollowObserved,
        command: FollowCommand,
    },
    Reconciling {
        shown: FollowObserved,
        command: FollowCommand,
    },
}

/// Session-scoped, monotonic-order state owner for the `#follow` control on an author profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FollowController {
    scope: FollowScope,
    phase: FollowPhase,
    highest_accepted_order: i64,
}

impl FollowController {
    pub fn new(scope: FollowScope) -> Self {
        Self {
            scope,
            phase: FollowPhase::Loading,
            highest_accepted_order: -1,
        }
    }

    pub fn scope(&self) -> &FollowScope {
        &self.scope
    }

    pub fn phase(&self) -> &FollowPhase {
        &self.phase
    }

    pub fn highest_accepted_order(&self) -> i64 {
        self.highest_accepted_order
    }

    /// Resets all viewer-private state when switching targets or changing session (`A -> B -> A`).
    pub fn reset_scope(&mut self, scope: FollowScope) {
        self.scope = scope;
        self.phase = FollowPhase::Loading;
        self.highest_accepted_order = -1;
    }

    pub fn retry_load(&mut self) {
        if matches!(self.phase, FollowPhase::Error) {
            self.phase = FollowPhase::Loading;
        }
    }

    fn action_currently_visible(&self) -> bool {
        matches!(
            self.phase,
            FollowPhase::Ready {
                can_follow: true,
                ..
            } | FollowPhase::Pending { .. }
                | FollowPhase::Reconciling { .. }
        )
    }

    pub fn finish_load(
        &mut self,
        scope: &FollowScope,
        result: Result<ViewerRelationship, ApiError>,
    ) -> FollowEffect {
        if scope != &self.scope {
            return FollowEffect::default();
        }
        let had_action = self.action_currently_visible();
        match result {
            Ok(ViewerRelationship::SelfProfile {
                target_handle,
                own_relationships,
                ..
            }) => {
                if target_handle != self.scope.target_handle {
                    return FollowEffect::default();
                }
                self.phase = FollowPhase::SelfProfile(own_relationships);
                FollowEffect {
                    should_focus_heading: had_action,
                    invalidation: None,
                }
            }
            Ok(ViewerRelationship::Other {
                target_handle,
                can_follow,
                following,
                mutual,
                pair_version,
                accepted_order,
            }) => {
                if target_handle != self.scope.target_handle
                    || accepted_order < self.highest_accepted_order
                {
                    return FollowEffect::default();
                }
                // Do not let a stale background read clobber an active in-flight write of the same order.
                if matches!(
                    self.phase,
                    FollowPhase::Pending { .. } | FollowPhase::Reconciling { .. }
                ) && accepted_order == self.highest_accepted_order
                {
                    return FollowEffect::default();
                }
                self.highest_accepted_order = accepted_order;
                if !can_follow {
                    self.phase = FollowPhase::Unavailable;
                    return FollowEffect {
                        should_focus_heading: had_action,
                        invalidation: None,
                    };
                }
                self.phase = FollowPhase::Ready {
                    state: FollowObserved {
                        target_handle,
                        following,
                        mutual,
                        pair_version,
                        accepted_order,
                    },
                    can_follow: true,
                    notice: None,
                };
                FollowEffect::default()
            }
            Err(err) => self.apply_terminal_or_transient_error(err, had_action, false),
        }
    }

    /// Starts a Follow/Unfollow command only from `Ready { can_follow: true }`.
    /// Duplicate clicks while `Pending` or `Reconciling` return `None`.
    pub fn begin_intent(
        &mut self,
        command_id: String,
        desired_following: Option<bool>,
    ) -> Option<SetFollowRequest> {
        let state = match &self.phase {
            FollowPhase::Ready {
                state,
                can_follow: true,
                ..
            } => state.clone(),
            _ => return None,
        };
        let following = desired_following.unwrap_or(!state.following);
        let expected_order = state.accepted_order;
        let command = FollowCommand {
            command_id: command_id.clone(),
            target_handle: self.scope.target_handle.clone(),
            expected_order,
            following,
        };
        self.phase = FollowPhase::Pending {
            shown: state,
            command,
        };
        Some(SetFollowRequest {
            command_id,
            expected_order,
            following,
        })
    }

    pub fn finish_write(
        &mut self,
        scope: &FollowScope,
        command_id: &str,
        result: Result<SetFollowResponse, ApiError>,
    ) -> FollowEffect {
        if scope != &self.scope {
            return FollowEffect::default();
        }
        let had_action = self.action_currently_visible();
        match result {
            Ok(resp) => {
                if resp.target_handle != self.scope.target_handle {
                    return FollowEffect::default();
                }
                let matches_inflight = matches!(
                    &self.phase,
                    FollowPhase::Pending { command, .. } | FollowPhase::Reconciling { command, .. }
                        if command.command_id == command_id
                );
                // Reject stale ACKs that arrive after a newer accepted order has already settled.
                if resp.accepted_order < self.highest_accepted_order
                    || (resp.accepted_order == self.highest_accepted_order && !matches_inflight)
                {
                    return FollowEffect::default();
                }
                let changed = resp.accepted_order > self.highest_accepted_order;
                self.highest_accepted_order = resp.accepted_order;
                let observed = FollowObserved {
                    target_handle: resp.target_handle.clone(),
                    following: resp.following,
                    mutual: resp.mutual,
                    pair_version: resp.pair_version,
                    accepted_order: resp.accepted_order,
                };
                let notice = (resp.outcome == "replayed").then_some(FollowNotice {
                    text: "Đã xác nhận trạng thái theo dõi hiện tại.",
                    is_error: false,
                });
                self.phase = FollowPhase::Ready {
                    state: observed,
                    can_follow: true,
                    notice,
                };
                let invalidation = changed.then(|| FollowInvalidation {
                    target_handle: resp.target_handle,
                    following: resp.following,
                    accepted_order: resp.accepted_order,
                    relationships: (!resp.relationships.is_empty()).then_some(resp.relationships),
                });
                FollowEffect {
                    should_focus_heading: false,
                    invalidation,
                }
            }
            Err(ApiError::FollowConflict { observed }) => {
                if observed.target_handle != self.scope.target_handle
                    || observed.accepted_order < self.highest_accepted_order
                {
                    return FollowEffect::default();
                }
                if observed.accepted_order == self.highest_accepted_order
                    && !matches!(&self.phase, FollowPhase::Pending { command, .. } | FollowPhase::Reconciling { command, .. } if command.command_id == command_id)
                {
                    return FollowEffect::default();
                }
                let changed = observed.accepted_order > self.highest_accepted_order;
                self.highest_accepted_order = observed.accepted_order;
                let invalidation = changed.then(|| FollowInvalidation {
                    target_handle: observed.target_handle.clone(),
                    following: observed.following,
                    accepted_order: observed.accepted_order,
                    relationships: None,
                });
                self.phase = FollowPhase::Ready {
                    state: observed,
                    can_follow: true,
                    notice: Some(FollowNotice {
                        text: "Trạng thái theo dõi vừa thay đổi; đã đồng bộ lại.",
                        is_error: false,
                    }),
                };
                FollowEffect {
                    should_focus_heading: false,
                    invalidation,
                }
            }
            Err(err) => {
                if !matches!(
                    &self.phase,
                    FollowPhase::Pending { command, .. } | FollowPhase::Reconciling { command, .. }
                        if command.command_id == command_id
                ) {
                    return FollowEffect::default();
                }
                self.apply_terminal_or_transient_error(err, had_action, true)
            }
        }
    }

    pub fn finish_reconcile(
        &mut self,
        scope: &FollowScope,
        command_id: &str,
        receipt: Option<Result<FollowReceipt, ApiError>>,
        viewer: Result<ViewerRelationship, ApiError>,
    ) -> FollowEffect {
        if scope != &self.scope {
            return FollowEffect::default();
        }
        let Some((shown, command)) = (match &self.phase {
            FollowPhase::Pending { shown, command }
            | FollowPhase::Reconciling { shown, command }
                if command.command_id == command_id =>
            {
                Some((shown.clone(), command.clone()))
            }
            _ => None,
        }) else {
            return FollowEffect::default();
        };

        let mut best: Option<FollowObserved> = None;
        if let Some(Ok(r)) = receipt
            && r.command_id == command_id
            && r.observed.target_handle == self.scope.target_handle
        {
            best = Some(r.observed);
        }

        let had_action = self.action_currently_visible();
        match viewer {
            Ok(ViewerRelationship::SelfProfile {
                target_handle,
                own_relationships,
                ..
            }) if target_handle == self.scope.target_handle => {
                self.phase = FollowPhase::SelfProfile(own_relationships);
                return FollowEffect {
                    should_focus_heading: had_action,
                    invalidation: None,
                };
            }
            Ok(ViewerRelationship::Other {
                target_handle,
                can_follow,
                following,
                mutual,
                pair_version,
                accepted_order,
            }) if target_handle == self.scope.target_handle => {
                if !can_follow {
                    self.highest_accepted_order = self.highest_accepted_order.max(accepted_order);
                    self.phase = FollowPhase::Unavailable;
                    return FollowEffect {
                        should_focus_heading: had_action,
                        invalidation: None,
                    };
                }
                let candidate = FollowObserved {
                    target_handle,
                    following,
                    mutual,
                    pair_version,
                    accepted_order,
                };
                if best
                    .as_ref()
                    .is_none_or(|b| candidate.accepted_order >= b.accepted_order)
                {
                    best = Some(candidate);
                }
            }
            Ok(ViewerRelationship::Other { .. }) | Ok(ViewerRelationship::SelfProfile { .. }) => {}
            Err(
                err @ (ApiError::Unauthenticated
                | ApiError::Forbidden
                | ApiError::NotFound
                | ApiError::ProfileGone
                | ApiError::FollowActorUnavailable
                | ApiError::TargetUnavailable),
            ) => {
                return self.apply_terminal_or_transient_error(err, had_action, false);
            }
            Err(_) => {}
        }

        if let Some(observed) = best {
            if observed.accepted_order < self.highest_accepted_order {
                return FollowEffect::default();
            }
            let committed_during_lost_ack = observed.accepted_order > self.highest_accepted_order;
            self.highest_accepted_order = observed.accepted_order;
            let notice = if committed_during_lost_ack {
                Some(FollowNotice {
                    text: "Đã đối chiếu trạng thái mới nhất từ máy chủ.",
                    is_error: false,
                })
            } else {
                Some(FollowNotice {
                    text: "Thao tác trước chưa được ghi nhận; bạn có thể thử lại.",
                    is_error: true,
                })
            };
            let invalidation = committed_during_lost_ack.then(|| FollowInvalidation {
                target_handle: observed.target_handle.clone(),
                following: observed.following,
                accepted_order: observed.accepted_order,
                relationships: None,
            });
            self.phase = FollowPhase::Ready {
                state: observed,
                can_follow: true,
                notice,
            };
            return FollowEffect {
                should_focus_heading: false,
                invalidation,
            };
        }

        self.phase = FollowPhase::Reconciling { shown, command };
        FollowEffect::default()
    }

    fn apply_terminal_or_transient_error(
        &mut self,
        err: ApiError,
        had_action: bool,
        from_write: bool,
    ) -> FollowEffect {
        match err {
            ApiError::ProfileGone => {
                self.phase = FollowPhase::Gone;
                FollowEffect {
                    should_focus_heading: had_action,
                    invalidation: None,
                }
            }
            ApiError::Unauthenticated
            | ApiError::Forbidden
            | ApiError::NotFound
            | ApiError::FollowActorUnavailable
            | ApiError::TargetUnavailable => {
                self.phase = FollowPhase::Unavailable;
                FollowEffect {
                    should_focus_heading: had_action,
                    invalidation: None,
                }
            }
            ApiError::SelfFollow | ApiError::FollowKeyReused | ApiError::Invalid { .. }
                if from_write =>
            {
                if let FollowPhase::Pending { shown, .. } | FollowPhase::Reconciling { shown, .. } =
                    &self.phase
                {
                    self.phase = FollowPhase::Ready {
                        state: shown.clone(),
                        can_follow: true,
                        notice: Some(FollowNotice {
                            text: "Không thể cập nhật trạng thái theo dõi lúc này.",
                            is_error: true,
                        }),
                    };
                } else {
                    self.phase = FollowPhase::Error;
                }
                FollowEffect::default()
            }
            _ => {
                if from_write {
                    if let FollowPhase::Pending { shown, command }
                    | FollowPhase::Reconciling { shown, command } = &self.phase
                    {
                        self.phase = FollowPhase::Reconciling {
                            shown: shown.clone(),
                            command: command.clone(),
                        };
                    } else {
                        self.phase = FollowPhase::Error;
                    }
                } else if !matches!(self.phase, FollowPhase::Reconciling { .. }) {
                    self.phase = FollowPhase::Error;
                }
                FollowEffect::default()
            }
        }
    }
}

/// Allows returning ONLY to internal canonical `/@{handle}` or `/@{handle}/(nguoi-theo-doi|dang-theo-doi)`.
pub fn safe_profile_return(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    let rest = trimmed.strip_prefix("/@")?;
    if rest.contains('\\') || rest.contains('?') || rest.contains('#') {
        return None;
    }
    match rest.split_once('/') {
        None => valid_canonical_handle(rest).then(|| format!("/@{rest}")),
        Some((handle, segment)) => {
            if segment.contains('/') {
                return None;
            }
            let valid_seg = segment == "nguoi-theo-doi" || segment == "dang-theo-doi";
            (valid_seg && valid_canonical_handle(handle)).then(|| format!("/@{handle}/{segment}"))
        }
    }
}

/// Formats a non-negative integer using Vietnamese thousands separators (`1.234`).
pub fn format_vi_count(count: i64) -> String {
    let negative = count < 0;
    let digits = count.unsigned_abs().to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3 + usize::from(negative));
    if negative {
        out.push('-');
    }
    for (idx, ch) in digits.chars().enumerate() {
        let remaining = digits.len() - idx;
        if idx > 0 && remaining.is_multiple_of(3) {
            out.push('.');
        }
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::follow::{FollowDirection, FollowToken};

    fn scope(generation: u64, handle: &str) -> FollowScope {
        FollowScope {
            session_gen: generation,
            target_handle: handle.into(),
        }
    }

    fn viewer_other(
        handle: &str,
        following: bool,
        mutual: bool,
        version: i64,
        order: i64,
    ) -> ViewerRelationship {
        ViewerRelationship::Other {
            target_handle: handle.into(),
            can_follow: true,
            following,
            mutual,
            pair_version: version,
            accepted_order: order,
        }
    }

    #[test]
    fn aba_lost_ack_reconcile_unfollow_and_stale_v1_ack_is_ignored() {
        let s = scope(1, "an-may");
        let mut ctrl = FollowController::new(s.clone());
        ctrl.finish_load(&s, Ok(viewer_other("an-may", false, false, 0, 0)));

        // Duplicate press while Pending is rejected.
        let req1 = ctrl
            .begin_intent("cmd-1".into(), None)
            .expect("starts cmd-1");
        assert_eq!(req1.expected_order, 0);
        assert!(req1.following);
        assert!(ctrl.begin_intent("cmd-dup".into(), None).is_none());

        // Simulate lost ACK on cmd-1 -> enters Reconciling with zero invalidation yet.
        let lost = ctrl.finish_write(&s, "cmd-1", Err(ApiError::Network));
        assert!(lost.invalidation.is_none());
        assert!(matches!(ctrl.phase(), FollowPhase::Reconciling { .. }));

        // Server actually committed cmd-1 at order 1. Reconciliation observes order 1 and invalidates once.
        let reconciled = ctrl.finish_reconcile(
            &s,
            "cmd-1",
            Some(Ok(FollowReceipt {
                command_id: "cmd-1".into(),
                target_handle: "an-may".into(),
                expected_order: 0,
                desired_following: true,
                committed: FollowToken {
                    following: true,
                    pair_version: 1,
                    accepted_order: 1,
                },
                observed: FollowObserved {
                    target_handle: "an-may".into(),
                    following: true,
                    mutual: false,
                    pair_version: 1,
                    accepted_order: 1,
                },
            })),
            Ok(viewer_other("an-may", true, false, 1, 1)),
        );
        assert_eq!(
            reconciled
                .invalidation
                .as_ref()
                .map(|i| (i.following, i.accepted_order)),
            Some((true, 1))
        );

        // User now clicks Unfollow (cmd-2 from expected_order 1) and receives ACK order 2.
        let req2 = ctrl
            .begin_intent("cmd-2".into(), None)
            .expect("starts cmd-2");
        assert_eq!(req2.expected_order, 1);
        assert!(!req2.following);
        let eff2 = ctrl.finish_write(
            &s,
            "cmd-2",
            Ok(SetFollowResponse {
                target_handle: "an-may".into(),
                outcome: "accepted".into(),
                following: false,
                mutual: false,
                pair_version: 2,
                accepted_order: 2,
                committed: None,
                relationships: PublicRelationships {
                    followers_count: Some(0),
                    followers_href: Some("/@an-may/nguoi-theo-doi".into()),
                    ..Default::default()
                },
            }),
        );
        assert_eq!(
            eff2.invalidation.as_ref().map(|i| i.accepted_order),
            Some(2)
        );

        // Late ACK of cmd-1 (order 1, following=true) arrives now -> MUST be completely ignored!
        let stale = ctrl.finish_write(
            &s,
            "cmd-1",
            Ok(SetFollowResponse {
                target_handle: "an-may".into(),
                outcome: "accepted".into(),
                following: true,
                mutual: false,
                pair_version: 1,
                accepted_order: 1,
                committed: None,
                relationships: PublicRelationships {
                    followers_count: Some(1),
                    ..Default::default()
                },
            }),
        );
        assert!(stale.invalidation.is_none());
        assert_eq!(ctrl.highest_accepted_order(), 2);
        match ctrl.phase() {
            FollowPhase::Ready { state, notice, .. } => {
                assert!(!state.following);
                assert_eq!(state.accepted_order, 2);
                assert!(notice.is_none());
            }
            other => panic!("unexpected phase {other:?}"),
        }
    }

    #[test]
    fn session_switch_a_b_a_drops_stale_in_flight_answers() {
        let s1 = scope(1, "an-may");
        let mut ctrl = FollowController::new(s1.clone());
        ctrl.finish_load(&s1, Ok(viewer_other("an-may", true, true, 1, 1)));
        let _ = ctrl.begin_intent("cmd-a1".into(), None);

        // Session switches A -> B -> A, incrementing generation cleanly.
        let s2 = scope(3, "an-may");
        ctrl.reset_scope(s2.clone());
        assert!(matches!(ctrl.phase(), FollowPhase::Loading));

        // Late answers for session 1 never leak into session 3.
        let old_write = ctrl.finish_write(
            &s1,
            "cmd-a1",
            Ok(SetFollowResponse {
                target_handle: "an-may".into(),
                outcome: "accepted".into(),
                following: false,
                mutual: false,
                pair_version: 2,
                accepted_order: 2,
                committed: None,
                relationships: PublicRelationships::default(),
            }),
        );
        assert!(old_write.invalidation.is_none());
        assert!(matches!(ctrl.phase(), FollowPhase::Loading));

        // Session 3 load settles cleanly.
        ctrl.finish_load(&s2, Ok(viewer_other("an-may", false, false, 0, 0)));
        assert!(matches!(
            ctrl.phase(),
            FollowPhase::Ready {
                state: FollowObserved {
                    following: false,
                    ..
                },
                ..
            }
        ));
    }

    #[test]
    fn conflict_reconciles_and_disappearance_requests_heading_focus() {
        let s = scope(1, "an-may");
        let mut ctrl = FollowController::new(s.clone());
        ctrl.finish_load(&s, Ok(viewer_other("an-may", false, false, 0, 0)));
        ctrl.begin_intent("cmd-1".into(), None);

        let conflict = ctrl.finish_write(
            &s,
            "cmd-1",
            Err(ApiError::FollowConflict {
                observed: FollowObserved {
                    target_handle: "an-may".into(),
                    following: true,
                    mutual: true,
                    pair_version: 1,
                    accepted_order: 1,
                },
            }),
        );
        assert_eq!(
            conflict
                .invalidation
                .as_ref()
                .map(|i| (i.following, i.accepted_order)),
            Some((true, 1))
        );

        ctrl.begin_intent("cmd-2".into(), None);
        let gone = ctrl.finish_write(&s, "cmd-2", Err(ApiError::ProfileGone));
        assert!(gone.should_focus_heading);
        assert!(matches!(ctrl.phase(), FollowPhase::Gone));
    }

    #[test]
    fn safe_profile_return_and_vi_count_enforce_strict_rules() {
        assert_eq!(safe_profile_return("/@an-may"), Some("/@an-may".into()));
        assert_eq!(
            safe_profile_return("/@an-may/nguoi-theo-doi"),
            Some("/@an-may/nguoi-theo-doi".into())
        );
        assert_eq!(
            safe_profile_return("/@an-may/dang-theo-doi"),
            Some("/@an-may/dang-theo-doi".into())
        );

        for bad in [
            "https://evil.example/@an-may",
            "//evil.example/@an-may",
            "/@an-may?x=1",
            "/@an-may#frag",
            "/@an--may",
            "/@AnMay",
            "/@an-may/khac",
            "/@an-may\\evil",
            "/toi",
            "",
        ] {
            assert_eq!(safe_profile_return(bad), None, "{bad}");
        }

        assert_eq!(format_vi_count(0), "0");
        assert_eq!(format_vi_count(999), "999");
        assert_eq!(format_vi_count(1000), "1.000");
        assert_eq!(format_vi_count(1_234_567), "1.234.567");
        let _ = FollowDirection::Followers;
    }
}
