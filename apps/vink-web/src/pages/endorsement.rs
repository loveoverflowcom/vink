//! Literary endorsement (LVE #73) feature adapter for the reader. HTTP, command keys, session
//! scope and late answers stay here and in [`crate::endorsement_state`]; `EndorsementAction`
//! receives facts and emits desired states only. Production has no auth authority (D-05): a
//! guest sees public counts and a text reason, and no write or own-state read is sent. Writes
//! run only with the loopback LocalFixture marker, labelled on the page.
use leptos::prelude::*;
use vink_ui::RecoveryIntent;

use crate::contract::EndorsementSummaries;
use crate::endorsement_state::{EndorsementController, Scope, TargetKey};
use crate::ui::patterns::{CommentEndorsements, EndorsementAction, EndorsementKind};

/// One reader page's endorsement state, shared by the work action and every comment action.
#[derive(Clone, Copy)]
pub(crate) struct Endorsements {
    controller: RwSignal<EndorsementController>,
    /// Asks READ-02 to revalidate the publication when the server names a newer revision.
    #[cfg_attr(not(feature = "hydrate"), allow(dead_code))]
    on_superseded: Callback<()>,
    /// The reader's owner: transports survive comment re-renders and end with the page.
    #[cfg(feature = "hydrate")]
    owner: StoredValue<leptos::reactive::owner::WeakOwner>,
}

impl Endorsements {
    /// Creates and provides the page's endorsement state; call in the reader's component body.
    pub(crate) fn provide(work_id: String, on_superseded: Callback<()>) -> Self {
        let scope = Scope {
            work_id,
            session: crate::api::local_discussion_fixture_scope(),
        };
        let this = Self {
            controller: RwSignal::new(EndorsementController::new(scope)),
            on_superseded,
            #[cfg(feature = "hydrate")]
            owner: StoredValue::new(
                Owner::current()
                    .expect("Endorsements::provide runs in a component")
                    .downgrade(),
            ),
        };
        provide_context(this);
        this
    }

    pub(crate) fn current() -> Option<Self> {
        use_context::<Self>()
    }

    /// Seeds counts from a server-rendered read before anything renders them, identically on
    /// the server and during hydration (untracked: nothing already rendered depends on them).
    pub(crate) fn seed(&self, targets: Vec<TargetKey>, summaries: Option<&EndorsementSummaries>) {
        self.controller
            .update_untracked(|c| c.seed(targets, summaries));
    }

    fn facts(&self, target: &TargetKey) -> Option<vink_patterns::EndorsementFacts> {
        self.controller.with(|c| c.facts(target))
    }

    /// The slot `DiscussionThread` renders for each comment.
    pub(crate) fn comments(self) -> CommentEndorsements {
        CommentEndorsements {
            facts: Callback::new(move |id: String| self.facts(&TargetKey::comment(&id))),
            on_set: Callback::new(move |(id, desired): (String, bool)| {
                self.press(TargetKey::comment(&id), desired)
            }),
            on_recover: Callback::new(move |id: String| self.recover(TargetKey::comment(&id))),
        }
    }

    /// Reads public summaries (when `counts`) and the viewer's own states of `targets` in the
    /// browser, at most 100 comments per request. A guest reads counts only.
    #[cfg_attr(not(feature = "hydrate"), allow(dead_code))]
    pub(crate) fn read(&self, targets: Vec<TargetKey>, counts: bool) {
        #[cfg(feature = "hydrate")]
        {
            let (work, comments): (Vec<_>, Vec<_>) = targets
                .into_iter()
                .partition(|target| matches!(target, TargetKey::Work { .. }));
            let mut batches: Vec<Vec<TargetKey>> = comments
                .chunks(crate::endorsement_state::MAX_BATCH)
                .map(<[TargetKey]>::to_vec)
                .collect();
            match batches.first_mut() {
                Some(first) => first.extend(work),
                None if !work.is_empty() => batches.push(work),
                None => {}
            }
            let this = *self;
            for batch in batches {
                if counts {
                    let Some(ticket) = this
                        .controller
                        .try_update(|c| c.begin_summaries(batch.clone()))
                    else {
                        return;
                    };
                    let work_id = this.work_id();
                    this.spawn(async move {
                        let result =
                            crate::api::load_endorsement_summaries(&work_id, &ticket.comment_ids())
                                .await;
                        let superseded = this
                            .controller
                            .try_update(|c| c.apply_summaries(&ticket, result.ok().as_ref()));
                        if superseded == Some(true) {
                            this.on_superseded.run(());
                        }
                    });
                }
                this.read_own(batch);
            }
        }
        #[cfg(not(feature = "hydrate"))]
        let _ = (targets, counts);
    }

