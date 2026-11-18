# Baseline release cho hành trình Khám phá → Reader → Back

Đo ngày 18/11/2026 tại `acfe00369ccf016a95d1de6cbc3faf6a3f1f7c9c`, từ develop `9fcd75ec03229a27464123eac1d68104e8447b19`. Mỗi cột gồm 10 document cold, 10 lượt warm và 10 lượt Reader cold-key tách riêng. Cold xóa HTTP cache trình duyệt, PostgreSQL và cache hệ điều hành vẫn ấm. Khách trên Chrome 157.0.8103.78/Linux x86_64, 1280×900 DPR 1, CPU 1×, loopback, PostgreSQL 17.11, Rust 1.99.1, Node 24.22.3, wasm-bindgen 0.2.129. Release WASM có probe 123.727.343 byte. Ledger chỉ lưu loại request, đường dẫn mẫu, tên tham số, mã key, thời gian, byte và trạng thái.

| Median (min–max, p95 lab), ms | Một đoạn | #229, 200.000 đoạn |
| --- | ---: | ---: |
| TTFB Khám phá | 12.25 (10.90–17.60, 17.60) | 12.30 (9.20–17.20, 17.20) |
| Hàng đầu tiên | 33.90 (29.80–43.40, 43.40) | 31.05 (29.10–44.00, 44.00) |
| Hydration | 568.85 (523.20–748.30, 748.30) | 567.10 (506.90–759.30, 759.30) |
| Callback observer mỗi document | 0.85 (0.60–1.00, 1.00) | 0.60 (0.40–1.30, 1.30) |
| Click → article, warm | 19.45 (13.10–84.60, 84.60) | 2896.45 (1421.50–4702.40, 4702.40) |
| GET Reader, cold-key có CDP interception | 21.70 (16.70–26.30, 26.30) | 67.40 (55.60–81.10, 81.10) |
| Click → article, cold-key | 43.05 (38.30–58.50, 58.50) | 1240.55 (1096.80–1451.00, 1451.00) |
| Layout + style khi mount discussion | 16.75 (14.27–20.29, 20.29) | 44.82 (32.68–103.90, 103.90) |

Mở [guest.json.gz](guest.json.gz) và [blocks.json.gz](blocks.json.gz) để xem từng vòng, font, ledger, interaction, long task, layout shift, DOM/V8, số row và toàn bộ phân bố. p95 là nearest-rank trên 10 mẫu. CDP poll và interception làm tăng thời gian. CPU observer chỉ tính thân callback. Chưa đo riêng RSS, WASM allocator hay máy yếu trong release này. Có phép đo khác build cho RSS/linear memory trong [#237](../web-perf-237/canonical-20261118/README.md).

| AC | Kết quả thực đo |
| --- | --- |
| 01 | PASS. 200 work mẫu, 1.000 comments trên một work, trang đầu 20 gốc + 20 reply. Hai SQL seed có digest trong raw. Bài dài dùng đúng source #229, bản sao DB chỉ seed hai tag dùng trong phép lọc. |
| 02 | PASS. Fresh hydrate, history document back_forward, stale burst, manual, offline và response thu hồi đều có ledger. Burst bốn tín hiệu khởi động một flight. Wire 304 và response 200 giao cho ứng dụng là hai trường riêng. |
| 03 | PASS trong lab. 10+10 cùng cấu hình cho mỗi corpus. READ-02 đếm trực tiếp observer, subscription, timer, resource, waiter và flight. Các con số DOM/V8 còn chịu ảnh hưởng GC. |
| 04 | PASS. Một GET dư có chủ đích làm assertion duplicate đỏ. Giữ response Published thật, cho Hidden tổng hợp đến trước, thả response cũ và đợi coordinator xử lý. View dispose về 0 observer/resource/timer/flight/waiter. Giữ hoặc trả 503 cho discussion vẫn thấy article, retry lấy lại trang từ App. Ledger tổng hợp kiểm UI/fence. Quyền đọc ở App/PostgreSQL có bằng chứng riêng ở #235. |
| 05 | PASS. Giữ Async (#235), sửa thời điểm focus URL anchor (#236), giữ trần cache đã đo (#237). Layout/style vùng phụ vượt ngưỡng thử [#238](https://github.com/loveoverflowcom/vink/issues/238). A/B cho thấy lợi ích chủ yếu trên bài 200.000 đoạn, B dịch mép danh sách 19 px. Giữ layout thường. [#239](https://github.com/loveoverflowcom/vink/pull/311) giữ GET lúc click vì GET bài ngắn chỉ 8,65 ms và bài dài vượt ngân sách 32 KiB. Chọn scenario mới bật probe; nghiệm thu không phụ thuộc một số p95 hay SLA. |

Tại gốc repo, đặt `VINK_TEST_DATABASE_URL` cho scratch PostgreSQL, `VINK_CHROME` cho Chrome, rồi chạy

```sh
cargo xtask verify web --scenario perf-baseline --release
node --test apps/vink-web/e2e/web-perf-baseline.test.mjs
```

Lượt thứ hai dùng App guest trỏ vào bản sao scratch có revision dài. Dựng lại corpus bằng [hướng dẫn bên dưới](#corpus-dài), phục vụ đúng release có probe, rồi chạy cùng harness

```sh
node apps/vink-web/e2e/browser-smoke.mjs --base "$SCRATCH_ORIGIN" --chrome "$VINK_CHROME" --state "$ARTIFACTS/state.json" --phase perf-baseline --build-profile release
```

## Corpus dài

Dùng database scratch riêng sau migrations, hai tag Thơ/Ký ức, fixture `discovery-publications.sql` rồi `web-perf-journey.sql`. Từ gốc repo, truyền SQL do script sinh vào psql trên **bản sao scratch**. Script yêu cầu đúng DB lab và đúng revision xuất phát, transaction cập nhật một publication. Mở cùng q/tag, dùng cùng App guest và cùng bundle trên cả hai corpus.

```sh
python3 docs/development/evidence/web-perf-234-canonical/blocks.py | psql -X -v ON_ERROR_STOP=1 "$BLOCKS_SCRATCH_URL"
```

[Decision registry](decisions.json.gz) ghi các lựa chọn READ-01/02 và DISC-WEB, O-03/O-17 r2 và đề xuất VDR-PUBLIC-OFFLINE-AGE r1 PENDING. Đo trong Web lab với gate lưu bền tắt.
