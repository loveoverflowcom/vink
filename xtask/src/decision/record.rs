use std::{
    collections::{BTreeMap, BTreeSet},
    fmt, fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{
    decision_log,
    legacy::{self, Status},
};
use crate::checks::sha256_hex;

pub(super) const VERSION: u16 = 1;
pub(super) const PROPOSALS: &str = "decisions/proposals";
pub(super) const SIGNOFFS: &str = "decisions/signoffs";

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(super) struct Key {
    pub id: String,
    pub revision: u32,
}

impl Key {
    pub fn new(id: &str, revision: u32) -> Self {
        Self {
            id: id.into(),
            revision,
        }
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        let (id, revision) = value
            .split_once("@r")
            .ok_or_else(|| format!("expected ID@rN, got {value:?}"))?;
        validate_id(id)?;
        Ok(Self::new(id, parse_revision(revision)?))
    }
}

impl fmt::Display for Key {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{}@r{}", self.id, self.revision)
    }
}

pub(super) fn parse_revision(value: &str) -> Result<u32, String> {
    value
        .parse::<u32>()
        .ok()
        .filter(|n| *n > 0 && n.to_string() == value)
        .ok_or_else(|| format!("invalid positive revision {value:?}"))
}

pub(super) fn validate_id(id: &str) -> Result<(), String> {
    if id.len() <= 80
        && id.starts_with(|c: char| c.is_ascii_uppercase())
        && id.contains('-')
        && id.split('-').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
        })
    {
        return Ok(());
    }
    Err(format!("invalid decision ID {id:?}"))
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Kind {
    Product,
    Architecture,
}

#[derive(Clone, Copy, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Audience {
    Private,
    Public,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Content {
    pub kind: Kind,
    pub domain: String,
    pub title: String,
    pub resolution: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rationale: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires: Vec<String>,
    /// Requirement, D- or other IDs this revision traces to. Links only, never aliases.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub work_audiences: Vec<Audience>,
}

impl Content {
    pub fn validate(&mut self, path: &Path, proposal: bool) -> Result<(), String> {
        let error = |key: &str, message: &str| format!("{}: {key}: {message}", path.display());
        if !valid_domain(&self.domain) {
            return Err(error("domain", "expected a lowercase kebab-case name"));
        }
        for (name, value) in [("title", &self.title), ("resolution", &self.resolution)] {
            if value.trim().is_empty() || value.contains('\r') {
                return Err(error(name, "expected nonblank text with LF line breaks"));
            }
        }
        if let Some(rationale) = &self.rationale {
            if rationale.trim().is_empty() || rationale.contains('\r') {
                return Err(error(
                    "rationale",
                    "expected nonblank text with LF line breaks",
                ));
            }
        } else if proposal {
            return Err(error("rationale", "required for a proposal"));
        }
        for reference in &self.requires {
            Key::parse(reference).map_err(|e| error("requires", &e))?;
        }
        for id in &self.related_ids {
            validate_id(id).map_err(|e| error("related_ids", &e))?;
        }
        self.requires.sort();
        self.related_ids.sort();
        self.work_audiences.sort();
        if self.requires.windows(2).any(|items| items[0] == items[1]) {
            return Err(error("requires", "duplicate revision reference"));
        }
        if self
            .related_ids
            .windows(2)
            .any(|items| items[0] == items[1])
        {
            return Err(error("related_ids", "duplicate ID"));
        }
        if !self.work_audiences.is_empty()
            && (self.kind != Kind::Product
                || self.domain != "publication"
                || self
                    .work_audiences
                    .windows(2)
                    .any(|items| items[0] == items[1]))
        {
            return Err(error(
                "work_audiences",
                "expected distinct audiences on a product decision in publication",
            ));
        }
        Ok(())
    }
}

pub(super) fn valid_domain(domain: &str) -> bool {
    domain.len() <= 48
        && domain.split('-').all(|part| {
            part.starts_with(|c: char| c.is_ascii_lowercase())
                && part
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        })
}

#[derive(Deserialize, Serialize, Debug, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct Base {
    pub revision: u32,
    pub sha256: String,
}

impl Base {
    pub fn parse(value: &str) -> Result<Option<Self>, String> {
        if value == "new" {
            return Ok(None);
        }
        let (revision, digest) = value
            .split_once(':')
            .ok_or("expected new or REVISION:SHA256")?;
        Ok(Some(Self {
            revision: parse_revision(revision)?,
            sha256: parse_digest(digest)?.into(),
        }))
    }
}

