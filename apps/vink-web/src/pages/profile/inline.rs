//! One CAS at a time. Only an explicit field intent can enter a full-profile request.
use crate::contract::profile::{OwnProfile, Policies, SaveProfile, Visibility};
use std::collections::BTreeMap;
use unicode_normalization::UnicodeNormalization;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Field {
    Name,
    Bio,
    Policy(Policy),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Policy {
    Avatar,
    Bio,
    FollowersList,
    FollowersCount,
    FollowingList,
    FollowingCount,
}

impl Policy {
    pub(super) const ALL: [Self; 6] = [
        Self::Avatar,
        Self::Bio,
        Self::FollowersList,
        Self::FollowersCount,
        Self::FollowingList,
        Self::FollowingCount,
    ];
    pub(super) fn access(self, p: &mut Policies) -> &mut Visibility {
        match self {
            Self::Avatar => &mut p.avatar,
            Self::Bio => &mut p.bio,
            Self::FollowersList => &mut p.followers_list,
            Self::FollowersCount => &mut p.followers_count,
            Self::FollowingList => &mut p.following_list,
            Self::FollowingCount => &mut p.following_count,
        }
    }
}
impl Field {
    pub(super) fn id(self) -> &'static str {
        match self {
            Self::Name => "profile-pen-name",
            Self::Bio => "profile-bio",
            Self::Policy(p) => match p {
                Policy::Avatar => "profile-avatar-policy",
                Policy::Bio => "profile-bio-policy",
                Policy::FollowersList => "profile-followers-list",
                Policy::FollowersCount => "profile-followers-count",
                Policy::FollowingList => "profile-following-list",
                Policy::FollowingCount => "profile-following-count",
            },
        }
    }
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Name => "Bút danh",
            Self::Bio => "Giới thiệu",
            Self::Policy(p) => match p {
                Policy::Avatar => "Ảnh đại diện",
                Policy::Bio => "Giới thiệu",
                Policy::FollowersList => "Danh sách người theo dõi",
                Policy::FollowersCount => "Số người theo dõi",
                Policy::FollowingList => "Danh sách đang theo dõi",
                Policy::FollowingCount => "Số người đang theo dõi",
            },
        }
    }
    fn stored(self, profile: &OwnProfile) -> Value {
        match self {
            Self::Name => Value::Name(profile.pen_name.clone()),
            Self::Bio => Value::Bio(profile.bio.clone()),
            Self::Policy(p) => Value::Policy(p, *p.access(&mut profile.policies.clone())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Value {
    Name(String),
    Bio(Option<String>),
    Policy(Policy, Visibility),
}
impl Value {
    fn apply(&self, command: &mut SaveProfile) {
        match self {
            Self::Name(s) => command.pen_name.clone_from(s),
            Self::Bio(s) => command.bio.clone_from(s),
            Self::Policy(p, v) => *p.access(&mut command.policies) = *v,
        }
    }
    fn display(&self) -> String {
        match self {
            Self::Name(s) => s.clone(),
            Self::Bio(s) => s.clone().unwrap_or_default(),
            Self::Policy(_, Visibility::Owner) => "Chỉ mình tôi".into(),
            Self::Policy(_, Visibility::Everyone) => "Công khai".into(),
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) enum Status {
    #[default]
    Idle,
    Queued,
    Saving,
    Checking,
    CheckFailed,
    Retry,
    Invalid,
    Conflict,
    Saved,
    Observed,
}
#[derive(Debug, Default)]
struct Edit {
    // Baseline at first input, including text typed before another tab's change.
    baseline: Option<Value>,
    raw: Option<String>,
    generation: u64,
    intent: Option<(u64, Value)>,
    status: Status,
    races: u8,
    invalid: bool,
    notice_version: i64,
}
#[derive(Debug, Clone)]
pub(super) struct Flight {
    pub field: Field,
    generation: u64,
    submitted: Value,
    pub command: SaveProfile,
}

#[derive(Debug)]
pub(super) struct Editor {
    pub acked: OwnProfile,
    edits: BTreeMap<Field, Edit>,
    pub flight: Option<Flight>,
    pub reading: bool,
    pub unknown: bool,
    retry_barrier: Option<Field>,
}
impl Editor {
    pub fn new(acked: OwnProfile) -> Self {
        Self {
            acked,
            edits: BTreeMap::new(),
            flight: None,
            reading: false,
            unknown: false,
            retry_barrier: None,
        }
    }
    pub fn text(&self, field: Field) -> String {
        self.edits
            .get(&field)
            .and_then(|e| e.raw.clone())
            .unwrap_or_else(|| field.stored(&self.acked).display())
    }
    pub fn stored_text(&self, field: Field) -> String {
        field.stored(&self.acked).display()
    }
    pub fn input(&mut self, field: Field, raw: String) {
        let edit = self.edits.entry(field).or_default();
        if edit.raw.as_ref() == Some(&raw) {
            return;
        }
        edit.baseline
            .get_or_insert_with(|| field.stored(&self.acked));
        edit.raw = Some(raw);
        edit.invalid = false;
        edit.generation += 1;
        if matches!(edit.status, Status::Saved | Status::Observed) {
            edit.status = Status::Idle
        }
    }
    pub fn commit(&mut self, field: Field) {
        let Some(raw) = self.edits.get(&field).and_then(|e| e.raw.as_deref()) else {
            return;
        };
        let value = match field {
            Field::Name => {
                let text: String = raw.trim().nfc().collect();
                (!text.is_empty()
                    && text.chars().count() <= 80
                    && !text.chars().any(char::is_control))
                .then_some(Value::Name(text))
            }
            Field::Bio => {
                let text = raw.trim().replace("\r\n", "\n");
                (text.chars().count() <= 500 && !text.chars().any(|c| c.is_control() && c != '\n'))
                    .then(|| Value::Bio((!text.is_empty()).then_some(text)))
            }
            Field::Policy(_) => return,
        };
        if let Some(value) = value {
            self.enqueue(field, value)
        } else {
            self.edits.entry(field).or_default().invalid = true;
        }
    }
    pub fn narrow(&mut self, policy: Policy) {
        self.enqueue(
            Field::Policy(policy),
            Value::Policy(policy, Visibility::Owner),
        );
    }
    fn enqueue(&mut self, field: Field, value: Value) {
        let stored = field.stored(&self.acked);
        let held = self.retry_barrier == Some(field);
        let active = self.flight.as_ref().is_some_and(|f| f.field == field);
        let edit = self.edits.entry(field).or_default();
        if edit.intent.as_ref() == Some(&(edit.generation, value.clone())) {
            return;
        }
        if !active && !held && value == stored {
            *edit = Edit::default();
            return;
        }
        let diverged = edit
            .baseline
            .as_ref()
            .is_some_and(|base| *base != stored && value != stored);
        edit.intent = Some((edit.generation, value));
        edit.status = if diverged || edit.status == Status::Conflict {
            Status::Conflict
        } else if held {
            Status::Retry
        } else {
            Status::Queued
        };
    }
    pub fn escape(&mut self, field: Field) {
        let Some(e) = self.edits.get_mut(&field) else {
            return;
        };
        if let Some((generation, value)) = &e.intent {
            e.raw = Some(value.display());
            e.generation = *generation;
            e.invalid = false;
        } else {
            *e = Edit::default()
        }
    }
    pub fn invalid(&self, field: Field) -> bool {
        self.edits.get(&field).is_some_and(|e| e.invalid)
    }
    pub fn status(&self, field: Field) -> Status {
        if self.flight.as_ref().is_some_and(|f| f.field == field) {
            if self.reading {
                return Status::Checking;
            }
            if self.unknown {
                return Status::CheckFailed;
            }
            return Status::Saving;
        }
        self.edits.get(&field).map_or(Status::Idle, |e| {
            if e.invalid && e.status == Status::Idle {
                Status::Invalid
            } else {
                e.status
            }
        })
    }
    pub fn unsaved(&self) -> bool {
        self.flight.is_some()
            || self
                .edits
                .values()
                .any(|e| e.raw.is_some() || e.intent.is_some())
    }
    pub fn start(&mut self) -> Option<Flight> {
        if self.flight.is_some() || self.retry_barrier.is_some() {
            return None;
        }
        let (&field, e) = self
            .edits
            .iter()
            .find(|(_, e)| e.status == Status::Queued && e.intent.is_some())?;
        let (generation, value) = e.intent.as_ref()?;
        let p = &self.acked;
        let mut command = SaveProfile {
            principal_id: p.principal_id.clone(),
            expected_version: p.profile_version,
            pen_name: p.pen_name.clone(),
            bio: p.bio.clone(),
            policies: p.policies,
        };
        value.apply(&mut command);
        let f = Flight {
            field,
            generation: *generation,
            submitted: value.clone(),
            command,
        };
        self.flight = Some(f.clone());
        Some(f)
    }
    // Only observe a newer projection. An older GET from route re-entry cannot settle a PUT.
    pub fn observe(&mut self, stored: OwnProfile) {
        if self.flight.is_none()
            && stored.profile_version > self.acked.profile_version
            && stored.principal_id == self.acked.principal_id
        {
            self.merge(stored, None, false);
        }
    }
    pub fn settle(&mut self, stored: OwnProfile, acknowledged: bool) {
        let Some(flight) = self.flight.take() else {
            return;
        };
        self.reading = false;
        self.unknown = false;
        self.merge(stored, Some(flight), acknowledged);
    }
    fn merge(&mut self, stored: OwnProfile, flight: Option<Flight>, acknowledged: bool) {
        let exact = flight
            .as_ref()
            .is_some_and(|f| f.command.matches_successor(&stored));
        if stored.profile_version > self.acked.profile_version {
            self.retry_barrier = None;
        }
        for (&field, e) in &mut self.edits {
            let old = field.stored(&self.acked);
            let new = field.stored(&stored);
            let sent = flight.as_ref().filter(|f| f.field == field);
            // The server may have advanced again after storing the submitted field.
            // This permits rebasing newer intent without attributing a receipt to this tab.
            let matched = stored.profile_version > self.acked.profile_version
                && sent.is_some_and(|f| f.submitted == new);
            if matched {
                e.baseline = Some(new.clone())
            }
            let Some((generation, wanted)) = &e.intent else {
                continue;
            };
            if wanted == &new {
                let current = e.generation == *generation
                    && !(matched && sent.is_some_and(|f| f.generation != *generation));
                if current {
                    e.raw = None;
                    e.baseline = None;
                } else if matched {
                    e.baseline = Some(new.clone())
                }
                e.status = if !current {
                    Status::Idle
                } else if acknowledged && exact && sent.is_some_and(|f| f.generation == *generation)
                {
                    Status::Saved
                } else {
                    Status::Observed
                };
                e.intent = None;
                e.races = 0;
                e.notice_version = stored.profile_version;
            } else if !matched && old != new {
                e.status = Status::Conflict;
            } else if sent.is_some() {
                // At most one automatic rebase for a disjoint CAS race. An unchanged
                // version after an ambiguous result requires an explicit retry.
                e.races = e.races.saturating_add(u8::from(!matched));
                if stored.profile_version == self.acked.profile_version {
                    self.retry_barrier = Some(field);
                }
                e.status = if !matched
                    && (stored.profile_version == self.acked.profile_version || e.races > 1)
                {
                    Status::Retry
                } else {
                    Status::Queued
                };
            }
        }
        self.acked = stored;
    }
    pub fn refused(&mut self) {
        if let Some(f) = self.flight.take() {
            self.edits.entry(f.field).or_default().status = Status::Retry
        }
    }
    pub fn resolve(&mut self, field: Field, keep: bool) {
        if self.retry_barrier == Some(field) {
            self.retry_barrier = None;
        }
        let e = self.edits.entry(field).or_default();
        if keep {
            e.baseline = Some(field.stored(&self.acked));
            e.races = 0;
            e.status = Status::Queued;
        } else {
            *e = Edit::default();
        }
    }
    pub fn clear_notices(&mut self, version: i64) {
        for e in self.edits.values_mut() {
            if e.notice_version == version && matches!(e.status, Status::Saved | Status::Observed) {
                e.status = Status::Idle;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Field::{Bio, Name};
    fn initial() -> OwnProfile {
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../services/vink-app/contracts/http/v1/fixtures/author.profile.own.json"
        )))
        .unwrap()
    }
    fn commit(e: &mut Editor, field: Field, text: &str) {
        e.input(field, text.into());
        e.commit(field);
    }
    #[test]
    fn exact_ack_keeps_newer_generation_even_when_normalized_text_matches() {
        let mut e = Editor::new(initial());
        commit(&mut e, Name, "Tên");
        let f = e.start().unwrap();
        commit(&mut e, Name, "  Tên  ");
        assert!(e.start().is_none());
        let mut stored = initial();
        stored.pen_name = "Tên".into();
        stored.profile_version += 1;
        assert!(f.command.matches_successor(&stored));
        e.settle(stored, true);
        assert_eq!(e.text(Name), "  Tên  ");
        assert_eq!(e.status(Name), Status::Idle);
        assert!(e.start().is_none());
        e.escape(Name);
        assert_eq!(e.text(Name), "Tên");
    }
    #[test]
    fn unchanged_unknown_requires_readback_and_explicit_retry_before_queued_fields() {
        let mut e = Editor::new(initial());
        commit(&mut e, Name, "Tên");
        e.start().unwrap();
        e.unknown = true;
        commit(&mut e, Bio, "Một");
        assert!(e.start().is_none());
        e.settle(initial(), false);
        assert_eq!(e.status(Name), Status::Retry);
        assert!(e.start().is_none());
        e.resolve(Name, true);
        let f = e.start().unwrap();
        assert_eq!(f.command.bio, initial().bio);
        assert_eq!(f.command.expected_version, 3);
    }
    #[test]
    fn conflicts_stay_local_races_are_bounded_and_locked_keys_travel_unchanged() {
        let mut e = Editor::new(initial());
        commit(&mut e, Name, "Của tôi");
        e.start().unwrap();
        e.narrow(Policy::FollowersCount);
        let mut other = initial();
        other.profile_version += 1;
        other.pen_name = "Ở tab khác".into();
        other.policies.followers_list = Visibility::Owner;
        e.settle(other.clone(), false);
        assert_eq!(e.status(Name), Status::Conflict);
        let next = e.start().unwrap();
        assert_eq!(next.command.pen_name, "Ở tab khác");
        assert_eq!(next.command.policies.followers_list, Visibility::Owner);
        assert_eq!(next.command.policies.date_of_birth, Visibility::Owner);
        assert_eq!(next.command.policies.followers_count, Visibility::Owner);
        other.profile_version += 1;
        other.bio = None;
        e.settle(other.clone(), false);
        e.start().unwrap();
        other.profile_version += 1;
        other.bio = Some("Mới".into());
        e.settle(other, false);
        assert_eq!(
            e.status(Field::Policy(Policy::FollowersCount)),
            Status::Retry
        );
        assert!(e.start().is_none());
        e.resolve(Name, false);
        assert_eq!(e.text(Name), "Ở tab khác");
    }
    #[test]
    fn advanced_convergence_is_observed_and_rebase_never_includes_uncommitted_bio() {
        let mut e = Editor::new(initial());
        e.input(Bio, "Nháp".into());
        commit(&mut e, Name, "Tên");
        let f = e.start().unwrap();
        let mut other = initial();
        other.pen_name = "Tên".into();
        other.profile_version += 2;
        other.policies.avatar = Visibility::Owner;
        assert!(!f.command.matches_successor(&other));
        e.settle(other, false);
        assert_eq!(e.status(Name), Status::Observed);
        assert_eq!(e.text(Bio), "Nháp");
        e.clear_notices(5);
        assert_eq!(e.status(Name), Status::Idle);
        assert!(e.start().is_none());
    }
    #[test]
    fn lost_ack_then_another_field_advanced_preserves_the_newer_commit() {
        let mut e = Editor::new(initial());
        commit(&mut e, Name, "Đầu");
        e.start().unwrap();
        commit(&mut e, Name, "Cuối");
        let mut latest = initial();
        latest.pen_name = "Đầu".into();
        latest.bio = None;
        latest.profile_version += 2;
        e.settle(latest, false);
        let next = e.start().unwrap();
        assert_eq!(next.command.expected_version, 5);
        assert_eq!(next.command.pen_name, "Cuối");
        assert_eq!(next.command.bio, None);
    }
    #[test]
    fn bounds_normalization_empty_bio_and_validation_are_field_specific() {
        let mut e = Editor::new(initial());
        commit(&mut e, Name, &"ế".repeat(81));
        assert_eq!(e.status(Name), Status::Invalid);
        commit(&mut e, Bio, &"ồ".repeat(501));
        assert_eq!(e.status(Bio), Status::Invalid);
        assert!(e.start().is_none());
        commit(&mut e, Bio, "  ");
        let f = e.start().unwrap();
        assert_eq!(f.command.bio, None);
        assert_eq!(f.command.pen_name, initial().pen_name);
        let mut stored = initial();
        stored.bio = None;
        stored.profile_version += 1;
        e.settle(stored, true);
        commit(&mut e, Bio, "  a\r\nb  ");
        assert_eq!(e.start().unwrap().command.bio.as_deref(), Some("a\nb"));
    }
}
