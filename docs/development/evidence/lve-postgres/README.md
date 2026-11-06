# Bằng chứng PostgreSQL của ghi nhận văn học (LVE, #73)

**Ngày đo:** 05/11/2026, đo lại sau khi port sang seam #80 (mỗi revision một hàng, `accepted_order`, receipt) · **Nhánh:** `codex/lve-application-postgres` · **Mục đích:** chọn CHOICE-05
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
| [dataset.sql](dataset.sql) | 20 000 principal; tác phẩm nóng 3 revision (công khai r3) với 15 000 hàng ghi nhận (6 000 chọn ở r3, 5 000 ở r2, 2 000 ở r1, 2 000 đã bỏ ở r3); receipt không được nạp vì không đường đọc nào chạm tới; 200 tác phẩm nền × 50 hàng; thread 500 gốc + 1 500 phản hồi (≈80 % duyệt, 10 % chờ, 5 % từ chối, 5 % bị gỡ); ghi nhận bình luận lệch: gốc nóng 5 000 hàng (4 000 chọn), bình luận hạng k ≈ 2 000/k, 1/5 đã bỏ. `setseed(0.73)` |
| [scale-1m.sql](scale-1m.sql) | Thêm 1 000 000 hàng ghi nhận bình luận (500 người × 2 000 bình luận). Bản đo ban đầu ghi hàng đã bỏ với `version = 2` — trạng thái lõi không đạt được, schema khi đó vẫn nhận; từ migration `20261105120000` script ghi `version = 3` (OFF ở version lẻ). Phân bố `selected` và plan không đổi |
| [queries.sql](queries.sql) | Đúng các câu lệnh `PgStore` phát ra, với ID literal (custom plan) và một trang thảo luận mặc định 80 ID (20 gốc + 60 phản hồi, lẫn trạng thái, có gốc nóng) |
| `plans-25k-*.txt`, `plans-1m-*.txt` | Toàn văn `EXPLAIN (ANALYZE, BUFFERS)`, danh sách 80 ID được rút gọn |
| [hot-target-count-on-read.pgbench](hot-target-count-on-read.pgbench) | Hình dạng transaction ghi bình luận (các khóa, khóa lệnh, UPDATE, receipt, đếm) trên **một** bình luận, người ghi khác nhau |
| [hot-target-counter.pgbench](hot-target-counter.pgbench) | Phương án bị loại CHOICE-05 B để so sánh: thêm một hàng counter giao dịch |

## Số lượng câu lệnh mỗi request

Không phụ thuộc số thẻ/bình luận được hỏi (tối đa 100 ID):

| Request | Câu lệnh SQL |
| --- | --- |
| `GET …/endorsements` | `BEGIN`, `SET TRANSACTION REPEATABLE READ, READ ONLY`, tác phẩm + số đếm, các bình luận + số đếm (bỏ khi không hỏi bình luận), `COMMIT` |
| `GET …/endorsements/me` | Như trên + kiểm principal; bình luận dùng một `LEFT JOIN` theo khóa chính |
| `PUT …/endorsement/me` (tác phẩm) | `BEGIN`, principal, publication `FOR SHARE`, works, advisory lock (actor, key), receipt, hàng của actor `FOR UPDATE`, `UPDATE`/`INSERT` + `INSERT` receipt khi chấp nhận, đếm, `COMMIT`; thêm một lần đọc-lại khi thua `INSERT` đầu tiên |
| `PUT …/comments/{id}/endorsement/me` | Như trên + thread `FOR SHARE` + bình luận `FOR SHARE` |

## Kết quả `EXPLAIN (ANALYZE, BUFFERS)`

Thời gian là `Execution Time` của **một** lần chạy (không phải phân vị, dao động vài trăm µs giữa các
lần); buffer là `shared hit/read`.

