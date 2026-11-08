# Phân loại tác phẩm — core và registry TAG-PR-01

Baseline `develop@6c97917b6a7aa4c51f3b2fe816462c8a0a47fc64`, 08/11/2026, Linux x86_64, Rust 1.99.1.
[PRD §13](../../prd/requirements.md#13-phân-loại-tác-phẩm-tag),
[decision record](../architecture/work-taxonomy.md), issue [#95](https://github.com/loveoverflowcom/vink/issues/95).
Owner: `vink-app::domain::taxonomy` (luật) và `vink-app::application::taxonomy` (use case thẻ nháp,
port `WorkTagStore`). #96/#97/#98 gọi đúng các hàm này, không viết lại luật trong SQL/handler.
Adapter PostgreSQL là TAG-PR-02 (#96, [storage](taxonomy-postgres.md)); chưa có route, UI hay authority
production: epic #94 chưa có lát nào chạy end-to-end. Cột “Gap ranh giới” dưới đây ghi cả test
PostgreSQL (`pg::` = `tests/postgres/taxonomy.rs`) đã đóng phần gap của #96.

## Registry traceability

Mapping là coverage, không phải bảng PASS. Tên test bỏ tiền tố module; `d::` là
`domain::taxonomy::tests`, `a::` là `application::taxonomy_tests`; harness Kani bỏ tiền tố `taxonomy_`.

| Yêu cầu / bất biến | Production | Unit / fake | Kani | Gap ranh giới |
| --- | --- | --- | --- | --- |
| TAG-FR-03, UC-09, INV-06 | `TagLabel::parse`, `TagSlug::{from_label, parse}`, `SearchKey::of`, `TagQuery::parse` | `d::labels_keep_marks…`, `labels_reject_other_scripts…`, `label_length…boundary`, `slugs_keep_vietnamese_marks…`, `ma_family_shares_a_search_key_but_never_a_slug`, `slug_parsing_normalizes…`, `search_keys_fold…`, `descriptions_and_queries_are_bounded` | — (chuỗi Unicode ngoài phạm vi Kani) | Collation/xếp hạng ngữ nghĩa tiếng Việt; tìm toàn văn là #98 |
| TAG-FR-01/04/05, UC-02/05, INV-01/02/08 | `decide_catalog_change`, `CatalogTag::restore`, `TagAlias::restore`, `resolve_slug` | `d::creating_tags…`, `every_member_of_the_ma_family…`, `renaming_keeps_identity…redirect`, `aliases_point_only_at_active_canonical_tags`, `catalog_authority_version…first`, `stored_catalog_rows_fail_closed…`, `slugs_resolve_to…` | bốn harness `catalog_*` (tạo/mô tả, đổi tên, alias, ngừng dùng) | Unique slug/một canonical/không xóa: `pg::catalog_rules_hold_in_sql_without_rust`, `pg::catalog_changes_keep_every_slug_with_its_tag`; audit và entrypoint curator (CHOICE-02 #97) |
| TAG-FR-06, UC-04, INV-04 | Retire trong `decide_catalog_change`; nhánh retired của `decide_tag_change`; `public_tags` | `d::retiring_keeps_references…`, `unknown_and_newly_chosen_retired_tags…`, `public_projection_reads_only_the_bound_snapshot`; `a::refusals_stay_distinct…`, `author_view_shows_retired_tags…` | `tag_change_respects_owner_version_slots_bounds_and_retired_tags` | `FOR SHARE` thật, hai thứ tự commit: `pg::retirement_and_assignment_serialize_on_the_catalog_row` |
| TAG-FR-02, UC-01/02, INV-03 | `TagSelection::{new, parse}`, `decide_tag_change`, `WorkTags::restore` | `d::teach_example…`, `selections_are_bounded…`, `selection_dedup_is_exhaustively_first_seen…` (3 280 input), `theme_bound_is_three…`; `a::accepted_save…` | `tag_change_…` (7 hình dạng cụ thể × facts ký hiệu) | Slot 0..3 + FK ghép facet + unique theo tác phẩm: `pg::assignment_and_snapshot_rules_hold_in_sql_without_rust` (không CHECK đếm hàng) |
| TAG-FR-07, UC-07/08/10, INV-05 | `decide_tag_change` (thứ tự ưu tiên), `MetadataVersion`, `WorkTags::matches`, `Vink::save_work_tags` | `d::stale_tab_gets_a_conflict…`, `unchanged_save_still_advances…`, `other_owners_missing_works…`; `a::unauthenticated_and_invalid_input_never_reach_the_store`, `stale_tab_conflicts…lost_ack…`, `store_failures_are_not_turned…` | `tag_change_…`, `version_tokens_are_total` | `pg::stale_tabs_…`, `pg::concurrent_saves_…`, `pg::draft_tags_round_trip_and_stay_private_…`, `pg::suspension_and_tag_saves_…`; receipt chính xác nếu #97 chọn |
| TAG-FR-08, UC-06, INV-07 | `MetadataDigest::of`, `MetadataSnapshot::{capture, restore}`, `public_tags`, `draft_tags`; `ContentHash` v1 không đổi | `d::metadata_digest_matches_independent_v1_vectors` (Python hashlib), `tags_never_change_the_content_hash_v1…`, `public_projection_reads_only_the_bound_snapshot` | — | Digest SQL = Rust = golden và seal lúc commit: `pg::sql_digest_matches_…`; binding qua sửa/thay/rút bằng fixture: `pg::public_tags_follow_the_binding_…`, `pg::capture_and_tag_saves_…`; workflow publish thật: owner publication, chưa có |
| TAG-FR-09/10 | `TagOutcome`/`TagRefusal` chỉ có metadata; `TagError`, `TagRejection`, `TagInput::field` | `a::refusals_stay_distinct…`, `store_failures…`, `unauthenticated…` | `tag_change_…` (refusal chỉ cho chủ ở đúng version) | Ánh xạ HTTP/status/`no-store` (#97) |

Doctest ranh giới type (`quick`, lane doctest): `TagId` không thay `WorkId`; `TagLabel` không
deserialize; `WorkTags` không dựng từ field; `TagChange` không nhận `i64` — mỗi khối có lawful twin.

## Kani

Pin 0.67.0 (bundled `nightly-2025-11-21`), manifest [`services/vink-app/kani`](../../services/vink-app/kani/README.md),
harness tại [`src/domain/taxonomy/proofs.rs`](../../services/vink-app/src/domain/taxonomy/proofs.rs),
`cfg(kani)`, đăng ký trong `xtask/src/kani.rs`. App: 13 discussion + 6 LVE + 6 taxonomy = **25**.

- `version_tokens_are_total`: `MetadataVersion`/`CatalogVersion` với mọi `i64`: hợp lệ ⇔ ≥ 1, `next`
  = +1 hoặc `None` đúng tại `i64::MAX`.
- `tag_change_respects_owner_version_slots_bounds_and_retired_tags`: catalog bốn thẻ với facet/status
  ký hiệu và một thẻ không có trong catalog; owner/standing/version/expected ký hiệu (mọi `i64` ≥ 1);
  bảy hình dạng (selection, current) cụ thể: trùng trạng thái, ba chủ đề với thẻ cũ, bốn chủ đề, một
  danh tính ở hai ô, khử trùng `[2,1,2]`, thẻ lạ, đổi sang dạng bài mới. Chứng minh: chấp nhận ⇔ chủ + đang hoạt động + đúng
  version, version + 1; kết quả bằng selection, ≤ 3 chủ đề, đúng facet, có trong catalog, form không
  nằm trong chủ đề, thẻ ngừng dùng chỉ khi đã có trên nháp; `changed` ⇔ khác current; xung đột trả
  đúng version/observed; mọi refusal catalog chỉ cho chủ ở đúng version, không `InvalidFacts`.
  Covers: thay đổi, trùng trạng thái, giữ thẻ ngừng dùng.
- Bốn harness catalog dùng chung một bộ kiểm (`check_catalog_change`): thay đổi cụ thể, với chủ thể
  có/không, facet/status, holder của slug được claim (không/của chính thẻ/của thẻ khác × canonical,
  alias, tên cũ), facts kế nhiệm, quyền curator, version/expected ký hiệu. Chung cho cả bốn: `NotCurator`
  ⇔ không có quyền; chấp nhận ⇒ đúng version, version + 1, mọi ghi slug thuộc chính thẻ, ghi đầu tiên
  là slug được claim (hàng mới, hoặc hàng có sẵn của chính thẻ và không phải canonical), ghi thứ hai
  chỉ hạ slug canonical cũ thành tên cũ; `SlugTaken` báo đúng holder; xung đột trả đúng version.
  - `catalog_create_and_describe_claim_only_free_slugs`: tạo chỉ khi id và slug còn trống; mô tả chỉ
    trên thẻ đang hoạt động. Covers: tạo, `SlugTaken`, `TagIdTaken`, mô tả trùng trạng thái, `UnknownTag`.
  - `catalog_rename_keeps_identity_and_never_takes_another_tags_slug`: đổi sang slug khác (hai ghi) và
    đổi cách viết cùng slug (không ghi slug). Covers: hai ghi, `SlugTaken`, chỉ đổi nhãn, `Retired`.
  - `catalog_aliases_point_only_at_active_canonical_tags`: alias chỉ cho thẻ đang hoạt động, khác tên
    canonical. Covers: alias mới, `SlugTaken`, `UnknownTag`, `AliasIsCanonicalName`.
  - `catalog_retirement_validates_the_successor_hint`: kế nhiệm khác thẻ, đang hoạt động, cùng facet.
    Covers: ngừng dùng, tự làm kế nhiệm, kế nhiệm khác facet, kế nhiệm không tồn tại, kế nhiệm hợp lệ.

**Bounds và giới hạn.** `#[kani::unwind(17)]` vì so `TagId` là `memcmp` 16 byte; mọi vòng lặp trong
harness có cận cụ thể và giữ unwinding assertions. Danh tính cụ thể, facts ký hiệu: chọn hình dạng bằng
giá trị ký hiệu làm độ dài collection phụ thuộc đường đi và CBMC hết bộ nhớ (đã thử: >12 GB). Vì vậy
khử trùng trên **nội dung** ký hiệu được kiểm bằng unit test vét cạn (mọi chuỗi trên ba danh tính, độ
dài 0..=7) thay vì Kani. Kết quả catalog có danh sách slug dài theo đường đi nên được `mem::forget` sau
khi kiểm, tránh unroll destructor; một harness gộp cả chín thay đổi catalog mất 36 phút và 10,9 GB nên
được tách thành bốn. Không chứng minh: Unicode/chuẩn hóa (nhãn cố định `a`/`A`/`b`), SHA-256 của
digest, unique index, khóa, transaction, I/O, nguồn gốc facts, auth. Các chỗ production được viết để
verifier theo được, cùng ngữ nghĩa và cùng unit test: khử trùng so với tiền tố input (không so với
output đang lớn dần); kiểm dạng bài rồi lặp chủ đề thay vì `Chain`; tách `facts` rồi so owner thay vì
`Option::filter` (tham chiếu phụ thuộc đường đi làm mọi độ dài thành ký hiệu); fast-path ASCII cho NFC
và kiểm chữ cái (chuỗi ASCII vốn là NFC; `ascii_fast_path_agrees_with_full_normalization` so với
chuẩn hóa đầy đủ), nên nhãn `a`/`A`/`b` của harness không đi vào bảng Unicode.

## Không chọn

Quint: trạng thái mỗi tác phẩm là một thanh ghi CAS với thứ tự chấp nhận toàn phần; thuộc tính
nhiều bước có ý nghĩa (fence lệnh trùng trạng thái, tab cũ, mất ACK) là chuỗi hai lệnh, đã có unit
test và Kani; race thật là của PostgreSQL (#96, lane `postgres`). Không thêm lane `verify taxonomy`.

## Bằng chứng phiên

Chạy trên `develop@6c97917` cộng diff của lát này, rồi rebase lên `develop@3055d8a` (#103/#104/#106 không
đổi `services/vink-app/src`, manifest Kani hay `xtask/src/kani.rs`, nên Kani đã kiểm đúng các byte
nguồn đó); `verify quick` chạy lại trên head sau rebase. Linux x86_64, Rust 1.99.1, `cargo-kani 0.67.0`
cài sẵn (`~/.kani/kani-0.67.0`), ngày 08/11/2026. Đây là kết quả của lần chạy đó, không phải trạng thái
xanh vĩnh viễn.

| Lệnh / ranh giới | Kết quả |
| --- | --- |
| `cargo test --locked -p vink-app --lib taxonomy` | PASS — 36 test (30 domain, 6 application) |
| `cargo xtask verify quick` | PASS — 9 bước trên head sau rebase: fmt, ba clippy, 41 doctest (gồm 4 compile_fail + 4 lawful twin của taxonomy), 420 unit/contract, repository checks |
| `cargo xtask verify kani` | PASS — expressions 3/3, App 25/25, 64/64 cover SATISFIED; 22 phút 43 giây, đỉnh RSS 3,5 GB. Taxonomy: thẻ nháp 459 s, bốn catalog 126–209 s, token 0,2 s |
| Negative control unit (production đột biến rồi khôi phục khớp SHA-256) | PASS sensitivity — M1 thẻ ngừng dùng luôn được nhận: `unknown_and_newly_chosen_retired_tags…` và `refusals_stay_distinct…` FAIL; M2 lưu trùng trạng thái không tăng version: `unchanged_save_still_advances…` FAIL; M3 đổi tên lấy slug của thẻ khác: `renaming_keeps_identity…` FAIL |
| Negative control Kani (bản sao riêng dưới `target/neg`, nguồn chính không đổi) | PASS sensitivity — M1 trên harness thẻ nháp (bảng sáu hình dạng, trước khi thêm case dạng bài mới): đúng assertion `found.status.is_active() \|\| current.contains(*id)` FAIL; M3 trên harness đổi tên: đúng assertion `subject_id == holder.tag` FAIL |
| Thiết kế harness bị loại | Độ dài ký hiệu (`Vec` sau khử trùng): hết bộ nhớ 12 GB; `Option::filter` + `Chain`: timeout 15 phút; harness catalog gộp chín thay đổi: 36 phút, 10,9 GB (đã tách) |
| PostgreSQL, HTTP, trình duyệt, Quint | NOT_RUN — không có adapter, route, UI hay model mới |

Tái lập từ repo root: `cargo xtask verify quick`, rồi `cargo xtask verify kani` (cần pin 0.67.0; lệnh cài ở
[development guide](README.md#kani)). GitHub Actions không được đọc hay chờ trong lát này.
