# Raw evidence #234 — baseline Khám phá → Reader → Back

Owner và cách chạy: [baseline Web Perf #234](../../web-perf-baseline.md). Ngày 16/11/2026,
source `997d97d5520355818d5e8a7d77e312994b4d2eea` (cây sạch), lệnh
`cargo xtask verify web --scenario perf-baseline`, Chrome 157.0.8103.78 headless (wrapper
`--no-sandbox` của sandbox công cụ), Node v24.22.3, PostgreSQL 17.11 container tạm qua
`VINK_TEST_DATABASE_URL`. Corpus tổng hợp, không có manuscript cá nhân; digest hai fixture SQL
`d6824beb0d63acd08df2091cbaa02860b957535032200248a3d082fd9c391d87`.

| File | Nội dung |
| --- | --- |
| [web-perf-baseline.json](web-perf-baseline.json) | Raw 10 cold + 10 warm, transport ledger CDP, intents, distributions, limitations, go/no-go `PENDING` |
| [run.log](run.log) | Log xtask/browser-smoke của lượt trên (bỏ dòng Compiling, đường dẫn máy được thay) |
| [negative-control.log](negative-control.log) | Đối chứng âm: hạ tạm `MAX_MOUNTED_COMMENTS` xuống 39 → FAIL đúng assertion (40 nodes); không commit |

## Số đo lab (debug profile, một máy, không phải release/RUM)

| Chỉ số | n | median | min–max | p95 lab |
| --- | --- | --- | --- | --- |
| Cold TTFB (ms) | 10 | 34 | 28.8–48.8 | 48.8 |
| Cold nội dung có nghĩa đầu tiên (ms) | 10 | 60.8 | 51.6–147.3 | 147.3 |
| Cold hydration (ms) | 10 | 1524.5 | 1432.7–1687.7 | 1687.7 |
| Warm Reader body sau click (ms) | 10 | 101.7 | 83–275.8 | 275.8 |
| Warm trang thảo luận đầu (ms) | 10 | 291.5 | 233.9–637.5 | 637.5 |
| Trở lại Khám phá (ms) | 10 | 257.5 | 103.8–548.5 | 548.5 |
| Cold DOM nodes | 10 | 1022 | 896–1044 | 1044 |
| Cold V8 heap used (bytes) | 10 | 3 812 108 | 3 809 432–3 992 180 | 3 992 180 |

Bình luận đang mount: 40 nodes ở mọi lượt warm (20 root + reply preview) trên thread 1.000 bình
luận; trang Khám phá giữ 12 hàng. Cold layout shift tổng 0 ở cả 10 lượt; 3–6 long task mỗi lượt.

**Không dùng bundle/hydration làm baseline release:** `vink_bg.wasm` debug ≈ 350 MB, nên transfer
bundle (~350 MB) và hydration ~1,5 s chủ yếu đo build debug. Release profile vẫn `NOT_RUN`.

## Quan sát ledger (giả thuyết cho slice sau, không phải kết luận lỗi)

- Cold: mỗi lượt 1 document + 2 `session`, **0 GET danh sách** sau hydration (structural PASS).
- Warm: GET `/reader` chỉ ở lượt 0; 9 lượt sau được READ-02 memory phục vụ, không có transport.
- Mỗi lần mở Reader: 5 GET `endorsement`, 2 `public-discussion`, ~3 `own-discussion`, 1
  `actor-discussion`, 1 `catalog` (`/api/v1/tags`). Việc lặp endorsement/discussion là ứng viên
  duplicate cần phân biệt logical read với transport ở AC-02/#235 trước khi sửa.
- 0 request gửi `If-None-Match`, 0 wire 304: cold xóa HTTP cache, warm dùng memory; đường
  validator/304 chưa được scenario này thực thi.
- Browser chạy với `VINK_DEV_PRINCIPAL` (fixture loopback, chủ các work fixture), nên ledger là
  góc nhìn tác giả xem tác phẩm của mình, không phải độc giả ẩn danh.