| # | Truy vấn | 25k: ngay sau nạp | 25k: sau VACUUM | 1M: ngay sau nạp | 1M: sau VACUUM |
| --- | --- | --- | --- | --- | --- |
| 1 | Tổng số tác phẩm (6 000 hàng chọn ở r3) | 1,14 ms, index-only `work_endorsements_selected_idx` | 0,75 ms | 1,37 ms | 0,74 ms |
| 2 | Tổng số 80 bình luận | 1,20 ms, index-only `comment_endorsements_selected_idx` ×80 | 1,06 ms | 1,62 ms | 3,54 ms (≈ 500 hàng/bình luận) |
| 3 | Trạng thái riêng: tác phẩm | 0,06 ms, `work_endorsements_pkey` | 0,05 ms | 0,03 ms | 0,04 ms |
| 4 | Trạng thái riêng: 80 bình luận | 0,78 ms, **Seq Scan** `comment_endorsements` + hash join | 1,50 ms | 1,69 ms, 80 lần dò `comment_endorsements_pkey` | 0,21 ms |
| 5 | Đếm lại sau ghi: tác phẩm nóng | 0,34 ms, Bitmap Heap Scan (visibility map chưa đặt) | 0,40 ms, index-only | 0,32 ms, Bitmap Heap Scan | 0,19 ms, index-only |
| 6 | Đếm lại sau ghi: gốc nóng (4 000 / 4 400 hàng) | 0,27 ms, index-only | 0,24 ms | 0,13 ms | 0,13 ms |
| 7 | Hàng của actor `FOR UPDATE` | 0,02 ms, `comment_endorsements_pkey` | 0,03 ms | 0,02 ms | 0,02 ms |

Nhận xét đã kiểm:

- **Không N+1:** mỗi batch là một câu lệnh; bình luận dùng `comments_pkey` với `= ANY`, số đếm là
  subplan trên partial index. Seq scan ở #4 tại 25k là lựa chọn theo chi phí của planner; ở 1M
  planner tự chuyển sang dò khóa chính. Đã thử `LEFT JOIN LATERAL` ở lần đo trước: planner gộp thành
  cùng plan nên không giữ.
- **Visibility map quyết định chi phí đếm:** khi trang chưa all-visible, đếm phải đọc heap (#5 trước
  VACUUM). Ở lần đo trước port (cùng hình dạng truy vấn) #2 ở 1M lên 11,3 ms / 32 436 buffer khi
  autovacuum chưa chạy; lần này autovacuum đã chạy trước khi đo cột "ngay sau nạp" nên không tái
  hiện. Bảng bị UPDATE ở mỗi lệnh được chấp nhận nên production sẽ dao động giữa hai trạng thái.
- Ghi nhận của bản trước (r1, r2) nằm ở khóa khác nên không bị đọc khi đếm r3.

## Hot-target (CHOICE-05 A so với B)

`pgbench -M prepared -T 15`, 5 000 người ghi khác nhau lật hàng của chính họ trên **một** gốc nóng,
mỗi transaction lấy khóa lệnh riêng và thêm một receipt. Đây là hình dạng SQL, không phải code Rust.

| Kịch bản | 1 client | 16 client |
| --- | --- | --- |
| A, đếm-khi-đọc (chạy trước) | 1 536 tps, 0,65 ms | 6 236 tps, 2,57 ms |
| B, + một hàng counter giao dịch | 4 761 tps, 0,21 ms | 5 592 tps, 2,86 ms |
| A, chạy lại sau ≈ 352 000 UPDATE (206 644 dead tuple) | 684 tps, 1,46 ms | 6 449 tps, 2,48 ms |

Một client thì counter nhanh hơn rõ rệt, và A chậm thêm khi dead tuple tích lũy (đếm 4 000 hàng phải
đọc heap). Với 16 người ghi đồng thời, B gần như không tăng thông lượng vì mọi transaction tuần tự
hóa trên một hàng counter; A vẫn cao hơn vì mỗi người chỉ khóa độc quyền hàng và khóa lệnh của mình.
Không transaction nào lỗi.

## Quyết định rút ra và ngưỡng xem lại

- Chọn **A (đếm từ hàng chuẩn)**: số đếm luôn khớp nguồn, không cần đối soát, không thêm điểm tranh
  chấp khi nhiều người ghi cùng đích. Chi phí: độ trễ ghi đơn lẻ trên đích rất nóng và phụ thuộc
  VACUUM. B bị loại vì tuần tự hóa người ghi trên đích nóng và cần đối soát; C (outbox) vì chưa có
  nhu cầu bất đồng bộ.
- Xem lại CHOICE-05 khi số đo thật (không phải dataset này) cho thấy độ trễ ghi/đọc trên đích nóng
  vượt ngân sách, khi autovacuum không theo kịp bảng ghi nhận, hoặc khi cần đếm cho danh sách nhiều
  tác phẩm (Khám phá). Không có SLO nào được đặt ở đây.
