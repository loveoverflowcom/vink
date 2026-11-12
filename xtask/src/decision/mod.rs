//! Repository decision queries and proposals. Read-only queries work in a shallow checkout.

mod decision_log;
mod dev_choices;
mod legacy;
mod record;
mod refs;
#[cfg(test)]
mod tests;

use std::{
    collections::BTreeSet,
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use serde_json::{Value, json};

use record::{
    Base, Content, Entry, Key, Origin, Proposal, Registry, Signoff, SignoffInput, VERSION,
};

const HELP: &str = "usage: cargo xtask decision <command>

  validate                           validate the registry and declared references
  query                              export all revisions as JSON
  query ID[@rN]                      look up current or an exact revision
  query ID --history                 list all revisions of an exact ID
  query --domain DOMAIN              look up decisions in a domain
  diff ID@rN ID@rM                   compare the changed fields of two revisions
  propose ID new INPUT.toml          append a pending proposal for a new VDR-* ID
  propose ID REV:SHA256 INPUT.toml   append a pending revision at the expected head
  signoff ID@rN SHA256 INPUT.toml    record the owner's outcome for that exact proposal payload
  refs                               report declared Markdown reference freshness
  migrate --dry-run [GROUP]          render a pinned import to stdout (default: DecisionDelta)
  migrate [GROUP]                    install a pinned import once, without overwriting
                                     GROUP: PRODUCT-DECISIONS-20261110, DECISION-LOG-20261111
                                     or DECISION-DEV-20261112

See decisions/README.md for format, coverage and the approval boundary.";

pub fn run(root: &Path, args: &[&str], out: &mut impl Write) -> Result<(), String> {
    match args {
        [] | ["--help"] => writeln!(out, "{HELP}").map_err(io_error),
        ["migrate", rest @ ..] => {
            let (dry_run, group) = match rest {
                [] => (false, legacy::GROUP_ID),
                ["--dry-run"] => (true, legacy::GROUP_ID),
                [group] if !group.starts_with('-') => (false, *group),
                ["--dry-run", group] => (true, *group),
                _ => return Err("expected migrate [--dry-run] [GROUP]".into()),
            };
            migrate(root, dry_run, group, out)
        }
        ["validate"] => {
            let registry = Registry::load(root)?;
            let references = refs::report(root, &registry)?;
            references.validate()?;
            writeln!(
                out,
                "{} revisions; snapshot {}; {} declared references; semantic review is separate",
                registry.entries.len(),
                registry.snapshot,
                references.len()
            )
            .map_err(io_error)
        }
        ["refs"] => {
            let registry = Registry::load(root)?;
            let references = refs::report(root, &registry)?;
            print_json(out, &references.output(&registry))?;
            references.validate()
        }
        ["query", rest @ ..] => print_json(out, &query(&Registry::load(root)?, rest)?),
        ["diff", old, new] => print_json(out, &diff(&Registry::load(root)?, old, new)?),
        ["propose", id, base, input] => propose(root, id, base, Path::new(input), out),
        ["signoff", pin, digest, input] => signoff(root, pin, digest, Path::new(input), out),
        _ => Err("unknown decision arguments; run cargo xtask decision --help".into()),
    }
}

pub fn check(root: &Path) -> Result<(), String> {
    let registry = Registry::load(root)?;
    refs::report(root, &registry)?.validate()
}

fn query(registry: &Registry, args: &[&str]) -> Result<Value, String> {
    let result = match args {
        [] => json!({
            "revisions": registry.entries.iter().map(|(key, entry)| describe(registry, key, entry)).collect::<Vec<_>>(),
        }),
        ["--domain", domain] if record::valid_domain(domain) => {
            let ids: BTreeSet<_> = registry
                .entries
                .iter()
                .filter_map(|(key, entry)| {
                    (entry.content.domain == *domain).then_some(key.id.as_str())
                })
                .collect();
            json!({ "domain": domain, "decisions": ids.into_iter().map(|id| lookup(registry, id)).collect::<Vec<_>>() })
        }
        [id, "--history"] => {
            record::validate_id(id)?;
            let history: Vec<_> = registry
                .history(id)
                .map(|(key, entry)| describe(registry, key, entry))
                .collect();
            json!({ "id": id, "lookup": if history.is_empty() { "unknown" } else { "history" }, "revisions": history })
        }
        [pin] if pin.contains('@') => {
            let key = Key::parse(pin)?;
            match registry.entries.get(&key) {
                Some(entry) => {
                    json!({ "lookup": "revision", "record": describe(registry, &key, entry) })
                }
                None => json!({ "lookup": "unknown", "requested": pin }),
            }
        }
        [id] => {
            record::validate_id(id)?;
            lookup(registry, id)
        }
        _ => return Err("expected query [ID[@rN] | ID --history | --domain DOMAIN]".into()),
    };
    Ok(json!({ "version": VERSION, "snapshot_sha256": registry.snapshot, "result": result }))
}

fn lookup(registry: &Registry, id: &str) -> Value {
    let head = registry
        .history(id)
        .next_back()
        .map(|(key, entry)| entry.base(key));
    if let Some((key, entry)) = registry.current(id) {
        return json!({ "lookup": "current", "record": describe(registry, key, entry), "head": head });
    }
    if let Some((key, entry)) = registry.history(id).next_back() {
        let lookup = if entry.signoff.is_some() {
            "no_current"
        } else {
            "pending_only"
        };
        return json!({ "lookup": lookup, "id": id, "current": null, "head": head, "head_record": describe(registry, key, entry) });
    }
    // An import group lists every revision it contained, superseded ones included.
    if legacy::GROUPS.contains(&id) || id == decision_log::GROUP_ID || id == dev_choices::GROUP_ID {
        let revisions: Vec<_> = registry
            .entries
            .iter()
            .filter(
                |(_, entry)| matches!(&entry.origin, Origin::Legacy(source) if source.group == id),
            )
            .map(|(key, entry)| describe(registry, key, entry))
            .collect();
        return json!({ "lookup": "group", "id": id, "current": null, "revisions": revisions });
    }
    // Requirement and D-IDs are many-to-many links; list the current records that cite them.
    let related: Vec<_> = registry
        .entries
        .iter()
        .filter(|(key, entry)| {
            entry.related_ids().any(|item| item == id)
                && registry
                    .current(&key.id)
                    .is_some_and(|(current, _)| current == *key)
        })
        .map(|(key, entry)| describe(registry, key, entry))
        .collect();
    json!({
        "lookup": if related.is_empty() { "unknown" } else { "related_only" },
        "id": id,
        "current": null,
        "related": related,
    })
}

fn describe(registry: &Registry, key: &Key, entry: &Entry) -> Value {
    let mut value = entry.fields();
    value["id"] = json!(key.id);
    value["revision"] = json!(key.revision);
    value["sha256"] = json!(entry.hash);
    let file = |dir: &str, path: &Path| {
        format!(
            "{dir}/{}",
            path.file_name().unwrap_or_default().to_string_lossy()
        )
    };
    value["source"] = match &entry.origin {
        Origin::Legacy(source) if source.group == dev_choices::GROUP_ID => json!({
            "kind": "legacy_import",
            "group": source.group,
            "path": source.source_path,
            "commit": dev_choices::COMMIT,
            "line": source.source_line,
        }),
        Origin::Legacy(source) if source.group == decision_log::GROUP_ID => json!({
            "kind": "legacy_import",
            "group": source.group,
            "path": source.source_path,
            "commit": decision_log::COMMIT,
            "line": source.source_line,
        }),
        Origin::Legacy(source) => json!({
            "kind": "legacy_import",
            "group": source.group,
            "path": legacy::SOURCE,
            "commit": legacy::COMMIT,
            "line": source.source_line,
        }),
        Origin::Proposal(_) => json!({
            "kind": "proposal",
            "path": file(record::PROPOSALS, &entry.path),
            "signoff_path": entry.signoff.as_ref().map(|signed| file(record::SIGNOFFS, &signed.path)),
        }),
    };
    // Compare only the structured work-audience field. Free text and other domains need review.
    if matches!(entry.origin, Origin::Proposal(_))
        && !entry.content.work_audiences.is_empty()
        && let Some((current, baseline)) = registry.current("USER-02")
    {
        value["work_audience_comparison"] = json!({
            "against": current.to_string(),
            "result": if entry.content.work_audiences == baseline.content.work_audiences {
                "matches"
            } else {
                "conflicts"
            },
        });
    }
    value
}

fn diff(registry: &Registry, old: &str, new: &str) -> Result<Value, String> {
    let from = Key::parse(old)?;
    let to = Key::parse(new)?;
    if from.id != to.id {
        return Err("compare two revisions of the same decision ID".into());
    }
    let before = registry
        .entries
        .get(&from)
        .ok_or_else(|| format!("unknown revision {from}"))?
        .fields();
    let after = registry
        .entries
        .get(&to)
        .ok_or_else(|| format!("unknown revision {to}"))?
        .fields();
    let mut changes = Vec::new();
    compare("", &before, &after, &mut changes);
    Ok(json!({
        "snapshot_sha256": registry.snapshot,
        "from": old,
        "to": new,
        "changes": changes,
    }))
}

fn compare(path: &str, before: &Value, after: &Value, changes: &mut Vec<Value>) {
    if before == after {
        return;
    }
    if let (Some(before), Some(after)) = (before.as_object(), after.as_object()) {
        let fields: BTreeSet<_> = before.keys().chain(after.keys()).collect();
        for key in fields {
            let field = if path.is_empty() {
                key.clone()
            } else {
                format!("{path}.{key}")
            };
            compare(
                &field,
                before.get(key).unwrap_or(&Value::Null),
                after.get(key).unwrap_or(&Value::Null),
                changes,
            );
        }
    } else {
        changes.push(json!({ "field": path, "before": before, "after": after }));
    }
}

fn propose(
    root: &Path,
    id: &str,
    expected: &str,
    input: &Path,
    out: &mut impl Write,
) -> Result<(), String> {
    record::validate_id(id)?;
    let base = Base::parse(expected)?;
    let revision = match &base {
        Some(base) => base.revision.checked_add(1).ok_or("revision overflow")?,
        None => 1,
    };
    let mut registry = Registry::load(root)?;
    let mut content: Content = record::decode(input, &record::read_file(input)?)?;
    content.validate(input, true)?;
    let proposal = Proposal {
        version: VERSION,
        id: id.into(),
        revision,
        base,
        content,
    };
    let dir = root.join(record::PROPOSALS);
    let path = dir.join(proposal.filename());
    let key = Key::new(id, revision);
    registry.check_next(&key, &proposal.base, &proposal.content)?;
    let canonical = record::encode(&proposal)?;
    let hash = crate::checks::sha256_hex(canonical.as_bytes());
    let base = proposal.base;
    registry.entries.insert(
        key.clone(),
        Entry {
            content: proposal.content,
            hash: hash.clone(),
            path: path.clone(),
            origin: Origin::Proposal(base),
            signoff: None,
        },
    );
    registry.check_dependencies()?;
    match fs::create_dir(&dir) {
        Ok(()) => {}
        Err(error)
            if error.kind() == io::ErrorKind::AlreadyExists && record::is_directory(&dir)? => {}
        Err(error) => return Err(format!("{}: {error}", dir.display())),
    }
    publish(&path, canonical.as_bytes())?;
    print_json(
        out,
        &json!({ "proposal": key.to_string(), "status": "PENDING", "sha256": hash }),
    )
}

// The CLI checks payload binding and integrity. It cannot authenticate who decided: callers run
// it only for the owner's explicit words about this exact revision, quoted in `source`.
fn signoff(
    root: &Path,
    pin: &str,
    digest: &str,
    input: &Path,
    out: &mut impl Write,
) -> Result<(), String> {
    let key = Key::parse(pin)?;
    let digest = record::parse_digest(digest)?;
    let registry = Registry::load(root)?;
    let outcome: SignoffInput = record::decode(input, &record::read_file(input)?)?;
    let mut signoff = Signoff {
        version: VERSION,
        id: key.id.clone(),
        revision: key.revision,
        sha256: digest.into(),
        status: outcome.status,
        decided_on: outcome.decided_on,
        source: outcome.source,
    };
    let dir = root.join(record::SIGNOFFS);
    let path = dir.join(signoff.filename());
    // Validate the input file's fields under its own name before checking the target.
    signoff.input().validate(
        input,
        registry
            .entries
            .get(&key)
            .map_or(record::Kind::Product, |e| e.content.kind),
    )?;
    registry.attach(&path, &mut signoff)?;
    let canonical = record::encode(&signoff)?;
    match fs::create_dir(&dir) {
        Ok(()) => {}
        Err(error)
            if error.kind() == io::ErrorKind::AlreadyExists && record::is_directory(&dir)? => {}
        Err(error) => return Err(format!("{}: {error}", dir.display())),
    }
    publish(&path, canonical.as_bytes())?;
    let effective = registry
        .current(&key.id)
        .is_none_or(|(current, _)| current.revision < key.revision)
        && signoff.status != legacy::Status::Rejected;
    print_json(
        out,
        &json!({
            "signoff": key.to_string(),
            "status": signoff.status,
            "sha256": signoff.sha256,
            "current": effective,
        }),
    )
}

type Verify = fn(&str) -> Result<(), String>;

fn migrate(root: &Path, dry_run: bool, group: &str, out: &mut impl Write) -> Result<(), String> {
    let (text, verify, file): (String, Verify, &str) = match group {
        legacy::GROUP_ID => (legacy::render(root)?, legacy::verify_import, legacy::FILE),
        decision_log::GROUP_ID => (
            decision_log::render(root)?,
            decision_log::verify_import,
            decision_log::FILE,
        ),
        dev_choices::GROUP_ID => (
            dev_choices::render(root)?,
            dev_choices::verify_import,
            dev_choices::FILE,
        ),
        _ => {
            return Err(format!(
                "unknown import group {group:?}; expected {}, {} or {}",
                legacy::GROUP_ID,
                decision_log::GROUP_ID,
                dev_choices::GROUP_ID
            ));
        }
    };
    if dry_run {
        return out.write_all(text.as_bytes()).map_err(io_error);
    }
    verify(&text)?;
    if !record::is_directory(&root.join("decisions"))? {
        return Err("decisions: missing registry directory".into());
    }
    let path = root.join(file);
    match fs::symlink_metadata(&path) {
        Ok(_) => {
            verify(&record::read_file(&path)?)?;
            writeln!(out, "{file} already matches the pinned import").map_err(io_error)
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            publish(&path, text.as_bytes())?;
            writeln!(out, "installed {file}").map_err(io_error)
        }
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

fn print_json(out: &mut impl Write, value: &Value) -> Result<(), String> {
    serde_json::to_writer_pretty(&mut *out, value)
        .map_err(|e| format!("write decision output: {e}"))?;
    writeln!(out).map_err(io_error)
}

fn io_error(error: io::Error) -> String {
    format!("write decision output: {error}")
}

struct StagedFile(PathBuf);

impl Drop for StagedFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

// A closed, synced inode becomes visible at the final path in one no-clobber operation.
// If concurrent writers choose the same revision, only one hard link can succeed.
fn publish(path: &Path, bytes: &[u8]) -> Result<(), String> {
    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);
    let dir = path.parent().expect("record has a parent directory");
    let (mut file, staged) = loop {
        let counter = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let temp = dir.join(format!(".tmp-{}-{counter}", std::process::id()));
        match fs::File::create_new(&temp) {
            Ok(file) => break (file, StagedFile(temp)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(format!("{}: {error}", temp.display())),
        }
    };
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|e| format!("stage {}: {e}", path.display()))?;
    drop(file);
    fs::hard_link(&staged.0, path).map_err(|error| {
        if error.kind() == io::ErrorKind::AlreadyExists {
            format!(
                "{}: revision already exists; query history and retry",
                path.display()
            )
        } else {
            format!("publish {}: {error}", path.display())
        }
    })
}
