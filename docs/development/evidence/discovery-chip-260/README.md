# Hình học checkbox-chip Khám phá — #260

Base thực thi: `abf5c9768c8fdaf56ddbd189c16d0452aa7cd856` (`origin/develop`, đã fetch).
Checkout riêng trên macOS arm64; không có PR mở trùng ở thời điểm bắt đầu.
Ảnh snapshot trong issue chỉ là regression baseline lịch sử; **NO_APPROVED_REFERENCE**
cho phương án sửa. Source và browser evidence của lượt này được ghi riêng bên dưới.

[LayoutSpec R1 và CHOICE-01/02](../../../design/discovery-chip-260.md) là owner bố cục.

## Kiểm chứng local

Raw artifacts (đủ PNG/metadata) tại `/tmp/vink260/` và `target/verify-web/`; selected artifacts đã lưu
trong thư mục này, metadata chỉ chứa ảnh đã chọn mỗi run. Kết quả và ảnh đã mở được phân biệt bên dưới.

## Kết quả cuối

CWD `/Users/manhblue/Documents/Codex/2026-11-17/task-4/vink`; Mac arm64, Rust 1.99.1,
Node 22.15.0, Chrome 157, PostgreSQL 17.11, wasm-bindgen 0.2.129. Source thực thi là
base SHA ở trên + delta của PR; [results.json](results.json) pin SHA-256 của từng input
và artifact runtime. Native App baseline được build mới từ checkout này. Hydration/CSS
baseline được clone từ build #259 và xác minh CSS ghép **byte-identical** với source
baseline trước sửa; final `build web` tạo lại toàn bộ bundle/App từ source hiện hành.

Heavy builds/Quick dùng `CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0
CARGO_PROFILE_TEST_DEBUG=0` qua resource-budget; npm cache riêng `/tmp/vink260/npm`.
Browser lane **UNSUPPORTED qua local-budget** theo owner guide, nên chạy tuần tự với
`CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2`. PostgreSQL mới của task ở loopback 55560,
restricted runtime role và dữ liệu mẫu, không dùng DB hay worker của task khác.
Không clean Cargo, không sửa/xóa artifact hoặc source của checkout khác.

| Lệnh / boundary | Kết quả thật | Evidence |
| --- | --- | --- |
| `./tools/local-budget run -- cargo build --locked -p vink-app --bin vink-app` | PASS baseline SSR; 66,07s | Log raw `/tmp/vink260/`; source base như trên |
| `./tools/local-budget run -- cargo xtask verify quick` | PASS cả lane; 214,17s; sampled peak RSS sum 2.026.688 KiB | [quick.log](quick.log) |
| Quick lần cuối trên code/docs | PASS cả lane; 196,84s; sampled peak RSS sum 2.346.208 KiB | [quick-final.log](quick-final.log) |
| `./tools/local-budget run -- cargo xtask build web` | PASS bundle + App; 154,94s; sampled peak RSS sum 2.053.216 KiB | [build.log](build.log) |
| Browser runner `--phase discovery-chip-geometry` trên CSS baseline | FAIL đúng tại tâm nhãn: 9,99–10 CSS px ở 1440×900/light100 | [before.log](before.log), [measurement](before-measurements.json) |
| Diagnostic baseline collector, cùng module geometry và CSS đóng băng | 80 records / 66 captures; exit 1 với 82 assertion FAIL dự kiến (80 geometry + 2 AX names), không gọi PASS | [before-matrix.log](before-matrix.log), [measurements](before-matrix-measurements.json), [process exits](before-matrix-process-result.json), [collector diff script](make-baseline-probe.py.txt) |
| Browser runner `--phase discovery-chip-geometry` sau sửa | PASS 165 assertions / 80 records / 66 captures; label-box center error max **0 px**, rendered text max **0,0078125 px**; target nhỏ nhất **132×48 px**, rect/row ổn định <=1 px | [after.log](after.log), [measurements](after-measurements.json), [process exits](after-process-result.json) |
| Browser runner `--phase public-search` | PASS 211 assertions: GET/no-JS, q/tag/cursor, Apply đúng một submit, Filled/Tonal, max4, lỗi catalog, Escape/focus, history, reader return và matrix hiện hữu | [public-search.log](public-search.log) |
| `cargo xtask verify web --scenario discovery` | PASS cả 6 bước selected; 181 browser assertions gồm geometry + real SQL corpus/AND/paging/SSR/reader return/race; SIGTERM graceful | [web-discovery.log](web-discovery.log) |

Browser command dùng `node apps/vink-web/e2e/browser-smoke.mjs --base
http://127.0.0.1:30560 --chrome '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'
--state /tmp/vink260/<run>/state.json --phase <phase>`. Baseline collector dùng port30561,
`/tmp/vink260/baseline-site`, **không thay CSS của final runtime**. Collector giữ chạy để
thu đủ matrix và trả exit1 nếu còn lỗi; regression thật vẫn fail-fast. Context metadata
của collector phản ánh checkout/harness mới; CSS thực phục vụ là baseline hash riêng trong
`results.json`, không giả source fingerprint đó là CSS final đã chạy ở baseline.

