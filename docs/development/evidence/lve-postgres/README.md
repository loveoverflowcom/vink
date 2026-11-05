# Bằng chứng PostgreSQL của ghi nhận văn học (LVE, #73)

**Ngày đo:** 05/11/2026 · **Nhánh:** `codex/lve-application-postgres` · **Mục đích:** chọn CHOICE-05
(đếm-khi-đọc / counter giao dịch / outbox) bằng số đo, và kiểm truy vấn batch không quét theo từng
thẻ. Đây là số đo trên một máy dev, **không phải SLO** và không hứa hiệu năng production.

## Môi trường

- Apple M4, 10 nhân, 16 GB RAM, macOS (Darwin 25.5).
- PostgreSQL 17.11 (Homebrew) cụm scratch cục bộ, TCP `127.0.0.1:55432`, cấu hình mặc định ngoài
  `max_connections=200`: `shared_buffers=128MB`, `work_mem=4MB`, `synchronous_commit=on`.
- Mọi bảng nằm trong bộ nhớ đệm khi đo (cột `Buffers` chủ yếu là `hit`); chưa đo trên đĩa lạnh hay
  máy chủ dùng chung.

## Tái lập

Chỉ chạy trên database scratch. Áp toàn bộ migration của App theo thứ tự, rồi dataset:

```bash
createdb -h 127.0.0.1 -p 55432 -U postgres vink_lve_evidence
```

```bash
for m in services/vink-app/migrations/*.sql; do psql -X -v ON_ERROR_STOP=1 -h 127.0.0.1 -p 55432 -U postgres -d vink_lve_evidence -f "$m"; done
```

```bash
psql -X -v ON_ERROR_STOP=1 -h 127.0.0.1 -p 55432 -U postgres -d vink_lve_evidence -f docs/development/evidence/lve-postgres/dataset.sql
```

```bash
psql -X -v ON_ERROR_STOP=1 -h 127.0.0.1 -p 55432 -U postgres -d vink_lve_evidence -f docs/development/evidence/lve-postgres/queries.sql
```

Quy mô thứ hai: tạo database từ `vink_lve_evidence` (`CREATE DATABASE … TEMPLATE …`) rồi chạy
[scale-1m.sql](scale-1m.sql). Mỗi quy mô đo hai lần: ngay sau khi nạp (visibility map chưa đặt) và
sau `VACUUM (ANALYZE)`.

| File | Nội dung |
| --- | --- |
| [dataset.sql](dataset.sql) | 20 000 principal; tác phẩm nóng 3 revision (công khai r3) với 15 000 hàng ghi nhận (6 000 ở r3, 5 000 ở r2, 2 000 ở r1, 2 000 đã bỏ); 200 tác phẩm nền × 50 hàng; thread 500 gốc + 1 500 phản hồi (≈80 % duyệt, 10 % chờ, 5 % từ chối, 5 % bị gỡ); ghi nhận bình luận lệch: gốc nóng 5 000 hàng (4 000 chọn), bình luận hạng k ≈ 2 000/k, 1/5 đã bỏ. `setseed(0.73)` |
| [scale-1m.sql](scale-1m.sql) | Thêm 1 000 000 hàng ghi nhận bình luận (500 người × 2 000 bình luận) |
| [queries.sql](queries.sql) | Đúng các câu lệnh `PgStore` phát ra, với ID literal (custom plan) và một trang thảo luận mặc định 80 ID (20 gốc + 60 phản hồi, lẫn trạng thái, có gốc nóng) |
| `plans-25k-*.txt`, `plans-1m-*.txt` | Toàn văn `EXPLAIN (ANALYZE, BUFFERS)`, danh sách 80 ID được rút gọn |
| [hot-target-count-on-read.pgbench](hot-target-count-on-read.pgbench) | Hình dạng transaction ghi bình luận (các khóa + UPDATE + đếm) trên **một** bình luận, người ghi khác nhau |
| [hot-target-counter.pgbench](hot-target-counter.pgbench) | Phương án bị loại CHOICE-05 B để so sánh: thêm một hàng counter giao dịch |

## Số lượng câu lệnh mỗi request

Không phụ thuộc số thẻ/bình luận được hỏi (tối đa 100 ID):

| Request | Câu lệnh SQL |
| --- | --- |
| `GET …/endorsements` | `BEGIN`, `SET TRANSACTION REPEATABLE READ, READ ONLY`, tác phẩm + số đếm, các bình luận + số đếm (bỏ khi không hỏi bình luận), `COMMIT` |
| `GET …/endorsements/me` | Như trên + kiểm principal; bình luận dùng một `LEFT JOIN` theo khóa chính |
| `PUT …/endorsement/me` (tác phẩm) | `BEGIN`, principal, publication `FOR SHARE`, works, hàng của actor `FOR UPDATE`, `UPDATE`/`INSERT`, đếm, `COMMIT`; thêm một lần đọc-lại khi thua `INSERT` đầu tiên |
| `PUT …/comments/{id}/endorsement/me` | Như trên + thread `FOR SHARE` + bình luận `FOR SHARE` |

