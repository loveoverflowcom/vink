# Đọc công khai có điều kiện (READ-01)

**Issue:** [#52](https://github.com/loveoverflowcom/vink/issues/52) · **Nền:** `develop@c6aa486`
· **Trạng thái:** pilot đã triển khai cho hai endpoint; đã kiểm bằng unit, PostgreSQL thật và
Chrome headless cục bộ. Chưa đo trên môi trường production. READ-02
([#53](https://github.com/loveoverflowcom/vink/issues/53)) vẫn sở hữu query cache/SWR, lifecycle
và UI; lát này không thêm cache phía server, Service Worker hay CDN.

Hợp đồng chính xác nằm ở [API v1 — Conditional public reads](../../services/vink-app/contracts/http/v1/README.md#conditional-public-reads-read-01).
Trang này ghi lý do chọn, ranh giới, bằng chứng và cách rút lại.

## HOW

```text
GET/HEAD /api/v1/works | /api/v1/works/{id}/publication           http/api.rs
  → Vink::public_works / Vink::publication                         application (không đổi)
      → PgStore: một câu SELECT, một snapshot                       persistence/postgres.rs (không đổi)
  lỗi 404/503/500 → trả ngay, layer gắn Cache-Control: no-store
  thành công → conditional::respond(headers, dto)                   http/conditional.rs
      serde_json::to_vec(dto) một lần → ETag = "hex(SHA-256[..16])"
      If-None-Match khớp (hoặc *) → 304 rỗng + ETag + private, no-cache
      không khớp/vắng/sai cú pháp → 200 + đúng các byte đó
```

SSR vẫn đọc in-process qua `SharedPublicReads`; HTML reader/Khám phá không mang ETag của API.
Draft và discussion giữ `no-store`, không có validator. `PUT` draft vẫn dùng
`expected_version`/`409`, không đổi sang `If-Match`.

## Quyết định: phương án A — hash representation

<!-- vdr: active READ-01-CHOICE-01@r1 -->
<!-- vdr: active READ-01-CHOICE-02@r1 -->
<!-- vdr: active READ-01-CHOICE-03@r1 -->
<!-- vdr: active READ-01-CHOICE-04@r1 -->

| Lựa chọn | Lý do |
| --- | --- |
| **A, ETag = SHA-256 của byte JSON sau query** | Bao phủ mọi trường thật sự gửi đi (title, byline, document, revision, `published_at`, thứ tự, membership) mà không cần danh sách phụ thuộc. Đổi tên tác giả không đổi `revision_id` nhưng đổi tag. Không thêm cột, không thêm migration. |
| Không chọn B (validator từ revision/generation) | Phải chứng minh đủ phụ thuộc (byline nằm ở `principals`, thứ tự/membership của list) và tách query metadata/body sẽ tạo race. Chưa có benchmark nào cho thấy A là nút thắt. |
| C (giữ 200) vẫn là đường rút lui | Xem [Rút lại](#rút-lại-theo-endpoint). |
| **Strong**, không weak | Tag là digest của đúng byte gửi đi; không có content coding (`CompressionLayer` không tồn tại), nên byte hash = byte trên dây. Thêm nén thì phải đổi tag theo coding hoặc chuyển weak trước. |
| 128 bit đầu của SHA-256, hex | Đủ chống trùng cho số representation của một URL; header ngắn. Tag là opaque: client không được suy diễn cấu trúc. |
| `Cache-Control: private, no-cache` | Trình duyệt được lưu nhưng phải hỏi lại server trước mỗi lần dùng; không shared cache (chưa có CDN và nội dung rút bài không được nằm ở cache trung gian). Không đặt TTL, nên không có hai TTL chồng nhau với READ-02. |
| Lỗi: `no-store` | `404`/`503`/`500` không bị lưu, nên không giữ một 404 qua lúc phục hồi. |
| Không `Vary` | Không header nào chọn representation (không nén, không locale/filter). |

Body và tag đến từ **một** lần serialize của **một** câu SQL (PostgreSQL READ COMMITTED cho mỗi
câu một snapshot), nên không thể gắn tag của bản A lên body B. Không giữ transaction qua request.

## Thứ tự đánh giá và chính sách input

1. Use case trả lỗi trước (`404` cho thiếu/chưa công khai/ID sai, `503` khi PostgreSQL không
   trả lời, `500` khi dữ liệu lưu sai). Không bao giờ đoán “not modified” khi chưa đọc được.
2. Chỉ representation đang tồn tại mới được so với `If-None-Match`.
3. Parser theo grammar RFC 9110 trên byte (không substring, không tách chuỗi bằng dấu phẩy vì
   dấu phẩy hợp lệ trong tag): `*` đứng một mình; hoặc danh sách `[W/]"etagc*"` có OWS và phần tử
   rỗng; nhiều dòng header ghép thành một list; so khớp weak (bỏ `W/`).
4. Field sai cú pháp, `*` lẫn trong list, hơn 32 tag hoặc hơn 4096 byte: **bỏ qua** và trả `200`
   đủ body. Bỏ qua chỉ có thể tốn byte, không thể tạo 304 sai hay vượt quyền: tài nguyên đã được
   xác nhận công khai ở bước 1.

Public read không gọi Identity; header `Authorization` không ảnh hưởng hai route này.

## Rút bài và bản đã gửi

Request bắt đầu đọc sau khi lệnh rút (hiện là SQL fixture `DELETE FROM publications`) commit sẽ
nhận `404 no-store`, kể cả khi gửi tag cũ hoặc `*`. Request có snapshot trước commit có thể còn
trả bản cũ — đó là ngữ nghĩa của một câu SELECT đã bắt đầu. Bản sao đã nằm ở client không bị
endpoint này thu hồi; READ-02 phải quyết định hiển thị bản cũ/offline và xóa khi có bằng chứng mất
quyền. PUB-03 vẫn chưa hoàn tất: chưa có lệnh rút bài sản phẩm.

## Phân chia với READ-02

| Trách nhiệm | Owner hiện tại |
| --- | --- |
| Validator, thứ tự lỗi, 304, header cache của hai endpoint | READ-01 — `vink-app` `http/conditional.rs`, contract v1 |
| Lưu byte và tự gửi `If-None-Match` | HTTP cache của trình duyệt (READ-02 gọi fetch với `cache: "no-cache"`). Web app **không** tự gửi `If-None-Match`; raw `304` không bao giờ được parse như JSON mà đổi thành đúng một request `reload` không điều kiện |
| Giữ snapshot UI khi đang làm mới, nhịp 1–3 phút, lifecycle, offline | READ-02 — `apps/vink-web` `freshness.rs` + `public_reads.rs` (Web pilot, phương án A: [ghi chú](public-read-freshness.md)) |
| Nếu READ-02 tự quản validator | Phải tự xử lý `304` (dùng body đã giữ) và không đặt TTL HTTP riêng chồng lên `no-cache` |

Không gọi một response cũ là kết quả GET mới đã được server xác nhận: với `no-cache`, trình
duyệt chỉ dùng byte đã lưu sau khi server trả `304`.

## Bằng chứng

| Kiểm | Lane | Kết quả |
| --- | --- | --- |
| `http::conditional::tests` (grammar, malformed/oversized, so khớp, 304 không body) | `quick` | PASS cục bộ |
| `http::tests::database_outage_is_reported_not_hidden` (`*` khi mất DB → 503 `no-store`) | `quick` | PASS cục bộ |
| `conditional_reads::*` trên PostgreSQL 17.11 thật: 200→304, GET/HEAD, nháp riêng, đổi tên tác giả, con trỏ revision, thêm/bớt/thứ tự/hòa thời điểm, danh sách rỗng, tag cũ sau rút bài → 404, rename đồng thời | `postgres` | PASS cục bộ |
| `conditionalPublicReads` trong browser smoke (Chrome headless + CDP) | `web` | PASS cục bộ |

Ở `conditionalPublicReads`, lần đọc client thứ hai của Explore gửi `If-None-Match` do HTTP cache
thêm vào và nhận **304 trên dây** (`Network.responseReceivedExtraInfo.statusCode`), trong khi
`Network.responseReceived.status` và giá trị `fetch()` trả cho application đều là **200** với đúng
byte cũ. Vì vậy không dùng `response.status === 304` làm bằng chứng cache hoạt động.

Lần chạy `verify web` đầu tiên trượt một lần ở bước lưu nháp của `editorChrome` (route draft, không
đổi trong lát này) khi máy đang chạy `cargo clippy` song song; chạy lại trên máy rảnh thì PASS toàn
bộ. Ghi lại để không che một flake có thể có.

## Số đo cục bộ

**Môi trường:** Apple M4, macOS 26.5.2, PostgreSQL 17.11 Homebrew trên 127.0.0.1, binary release,
loopback HTTP/1.1 keep-alive, một kết nối, request tuần tự. **Trước:** `develop@c6aa486`.
**Sau:** lát này. **Dataset có tên** `vink_bench`: dev seed (1 tác phẩm, 2 khổ) + 11 publication
tổng hợp do SQL tạo (mỗi bài 40 khổ × 4 dòng, có nhãn “Bài đo tổng hợp”); list trả đủ 12 thẻ.
Đếm SQL bằng `pg_stat_statements` (câu có `FROM publications`) sau 200 request làm ấm.

| Endpoint | Body 200 (byte) | Header thêm ở 200 | Tổng byte/request 304 | SQL mỗi request trước → sau (200 và 304) |
| --- | --- | --- | --- | --- |
| `GET /api/v1/works` (12 thẻ) | 2 439 | +76 | 166 (so với 2 649 khi 200) | 1 → 1 |
| `GET …/publication` (seed) | 625 | +76 | 166 (so với 834) | 1 → 1 |
| `GET …/publication` (160 dòng) | 11 279 | +76 | 166 (so với 11 490) | 1 → 1 |

Thời gian thực thi SQL trung bình mỗi lần gọi không đổi trong độ nhiễu (0,004–0,06 ms). Latency
end-to-end trên loopback (3 vòng xen kẽ, 3 000 request mỗi cấu hình) nằm trong khoảng p50
≈ 90–690 µs và **dao động giữa các vòng lớn hơn chênh lệch giữa trước/sau hoặc 200/304**, nên
không kết luận nhanh hơn hay chậm hơn. Điều số liệu chứng minh: khi dữ liệu không đổi, 304 bỏ toàn
bộ body (tổng byte response giảm ≈ 80 % ở bài seed nhỏ, 94 % ở list, 99 % ở bài 160 dòng) và **không** giảm số câu SQL.
Chưa đo riêng CPU serialize/hash, mạng thật, đồng thời cao hay `EXPLAIN (ANALYZE, BUFFERS)`
(truy vấn không đổi nên không bắt buộc).

Script đo không được commit (không thêm runner chỉ để đo một lần); lệnh đã dùng: `ab -n 2000 -c 1
-k` cho byte và đếm SQL, client Python `http.client` keep-alive với `perf_counter` cho p50/p95.

## Rút lại theo endpoint

Không có cờ runtime. Để rút một endpoint về phương án C, đổi handler đó trong
[`http/api.rs`](../../services/vink-app/src/http/api.rs) từ `conditional::respond(&headers, &dto)`
về `Json(dto)` và cập nhật contract. Trình duyệt chỉ gửi `If-None-Match` khi có tag đã lưu; không
có tag mới thì mọi request là `200` thường, không cần đổi client. Quyền đọc, thứ tự lỗi và
`no-store` của draft/discussion không phụ thuộc lát này.

## Chưa làm / rủi ro còn lại

- Không có lệnh publish/withdraw sản phẩm; mọi thay đổi publication/byline trong test là SQL
  fixture có nhãn. Không tuyên bố PUB-03 hay thu hồi toàn hệ thống đã xong.
- `304` vẫn chạy đủ SQL + serialize + hash; chỉ giảm byte truyền. Phương án B chỉ nên mở khi có số
  đo cho thấy SQL/serialize là nút thắt và chứng minh được đủ phụ thuộc.
- Chưa có số đo production, mạng thật, CDN hay nhiều trình duyệt (chỉ Chrome headless).
- Nếu thêm nén HTTP, tag strong hiện tại sẽ sai cho biến thể nén: phải đổi trước khi bật.
