# Search R3 #90 — bằng chứng local

Ngày 08/11/2026; base `develop@f7bd0c4`, branch `codex/search-r3-continuous`.
[DecisionMap và LayoutSpec](../design/search-r3.md) ghi lựa chọn A/A của agent;
không gọi là người dùng đã duyệt hoặc usability-tested. Commit chứa file này là
implementation revision; manifest local lưu full SHA/content fingerprint và nguồn capture.

## Môi trường và cách chạy

macOS arm64, Rust 1.99.1, wasm-bindgen 0.2.129, Node 22.15.0, Chrome headless
156.0.8078.80, PostgreSQL 17.11 tại loopback port55490. Database mới riêng
`vink90_baseline`, `vink90_after`; full lane dùng scratch `vink_verify_web` theo owner.
Định danh LocalFixture và dữ liệu `[Mẫu]`/HTTP fixtures của runner; không đăng nhập production.
Font Be Vietnam Pro 400/600 và Literata tự host; không dùng font prototype thay production.

Ổ đĩa ban đầu gần đầy; hai build baseline FAIL ENOSPC. Sau đó npm cache dùng chung
FAIL EACCES; chuyển sang `npm_config_cache=/tmp/vink90-npm`. Baseline full lane tiếp theo
build/SSR chạy được nhưng FAIL ENOSPC khi ghi ảnh. Giữ nguyên các log lỗi trong bundle.
Thu hồi cache Cargo cũ đã ngừng chạy, giữ source và evidence. Local `.cargo/config.toml`
tạm dùng `profile.dev.debug=0`, `incremental=false` để vừa ổ; không đổi optimization,
debug assertions hay product configuration và không commit override này.

Lệnh chạy tại repo root, thêm `VINK_TEST_DATABASE_URL` trỏ PostgreSQL tạm trên và
`VINK_CHROME` trỏ Chrome cài sẵn khi cần; không dùng credential production:

| Lệnh | Kết quả | Boundary |
| --- | --- | --- |
| `cargo test --locked -p vink-ui search_bar` | PASS, exit0 | SSR primitive: một input, một submit, real label/help/name, raw NFD/whitespace/escaping, native clear href, private field không name/href |
| `cargo xtask verify quick` | PASS, exit0 | Token drift, fmt, clippy SSR/hydrate/preview, unit/contract/doctests và repo boundaries |
| `cargo xtask verify web` | PASS, exit0 | App thật + PostgreSQL scratch, SSR privacy/no-JS, hydration, reader return, dashboard/editor/discussion/endorsement và restart |
| `cargo xtask ui-preview leptos verify` | PASS, exit0, 774 captures | Shared renderer/gallery và SearchField mặc định; không đại diện CMP/native |
| `node apps/vink-web/e2e/browser-smoke.mjs --base http://127.0.0.1:3049 --chrome <Chrome> --state /tmp/vink90-before/smoke-state.json --phase desk-layout` | PASS, exit0 | Baseline binary build từ f7bd0c4 trước patch; app thật, cùng fixtures |
| Cùng runner với port3048, output `/tmp/vink90-after-final`, phase `desk-layout` / `public-search` | PASS, exit0 | Search R3: scoped browser regression và ma trận after |

Full web PASS trước delta cuối cho callback/fallback optional của Bar; hai consumer thật
đều dùng native form (không truyền `on_submit`), nên đường tương tác đó không đổi. Sau delta
đã chạy quick, rebuild và focused `desk-layout`/`public-search` trên source cuối; manifest
lưu fingerprint tương ứng. Gallery requalify SearchField mặc định, không suy ra mọi optional
callback của Bar đã có browser coverage.

Full/focused lanes từng FAIL trước khi sửa: test chờ count=1 khi refresh nay vẫn giữ row cũ;
đổi sang chờ response complete + title mới, giữ assertion title của row. Shared focus clear
bị rule scaffold ghi đè 3px; sửa specificity và kiểm đúng một perimeter2px. Không đổi các
FAIL đó thành NOT_RUN. Browser runtime không có console errors ở các lượt PASS.

## Coverage và acceptance

Bundle local: `target/ui-evidence/search-r3-90/`, gồm `manifest.json`, reports, logs,
ảnh before/after và context gốc. Không commit binary/PNG/log vào source repository.
Matrix chính: 2 màn × 5 widths320/390/834/1024/1440 × 2 theme × text100/200 =40 ảnh
mỗi phía. Cùng fixtures12 public cards và100 drafts, viewport height900/DPR1; loaded/default
matrix không focus, các state focus/filled/clear có capture/interaction riêng.
Đã mở đối chiếu10 cặp representative (20 ảnh) bao phủ cả5 widths, hai theme và hai text scale;
60 ảnh matrix khác CAPTURED + geometry assertions, chưa mở từng ảnh. Đã mở thêm ảnh focus
tonal/forced-colors và shared SearchField preview; không gọi toàn774 ảnh gallery là đã review.

| AC | Kết luận và giới hạn |
| --- | --- |
| AC-01 | DecisionMap A/A, tradeoff và hướng loại trong LayoutSpec scoped; không thay baseline R1/R2. |
| AC-02 | Một surface tonal, nhãn thật, submit/clear>=48px riêng; input/shell/action đúng một focus perimeter, forced-colors + reduced-motion. |
| AC-03 | Native no-JS Enter/click GET đúng một q, không unnamed pair; clear href=/; hydrated clear/touch và reader Back/Forward/reload/query/anchor/focus qua server thật. |
| AC-04 | Raw NFD/Đ/đ/whitespace/caret/selection và input identity qua theme/resize/text; synthetic composition Enter trên cả hai màn. OS IME NOT_RUN. Giữ criterion unchecked. |
| AC-05 | Tìm/sort chỉ loaded drafts100/title, resume/create/reconcile đúng work/version; Enter/click không đưa query riêng vào URL/history. |
| AC-06 | Matrix/keyboard/touch/focus/forced-colors/reduced-motion qua Chrome. Root text200 không phải browser zoom. Actual browser zoom NOT_RUN: CUA xác nhận Mac đang khóa; không tự mở khóa. Mobile thật/screen reader NOT_RUN. Giữ criterion unchecked. |
| AC-07 | Loading/empty/no-results/unavailable riêng; pending/503 giữ last-success/query/input, retry khôi phục; 401/403 xóa snapshot riêng và không lộ resume/create. |
| AC-08 | Primitive đúng owner, hai consumer, app giữ router/fetch, token không đổi và drift check; gallery Leptos regression. CMP/native NOT_RUN, Bar là Web-only. |
| AC-09 | Lệnh/log/exit codes, code fingerprint, before/after và gap trong bundle/handoff. Merge hoặc PNG không được dùng để đóng issue. |

Trong fixture12 public cards, first-row top (CSS px, light) trước → sau là:
320/text100:488→398; 320/text200:1482→1305; 390/text100:399→378;
390/text200:1180→1006; 1440/text100:309→289. Search chuyển lên trước H1 nhưng anatomy
và copy gọn hơn nên hàng đầu không bị đẩy xa thêm trong các ca đo này.

Trong ảnh320/text200, scope và H1 vẫn cần cuộn; không tuyên bố toàn nội dung đầu trang
cùng hiện trong một viewport. Các mục chính/clear/submit vẫn có đường truy cập và khoảng
tránh bottom bar hiện hành. Không test discoverability với người dùng thật. Không deploy,
không đọc/chờ/rerun GitHub CI, không tự đóng #90 hoặc tick các issue R1/R2 khác.
