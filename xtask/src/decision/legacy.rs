//! Import the reviewed DecisionDelta slices (§2–4 of 10/11 and the §7 admin addendum of 11/11).
//! Later revisions go through proposals and owner signoffs, never through this importer.

use std::{collections::BTreeMap, path::Path, process::Command};

use serde::{Deserialize, Serialize};

use super::record::{self, Audience, Content, Entry, Key, Kind, Origin, VERSION};
use crate::checks::sha256_hex;

pub(super) const GROUP_ID: &str = "PRODUCT-DECISIONS-20261110";
pub(super) const ADMIN_GROUP_ID: &str = "ADMIN-MODERATION-20261111";
pub(super) const GROUPS: [&str; 2] = [GROUP_ID, ADMIN_GROUP_ID];
pub(super) const FILE: &str = "decisions/decision-delta-20261110.toml";
// develop after #165 added §7; §2–4 are byte-identical to the first import at 0c5bb27.
pub(super) const COMMIT: &str = "af3c34bd624be18d404bd5789620759cae0b0ddb";
pub(super) const SOURCE: &str = "docs/architecture/product-decisions-20261110.md";
// Independent of the editable TOML. Review changes to this pin and to the migration together.
// This checks a legacy import, never an actor, owner receipt, or PR approval.
pub(super) const IMPORT_SHA256: &str =
    "d6c1b35064ffdac9d3222e24c1c6d7314d07e9564711c617da0a2c3b4ccf7b07";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(super) enum Status {
    ProductConfirmed,
    Deferred,
    Open,
    DesignProposed,
    Provisional,
    /// Architecture only, and only through an owner signoff.
    Accepted,
    /// Signoff only: the owner declined this revision; it never becomes current.
    Rejected,
}

