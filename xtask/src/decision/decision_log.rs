//! Import the decisions of the D-log and the domain ADRs at one pinned commit (#164, CHOICE-07).
//!
//! Each imported sentence is a verbatim excerpt that `render` must find in the pinned source,
//! either a whole table cell or a quote inside a named block. The status is the one the source
//! records, quoted in `context.source_status`. Splitting a row into decision and code/evidence
//! snapshot only selects excerpts: it never rewords a rule, never promotes a status and never
//! imports `ACCEPTED`. Titles of issue-local choices are labels, not rules. Later changes go
//! through proposals and owner signoffs, never through this importer.

use std::{collections::BTreeMap, path::Path, process::Command};

use serde::{Deserialize, Serialize};

use super::{
    legacy::{self, Context, Provenance, Row, Status},
    record::{self, Content, Entry, Key, Kind, Origin, VERSION},
};
use crate::checks::sha256_hex;

pub(super) const GROUP_ID: &str = "DECISION-LOG-20261111";
pub(super) const FILE: &str = "decisions/decision-log-20261111.toml";
// develop after #166 merged the TOML registry; the D-log and ADRs below are read at this commit.
pub(super) const COMMIT: &str = "73ab92f0064674833ec31bc7533b210149b26b79";
// Independent of the editable TOML. Review changes to this pin and to SPECS together.
pub(super) const IMPORT_SHA256: &str =
    "04e5543039b5c72a210f35a3511f8581cbc9e42f159841046ed19092341251c4";

const LOG: &str = "docs/architecture/decisions.md";
const LVE: &str = "docs/architecture/literary-endorsement.md";
const TAG: &str = "docs/architecture/work-taxonomy.md";
const ADMISSION: &str = "docs/architecture/discussion-admission.md";
const READS: &str = "docs/architecture/discussion-reads.md";
const OUTBOX: &str = "docs/architecture/discussion-outbox.md";

// Blocks that state the scope and status of each ADR's issue-local choices.
const LVE_PR1: &str = "**Ngày:** 05/11/2026.";
const LVE_API: &str = "Chọn cho lát gộp LVE-PR-02/03";
const LVE_OPEN: &str = "**Còn OPEN (tại PR1):**";
const TAG_PR1: &str = "**Ngày:** 08/11/2026.";
const DISC_01: &str = "**Ngày:** 09/11/2026.";
const DISC_10: &str = "**Ngày:** 10/11/2026.";

/// One excerpt of the pinned source. `Cell` reads a cell of the first block, which must be a
/// table row. `Quote` must occur verbatim in one of the blocks (soft line breaks read as spaces).
/// `Label` names a record and is accepted only as a title.
#[derive(Clone, Copy)]
pub(super) enum Part {
    Cell(usize),
    Quote(&'static str),
    Label(&'static str),
}

pub(super) use Part::{Cell, Label, Quote};

#[derive(Clone, Copy)]
pub(super) struct Spec {
    pub id: &'static str,
    pub kind: Kind,
    pub domain: &'static str,
    pub status: Status,
    pub source: &'static str,
    /// Line prefixes, each matching exactly one line: a table row, a heading (until the next
    /// heading of the same or a higher level) or a paragraph (until the next blank line).
    pub blocks: &'static [&'static str],
    pub title: Part,
    pub source_status: Part,
    pub resolution: &'static [Part],
    pub rationale: &'static [Part],
    pub alternative: &'static [Part],
    pub review_when: &'static [Part],
    pub requires: &'static [&'static str],
    pub related_ids: &'static [&'static str],
    /// Owner documents that keep the split-off spec, runtime snapshot and evidence.
    pub references: &'static str,
}

pub(super) const D_ROW: Spec = Spec {
    id: "",
    kind: Kind::Architecture,
    domain: "platform",
    status: Status::DesignProposed,
    source: LOG,
    blocks: &[],
    title: Cell(1),
    source_status: Cell(2),
    resolution: &[Cell(1)],
    rationale: &[],
    alternative: &[],
    review_when: &[],
    requires: &[],
    related_ids: &[],
    references: "",
};