    #[cfg(feature = "hydrate")]
    fn work_id(&self) -> String {
        self.controller
            .with_untracked(|c| c.scope().work_id.clone())
    }

    #[cfg(feature = "hydrate")]
    fn spawn(&self, task: impl std::future::Future<Output = ()> + 'static) {
        if let Some(owner) = self.owner.try_get_value().and_then(|owner| owner.upgrade()) {
            owner.with(|| leptos::task::spawn_local_scoped_with_cancellation(task));
        }
    }

    #[cfg(feature = "hydrate")]
    fn read_own(&self, targets: Vec<TargetKey>) {
        let Some(Some(ticket)) = self.controller.try_update(|c| c.begin_own(targets)) else {
            return;
        };
        self.send_own_read(ticket);
    }

    #[cfg(feature = "hydrate")]
    fn send_own_read(&self, ticket: crate::endorsement_state::ReadTicket) {
        let this = *self;
        let work_id = this.work_id();
        this.spawn(async move {
            let result = crate::api::load_own_endorsements(&work_id, &ticket.comment_ids()).await;
            let superseded = this.controller.try_update(|c| c.apply_own(&ticket, result));
            if superseded == Some(true) {
                this.on_superseded.run(());
            }
        });
    }

    /// `ToggleAction` requires the target to be pending before this returns.
    fn press(&self, target: TargetKey, desired: bool) {
        #[cfg(feature = "hydrate")]
        {
            let Some(command_key) = random_key() else {
                return;
            };
            let Some(Some(command)) = self
                .controller
                .try_update(|c| c.press(&target, desired, command_key))
            else {
                return;
            };
            self.send(command);
        }
        #[cfg(not(feature = "hydrate"))]
        let _ = (target, desired);
    }

    fn recover(&self, target: TargetKey) {
        #[cfg(feature = "hydrate")]
        {
            use crate::endorsement_state::Recovery;
            match self.controller.try_update(|c| c.recover(&target)) {
                Some(Recovery::Replay(command)) => self.send(command),
                Some(Recovery::ReadOwn(ticket)) => self.send_own_read(ticket),
                Some(Recovery::Nothing) | None => {}
            }
        }
        #[cfg(not(feature = "hydrate"))]
        let _ = target;
    }

    #[cfg(feature = "hydrate")]
    fn send(&self, command: crate::endorsement_state::Command) {
        use crate::endorsement_state::Completion;
        let this = *self;
        let work_id = this.work_id();
        this.spawn(async move {
            let result = crate::api::set_endorsement(&work_id, &command.request()).await;
            let completion = this.controller.try_update(|c| c.complete(&command, result));
            if completion == Some(Completion::RevisionChanged) {
                this.on_superseded.run(());
            }
        });
    }
}

/// A random version-4 UUID from the browser's CSPRNG; `None` sends nothing.
#[cfg(feature = "hydrate")]
fn random_key() -> Option<String> {
    let mut random = [0_u8; 16];
    web_sys::window()?
        .crypto()
        .ok()?
        .get_random_values_with_u8_array(&mut random)
        .ok()?;
    Some(crate::endorsement_state::uuid_v4(random))
}

