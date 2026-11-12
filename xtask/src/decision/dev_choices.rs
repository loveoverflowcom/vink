//! Import the long-lived slice and development choices at one pinned commit (#164 follow-up).
//!
//! Covers the 10 taxonomy/discovery slice choices (`TAG-PR-02..05`), the Auth §11 production-path
//! design choices (`D-05-C1..C5`), the Literary Endorsement WP-04 Pen Flight R2 choice
//! (`LVE-WP-04-CHOICE-07`), the conditional/cache-first public read choices (`READ-01`, `READ-02`)
//! and the Discussion Web public/own read choices (`DISC-WEB-CHOICE-01..03`). Every sentence is a
//! verbatim excerpt of the pinned commit; no status is promoted and no `ACCEPTED` or fabricated
//! product signoff is created.

use std::{collections::BTreeMap, path::Path};

use super::{
    decision_log::{self, D_ROW, Label, Quote, Spec},
    legacy::Status,
    record::{Entry, Key},
};

pub(super) const GROUP_ID: &str = "DECISION-DEV-20261112";
pub(super) const FILE: &str = "decisions/decision-dev-20261112.toml";
// develop after #168 merged password enrollment; all sources below are read at this commit.
pub(super) const COMMIT: &str = "630e80a9a1b2790eb4f3c663afb7ad627163d3cb";
// Independent of the editable TOML. Review changes to this pin and to SPECS together.
pub(super) const IMPORT_SHA256: &str =
    "f6799d0ecef175e597f0c0d1eaf252ee2d0c6736e3500080b6ce81064fe2f9a9";

const AUTH: &str = "docs/architecture/auth-authority.md";
const PEN_FLIGHT: &str = "docs/design/pen-flight-r2.md";
const PEN_FLIGHT_HEADER: &str = "# Pen Flight R2 — CHOICE-07 A · Mềm";
const PEN_FLIGHT_STATUS: &str = "Thiết kế chốt 09/11/2026, không mở lại policy LVE.";
const TAG_PG: &str = "docs/development/taxonomy-postgres.md";
const TAG_API: &str = "docs/development/taxonomy-api.md";
const DISC_API: &str = "docs/development/discovery-api.md";
const TAG_EDITOR: &str = "docs/development/taxonomy-editor.md";
const READ_01: &str = "docs/development/public-conditional-reads.md";
const READ_02: &str = "docs/development/public-read-freshness.md";
const DISC_WEB: &str = "docs/development/discussion-web-reads.md";

const AUTH_HEADER: &str = "**Ngày:** 07/11/2026 (spike), cập nhật 10/11/2026";
const AUTH_STATUS: &str =
    "**Trạng thái:** **Đề xuất cho pilot (Recommended / Provisional)**, chưa Accepted.";
const AUTH_REFS: &str =
    "docs/development/auth-production-evidence.md, docs/development/auth-operations.md";

const TAG_PR2_SCOPE: &str = "**Ngày:** 08/11/2026. **Nguồn:** [#96]";
const TAG_PR2_STATUS: &str = "Đây là lựa chọn triển khai của slice, không phải product Accepted; mỗi mục có điều kiện xem lại.";

const TAG_PR3_SCOPE: &str = "**Ngày:** 08/11/2026. **Nguồn:** [#97]";
const TAG_PR3_STATUS: &str = "Đây là lựa chọn triển khai của slice, không phải product Accepted; mỗi mục có điều kiện xem lại.";

const TAG_PR4_SCOPE: &str = "**Ngày:** 08/11/2026. **Nguồn:** [#98]";
const TAG_PR4_STATUS: &str = "Đây là lựa chọn triển khai của slice, không phải product Accepted; mỗi mục có điều kiện xem lại.";

const TAG_PR5_SCOPE: &str = "**Ngày:** 09/11/2026. **Nguồn:** [#99]";
const TAG_PR5_STATUS: &str = "Hai câu OPEN của issue được chọn bằng bằng chứng cục bộ; chi tiết và phương án bị loại ở [LayoutSpec](../design/tag-selector.md). Đây là lựa chọn của lát, chưa có usability test.";

const READ_01_SCOPE: &str = "**Issue:** [#52]";
const READ_01_STATUS: &str = "pilot đã triển khai cho hai endpoint; đã kiểm bằng unit, PostgreSQL thật và Chrome headless cục bộ. Chưa đo trên môi trường production.";

const READ_02_SCOPE: &str = "**Issue:** [#53]";
const READ_02_STATUS: &str = "Chính sách hiển thị dữ liệu cũ dưới đây là **đề xuất v1**, chưa phải quyết định sản phẩm đã chốt.";

const DISC_WEB_SCOPE: &str = "Lát `DISC-WEB-PUBLIC-OWN-READS`";
const DISC_WEB_STATUS: &str =
    "Lát `DISC-WEB-PUBLIC-OWN-READS`, tiếp nối #144 và #146, nền `develop@4891c77`";

