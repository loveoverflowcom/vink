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

use super::{legacy, publish, record::PROPOSALS, run};

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
        sandbox.write(
            legacy::FILE,
            &fs::read_to_string(crate::root().join(legacy::FILE)).unwrap(),
        );
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
    assert_eq!(bundle["result"]["related"].as_array().unwrap().len(), 35);
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
        36
    );
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

    let open = sandbox.head("O-03");
    sandbox.propose("O-03", &open, &input).unwrap();
    assert_eq!(
        sandbox.json(&["query", "O-03"])["result"]["record"]["status"],
        "OPEN"
    );
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
            .propose("D-05", "new", &input)
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
            .contains("pending proposal")
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