#[derive(Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Context {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    canonical_references: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    previous_rule: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    alternative: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    review_when: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    model_baseline: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    change_impact: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    core: Option<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Provenance {
    pub group: String,
    pub status: Status,
    pub historical: bool,
    pub source_line: usize,
    pub related_ids: Vec<String>,
    pub context: Context,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Row {
    id: String,
    revision: u32,
    content: Content,
    legacy: Provenance,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Import {
    version: u16,
    source_commit: String,
    source_path: String,
    records: Vec<Row>,
}

pub(super) fn verify_import(text: &str) -> Result<(), String> {
    let normalized = text.replace("\r\n", "\n");
    if sha256_hex(normalized.as_bytes()) != IMPORT_SHA256 {
        return Err(format!(
            "{FILE}: differs from the reviewed {COMMIT} import; use decision migrate --dry-run to compare"
        ));
    }
    Ok(())
}

pub(super) fn load(root: &Path) -> Result<BTreeMap<Key, Entry>, String> {
    let path = root.join(FILE);
    if !record::is_directory(&root.join("decisions"))? {
        return Err("decisions: missing registry directory".into());
    }
    let text = record::read_file(&path)?;
    let import: Import = record::decode(&path, &text)?;
    if import.version != VERSION {
        return Err(format!(
            "{FILE}: version: unsupported format {}",
            import.version
        ));
    }
    verify_import(&text)?;
    let mut entries = BTreeMap::new();
    for mut row in import.records {
        row.content.validate(&path, false)?;
        let hash = sha256_hex(record::encode(&row)?.as_bytes());
        entries.insert(
            Key::new(&row.id, row.revision),
            Entry {
                content: row.content,
                hash,
                path: path.clone(),
                origin: Origin::Legacy(Box::new(row.legacy)),
                signoff: None,
            },
        );
    }
    Ok(entries)
}

pub(super) fn render(root: &Path) -> Result<String, String> {
    let result = Command::new("git")
        .args([
            "--no-replace-objects",
            "show",
            &format!("{COMMIT}:{SOURCE}"),
        ])
        .current_dir(root)
        .env("GIT_NO_LAZY_FETCH", "1")
        .output()
        .map_err(|e| format!("read migration source with git: {e}"))?;
    if !result.status.success() {
        return Err(format!(
            "migration source {COMMIT}:{SOURCE} unavailable; use a checkout with that Git object"
        ));
    }
    let text = std::str::from_utf8(&result.stdout)
        .map_err(|e| format!("migration source must be UTF-8: {e}"))?;
    let mut import = Import {
        version: VERSION,
        source_commit: COMMIT.into(),
        source_path: SOURCE.into(),
        records: Vec::new(),
    };
    let mut counts = [0; 5];
    let mut section = "";
    let mut titles = BTreeMap::new();
    for (index, line) in text.lines().enumerate() {
        if line.starts_with("## ") || line.starts_with("### ") {
            section = line.split_whitespace().nth(1).unwrap_or_default();
        }
        let (table, prefix, expected_fields) = match section {
            "2." => (Table::Product, "| USER-", 6),
            "3." => (Table::Design, "| 0", 5),
            "4." => (Table::Open, "| O-", 4),
            "7.1." => (Table::Admin, "| ADM-", 6),
            "7.2." => (Table::AdminOpen, "| O-", 2),
            _ => continue,
        };
        if !line.starts_with(prefix) {
            continue;
        }
        let fields: Vec<_> = line.trim_matches('|').split('|').map(str::trim).collect();
        if fields.len() != expected_fields || fields.iter().any(|field| field.is_empty()) {
            return Err(format!(
                "{SOURCE}:{}: unexpected DecisionDelta row",
                index + 1
            ));
        }
        counts[table as usize] += 1;
        let rows = match table {
            Table::Admin => vec![admin_row(&fields, index + 1)?],
            Table::AdminOpen => vec![admin_open_row(&fields, &titles, index + 1)?],
            _ => import_row(&fields, table, index + 1)?,
        };
        for row in &rows {
            titles.insert(row.id.clone(), row.content.title.clone());
        }
        import.records.extend(rows);
    }
    if counts != [15, 4, 16, 10, 6] {
        return Err(format!(
            "{SOURCE}: expected 15 USER, 4 CHOICE, 16 O, 10 ADM and 6 O-update rows, got {counts:?}"
        ));
    }
    import
        .records
        .sort_by(|a, b| (&a.id, a.revision).cmp(&(&b.id, b.revision)));
    // A later revision in the source supersedes an earlier one of the same ID.
    for index in 1..import.records.len() {
        let (before, after) = import.records.split_at_mut(index);
        if before[index - 1].id == after[0].id {
            before[index - 1].legacy.historical = true;
        }
    }
    record::encode(&import)
}

#[derive(Clone, Copy, PartialEq)]
enum Table {
    Product,
    Design,
    Open,
    Admin,
    AdminOpen,
}

fn optional(field: &str) -> Option<String> {
    (field != "—").then(|| field.into())
}

// §7.1: Quyết định | ID canonical | Luật cũ | Luật mới | Model | Core. The source marks the
// rules PRODUCT_CONFIRMED and type names, rejection codes and technical baselines DESIGN_PROPOSED.
fn admin_row(fields: &[&str], source_line: usize) -> Result<Row, String> {
    let (id, title) = fields[0]
        .split_once(' ')
        .ok_or_else(|| format!("{SOURCE}:{source_line}: missing ID and title"))?;
    let domain = match id {
        "ADM-06" | "ADM-07" | "ADM-08" => "publication",
        "ADM-01" | "ADM-02" | "ADM-03" | "ADM-04" | "ADM-05" | "ADM-09" | "ADM-10" => "moderation",
        _ => return Err(format!("{SOURCE}:{source_line}: unmapped ID {id}")),
    };
    Ok(Row {
        id: id.into(),
        revision: 1,
        content: Content {
            kind: Kind::Product,
            domain: domain.into(),
            title: title.into(),
            resolution: fields[3].into(),
            rationale: None,
            requires: vec![],
            related_ids: vec![],
            work_audiences: vec![],
        },
        legacy: Provenance {
            group: ADMIN_GROUP_ID.into(),
            status: Status::ProductConfirmed,
            historical: false,
            source_line,
            related_ids: related_ids(fields[1]),
            context: Context {
                canonical_references: optional(fields[1]),
                previous_rule: optional(fields[2]),
                model: optional(fields[4]),
                core: optional(fields[5]),
                ..Context::default()
            },
        },
    })
}

// §7.2: # | Thay đổi. An existing O-ID gets its next revision; a new one starts at r1.
// “Đóng.” and an explicit `PRODUCT_CONFIRMED` close the question; everything else stays OPEN,
// including “Đóng một phần.”, whose remaining part is still a question.
fn admin_open_row(
    fields: &[&str],
    titles: &BTreeMap<String, String>,
    source_line: usize,
) -> Result<Row, String> {
    let (id, change) = (fields[0], fields[1]);
    let (domain, links, new_title) = match id {
        "O-03" | "O-05" => ("publication", spec_links(id), None),
        "O-15" => ("moderation", spec_links(id), None),
        "O-17" => (
            "publication",
            "MOD-13, PUB-05",
            Some("Trang ở URL cũ sau khi moderator bác bản đang đăng"),
        ),
        "O-18" => (
            "moderation",
            "MOD-11, MOD-15",
            Some("Thời hạn mặc định của grant và hồ sơ điều tra"),
        ),
        "O-19" => (
            "publication",
            "COM-11",
            Some("Danh mục sự kiện thông báo cho tác giả"),
        ),
        _ => return Err(format!("{SOURCE}:{source_line}: unmapped ID {id}")),
    };
    let (revision, title) = match (titles.get(id), new_title) {
        (Some(title), None) => (2, title.clone()),
        (None, Some(title)) => (1, title.into()),
        _ => {
            return Err(format!(
                "{SOURCE}:{source_line}: unexpected revision for {id}"
            ));
        }
    };
    let status = if change.starts_with("**Đóng.**") || change.contains("(`PRODUCT_CONFIRMED`)") {
        Status::ProductConfirmed
    } else {
        Status::Open
    };
    Ok(Row {
        id: id.into(),
        revision,
        content: Content {
            kind: Kind::Product,
            domain: domain.into(),
            title,
            resolution: change.into(),
            rationale: None,
            requires: vec![],
            related_ids: vec![],
            work_audiences: vec![],
        },
        legacy: Provenance {
            group: ADMIN_GROUP_ID.into(),
            status,
            historical: false,
            source_line,
            related_ids: related_ids(links),
            context: Context::default(),
        },
    })
}

fn import_row(fields: &[&str], table: Table, source_line: usize) -> Result<Vec<Row>, String> {
    let (id, title) = if table == Table::Open {
        (fields[0], fields[1])
    } else {
        fields[0]
            .split_once(' ')
            .ok_or_else(|| format!("{SOURCE}:{source_line}: missing ID and title"))?
    };
    let canonical = if table == Table::Design {
        format!("{GROUP_ID}-CHOICE-{id}")
    } else {
        id.into()
    };
    let domain = match id {
        "USER-01" => "audience-age",
        "USER-02" | "USER-07" | "O-03" | "O-04" | "O-05" | "O-08" | "O-09" | "O-10" => {
            "publication"
        }
        "USER-03" | "USER-05" | "USER-06" | "USER-14" | "O-15" => "moderation",
        "USER-04" | "USER-15" | "O-06" | "O-16" | "01" => "discussion",
        "USER-08" | "O-11" | "O-12" => "account",
        "USER-09" | "O-01" | "O-02" => "collections",
        "USER-10" => "curation",
        "USER-11" | "O-13" | "04" => "hashtags",
        "USER-12" | "USER-13" | "O-07" | "O-14" | "02" | "03" => "discovery",
        _ => return Err(format!("{SOURCE}:{source_line}: unmapped ID {id}")),
    };
    let (status, resolution, rationale, related_ids, context) = match table {
        Table::Product => (
            if fields[3].starts_with("`DEFERRED`") {
                Status::Deferred
            } else {
                Status::ProductConfirmed
            },
            fields[3],
            None,
            related_ids(fields[1]),
            Context {
                canonical_references: Some(fields[1].into()),
                previous_rule: Some(fields[2].into()),
                ..Context::default()
            },
        ),
        Table::Design => (
            Status::DesignProposed,
            fields[1],
            Some(fields[3].into()),
            related_ids(spec_links(id)),
            Context {
                alternative: Some(fields[2].into()),
                review_when: Some(fields[4].into()),
                ..Context::default()
            },
        ),
        Table::Open => (
            Status::Open,
            fields[1],
            None,
            related_ids(spec_links(id)),
            Context {
                model_baseline: Some(fields[2].into()),
                change_impact: Some(fields[3].into()),
                ..Context::default()
            },
        ),
        Table::Admin | Table::AdminOpen => {
            return Err(format!(
                "{SOURCE}:{source_line}: §7 rows use their own importer"
            ));
        }
    };
    let mut rows = Vec::new();
    if id == "USER-08" {
        // The old sign-in recommendation is explicitly marked Provisional in the source.
        // It is historical for the choice of sign-in method. D-05 still owns auth authority.
        rows.push(Row {
            id: canonical.clone(),
            revision: 1,
            content: Content {
                kind: Kind::Architecture,
                domain: domain.into(),
                title: "Phương thức đăng nhập được đề xuất trước 10/11/2026".into(),
                resolution: fields[2].into(),
                rationale: None,
                requires: vec![],
                related_ids: vec![],
                work_audiences: vec![],
            },
            legacy: Provenance {
                group: GROUP_ID.into(),
                status: Status::Provisional,
                historical: true,
                source_line,
                related_ids: related_ids.clone(),
                context: Context::default(),
            },
        });
    }
    rows.push(Row {
        id: canonical,
        revision: if id == "USER-08" { 2 } else { 1 },
        content: Content {
            kind: if table == Table::Design {
                Kind::Architecture
            } else {
                Kind::Product
            },
            domain: domain.into(),
            title: title.into(),
            resolution: resolution.into(),
            rationale,
            requires: vec![],
            related_ids: vec![],
            work_audiences: if id == "USER-02" {
                vec![Audience::Private, Audience::Public]
            } else {
                vec![]
            },
        },
        legacy: Provenance {
            group: GROUP_ID.into(),
            status,
            historical: false,
            source_line,
            related_ids,
            context,
        },
    });
    Ok(rows)
}

// Cross-references from PRD §14 and its domain specs at COMMIT. These link decisions to
// acceptance criteria without treating the criterion or the model baseline as the decision.
fn spec_links(id: &str) -> &'static str {
    match id {
        "01" | "O-16" => "COM-10",
        "02" | "O-14" => "RANK-01, RANK-04",
        "03" => "TOP-04",
        "04" => "HTG-01, HTG-04, HTG-05",
        "O-01" => "COL-06",
        "O-02" => "COL-05",
        "O-03" | "O-04" | "O-05" => "PUB-04, PUB-05",
        "O-06" => "COM-10, LVE-FR-04",
        "O-07" => "RANK-02, LVE-FR-01",
        "O-08" | "O-10" => "PUB-03, PUB-04",
        "O-09" => "PUB-04",
        "O-11" | "O-12" => "ACC-03",
        "O-13" => "HTG-06",
        "O-15" => "MOD-10",
        _ => "",
    }
}

// The legacy column contains requirement IDs and ranges as well as prose (vision §2, §12).
// These are many-to-many links. They never turn a D/requirement ID into a decision alias.
fn related_ids(text: &str) -> Vec<String> {
    let mut ids = Vec::new();
    for token in text
        .split([';', ',', ' '])
        .filter(|token| !token.is_empty())
    {
        let (first, end) = token.split_once("..").unwrap_or((token, ""));
        let Some((prefix, start)) = first.rsplit_once('-') else {
            continue;
        };
        let Ok(start_number) = start.parse::<u16>() else {
            continue;
        };
        if record::validate_id(first).is_err() {
            continue;
        }
        if let Ok(end) = end.parse::<u16>() {
            ids.extend(
                (start_number..=end).map(|n| format!("{prefix}-{n:0width$}", width = start.len())),
            );
        } else {
            ids.push(first.into());
        }
    }
    ids.sort();
    ids.dedup();
    ids
}