Selected xtask command thêm `VINK_TEST_DATABASE_URL=postgres://vink260@127.0.0.1:55560/postgres`
và `VINK_CHROME` như trên. Đây là cluster test trust riêng của task, không credential production.
Runner tự tạo/migrate/seed scratch và runtime role; SQL publications có nhãn là fixture công khai,
không chứng minh author publication. Bản direct public-search dùng cùng seed/catalog/SQL fixture
qua operator CLI hiện hữu và restricted runtime role.

## AC và self-review

| AC | Evidence và kết luận |
| --- | --- |
| AC-01 | PASS: execution base pin, native `.vk-ui-choice > label > input + span`, CSS thực khớp baseline và phép đo 9,99–10px trước sửa |
| AC-02 | PASS: 0px center error trong 80 records; cả 6 target/label rect ổn định khi 0→1→4; text range không đè mark; ảnh before/after đã mở |
| AC-03 | PASS: 320×900, 390×844, 834×1112, 1440×900 × light/dark × root text100/200; long Vietnamese NFC/NFD tại DOM label thật; không clip/overflow/overlap, target>=48px. Hai fieldset, gap token và hai cột theo spec; text200 màn hẹp xuống một cột và cuộn dọc |
| AC-04 | PASS: native name/value/checked/disabled/label không thay; scoped corpus + public-search giữ draft-vs-applied, max4 AND/deselect, Filled/Tonal, q/tag/cursor, history, catalog error và no-JS GET/validation |
| AC-05 | PASS Chrome/CDP: một pointer/Space thay đúng một choice, Enter Apply một submit; Tab/focus-visible/Escape/return, cue không chỉ màu; AX names khớp chính xác labels, glyph trang trí không lặp tên; emulated reduced-motion/forced-colors. Named screen reader/thiết bị thật NOT_RUN |
| AC-06 | PASS scoped: 66 before + 66 after PNG cùng fixture; DOM assertions/measurements, font ready, settled finite animations, DPR1, viewport/clientWidth rõ; 7 cặp đại diện và 2 ảnh media/focus sau sửa đã mở (liệt kê ở `results.json`). Không tuyên bố mở tất cả 132 PNG |
| AC-07 | PASS: docs/spec/choices/feature-map đồng bộ; CSS opt-in app consumer, không đổi global Checkbox/FilterChip, backend/Quint/empty-state/scrollbar/cleanup. Không đóng issue liên quan |

| Chiều UX | Kết quả và giới hạn |
| --- | --- |
| Functional | PASS selected discovery + public-search, PostgreSQL/Chrome thật; không full-Web |
| Geometry | PASS toàn 16 scene, 0/1/4 và NFC/NFD; tâm nhãn 0px, ổn định toggle |
| Visual | PASS self-review 7 cặp PNG đại diện đã mở: nhãn ở tâm, mark riêng; có wrap theo từ ở320/text200, cuộn dọc được chấp nhận. NO_APPROVED_REFERENCE; không chấm điểm thẩm mỹ toàn màn |
| Accessibility/Responsive | PASS browser DOM/AX/keyboard/media/reflow100/200; named VoiceOver, OS Dynamic Type/zoom và thiết bị thật NOT_RUN |
| Interaction | PASS CDP native pointer/Space/Tab/Enter/Escape, draft/Apply/history; OS IME không liên quan patch và NOT_RUN |

Một vòng sửa CSS; không đổi tiêu chí để PASS. Baseline FAIL được giữ nguyên.
Khi tăng oracle để đo cả tâm text range, lượt thử đầu timeout tải trang vì App terminal cũ
đã dừng; chưa chạy geometry assertion, giữ [FAIL môi trường](geometry-server-timeout.log). Lượt cuối dùng supervisor
giữ App/browser và ghi exit codes; không sửa CSS để xử lý lỗi này. Full `verify web`, `verify postgres|discussion|expressions|kani`,
shared preview và native app **NOT_RUN**: không đổi các boundary tương ứng; không dùng
selected PASS thay full suite. Không cần đổi Quint vì không đổi state/domain/API.
Không đọc/chờ Actions/status/log CI, không bypass branch protection, không deploy VPS.

Review source theo `vink-code-review`: chỉ một app recipe, native input ngoài normal flow,
fixed symmetric tracks không thêm slot theo checked, không đổi padding/weight/hit target;
hydration/controller/serialization byte-identical với base. Test đã fail đúng CSS cũ và
PASS source mới. Không tìm thấy regression cần chặn merge trong phạm vi đã kiểm.

Oracle cuối bổ sung tâm text range ngoài tâm hộp nhãn; [supervisor](run-owned.py.txt)
giữ App/browser cùng lượt, final browser/server exit0. Public-search, selected Web và
Quick chạy trước bổ sung riêng assertion này; production CSS/Rust/GET không đổi sau các
lượt PASS đó. Phase geometry cuối chạy oracle mới; [hash đối chiếu](visual-equivalence.json)
xác nhận cả 9 PNG final được chọn byte-identical với ảnh sau sửa đã mở. Harness hash cũ
của selected Web và hash input cuối được tách trong `results.json`.
