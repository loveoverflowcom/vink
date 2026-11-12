use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        Barrier,
        atomic::{AtomicU64, Ordering},
    },
    thread,
};

use serde_json::Value;

use super::{
    decision_log, dev_choices, legacy, publish,
    record::{PROPOSALS, SIGNOFFS},
    run,
};

const DRAFT: &str = r#"kind = "product"
domain = "publication"
title = "Quyền đọc tiếng Việt"
resolution = """
Dòng một.
Dòng hai.
"""
rationale = "Lý do"
"#;

struct Sandbox(PathBuf);

impl Sandbox {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = loop {
            let path = std::env::temp_dir().join(format!(
                "vink-decision-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&path) {
                Ok(()) => break path,
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => panic!("create test directory: {e}"),
            }
        };
        let sandbox = Self(path);
        for file in [legacy::FILE, decision_log::FILE, dev_choices::FILE] {
            sandbox.write(file, &fs::read_to_string(crate::root().join(file)).unwrap());
        }
        sandbox
    }

    fn write(&self, path: &str, data: &str) -> PathBuf {
        let path = self.0.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, data).unwrap();
        path
    }

    fn command(&self, args: &[&str]) -> Result<String, String> {
        let mut out = Vec::new();
        run(&self.0, args, &mut out)?;
        Ok(String::from_utf8(out).unwrap())
    }

    fn json(&self, args: &[&str]) -> Value {
        serde_json::from_str(&self.command(args).unwrap()).unwrap()
    }

    fn propose(&self, id: &str, base: &str, path: &Path) -> Result<String, String> {
        self.command(&["propose", id, base, path.to_str().unwrap()])
    }

    fn proposed(&self, id: &str, base: &str, path: &Path) -> String {
        let output: Value = serde_json::from_str(&self.propose(id, base, path).unwrap()).unwrap();
        output["sha256"].as_str().unwrap().into()
    }

    fn signoff(&self, pin: &str, digest: &str, outcome: &str) -> Result<String, String> {
        let path = self.write("signoff.toml", outcome);
        self.command(&["signoff", pin, digest, path.to_str().unwrap()])
    }

    fn head(&self, id: &str) -> String {
        let value = self.json(&["query", id]);
        let head = &value["result"]["head"];
        format!("{}:{}", head["revision"], head["sha256"].as_str().unwrap())
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn imported_status_history_and_related_ids_resolve_without_git() {
    let sandbox = Sandbox::new();
    for (id, status) in [
        ("USER-02", "PRODUCT_CONFIRMED"),
        ("USER-01", "DEFERRED"),
        ("USER-10", "DEFERRED"),
        ("O-12", "OPEN"),
        ("PRODUCT-DECISIONS-20261110-CHOICE-01", "DESIGN_PROPOSED"),
    ] {
        let result = &sandbox.json(&["query", id])["result"];
        assert_eq!(result["lookup"], "current");
        assert_eq!(result["record"]["status"], status);
        assert_eq!(result["record"]["source"]["commit"], legacy::COMMIT);
    }
    let bundle = sandbox.json(&["query", legacy::GROUP_ID]);
    assert_eq!(bundle["result"]["lookup"], "group");
    assert_eq!(bundle["result"]["revisions"].as_array().unwrap().len(), 36);
    let admin = sandbox.json(&["query", legacy::ADMIN_GROUP_ID]);
    assert_eq!(admin["result"]["revisions"].as_array().unwrap().len(), 16);
    // §7 of 11/11 closes O-05 and revises O-03/O-15 without rewriting their 10/11 revision.
    for (id, status) in [
        ("O-03", "PRODUCT_CONFIRMED"),
        ("O-05", "PRODUCT_CONFIRMED"),
        ("O-15", "OPEN"),
    ] {
        let current = &sandbox.json(&["query", id])["result"]["record"];
        assert_eq!(current["revision"], 2);
        assert_eq!(current["status"], status);
        assert_eq!(current["source"]["group"], legacy::ADMIN_GROUP_ID);
        let first = &sandbox.json(&["query", &format!("{id}@r1")])["result"]["record"];
        assert_eq!(first["status"], "OPEN");
        assert_eq!(first["historical"], true);
        assert_eq!(first["source"]["group"], legacy::GROUP_ID);
    }
    for (id, status) in [("ADM-04", "PRODUCT_CONFIRMED"), ("O-17", "OPEN")] {
        let current = &sandbox.json(&["query", id])["result"]["record"];
        assert_eq!(current["revision"], 1);
        assert_eq!(current["status"], status);
    }
    let deletion = sandbox.json(&["query", "PUB-04"]);
    let related = deletion["result"]["related"].as_array().unwrap();
    assert!(
        related
            .iter()
            .any(|r| r["id"] == "O-05" && r["revision"] == 2)
    );
    assert!(
        !related
            .iter()
            .any(|r| r["revision"] == 1 && r["id"] == "O-05")
    );
    assert!(related.iter().any(|r| r["id"] == "ADM-08"));
    let account = sandbox.json(&["query", "ACC-03"]);
    assert_eq!(account["result"]["related"].as_array().unwrap().len(), 3);
    assert!(
        account["result"]["related"]
            .as_array()
            .unwrap()
            .iter()
            .any(|record| record["id"] == "O-12" && record["status"] == "OPEN")
    );
    let history = sandbox.json(&["query", "USER-08", "--history"]);
    assert_eq!(history["result"]["revisions"][0]["status"], "PROVISIONAL");
    assert_eq!(history["result"]["revisions"][0]["historical"], true);
    assert_eq!(
        history["result"]["revisions"][1]["status"],
        "PRODUCT_CONFIRMED"
    );
    assert_eq!(
        sandbox.json(&["query", "USER-08"])["result"]["record"]["revision"],
        2
    );
    assert_eq!(
        sandbox.json(&["query", "D-07"])["result"]["related"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        sandbox.json(&["query", "COL-06"])["result"]["lookup"],
        "related_only"
    );
    assert_eq!(
        sandbox.json(&["query", "CHOICE-01"])["result"]["lookup"],
        "unknown"
    );
    assert!(
        sandbox.json(&["query", "--domain", "publication"])["result"]["decisions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|decision| decision["record"]["id"] == "O-03")
    );
    assert_eq!(
        sandbox.json(&["query"])["result"]["revisions"]
            .as_array()
            .unwrap()
            .len(),
        52 + 32 + 27
    );
}

#[test]
fn decision_log_import_keeps_source_status_and_one_authority_per_d_id() {
    let sandbox = Sandbox::new();
    let group = sandbox.json(&["query", decision_log::GROUP_ID]);
    let revisions = group["result"]["revisions"].as_array().unwrap();
    assert_eq!(revisions.len(), 32);
    for record in revisions {
        // The importer never promotes: no ACCEPTED, and every status quotes its source phrase.
        assert_ne!(record["status"], "ACCEPTED");
        assert!(record["context"]["source_status"].as_str().is_some());
        assert_eq!(record["source"]["commit"], decision_log::COMMIT);
        assert!(
            record["source"]["path"]
                .as_str()
                .unwrap()
                .starts_with("docs/architecture/")
        );
    }
    for (id, status, kind) in [
        ("D-05", "PROVISIONAL", "architecture"),
        ("D-08", "DESIGN_PROPOSED", "architecture"),
        ("D-12", "DESIGN_PROPOSED", "architecture"),
        ("D-13", "PRODUCT_CONFIRMED", "product"),
        ("D-14", "DESIGN_PROPOSED", "architecture"),
        ("LVE-PR-01-CHOICE-02", "DESIGN_PROPOSED", "architecture"),
        ("TAG-PR-01-CHOICE-02", "DESIGN_PROPOSED", "architecture"),
        ("DISC-01-CHOICE-01", "DESIGN_PROPOSED", "architecture"),
    ] {
        let result = &sandbox.json(&["query", id])["result"];
        assert_eq!(result["lookup"], "current", "{id}");
        assert_eq!(result["record"]["status"], status, "{id}");
        assert_eq!(result["record"]["content"]["kind"], kind, "{id}");
    }
    // The 10/11 sign-in method stays with USER-08@r2; D-05 keeps only the authority decision.
    let auth = &sandbox.json(&["query", "D-05"])["result"]["record"];
    assert_eq!(auth["content"]["requires"][0], "USER-08@r2");
    assert!(
        !auth["content"]["resolution"]
            .as_str()
            .unwrap()
            .contains("#152")
    );
    // D-02 keeps both source statuses as two scoped records.
    let ownership = sandbox.json(&["query", "D-02"]);
    let parts: Vec<_> = ownership["result"]["related"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| (r["id"].as_str().unwrap(), r["status"].as_str().unwrap()))
        .collect();
    assert_eq!(
        parts,
        [
            ("D-02-CONTENT", "DESIGN_PROPOSED"),
            ("D-02-DISCUSSION", "PRODUCT_CONFIRMED")
        ]
    );
    // D-IDs fully owned by DecisionDelta records resolve to them, not to a second copy.
    for (id, owner) in [
        ("D-06", "USER-02"),
        ("D-09", "USER-10"),
        ("D-10", "USER-09"),
        ("D-11", "USER-01"),
        ("D-16", "USER-09"),
        ("D-17", "USER-11"),
        ("D-18", "USER-13"),
        ("D-19", "USER-14"),
    ] {
        let result = &sandbox.json(&["query", id])["result"];
        assert_eq!(result["lookup"], "related_only", "{id}");
        assert!(
            result["related"]
                .as_array()
                .unwrap()
                .iter()
                .all(|r| r["source"]["group"] != decision_log::GROUP_ID),
            "{id}"
        );
        assert!(
            result["related"]
                .as_array()
                .unwrap()
                .iter()
                .any(|r| r["id"] == owner),
            "{id}"
        );
    }
    assert_eq!(
        sandbox.json(&["query", "D-01"])["result"]["lookup"],
        "unknown"
    );
    // An imported revision keeps its source status; a change needs a proposal and a signoff.
    let digest = auth["sha256"].as_str().unwrap();
    assert!(
        sandbox
            .signoff(
                "D-05@r1",
                digest,
                &CONFIRMED.replace("PRODUCT_CONFIRMED", "ACCEPTED")
            )
            .unwrap_err()
            .contains("imported revisions keep their source status")
    );
    let input = sandbox.write(
        "draft.toml",
        &DRAFT.replace(
            "kind = \"product\"\ndomain = \"publication\"",
            "kind = \"architecture\"\ndomain = \"account\"",
        ),
    );
    let head = sandbox.head("D-05");
    sandbox.propose("D-05", &head, &input).unwrap();
    let pending = sandbox.json(&["query", "D-05"]);
    assert_eq!(pending["result"]["record"]["revision"], 1);
    assert_eq!(pending["result"]["head"]["revision"], 2);
}

#[test]
fn dev_choices_import_keeps_source_status_and_reproducible_provenance() {
    let sandbox = Sandbox::new();
    let group = sandbox.json(&["query", dev_choices::GROUP_ID]);
    let revisions = group["result"]["revisions"].as_array().unwrap();
    assert_eq!(revisions.len(), 27);
    for record in revisions {
        assert_ne!(record["status"], "ACCEPTED");
        assert!(record["context"]["source_status"].as_str().is_some());
        assert_eq!(record["source"]["commit"], dev_choices::COMMIT);
    }
    for (id, status, kind, domain) in [
        ("D-05-C1", "PROVISIONAL", "architecture", "account"),
        ("D-05-C4", "PROVISIONAL", "architecture", "account"),
        (
            "DISC-WEB-CHOICE-01",
            "DESIGN_PROPOSED",
            "architecture",
            "discussion",
        ),
        (
            "LVE-WP-04-CHOICE-07",
            "DESIGN_PROPOSED",
            "architecture",
            "endorsement",
        ),
        (
            "READ-01-CHOICE-01",
            "DESIGN_PROPOSED",
            "architecture",
            "platform",
        ),
        (
            "READ-02-CHOICE-01",
            "DESIGN_PROPOSED",
            "architecture",
            "platform",
        ),
        (
            "TAG-PR-02-CHOICE-01",
            "DESIGN_PROPOSED",
            "architecture",
            "taxonomy",
        ),
        (
            "TAG-PR-03-CHOICE-01",
            "DESIGN_PROPOSED",
            "architecture",
            "taxonomy",
        ),
        (
            "TAG-PR-04-CHOICE-01",
            "DESIGN_PROPOSED",
            "architecture",
            "taxonomy",
        ),
        (
            "TAG-PR-05-CHOICE-01",
            "DESIGN_PROPOSED",
            "architecture",
            "taxonomy",
        ),
    ] {
        let result = &sandbox.json(&["query", id])["result"];
        assert_eq!(result["lookup"], "current", "{id}");
        assert_eq!(result["record"]["status"], status, "{id}");
        assert_eq!(result["record"]["content"]["kind"], kind, "{id}");
        assert_eq!(result["record"]["content"]["domain"], domain, "{id}");
    }
}

#[test]
fn pinned_imports_render_byte_identical_when_their_sources_are_available() {
    // CI uses a shallow checkout without the pinned objects; render needs them.
    for (render, file) in [
        (
            legacy::render as fn(&Path) -> Result<String, String>,
            legacy::FILE,
        ),
        (decision_log::render, decision_log::FILE),
        (dev_choices::render, dev_choices::FILE),
    ] {
        match render(&crate::root()) {
            Ok(text) => assert_eq!(
                text,
                fs::read_to_string(crate::root().join(file)).unwrap(),
                "{file}"
            ),
            Err(error) => assert!(error.contains("unavailable"), "{error}"),
        }
    }
}

#[test]
fn semantic_diff_reports_fields_and_pending_cannot_replace_current() {
    let sandbox = Sandbox::new();
    let input = sandbox.write(
        "draft.toml",
        &format!("{DRAFT}work_audiences = [\"private\"]\n"),
    );
    let head = sandbox.head("USER-02");
    sandbox.propose("USER-02", &head, &input).unwrap();

    let lookup = sandbox.json(&["query", "USER-02"]);
    assert_eq!(lookup["result"]["record"]["revision"], 1);
    assert_eq!(lookup["result"]["head"]["revision"], 2);
    let pending = sandbox.json(&["query", "USER-02@r2"]);
    assert_eq!(pending["result"]["record"]["status"], "PENDING");
    assert_eq!(
        pending["result"]["record"]["work_audience_comparison"]["result"],
        "conflicts"
    );
    assert_eq!(
        pending["result"]["record"]["content"]["resolution"],
        "Dòng một.\nDòng hai.\n"
    );
    let diff = sandbox.json(&["diff", "USER-02@r1", "USER-02@r2"]);
    let changes = diff["changes"].as_array().unwrap();
    assert!(
        changes
            .iter()
            .any(|change| change["field"] == "content.resolution")
    );
    assert!(
        changes
            .iter()
            .any(|change| change["field"] == "status" && change["after"] == "PENDING")
    );
    assert!(
        !changes
            .iter()
            .any(|change| change["field"] == "content.domain")
    );
    assert_eq!(
        sandbox.json(&["diff", "USER-02@r2", "USER-02@r2"])["changes"],
        serde_json::json!([])
    );

    let open = sandbox.head("O-12");
    sandbox
        .propose(
            "O-12",
            &open,
            &sandbox.write("account.toml", &DRAFT.replace("publication", "account")),
        )
        .unwrap();
    let lookup = sandbox.json(&["query", "O-12"]);
    assert_eq!(lookup["result"]["record"]["status"], "OPEN");
    assert_eq!(lookup["result"]["record"]["revision"], 1);
}

#[test]
fn rejects_malformed_inputs_and_self_attestations() {
    let sandbox = Sandbox::new();
    for (input, message) in [
        ("[broken", "unclosed table"),
        (r#"kind = "operator""#, "kind"),
        (
            &format!("{DRAFT}authority = \"product_owner\""),
            "authority",
        ),
        (&format!("{DRAFT}status = \"PRODUCT_CONFIRMED\""), "status"),
        (&format!("{DRAFT}approval = \"self\""), "approval"),
        (
            &format!("{DRAFT}approval = {{ scope = \"other\", payload = \"old\" }}"),
            "approval",
        ),
        (&format!("{DRAFT}title = \"duplicate\""), "duplicate"),
        (
            &format!("{DRAFT}work_audiences = [\"private\", \"private\"]"),
            "work_audiences",
        ),
        (
            &format!("{DRAFT}work_audiences = [\"unlisted\"]"),
            "work_audiences",
        ),
        (
            &format!("{DRAFT}requires = [\"USER-02@r1\", \"USER-02@r1\"]"),
            "requires",
        ),
        (
            &format!("{DRAFT}related_ids = [\"ACC-03\", \"ACC-03\"]"),
            "related_ids",
        ),
        (&format!("{DRAFT}related_ids = [\"acc 03\"]"), "related_ids"),
        (
            &DRAFT.replace("\"Lý do\"", "2026-11-11T12:00:00Z"),
            "rationale",
        ),
        (&DRAFT.replace("\"Lý do\"", "inf"), "rationale"),
        (&DRAFT.replace("\"Lý do\"", "null"), "invalid"),
        (&DRAFT.replace("rationale = \"Lý do\"", ""), "rationale"),
        (&DRAFT.replace("publication", "../publication"), "domain"),
    ] {
        let path = sandbox.write("draft.toml", input);
        let error = sandbox.propose("VDR-TEST", "new", &path).unwrap_err();
        assert!(
            error.contains("draft.toml") && error.contains(message),
            "{error}"
        );
    }
    let input = sandbox.write("draft.toml", DRAFT);
    assert!(
        sandbox
            .propose("../OUTSIDE", "new", &input)
            .unwrap_err()
            .contains("invalid decision ID")
    );
    assert!(
        sandbox
            .propose("D-99", "new", &input)
            .unwrap_err()
            .contains("VDR- ID")
    );
    assert!(
        sandbox
            .command(&["apply", "USER-02@r1", "--confirm"])
            .is_err()
    );
    assert!(!sandbox.0.join(PROPOSALS).exists());
}

#[test]
fn altered_import_and_proposal_schema_are_rejected() {
    let sandbox = Sandbox::new();
    let path = sandbox.0.join(legacy::FILE);
    let legacy = fs::read_to_string(&path).unwrap();
    fs::write(
        &path,
        legacy.replace("status = \"OPEN\"", "status = \"PRODUCT_CONFIRMED\""),
    )
    .unwrap();
    assert!(
        sandbox
            .command(&["query", "O-12"])
            .unwrap_err()
            .contains("differs from the reviewed")
    );
    fs::write(&path, legacy.replace("version = 1", "version = 2")).unwrap();
    assert!(
        sandbox
            .command(&["validate"])
            .unwrap_err()
            .contains("version")
    );
    fs::write(&path, legacy).unwrap();

    let path = sandbox.0.join(decision_log::FILE);
    let imported = fs::read_to_string(&path).unwrap();
    fs::write(
        &path,
        imported.replacen("status = \"PROVISIONAL\"", "status = \"ACCEPTED\"", 1),
    )
    .unwrap();
    assert!(
        sandbox
            .command(&["query", "D-05"])
            .unwrap_err()
            .contains("differs from the reviewed")
    );
    fs::remove_file(&path).unwrap();
    assert!(sandbox.command(&["validate"]).is_err());
    fs::write(&path, imported).unwrap();

    let path = sandbox.0.join(dev_choices::FILE);
    let dev = fs::read_to_string(&path).unwrap();
    fs::write(
        &path,
        dev.replacen("status = \"PROVISIONAL\"", "status = \"ACCEPTED\"", 1),
    )
    .unwrap();
    assert!(
        sandbox
            .command(&["query", "D-05-C1"])
            .unwrap_err()
            .contains("differs from the reviewed")
    );
    fs::remove_file(&path).unwrap();
    assert!(sandbox.command(&["validate"]).is_err());
    fs::write(&path, dev).unwrap();

    let input = sandbox.write("draft.toml", DRAFT);
    sandbox.propose("VDR-TEST", "new", &input).unwrap();
    let path = sandbox.0.join(PROPOSALS).join("VDR-TEST.r1.toml");
    let saved = fs::read_to_string(&path).unwrap();
    fs::write(&path, saved.replace("version = 1", "version = 9")).unwrap();
    assert!(
        sandbox
            .command(&["validate"])
            .unwrap_err()
            .contains("unsupported format 9")
    );
    fs::write(&path, saved.replace("revision = 1", "revision = -1")).unwrap();
    assert!(
        sandbox
            .command(&["validate"])
            .unwrap_err()
            .contains("revision")
    );
    fs::write(&path, format!("approval = \"copied\"\n{saved}")).unwrap();
    assert!(
        sandbox
            .command(&["validate"])
            .unwrap_err()
            .contains("unknown field `approval`")
    );
}

#[test]
fn compare_and_swap_uses_revision_and_payload_and_supports_multiple_digits() {
    let sandbox = Sandbox::new();
    let input = sandbox.write("draft.toml", DRAFT);
    sandbox.propose("VDR-TEST", "new", &input).unwrap();
    let stale = sandbox.head("VDR-TEST");
    let forged = format!("1:{}", "0".repeat(64));
    assert!(
        sandbox
            .propose("VDR-TEST", &forged, &input)
            .unwrap_err()
            .contains("base: expected")
    );
    for revision in 2..=11 {
        let head = sandbox.head("VDR-TEST");
        sandbox.propose("VDR-TEST", &head, &input).unwrap();
        let lookup = sandbox.json(&["query", "VDR-TEST"]);
        assert_eq!(lookup["result"]["head"]["revision"], revision);
        assert_eq!(lookup["result"]["lookup"], "pending_only");
    }
    assert!(
        sandbox
            .propose("VDR-TEST", &stale, &input)
            .unwrap_err()
            .contains("base: expected")
    );
    // Editing an earlier payload invalidates its successor's stored base.
    let first = sandbox.0.join(PROPOSALS).join("VDR-TEST.r1.toml");
    let changed = fs::read_to_string(&first)
        .unwrap()
        .replace("Dòng một.", "Dòng đã đổi.");
    fs::write(first, changed).unwrap();
    assert!(
        sandbox
            .command(&["validate"])
            .unwrap_err()
            .contains("VDR-TEST@r2: base")
    );
}

#[test]
fn same_base_writers_do_not_overwrite_and_disjoint_writes_both_land() {
    let sandbox = Sandbox::new();
    let input = sandbox.write("draft.toml", DRAFT);
    sandbox.propose("VDR-RACE", "new", &input).unwrap();
    let base = sandbox.head("VDR-RACE");
    let other = sandbox.write("other.toml", &DRAFT.replace("Dòng một.", "Bản khác."));
    let barrier = Barrier::new(2);
    let race = |left, right, left_base, right_base| {
        thread::scope(|scope| {
            let first = scope.spawn(|| {
                barrier.wait();
                sandbox.propose(left, left_base, &input)
            });
            barrier.wait();
            let second = sandbox.propose(right, right_base, &other);
            [first.join().unwrap(), second]
        })
    };
    let results = race("VDR-RACE", "VDR-RACE", &base, &base);
    assert_eq!(
        results.iter().filter(|result| result.is_ok()).count(),
        1,
        "{results:?}"
    );
    assert!(
        results
            .iter()
            .filter_map(|result| result.as_ref().err())
            .all(|error| error.contains("base: expected")
                || error.contains("revision already exists"))
    );
    assert_eq!(
        sandbox.json(&["query", "VDR-RACE", "--history"])["result"]["revisions"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert!(
        race("VDR-LEFT", "VDR-RIGHT", "new", "new")
            .iter()
            .all(Result::is_ok)
    );
    sandbox.command(&["validate"]).unwrap();
}

#[test]
fn publication_failure_leaves_original_and_cleans_staged_file() {
    let sandbox = Sandbox::new();
    let path = sandbox.write("stage/final.toml", "winner");
    assert!(
        publish(&path, b"discard")
            .unwrap_err()
            .contains("already exists")
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), "winner");
    assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
    sandbox.write(&format!("{PROPOSALS}/.tmp-interrupted"), "partial content");
    sandbox.command(&["validate"]).unwrap();
}

#[test]
fn missing_and_cyclic_dependencies_fail_and_never_publish_candidate() {
    let sandbox = Sandbox::new();
    let input = sandbox.write(
        "draft.toml",
        &format!("{DRAFT}requires = [\"VDR-OTHER@r1\"]"),
    );
    assert!(
        sandbox
            .propose("VDR-TEST", "new", &input)
            .unwrap_err()
            .contains("requires: missing")
    );
    let input = sandbox.write(
        "draft.toml",
        &format!("{DRAFT}requires = [\"VDR-TEST@r1\"]"),
    );
    assert!(
        sandbox
            .propose("VDR-TEST", "new", &input)
            .unwrap_err()
            .contains("dependency cycle")
    );
    assert!(!sandbox.0.join(PROPOSALS).exists());

    let input = sandbox.write("draft.toml", DRAFT);
    sandbox.propose("VDR-ONE", "new", &input).unwrap();
    let input = sandbox.write("draft.toml", &format!("{DRAFT}requires = [\"VDR-ONE@r1\"]"));
    sandbox.propose("VDR-TWO", "new", &input).unwrap();
    let path = sandbox.0.join(PROPOSALS).join("VDR-ONE.r1.toml");
    let saved = fs::read_to_string(&path).unwrap();
    fs::write(&path, format!("{saved}\nrequires = [\"VDR-TWO@r1\"]\n")).unwrap();
    assert!(
        sandbox
            .command(&["validate"])
            .unwrap_err()
            .contains("dependency cycle")
    );
}

#[test]
fn historical_ref_is_valid_and_active_ref_must_target_current_import() {
    let sandbox = Sandbox::new();
    sandbox.write(
        "docs/consumer.md",
        "<!-- vdr: historical USER-08@r1 -->\n<!-- vdr: active USER-08@r2 -->\n",
    );
    let output = sandbox.json(&["refs"]);
    assert_eq!(output["references"].as_array().unwrap().len(), 2);
    assert_eq!(output["semantic_review"], "NOT_CHECKED");
    sandbox.write(
        "prd/stale.md",
        "<!-- vdr: active USER-08@r1 -->\n<!-- vdr: active USER-08@r9 -->\n",
    );
    let error = sandbox.command(&["refs"]).unwrap_err();
    assert!(error.contains("prd/stale.md:1: stale active reference USER-08@r1"));
    assert!(error.contains("prd/stale.md:2: unknown VDR reference USER-08@r9"));

    let input = sandbox.write("draft.toml", DRAFT);
    let base = sandbox.head("USER-02");
    sandbox.propose("USER-02", &base, &input).unwrap();
    sandbox.write("prd/stale.md", "<!-- vdr: active USER-02@r2 -->\n");
    assert!(
        sandbox
            .command(&["refs"])
            .unwrap_err()
            .contains("pending or rejected proposal")
    );
    sandbox.write("prd/stale.md", "<!-- vdr: active USER-02@r1 -->\n");
    sandbox.command(&["validate"]).unwrap();
}

#[test]
fn snapshot_and_toml_roundtrip_are_deterministic() {
    let first = Sandbox::new();
    let second = Sandbox::new();
    let draft_one = first.write("draft.toml", &format!("{DRAFT}requires = [\"O-03@r1\", \"USER-02@r1\"]\nwork_audiences = [\"public\", \"private\"]"));
    let draft_two = second.write("draft.toml", &format!("# scratch note\n{DRAFT}requires = [\"USER-02@r1\", \"O-03@r1\"]\nwork_audiences = [\"private\", \"public\"]"));
    first.propose("VDR-REBUILD", "new", &draft_one).unwrap();
    second.propose("VDR-REBUILD", "new", &draft_two).unwrap();
    let path = format!("{PROPOSALS}/VDR-REBUILD.r1.toml");
    let raw = fs::read_to_string(first.0.join(&path)).unwrap();
    assert_eq!(raw, fs::read_to_string(second.0.join(&path)).unwrap());
    let decoded: super::record::Proposal = super::record::decode(Path::new(&path), &raw).unwrap();
    assert_eq!(raw, super::record::encode(&decoded).unwrap());
    assert_eq!(
        first.command(&["query"]).unwrap(),
        second.command(&["query"]).unwrap()
    );
}

const CONFIRMED: &str = r#"status = "PRODUCT_CONFIRMED"
decided_on = "2026-11-11"
source = "Chủ sản phẩm, chat 11/11/2026: “đồng ý bản này”"
"#;

#[test]
fn signoff_binds_the_exact_payload_and_only_then_changes_current() {
    let sandbox = Sandbox::new();
    let account = sandbox.write(
        "account.toml",
        &format!(
            "{}related_ids = [\"ACC-03\"]\n",
            DRAFT.replace("publication", "account")
        ),
    );
    let base = sandbox.head("O-12");
    let digest = sandbox.proposed("O-12", &base, &account);
    let legacy_digest = sandbox.json(&["query", "USER-02"])["result"]["record"]["sha256"]
        .as_str()
        .unwrap()
        .to_owned();
    let zero = "0".repeat(64);
    for (pin, sha, outcome, message) in [
        (
            "O-12@r2",
            zero.as_str(),
            CONFIRMED,
            "sha256: signoff is for payload",
        ),
        ("O-12@r2", "ABC", CONFIRMED, "lowercase SHA-256"),
        ("O-12@r9", digest.as_str(), CONFIRMED, "unknown revision"),
        (
            "USER-02@r1",
            legacy_digest.as_str(),
            CONFIRMED,
            "imported revisions",
        ),
        (
            "O-12@r2",
            digest.as_str(),
            &CONFIRMED.replace("PRODUCT_CONFIRMED", "ACCEPTED"),
            "not a signoff outcome",
        ),
        (
            "O-12@r2",
            digest.as_str(),
            &CONFIRMED.replace("PRODUCT_CONFIRMED", "PENDING"),
            "status",
        ),
        (
            "O-12@r2",
            digest.as_str(),
            &CONFIRMED.replace("2026-11-11", "2026-02-30"),
            "decided_on",
        ),
        (
            "O-12@r2",
            digest.as_str(),
            &CONFIRMED.replace("\"2026-11-11\"", "2026-11-11T00:00:00Z"),
            "decided_on",
        ),
        (
            "O-12@r2",
            digest.as_str(),
            &format!("{CONFIRMED}authority = \"product_owner\""),
            "unknown field `authority`",
        ),
        (
            "O-12@r2",
            digest.as_str(),
            &format!("{CONFIRMED}sha256 = \"{zero}\""),
            "unknown field `sha256`",
        ),
        (
            "O-12@r2",
            digest.as_str(),
            &CONFIRMED.replace("Chủ sản phẩm, chat 11/11/2026: “đồng ý bản này”", " "),
            "source",
        ),
    ] {
        let error = sandbox.signoff(pin, sha, outcome).unwrap_err();
        assert!(error.contains(message), "{pin} {message}: {error}");
    }
    assert!(!sandbox.0.join(SIGNOFFS).exists());
    assert_eq!(
        sandbox.json(&["query", "O-12"])["result"]["record"]["revision"],
        1
    );

    let output: Value =
        serde_json::from_str(&sandbox.signoff("O-12@r2", &digest, CONFIRMED).unwrap()).unwrap();
    let traced = sandbox.json(&["query", "ACC-03"]);
    assert!(
        traced["result"]["related"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["id"] == "O-12" && r["revision"] == 2)
    );
    assert_eq!(output["current"], true);
    let current = sandbox.json(&["query", "O-12"]);
    assert_eq!(current["result"]["record"]["revision"], 2);
    assert_eq!(current["result"]["record"]["status"], "PRODUCT_CONFIRMED");
    assert_eq!(
        current["result"]["record"]["signoff"]["decided_on"],
        "2026-11-11"
    );
    assert_eq!(
        current["result"]["record"]["source"]["signoff_path"],
        format!("{SIGNOFFS}/O-12.r2.toml")
    );
    assert!(
        sandbox
            .signoff("O-12@r2", &digest, CONFIRMED)
            .unwrap_err()
            .contains("already has a signoff")
    );
    // Markdown can pin the signed revision; the imported one is now stale for active use.
    sandbox.write(
        "docs/auth.md",
        "<!-- vdr: active O-12@r2 -->\n<!-- vdr: historical O-12@r1 -->\n",
    );
    sandbox.command(&["validate"]).unwrap();
    sandbox.write("docs/old.md", "<!-- vdr: active O-12@r1 -->\n");
    assert!(
        sandbox
            .command(&["refs"])
            .unwrap_err()
            .contains("stale active reference O-12@r1; current is O-12@r2")
    );
    fs::remove_file(sandbox.0.join("docs/old.md")).unwrap();

    // A later revision stays pending; a rejection never makes it current.
    let head = sandbox.head("O-12");
    let next = sandbox.proposed("O-12", &head, &account);
    sandbox.write("docs/next.md", "<!-- vdr: historical O-12@r3 -->\n");
    assert!(
        sandbox
            .command(&["refs"])
            .unwrap_err()
            .contains("pending or rejected")
    );
    sandbox
        .signoff(
            "O-12@r3",
            &next,
            &CONFIRMED.replace("PRODUCT_CONFIRMED", "REJECTED"),
        )
        .unwrap();
    assert!(
        sandbox
            .command(&["refs"])
            .unwrap_err()
            .contains("pending or rejected")
    );
    fs::remove_file(sandbox.0.join("docs/next.md")).unwrap();
    let lookup = sandbox.json(&["query", "O-12"]);
    assert_eq!(lookup["result"]["record"]["revision"], 2);
    assert_eq!(lookup["result"]["head"]["revision"], 3);
    assert_eq!(
        sandbox.json(&["query", "O-12@r3"])["result"]["record"]["status"],
        "REJECTED"
    );
    let diff = sandbox.json(&["diff", "O-12@r2", "O-12@r3"]);
    assert!(
        diff["changes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|change| change["field"] == "status" && change["after"] == "REJECTED")
    );

    // Architecture outcomes differ from product ones; a rejected-only ID has no current record.
    let architecture = sandbox.write(
        "arch.toml",
        &DRAFT.replace("\"product\"", "\"architecture\""),
    );
    let arch = sandbox.proposed("VDR-ARCH", "new", &architecture);
    assert!(
        sandbox
            .signoff("VDR-ARCH@r1", &arch, CONFIRMED)
            .unwrap_err()
            .contains("not a signoff outcome")
    );
    sandbox
        .signoff(
            "VDR-ARCH@r1",
            &arch,
            &CONFIRMED.replace("PRODUCT_CONFIRMED", "ACCEPTED"),
        )
        .unwrap();
    assert_eq!(
        sandbox.json(&["query", "VDR-ARCH"])["result"]["record"]["status"],
        "ACCEPTED"
    );
    let input = sandbox.write("draft.toml", DRAFT);
    let declined = sandbox.proposed("VDR-NO", "new", &input);
    sandbox
        .signoff(
            "VDR-NO@r1",
            &declined,
            &CONFIRMED.replace("PRODUCT_CONFIRMED", "REJECTED"),
        )
        .unwrap();
    assert_eq!(
        sandbox.json(&["query", "VDR-NO"])["result"]["lookup"],
        "no_current"
    );

    // Editing a signed payload, or the digest a signoff names, invalidates the signoff.
    let path = sandbox.0.join(PROPOSALS).join("VDR-ARCH.r1.toml");
    let saved = fs::read_to_string(&path).unwrap();
    fs::write(&path, saved.replace("Dòng một.", "Dòng đã đổi.")).unwrap();
    assert!(
        sandbox
            .command(&["validate"])
            .unwrap_err()
            .contains("VDR-ARCH@r1: sha256: signoff is for payload")
    );
    fs::write(&path, saved).unwrap();
    let signed = sandbox.0.join(SIGNOFFS).join("VDR-NO.r1.toml");
    let record = fs::read_to_string(&signed).unwrap();
    fs::write(&signed, record.replace(&declined, &zero)).unwrap();
    assert!(
        sandbox
            .command(&["validate"])
            .unwrap_err()
            .contains("VDR-NO@r1: sha256")
    );
    fs::write(&signed, record.replace("revision = 1", "revision = 2")).unwrap();
    assert!(
        sandbox
            .command(&["validate"])
            .unwrap_err()
            .contains("does not match signoff filename")
    );
    fs::write(&signed, record).unwrap();
    sandbox.write(&format!("{SIGNOFFS}/.tmp-interrupted"), "partial");
    sandbox.command(&["validate"]).unwrap();
}
