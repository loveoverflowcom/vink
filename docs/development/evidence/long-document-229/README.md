# Raw evidence #229

Owner và cách chạy: [ngân sách document](../../long-document-budget.md).
Ngày 16/11/2026, source `5833624d8f77259a22f65abedfb47b691dffd791`, integration base
`a3e65360e316d27e5d029a43aef8b3083f622fb7`. Đây là corpus tổng hợp, không có manuscript cá nhân.
Smoke tương thích sau merge dùng source `3d05fa601e8bf02b5a00e33612cdcd703cd9f600` trên base
`69f71f0cb61228dbe5ceea775b3c34c237705922`; số timing của smoke không thay baseline.

| File | Nội dung |
| --- | --- |
| [environment.json](environment.json) | Host/toolchain/build flags, decision snapshot và boundary |
| [postgres.json](postgres.json) | 21 case; raw CPU/app read/save/WAL samples, EXPLAIN plans, history và relation sizes |
| [compression.json](compression.json) | Fresh default/pglz/LZ4, từng sample, WAL/heap/TOAST/index và old/new compression |
| [browser.json](browser.json) | 42 Reader + 84 editor scenes, network/hydration payload, DOM/heap, snapshot/save/conflict và oversized cache |
| [postmerge-browser.json](postmerge-browser.json) | PASS tương thích UI #227: 3 case, 9 Reader + 12 editor scenes; không dùng để so sánh performance |
| [repeat.json](repeat.json) | Đối chiếu 21 case hai lượt; exact byte/hash equality và read medians; raw SHA lượt đầu được giữ để kiểm lại |
| [manifest.json](manifest.json) | SHA-256 của các evidence file được commit |

Ảnh của `poem_tabs-200000` không phải mốc thiết kế được phê duyệt. Đã mở kiểm
[390/chữ200](budget-reader-390-200.png) và [1440/chữ100](budget-reader-1440-100.png).
[390/chữ100](budget-reader-390-100.png) và [1440/chữ200](budget-reader-1440-200.png) đã capture,
chưa mở để kết luận visual. [contexts.json](contexts.json) giữ viewport/font/theme/source metadata.

`fixtures.json` 26 MB và logs đầy đủ nằm ngoài repo tại `/tmp/vink-229-evidence`; corpus có thể
tái tạo từ seed 229. Final test PASS; raw của lượt lỗi harness đầu vẫn được giữ ở `run/` tại đó.
Không dùng V8 heap warm/không ép GC làm tổng browser memory hay SLA mobile.