pub(super) fn parse_digest(digest: &str) -> Result<&str, String> {
    if digest.len() == 64
        && digest
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        Ok(digest)
    } else {
        Err("expected a lowercase SHA-256 digest".into())
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Proposal {
    pub version: u16,
    pub id: String,
    pub revision: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<Base>,
    pub content: Content,
}

impl Proposal {
    pub fn filename(&self) -> String {
        format!("{}.r{}.toml", self.id, self.revision)
    }

    pub fn validate(&mut self, path: &Path) -> Result<(), String> {
        let error = |message: &str| format!("{}: {message}", path.display());
        if self.version != VERSION {
            return Err(error(&format!(
                "version: unsupported format {}",
                self.version
            )));
        }
        validate_id(&self.id).map_err(|e| error(&format!("id: {e}")))?;
        if self.revision == 0 {
            return Err(error("revision: expected a positive revision"));
        }
        if path.file_name().and_then(|part| part.to_str()) != Some(self.filename().as_str()) {
            return Err(error("id/revision: does not match proposal filename"));
        }
        self.content.validate(path, true)
    }
}

/// What the owner decided about one proposal revision. Written by `decision signoff` only when
/// the owner's own words approve or decline that exact revision; `source` quotes them.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SignoffInput {
    pub status: Status,
    pub decided_on: String,
    pub source: String,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Signoff {
    pub version: u16,
    pub id: String,
    pub revision: u32,
    pub sha256: String,
    pub status: Status,
    pub decided_on: String,
    pub source: String,
}

impl SignoffInput {
    pub fn validate(&self, path: &Path, kind: Kind) -> Result<(), String> {
        let error = |key: &str, message: &str| format!("{}: {key}: {message}", path.display());
        let allowed = match kind {
            Kind::Product => matches!(
                self.status,
                Status::ProductConfirmed | Status::Deferred | Status::Open | Status::Rejected
            ),
            Kind::Architecture => matches!(
                self.status,
                Status::DesignProposed | Status::Provisional | Status::Accepted | Status::Rejected
            ),
        };
        if !allowed {
            return Err(error(
                "status",
                &format!(
                    "{:?} is not a signoff outcome for a {kind:?} decision",
                    self.status
                ),
            ));
        }
        if !valid_date(&self.decided_on) {
            return Err(error("decided_on", "expected a YYYY-MM-DD date string"));
        }
        if self.source.trim().is_empty() || self.source.contains('\r') {
            return Err(error(
                "source",
                "expected nonblank text with LF line breaks",
            ));
        }
        Ok(())
    }
}

impl Signoff {
    pub fn filename(&self) -> String {
        format!("{}.r{}.toml", self.id, self.revision)
    }

    pub fn input(&self) -> SignoffInput {
        SignoffInput {
            status: self.status,
            decided_on: self.decided_on.clone(),
            source: self.source.clone(),
        }
    }
}

fn valid_date(value: &str) -> bool {
    let parts: Vec<_> = value.split('-').collect();
    let [year, month, day] = parts.as_slice() else {
        return false;
    };
    let number = |text: &str, len: usize| {
        (text.len() == len && text.bytes().all(|b| b.is_ascii_digit()))
            .then(|| text.parse::<u32>().ok())
            .flatten()
    };
    let (Some(year), Some(month), Some(day)) = (number(year, 4), number(month, 2), number(day, 2))
    else {
        return false;
    };
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    (1..=days).contains(&day)
}

pub(super) fn decode<T: serde::de::DeserializeOwned>(
    path: &Path,
    source: &str,
) -> Result<T, String> {
    toml::from_str(source).map_err(|e| format!("{}: {e}", path.display()))
}

pub(super) fn encode(value: &impl Serialize) -> Result<String, String> {
    toml::to_string_pretty(value).map_err(|e| format!("serialize decision TOML: {e}"))
}

pub(super) struct Registry {
    pub entries: BTreeMap<Key, Entry>,
    pub snapshot: String,
}

pub(super) struct Entry {
    pub content: Content,
    pub hash: String,
    pub path: PathBuf,
    pub origin: Origin,
    pub signoff: Option<SignedOff>,
}

pub(super) struct SignedOff {
    pub record: Signoff,
    pub hash: String,
    pub path: PathBuf,
}

pub(super) enum Origin {
    Legacy(Box<legacy::Provenance>),
    Proposal(Option<Base>),
}

impl Entry {
    pub fn base(&self, key: &Key) -> Base {
        Base {
            revision: key.revision,
            sha256: self.hash.clone(),
        }
    }

    pub fn fields(&self) -> Value {
        let mut value = json!({ "content": self.content });
        match &self.origin {
            Origin::Legacy(source) => {
                value["status"] = json!(source.status);
                value["historical"] = json!(source.historical);
                value["context"] = json!(source.context);
                value["related_ids"] = json!(source.related_ids);
            }
            Origin::Proposal(base) => {
                value["base"] = json!(base);
                match &self.signoff {
                    Some(signed) => {
                        value["status"] = json!(signed.record.status);
                        value["signoff"] = json!({
                            "decided_on": signed.record.decided_on,
                            "source": signed.record.source,
                        });
                    }
                    None => value["status"] = json!("PENDING"),
                }
            }
        }
        value
    }

    pub fn related_ids(&self) -> impl Iterator<Item = &String> {
        let imported = match &self.origin {
            Origin::Legacy(source) => source.related_ids.as_slice(),
            Origin::Proposal(_) => &[],
        };
        imported.iter().chain(&self.content.related_ids)
    }

    /// Imported current rules and owner-signed proposals can be current; pending and rejected
    /// proposals never are, whatever their revision number.
    pub fn effective(&self) -> bool {
        match &self.origin {
            Origin::Legacy(source) => !source.historical,
            Origin::Proposal(_) => self
                .signoff
                .as_ref()
                .is_some_and(|signed| signed.record.status != Status::Rejected),
        }
    }
}

impl Registry {
    pub fn load(root: &Path) -> Result<Self, String> {
        let mut entries = legacy::load(root)?;
        for (key, entry) in decision_log::load(root)? {
            if entries.contains_key(&key) {
                return Err(format!(
                    "{}: {key} is already imported from {}",
                    decision_log::FILE,
                    legacy::FILE
                ));
            }
            entries.insert(key, entry);
        }
        let mut registry = Self {
            entries,
            snapshot: String::new(),
        };
        let dir = root.join(PROPOSALS);
        if is_directory(&dir)? {
            // Load numerically. Lexical filename order puts r10 before r2.
            let mut proposals = BTreeMap::new();
            for path in record_files(&dir)? {
                let mut proposal: Proposal = decode(&path, &read_file(&path)?)?;
                proposal.validate(&path)?;
                let key = Key::new(&proposal.id, proposal.revision);
                proposals.insert(key, (path, proposal));
            }
            for (key, (path, proposal)) in proposals {
                registry
                    .check_next(&key, &proposal.base, &proposal.content)
                    .map_err(|error| format!("{}: {error}", path.display()))?;
                let hash = sha256_hex(encode(&proposal)?.as_bytes());
                registry.entries.insert(
                    key,
                    Entry {
                        content: proposal.content,
                        hash,
                        path,
                        origin: Origin::Proposal(proposal.base),
                        signoff: None,
                    },
                );
            }
        }
        let dir = root.join(SIGNOFFS);
        if is_directory(&dir)? {
            for path in record_files(&dir)? {
                let mut signoff: Signoff = decode(&path, &read_file(&path)?)?;
                registry
                    .attach(&path, &mut signoff)
                    .map_err(|error| format!("{}: {error}", path.display()))?;
                let hash = sha256_hex(encode(&signoff)?.as_bytes());
                let entry = registry
                    .entries
                    .get_mut(&Key::new(&signoff.id, signoff.revision))
                    .expect("attach checked the target");
                entry.signoff = Some(SignedOff {
                    record: signoff,
                    hash,
                    path,
                });
            }
        }
        registry.check_dependencies()?;
        let mut snapshot = format!(
            "v{VERSION}:{}:{}",
            legacy::IMPORT_SHA256,
            decision_log::IMPORT_SHA256
        );
        snapshot.extend(registry.entries.values().map(|entry| entry.hash.as_str()));
        snapshot.push_str(":signoffs:");
        snapshot.extend(
            registry
                .entries
                .values()
                .filter_map(|entry| entry.signoff.as_ref())
                .map(|signed| signed.hash.as_str()),
        );
        registry.snapshot = sha256_hex(snapshot.as_bytes());
        Ok(registry)
    }

    /// Checks a signoff against the proposal payload it names. A changed payload, a legacy or
    /// missing target, a second signoff and a self-declared outcome outside the kind all fail.
    pub fn attach(&self, path: &Path, signoff: &mut Signoff) -> Result<(), String> {
        if signoff.version != VERSION {
            return Err(format!("version: unsupported format {}", signoff.version));
        }
        validate_id(&signoff.id).map_err(|e| format!("id: {e}"))?;
        if signoff.revision == 0 {
            return Err("revision: expected a positive revision".into());
        }
        if path.file_name().and_then(|part| part.to_str()) != Some(signoff.filename().as_str()) {
            return Err("id/revision: does not match signoff filename".into());
        }
        parse_digest(&signoff.sha256).map_err(|e| format!("sha256: {e}"))?;
        let key = Key::new(&signoff.id, signoff.revision);
        let entry = self
            .entries
            .get(&key)
            .ok_or_else(|| format!("{key}: signoff names an unknown revision"))?;
        if !matches!(entry.origin, Origin::Proposal(_)) {
            return Err(format!(
                "{key}: imported revisions keep their source status; sign off a proposal"
            ));
        }
        if entry.signoff.is_some() {
            return Err(format!("{key}: revision already has a signoff"));
        }
        if entry.hash != signoff.sha256 {
            return Err(format!(
                "{key}: sha256: signoff is for payload {}, current payload is {}; re-review",
                signoff.sha256, entry.hash
            ));
        }
        signoff.input().validate(path, entry.content.kind)
    }

    pub fn history(&self, id: &str) -> impl DoubleEndedIterator<Item = (&Key, &Entry)> {
        self.entries.range(Key::new(id, 0)..=Key::new(id, u32::MAX))
    }

    pub fn current(&self, id: &str) -> Option<(&Key, &Entry)> {
        self.history(id).rev().find(|(_, entry)| entry.effective())
    }

    pub fn check_next(
        &self,
        key: &Key,
        base: &Option<Base>,
        content: &Content,
    ) -> Result<(), String> {
        if let Some((head, entry)) = self.history(&key.id).next_back() {
            let expected = entry.base(head);
            if base.as_ref() != Some(&expected)
                || head.revision.checked_add(1) != Some(key.revision)
            {
                return Err(format!(
                    "{key}: base: expected {}:{}; query history and retry",
                    expected.revision, expected.sha256
                ));
            }
            if content.kind != entry.content.kind || content.domain != entry.content.domain {
                return Err(format!(
                    "{key}: content.kind/domain changed within the same ID"
                ));
            }
        } else if !key.id.starts_with("VDR-") || base.is_some() || key.revision != 1 {
            return Err(format!(
                "{key}: new decisions require a VDR- ID and revision 1 with no base"
            ));
        }
        Ok(())
    }

    pub fn check_dependencies(&self) -> Result<(), String> {
        let mut outgoing = BTreeMap::new();
        let mut incoming: BTreeMap<&Key, Vec<&Key>> = BTreeMap::new();
        for (key, entry) in &self.entries {
            let mut dependencies = BTreeSet::new();
            if let Origin::Proposal(Some(base)) = &entry.origin {
                dependencies.insert(Key::new(&key.id, base.revision));
            }
            for pin in &entry.content.requires {
                dependencies.insert(Key::parse(pin)?);
            }
            outgoing.insert(key, dependencies.len());
            for dependency in dependencies {
                let Some((target, _)) = self.entries.get_key_value(&dependency) else {
                    return Err(format!(
                        "{}: {key}: requires: missing {dependency}",
                        entry.path.display()
                    ));
                };
                incoming.entry(target).or_default().push(key);
            }
        }
        let mut ready: Vec<_> = outgoing
            .iter()
            .filter_map(|(&key, &count)| (count == 0).then_some(key))
            .collect();
        while let Some(key) = ready.pop() {
            outgoing.remove(key);
            for dependent in incoming.get(key).into_iter().flatten() {
                let count = outgoing.get_mut(*dependent).expect("unvisited dependent");
                *count -= 1;
                if *count == 0 {
                    ready.push(dependent);
                }
            }
        }
        if let Some(key) = outgoing.keys().next() {
            return Err(format!(
                "{key}: requires: dependency cycle or depends on a cycle"
            ));
        }
        Ok(())
    }
}

/// Record files in a registry directory, skipping staged `.tmp-*` files of interrupted writes.
fn record_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    for entry in fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))? {
        let path = entry.map_err(|e| format!("{}: {e}", dir.display()))?.path();
        if !path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with(".tmp-"))
        {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

pub(super) fn is_directory(path: &Path) -> Result<bool, String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => Ok(true),
        Ok(_) => Err(format!("{}: expected a directory", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!("{}: {error}", path.display())),
    }
}

pub(super) fn read_file(path: &Path) -> Result<String, String> {
    let metadata = fs::symlink_metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if !metadata.is_file() {
        return Err(format!("{}: expected a regular file", path.display()));
    }
    fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
}
