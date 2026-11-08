# Phân loại tác phẩm — PostgreSQL TAG-PR-02

**Ngày:** 08/11/2026. **Nguồn:** [#96](https://github.com/loveoverflowcom/vink/issues/96) (TAG-PR-02, một
phần của epic [#94](https://github.com/loveoverflowcom/vink/issues/94)), baseline
`develop@6d5e224` (sau #95). **Trạng thái:** storage slice của App — migration, adapter, seam
binding công khai và test PostgreSQL thật. Không có HTTP, UI, workflow publish/kiểm duyệt hay
authority production; epic #94 chưa có lát nào chạy end-to-end cho người dùng.

Semantics thuộc [decision record #95](../architecture/work-taxonomy.md) và
[registry core](taxonomy-core.md); tài liệu này chỉ ghi cách lưu, khóa, kiểm và vận hành.

| Owner | File |
| --- | --- |
| Schema, constraint, trigger, hàm digest SQL | [`migrations/20261108000000_work_taxonomy.sql`](../../services/vink-app/migrations/20261108000000_work_taxonomy.sql) |
| Adapter: `WorkTagStore`, `PublicTagStore`, `change_catalog`, `capture_metadata_snapshot` | [`src/persistence/taxonomy.rs`](../../services/vink-app/src/persistence/taxonomy.rs) |
| Port `PublicTagStore`, use case `Vink::public_work_tags` | [`application/ports.rs`](../../services/vink-app/src/application/ports.rs), [`application/taxonomy.rs`](../../services/vink-app/src/application/taxonomy.rs) |
| Test PostgreSQL thật | [`tests/postgres/taxonomy.rs`](../../services/vink-app/tests/postgres/taxonomy.rs) |

## 1. Lựa chọn của #96

Hai câu hỏi OPEN của issue được chọn bằng bằng chứng cục bộ dưới đây. Đây là lựa chọn triển khai
của slice, không phải product Accepted; mỗi mục có điều kiện xem lại.

### CHOICE-01 — Biểu diễn vật lý: **A, catalog và assignment chuẩn hóa**

| Phương án | Đánh giá |
| --- | --- |
| **A. Catalog chuẩn hóa + một hàng cho mỗi slot** (chọn) | `work_draft_tags` và `work_metadata_snapshot_tags` có `slot` 0 (dạng bài) hoặc 1..3 (chủ đề), khóa `(owner, slot)`, `UNIQUE (owner, tag_id)` và FK ghép `(tag_id, facet) → tags (id, facet)`. Sai facet, thẻ dangling, trùng thẻ và chủ đề thứ tư **không biểu diễn được**, không cần CHECK đếm hàng giả vờ thấy hàng khác. Discovery (#98) truy vấn được `tag_id` trực tiếp |
| B. Catalog + payload snapshot JSON/array | Không có FK cho từng phần tử; cần một writer duy nhất kiểm tham chiếu và một chiến lược truy vấn JSON. Đọc một snapshot rẻ hơn chút, nhưng không đáng ở quy mô ≤ 4 thẻ/tác phẩm |

Snapshot là nhiều hàng nên cần chặn “thêm slot vào snapshot cũ”: trigger từ chối `UPDATE`/`DELETE`/
`TRUNCATE`, và một **constraint trigger hoãn tới commit** tính lại digest bằng hàm SQL
`work_metadata_digest` (cùng byte layout với `MetadataDigest::of`) rồi so với `digest` đã lưu. Slot
thêm muộn đổi digest nên commit của transaction đó thất bại. Hai cài đặt độc lập (Rust và SQL) được
so với golden vector Python của #95.

Đánh đổi: tăng `MAX_THEMES_PER_WORK` cần một migration cộng thêm nới hai CHECK `slot BETWEEN 0 AND 3`
(test `assignment_and_snapshot_rules_hold_in_sql_without_rust` chốt hằng số = 3 để nhắc). ADR #95 nói
“tăng hằng số, không đổi schema” cho core; ở database ta cố ý giữ giới hạn cấu trúc. Xem lại nếu
pilot cần nhiều chủ đề hơn hoặc nhiều dạng bài (khi đó slot dạng bài cũng phải đổi).

### CHOICE-02 — Version của metadata nháp: **B, version riêng + chụp cặp tường minh**

| Phương án | Đánh giá |
| --- | --- |
| A. Chung version với văn bản nháp | Một lần lưu sở hữu cả hai, nhưng autosave văn bản dày đặc sẽ làm mọi lần sửa thẻ ở tab khác xung đột (và ngược lại) |
| **B. `work_draft_metadata.version` riêng** (chọn, như khuyến nghị của #95) | Lưu văn bản và lưu thẻ độc lập (`text_and_tag_saves_of_one_work_do_not_conflict`). Cặp công khai được chụp tường minh: seam `capture_metadata_snapshot` khóa hàng metadata `FOR SHARE` và chỉ chụp đúng version tác giả xác nhận, nên lưu thẻ đồng thời không thể tạo snapshot lệch (`capture_and_tag_saves_cannot_publish_a_mismatched_snapshot`) |

Xem lại khi owner publication quyết định một lệnh gửi duyệt phải mang cả version văn bản và version
metadata: khi đó transaction publish khóa cả hai hàng (văn bản trước, metadata sau) và kiểm cả hai.

### Các lựa chọn phụ

| Lựa chọn | Lý do | Phương án bị loại |
| --- | --- | --- |
| Hàng metadata nháp tạo lười; vắng = version 1, không thẻ | Không backfill, không đổi `create_draft`/seed; mọi đường ghi (save, capture) `INSERT … ON CONFLICT DO NOTHING` rồi khóa hàng; save bị từ chối/xung đột rollback cả insert | Backfill mọi work + thêm vào `create_draft`: thêm bất biến “mọi work có hàng” mà fixture SQL khác dễ phá |
| Binding là cột `publications.metadata_snapshot_id`, FK ghép `(work_id, metadata_snapshot_id)` | Cặp (revision, snapshot) đổi bằng **một** `UPDATE` một hàng; đọc công khai không thể ghép hai lần publish; snapshot phải cùng work | Bảng binding riêng: hai lần ghi, có thể quên một nửa |
| Standing fence bằng `principals … FOR UPDATE` | Xung đột với `FOR KEY SHARE` mà FK của `principal_suspensions` lấy khi chèn, nên **mọi** cách chèn đình chỉ (kể cả SQL tay) đều tuần tự với save, không sửa lệnh `suspend`. Đánh đổi: trong lúc một lần lưu thẻ ngắn đang chạy, các insert khác có FK tới chính principal đó (work mới, bình luận, receipt của họ) và các lần lưu thẻ khác của cùng tác giả phải chờ; chấp nhận được vì sửa thẻ hiếm. Xem lại nếu đo thấy chờ đáng kể | Advisory lock: phải sửa lệnh đình chỉ và mọi writer tương lai phải nhớ giao thức. SERIALIZABLE: chỉ phát hiện khi cả hai phía cùng mức |
| Catalog đổi bằng quyền operator (`can_curate: true`) | Giống `grant_moderator`; entrypoint curator là CHOICE-02 của #97 | Bảng grant curator ở #96: đoán trước quyết định của #97 |
| Không lưu `search_key` | Chiến lược tìm ứng viên (prefix index, trigram hay nạp cả catalog nhỏ) là của #97/#98; cột suy ra từ Rust mà không có consumer chỉ là nghĩa vụ đồng bộ | — |

## 2. Schema: database giữ gì, core giữ gì

| Bảng | Database tự giữ | Core #95 quyết định |
| --- | --- | --- |
| `tag_catalog` (một hàng) | version ≥ 1, chỉ tăng, không xóa | Expected version, mọi thay đổi được chấp nhận tăng version |
| `tags` | id/facet bất biến; kế nhiệm khác chính nó, cùng facet (FK ghép), chỉ khi retired; retired là terminal (không sửa gì nữa); `slug` phải là hàng **canonical** trong `tag_names` (FK hoãn); không xóa | Nhãn hợp lệ, slug suy từ nhãn, kế nhiệm đang hoạt động |
| `tag_names` | Một hàng mỗi slug mãi mãi (PK); slug/tag bất biến; đúng một canonical mỗi thẻ (unique index một phía + FK hoãn phía kia); `alias_label` có ⇔ alias; thẻ retired không nhận tên mới; không xóa | Alias chỉ trỏ thẻ canonical đang hoạt động, không trùng tên canonical; đổi tên hạ slug cũ thành former |
| `work_draft_metadata` | version ≥ 1, chỉ tăng, không xóa | Owner, standing, expected version |
| `work_draft_tags` | slot 0 = form, 1..3 = theme, facet qua FK, mỗi thẻ một lần | Thẻ ngừng dùng chỉ được giữ, không thêm mới; ≤ 6 tham chiếu thô |
| `work_metadata_snapshots` + `_tags` | Bất biến; digest khớp các slot lúc commit; cùng luật slot/facet | Snapshot chụp từ đúng version |
| `publications.metadata_snapshot_id` | Snapshot thuộc cùng work; NULL = công khai không metadata | Chỉ transaction của owner publication được đổi (PUB-01..03) |

Role runtime nhận `SELECT/INSERT/UPDATE/DELETE` qua default privileges của provisioning; việc
từ chối xóa/sửa là của trigger, áp cho mọi role (kể cả superuser, trừ khi tắt trigger có chủ đích).

## 3. Khóa, thứ tự và lỗi

| Transaction | Thứ tự khóa |
| --- | --- |
| Lưu thẻ nháp (`save_work_tags`) | principal `FOR UPDATE` → (câu lệnh mới) đọc đình chỉ → work `FOR SHARE` → thẻ được chọn `FOR SHARE` theo id → `INSERT` hàng metadata nếu vắng → metadata `FOR UPDATE` → quyết định → `UPDATE … WHERE version = expected` + thay slot → commit |
| Đổi catalog (`change_catalog`) | `tag_catalog FOR UPDATE` → thẻ chủ thể `FOR NO KEY UPDATE` → kế nhiệm `FOR SHARE` → hàng slug được claim `FOR NO KEY UPDATE` → quyết định → ghi version, hàng thẻ, slug (hạ cấp trước, claim canonical sau) → commit |
| Chụp snapshot (seam publication) | `INSERT` hàng metadata nếu vắng → metadata `FOR SHARE` → kiểm version → chèn snapshot + slot; digest kiểm lúc commit của caller |
| Đọc nháp/công khai | Một transaction `REPEATABLE READ, READ ONLY`; không khóa, không bị writer chặn |

Đổi catalog không bao giờ khóa principal, work hay hàng nháp, còn save chỉ lấy khóa chia sẻ trên hàng
catalog, nên hai thứ tự không thể tạo vòng. Đình chỉ chỉ khóa principal (qua FK) nên không vòng với
save. Không có lời gọi mạng trong các transaction này.

**Thử lại và ánh xạ lỗi.** Không adapter nào tự thử lại một ý định: xung đột version trả
`TagError::VersionConflict { current, observed }` / `CatalogOutcome::Conflict` để client đối soát.
Lỗi kết nối (`08*`, `53*`, `57*`, pool) là `StoreError::Unavailable` → `TagError::Unavailable` /
`ReadError::Unavailable`; vi phạm constraint, trigger hay dữ liệu lưu không qua `restore` là
`StoreError::Integrity` (log `error`, không chứa giá trị bind) → `Internal`, không bao giờ thành danh
sách rỗng. Principal không có hàng là `UnknownPrincipal` → `Unauthenticated`. Work vắng và work của
người khác cùng là `NotFound`; adapter không đọc metadata của người khác và không tạo hàng cho họ.

## 4. HOW / WHY / TEACH

**HOW — một truy vấn thật.** `PgStore::save_work_tags` (adapter) chạy đúng các câu lệnh ở bảng trên;
câu quan trọng nhất là `SELECT … FROM tags WHERE id = ANY($1) ORDER BY id FOR SHARE`: ở READ
COMMITTED, nếu một lệnh ngừng dùng đang giữ hàng, câu này **chờ**, rồi trả về phiên bản **mới** của
hàng sau khi lệnh kia commit, nên `decide_tag_change` thấy `Retired` và từ chối. Đọc công khai
`PgStore::public_work_tags` đọc `publications.revision_id, metadata_snapshot_id` từ **một** hàng, rồi
slot và hàng catalog trong cùng snapshot; không câu nào join `work_draft_tags`.

**WHY.** Kiểm trước rồi ghi sau không đủ khi có hai caller: luật phải nằm ở nơi cả hai cùng đi qua —
khóa hàng (tuần tự hóa), điều kiện trong `UPDATE` (CAS) và constraint (cấu trúc). Phân biệt bốn thứ:
*expected version* là ý định của client (xung đột → đối soát); *isolation/khóa* quyết định ai thấy gì
và ai chờ ai; *constraint* bắt trạng thái không quyết định nào tạo ra (lỗi toàn vẹn, không phải lỗi
người dùng); *lỗi nghiệp vụ* (thẻ lạ, sai facet, quá ba chủ đề, đình chỉ) là refusal của core, không
ghi gì.

**TEACH — timeline hai transaction** (mỗi dòng là một test; “T1/T2” là hai kết nối thật):

1. *Cùng sửa metadata.* T1 và T2 cùng gửi expected 1. Cả hai khóa principal rồi hàng metadata; T1
   commit version 2; T2 đọc lại hàng (đã là 2) → `Conflict { current: 2, observed: thẻ của T1 }`, không
   ghi. Tám lệnh đồng thời: đúng một thắng (`concurrent_saves_of_one_metadata_version_have_one_winner`).
2. *Sửa nội dung cùng lúc sửa thẻ.* Lưu văn bản khóa `working_drafts`, lưu thẻ khóa
   `work_draft_metadata`: không xung đột, mỗi bên version 2 (`text_and_tag_saves_…`). Khi publish, seam
   chụp khóa metadata `FOR SHARE`: lưu thẻ đến sau phải chờ, và một lần chụp gặp version mới hơn trả
   `Conflict`, nên không có cặp (văn bản A, thẻ B) mà tác giả chưa xác nhận (`capture_and_tag_saves_…`).
3. *Ngừng dùng thẻ khi đang gắn.* (a) Lệnh ngừng dùng giữ hàng `Đô thị` trước; save thêm `Đô thị` chờ,
   rồi bị `RetiredTag { successor: Mùa }`. (b) Save giữ `Gia đình` `FOR SHARE` trước; lệnh ngừng dùng
   chờ save commit; thẻ ở lại trên nháp như thẻ đã có (`retirement_and_assignment_…`).
4. *Đình chỉ.* (a) Đình chỉ chưa commit giữ principal qua FK; save chờ rồi thấy `Suspended`. (b) Save
   đang giữ principal; lệnh `suspend` thật chờ; save commit trước, lần lưu sau bị từ chối
   (`suspension_and_tag_saves_…`). Đây chỉ là đình chỉ của Vink; Identity thu hồi phiên nhanh tới đâu là
   ranh giới khác.
5. *Rút bài khi có đọc công khai.* Rút bài chưa commit: đọc vẫn thấy bản cũ và không bị chặn. Đọc bắt
   đầu **sau** commit: `NotFound` cho cả văn bản lẫn thẻ. Đọc đã bắt đầu trước commit (snapshot đã lấy)
   hoàn tất với trạng thái cũ; byte đã gửi không thu hồi được
   (`public_tags_follow_the_binding_through_edits_replacement_and_withdrawal`).

## 5. Runbook migration

- **Áp dụng:** như mọi migration App: `vink-app migrate` bằng role migrator
  ([runbook](../../services/postgres/README.md#migration-theo-owner-restart-và-rollback-code)). Migration
  chạy trong một transaction: lỗi giữa chừng không để lại gì, sửa nguyên nhân rồi chạy lại.
  `ALTER TABLE publications` lấy khóa `ACCESS EXCLUSIVE` ngắn (thêm cột nullable không default) và
  kiểm FK trên cột toàn NULL; ở quy mô hiện tại là tức thời nhưng vẫn chặn đọc/ghi publication trong
  lúc đó.
- **Kiểm sau migrate:** readiness `current`; `SELECT version FROM tag_catalog` = 1; `tags`,
  `work_draft_metadata` rỗng; tác phẩm cũ đọc như không thẻ (test
  `upgrade_keeps_existing_untagged_works_readable_without_backfill` đi đúng đường này).
- **Backfill:** không có. Chỉ curator thêm thẻ vào catalog; chỉ tác giả gắn thẻ; không suy từ văn bản.
- **Rollback / forward recovery:** không có down migration. Binary cũ gặp lịch sử mới sẽ `diverged`
  (tín hiệu dừng). Nếu phải bỏ tính năng: một migration **mới** `DROP COLUMN metadata_snapshot_id`
  rồi các bảng taxonomy — chỉ khi đã quyết định bỏ dữ liệu thẻ, sau backup theo owner. Không xóa dòng
  `_sqlx_migrations`, không tắt trigger để sửa dữ liệu; hàng lỗi phải được kiểm tay như hướng dẫn của
  `20261105120000_endorsement_version_parity.sql`.
- **Dọn scratch của test:** `#[sqlx::test]` tạo database tạm cho mỗi test và giữ database của test
  thất bại để điều tra; lần chạy sau tự dọn. Container PostgreSQL tạm của phiên được xóa sau khi kiểm.

## 6. AC → bằng chứng

Kết quả chạy ở mục 7. “Core” là luật thuần của #95, “fake” là wiring application, “PG” là PostgreSQL
thật, “fixture” là thao tác publication bằng SQL có nhãn.

| AC | Bất biến | Oracle | Loại | Gap |
| --- | --- | --- | --- | --- |
| AC-01 | Schema đúng #95; không suy thẻ; DB tự giữ luật cấu trúc | `catalog_rules_hold_in_sql_without_rust`, `assignment_and_snapshot_rules_hold_in_sql_without_rust`, `sql_digest_matches_rust_and_the_independent_vectors`, `upgrade_keeps_existing_untagged_works_readable_without_backfill` | PG (SQL trực tiếp) | Role runtime chỉ kiểm qua lane provisioning chung, không có test riêng cho bảng mới |
| AC-02 | CAS, owner/standing tại commit, nguyên tử | `stale_tabs_…`, `concurrent_saves_…`, `draft_tags_round_trip_and_stay_private_…`, `catalog_refusals_and_suspension_write_nothing`, `suspension_and_tag_saves_…`, `a_failure_after_partial_work_rolls_the_whole_transaction_back`, `catalog_changes_keep_every_slug_with_its_tag` | PG | Không có authority production (D-05 Provisional); actor trong test là principal cố định |
| AC-03 | Sửa nháp, thay publication, rút bài không lộ/lệch | `public_tags_follow_the_binding_…`, `capture_and_tag_saves_cannot_publish_a_mismatched_snapshot`; `public_tags_come_only_from_the_bound_snapshot_and_fail_closed` | PG + fixture; fake | Không có workflow publish/rút bài thật; capture chưa có caller production |
| AC-04 | Race có đồng bộ, rollback, constraint | Chờ được **quan sát** qua `pg_blocking_pids` (`wait_blocked*`), hai thứ tự commit; barrier cho lệnh đồng thời; fault injection trigger cho rollback | PG | — |
| AC-05 | HOW/WHY/TEACH, runbook, verification map | Tài liệu này, [registry](taxonomy-core.md), [feature map](feature-map.md), [development guide](README.md) | Tài liệu | — |

**Còn thiếu cho #97/#98 và owner publication (kiểm lại khi tích hợp):**

- #97: use case/entrypoint curator gọi `change_catalog` (hoặc grant DB kiểm `FOR SHARE` trong cùng
  transaction); HTTP cho thẻ nháp với `no-store`, presence semantics khi client cũ bỏ trường; đọc
  catalog/lookup bằng `rank_matches`/`resolve_slug` (cần truy vấn ứng viên — chưa có index tìm).
- #98: truy vấn discovery qua `publications.metadata_snapshot_id → work_metadata_snapshot_tags`
  (thêm index `(tag_id, snapshot_id)` khi đo query plan); không bao giờ đếm từ `work_draft_tags`.
  Consumer render văn bản và thẻ từ hai lần đọc phải so `revision_id` (hoặc gộp một truy vấn).
- Owner publication: gọi `capture_metadata_snapshot` trong transaction publish **sau** khi kiểm ý định
  tác giả, quyền và moderation; khóa văn bản nháp trước metadata; bind bằng một `UPDATE` cả cặp; rút bài
  xóa pointer (snapshot giữ làm lịch sử). Có duyệt lại snapshot metadata hay bắt buộc dạng bài khi gửi
  duyệt vẫn OPEN (#95).

## 7. Bằng chứng phiên

Chạy trên `develop@eed4cde` cộng commit của lát này (rebase từ `6d5e224`; develop mới chỉ đổi UI/docs), Linux x86_64, Rust 1.99.1, PostgreSQL
`postgres:17.11-alpine` trong container tạm riêng của phiên (không chạm database dev), ngày
08/11/2026. Đây là kết quả của lần chạy đó, không phải trạng thái xanh vĩnh viễn; credential
không được in.

| Lệnh / ranh giới | Kết quả |
| --- | --- |
| `cargo test -p vink-app --lib taxonomy` | PASS — 37 test (30 domain, 7 application; thêm `public_tags_come_only_from_the_bound_snapshot_and_fail_closed`) |
| `cargo test -p vink-app --test postgres taxonomy` (`DATABASE_URL` = superuser của container tạm) | PASS — 16 test |
| `cargo xtask verify quick` | PASS — 9 bước: fmt, ba clippy, 41 doctest, 421 unit/contract, repository checks |
| `cargo xtask verify postgres` | PASS — store dùng chung 6, App 68 (52 có sẵn + 16 taxonomy), Identity 13, provisioning/role/worker/Identity/S-03/backup-restore. Lần chạy đầu trên container `max_connections=100` FAIL 3 test có sẵn vì `PoolTimedOut` khi harness sqlx tạo database (máy 22 CPU chạy song song); ba test đó PASS khi chạy riêng, và cả lane PASS với `-c max_connections=300`. CI (runner nhỏ) không chạm giới hạn này |

**Negative control** (đột biến `src/persistence/taxonomy.rs` bằng script ngoài repo, chạy test mục
tiêu, khôi phục và đối chiếu SHA-256 `96809707…`): mọi đột biến đều làm test mục tiêu FAIL.

| Đột biến | Test mục tiêu | Kết quả |
| --- | --- | --- |
| M1 version guard: facts lấy version = expected và bỏ `AND version = $2` | `stale_tabs_…` | FAIL — `Internal` thay vì `VersionConflict` (trigger “version must advance” chặn ghi lùi: lớp phòng thủ thứ ba) |
| M2 access guard: bỏ so owner | `draft_tags_round_trip_and_stay_private_…` | FAIL — lệnh của chủ khác được chấp nhận |
| M3 binding guard: đọc công khai từ `work_draft_tags` | `public_tags_follow_the_binding_…` | FAIL — thẻ nháp Thơ/Mùa lộ ra công khai |
| M4 standing fence: principal `FOR KEY SHARE` | `suspension_and_tag_saves_…` | FAIL — save không chờ đình chỉ (`wait_blocked_by` hết hạn) |
| M5 retire fence: bỏ `FOR SHARE` trên thẻ được chọn | `retirement_and_assignment_…` | FAIL — chỉ 1 backend bị chặn, save không chờ lệnh ngừng dùng |
| M6 capture lock: bỏ `FOR SHARE` của seam chụp | `capture_and_tag_saves_…` | FAIL — save không chờ transaction publish |

Kani, Quint và trình duyệt: NOT_RUN — lát này không đổi `src/domain`, model hay UI. GitHub Actions
không được đọc hay chờ.