/// An issue-local choice row: ID | choice | reason, recorded by one slice as an implementation
/// choice that has not been accepted.
pub(super) const fn choice(
    id: &'static str,
    domain: &'static str,
    source: &'static str,
    blocks: &'static [&'static str],
    title: &'static str,
    source_status: &'static str,
    related_ids: &'static [&'static str],
) -> Spec {
    Spec {
        id,
        kind: Kind::Architecture,
        domain,
        status: Status::DesignProposed,
        source,
        blocks,
        title: Label(title),
        source_status: Quote(source_status),
        resolution: &[Cell(1)],
        rationale: &[Cell(2)],
        alternative: &[],
        review_when: &[],
        requires: &[],
        related_ids,
        references: "",
    }
}

const LVE_PR1_STATUS: &str = "Đây là lựa chọn triển khai có phạm vi **pure core PR1**";
const LVE_API_STATUS: &str =
    "Chọn cho lát gộp LVE-PR-02/03 (05/11/2026), port lên seam trên; không đổi semantics lõi.";
const TAG_STATUS: &str = "**Trạng thái:** lựa chọn triển khai cho **pure core** của App";
const DISC_01_STATUS: &str = "Đây là lựa chọn triển khai có phạm vi **pure core**";
const DISC_04_STATUS: &str = "Lát này là **hạ tầng bền + một consumer shadow/audit**.";

