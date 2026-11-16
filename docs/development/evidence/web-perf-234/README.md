# Bằng chứng thô Web Perf Baseline (#234)

Thư mục này lưu snapshot raw artifacts của lượt chạy `cargo xtask verify web --scenario perf-baseline --release` trên PostgreSQL 17.11 và Headless Chrome `157.0.8103.78` tại loopback (`target/verify-web/1794836243468935000-95798/perf-baseline/`).

Báo cáo tổng hợp và quyết định Go/No-Go nằm ở [web-perf-baseline.md](../../web-perf-baseline.md).

## Thành phần tệp

| Tệp | Nội dung |
| --- | --- |
| [environment.json](environment.json) | SHA nền (`ca9369653a516715fea95ca71fbd818d54c15b0a`), `sourceFingerprint`, profile `release`, thông số Chrome/Node/Rust/PG 17.11, CPU/font/viewport và breakdown kích thước bundle (gồm section `custom:name` của `vink_bg.wasm`) |
| [corpus-manifest.json](corpus-manifest.json) | Manifest gọn của corpus tổng hợp `seed = 234` tái sử dụng `#229` (`seed = 229`), `corpus_digest = e3bb862747f1be49e553e425a0a2516793c729dbebdac9b1af6dfaa8c3adbc9c`, 200 tác phẩm, 1.000 bình luận, không chứa nội dung body |
| [ledger.json](ledger.json) | Request-reason ledger tách `logical_reads`, `transport_attempts`, `validator_304_count` và bytes theo danh mục cho cold/warm/lifecycle/offline/503/withdrawal |
| [summary.json](summary.json) | Phân phối `n=10` (`min`, `max`, `range`, `median`, `lab_p95`, `iqr`, `cv_pct`, `field_p75 = null`) cho Desktop `1440x900@1` và Mobile `390x844@2`, chi phí instrumentation (`n=5`), 25-cycle disposal, negative controls và bảng Go/No-Go |
| [runs.json](runs.json) | Toàn bộ raw samples từng vòng chạy cold/warm và 25 chu kỳ `disposal_cycles` (trước và sau `HeapProfiler.collectGarbage`) |
