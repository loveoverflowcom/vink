# Khám phá công khai — tìm toàn thư viện theo chữ và thẻ (TAG-PR-04)

**Ngày:** 08/11/2026. **Nguồn:** [#98](https://github.com/loveoverflowcom/vink/issues/98) (TAG-PR-04, một
phần của epic [#94](https://github.com/loveoverflowcom/vink/issues/94)), baseline `develop@f23df33` (sau
#95–#97). **Trạng thái:** use case, adapter PostgreSQL, migration, hợp đồng HTTP v1 và consumer DTO của
web đã có; kiểm bằng unit, store giả, PostgreSQL thật qua router production và một phép đo 20 000 tác
phẩm. UI tìm/lọc/phân trang đã có trong [TAG-PR-06](discovery-ui.md); chưa có workflow publish (PUB-01..03) hay authority production
(D-05): mọi publication trong test là fixture SQL có nhãn, nên epic #94 chưa có lát nào chạy end-to-end cho
người dùng thật.

Hợp đồng (route, tham số, field, lỗi, cache) là của
[HTTP v1 › Public discovery](../../services/vink-app/contracts/http/v1/README.md#public-discovery-tag-pr-04);
semantics thẻ là của [decision record #95](../architecture/work-taxonomy.md), cách lưu snapshot/binding của
[storage #96](taxonomy-postgres.md), catalog/slug của [API #97](taxonomy-api.md). Tài liệu này ghi lựa chọn,
luồng dữ liệu, phép đo và bằng chứng.

| Owner | File |
| --- | --- |
| Luật thuần: truy vấn, bộ lọc thẻ, kích thước trang, cursor gắn phạm vi, cắt trang, `filter_tags` | [`domain/discovery.rs`](../../services/vink-app/src/domain/discovery.rs) (+ `discovery/tests.rs`) |
| Use case `discover_works`, `publication` (kèm thẻ công khai); thứ tự validate; ánh xạ lỗi | [`application/discovery.rs`](../../services/vink-app/src/application/discovery.rs); resolve slug dùng chung `catalog::resolution` |
| Port `PublicationStore` (`discover_works`, `publication`), facts | [`application/ports.rs`](../../services/vink-app/src/application/ports.rs) |
| SQL một snapshot, keyset, semi-join thẻ | [`persistence/discovery.rs`](../../services/vink-app/src/persistence/discovery.rs) |
| Hàm gập dấu, khóa lưu sẵn, index | [`migrations/20261110000000_public_discovery.sql`](../../services/vink-app/migrations/20261110000000_public_discovery.sql) |
| Route, đọc query thô, DTO, lỗi; SSR trang chủ | [`http/api.rs`](../../services/vink-app/src/http/api.rs), [`http/contract.rs`](../../services/vink-app/src/http/contract.rs), [`http/error.rs`](../../services/vink-app/src/http/error.rs), [`http/ssr.rs`](../../services/vink-app/src/http/ssr.rs) |
| Consumer DTO (chưa render thẻ/bộ lọc) | [`apps/vink-web/src/contract.rs`](../../apps/vink-web/src/contract.rs) |
| Test | `domain::discovery::tests`, [`discovery_tests.rs`](../../services/vink-app/src/application/discovery_tests.rs), `http::tests::work_search_validates_before_reading_and_reports_outages`, `http::contract::tests::taxonomy::discovery_reads_match_fixtures`, `decodes_public_fixtures` (web), [`tests/postgres/discovery.rs`](../../services/vink-app/tests/postgres/discovery.rs) |

## 1. Bề mặt đã có

`GET /api/v1/works?q=&tag=&tag=&limit=&cursor=`: một trang của thư viện công khai, mới nhất trước. Không
tham số = trang chủ (12 bài đầu, ED-01 hữu hạn) và là đúng request SSR `/` gọi trong tiến trình. Mỗi thẻ
trên card và trên `GET /api/v1/works/{id}/publication` là thẻ **đang hoạt động** của snapshot metadata đang
bind, theo thứ tự snapshot, với nhãn hiện hành. `HOME_LIMIT` và `list_publications` không còn: trang chủ
là trang đầu của tìm kiếm không lọc.

## 2. Lựa chọn của #98

Ba câu hỏi OPEN được chọn bằng bằng chứng cục bộ. Đây là lựa chọn triển khai của slice, không phải
product Accepted; mỗi mục có điều kiện xem lại.

### CHOICE-01 — Hình dạng API: **A, mở rộng `GET /api/v1/works`**

<!-- vdr: active TAG-PR-04-CHOICE-01@r1 -->

| Phương án | Đánh giá |
| --- | --- |
| **A. Một resource, tham số validate** (chọn) | Trang chủ đúng là trang đầu của tìm kiếm không lọc: một SQL, một luật hiển thị, một validator, một port SSR. Request mặc định giữ URL, thứ tự và 12 bài; body chỉ **thêm** field (`tags`, `filters`, `next_cursor`) nên consumer cũ không vỡ (consumer web cập nhật DTO cùng PR, fixture hai phía đều chạy) |
| B. Endpoint discovery riêng | Hai bề mặt phải giữ cùng luật hiển thị/thứ tự/thẻ; trang chủ và trang tìm có thể lệch nhau trên cùng dữ liệu |

Đánh đổi: ETag của trang chủ đổi một lần (body có thêm field). Xem lại nếu trang chủ cần nội dung biên
tập không phải “mới nhất” (curation): khi đó trang chủ thành resource riêng, discovery giữ route này.

### CHOICE-02 — Chiến lược PostgreSQL: **join trực tiếp có index + khóa gập dấu lưu sẵn do PostgreSQL duy trì**

<!-- vdr: active TAG-PR-04-CHOICE-02@r1 -->

| Phương án | Đánh giá |
| --- | --- |
| A thuần: join trực tiếp, gập dấu lúc đọc | Đo trên 20 000 tác phẩm (mục 6): gập 20 000 tiêu đề mỗi lần tìm ~0,2–0,5 s; lọc thẻ bằng `<@ ARRAY(subquery)` làm planner ước lượng sai và chạy ~20 s |
| B thuần: bảng projection tìm kiếm | Thêm writer, repair, đồng bộ khi rút/tái bản/đổi tên; không có writer publication production để gắn vào |
| **A + cột generated** (chọn) | Join trực tiếp qua `publications` (không authority thứ hai), cộng: hàm SQL `vink_search_key` (IMMUTABLE, ICU tường minh) và cột `GENERATED ALWAYS … STORED` trên `revisions.title` và `principals.display_name`, do PostgreSQL tính trong chính câu lệnh ghi chữ — không writer nào quên được, không job sửa, khóa không thể thuộc chữ khác. Index `publications (published_at DESC, work_id)` và `work_metadata_snapshot_tags (tag_id, snapshot_id)`; thẻ lọc bằng semi-join `GROUP BY … HAVING count(*) = cardinality`; `SET LOCAL plan_cache_mode = force_custom_plan` |

Hai cài đặt của một khóa: `vink_search_key` (SQL) gương `SearchKey::of` (Rust). **Cả hai vế** của phép so
(chữ lưu và truy vấn) đi qua hàm SQL. Rust kiểm tra độ dài, ký tự NUL và yêu cầu có chữ hoặc số.
Digest cursor dùng chính khóa SQL đã dùng để lọc. Với tiếng Việt và Latin hai bên ra khóa giống hệt
(`sql_search_key_matches_rust_for_vietnamese_and_latin`: mọi chữ cái tiếng Việt hoa/thường × NFC/NFD và
các câu có dấu câu/khoảng trắng/emoji). Ngoài phạm vi đó có khác biệt đã ghi nhận (`x²`, `नमस्ते`, `Ⅻ`):
test khẳng định chúng **vẫn khác** để tài liệu không lỗi thời âm thầm. Nếu truy vấn có khóa SQL rỗng
(chỉ gồm ký tự hai bên phân loại khác), adapter trả không kết quả thay vì khớp mọi tiêu đề. Ví dụ ngoài
phạm vi tiếng Việt, `ΟΣ` và `ος` cùng khóa SQL nên dùng được cùng cursor, còn `οσ` mở một tìm kiếm khác.
Cursor version 2 dùng khóa SQL. Cursor version 1 trả `422 cursor` để client bắt đầu lại từ trang đầu.

Đánh đổi: hai bảng được ghi lại một lần khi migrate; nâng ICU có thể đổi cách gập ký tự hiếm — cách tính
lại là `ALTER TABLE … ALTER COLUMN … SET EXPRESSION AS (vink_search_key(…))`. Server không có ICU làm
migration thất bại thay vì gập khác đi. Truy vấn chữ không có kết quả vẫn đọc mọi publication (tuyến tính,
~4 µs/tác phẩm ở máy đo). Xem lại khi thư viện công khai tới khoảng 10^5 tác phẩm hoặc đo được độ trễ
đáng kể: index trigram (`pg_trgm`) trên hai cột khóa, hoặc projection.

### CHOICE-03 — Nhiều thẻ: **A, phải có đủ mọi thẻ đã chọn (AND)**

<!-- vdr: active TAG-PR-04-CHOICE-03@r1 -->

| Phương án | Đánh giá |
| --- | --- |
| **A. AND** (chọn) | Người đọc thu hẹp dần một tập (“Tản văn” + “Ký ức”); truth table nhỏ, rõ (mục 5); một tác phẩm có ≤ 1 dạng bài + ≤ 3 chủ đề nên tối đa 4 thẻ có nghĩa |
| B. OR | Rộng hơn nhưng UI phải giải thích hợp; hai dạng bài OR là hợp lý nhưng AND chủ đề lại mất |

Đánh đổi: nhiều kết quả rỗng hơn; hai dạng bài cùng lúc luôn rỗng (đúng luật một dạng bài, không phải lỗi).
UI #100 phải ghi đúng “có tất cả các thẻ”. Xem lại khi pilot cho thấy người đọc thường muốn hợp.

### Các lựa chọn phụ

| Lựa chọn | Lý do | Xem lại khi |
| --- | --- | --- |
| `q` khớp **tiền tố từ** của tiêu đề công khai hoặc tên hiển thị hiện hành; không khớp nhãn thẻ, không khớp chữ tác phẩm | Một luật tìm duy nhất với gợi ý thẻ (#97 dùng tiền tố từ); thẻ có tham số riêng và gợi ý riêng (`/tags/suggestions`), tránh hai authority chuẩn hóa | Có yêu cầu full-text bản thảo (ngoài scope #98) |
| `q`, `tag`, `limit`, `cursor` rỗng = vắng; tham số lạ hoặc lặp (`q`, `limit`, `cursor`) = `422 query` | Form GET native gửi `q=` khi ô trống; tham số gõ sai không được âm thầm mở rộng kết quả | — |
| `tag` lặp lại thay vì một chuỗi phân cách | Đúng cách form GET native gửi nhiều ô cùng tên; đếm tối đa 8 giá trị thô trước khi giữ lại | — |
| Slug không có thẻ / thẻ ngừng dùng: `200`, không tác phẩm, `filters.tags[].resolution` nói lý do; slug sai cú pháp: `422 tag` | Ngữ pháp khác sự thật: URL hỏng là lỗi input, thẻ không có hay đã ngừng là sự thật về catalog. Thẻ ngừng dùng không hiện/không link công khai (#95), nên không là facet công khai | #100 cần mã HTTP khác cho trang thẻ (SSR có thể trả `404` từ `not_found`) |
| Không có tổng số; `next_cursor` chỉ có khi đã đọc được thêm một dòng | Đếm buộc đọc hết tập khớp (mất early stop của index); `next_cursor` luôn đúng vì đọc `limit + 1` | Có nguồn đếm đúng và nhu cầu sản phẩm cụ thể |
| `limit` mặc định 12, tối đa 24 | Giữ trang chủ ED-01; trang hữu hạn, không tải cả thư viện | — |
| Cursor = hex của (phiên bản, `published_at` µs, `work_id`, 8 byte digest phạm vi) | Phạm vi gồm khóa SQL dùng để tìm và ID thẻ canonical. Cách viết cùng khóa hoặc alias cùng thẻ vẫn tiếp tục, phạm vi khác trả `422 cursor`. Mọi predicate hiển thị vẫn áp khi cursor bị sửa | Cần ẩn vị trí hoặc chống liệt kê |
| Mỗi trang một snapshot; danh sách nhiều trang không là snapshot | Keyset trên `(published_at, work_id)`: tác phẩm không đổi không lặp/không sót; mới công khai không chen vào trang sau; rút bài biến mất; tái bản nhảy lên đầu (có thể bị client đã qua bỏ lỡ) | Có yêu cầu đọc nhất quán toàn danh sách |
| `force_custom_plan` trong transaction đọc | Generic plan không biết phần tùy chọn nào vắng; chi phí lập plan ~2–4 ms mỗi lần | Đo thấy planning chiếm phần lớn |

## 3. HOW — một truy vấn từ URL tới SQL

```text
GET /api/v1/works?q=Mưa&tag=hồi-ức&limit=12            (không cookie, không actor)
  → works_query: form_urlencoded, chỉ q/tag/limit/cursor; lạ/lặp → 422 query    http/api.rs
  → Vink::discover_works                                                    application/discovery.rs
      WorkQuery::parse("Mưa") → key "mua" (giữ "Mưa" để echo)      → 422 q nếu sai
      TagFilter::parse(["hồi-ức"]) → slug chuẩn hóa, khử trùng      → 422 tag
      PageSize 12; DiscoveryCursor::decode (nếu có)                 → 422 limit / cursor
  → PgStore::discover_works: READ ONLY REPEATABLE READ (một snapshot)      persistence/discovery.rs
      tag_names WHERE slug = ANY → holder (alias → Ký ức); hàng tags của holder (+ kế nhiệm)
      resolve_slug → filter_tags → [Ký ức]   (không có/ngừng dùng → dừng, không tìm)
      SELECT vink_search_key('Mưa') → 'mua'
      SET LOCAL plan_cache_mode = force_custom_plan
      DISCOVERY_SEARCH_SQL: publications → revisions → works → principals
        WHERE strpos(' ' || title_search_key, ' mua') > 0 OR … display_name_search_key …
          AND metadata_snapshot_id IN (SELECT snapshot_id … tag_id = ANY … HAVING count = 1)
          AND (published_at, work_id) sau cursor
        ORDER BY published_at DESC, work_id LIMIT 13
      slot của các snapshot (một câu) → digest kiểm lại; hàng catalog của các thẻ (một câu)
      COMMIT
  → resolution (canonical/redirect/retired/not_found), SearchScope (khóa SQL + ID canonical), public_tags, cut_page
  → WorkList → conditional::respond: ETag mạnh trên byte body, private, no-cache, 304 nếu khớp
```

Số câu lệnh mỗi request bị chặn trên, không phụ thuộc số tác phẩm: tối đa 7 câu đọc (slug, hàng thẻ của
slug, kế nhiệm, khóa truy vấn, tìm, slot, catalog của card; câu nào không cần thì bỏ) cộng `SET TRANSACTION`
và `SET LOCAL`; không N+1, không tải bản thảo. Không bước nào gọi Identity.

## 4. WHY

- **Hiển thị là phép join.** Tác phẩm chỉ công khai qua `publications`. `working_drafts`,
  `work_draft_metadata` và `work_draft_tags` không được nhắc trong adapter, nên tiêu đề riêng, thẻ nháp hay
  tác phẩm chưa/đã rút không thể khớp, đếm, đổi `next_cursor` hay đổi ETag. Thẻ đọc từ snapshot đang bind
  cùng hàng với revision.
- **Một snapshot cho body và validator.** Slug, kết quả, slot và nhãn đọc trong một transaction
  `REPEATABLE READ`; ETag tính từ đúng byte được gửi. Rút/đổi tên commit trước câu lệnh đầu được thấy
  trọn vẹn, commit sau không thấy chút nào.
- **Validate trước khi đọc, lỗi không thành rỗng.** Input sai không chạm database; database sập là `503`
  (không phải trang rỗng, số 0 hay `304`); dữ liệu hỏng (digest, hàng catalog thiếu) là `500`.
- **Một authority cho mỗi luật.** Resolve slug dùng cùng hàm với URL thẻ (#97); chiếu thẻ công khai dùng
  `public_tags` (#95); “thẻ nào là facet” là `filter_tags`; SQL chỉ thực thi.

## 5. TEACH

Dataset ở `tests/postgres/discovery.rs` (fixture SQL có nhãn; một tác phẩm đi qua seam
`capture_metadata_snapshot` thật):

| Tác phẩm | Công khai | Thẻ trên snapshot |
| --- | --- | --- |
| W1 “Mưa tháng Bảy” (Lê Thu) | có | Tản văn + Ký ức, Gia đình |
| W2 “Má tôi” (Đặng Thùy, tên lưu NFD) | có | Thơ + Ký ức |
| W3 “Mà thôi” (Trần Bình) | có | Thơ + Gia đình, Mùa |
| W4 “Đường về” | có | không có snapshot |
| W5 “Giá_trị 50%” | có | snapshot rỗng |
| W6 “Phố thị” | có | Tản văn + Đô thị (ngừng dùng sau đó, kế nhiệm Mùa), Mùa |
| “Ký ức riêng tư” | **chưa** (nháp có thẻ Ký ức) | — |
| W8 “Mưa đã rút” | **đã rút** | Ký ức |
| “Gia phả” | có | Gia đình; nháp sau đó đổi sang Ký ức (riêng tư) |

1. *Chữ.* `ma`/`Má` → W2, W3 (tiền tố từ; `oi` không khớp gì); `MƯA`/`mua` → W1 (W8 đã rút); NFD
   `Đường`/`DUONG VE` → W4; `dang thuy` → W2 (tên tác giả NFD); `tri 5` → W5 (`_`, `%` là dấu phân
   cách, không wildcard); `riêng tư` → không gì.
2. *Thẻ và alias.* `ký-ức` = `hồi-ức` → W1, W2 (không W8, không nháp, không “Gia phả” dù nháp có Ký ức);
   `ký-ức`+`gia-đình` → W1; `thơ`+`tản-văn` → rỗng; `đô-thị` → rỗng + `retired` + kế nhiệm `mùa`; card W6
   chỉ hiện Tản văn, Mùa.
3. *Đổi tên.* Ký ức → “Hoài niệm”: `ký-ức` thành `redirect` tới `hoài-niệm`, cùng W1, W2; nhãn trên mọi
   card đổi; ETag đổi.
4. *Ngoài 12 đầu.* 30 tác phẩm, “Hồi ức tuổi thơ” ở vị trí 26: trang chủ không có, `q=hoi uc` tìm thấy;
   đi hết các trang với `limit` 1/5/7/24 ra đúng thứ tự viết tay, kể cả qua ba tác phẩm cùng thời điểm.
5. *Race.* Trang 1 lấy W1, W2; giữa hai trang, một bài mới được công khai, W3 bị rút, W4 tái bản: trang 2
   là W5, W6 (bài mới và W4 ở đầu trang chủ, không chen vào trang 2). Một cursor dùng cho tìm kiếm khác →
   `422 cursor`, client bắt đầu lại. Hai request đang bay cho hai bộ lọc khác nhau có `filters` khác nhau,
   nên client bỏ được câu trả lời của bộ lọc cũ (#100).

`has_more`/`next_cursor`, nhãn thẻ và ETag cũng là dữ liệu: chúng chỉ được tính trên tập công khai, nên
một sửa nháp riêng tư không đổi được chúng (`304` sau khi lưu thẻ nháp mới).

## 6. Đo đạc

Máy: Intel Core Ultra 7 155H, 22 luồng, 22 GB RAM, Linux; PostgreSQL 17.11 (`postgres:17.11-alpine`,
Podman, cấu hình mặc định + `max_connections=300`), binary release. Dataset: 20 000 publication do SQL tạo
(200 tác giả, mỗi bài một dạng bài và 1–2 chủ đề, snapshot đã niêm phong), `ANALYZE` sau khi nạp. Chạy:
`cargo test --release -p vink-app --test postgres discovery_measurement -- --ignored --nocapture`
(`VINK_DISCOVERY_ROWS` đổi cỡ). Số đo **warm** (shared buffers vừa được nạp; không có lần cold thật vì
database test tạo và hủy trong một test). Không có mục tiêu p95 nào được đặt.

`EXPLAIN (ANALYZE, BUFFERS)` của đúng `DISCOVERY_SEARCH_SQL`, `LIMIT 13`:

| Hình dạng | Trước (join + gập lúc đọc, `<@`) | Sau (lựa chọn hiện tại) | Plan sau |
| --- | --- | --- | --- |
| Trang đầu | ~44 ms (hash join cả 20 000 rồi sort) | 0,38 ms, 146 buffer | index thứ tự, dừng sau 13 |
| `q=mua` | ~500 ms (gập mỗi dòng) | 0,57 ms | index thứ tự + `strpos` trên khóa lưu |
| `q=khong co` (không khớp) | — | 84 ms, 120 806 buffer | đọc hết 20 000 dòng (tuyến tính) |
| `tag=đô-thị` (không bài nào) | — | 3,2 ms | hashed subplan rỗng, lọc mọi dòng index |
| `q=hoi uc` + `ký-ức` | — | 9,1 ms | hashed subplan từ index thẻ |
| `thơ` + `mùa` | ~20 300 ms (nested loop + seq scan, ước lượng sai) | 16,2 ms, 592 buffer | hashed subplan + index thứ tự |
| Cursor giữa danh sách | ~44 ms | 0,37 ms | index bắt đầu tại cursor |

Planning mỗi lần 1,8–3,8 ms. HTTP qua router (trung vị 15 lần, gồm transaction, 3–6 câu đọc, serialize,
ETag): trang chủ 4,8 ms; `q=mua` 5,4 ms; `q`+thẻ 15,2 ms; hai thẻ 18,3 ms; `q` không khớp 66 ms. Cỡ thư viện
pilot hiện tại nhỏ hơn nhiều; con số chỉ cho biết bậc độ lớn và đường cong, không phải SLA.

## 7. Hợp đồng cho #100

- **Route duy nhất:** `GET /api/v1/works` với `q`, `tag` (lặp), `limit`, `cursor`; mọi field và lỗi ở
  [hợp đồng](../../services/vink-app/contracts/http/v1/README.md#public-discovery-tag-pr-04); fixture
  `works.list.json`, `works.search.json`, `works.search.unmatched.json`, `publication.get.json`.
- **SSR:** gọi `Vink::discover_works(DiscoverWorks { … })` trong tiến trình (port `PublicReads` hiện chỉ
  có `works()` mặc định; #100 thêm tham số), không HTTP về chính server, không Identity. URL của app (ví dụ
  `/?q=&tag=`) map 1–1 sang `DiscoverWorks`; giá trị rỗng là vắng như API.
- **Canonical URL thẻ:** khi `resolution = redirect`, chuyển sang `tag.slug`; `retired` hiện kế nhiệm (nếu
  có) và không có kết quả; `not_found` là trạng thái riêng (trang thẻ SSR có thể trả `404`). Không đoán
  slug, không redirect ra ngoài.
- **Cache trình duyệt:** owner READ-02 hiện khóa theo endpoint + `work_id`
  ([freshness](public-read-freshness.md)); #100 phải đưa **cả query string** (q, tag, limit, cursor) vào
  khóa, nếu không một câu trả lời của bộ lọc cũ hiện dưới nhãn mới. So `filters` của câu trả lời với bộ lọc
  đang áp dụng để bỏ câu trả lời trễ.
- **Phân trang:** `next_cursor` là chuỗi mờ; đổi `q`/thẻ phải bỏ cursor (server trả `422 cursor` nếu
  không). Không có tổng số; không tự suy số từ danh sách đã tải.
- **Thẻ trên card/reader:** `tags` đã là thẻ công khai đang hoạt động với nhãn hiện hành; mô tả/kế nhiệm có
  ở `filters.tags[].tag` hoặc `GET /api/v1/tags/slugs/{slug}`.

## 8. AC → bằng chứng

“Unit” là `quick` không database; “fake” là store giả gọi luật thật; “PG” là `tests/postgres` trên
PostgreSQL thật qua router production. Kết quả chạy ở mục 10.

| AC | Bất biến | Oracle | Loại | Gap |
| --- | --- | --- | --- | --- |
| AC-01 | Tìm ngoài 12 đầu; trang hữu hạn, thứ tự xác định | `search_finds_works_beyond_the_first_twelve_in_bounded_ordered_pages` (thứ tự sort trong Rust từ định nghĩa fixture; ties; `limit` 1/5/7/24; cùng request cùng byte); `a_page_is_cut_with_a_cursor_bound_to_the_canonical_search` | PG + fake | — |
| AC-02 | Unicode/alias, nhiều thẻ, cursor có hợp đồng + fixture | `vietnamese_text_aliases_and_tag_filters_follow_an_explicit_truth_table` (danh sách kỳ vọng viết tay); `cursors_are_bound_to_their_search_and_dataset_changes_are_honest`; `cursor_scope_uses_the_same_query_key_as_postgres_matching` (sigma Hy Lạp, cả cùng khóa và khác khóa, từ chối cursor version 1); `sql_search_key_matches_rust_for_vietnamese_and_latin`; `domain::discovery::tests`; `discovery_reads_match_fixtures` + `decodes_public_fixtures` (web) | Unit + PG | — |
| AC-03 | Không lộ nháp/chưa công khai/đã rút qua kết quả, cursor, thẻ hay validator | `private_unpublished_and_withdrawn_works_never_reach_results_or_validators` (tập công khai viết tay; JSON + HTML; `304` sau khi sửa thẻ nháp; rút → ETag mới); `unknown_or_retired_filters_match_nothing_and_say_why` | PG + fake | Publish/rút thật chưa có (fixture) |
| AC-04 | SQL/HTTP/conditional có test thật và đo thật | `search_validators_revalidate_and_stay_coherent_with_their_body` (200→304, đổi nhãn → 200, mỗi body dưới rename đồng thời có một nhãn và ETag của chính nó); `work_search_validates_before_reading_and_reports_outages` (outage `503` không ETag/không lưu); `discovery_measurement` (mục 6) | PG + unit + đo | Tương tranh rename là best-effort (24 đọc song song 8 rename), không ép thứ tự |
| AC-05 | HOW/WHY/TEACH, feature map, verification | Tài liệu này; [feature map](feature-map.md); [development guide](README.md); [hợp đồng](../../services/vink-app/contracts/http/v1/README.md#public-discovery-tag-pr-04) | Tài liệu | — |

## 9. Còn thiếu

- **UI** (#100; #99 chỉ thêm bộ chọn thẻ cho bản nháp): Khám phá vẫn tìm trong 12 card đã tải; chưa có ô lọc thẻ, trang thẻ, phân trang hay
  hiển thị thẻ trên card/reader. Trình duyệt: không đổi UI nên `verify web` chỉ là hồi quy (mục 10); không
  có ảnh chụp mới vì không có thay đổi hiển thị.
- **Publication**: chưa có writer production của `publications`/`metadata_snapshot_id`; mọi kết quả trong
  test đến từ fixture SQL. Không bằng chứng nào ở đây cho thấy tác giả xuất bản được.
- **Quy mô**: truy vấn chữ không khớp là tuyến tính; chưa có `pg_trgm`, tổng số, sắp xếp khác “mới nhất”.
- **Cache trình duyệt cho query** (READ-02): khóa theo query string là việc của #100.
- **Native/CMP**: không consumer.

## 10. Bằng chứng phiên

Chạy trên `develop@f23df33` cộng diff của lát này, Linux x86_64, Rust 1.99.1, PostgreSQL
`postgres:17.11-alpine` trong container tạm riêng của phiên (`-c max_connections=300`, không chạm database
dev), ngày 08/11/2026. Đây là kết quả của lần chạy đó, không phải trạng thái xanh vĩnh viễn.

| Lệnh / ranh giới | Kết quả |
| --- | --- |
| `cargo test -p vink-app --lib` | PASS — 214 test (mới: `domain::discovery` 8, `discovery_tests` 6, fixture discovery 1, HTTP 1). Một lần chạy có `http::identity_tests::sign_in_keeps_secrets_in_host_cookies_and_never_grants_by_itself` FAIL rồi PASS khi chạy lại (2/2) và khi chạy riêng; test dùng một static `REVOKED` chung, không đụng tới code của lát này |
| `cargo test -p vink-web --lib --features ssr` | PASS — 87 test (gồm decode ba fixture works) |
| `cargo test -p vink-app --test postgres discovery` | PASS — 6 test (+1 đo `#[ignore]`) |
| `cargo xtask verify quick` | PASS — 9 bước (fmt, ba clippy, doctest, unit/contract, repository checks) |
| `cargo xtask verify postgres` | PASS — 28 bước (App 82 test + 1 đo `#[ignore]`; store dùng chung 6; Identity 13; provisioning/migrate bằng role tách — migration có cột generated và hàm ICU chạy được bằng role migrator; backup/restore `pg_dump` khớp). Lần chạy đầu FAIL 5 test cũ khẳng định hình dạng cũ (`{"works": []}`, publication không có `tags`, danh sách migration sau #97, đọc publication bằng code mới trên schema cũ); đã sửa test cho hợp đồng cộng thêm, không đổi hành vi |
| `cargo xtask verify web` | PASS — 8 bước (SSR/hydration/editor/reader/endorsement, restart). Lần đầu FAIL môi trường: Chrome không mở DevTools trong sandbox của công cụ chạy lệnh; chạy với `VINK_CHROME` là wrapper thêm `--no-sandbox` (chỉ duyệt localhost của lane). Lần hai FAIL thật: fixture HTTP của `browser-smoke.mjs` vẫn là `{works:[…]}` kiểu cũ nên consumer không decode được; fixture đã cập nhật theo hợp đồng (`filters`, `next_cursor`, `tags`). UI không đổi nên đây chỉ là hồi quy |

**Negative control** (script ngoài repo đột biến production, chạy test mục tiêu, khôi phục và đối chiếu
SHA-256): mọi đột biến đều làm test mục tiêu FAIL.

| Đột biến | Test mục tiêu | Kết quả |
| --- | --- | --- |
| M1 AND → bất kỳ thẻ nào (`HAVING count(*) >= 1`) | truth table | FAIL |
| M2 lọc theo `work_draft_tags` thay vì snapshot công khai | privacy | FAIL |
| M3 bỏ kiểm phạm vi cursor | cursor | FAIL |
| M4 chuỗi con thay vì tiền tố từ | truth table | FAIL |
| M5 thẻ ngừng dùng là facet | truth table | FAIL |
| M7 cursor lặp lại dòng cuối (`>=`) | phân trang ngoài 12 đầu | FAIL |

Kani, Quint: NOT_RUN — lát này không đổi model, không thêm harness (cursor/Unicode thuộc unit test như
các lát trước). GitHub Actions không được đọc hay chờ.