static SPECS: &[Spec] = &[
    // D-02 records two statuses in one row; each part keeps its own.
    Spec {
        id: "D-02-CONTENT",
        blocks: &["| D-02 |", "### Quyết định 1 — Ai sở hữu tác phẩm?"],
        title: Quote("Vink sở hữu content"),
        source_status: Quote("Content: đề xuất ưu tiên"),
        resolution: &[Quote(
            "**Đề xuất:** Vink sở hữu Work/Chapter/Revision/Publication trong PostgreSQL.",
        )],
        rationale: &[Quote(
            "Lợi ích là làm chủ bản nháp song song với bản công khai, quyền xuất bản, tuyển chọn và export. Chi phí là Vink phải xây một màn vận hành tác phẩm riêng và chịu trách nhiệm về revision schema, backup, migration.",
        )],
        related_ids: &["D-02"],
        references: "docs/architecture/decisions.md §1; docs/architecture/tech-stack.md",
        ..D_ROW
    },
    Spec {
        id: "D-02-DISCUSSION",
        kind: Kind::Product,
        domain: "discussion",
        status: Status::ProductConfirmed,
        blocks: &["| D-02 |", "### Quyết định 1 — Ai sở hữu tác phẩm?"],
        title: Quote("thảo luận do Vink sở hữu (Rust core + PostgreSQL App)"),
        source_status: Quote("Thảo luận: **đã chốt 03/11/2026**"),
        resolution: &[
            Quote(
                "**Đã chốt 03/11/2026 (chủ sản phẩm, phần thảo luận):** Vink cũng sở hữu bình luận bằng Rust core và PostgreSQL của App. Discourse chỉ là nguồn tham khảo thiết kế, API, kiểm duyệt và trải nghiệm cộng đồng; không là dependency runtime hay nơi lưu bình luận",
            ),
            Cell(3),
        ],
        related_ids: &["D-02"],
        references: "docs/architecture/discussion-ownership.md",
        ..D_ROW
    },
    Spec {
        id: "D-03",
        blocks: &["| D-03 |"],
        resolution: &[Cell(1), Cell(3)],
        references: "docs/architecture/tech-stack.md; docs/development/README.md",
        ..D_ROW
    },
    Spec {
        id: "D-04",
        blocks: &["| D-04 |"],
        review_when: &[Cell(3)],
        related_ids: &["S-01"],
        references: "docs/architecture/decisions.md §6 (S-01)",
        ..D_ROW
    },
    // The 10/11 sign-in method is USER-08@r2; the code snapshot, production path and blockers
    // stay with the ADR and the runbook.
    Spec {
        id: "D-05",
        domain: "account",
        status: Status::Provisional,
        blocks: &["| D-05 |"],
        source_status: Quote("**Đề xuất A cho pilot (Provisional, 07/11/2026)**; chưa Accepted."),
        resolution: &[
            Quote(
                "Authority native trong `vink-identity`, phiên opaque, binding/đình chỉ ở App không đổi.",
            ),
            Quote(
                "App giữ principal/binding/đình chỉ; Keycloak và Supabase/Clerk không chọn cho pilot.",
            ),
            Quote("Không còn DiscourseConnect"),
        ],
        review_when: &[
            Quote(
                "Blocker còn: chủ sản phẩm chấp nhận UX, gửi thật qua Resend + deliverability, edge/WAF, qualification trình duyệt trên HTTPS thật + native",
            ),
            Quote("review bảo mật độc lập, người chịu trách nhiệm auth"),
        ],
        requires: &["USER-08@r2"],
        related_ids: &["S-03"],
        references: "docs/architecture/auth-authority.md §6, §9, §11, §12; docs/development/auth-operations.md; docs/development/auth-web.md",
        ..D_ROW
    },
    // Moderation by the evaluator is USER-04 and editing one's own comment is USER-15; the
    // `premoderation.v1` snapshot stays with the admission ADR and the discussion registry.
    Spec {
        id: "D-08",
        domain: "discussion",
        blocks: &["| D-08 |"],
        source_status: Quote("Cần spike"),
        resolution: &[
            Cell(1),
            Quote("Nếu edit gate chưa đạt, chặn edit cả UI/API trong pilot."),
        ],
        requires: &["USER-04@r1", "USER-15@r1"],
        related_ids: &["COM-02", "COM-10"],
        references: "docs/architecture/discussion-admission.md §5, §8; docs/architecture/comment-editing.md; docs/development/discussion-core.md",
        ..D_ROW
    },
    // The four deletion cases are USER-07 and the hidden page ADM-07/08; retention and purge
    // stay OPEN in O-08.
    Spec {
        id: "D-12",
        domain: "publication",
        blocks: &["| D-12 |"],
        source_status: Quote("Cần chốt policy + spike"),
        resolution: &[Quote(
            "Vink chặn đọc tác phẩm và thảo luận ngay vì cùng owner (nay chỉ đúng cho riêng tư/gỡ do kiểm duyệt); cache/excerpt có trạng thái riêng; không báo hoàn tất trước khi xác minh.",
        )],
        requires: &["ADM-07@r1", "ADM-08@r1", "USER-07@r1"],
        related_ids: &["PUB-04", "PUB-05"],
        references: "docs/architecture/decisions.md §5; docs/architecture/publication-moderation.md §6",
        ..D_ROW
    },
    Spec {
        id: "D-13",
        kind: Kind::Product,
        domain: "discussion",
        status: Status::ProductConfirmed,
        blocks: &["| D-13 |"],
        resolution: &[Cell(1), Cell(3)],
        related_ids: &["COM-07", "COM-08"],
        references: "docs/architecture/discussion-ownership.md §4",
        ..D_ROW
    },
    // Ranking inputs are USER-12/13 and O-06/O-07; the slice choices are LVE-* records below.
    Spec {
        id: "D-14",
        domain: "endorsement",
        blocks: &["| D-14 |"],
        resolution: &[Cell(1), Quote("không đổi COM-07/08 hoặc luật #79")],
        references: "docs/architecture/literary-endorsement.md; docs/development/endorsement-core.md; docs/development/literary-endorsement-runtime.md",
        ..D_ROW
    },
    // Hashtags stay separate from the curated catalog under USER-11; the slice choices are
    // TAG-* records below and in the development registries.
    Spec {
        id: "D-15",
        domain: "taxonomy",
        blocks: &["| D-15 |", "Phân loại tác phẩm [#95]"],
        resolution: &[
            Cell(1),
            Quote("Thẻ không cấp quyền, không xuất bản hay tuyển chọn"),
        ],
        review_when: &[Quote(
            "bắt buộc dạng bài khi gửi duyệt và duyệt lại snapshot metadata còn OPEN; curator theo người (grant/HTTP) chờ authority production",
        )],
        references: "docs/architecture/work-taxonomy.md; docs/development/taxonomy-core.md",
        ..D_ROW
    },
    choice(
        "LVE-PR-01-CHOICE-01",
        "endorsement",
        LVE,
        &["| CHOICE-01 |", LVE_PR1],
        "#73 CHOICE-01, lát LVE-PR-01",
        LVE_PR1_STATUS,
        &["D-14"],
    ),
    choice(
        "LVE-PR-01-CHOICE-02",
        "endorsement",
        LVE,
        &["| CHOICE-02 |", LVE_PR1],
        "#73 CHOICE-02, lát LVE-PR-01",
        LVE_PR1_STATUS,
        &["D-14"],
    ),
    choice(
        "LVE-PR-01-CHOICE-03",
        "endorsement",
        LVE,
        &["| CHOICE-03 | B", LVE_PR1],
        "#73 CHOICE-03 (core ordering/retry), lát LVE-PR-01",
        LVE_PR1_STATUS,
        &["D-14"],
    ),
    choice(
        "LVE-PR-01-CHOICE-08",
        "endorsement",
        LVE,
        &["| CHOICE-08 |", LVE_PR1],
        "#73 CHOICE-08, lát LVE-PR-01",
        LVE_PR1_STATUS,
        &["D-14"],
    ),
    choice(
        "LVE-PR-01-CHOICE-09",
        "endorsement",
        LVE,
        &["| CHOICE-09 |", LVE_PR1],
        "#73 CHOICE-09, lát LVE-PR-01",
        LVE_PR1_STATUS,
        &["D-14"],
    ),
    choice(
        "LVE-PR-01-CHOICE-11",
        "endorsement",
        LVE,
        &["| CHOICE-11 |", LVE_PR1],
        "#73 CHOICE-11, lát LVE-PR-01",
        LVE_PR1_STATUS,
        &["D-14"],
    ),
    choice(
        "LVE-PR-02-03-CHOICE-03",
        "endorsement",
        LVE,
        &["| CHOICE-03 (HTTP) |", LVE_API],
        "#73 CHOICE-03 (HTTP), lát API + PostgreSQL #81 (LVE-PR-02/03)",
        LVE_API_STATUS,
        &["D-14"],
    ),
    choice(
        "LVE-PR-02-03-CHOICE-04",
        "endorsement",
        LVE,
        &["| CHOICE-04 |", LVE_API],
        "#73 CHOICE-04, lát API + PostgreSQL #81 (LVE-PR-02/03)",
        LVE_API_STATUS,
        &["D-14"],
    ),
    choice(
        "LVE-PR-02-03-CHOICE-05",
        "endorsement",
        LVE,
        &["| CHOICE-05 |", LVE_API],
        "#73 CHOICE-05, lát API + PostgreSQL #81 (LVE-PR-02/03)",
        LVE_API_STATUS,
        &["D-14"],
    ),
    choice(
        "LVE-PR-02-03-CHOICE-12",
        "endorsement",
        LVE,
        &["| CHOICE-12 |", LVE_API],
        "#73 CHOICE-12, lát API + PostgreSQL #81 (LVE-PR-02/03)",
        LVE_API_STATUS,
        &["D-14"],
    ),
    choice(
        "LVE-PR-02-03-CHOICE-13",
        "endorsement",
        LVE,
        &["| CHOICE-13 |", LVE_API],
        "#73 CHOICE-13, lát API + PostgreSQL #81 (LVE-PR-02/03)",
        LVE_API_STATUS,
        &["D-14"],
    ),
    choice(
        "LVE-WP-04-CHOICE-10",
        "endorsement",
        LVE,
        &["| CHOICE-10 |", LVE_OPEN],
        "#73 CHOICE-10, lát Web adapter WP-04",
        "10 chốt A ở [lát Web adapter](#lát-web-adapter-wp-04-06112026)",
        &["D-14"],
    ),
    Spec {
        id: "TAG-PR-01-CHOICE-01",
        domain: "taxonomy",
        source: TAG,
        blocks: &["### CHOICE-01 — Taxonomy pilot", TAG_PR1],
        title: Label("#95 CHOICE-01 taxonomy pilot, lát TAG-PR-01"),
        source_status: Quote(TAG_STATUS),
        resolution: &[
            Quote(
                "Một catalog, mỗi thẻ thuộc đúng một facet trong enum **đóng** `{Form, Theme}`; luật số lượng là hằng trong code, không phải engine nhóm do dữ liệu định nghĩa.",
            ),
            Quote(
                "Số lượng: **dạng bài ≤ 1** (bằng type `Option`), **chủ đề ≤ 3** thẻ khác nhau sau khi resolve alias và khử trùng (`MAX_THEMES_PER_WORK`).",
            ),
            Quote(
                "Dạng bài **không bắt buộc** ở nháp; có bắt buộc khi gửi duyệt hay không là quyết định của owner publication.",
            ),
        ],
        rationale: &[Quote(
            "Tái dùng toàn bộ luật danh tính; facet đóng nên không thành engine taxonomy tổng quát; thêm facet là thay đổi code được review",
        )],
        alternative: &[
            Quote("A. Trường dạng bài riêng + thẻ chủ đề"),
            Quote("C. Chỉ chủ đề"),
        ],
        review_when: &[Quote(
            "Xem lại khi dữ liệu pilot cho thấy tác giả thường chạm giới hạn, hoặc tác phẩm lai thường cần hai dạng bài (khi đó tăng hằng số, không đổi schema).",
        )],
        related_ids: &["D-15"],
        ..D_ROW
    },
    Spec {
        id: "TAG-PR-01-CHOICE-02",
        domain: "taxonomy",
        source: TAG,
        blocks: &["### CHOICE-02 — Gắn metadata công khai", TAG_PR1],
        title: Label("#95 CHOICE-02 gắn metadata công khai, lát TAG-PR-01"),
        source_status: Quote(TAG_STATUS),
        resolution: &[
            Quote(
                "Hash v1, `RevisionId` và anchor endorsement không đổi khi sửa thẻ; publication trỏ **cặp** (revision, metadata snapshot) và đổi cặp nguyên tử",
            ),
            Quote(
                "cập nhật tag-only **không được** bypass: chỉ transaction của owner publication (ý định tác giả + authority hiện hành + generation, PUB-01..03) được đổi binding; có cần duyệt lại snapshot metadata hay không là quyết định của owner đó, còn OPEN.",
            ),
            Quote(
                "`MetadataDigest` (SHA-256, separator `vink-work-metadata-v1`, thứ tự chủ đề có nghĩa) là danh tính nội dung của snapshot, tách hẳn khỏi `ContentHash` v1.",
            ),
        ],
        rationale: &[Quote(
            "Đánh đổi: thêm một version (thẻ nháp) và một ranh giới binding.",
        )],
        alternative: &[Quote(
            "Sửa tag-only phải tạo revision mới: `RevisionId` mới tách số “Đánh giá cao” (LVE neo revision) và lẫn danh tính văn bản với mô tả",
        )],
        related_ids: &["D-15"],
        ..D_ROW
    },
    Spec {
        id: "TAG-PR-01-CHOICE-03",
        domain: "taxonomy",
        source: TAG,
        blocks: &["### CHOICE-03 — Quản trị catalog", TAG_PR1],
        title: Label("#95 CHOICE-03 quản trị catalog, lát TAG-PR-01"),
        source_status: Quote(TAG_STATUS),
        resolution: &[
            Quote(
                "Curator (operator) tạo/đổi tên/mô tả/thêm alias/ngừng dùng; tác giả chỉ chọn thẻ có sẵn.",
            ),
            Quote("“Chưa có thẻ này” trả `UnknownTag`, không có nút hay endpoint đề xuất giả."),
        ],
        rationale: &[Quote(
            "Phương án B (đề xuất chờ duyệt) cần hàng đợi, quyền, chống lạm dụng và bảo đảm đề xuất không lộ; chưa có dữ liệu pilot cho thấy thiếu thẻ thường xuyên.",
        )],
        review_when: &[Quote(
            "Xem lại khi pilot ghi nhận nhiều lượt tìm không ra thẻ.",
        )],
        related_ids: &["D-15"],
        ..D_ROW
    },
    choice(
        "DISC-01-CHOICE-01",
        "discussion",
        ADMISSION,
        &["| CHOICE-01 Risk-policy rollout |", DISC_01],
        "#120 CHOICE-01 risk-policy rollout, lát DISC-01",
        DISC_01_STATUS,
        &["D-08"],
    ),
    choice(
        "DISC-01-CHOICE-06",
        "discussion",
        ADMISSION,
        &["| CHOICE-06 Trình bày gửi/thử lại", DISC_01],
        "#120 CHOICE-06 trình bày gửi/thử lại, lát DISC-01",
        DISC_01_STATUS,
        &[],
    ),
    Spec {
        id: "DISC-02-03-CHOICE-02",
        domain: "discussion",
        source: READS,
        blocks: &["### CHOICE-02 — Hợp đồng đọc công khai/riêng tư", DISC_10],
        title: Label("#120 CHOICE-02 hợp đồng đọc công khai/riêng tư, lát DISC-02/03 phần 2"),
        source_status: Quote("Phạm vi: phần **đọc** của DISC-02/03"),
        resolution: &[Quote(
            "**B+: giữ composed v1, thêm public projection độc lập và overlay riêng**",
        )],
        rationale: &[Quote(
            "Lý do: B+ tách dữ liệu công khai khỏi trạng thái người xem ở cả kiểu Rust lẫn wire mà không phá consumer hiện có.",
        )],
        alternative: &[
            Quote("A: public endpoint + overlay, bỏ composed"),
            Quote("C: chỉ thêm overlay"),
        ],
        ..D_ROW
    },
    choice(
        "DISC-04-CHOICE-01",
        "discussion",
        OUTBOX,
        &["| CHOICE-01 Event và delivery |", DISC_10],
        "Prompt DISC-04 CHOICE-01 event và delivery (#120)",
        DISC_04_STATUS,
        &[],
    ),
    choice(
        "DISC-04-CHOICE-02",
        "discussion",
        OUTBOX,
        &["| CHOICE-02 Đánh thức worker |", DISC_10],
        "Prompt DISC-04 CHOICE-02 đánh thức worker (#120, cho CHOICE-05 của epic)",
        DISC_04_STATUS,
        &[],
    ),
    choice(
        "DISC-04-CHOICE-03",
        "discussion",
        OUTBOX,
        &["| CHOICE-03 Consumer đầu tiên |", DISC_10],
        "Prompt DISC-04 CHOICE-03 consumer đầu tiên (#120)",
        DISC_04_STATUS,
        &["D-08"],
    ),
    choice(
        "DISC-04-CHOICE-04",
        "discussion",
        OUTBOX,
        &["| CHOICE-04 Mức tái sử dụng |", DISC_10],
        "Prompt DISC-04 CHOICE-04 mức tái sử dụng (#120)",
        DISC_04_STATUS,
        &[],
    ),
];

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Import {
    version: u16,
    source_commit: String,
    records: Vec<Row>,
}