/// “Đánh giá cao” for the revision the reader shows, after the text. The count is the public
/// summary of that revision only; another revision's count is never shown for it.
#[component]
pub(crate) fn WorkEndorsement(
    revision_id: String,
    summaries: Option<EndorsementSummaries>,
) -> impl IntoView {
    let Some(endorsements) = Endorsements::current() else {
        return ().into_any();
    };
    let target = TargetKey::Work { revision_id };
    endorsements.seed(vec![target.clone()], summaries.as_ref());
    #[cfg(feature = "hydrate")]
    {
        // After hydration: the viewer's own state (fixture only), and counts if SSR had none
        // or they describe another revision.
        let counts = summaries.as_ref().is_none_or(|s| {
            TargetKey::Work {
                revision_id: s.work.revision_id.clone(),
            } != target
        });
        let read = target.clone();
        endorsements.spawn(async move { endorsements.read(vec![read], counts) });
    }
    let facts = {
        let target = target.clone();
        Memo::new(move |_| endorsements.facts(&target).unwrap_or_default())
    };
    let (set_target, recover_target) = (target.clone(), target);
    view! {
        <div id="work-endorsement" class="vk-reader-endorsement">
            <EndorsementAction
                id="endorse-work"
                kind=EndorsementKind::Work
                facts
                on_set=move |desired: bool| endorsements.press(set_target.clone(), desired)
                on_recover=move |_: RecoveryIntent| endorsements.recover(recover_target.clone())
            />
        </div>
    }
    .into_any()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::{EndorsementSummaries, WorkEndorsementSummary};

    fn summaries(revision: &str, count: i64) -> EndorsementSummaries {
        EndorsementSummaries {
            work_id: "w".into(),
            work: WorkEndorsementSummary {
                status: "available".into(),
                count: Some(count),
                revision_id: revision.into(),
            },
            comments: vec![],
        }
    }

    fn render(session: Option<&str>, summary: Option<EndorsementSummaries>) -> String {
        Owner::new().with(|| {
            let controller = RwSignal::new(EndorsementController::new(Scope {
                work_id: "w".into(),
                session: session.map(str::to_owned),
            }));
            provide_context(Endorsements {
                controller,
                on_superseded: Callback::new(|()| {}),
                #[cfg(feature = "hydrate")]
                owner: StoredValue::new(Owner::current().unwrap().downgrade()),
            });
            view! { <WorkEndorsement revision_id="r1".to_string() summaries=summary /> }.to_html()
        })
    }

    #[test]
    fn lve_web_guest_ssr_shows_the_public_count_and_a_text_reason_without_a_login() {
        let html = render(None, Some(summaries("r1", 1234)));
        assert!(html.contains("id=\"endorse-work\""), "{html}");
        assert!(html.contains("1.234"), "grouped public count: {html}");
        assert!(html.contains("Cần đăng nhập để đánh giá."), "{html}");
        assert!(html.contains("aria-disabled=\"true\""), "{html}");
        assert!(!html.contains("href"), "no fake login link: {html}");
        assert!(!html.contains("Kiểm tra lại"), "{html}");
    }

    #[test]
    fn lve_web_ssr_never_shows_another_revisions_count_or_a_zero_for_a_failed_read() {
        let html = render(None, Some(summaries("r2", 7)));
        assert!(!html.contains(">7<"), "another revision's count: {html}");
        assert!(html.contains("data-count=\"unavailable\""), "{html}");
        let html = render(None, None);
        assert!(
            html.contains("data-count=\"unavailable\"") && !html.contains(">0<"),
            "{html}"
        );
    }

    #[test]
    fn lve_web_fixture_ssr_is_unknown_until_the_browser_reads_the_own_state() {
        let html = render(Some("fixture"), Some(summaries("r1", 3)));
        assert!(html.contains("data-pressed=\"unknown\""), "{html}");
        assert!(!html.contains("Cần đăng nhập"), "{html}");
        assert!(
            html.contains("Kiểm tra lại"),
            "unknown offers reconcile: {html}"
        );
    }
}