const SPECS: &[Spec] = &[
    Spec {
        id: "D-05-C1",
        domain: "account",
        status: Status::Provisional,
        source: AUTH,
        blocks: &[
            "### C1. SMTP hay API transactional email — **chọn SMTP (Resend)**",
            AUTH_HEADER,
        ],
        title: Label("D-05 §11 C1 gửi thư xác thực qua SMTP submission (lettre, rustls)"),
        source_status: Quote(AUTH_STATUS),
        resolution: &[
            Quote("### C1. SMTP hay API transactional email — **chọn SMTP (Resend)**"),
            Quote(
                "**Seam đổi nhà cung cấp:** worker chỉ giữ `Arc<dyn MailSender>`; `VINK_IDENTITY_SMTP_PROVIDER=resend` là một preset cấu hình; nhà cung cấp SMTP khác chỉ đổi cấu hình, nhà cung cấp chỉ có API HTTP là một `MailSender` mới — outbox, retry, nội dung thư không đổi.",
            ),
            Quote(
                "**HOW:** TLS bắt buộc (465 implicit hoặc 587 STARTTLS), plaintext chỉ cho relay loopback; mật khẩu (API key) chỉ qua file `0600`, đọc lại mỗi lần gửi (xoay không restart); lỗi rút gọn thành loại + mã số, không giữ văn bản relay (có thể chứa địa chỉ).",
            ),
        ],
        rationale: &[
            Quote(
                "**WHY:** trung lập nhà cung cấp, kiểm được bằng giao thức thật, cùng hướng Cantos đã vận hành (Resend qua SMTP, `lettre`).",
            ),
            Quote(
                "**TRADEOFF:** không có idempotency key → trùng thư có thể xảy ra (cùng mã, cùng `Message-ID`, vô hại); bounce/complaint cần webhook của nhà cung cấp — chưa làm.",
            ),
        ],
        alternative: &[Quote("API HTTP của nhà cung cấp")],
        requires: &["D-05@r1"],
        related_ids: &["D-05"],
        references: AUTH_REFS,
        ..D_ROW
    },
    Spec {
        id: "D-05-C2",
        domain: "account",
        status: Status::Provisional,
        source: AUTH,
        blocks: &[
            "### C2. Outbox PostgreSQL hay worker hiện có — **outbox của Identity, worker trong `serve`**",
            AUTH_HEADER,
        ],
        title: Label("D-05 §11 C2 outbox thư trong PostgreSQL của Identity và worker trong serve"),
        source_status: Quote(AUTH_STATUS),
        resolution: &[
            Quote(
                "### C2. Outbox PostgreSQL hay worker hiện có — **outbox của Identity, worker trong `serve`**",
            ),
            Quote(
                "**HOW:** bảng `mail_deliveries` trong `vink_identity`; challenge và ý định gửi commit cùng transaction; worker claim `FOR UPDATE SKIP LOCKED` + lease 45 s (nhiều instance an toàn), gửi ngoài transaction, ghi kết quả chỉ khi còn giữ `lease_token`; 5 lần, backoff 5→60 s + jitter; `5xx` dừng ngay; mã đã dùng/hết hạn bị hủy; trạng thái cuối xóa mã; worker thức ngay sau commit (Notify) và poll 2 s cho việc của instance khác/retry.",
            ),
        ],
        rationale: &[
            Quote(
                "`vink-worker` là tiến trình App, không được đọc DB Identity (mã là secret của Identity).",
            ),
            Quote(
                "**TRADEOFF:** at-least-once, không exactly-once; thông lượng gắn với số instance Identity (đủ cho pilot; tách `deliver` thành lệnh riêng nếu cần).",
            ),
        ],
        requires: &["D-05@r1"],
        related_ids: &["D-05"],
        references: AUTH_REFS,
        ..D_ROW
    },
    Spec {
        id: "D-05-C3",
        domain: "account",
        status: Status::Provisional,
        source: AUTH,
        blocks: &[
            "### C3. Rate limit trong DB hay ở edge tin cậy — **DB làm authority, edge là phòng thủ thêm**",
            AUTH_HEADER,
        ],
        title: Label(
            "D-05 §11 C3 giới hạn lạm dụng lấy PostgreSQL làm authority, edge là phòng thủ thêm",
        ),
        source_status: Quote(AUTH_STATUS),
        resolution: &[
            Quote(
                "### C3. Rate limit trong DB hay ở edge tin cậy — **DB làm authority, edge là phòng thủ thêm**",
            ),
            Quote(
                "**HOW:** App: bảng `auth_rate_limits` (upsert nguyên tử, cửa sổ cố định theo đồng hồ DB, khóa = SHA-256 của bucket + IPv4 hoặc IPv6 /64); `X-Forwarded-For` chỉ đọc khi peer thuộc `VINK_TRUSTED_PROXIES`, lấy hop không tin cậy ngoài cùng bên phải; không đếm được = `503`.",
            ),
            Quote(
                "Identity: trần theo địa chỉ (chính xác, khóa advisory), theo tài khoản (chính xác, khóa hàng tài khoản), toàn cục (mềm: đồng thời có thể vượt tối đa bằng số request đồng thời).",
            ),
        ],
        rationale: &[
            Quote(
                "**WHY:** App là tiến trình duy nhất thấy mạng nguồn; nhiều process App cần một bộ đếm chung; không có hạ tầng edge nào được xác nhận.",
            ),
            Quote(
                "cửa sổ cố định cho phép tối đa 2× ở ranh giới cửa sổ; không chống DDoS thể tích (việc của edge).",
            ),
        ],
        requires: &["D-05@r1"],
        related_ids: &["D-05"],
        references: AUTH_REFS,
        ..D_ROW
    },
    Spec {
        id: "D-05-C4",
        domain: "account",
        status: Status::Provisional,
        source: AUTH,
        blocks: &[
            "### C4. Fence chung hay guard theo owner — **một primitive chung, gọi bởi từng owner**",
            AUTH_HEADER,
        ],
        title: Label("D-05 §11 C4 fence đình chỉ principal dùng chung tại commit"),
        source_status: Quote(AUTH_STATUS),
        resolution: &[
            Quote(
                "### C4. Fence chung hay guard theo owner — **một primitive chung, gọi bởi từng owner**",
            ),
            Quote(
                "**HOW:** `principal::require_active` khóa hàng principal `FOR UPDATE` (xung đột với khóa `FOR KEY SHARE` mà insert đình chỉ lấy qua FK) rồi đọc đình chỉ ở câu lệnh riêng; trả `StoreError::PrincipalSuspended` → `403 principal_suspended`.",
            ),
            Quote(
                "Mỗi adapter gọi nó **tại commit**, đúng thứ tự khóa của mình: nháp tạo/lưu (đầu tiên — không khóa gì khác), xóa bình luận (sau hàng comment), đổi chế độ (sau hàng thread), kiểm duyệt (sau grant + hàng comment), ghi nhận (sau các khóa share, trước khóa khóa-lệnh).",
            ),
        ],
        rationale: &[
            Quote("Không dùng middleware (chỉ là kiểm lúc request) hay trigger (ẩn thứ tự khóa)."),
            Quote(
                "**TRADEOFF:** mọi lệnh ghi của một principal tuần tự hóa trên hàng principal (bình thường không đáng kể); disable ở Identity không thể nguyên tử với ghi ở App — không giả lập giao dịch xuyên DB, runbook yêu cầu đình chỉ ở App trước.",
            ),
        ],
        requires: &["D-05@r1"],
        related_ids: &["D-05"],
        references: AUTH_REFS,
        ..D_ROW
    },
    Spec {
        id: "D-05-C5",
        domain: "account",
        status: Status::Provisional,
        source: AUTH,
        blocks: &["### C5. Retention, dọn dẹp, phục hồi", AUTH_HEADER],
        title: Label("D-05 §11 C5 retention, dọn dẹp định kỳ và phục hồi dữ liệu tạm auth"),
        source_status: Quote(AUTH_STATUS),
        resolution: &[
            Quote(
                "Challenge (kèm thư) xóa 7 ngày sau hết hạn; phiên 30 ngày sau khi đóng; bộ đếm 1 ngày sau cửa sổ.",
            ),
            Quote(
                "Prune chạy mỗi giờ (Identity `serve`, `vink-worker`) và bằng lệnh vận hành; idempotent, an toàn khi nhiều tiến trình.",
            ),
            Quote("Phục hồi: lease hết hạn → thư được gửi lại; relay phục hồi → backoff gửi tiếp."),
        ],
        rationale: &[Quote(
            "chỉ mất dữ liệu tạm (thư chờ, bộ đếm), không mất tài khoản, phiên hay principal.",
        )],
        requires: &["D-05@r1"],
        related_ids: &["D-05"],
        references: AUTH_REFS,
        ..D_ROW
    },
    Spec {
        id: "DISC-WEB-CHOICE-01",
        domain: "discussion",
        source: DISC_WEB,
        blocks: &["- **01 A — vùng riêng gọn**", DISC_WEB_SCOPE],
        title: Label("DISC-WEB CHOICE-01 vùng riêng gọn cho nhận xét của bạn"),
        source_status: Quote(DISC_WEB_STATUS),
        resolution: &[Quote(
            "“Nhận xét của bạn” ngay sau composer gốc. Giữ timeline công khai có nghĩa công khai; own list tới được pending/rejected ngoài trang hoặc dưới root không được liệt kê.",
        )],
        rationale: &[Quote(
            "Loại B (ghép theo chronology) vì cần thêm vị trí/cursor/context cho hàng riêng dưới root chưa tải và dễ làm sai `public_replies`.",
        )],
        alternative: &[Quote("B (ghép theo chronology)")],
        requires: &["DISC-02-03-CHOICE-02@r1"],
        related_ids: &["D-08", "DISC-02-03-CHOICE-02"],
        references: "docs/development/discussion-web-reads.md, docs/architecture/discussion-reads.md",
        ..D_ROW
    },
    Spec {
        id: "DISC-WEB-CHOICE-02",
        domain: "discussion",
        source: DISC_WEB,
        blocks: &[
            "- **02 B — composed capability seam có giới hạn**",
            DISC_WEB_SCOPE,
        ],
        title: Label("DISC-WEB CHOICE-02 composed capability seam có giới hạn"),
        source_status: Quote(DISC_WEB_STATUS),
        resolution: &[
            Quote(
                "sau hydration gọi composed v1 với `limit=1`, chỉ decode `work_id`, `viewer_is_work_author`; bỏ toàn bộ roots ngay tại decoder. Public SSR/browser dùng DTO/endpoint public riêng, không chờ hoặc phụ thuộc seam này.",
            ),
            Quote("Lookup own của các ID công khai cấp `can_remove`, không suy từ tên hoặc login."),
        ],
        rationale: &[Quote(
            "Loại A (API capability mới) vì provider hiện có đã cấp đủ fact; không cần thêm API/SQL.",
        )],
        alternative: &[Quote("A (API capability mới)")],
        requires: &["DISC-02-03-CHOICE-02@r1"],
        related_ids: &["D-08", "DISC-02-03-CHOICE-02"],
        references: "docs/development/discussion-web-reads.md, docs/architecture/discussion-reads.md",
        ..D_ROW
    },
    Spec {
        id: "DISC-WEB-CHOICE-03",
        domain: "discussion",
        source: DISC_WEB,
        blocks: &["- **03 — generation ở owner hiện có**", DISC_WEB_SCOPE],
        title: Label("DISC-WEB CHOICE-03 generation ở owner hiện có và đối soát lô"),
        source_status: Quote(DISC_WEB_STATUS),
        resolution: &[
            Quote(
                "`ViewState` giữ epoch public/mutation; `OwnState` giữ scope + read generation. Đổi scope dọn private facts và release busy ngay, không xếp hàng sau request cũ. Mỗi own/capability read có một child Owner hủy fetch và deadline khi read kế tiếp thay thế nó.",
            ),
            Quote("Write vẫn ở feature Owner: hủy transport không chứng minh server rollback."),
        ],
        rationale: &[Quote("Không thêm store toàn cục hoặc framework state.")],
        requires: &["DISC-02-03-CHOICE-02@r1"],
        related_ids: &["D-08", "DISC-02-03-CHOICE-02"],
        references: "docs/development/discussion-web-reads.md, docs/architecture/discussion-reads.md",
        ..D_ROW
    },
    Spec {
        id: "LVE-WP-04-CHOICE-07",
        domain: "endorsement",
        source: PEN_FLIGHT,
        blocks: &[
            PEN_FLIGHT_HEADER,
            "10/11/2026 · lát UI riêng",
            "Giữ viewBox 24×24",
            "Owner: `shared/icons.json`",
        ],
        title: Label("#73 CHOICE-07 biểu tượng Pen Flight R2 (A · Mềm), lát Web adapter WP-04"),
        source_status: Quote(PEN_FLIGHT_STATUS),
        resolution: &[
            Quote(
                "Giữ viewBox 24×24 và path SVG nguyên văn, không raster hay icon font. Một path đơn sắc, currentColor, hiển thị 20px trang trí (`aria-hidden`, không thêm tên truy cập). Ngòi bút hướng lên, lỗ thở tròn; không tim. Vector không đổi theo phase.",
            ),
            Quote(
                "Owner: `shared/icons.json` lưu source URL, revision, Vink ownership và SHA-256 SVG gốc; `xtask/src/ui_tokens.rs` kiểm provenance/hash rồi sinh Rust/CMP. Không gán license của Material cho glyph Vink. `EndorsementAction` Leptos/CMP nhận glyph qua slot có sẵn.",
            ),
        ],
        rationale: &[
            Quote("Thiết kế chốt 09/11/2026, không mở lại policy LVE."),
            Quote("Không thêm animation, backend, ranking, auth hoặc native product consumer."),
        ],
        requires: &["D-14@r1"],
        related_ids: &["D-14"],
        references: "docs/design/pen-flight-r2.md, docs/development/pen-flight-r2-evidence.md, docs/architecture/literary-endorsement.md",
        ..D_ROW
    },
    Spec {
        id: "READ-01-CHOICE-01",
        domain: "platform",
        source: READ_01,
        blocks: &[
            "| **A, ETag = SHA-256 của byte JSON sau query** |",
            "| Không chọn B (validator từ revision/generation) |",
            "| C (giữ 200) vẫn là đường rút lui |",
            "| **Strong**, không weak |",
            "| 128 bit đầu của SHA-256, hex |",
            READ_01_SCOPE,
        ],
        title: Label("READ-01 CHOICE-01 nguồn định danh và độ mạnh ETag cho đọc công khai"),
        source_status: Quote(READ_01_STATUS),
        resolution: &[
            Quote(
                "Bao phủ mọi trường thật sự gửi đi (title, byline, document, revision, `published_at`, thứ tự, membership) mà không cần danh sách phụ thuộc. Đổi tên tác giả không đổi `revision_id` nhưng đổi tag. Không thêm cột, không thêm migration.",
            ),
            Quote(
                "Tag là digest của đúng byte gửi đi; không có content coding (`CompressionLayer` không tồn tại), nên byte hash = byte trên dây. Thêm nén thì phải đổi tag theo coding hoặc chuyển weak trước.",
            ),
            Quote(
                "Đủ chống trùng cho số representation của một URL; header ngắn. Tag là opaque: client không được suy diễn cấu trúc.",
            ),
        ],
        rationale: &[
            Quote(
                "Phải chứng minh đủ phụ thuộc (byline nằm ở `principals`, thứ tự/membership của list) và tách query metadata/body sẽ tạo race. Chưa có benchmark nào cho thấy A là nút thắt.",
            ),
            Quote("Xem [Rút lại](#rút-lại-theo-endpoint)."),
        ],
        alternative: &[Quote("Không chọn B (validator từ revision/generation)")],
        requires: &["D-03@r1", "DISC-02-03-CHOICE-02@r1"],
        related_ids: &["D-03", "DISC-02-03-CHOICE-02"],
        references: "docs/development/public-conditional-reads.md",
        ..D_ROW
    },
    Spec {
        id: "READ-01-CHOICE-02",
        domain: "platform",
        source: READ_01,
        blocks: &[
            "| `Cache-Control: private, no-cache` |",
            "| Lỗi: `no-store` |",
            "| Không `Vary` |",
            READ_01_SCOPE,
        ],
        title: Label("READ-01 CHOICE-02 chính sách Cache-Control cho đọc công khai"),
        source_status: Quote(READ_01_STATUS),
        resolution: &[
            Quote(
                "Trình duyệt được lưu nhưng phải hỏi lại server trước mỗi lần dùng; không shared cache (chưa có CDN và nội dung rút bài không được nằm ở cache trung gian). Không đặt TTL, nên không có hai TTL chồng nhau với READ-02.",
            ),
            Quote("`404`/`503`/`500` không bị lưu, nên không giữ một 404 qua lúc phục hồi."),
            Quote("Không header nào chọn representation (không nén, không locale/filter)."),
        ],
        rationale: &[Quote(
            "chưa có CDN và nội dung rút bài không được nằm ở cache trung gian",
        )],
        requires: &["D-03@r1", "DISC-02-03-CHOICE-02@r1"],
        related_ids: &["D-03", "DISC-02-03-CHOICE-02"],
        references: "docs/development/public-conditional-reads.md",
        ..D_ROW
    },
    Spec {
        id: "READ-01-CHOICE-03",
        domain: "platform",
        source: READ_01,
        blocks: &[
            "Body và tag đến từ **một** lần serialize",
            "1. Use case trả lỗi trước",
            "2. Chỉ representation đang tồn tại",
            READ_01_SCOPE,
        ],
        title: Label("READ-01 CHOICE-03 đường dẫn 304 ở tầng lưu trữ từ một snapshot DB"),
        source_status: Quote(READ_01_STATUS),
        resolution: &[
            Quote(
                "Body và tag đến từ **một** lần serialize của **một** câu SQL (PostgreSQL READ COMMITTED cho mỗi câu một snapshot), nên không thể gắn tag của bản A lên body B. Không giữ transaction qua request.",
            ),
            Quote(
                "1. Use case trả lỗi trước (`404` cho thiếu/chưa công khai/ID sai, `503` khi PostgreSQL không trả lời, `500` khi dữ liệu lưu sai). Không bao giờ đoán “not modified” khi chưa đọc được.",
            ),
            Quote("2. Chỉ representation đang tồn tại mới được so với `If-None-Match`."),
        ],
        rationale: &[Quote("tài nguyên đã được xác nhận công khai ở bước 1.")],
        requires: &["D-03@r1", "DISC-02-03-CHOICE-02@r1"],
        related_ids: &["D-03", "DISC-02-03-CHOICE-02"],
        references: "docs/development/public-conditional-reads.md",
        ..D_ROW
    },
    Spec {
        id: "READ-01-CHOICE-04",
        domain: "platform",
        source: READ_01,
        blocks: &[
            "SSR vẫn đọc in-process qua `SharedPublicReads`",
            READ_01_SCOPE,
        ],
        title: Label("READ-01 CHOICE-04 hợp đồng bộ nhớ đệm HTTP ở adapter Web"),
        source_status: Quote(READ_01_STATUS),
        resolution: &[Quote(
            "SSR vẫn đọc in-process qua `SharedPublicReads`; HTML reader/Khám phá không mang ETag của API. Draft và discussion giữ `no-store`, không có validator. `PUT` draft vẫn dùng `expected_version`/`409`, không đổi sang `If-Match`.",
        )],
        rationale: &[Quote(
            "HTML reader/Khám phá không mang ETag của API. Draft và discussion giữ `no-store`, không có validator.",
        )],
        requires: &["D-03@r1", "DISC-02-03-CHOICE-02@r1"],
        related_ids: &["D-03", "DISC-02-03-CHOICE-02"],
        references: "docs/development/public-conditional-reads.md",
        ..D_ROW
    },
    Spec {
        id: "READ-02-CHOICE-01",
        domain: "platform",
        source: READ_02,
        blocks: &[
            "| **A — Query cache của app + HTTP cache trình duyệt** |",
            "| B — App tự quản validator |",
            "| `stale_after = 120 s` |",
            READ_02_SCOPE,
        ],
        title: Label("READ-02 CHOICE-01 phạm vi lưu trữ trong tab và cửa sổ tươi phía client"),
        source_status: Quote(READ_02_STATUS),
        resolution: &[
            Quote(
                "Trình duyệt giữ byte + ETag và tự gửi `If-None-Match`; app không đọc/giữ ETag. Request dùng `cache: \"no-cache\"`: nếu sau này server đổi header sang có `max-age`, trình duyệt vẫn phải hỏi lại origin trước khi dùng byte đã lưu, nên app không bao giờ ghi `validated_at` cho một bản trình duyệt tự trả từ cache. Không gắn timestamp lên URL; refresh thủ công cũng gửi validator.",
            ),
            Quote(
                "Ngưỡng kiểm tra lại, **không phải TTL**: không xóa gì. `validated_at` là thời điểm **bắt đầu** request được origin xác nhận (cận dưới, an toàn); đọc cache không gia hạn nó",
            ),
        ],
        rationale: &[Quote(
            "Không chọn: phải tự parse 304, đồng bộ data + tag, và tạo owner thứ hai cạnh HTTP cache. Chỉ mở khi A thiếu semantics.",
        )],
        alternative: &[Quote("B — App tự quản validator")],
        requires: &[
            "READ-01-CHOICE-01@r1",
            "READ-01-CHOICE-02@r1",
            "READ-01-CHOICE-04@r1",
        ],
        related_ids: &["D-03", "DISC-02-03-CHOICE-02"],
        references: "docs/development/public-read-freshness.md",
        ..D_ROW
    },
    Spec {
        id: "READ-02-CHOICE-02",
        domain: "platform",
        source: READ_02,
        blocks: &[
            "| Timer |",
            "| Mount, `focus`, hiện lại (`visibilitychange`), `online`, `pageshow` |",
            "| Ẩn tab |",
            "- **Single-flight:**",
            READ_02_SCOPE,
        ],
        title: Label("READ-02 CHOICE-02 điều phối làm mới theo vòng đời trang và single-flight"),
        source_status: Quote(READ_02_STATUS),
        resolution: &[
            Quote(
                "Một timer cho cả tab, hẹn đến mốc sớm nhất của các key đang được view quan sát: `validated_at + 120 s + spread`, spread ngẫu nhiên `0–15 s` chọn lại sau mỗi lần xác nhận. Không timer theo card",
            ),
            Quote(
                "Kiểm tra nếu đã quá 120 s (không cộng spread). Gộp với request đang chạy (single-flight); bốn tín hiệu đồng thời → một request. `online` chỉ là gợi ý: lỗi vẫn backoff",
            ),
            Quote(
                "Hủy timer, bỏ qua mọi tín hiệu trừ thao tác thủ công; hiện lại thì kiểm tra nếu stale",
            ),
        ],
        rationale: &[Quote(
            "view mount khi chưa có snapshot hiển thị được sẽ chờ request đang chạy (waiter), không tạo request thứ hai.",
        )],
        requires: &[
            "READ-01-CHOICE-01@r1",
            "READ-01-CHOICE-02@r1",
            "READ-01-CHOICE-04@r1",
        ],
        related_ids: &["D-03", "DISC-02-03-CHOICE-02"],
        references: "docs/development/public-read-freshness.md",
        ..D_ROW
    },
    Spec {
        id: "READ-02-CHOICE-03",
        domain: "platform",
        source: READ_02,
        blocks: &[
            "| Lỗi tạm (mạng, timeout 30 s, 5xx, phản hồi lạ) |",
            "| `404` (removal có thẩm quyền) |",
            "| `max_display_age = 15 phút` |",
            "1. **Hiển thị dữ liệu cũ:**",
            READ_02_SCOPE,
        ],
        title: Label("READ-02 CHOICE-03 xử lý ngoại tuyến/lỗi tạm thời và xóa ngay khi 404"),
        source_status: Quote(READ_02_STATUS),
        resolution: &[
            Quote(
                "Giữ last-success trên màn hình với thông báo riêng; backoff 5 → 10 → 20 → 40 → 80 → 120 s (trần), jitter 80–100 %, tôn trọng `Retry-After` dạng giây (trần 120 s). Không retry mutation",
            ),
            Quote(
                "Xóa entry ngay, reader thay chữ bằng trang “Không tìm thấy”; card của tác phẩm đó bị bỏ khỏi snapshot danh sách cho tới khi list được origin xác nhận lại. Không dùng stale-if-error cho trường hợp này",
            ),
            Quote(
                "Snapshot xác nhận quá 15 phút không được render ngay ở **lần mount mới** (sẽ tải với loading thật). Nội dung đang trên màn hình không bị rút vì lỗi tạm",
            ),
        ],
        rationale: &[Quote(
            "1. **Hiển thị dữ liệu cũ:** v1 chọn hướng 1 của issue — giữ snapshot công khai đã xem trong phiên tab, có trạng thái “chưa làm mới được”, revalidate khi online/hiện lại, và `max_display_age` 15 phút cho lần mount mới. Giá trị 15 phút là giả định cần chủ sản phẩm chốt.",
        )],
        review_when: &[Quote("Giá trị 15 phút là giả định cần chủ sản phẩm chốt.")],
        requires: &[
            "READ-01-CHOICE-01@r1",
            "READ-01-CHOICE-02@r1",
            "READ-01-CHOICE-04@r1",
        ],
        related_ids: &["D-03", "DISC-02-03-CHOICE-02"],
        references: "docs/development/public-read-freshness.md",
        ..D_ROW
    },
    Spec {
        id: "READ-02-CHOICE-04",
        domain: "platform",
        source: READ_02,
        blocks: &["| Bản đổi nội dung (200 khác) |", READ_02_SCOPE],
        title: Label("READ-02 CHOICE-04 cập nhật bài viết khi đang đọc dở (trình đọc bình tĩnh)"),
        source_status: Quote(READ_02_STATUS),
        resolution: &[Quote(
            "Khám phá cập nhật tại chỗ (key ổn định: giữ row, focus, scroll). Reader **không** thay chữ đang đọc: hiện “Tác phẩm vừa có bản cập nhật” + “Xem bản mới”; áp dụng thì về đầu nội dung và focus `#noi-dung`",
        )],
        rationale: &[Quote(
            "Reader **không** thay chữ đang đọc: hiện “Tác phẩm vừa có bản cập nhật” + “Xem bản mới”; áp dụng thì về đầu nội dung và focus `#noi-dung`",
        )],
        requires: &[
            "READ-01-CHOICE-01@r1",
            "READ-01-CHOICE-02@r1",
            "READ-01-CHOICE-04@r1",
        ],
        related_ids: &["D-03", "DISC-02-03-CHOICE-02"],
        references: "docs/development/public-read-freshness.md",
        ..D_ROW
    },
    Spec {
        id: "TAG-PR-02-CHOICE-01",
        domain: "taxonomy",
        source: TAG_PG,
        blocks: &[
            "### CHOICE-01 — Biểu diễn vật lý: **A, catalog và assignment chuẩn hóa**",
            "## 1. Lựa chọn của #96",
            TAG_PR2_SCOPE,
        ],
        title: Label(
            "#96 CHOICE-01 biểu diễn vật lý: A, catalog và assignment chuẩn hóa, lát TAG-PR-02",
        ),
        source_status: Quote(TAG_PR2_STATUS),
        resolution: &[
            Quote("A. Catalog chuẩn hóa + một hàng cho mỗi slot"),
            Quote(
                "`work_draft_tags` và `work_metadata_snapshot_tags` có `slot` 0 (dạng bài) hoặc 1..3 (chủ đề), khóa `(owner, slot)`, `UNIQUE (owner, tag_id)` và FK ghép `(tag_id, facet) → tags (id, facet)`.",
            ),
            Quote(
                "Snapshot là nhiều hàng nên cần chặn “thêm slot vào snapshot cũ”: trigger từ chối `UPDATE`/`DELETE`/ `TRUNCATE`, và một **constraint trigger hoãn tới commit** tính lại digest bằng hàm SQL `work_metadata_digest` (cùng byte layout với `MetadataDigest::of`) rồi so với `digest` đã lưu.",
            ),
        ],
        rationale: &[
            Quote(
                "Sai facet, thẻ dangling, trùng thẻ và chủ đề thứ tư **không biểu diễn được**, không cần CHECK đếm hàng giả vờ thấy hàng khác. Discovery (#98) truy vấn được `tag_id` trực tiếp",
            ),
            Quote(
                "Không có FK cho từng phần tử; cần một writer duy nhất kiểm tham chiếu và một chiến lược truy vấn JSON. Đọc một snapshot rẻ hơn chút, nhưng không đáng ở quy mô ≤ 4 thẻ/tác phẩm",
            ),
        ],
        alternative: &[Quote("B. Catalog + payload snapshot JSON/array")],
        review_when: &[Quote(
            "Xem lại nếu pilot cần nhiều chủ đề hơn hoặc nhiều dạng bài (khi đó slot dạng bài cũng phải đổi).",
        )],
        requires: &["TAG-PR-01-CHOICE-01@r1", "TAG-PR-01-CHOICE-02@r1"],
        related_ids: &["D-15"],
        references: TAG_PG,
        ..D_ROW
    },
    Spec {
        id: "TAG-PR-02-CHOICE-02",
        domain: "taxonomy",
        source: TAG_PG,
        blocks: &[
            "### CHOICE-02 — Version của metadata nháp: **B, version riêng + chụp cặp tường minh**",
            "## 1. Lựa chọn của #96",
            TAG_PR2_SCOPE,
        ],
        title: Label(
            "#96 CHOICE-02 version của metadata nháp: B, version riêng + chụp cặp tường minh, lát TAG-PR-02",
        ),
        source_status: Quote(TAG_PR2_STATUS),
        resolution: &[
            Quote("B. `work_draft_metadata.version` riêng"),
            Quote(
                "Lưu văn bản và lưu thẻ độc lập (`text_and_tag_saves_of_one_work_do_not_conflict`). Cặp công khai được chụp tường minh: seam `capture_metadata_snapshot` khóa hàng metadata `FOR SHARE` và chỉ chụp đúng version tác giả xác nhận, nên lưu thẻ đồng thời không thể tạo snapshot lệch (`capture_and_tag_saves_cannot_publish_a_mismatched_snapshot`)",
            ),
        ],
        rationale: &[Quote(
            "Một lần lưu sở hữu cả hai, nhưng autosave văn bản dày đặc sẽ làm mọi lần sửa thẻ ở tab khác xung đột (và ngược lại)",
        )],
        alternative: &[Quote("A. Chung version với văn bản nháp")],
        review_when: &[Quote(
            "Xem lại khi owner publication quyết định một lệnh gửi duyệt phải mang cả version văn bản và version metadata",
        )],
        requires: &["TAG-PR-01-CHOICE-02@r1"],
        related_ids: &["D-15"],
        references: TAG_PG,
        ..D_ROW
    },
    Spec {
        id: "TAG-PR-03-CHOICE-01",
        domain: "taxonomy",
        source: TAG_API,
        blocks: &[
            "### CHOICE-01 — Transport metadata tác giả: **B, resource riêng có version riêng**",
            "## 2. Lựa chọn của #97",
            TAG_PR3_SCOPE,
        ],
        title: Label(
            "#97 CHOICE-01 transport metadata tác giả: B, resource riêng có version riêng, lát TAG-PR-03",
        ),
        source_status: Quote(TAG_PR3_STATUS),
        resolution: &[
            Quote("B. `/drafts/{id}/tags` với `expected_version` riêng"),
            Quote(
                "Khớp storage #96; hợp đồng lưu văn bản **không đổi** (vẫn `deny_unknown_fields`), nên mọi client cũ tiếp tục đúng mà không cần presence semantics. Trong resource thẻ, body là **toàn bộ** trạng thái mong muốn và mọi field bắt buộc: thiếu field là `422`, chỉ `null`/`[]` mới xóa",
            ),
        ],
        rationale: &[Quote(
            "Một lệnh nguyên tử, nhưng #96 đã chọn version metadata **riêng** (CHOICE-02 B); gộp vào lưu văn bản buộc autosave dày đặc mang theo thẻ, và client cũ bỏ trường sẽ mơ hồ giữa “không đổi” và “xóa”",
        )],
        alternative: &[Quote(
            "A. Mở rộng `PUT /drafts/{id}` (văn bản + thẻ một lệnh)",
        )],
        review_when: &[Quote(
            "Xem lại nếu owner publication quyết định lệnh gửi duyệt phải mang cả hai version.",
        )],
        requires: &["TAG-PR-01-CHOICE-02@r1", "TAG-PR-02-CHOICE-02@r1"],
        related_ids: &["D-15"],
        references: TAG_API,
        ..D_ROW
    },
    Spec {
        id: "TAG-PR-03-CHOICE-02",
        domain: "taxonomy",
        source: TAG_API,
        blocks: &[
            "### CHOICE-02 — Entrypoint quản trị catalog: **A, lệnh operator**",
            "## 2. Lựa chọn của #97",
            TAG_PR3_SCOPE,
        ],
        title: Label("#97 CHOICE-02 entrypoint quản trị catalog: A, lệnh operator, lát TAG-PR-03"),
        source_status: Quote(TAG_PR3_STATUS),
        resolution: &[
            Quote("A. CLI operator gọi cùng application/core/store"),
            Quote(
                "Không có authority production (D-05 Provisional) nên một route admin HTTP sẽ không dùng được ở production nhưng vẫn mở thêm bề mặt; CLI dùng đúng mẫu `vink-app moderator`, gọi `Vink::change_catalog` → `CatalogStore` → `decide_catalog_change`, không có SQL đi tắt",
            ),
            Quote(
                "Authority là giá trị `CuratorAuthority` mà chỉ lệnh operator tạo (`CuratorAuthority::operator()`); store quyết định `can_curate` từ nó trong transaction thay đổi.",
            ),
        ],
        rationale: &[Quote(
            "Cần vòng đời grant (cấp/thu hồi/kiểm tại commit), chống lạm dụng và test giả actor/sai vai trò/grant bị thu hồi — trên một authority chưa được nghiệm thu",
        )],
        alternative: &[Quote("B. HTTP hạn chế với grant taxonomy")],
        review_when: &[Quote(
            "Xem lại khi có admin auth đã nghiệm thu và nhu cầu UI curator",
        )],
        requires: &["TAG-PR-01-CHOICE-03@r1", "TAG-PR-02-CHOICE-01@r1"],
        related_ids: &["D-15"],
        references: TAG_API,
        ..D_ROW
    },
    Spec {
        id: "TAG-PR-03-CHOICE-03",
        domain: "taxonomy",
        source: TAG_API,
        blocks: &[
            "### CHOICE-03 — Đối soát khi không biết lệnh đã lưu chưa: **A, desired state có version + đọc lại**",
            "## 2. Lựa chọn của #97",
            TAG_PR3_SCOPE,
        ],
        title: Label(
            "#97 CHOICE-03 đối soát khi không biết lệnh đã lưu chưa: A, desired state có version + đọc lại, lát TAG-PR-03",
        ),
        source_status: Quote(TAG_PR3_STATUS),
        resolution: &[
            Quote("A. Desired state + `expected_version`; `409` kèm thẻ đang lưu; GET để đối soát"),
            Quote(
                "Mọi lần lưu được chấp nhận tăng version (fence #95), body là toàn bộ lựa chọn, nên gửi lại sau timeout không thể âm thầm ghi đè lựa chọn mới hơn. `409` mang `tags` (ID đang lưu) để client so với mong muốn. Không cần bảng receipt hay retention",
            ),
        ],
        rationale: &[
            Quote(
                "Biết chính xác lệnh nào đã commit, nhưng tác giả cần trạng thái hiện tại, không cần lịch sử tab nào thắng; thêm bảng, retention, kiểm payload khác",
            ),
            Quote(
                "`409` có `tags` bằng mong muốn chỉ chứng minh **trạng thái** đang đúng, không chứng minh lệnh nào đã commit.",
            ),
        ],
        alternative: &[Quote(
            "B. Command key theo actor/resource + receipt chính xác (mẫu LVE)",
        )],
        requires: &["TAG-PR-02-CHOICE-02@r1", "TAG-PR-03-CHOICE-01@r1"],
        related_ids: &["D-15"],
        references: TAG_API,
        ..D_ROW
    },
    Spec {
        id: "TAG-PR-04-CHOICE-01",
        domain: "taxonomy",
        source: DISC_API,
        blocks: &[
            "### CHOICE-01 — Hình dạng API: **A, mở rộng `GET /api/v1/works`**",
            "## 2. Lựa chọn của #98",
            TAG_PR4_SCOPE,
        ],
        title: Label("#98 CHOICE-01 hình dạng API: A, mở rộng GET /api/v1/works, lát TAG-PR-04"),
        source_status: Quote(TAG_PR4_STATUS),
        resolution: &[
            Quote("A. Một resource, tham số validate"),
            Quote(
                "Trang chủ đúng là trang đầu của tìm kiếm không lọc: một SQL, một luật hiển thị, một validator, một port SSR. Request mặc định giữ URL, thứ tự và 12 bài; body chỉ **thêm** field (`tags`, `filters`, `next_cursor`) nên consumer cũ không vỡ (consumer web cập nhật DTO cùng PR, fixture hai phía đều chạy)",
            ),
        ],
        rationale: &[
            Quote(
                "Hai bề mặt phải giữ cùng luật hiển thị/thứ tự/thẻ; trang chủ và trang tìm có thể lệch nhau trên cùng dữ liệu",
            ),
            Quote("Đánh đổi: ETag của trang chủ đổi một lần (body có thêm field)."),
        ],
        alternative: &[Quote("B. Endpoint discovery riêng")],
        review_when: &[Quote(
            "Xem lại nếu trang chủ cần nội dung biên tập không phải “mới nhất” (curation)",
        )],
        requires: &["TAG-PR-01-CHOICE-01@r1", "TAG-PR-02-CHOICE-01@r1"],
        related_ids: &["D-15"],
        references: DISC_API,
        ..D_ROW
    },
    Spec {
        id: "TAG-PR-04-CHOICE-02",
        domain: "taxonomy",
        source: DISC_API,
        blocks: &[
            "### CHOICE-02 — Chiến lược PostgreSQL: **join trực tiếp có index + khóa gập dấu lưu sẵn do PostgreSQL duy trì**",
            "## 2. Lựa chọn của #98",
            TAG_PR4_SCOPE,
        ],
        title: Label(
            "#98 CHOICE-02 chiến lược PostgreSQL: join trực tiếp có index + khóa gập dấu lưu sẵn do PostgreSQL duy trì, lát TAG-PR-04",
        ),
        source_status: Quote(TAG_PR4_STATUS),
        resolution: &[
            Quote("A + cột generated"),
            Quote(
                "Join trực tiếp qua `publications` (không authority thứ hai), cộng: hàm SQL `vink_search_key` (IMMUTABLE, ICU tường minh) và cột `GENERATED ALWAYS … STORED` trên `revisions.title` và `principals.display_name`, do PostgreSQL tính trong chính câu lệnh ghi chữ — không writer nào quên được, không job sửa, khóa không thể thuộc chữ khác.",
            ),
            Quote(
                "Hai cài đặt của một khóa: `vink_search_key` (SQL) gương `SearchKey::of` (Rust). **Cả hai vế** của phép so (chữ lưu và truy vấn) đi qua hàm SQL. Rust kiểm tra độ dài, ký tự NUL và yêu cầu có chữ hoặc số.",
            ),
        ],
        rationale: &[
            Quote(
                "Đo trên 20 000 tác phẩm (mục 6): gập 20 000 tiêu đề mỗi lần tìm ~0,2–0,5 s; lọc thẻ bằng `<@ ARRAY(subquery)` làm planner ước lượng sai và chạy ~20 s",
            ),
            Quote(
                "Thêm writer, repair, đồng bộ khi rút/tái bản/đổi tên; không có writer publication production để gắn vào",
            ),
        ],
        alternative: &[
            Quote("A thuần: join trực tiếp, gập dấu lúc đọc"),
            Quote("B thuần: bảng projection tìm kiếm"),
        ],
        review_when: &[Quote(
            "Xem lại khi thư viện công khai tới khoảng 10^5 tác phẩm hoặc đo được độ trễ đáng kể",
        )],
        requires: &["TAG-PR-01-CHOICE-01@r1"],
        related_ids: &["D-15"],
        references: DISC_API,
        ..D_ROW
    },
    Spec {
        id: "TAG-PR-04-CHOICE-03",
        domain: "taxonomy",
        source: DISC_API,
        blocks: &[
            "### CHOICE-03 — Nhiều thẻ: **A, phải có đủ mọi thẻ đã chọn (AND)**",
            "## 2. Lựa chọn của #98",
            TAG_PR4_SCOPE,
        ],
        title: Label("#98 CHOICE-03 nhiều thẻ: A, phải có đủ mọi thẻ đã chọn (AND), lát TAG-PR-04"),
        source_status: Quote(TAG_PR4_STATUS),
        resolution: &[
            Quote("A. AND"),
            Quote(
                "Người đọc thu hẹp dần một tập (“Tản văn” + “Ký ức”); truth table nhỏ, rõ (mục 5); một tác phẩm có ≤ 1 dạng bài + ≤ 3 chủ đề nên tối đa 4 thẻ có nghĩa",
            ),
            Quote(
                "hai dạng bài cùng lúc luôn rỗng (đúng luật một dạng bài, không phải lỗi). UI #100 phải ghi đúng “có tất cả các thẻ”.",
            ),
        ],
        rationale: &[Quote(
            "Rộng hơn nhưng UI phải giải thích hợp; hai dạng bài OR là hợp lý nhưng AND chủ đề lại mất",
        )],
        alternative: &[Quote("B. OR")],
        review_when: &[Quote(
            "Xem lại khi pilot cho thấy người đọc thường muốn hợp.",
        )],
        requires: &["TAG-PR-04-CHOICE-01@r1"],
        related_ids: &["D-15"],
        references: DISC_API,
        ..D_ROW
    },
    Spec {
        id: "TAG-PR-05-CHOICE-01",
        domain: "taxonomy",
        source: TAG_EDITOR,
        blocks: &[
            "| CHOICE-01 bố cục |",
            "| Lưu |",
            "Hai câu OPEN của issue",
            TAG_PR5_SCOPE,
        ],
        title: Label(
            "#99 CHOICE-01 bố cục: A, mục “Thông tin tác phẩm” inline sau trang viết, không lớp phủ, lát TAG-PR-05",
        ),
        source_status: Quote(TAG_PR5_STATUS),
        resolution: &[
            Quote("**A**: mục “Thông tin tác phẩm” inline sau trang viết, không lớp phủ"),
            Quote(
                "Một nút **Lưu** (và Ctrl/Cmd+S) gửi phần đã đổi: thẻ nếu thẻ đổi, chữ nếu chữ đổi; không đổi gì thì lưu chữ như trước",
            ),
        ],
        rationale: &[
            Quote(
                "Danh mục cần nhóm/mô tả dài/nhiều facet: panel/sheet bằng `OverlayLayer` hiện có",
            ),
            Quote("Có autosave: autosave chỉ chữ, thẻ vẫn lưu theo ý định rõ"),
        ],
        requires: &[
            "TAG-PR-01-CHOICE-02@r1",
            "TAG-PR-03-CHOICE-01@r1",
            "TAG-PR-03-CHOICE-03@r1",
        ],
        related_ids: &["D-15"],
        references: TAG_EDITOR,
        ..D_ROW
    },
    Spec {
        id: "TAG-PR-05-CHOICE-02",
        domain: "taxonomy",
        source: TAG_EDITOR,
        blocks: &[
            "| CHOICE-02 ranh giới |",
            "| Giới hạn ở client |",
            "Hai câu OPEN của issue",
            TAG_PR5_SCOPE,
        ],
        title: Label(
            "#99 CHOICE-02 ranh giới: A, composition TagSelector/TagChip trong vink-patterns, lát TAG-PR-05",
        ),
        source_status: Quote(TAG_PR5_STATUS),
        resolution: &[
            Quote(
                "**A**: composition `TagSelector`/`TagChip` trong `vink-patterns`, ghép primitive có sẵn; không primitive/token mới",
            ),
            Quote(
                "Chặn thêm chủ đề thứ 4 và thẻ trùng như projection; server vẫn là authority (`too_many_themes` vẫn được xử lý)",
            ),
        ],
        rationale: &[Quote(
            "Combobox `aria-activedescendant` hoặc chọn nhiều nơi khác cần cùng ngữ nghĩa bàn phím: admit primitive vào `ui-system` với vectors",
        )],
        requires: &["TAG-PR-01-CHOICE-01@r1", "TAG-PR-03-CHOICE-01@r1"],
        related_ids: &["D-15"],
        references: TAG_EDITOR,
        ..D_ROW
    },
];

pub(super) fn verify_import(text: &str) -> Result<(), String> {
    decision_log::verify_bundle(FILE, COMMIT, GROUP_ID, IMPORT_SHA256, text)
}

pub(super) fn load(root: &Path) -> Result<BTreeMap<Key, Entry>, String> {
    decision_log::load_bundle(root, FILE, COMMIT, GROUP_ID, IMPORT_SHA256)
}

pub(super) fn render(root: &Path) -> Result<String, String> {
    decision_log::render_bundle(root, COMMIT, GROUP_ID, SPECS)
}