struct Block {
    line: usize,
    cells: Option<Vec<String>>,
    text: String,
}

fn heading_level(line: &str) -> Option<usize> {
    let level = line.bytes().take_while(|b| *b == b'#').count();
    (level > 0 && line[level..].starts_with(' ')).then_some(level)
}

fn block(commit: &str, source: &str, lines: &[&str], anchor: &str) -> Result<Block, String> {
    let found: Vec<_> = (0..lines.len())
        .filter(|&i| lines[i].starts_with(anchor))
        .collect();
    let [start] = found[..] else {
        return Err(format!(
            "{source}@{commit}: anchor {anchor:?} matches {} lines, expected one",
            found.len()
        ));
    };
    let first = lines[start];
    let end = if first.starts_with('|') {
        start + 1
    } else if let Some(level) = heading_level(first) {
        (start + 1..lines.len())
            .find(|&i| heading_level(lines[i]).is_some_and(|other| other <= level))
            .unwrap_or(lines.len())
    } else {
        (start + 1..lines.len())
            .find(|&i| lines[i].trim().is_empty())
            .unwrap_or(lines.len())
    };
    let cells = first.starts_with('|').then(|| {
        first
            .trim()
            .trim_matches('|')
            .split('|')
            .map(|cell| cell.trim().to_string())
            .collect::<Vec<_>>()
    });
    if cells
        .as_ref()
        .is_some_and(|cells| cells.iter().any(String::is_empty))
    {
        return Err(format!("{source}:{}: unexpected empty cell", start + 1));
    }
    let text = lines[start..end]
        .iter()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    Ok(Block {
        line: start + 1,
        cells,
        text,
    })
}