## Kết quả `EXPLAIN (ANALYZE, BUFFERS)`

Thời gian là `Execution Time` của một lần chạy (không phải phân vị); buffer là `shared hit/read`.

| # | Truy vấn | 25k: ngay sau nạp | 25k: sau VACUUM | 1M: ngay sau nạp | 1M: sau VACUUM |
| --- | --- | --- | --- | --- | --- |
| 1 | Tổng số tác phẩm (6 000 hàng hiệu lực) | 0,76 ms, index-only `work_endorsements_anchor_idx`, 0 heap fetch | 0,71 ms | 0,40 ms | 0,88 ms |
| 2 | Tổng số 80 bình luận | 1,04 ms, index-only `comment_endorsements_selected_idx` ×80 | 1,02 ms, 173 buffer | **11,3 ms, Bitmap Heap Scan, 32 436 buffer** | 4,10 ms, index-only, 253 buffer |
| 3 | Trạng thái riêng: tác phẩm | 0,06 ms, `work_endorsements_pkey` | 0,05 ms | 0,02 ms | 0,05 ms |
| 4 | Trạng thái riêng: 80 bình luận | 1,85 ms, **Seq Scan** `comment_endorsements` (191 trang, lọc theo actor) + hash join | 1,62 ms | 0,16 ms, 80 lần dò `comment_endorsements_pkey` | 0,51 ms |
| 5 | Đếm lại sau ghi: tác phẩm nóng | 0,39 ms, index-only | 0,39 ms | 0,21 ms | 0,46 ms |
| 6 | Đếm lại sau ghi: gốc nóng (4 000 / 4 400 hàng) | 0,25 ms, index-only | 0,25 ms | 0,33 ms, Bitmap Heap Scan | 0,31 ms, index-only |
| 7 | Hàng của actor `FOR UPDATE` | 0,03 ms, `comment_endorsements_pkey` | 0,03 ms | 0,02 ms | 0,03 ms |

Nhận xét đã kiểm:

- **Không N+1:** mỗi batch là một câu lệnh; bình luận dùng `comments_pkey` với `= ANY`, số đếm là
  subplan index-only trên partial index. Seq scan ở #4 tại 25k là lựa chọn theo chi phí của planner
  (191 trang rẻ hơn 80 lần dò); ở 1M planner tự chuyển sang dò khóa chính. Đã thử `LEFT JOIN
  LATERAL`: planner gộp thành cùng plan nên không giữ.
- **Visibility map quyết định chi phí đếm:** khi trang chưa all-visible (vừa ghi/cập nhật,
  autovacuum chưa chạy), đếm phải đọc heap: #2 ở 1M tăng từ 4,1 ms lên 11,3 ms và từ 253 lên 32 436
  buffer. Bảng ghi nhận bị UPDATE mỗi lần đổi ý, nên production sẽ nằm giữa hai cột này.
- `work_endorsements` dùng `(work_id, revision_id)`: ghi nhận của bản trước (r1, r2) không bị đọc
  khi đếm r3.

## Hot-target (CHOICE-05 A so với B)

`pgbench -M prepared -T 15`, 5 000 người ghi khác nhau lật hàng của chính họ trên **một** gốc nóng.
Đây là hình dạng SQL (khóa, ghi, đếm), không phải code Rust.

| Kịch bản | 1 client | 16 client |
| --- | --- | --- |
| A, đếm-khi-đọc (chạy trước) | 1 608 tps, 0,62 ms | 8 435 tps, 1,90 ms |
| B, + một hàng counter giao dịch | 6 281 tps, 0,16 ms | 6 864 tps, 2,33 ms |
| A, chạy lại sau ≈ 412 000 UPDATE (112 305 dead tuple) | 1 469 tps, 0,68 ms | 7 205 tps, 2,22 ms |

Một client thì counter nhanh hơn (A đếm 4 000 hàng mỗi lần ghi). Với 16 người ghi đồng thời, B gần
như không tăng thông lượng vì mọi transaction tuần tự hóa trên một hàng counter; A tăng ≈ 5 lần vì
mỗi người chỉ khóa độc quyền hàng của mình và chỉ khóa chia sẻ publication/thread/bình luận. Không
transaction nào lỗi ở cả hai.

## Quyết định rút ra và ngưỡng xem lại

- Chọn **A (đếm từ hàng chuẩn)**: số đếm luôn khớp nguồn, không cần đối soát, không thêm điểm tranh
  chấp; chi phí đọc đã đo ở trên. B bị loại vì tuần tự hóa người ghi trên mục tiêu nóng và cần
  đối soát; C (outbox) bị loại vì chưa có nhu cầu bất đồng bộ.
- Xem lại CHOICE-05 khi số đo thật (không phải dataset này) cho thấy một trang batch thường xuyên
  vượt ngân sách đọc của reader, hoặc khi cần đếm cho danh sách nhiều tác phẩm (Khám phá) — lát đó
  phải đo lại. Không có SLO nào được đặt ở đây.
