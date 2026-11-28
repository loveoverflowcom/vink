use super::super::inline::{Editor, Field, Status};
use super::announce_change;
use crate::{
    account::Session,
    api,
    contract::{ApiError, profile::OwnProfile},
};
use leptos::{prelude::*, task::spawn_local};

#[derive(Clone)]
pub(super) struct Driver {
    pub session: Session,
    scope: String,
    pub state: ArcRwSignal<Editor>,
}

#[derive(Clone, Copy)]
struct DraftSlot(RwSignal<Option<Driver>>);

pub(crate) fn provide_draft(session: Session) {
    let slot = RwSignal::new(None::<Driver>);
    provide_context(DraftSlot(slot));
    Effect::new(move |_| {
        let scope = session.profile_scope.get();
        if slot.with_untracked(|saved| {
            saved
                .as_ref()
                .is_some_and(|d| Some(&d.scope) != scope.as_ref())
        }) {
            slot.set(None);
        }
    });
}

impl Driver {
    pub fn mount(session: Session, scope: String, initial: OwnProfile) -> Self {
        let DraftSlot(slot) = expect_context();
        if let Some(d) = slot.get_untracked().filter(|d| d.scope == scope) {
            d.state.update(|e| e.observe(initial));
            return d;
        }
        let d = Self {
            session,
            scope,
            state: ArcRwSignal::new(Editor::new(initial)),
        };
        slot.set(Some(d.clone()));
        d
    }
    fn live(&self) -> bool {
        self.session
            .profile_scope
            .try_get_untracked()
            .flatten()
            .as_ref()
            == Some(&self.scope)
            && self.state.with_untracked(|e| {
                self.session
                    .principal
                    .try_get_untracked()
                    .flatten()
                    .as_ref()
                    == Some(&e.acked.principal_id)
            })
    }
    pub fn refresh(&self) {
        if !self.live() || self.state.with_untracked(|e| e.flight.is_some()) {
            return;
        }
        let d = self.clone();
        spawn_local(async move {
            if let Ok(p) = api::load_own_profile().await
                && d.live()
            {
                let version = p.profile_version;
                d.state.update(|e| e.observe(p));
                d.clear_later(version);
            }
        });
    }
    pub fn pump(&self) {
        if !self.live() {
            return;
        }
        let Some(f) = self.state.try_update(Editor::start).flatten() else {
            return;
        };
        let d = self.clone();
        spawn_local(async move {
            if !d.live() {
                return;
            }
            let result = api::save_profile(&f.command).await;
            let ambiguous = matches!(
                result,
                Ok(_)
                    | Err(ApiError::Network | ApiError::Unavailable | ApiError::Unexpected { .. })
            );
            // Public caches may need invalidation even after a session change.
            if ambiguous {
                announce_change()
            }
            if !d.live() {
                return;
            }
            match result {
                Ok(p) if f.command.matches_successor(&p) => {
                    d.accept(p, true);
                    d.pump();
                }
                Err(ApiError::Unauthenticated | ApiError::Forbidden | ApiError::NotFound) => {
                    d.session.fence_profile()
                }
                Err(ApiError::Invalid { .. }) => {
                    d.state.update(Editor::refused);
                    d.pump();
                }
                _ => d.read_back(),
            }
        });
    }
    pub fn read_back(&self) {
        if !self.live() {
            return;
        }
        let Some(f) = self
            .state
            .try_update(|e| {
                if e.reading {
                    return None;
                }
                let f = e.flight.clone()?;
                e.unknown = true;
                e.reading = true;
                Some(f)
            })
            .flatten()
        else {
            return;
        };
        let d = self.clone();
        spawn_local(async move {
            let result = api::load_own_profile().await;
            if !d.live() {
                return;
            }
            match result {
                Ok(p)
                    if p.principal_id == f.command.principal_id
                        && p.profile_version >= f.command.expected_version =>
                {
                    if p.profile_version > f.command.expected_version {
                        announce_change()
                    }
                    d.accept(p, false);
                    d.pump();
                }
                Err(ApiError::Unauthenticated | ApiError::Forbidden | ApiError::NotFound) => {
                    d.session.fence_profile()
                }
                _ => d.state.update(|e| e.reading = false),
            }
        });
    }
    fn accept(&self, p: OwnProfile, acknowledged: bool) {
        let version = p.profile_version;
        let name_changed = self
            .state
            .with_untracked(|e| e.acked.pen_name != p.pen_name);
        self.state.update(|e| e.settle(p, acknowledged));
        if name_changed {
            self.session.refresh();
        }
        self.clear_later(version);
    }
    fn clear_later(&self, version: i64) {
        let d = self.clone();
        set_timeout(
            move || {
                if d.live() {
                    d.state.update(|e| e.clear_notices(version));
                }
            },
            std::time::Duration::from_secs(3),
        );
    }
}

#[derive(Clone, Copy)]
pub(super) struct Form(pub StoredValue<Driver>);
impl Form {
    pub fn read<T>(self, f: impl FnOnce(&Editor) -> T) -> T {
        self.0.with_value(|d| d.state.with(f))
    }
    pub fn change(self, f: impl FnOnce(&mut Editor)) {
        self.0.with_value(|d| d.state.update(f))
    }
    pub fn commit(self, field: Field) {
        self.change(|e| e.commit(field));
        self.pump()
    }
    pub fn pump(self) {
        self.0.with_value(Driver::pump)
    }
    pub fn retry(self, field: Field) {
        if self.read(|e| e.status(field) == Status::CheckFailed) {
            self.0.with_value(Driver::read_back);
        } else {
            self.change(|e| e.resolve(field, true));
            self.pump();
        }
    }
}