impl Spec {
    fn excerpt(
        &self,
        commit: &str,
        blocks: &[Block],
        part: Part,
        title: bool,
    ) -> Result<String, String> {
        let error = |message: String| format!("{}@{commit}: {}: {message}", self.source, self.id);
        match part {
            Cell(index) => blocks[0]
                .cells
                .as_ref()
                .and_then(|cells| cells.get(index))
                .cloned()
                .ok_or_else(|| error(format!("no cell {index} in the first block"))),
            Quote(quote) if blocks.iter().any(|block| block.text.contains(quote)) => {
                Ok(quote.into())
            }
            Quote(quote) => Err(error(format!("quote not found verbatim: {quote:?}"))),
            Label(label) if title => Ok(label.into()),
            Label(_) => Err(error("a label is accepted only as a title".into())),
        }
    }

    fn join(
        &self,
        commit: &str,
        blocks: &[Block],
        parts: &[Part],
    ) -> Result<Option<String>, String> {
        if parts.is_empty() {
            return Ok(None);
        }
        let excerpts = parts
            .iter()
            .map(|part| self.excerpt(commit, blocks, *part, false))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Some(excerpts.join("\n")))
    }

    pub(super) fn row(&self, commit: &str, group_id: &str, text: &str) -> Result<Row, String> {
        if !legacy::status_matches_kind(self.status, self.kind) || self.status == Status::Accepted {
            return Err(format!(
                "{}: {:?} is not importable for a {:?} decision; an import never promotes",
                self.id, self.status, self.kind
            ));
        }
        let lines: Vec<_> = text.lines().collect();
        let blocks = self
            .blocks
            .iter()
            .map(|anchor| block(commit, self.source, &lines, anchor))
            .collect::<Result<Vec<_>, _>>()?;
        if blocks.is_empty() {
            return Err(format!("{}: no source block", self.id));
        }
        let mut requires: Vec<String> = self.requires.iter().map(|id| (*id).into()).collect();
        requires.sort();
        let mut related_ids: Vec<String> = self.related_ids.iter().map(|id| (*id).into()).collect();
        related_ids.sort();
        Ok(Row {
            id: self.id.into(),
            revision: 1,
            content: Content {
                kind: self.kind,
                domain: self.domain.into(),
                title: self.excerpt(commit, &blocks, self.title, true)?,
                resolution: self
                    .join(commit, &blocks, self.resolution)?
                    .ok_or_else(|| format!("{}: missing resolution", self.id))?,
                rationale: self.join(commit, &blocks, self.rationale)?,
                requires,
                related_ids: vec![],
                work_audiences: vec![],
            },
            legacy: Provenance {
                group: group_id.into(),
                status: self.status,
                historical: false,
                source_line: blocks[0].line,
                source_path: Some(self.source.into()),
                related_ids,
                context: Context {
                    canonical_references: (!self.references.is_empty())
                        .then(|| self.references.into()),
                    alternative: self.join(commit, &blocks, self.alternative)?,
                    review_when: self.join(commit, &blocks, self.review_when)?,
                    source_status: Some(self.excerpt(
                        commit,
                        &blocks,
                        self.source_status,
                        false,
                    )?),
                    ..Context::default()
                },
            },
        })
    }
}

