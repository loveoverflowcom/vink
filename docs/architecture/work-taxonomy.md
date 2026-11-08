# Phân loại tác phẩm — quyết định của lát TAG-PR-01

**Ngày:** 08/11/2026. **Nguồn:** [#95](https://github.com/loveoverflowcom/vink/issues/95) (TAG-PR-01,
một phần của epic [#94](https://github.com/loveoverflowcom/vink/issues/94)), do người dùng giao thực thi;
baseline `develop@6c97917b6a7aa4c51f3b2fe816462c8a0a47fc64`. **Trạng thái:** lựa chọn triển khai cho
**pure core** của App, đủ để #96 (PostgreSQL), #97 (catalog/author API) và #98 (discovery) tiêu thụ.
Đây không phải product Accepted, không mở UI, không có store/route production. Yêu cầu có ID ở
[PRD §13](../../prd/requirements.md#13-phân-loại-tác-phẩm-tag); requirement → function → test/proof
ở [registry](../development/taxonomy-core.md).

## 1. Hiện trạng đã đối chiếu (source review)

| Finding | Source | Hệ quả cho core |
| --- | --- | --- |
| `PublicWork`, `PublicWorkSummary`, `SaveDraft` chưa có tag; trang chủ giới hạn 12 | [application/mod.rs](../../services/vink-app/src/application/mod.rs) | Cần owner metadata riêng; discovery toàn thư viện là #98 |
| `ContentHash` v1 chỉ phủ title + document; revision bất biến; endorsement neo `RevisionId` | [revision.rs](../../services/vink-app/src/domain/revision.rs), [LVE CHOICE-02 B](literary-endorsement.md) | Tag không được vào hash v1 hay đổi revision; sửa tag mà tạo revision mới sẽ tách số “Đánh giá cao” |
| `PgStore::save_draft` đặt owner + version trong UPDATE; public read chỉ JOIN publication → revision | [persistence/postgres.rs](../../services/vink-app/src/persistence/postgres.rs) | Metadata nháp và công khai phải tách; kiểm owner/version tại commit (#96) |
| Bật/tắt bình luận và LVE: mọi lệnh được chấp nhận (kể cả trùng trạng thái) tăng version/order | [decisions D-13](decisions.md), [LVE](literary-endorsement.md) | Áp cùng nguyên tắc fence cho metadata, tránh lệnh cũ hồi sinh ý định cũ |
| Web có `search_key` cho danh sách đã tải (NFD, bỏ dấu, Đ→d) | [apps/vink-web/src/search.rs](../../apps/vink-web/src/search.rs) | Consumer tiện ích; provider App sở hữu khóa tìm thẻ riêng, không tạo crate dùng chung |
| Chưa có workflow publish/moderation, chưa có auth production (D-05 Provisional) | [AGENTS.md](../../AGENTS.md), [decisions](decisions.md) | Core chỉ định nghĩa binding; không dựng đường publish hay authority giả |

Discourse (tag groups, quyền theo nhóm) và Stack Overflow (tag wiki, synonym có quản trị) là tài
liệu tham khảo mô hình; Vink không lấy giới hạn năm thẻ, reputation hay quy tắc slug ASCII tiếng Anh.

## 2. Thuật ngữ

| Thuật ngữ | Nghĩa trong Vink | Type |
| --- | --- | --- |
| Thẻ (tag) | Mục do curator quản trong catalog, mô tả tác phẩm; không cấp quyền, không chấm chất lượng | `CatalogTag` |
| Canonical identity | Danh tính bền vững của thẻ; assignment và snapshot chỉ giữ cái này | `TagId` (UUID) |
| Nhãn hiển thị | Chữ người đọc thấy; giữ hoa/thường và dấu; đổi được | `TagLabel` |
| Slug | Khóa URL và khóa trùng tên: chữ thường NFC, nối bằng `-`, **giữ dấu** (`ký-ức`) | `TagSlug` |
| Search key | Khóa so khớp bỏ dấu (`ky uc`); nhiều thẻ có thể chung; không phải danh tính | `SearchKey` |
| Dạng bài (facet Form) | Thơ, tản văn, truyện ngắn…; tối đa một trên tác phẩm | `TagFacet::Form` |
| Chủ đề (facet Theme) | Ký ức, gia đình, đô thị…; tối đa ba | `TagFacet::Theme` |
| Alias | Cách viết khác trỏ tới một thẻ canonical đang hoạt động (`Hồi ức` → `Ký ức`) | `TagAlias` |
| Tên cũ (former slug) | Slug trước khi đổi tên; vẫn dẫn về thẻ đó | `SlugKind::Former` |
| Ngừng dùng (retired) | Không nhận dùng mới; tham chiếu cũ giữ nguyên; có thể gợi ý thẻ kế nhiệm | `TagStatus::Retired` |
| Thẻ nháp | Lựa chọn riêng của tác giả trên bản đang soạn, có version riêng | `WorkTags`, `MetadataVersion` |
| Metadata snapshot | Bộ thẻ bất biến được chụp cho một lần công khai, có digest riêng | `MetadataSnapshot` |

## 3. Lựa chọn và phương án

Mọi lựa chọn dưới đây có điều kiện xem lại; không lựa chọn nào được nâng thành policy sản phẩm
đã chốt chỉ vì có code.

### CHOICE-01 — Taxonomy pilot: **B có giới hạn**

Một catalog, mỗi thẻ thuộc đúng một facet trong enum **đóng** `{Form, Theme}`; luật số lượng là hằng
trong code, không phải engine nhóm do dữ liệu định nghĩa.

| Phương án | Đánh giá |
| --- | --- |
| A. Trường dạng bài riêng + thẻ chủ đề | Dạng bài và chủ đề có cùng vòng đời nhãn/slug/alias/retire; hai owner/API cho cùng vòng đời là trùng lặp |
| **B. Catalog có facet** (chọn) | Tái dùng toàn bộ luật danh tính; facet đóng nên không thành engine taxonomy tổng quát; thêm facet là thay đổi code được review |
| C. Chỉ chủ đề | Vision nêu người đọc tìm theo “thể loại” và pilot bắt đầu với thơ/tản văn/truyện ngắn/nhật ký; gộp “Thơ” vào chủ đề làm mất luật một dạng bài |

Số lượng: **dạng bài ≤ 1** (bằng type `Option`), **chủ đề ≤ 3** thẻ khác nhau sau khi resolve alias và
khử trùng (`MAX_THEMES_PER_WORK`). Ba đủ cho ví dụ “Ký ức / Gia đình” cộng một, giữ mỗi bộ lọc có
nghĩa; đây là giá trị tạm, không chép số năm. Dạng bài **không bắt buộc** ở nháp; có bắt buộc khi gửi
duyệt hay không là quyết định của owner publication. Request thô nhận tối đa 6 tham chiếu chủ đề
(`MAX_THEME_REFS`, mỗi chủ đề hai lần) trước khi khử trùng. Xem lại khi dữ liệu pilot cho thấy tác giả
thường chạm giới hạn, hoặc tác phẩm lai thường cần hai dạng bài (khi đó tăng hằng số, không đổi schema).

### CHOICE-02 — Gắn metadata công khai: **B, metadata snapshot riêng**

| Phương án | Đánh giá |
| --- | --- |
| A. Snapshot thẻ cùng revision | Sửa tag-only phải tạo revision mới: `RevisionId` mới tách số “Đánh giá cao” (LVE neo revision) và lẫn danh tính văn bản với mô tả |
| **B. Snapshot metadata bất biến riêng** (chọn) | Hash v1, `RevisionId` và anchor endorsement không đổi khi sửa thẻ; publication trỏ **cặp** (revision, metadata snapshot) và đổi cặp nguyên tử |

Đánh đổi: thêm một version (thẻ nháp) và một ranh giới binding. Thẻ chỉ là lựa chọn từ catalog đã
duyệt, không chứa chữ tự do của tác giả; tuy vậy cập nhật tag-only **không được** bypass: chỉ
transaction của owner publication (ý định tác giả + authority hiện hành + generation, PUB-01..03) được
đổi binding; có cần duyệt lại snapshot metadata hay không là quyết định của owner đó, còn OPEN.
`MetadataDigest` (SHA-256, separator `vink-work-metadata-v1`, thứ tự chủ đề có nghĩa) là danh tính nội
dung của snapshot, tách hẳn khỏi `ContentHash` v1.

Version nháp: core dùng `MetadataVersion` riêng, là khuyến nghị cho CHOICE-02 của #96 (version metadata
độc lập): sửa thẻ hiếm, autosave văn bản dày; dùng chung version làm hai tab văn bản/thẻ xung đột lẫn
nhau. Nếu #96 chọn version chung, giá trị version nháp được dùng làm `MetadataVersion` (cùng miền ≥ 1,
cùng ngữ nghĩa “mọi lần chấp nhận đều tăng”); semantics của core không đổi.

### CHOICE-03 — Quản trị catalog: **A, curated**

Curator (operator) tạo/đổi tên/mô tả/thêm alias/ngừng dùng; tác giả chỉ chọn thẻ có sẵn. “Chưa có thẻ
này” trả `UnknownTag`, không có nút hay endpoint đề xuất giả. Phương án B (đề xuất chờ duyệt) cần hàng
đợi, quyền, chống lạm dụng và bảo đảm đề xuất không lộ; chưa có dữ liệu pilot cho thấy thiếu thẻ
thường xuyên. Xem lại khi pilot ghi nhận nhiều lượt tìm không ra thẻ. Entrypoint curator (CLI operator
hay HTTP có grant) là CHOICE-02 của #97; core chỉ nhận fact `can_curate` được kiểm lại tại commit.

## 4. Luật của core

**Chữ.** Nhãn: trim, NFC, gộp khoảng trắng, 1–40 ký tự (Unicode scalar sau NFC), chỉ chữ cái
Latin/Việt (chữ ASCII kèm dấu kết hợp, `đ/Đ`), chữ số, khoảng trắng, `-`, `'`, `’`; chữ viết khác, dạng
tương thích (`ﬁ`, `Ａ`) và ký tự có chữ thường dài hơn một ký tự (`İ`) bị từ chối để slug luôn hợp lệ và
không có ký tự dễ nhầm. Slug suy ra từ nhãn, giữ dấu: `ma`, `má`, `mà`, `mả`, `mã`, `mạ` là sáu slug khác
nhau và chung search key `ma`. NFC/NFD là cùng một chữ; bỏ dấu chỉ để tìm, không bao giờ để gộp danh
tính hay suy alias. Mô tả ≤ 280 ký tự một đoạn; truy vấn ≤ 40 ký tự; mọi chuỗi thô bị chặn theo byte
trước khi chuẩn hóa. Không luật nào sửa văn bản tác phẩm.

**Catalog.** Một slug đã từng gọi tên thẻ nào (canonical, alias hay tên cũ) thì mãi gọi thẻ đó. Đổi tên
giữ `TagId`, slug cũ thành tên cũ (redirect); đổi về tên cũ của chính thẻ là hợp lệ. Alias chỉ trỏ thẻ
canonical đang hoạt động nên không thể có chuỗi, vòng hay alias dangling; alias trùng tên canonical
của chính thẻ bị từ chối. Ngừng dùng giữ mọi tham chiếu; kế nhiệm phải là thẻ khác, đang hoạt động,
cùng facet và chỉ là gợi ý. Không có thao tác gộp hai thẻ canonical, xóa thẻ hay kích hoạt lại trong
pilot. Thứ tự quyết định: không phải curator → facts không thuộc lệnh (fail closed) → version cũ
(`Conflict`) → hết version → luật từng thao tác. Mọi thay đổi được chấp nhận, kể cả trùng trạng thái,
tăng `CatalogVersion`.

**Thẻ nháp.** Thứ tự: không có/không phải chủ → `NotFound` (không phân biệt) → bị đình chỉ → facts trùng
(fail closed) → version cũ → `Conflict { current, observed }` → hết version → quá ba chủ đề → từng thẻ
(dạng bài trước, rồi chủ đề theo thứ tự): không có trong catalog, sai facet, đã ngừng dùng mà chưa có
trên nháp. Thẻ đã ngừng dùng **được giữ** nếu đã có trên nháp (không âm thầm viết lại lựa chọn của tác
giả), không được thêm mới; gỡ luôn được. Lệnh được chấp nhận ghi version + 1 và bộ thẻ, kể cả khi bộ
thẻ không đổi (`changed = false`), nên lệnh cũ cùng expected version không thể đến sau và thắng.

**Thử lại.** Mất ACK rồi gửi lại cùng lệnh → `Conflict`; `observed.matches(desired)` cho biết trạng thái
mong muốn đang đúng, **không** chứng minh lệnh nào đã commit. Receipt chính xác theo command key là
CHOICE-03 của #97 nếu cần. Không làm mới expected version rồi gửi lại ý định cũ một cách mù quáng.

**Công khai.** `public_tags` chỉ đọc snapshot đang được bind, theo thứ tự snapshot, với nhãn hiện hành
của catalog (đổi tên hiện ở mọi nơi vì snapshot giữ danh tính); thẻ đã ngừng dùng ở lại trong snapshot
nhưng không hiển thị/không link. Thiếu hàng catalog hoặc sai facet là lỗi toàn vẹn, không phải danh sách
rỗng. `draft_tags` cho tác giả thấy cả thẻ đã ngừng dùng.

## 5. Hợp đồng cho #96/#97/#98

- **Facts do adapter nạp trong transaction**, core không chứng minh nguồn gốc: hàng work (owner),
  hàng metadata nháp `FOR UPDATE`, standing của actor, hàng catalog của các thẻ được chọn `FOR SHARE`
  (tuần tự hóa với retire). Ghi đúng `TagWrite` rồi commit trước khi trả lời
  ([`WorkTagStore`](../../services/vink-app/src/application/ports.rs)). Không ghi văn bản nháp,
  revision, snapshot hay binding.
- **Catalog**: unique index trên slug (bảng slug một hàng mỗi slug, `tag_id` không đổi), một slug
  canonical mỗi thẻ, facet bất biến, không DELETE. `decide_catalog_change` cần holder của slug được
  claim, hàng chủ thể và facts của kế nhiệm; ghi `CatalogWrite` nguyên tử.
- **Publication**: binding (revision, `MetadataSnapshot`) đổi cùng pointer trong transaction publish;
  public read đi qua binding hiện hành, rút bài chặn cả metadata (D-12, PUB-03). Không có route ghi
  metadata công khai.
- **Ánh xạ lỗi**: `TagError` giữ riêng unauthenticated, suspended, not found, conflict (kèm observed),
  invalid input (`field()`), rejected (unknown/retired + gợi ý/wrong facet/too many), unavailable,
  internal; outage không thành danh sách rỗng hay số 0.
- **Tìm/URL** (#97/#98): adapter nạp ứng viên theo tiền tố search key; `rank_matches` xếp exact (cùng
  slug) → folded → prefix, một gợi ý mỗi thẻ canonical, bỏ thẻ ngừng dùng; `resolve_slug` trả
  canonical/redirect/retired/not found. Slug bỏ dấu (`ky-uc`) là slug khác, có thể dùng để gợi ý.

## 6. HOW / WHY / TEACH

**HOW (đường ghi thẻ nháp hiện có):** `Vink::save_work_tags(actor, SaveWorkTags)` → actor (không có →
`Unauthenticated`, không chạm store) → `MetadataVersion::new` → `TagSelection::parse` (đếm tham chiếu trước,
parse UUID, khử trùng giữ thứ tự) → `TagChange` → `WorkTagStore::save_work_tags` (một transaction: nạp
facts → `decide_tag_change` → ghi) → `TagOutcome` → `SavedWorkTags`/`TagError`. Chưa có adapter
PostgreSQL hay route: đường này chạy với store giả gọi đúng policy thật
([taxonomy_tests.rs](../../services/vink-app/src/application/taxonomy_tests.rs)).

**WHY:** mô tả tác phẩm (tag) tách khỏi mong đợi góp ý, tuyển chọn, kiểm duyệt và ACL; một nhãn không
cấp quyền hay chứng nhận chất lượng. Raw input → danh tính/assignment đã validate → policy thuần nhận
facts → port; HTTP/SQL chỉ là adapter, nên luật nằm một chỗ và kiểm được không cần server.

**TEACH — năm tình huống:**

1. *Tản văn / Ký ức / Gia đình.* Nháp version 1 chưa có thẻ. Tác giả chọn dạng bài `Tản văn` và chủ đề
   `Ký ức`, `Gia đình` → chấp nhận, version 2, hiển thị theo thứ tự đó. Thêm `Đô thị`, `Mùa` (bốn chủ đề)
   → `TooManyThemes`, không ghi.
2. *Alias.* Tác giả gõ `hồi` → `rank_matches` gợi ý `Ký ức` qua alias `Hồi ức` (prefix). Chọn nó trong
   khi `Ký ức` đã có → hai tham chiếu cùng một `TagId`, khử trùng còn một; `Ký ức` vẫn chỉ tính một lần.
3. *Sửa nháp khi bản công khai cũ.* Snapshot đang bind là Tản văn / Ký ức / Gia đình. Tác giả sửa nháp
   thành Thơ / Mùa: chỉ hàng metadata nháp đổi; `public_tags` của snapshot cũ y nguyên; hash v1,
   `RevisionId` và số “Đánh giá cao” không đổi. Chỉ lần publish sau (đúng quyền/ý định) mới bind snapshot mới.
4. *Tab cũ.* Hai tab cùng thấy version 4 với `[Ký ức]`. Tab A lưu `[Ký ức, Gia đình]` → version 5. Lệnh
   của tab B (expected 4) đến sau → `Conflict { current: 5, observed: [Ký ức, Gia đình] }`, không ghi.
   Tab A mất ACK và gửi lại → cũng `Conflict`, nhưng `observed` khớp mong muốn của nó.
5. *Có/không dấu.* Catalog có `ma`, `má`, `mà`, `mã`, `mạ`: năm thẻ, năm slug, chung search key `ma`. Tìm
   `MÁ` (hoặc NFD `má`) → `má` exact đứng đầu, bốn thẻ còn lại folded. Tìm `ma` → `ma` exact
   trước. URL `/…/ky-uc` không phải `ký-ức`: không tìm thấy theo slug, chỉ dùng để gợi ý.

Cách kiểm từng tình huống: tên test tương ứng trong
[registry](../development/taxonomy-core.md#registry-traceability).

## 7. Ngoài phạm vi và còn mở

SQL/migration (#96), HTTP/contract, use case catalog/curator và receipt (#97), discovery/query/ETag (#98),
UI (#99/#100), AI gắn thẻ, reputation/follow tag, publication workflow. Còn OPEN: dạng bài có bắt buộc khi
gửi duyệt; snapshot metadata có cần duyệt lại; entrypoint curator; presence semantics khi client cũ bỏ
trường; đề xuất thẻ của tác giả; ngôn ngữ thẻ ngoài Latin/Việt. D-05/06/12 giữ trạng thái hiện hành.
