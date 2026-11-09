# Phân loại tác phẩm — API catalog, thẻ nháp và quản trị TAG-PR-03

**Ngày:** 08/11/2026. **Nguồn:** [#97](https://github.com/loveoverflowcom/vink/issues/97) (TAG-PR-03, một
phần của epic [#94](https://github.com/loveoverflowcom/vink/issues/94)), baseline `develop@f5a266b` (sau
#95 và #96). **Trạng thái:** use case, hợp đồng HTTP v1, lệnh operator và audit của App, kiểm bằng store
giả và PostgreSQL thật. UI thẻ nháp có từ #99 ([editor](taxonomy-editor.md)); chưa có UI khám phá (#100),
workflow publish hay authority
production (D-05 Provisional): production vẫn trả `401` cho mọi route riêng tư, nên epic #94 chưa có lát
nào chạy end-to-end cho người dùng thật.

Semantics thuộc [decision record #95](../architecture/work-taxonomy.md) và [registry core](taxonomy-core.md);
cách lưu, khóa và migration của #96 thuộc [storage](taxonomy-postgres.md). Tài liệu này ghi lựa chọn API,
luồng dữ liệu, cách vận hành catalog và bằng chứng. Route, status và field là của
[hợp đồng HTTP v1](../../services/vink-app/contracts/http/v1/README.md#work-tags-tag-pr-03).

| Owner | File |
| --- | --- |
| Use case catalog: tra cứu, resolve slug, đổi catalog, listing/lịch sử; `CuratorAuthority` | [`application/catalog.rs`](../../services/vink-app/src/application/catalog.rs) |
| Use case thẻ nháp (`work_tags`, `save_work_tags`), slot của refusal, hint kế nhiệm | [`application/taxonomy.rs`](../../services/vink-app/src/application/taxonomy.rs) |
| Port `CatalogReadStore`, `CatalogStore` | [`application/ports.rs`](../../services/vink-app/src/application/ports.rs) |
| Adapter PostgreSQL (đọc catalog, đổi catalog + audit, listing, lịch sử) | [`persistence/taxonomy.rs`](../../services/vink-app/src/persistence/taxonomy.rs) |
| Migration audit | [`migrations/20261109000000_tag_catalog_audit.sql`](../../services/vink-app/migrations/20261109000000_tag_catalog_audit.sql) |
| Route, DTO, ánh xạ lỗi | [`http/api.rs`](../../services/vink-app/src/http/api.rs), [`http/contract.rs`](../../services/vink-app/src/http/contract.rs), [`http/error.rs`](../../services/vink-app/src/http/error.rs) |
| Lệnh operator `vink-app tag …` | [`src/main.rs`](../../services/vink-app/src/main.rs) |
| Test | [`catalog_tests.rs`](../../services/vink-app/src/application/catalog_tests.rs), [`taxonomy_tests.rs`](../../services/vink-app/src/application/taxonomy_tests.rs), `http::tests` trong [`http/mod.rs`](../../services/vink-app/src/http/mod.rs), fixture provider trong `http::contract::tests::taxonomy`, [`tests/postgres/taxonomy_api.rs`](../../services/vink-app/tests/postgres/taxonomy_api.rs) |

## 1. Bề mặt đã có

| Ai | Điểm vào | Authority |
| --- | --- | --- |
| Bất kỳ ai | `GET /api/v1/tags`, `GET /api/v1/tags/suggestions`, `GET /api/v1/tags/slugs/{slug}` | Không resolve actor; không phụ thuộc Identity |
| Tác giả | `GET`/`PUT /api/v1/drafts/{work_id}/tags` | Principal do server resolve; owner, standing, version kiểm lại tại commit |
| Operator | `vink-app tag list|history|create|rename|describe|alias|retire` | Credential database của App (như `vink-app moderator`); mỗi thay đổi có một hàng audit cùng transaction |

Không có route nào ghi catalog, đề xuất thẻ hay ghi metadata công khai. Quyền kiểm duyệt bình luận không
cho quyền gì với catalog (`comment_moderators_get_no_catalog_authority`).

## 2. Lựa chọn của #97

Ba câu hỏi OPEN của issue được chọn bằng bằng chứng cục bộ. Đây là lựa chọn triển khai của slice, không
phải product Accepted; mỗi mục có điều kiện xem lại.

### CHOICE-01 — Transport metadata tác giả: **B, resource riêng có version riêng**

| Phương án | Đánh giá |
| --- | --- |
| A. Mở rộng `PUT /drafts/{id}` (văn bản + thẻ một lệnh) | Một lệnh nguyên tử, nhưng #96 đã chọn version metadata **riêng** (CHOICE-02 B); gộp vào lưu văn bản buộc autosave dày đặc mang theo thẻ, và client cũ bỏ trường sẽ mơ hồ giữa “không đổi” và “xóa” |
| **B. `/drafts/{id}/tags` với `expected_version` riêng** (chọn) | Khớp storage #96; hợp đồng lưu văn bản **không đổi** (vẫn `deny_unknown_fields`), nên mọi client cũ tiếp tục đúng mà không cần presence semantics. Trong resource thẻ, body là **toàn bộ** trạng thái mong muốn và mọi field bắt buộc: thiếu field là `422`, chỉ `null`/`[]` mới xóa |

Đánh đổi: muốn khôi phục cả văn bản lẫn thẻ cần hai request; trả lời lưu thẻ chỉ nói về thẻ, không bao
giờ giả vờ “mọi thứ đã lưu”. Cặp (văn bản, thẻ) cho lần công khai do seam `capture_metadata_snapshot` của
owner publication chụp tường minh (#96). Bằng chứng: `text_and_tag_saves_of_one_work_do_not_conflict`
(#96) và bước “lưu chữ không đụng thẻ” trong `draft_tags_round_trip_privately_with_presence_and_catalog_rules`;
`tag_request_requires_every_field_and_refuses_authority_fields`. Xem lại nếu owner publication quyết định
lệnh gửi duyệt phải mang cả hai version.

### CHOICE-02 — Entrypoint quản trị catalog: **A, lệnh operator**

| Phương án | Đánh giá |
| --- | --- |
| **A. CLI operator gọi cùng application/core/store** (chọn) | Không có authority production (D-05 Provisional) nên một route admin HTTP sẽ không dùng được ở production nhưng vẫn mở thêm bề mặt; CLI dùng đúng mẫu `vink-app moderator`, gọi `Vink::change_catalog` → `CatalogStore` → `decide_catalog_change`, không có SQL đi tắt |
| B. HTTP hạn chế với grant taxonomy | Cần vòng đời grant (cấp/thu hồi/kiểm tại commit), chống lạm dụng và test giả actor/sai vai trò/grant bị thu hồi — trên một authority chưa được nghiệm thu |

Authority là giá trị `CuratorAuthority` mà chỉ lệnh operator tạo (`CuratorAuthority::operator()`); store
quyết định `can_curate` từ nó trong transaction thay đổi. Test `http_never_holds_curator_authority` chặn
mọi file `src/http` nhắc tới nó hoặc gọi `change_catalog`. Đánh đổi: thao tác tay; audit chỉ quy được
thay đổi về **credential database** (`session_user`), không về một người. Xem lại khi có admin auth đã
nghiệm thu và nhu cầu UI curator: khi đó thêm một biến thể authority mà store đọc grant `FOR SHARE` trong
chính transaction `change_catalog`, như kiểm duyệt bình luận.

### CHOICE-03 — Đối soát khi không biết lệnh đã lưu chưa: **A, desired state có version + đọc lại**

| Phương án | Đánh giá |
| --- | --- |
| **A. Desired state + `expected_version`; `409` kèm thẻ đang lưu; GET để đối soát** (chọn) | Mọi lần lưu được chấp nhận tăng version (fence #95), body là toàn bộ lựa chọn, nên gửi lại sau timeout không thể âm thầm ghi đè lựa chọn mới hơn. `409` mang `tags` (ID đang lưu) để client so với mong muốn. Không cần bảng receipt hay retention |
| B. Command key theo actor/resource + receipt chính xác (mẫu LVE) | Biết chính xác lệnh nào đã commit, nhưng tác giả cần trạng thái hiện tại, không cần lịch sử tab nào thắng; thêm bảng, retention, kiểm payload khác |

Giới hạn được nói thẳng: `409` có `tags` bằng mong muốn chỉ chứng minh **trạng thái** đang đúng, không
chứng minh lệnh nào đã commit. Bằng chứng: `lost_acknowledgements_and_parallel_tabs_never_overwrite_a_newer_choice`
(mất ACK + tab khác ghi mới hơn: lệnh gửi lại bị `409` với thẻ của tab kia, PostgreSQL giữ thẻ đó; sáu tab
cùng version: đúng một `200`). Xem lại nếu một consumer cần chứng từ chính xác (ví dụ thao tác hàng loạt).

### Các lựa chọn phụ

| Lựa chọn | Lý do | Xem lại khi |
| --- | --- | --- |
| Tìm ứng viên: nạp toàn bộ catalog **đang hoạt động** (thẻ + alias của thẻ đang hoạt động) trong một snapshot rồi xếp bằng `rank_matches` | Catalog curated nhỏ; không cột `search_key`/index phải đồng bộ với Rust; một luật xếp hạng duy nhất | Catalog vượt khoảng 1 000 tên đang hoạt động, hoặc đo thấy độ trễ gợi ý đáng kể: thêm khóa tìm lưu sẵn + index tiền tố (migration cộng thêm) |
| Danh sách tối đa 200 thẻ, cờ `truncated` | Không cắt im lặng, không phân trang giả | #99 cần chọn dạng bài/chủ đề từ catalog lớn hơn |
| Đọc catalog `no-store`, không validator | Giống tổng hợp LVE: chưa có owner cache/ETag cho catalog; đổi tên/ngừng dùng hiện ngay | Owner READ-02/#98 muốn cache: thêm ETag như READ-01 |
| Route slug ở `/tags/slugs/{slug}` | Tránh một thẻ tên `suggestions` bị route tĩnh che | — |
| `PUT` trả ID, không trả nhãn | Câu trả lời chỉ nói điều transaction đã ghi; đọc nhãn sau commit là snapshot khác và có thể làm một lệnh đã lưu trông như lỗi | #99 cần nhãn ngay: client đã có nhãn của thẻ nó chọn |
| Mã lỗi riêng (`unknown_tag`, `tag_retired`, `wrong_facet`, `too_many_themes`) kèm `field`, `tag_id` | Client sửa đúng ô; `tag_retired` là `409` vì catalog đổi sau khi tác giả đọc | — |
| Audit: bảng append-only, khóa bằng version catalog | Mỗi thay đổi được chấp nhận tăng version đúng 1, nên khoảng trống audit lộ ra; schema từ chối bước version không có audit và audit đi trước version (cả SQL tay) | Có authority theo người: thêm cột principal/grant |

## 3. HOW — từ request tới database

**Tác giả lưu thẻ.**

```text
PUT /api/v1/drafts/{work}/tags  Origin + cookie/Bearer
  → require_same_origin (403 nếu khác)                         http/api.rs
  → Actor: resolver phía server → principal | None | 503       http/mod.rs, identity/
  → 401 nếu None, trước khi đọc body; work_id sai → 404
  → JSON: thiếu/thừa field → 422 body
  → Vink::save_work_tags: TagSelection::parse, MetadataVersion  application/taxonomy.rs
  → PgStore::save_work_tags (MỘT transaction)                   persistence/taxonomy.rs
       principal FOR UPDATE → đình chỉ? → work FOR SHARE (owner)
       → thẻ được chọn FOR SHARE → metadata FOR UPDATE
       → decide_tag_change (core #95) → UPDATE … WHERE version = expected + slot → COMMIT
  → 200 {version, changed, form, themes} | 409 {current_version, tags} | 422/409 refusal
```

**Operator đổi catalog.**

```text
vink-app tag alias 7 <tag-uuid> Hồi ức
  → parse_tag + ChangeCatalog::validate (TRƯỚC khi đọc cấu hình/kết nối; sai → exit 2)
  → Vink::change_catalog(CuratorAuthority::operator(), command)      application/catalog.rs
  → PgStore::change_catalog (MỘT transaction)                         persistence/taxonomy.rs
       tag_catalog FOR UPDATE → thẻ chủ thể → kế nhiệm → slug được claim
       → decide_catalog_change(can_curate từ authority)
       → ghi thẻ/slug + INSERT tag_catalog_changes(version mới, …) → COMMIT
         (trigger hoãn: version phải có audit, audit không đi trước version)
  → stdout "catalog version 8: alias <id>" | exit 2 refused/conflict | exit 1 outage
```

**Người đọc tra thẻ.** `GET /tags/suggestions?q=hồi` → `SearchTags` kiểm `q`/`facet`/`limit` (sai → `422`,
không đọc DB) → `active_catalog` (một snapshot `REPEATABLE READ, READ ONLY`) → lọc facet →
`rank_matches` → `{query, suggestions}`. Không bước nào gọi Identity.

## 4. WHY

Chọn thẻ, tạo thẻ và đổi nghĩa một thẻ là ba hành động với ba authority khác nhau: tác giả chọn trong
catalog cho bản nháp của mình; operator tạo/đổi catalog; không ai sửa metadata công khai ngoài transaction
publish. Vì vậy:

- **Actor do server quyết.** Không field nào trong body chọn actor, owner hay quyền (`actor_id`,
  `owner_id`, `can_curate` đều là `422`); header giả không được tin.
- **Kiểm ở commit, không ở cửa.** Owner, standing, version và trạng thái thẻ được đọc dưới khóa trong
  chính transaction ghi (#96). Một kiểm tra ở HTTP rồi ghi sau sẽ thua race.
- **Một authority validate.** HTTP chỉ ánh xạ; mọi parse (nhãn, slug, query, ID, version) là của domain qua
  application; CLI dùng cùng `ChangeCatalog::validate`.
- **Không biến lỗi thành rỗng.** Database sập là `503`; dữ liệu lưu hỏng là `500`; danh sách rỗng chỉ khi
  catalog thật sự rỗng.

## 5. TEACH — bốn tình huống

1. *Request hợp lệ.* Tác giả mở nháp: `GET …/tags` → version 1, chưa có thẻ. Gõ `hồi` →
   `suggestions` trả `Ký ức` qua alias `Hồi ức`. Gửi `{"expected_version":1,"form":"<Tản văn>","themes":["<Ký ức>","<Gia đình>","<Ký ức>"]}`
   → `200`, version 2, chủ đề khử trùng còn `Ký ức, Gia đình`. Bản công khai không đổi.
2. *Sửa bài người khác.* Principal khác gửi cùng lệnh → `404`, giống hệt work không tồn tại; adapter không
   đọc metadata của người khác và không tạo hàng cho họ. PostgreSQL vẫn không có hàng metadata.
3. *Xung đột.* Hai tab cùng thấy version 2. Tab B lưu `[Mùa]` → version 3. Tab A (expected 2) →
   `409 version_conflict`, `current_version: 3`, `tags: {form: null, themes: ["<Mùa>"]}`. Tab A hiện thẻ
   đang lưu và để tác giả quyết; không gửi lại ý định cũ với version 3.
4. *Mất ACK sau commit.* Tab A gửi version 3 → database commit version 4 → mạng rớt, client timeout.
   **Timeout không có nghĩa là chưa lưu.** Client gửi lại cùng body → `409` với `current_version: 4` và
   `tags` đúng bằng mong muốn → nhận version 4, xong. Nếu giữa chừng tab khác đã lưu thứ khác, `tags` khác
   mong muốn và client phải hỏi lại tác giả. Điều đó nói về trạng thái, không phải chứng từ của lệnh nào.

## 6. Runbook vận hành catalog

Lệnh dùng `DATABASE_URL` (credential runtime) như `vink-app moderator`; cấu hình khác như
[development guide](README.md). Mỗi thay đổi cần **version catalog vừa đọc**:

```bash
vink-app tag list
```

In `catalog version<TAB>N`, rồi mỗi thẻ (`tag<TAB>id<TAB>facet<TAB>status<TAB>slug<TAB>nhãn<TAB>kế nhiệm<TAB>mô tả`)
và mọi tên của nó (`name<TAB>id<TAB>canonical|alias|former<TAB>slug<TAB>nhãn alias`).

```bash
vink-app tag create 1 form Tản văn
```

Các thay đổi khác: `rename <N> <tag> <nhãn>`, `describe <N> <tag> [mô tả]` (không mô tả = xóa mô tả),
`alias <N> <tag> <nhãn>`, `retire <N> <tag> [kế nhiệm]`. Nhãn và mô tả là phần còn lại của dòng lệnh.
Mã thoát: `0` đã commit (in version mới và các hàng đã ghi); `2` input sai (trước khi kết nối), bị từ chối
hoặc xung đột version — **không ghi gì**, đọc lại bằng `tag list` rồi quyết định lại; `1` lỗi runtime (database
không tới được…), không biết đã ghi hay chưa → `tag list`/`tag history` để đối soát. Lệnh không tự thử lại.

```bash
vink-app tag history 20
```

In audit mới nhất trước: `version`, thời điểm, thao tác, thẻ, slug được claim, kế nhiệm, `changed|unchanged`,
authority, role database. Audit không chứa văn bản, bản nháp hay principal.

**Phục hồi.**

- *Đổi tên nhầm:* đổi về tên cũ (`rename <N> <tag> <nhãn cũ>`); slug cũ của chính thẻ được dùng lại, slug
  mới thành tên cũ và vẫn redirect. Không slug nào chuyển sang thẻ khác.
- *Alias nhầm:* alias không xóa được trong pilot (slug thuộc thẻ mãi mãi); có thể đổi cách viết alias của
  chính slug đó. Một tên thật sự sai nghĩa cần quyết định sản phẩm (không có thao tác gỡ hay gộp).
- *Ngừng dùng nhầm:* **terminal** trong pilot: không kích hoạt lại, không đổi kế nhiệm sau đó. Thẻ đã gắn ở
  lại trên nháp và snapshot. Cách phục hồi duy nhất là tạo thẻ mới với nhãn khác (slug cũ vẫn thuộc thẻ đã
  ngừng). Vì vậy kiểm `tag list` và kế nhiệm trước khi `retire`.
- *Không có lệnh SQL tay nào là đường phục hồi:* trigger từ chối xóa/sửa catalog và audit, và từ chối bước
  version không có audit.

**Migration.** `20261109000000_tag_catalog_audit` chỉ thêm bảng `tag_catalog_changes`, hai hàm và ba trigger;
không đụng dữ liệu cũ. Catalog đã có trước migration giữ nguyên lịch sử (không audit bịa cho quá khứ);
thay đổi từ đó về sau được audit
(`upgrading_an_unaudited_catalog_keeps_it_and_audits_from_then_on`). Áp dụng/rollback như mọi migration App
([runbook #96](taxonomy-postgres.md#5-runbook-migration)); không có down migration.

## 7. AC → bằng chứng

“Unit” là `quick` không database; “fake” là store giả gọi policy thật; “PG” là `tests/postgres` trên
PostgreSQL thật qua router production; “binary” là chạy `vink-app tag` thật. Kết quả chạy ở mục 9.

| AC | Bất biến | Oracle | Loại | Gap |
| --- | --- | --- | --- | --- |
| AC-01 | Hợp đồng/fixture cho thành công và lỗi thật; không bịa route | 12 fixture mới so từng field (`catalog_reads_match_fixtures`, `draft_tag_messages_match_fixtures`, `tag_error_envelopes_match_fixtures`); [README v1](../../services/vink-app/contracts/http/v1/README.md#work-tags-tag-pr-03); không route ghi catalog: `tag_routes_validate_report_outages_and_fail_closed` (`405`) | Unit | Consumer web decode fixture thẻ từ #99 (`decodes_tag_fixtures`, `classifies_tag_error_fixtures`); `tag.resolve.*` chờ #100 |
| AC-02 | Actor từ server; authority/version kiểm tại commit; đóng khi chưa có authority | `tag_request_requires_every_field_and_refuses_authority_fields`, `tag_routes_validate_report_outages_and_fail_closed` (401 không authority, Origin), `http_never_holds_curator_authority`; PG: `standing_is_rechecked_when_the_tag_save_commits`, `comment_moderators_get_no_catalog_authority`, `draft_tags_round_trip_…` (chủ khác `404`, không ghi) | Unit + PG | Authority production không có (D-05); test dùng resolver header có nhãn |
| AC-03 | Catalog/alias và thẻ nháp qua core/store thật; mất ACK có semantics rõ | `catalog_tests.rs` (rank/resolve/validate với core thật); PG: `catalog_reads_are_public_ranked_and_survive_an_identity_outage`, `draft_tags_round_trip_…`, `lost_acknowledgements_and_parallel_tabs_…`, `catalog_governance_runs_through_the_use_case_and_audits_every_step` | Fake + PG | Không chứng từ lệnh chính xác (CHOICE-03 A, có chủ đích) |
| AC-04 | Mock chứng minh wiring; HTTP+PG chứng minh lưu/riêng tư/rollback | Fake: `NoStore` panic khi input sai, outage → `Unavailable` (không rỗng), refusal không áp dụng gì; PG: trạng thái bền kiểm bằng SQL độc lập, fault injection audit → rollback cả thay đổi, trigger từ chối bước version không audit/audit sửa xóa; binary: `the_operator_command_changes_and_reports_the_catalog` | Fake + PG + binary | — |
| AC-05 | HOW/WHY/TEACH, feature map, verification | Tài liệu này; [feature map](feature-map.md); [development guide](README.md); [storage](taxonomy-postgres.md), [registry](taxonomy-core.md) | Tài liệu | — |

## 8. Còn thiếu (activation gaps)

- **Authority production** (D-05): mọi route riêng tư vẫn `401` ở production; route thẻ nháp chỉ chạy thật
  với LocalFixture loopback hoặc resolver test.
- **UI**: Bàn viết là consumer Web của thẻ nháp, catalog và gợi ý từ #99 ([editor](taxonomy-editor.md));
  Explore/reader/trang thẻ (#100) chưa có. Consumer mới phải đọc [hợp đồng](../../services/vink-app/contracts/http/v1/README.md#work-tags-tag-pr-03),
  không suy route hay quyền từ prototype.
- **Discovery** (#98): đã có ([discovery](discovery-api.md)): tìm theo thẻ/chữ, ETag, thẻ công khai trên
  card/publication; không đọc `work_draft_tags`; không có tổng số.
- **Publication**: chưa có caller production của `capture_metadata_snapshot`; thẻ công khai chưa xuất hiện
  trên route đọc công khai nào.
- **Curator theo người**: audit quy về credential; grant taxonomy/HTTP admin là CHOICE-02 B khi có authority.

## 9. Bằng chứng phiên

Chạy trên `develop@f5a266b` cộng diff của lát này, Linux x86_64, Rust 1.99.1, PostgreSQL
`postgres:17.11-alpine` trong container tạm riêng của phiên (`-c max_connections=300`, không chạm database
dev), ngày 08/11/2026. Đây là kết quả của lần chạy đó, không phải trạng thái xanh vĩnh viễn; credential không
được in.

| Lệnh / ranh giới | Kết quả |
| --- | --- |
| `cargo test -p vink-app --lib` | PASS — 198 test (gồm `catalog_tests` 8, fixture thẻ 4, `http::tests` thẻ 2, `taxonomy_tests` cập nhật) |
| `cargo test -p vink-app --test postgres taxonomy` | PASS — 24 test (16 của #96, đã cập nhật chữ ký; 8 mới trong `taxonomy_api`, gồm chạy binary `vink-app tag` thật) |
| `cargo xtask verify quick` | PASS — 9 bước: fmt, ba clippy, 41 doctest, 436 unit/contract, repository checks |
| `cargo xtask verify postgres` | PASS — 28 bước: store dùng chung 6, App 76, Identity 13, provisioning/role/worker/Identity/S-03/backup-restore. Hai ghi chú môi trường: (1) chạy mặc định trên máy 22 CPU, vài test **có sẵn** FAIL ở harness sqlx (`failed to connect to setup test database: PoolTimedOut`, không phải assertion); chạy lại với `RUST_TEST_THREADS=8` thì App 76/76 PASS. (2) Từ đường dẫn worktree sâu, bước Identity `open-directory` vượt giới hạn 100 byte của đường dẫn Unix socket (`<repo>/target/verify-identity/open/identity.sock`); lane PASS đủ 28 bước khi xtask chạy qua một symlink ngắn tới cùng worktree (`cargo run --manifest-path <symlink>/Cargo.toml -p xtask -- verify postgres`). Code Identity không đổi trong lát này |

**Negative control** (script ngoài repo đột biến production, chạy test mục tiêu, khôi phục và đối chiếu
SHA-256): mọi đột biến đều làm test mục tiêu FAIL.

| Đột biến | Test mục tiêu | Kết quả |
| --- | --- | --- |
| M1 presence: `form` thiếu thì mặc định `null` (bỏ field = xóa) | `tag_request_requires_every_field_and_refuses_authority_fields` | FAIL |
| M2 slot của refusal luôn là chủ đề | `refusals_stay_distinct_and_write_nothing` | FAIL |
| M3 bỏ hàng audit của thay đổi được chấp nhận | `catalog_governance_runs_through_the_use_case_and_audits_every_step` | FAIL (schema từ chối commit) |
| M4 route danh sách catalog resolve actor | `catalog_reads_are_public_ranked_and_survive_an_identity_outage` | FAIL (`503` khi Identity sập) |
| M5 `409` không mang thẻ đang lưu | `lost_acknowledgements_and_parallel_tabs_never_overwrite_a_newer_choice` | FAIL |
| M6 resolve thẻ ngừng dùng không nạp hàng kế nhiệm | `catalog_reads_are_public_ranked_and_survive_an_identity_outage` | FAIL (`500`) |

Kani, Quint và trình duyệt: NOT_RUN — lát này không đổi `src/domain`, model hay UI. GitHub Actions không được
đọc hay chờ.