pub(super) fn verify_bundle(
    file: &str,
    commit: &str,
    group_id: &str,
    import_sha256: &str,
    text: &str,
) -> Result<(), String> {
    let normalized = text.replace("\r\n", "\n");
    if sha256_hex(normalized.as_bytes()) != import_sha256 {
        return Err(format!(
            "{file}: differs from the reviewed {commit} import; use decision migrate --dry-run {group_id} to compare"
        ));
    }
    Ok(())
}

pub(super) fn verify_import(text: &str) -> Result<(), String> {
    verify_bundle(FILE, COMMIT, GROUP_ID, IMPORT_SHA256, text)
}

pub(super) fn load_bundle(
    root: &Path,
    file: &str,
    commit: &str,
    group_id: &str,
    import_sha256: &str,
) -> Result<BTreeMap<Key, Entry>, String> {
    let path = root.join(file);
    let text = record::read_file(&path)?;
    let import: Import = record::decode(&path, &text)?;
    if import.version != VERSION {
        return Err(format!(
            "{file}: version: unsupported format {}",
            import.version
        ));
    }
    verify_bundle(file, commit, group_id, import_sha256, &text)?;
    let mut entries = BTreeMap::new();
    for mut row in import.records {
        if row.legacy.group != group_id || row.legacy.source_path.is_none() || row.revision != 1 {
            return Err(format!("{file}: {}: unexpected import provenance", row.id));
        }
        if !legacy::status_matches_kind(row.legacy.status, row.content.kind)
            || row.legacy.status == Status::Accepted
        {
            return Err(format!(
                "{file}: {}: status outside the import contract",
                row.id
            ));
        }
        row.content.validate(&path, false)?;
        let hash = sha256_hex(record::encode(&row)?.as_bytes());
        let key = Key::new(&row.id, row.revision);
        let entry = Entry {
            content: row.content,
            hash,
            path: path.clone(),
            origin: Origin::Legacy(Box::new(row.legacy)),
            signoff: None,
        };
        if entries.insert(key.clone(), entry).is_some() {
            return Err(format!("{file}: duplicate revision {key}"));
        }
    }
    Ok(entries)
}

pub(super) fn load(root: &Path) -> Result<BTreeMap<Key, Entry>, String> {
    load_bundle(root, FILE, COMMIT, GROUP_ID, IMPORT_SHA256)
}

fn show(root: &Path, commit: &str, source: &str) -> Result<String, String> {
    let result = Command::new("git")
        .args([
            "--no-replace-objects",
            "show",
            &format!("{commit}:{source}"),
        ])
        .current_dir(root)
        .env("GIT_NO_LAZY_FETCH", "1")
        .output()
        .map_err(|e| format!("read migration source with git: {e}"))?;
    if !result.status.success() {
        return Err(format!(
            "migration source {commit}:{source} unavailable; use a checkout with that Git object"
        ));
    }
    String::from_utf8(result.stdout).map_err(|e| format!("migration source must be UTF-8: {e}"))
}

pub(super) fn render_bundle(
    root: &Path,
    commit: &str,
    group_id: &str,
    specs: &[Spec],
) -> Result<String, String> {
    let mut sources = BTreeMap::new();
    let mut records = Vec::new();
    for spec in specs {
        if !sources.contains_key(spec.source) {
            sources.insert(spec.source, show(root, commit, spec.source)?);
        }
        records.push(spec.row(commit, group_id, &sources[spec.source])?);
    }
    records.sort_by(|a, b| a.id.cmp(&b.id));
    if let Some(pair) = records.windows(2).find(|pair| pair[0].id == pair[1].id) {
        return Err(format!("duplicate import ID {}", pair[0].id));
    }
    record::encode(&Import {
        version: VERSION,
        source_commit: commit.into(),
        records,
    })
}

pub(super) fn render(root: &Path) -> Result<String, String> {
    render_bundle(root, COMMIT, GROUP_ID, SPECS)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = "# Sổ\n\n| D-90 | Nội dung | Đề xuất | Mặc định dài\nnối |\n| D-90b | X | Y | Z |\n\n### Mục\n\nĐoạn một\nnối dòng.\n\n### Khác\n";

    const BASE: Spec = Spec {
        id: "D-90",
        blocks: &["| D-90 |", "### Mục"],
        ..D_ROW
    };

    #[test]
    fn excerpts_must_be_verbatim_in_their_blocks() {
        let row = BASE.row(COMMIT, GROUP_ID, SOURCE).unwrap();
        assert_eq!(row.content.title, "Nội dung");
        assert_eq!(row.legacy.context.source_status.as_deref(), Some("Đề xuất"));
        assert_eq!(row.legacy.source_line, 3);
        // A soft line break inside a paragraph reads as a space.
        let joined = Spec {
            resolution: &[Quote("Đoạn một nối dòng.")],
            ..BASE
        };
        assert!(joined.row(COMMIT, GROUP_ID, SOURCE).is_ok());
        for (spec, error) in [
            (
                Spec {
                    resolution: &[Quote("Đoạn một đã sửa.")],
                    ..BASE
                },
                "quote not found verbatim",
            ),
            (
                Spec {
                    resolution: &[Quote("Khác")],
                    ..BASE
                },
                "quote not found verbatim",
            ),
            (
                Spec {
                    resolution: &[Label("tự viết")],
                    ..BASE
                },
                "only as a title",
            ),
            (
                Spec {
                    blocks: &["| D-9"],
                    ..BASE
                },
                "matches 2 lines",
            ),
            (
                Spec {
                    resolution: &[Cell(9)],
                    ..BASE
                },
                "no cell 9",
            ),
            (
                Spec {
                    status: Status::Accepted,
                    ..BASE
                },
                "never promotes",
            ),
            (
                Spec {
                    status: Status::ProductConfirmed,
                    ..BASE
                },
                "never promotes",
            ),
        ] {
            let message = spec.row(COMMIT, GROUP_ID, SOURCE).err().unwrap();
            assert!(message.contains(error), "{message}");
        }
    }
}
