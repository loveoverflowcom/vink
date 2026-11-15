# Bàn viết gọn và Thấm — #213

Nguồn yêu cầu: [issue #213](https://github.com/loveoverflowcom/vink/issues/213),
không có comment tại lần đọc đầu 15/11/2026. Base mới nhất lúc tạo branch:
`97b29c30a87b5403759b2219ab0cdc710fb64cbd`; branch `codex/desk-compact-tham`.
[LayoutSpec delta](../design/desk-server-pagination.md#delta-213--hàng-gọn-và-thời-gian).

CHOICE-01 A: thời gian tuyệt đối vi-VN/device zone sau hydration, fallback UTC có zone rõ
trên SSR/no-JS. CHOICE-02 B: DraftRow compose metadata riêng; DraftMetadata/editor giữ version.
Full Unicode title giữ trong DOM/accessibility, focus/hover hiện đủ, touch mở cùng href editor.
Không đổi DTO, SQL, CAS, policy vote, command/order/session hoặc icon Pen Flight.

Snapshot decision registry `cef5662c19d3db1219bab14d197b5413f203aa816c99eaa11e8f2c31688091c2`:
`LVE-PR-01-CHOICE-02@r1` và `LVE-PR-01-CHOICE-08@r1` vẫn `DESIGN_PROPOSED`;
đổi copy không nâng trạng thái quyết định. Mức triển khai: `web` và copy `cmp` patterns.

Bằng chứng local được ghi bên dưới; không kế thừa PASS của #201/#202. Không đọc/chờ CI.

## Bằng chứng và môi trường

macOS aarch64, Rust 1.99.1, Node 22.15.0, Chrome 157; PostgreSQL 17 Homebrew riêng tại
`/tmp/vink-213-pg`, loopback `55439`, không dùng DB dev. Cấu hình build giới hạn dung lượng:
`CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0`;
các lượt cuối thêm `CARGO_BUILD_JOBS=1`. Chrome truyền qua `VINK_CHROME` trỏ bản cài local.

Source Rust của runtime đã build tại `d4f5038`; không đổi sau SHA đó. Delta sau đó chỉ CSS,
reference/copy docs và diagnostic JS. Phase tập trung ghim `ddf95a441ddcc99bdadea8528523d692dd624099`.
Stylesheet tập trung được ghép đúng bốn nguồn/thứ tự của `xtask/src/build.rs::stylesheet`;
hash CSS server đã fetch: `b15479560e070e13caa2135cf7fd82e74ffbeb43bfaa839b58b87fb33031a554`.
Không nhận đây là một full build/lane PASS. Runtime fixture dùng migrator/runtime roles tách,
`provision.sh`, migrations/dev-seed và cùng sáu tag + alias/description của `xtask`.

| Kiểm | Kết quả và ranh giới |
| --- | --- |
| `cargo test -p vink-patterns` | PASS 12 unit/SSR tests; offset đổi ngày UTC, invalid/future, full Unicode/href, version ở consumer khác, copy/reasons/counts |
| `JAVA_HOME=<Temurin24> ./gradlew :patterns:desktopTest --tests vink.patterns.EndorsementTest --no-daemon` | PASS 1 test, 14 tasks; common logic qua JVM, không native render |
| `node apps/vink-web/e2e/browser-smoke.mjs --base http://127.0.0.1:3013 --chrome <Chrome> --state target/issue213/final-desk/state.json --phase desk-layout` | PASS toàn phase; Desk + public search/byline/SSR/no-JS/hydration; assertions cũ giữ nguyên ngoài delta resume/row được thay bằng contract #213 |
| Cùng runner, `--state target/issue213/final-editor/state.json --phase editor-chrome` | FAIL ở caret sau desktop→390×420: range y −118,72…−93,72, scrollY 1865; toolbar static, cùng editor, không console error. Các assertions toolbar/node/text/selection/undo/composition/save/leave trước đó PASS; phần sau điểm dừng NOT_RUN |

Lỗi caret cũng tái hiện trên baseline `97b29c3` trong `main`, trước source #213. Không đổi bound,
không cuộn hộ test, không coi đây là PASS. Ảnh lỗi đã mở:
`target/issue213/final-editor/screenshots/editor-short-mobile-caret-failed.png`.
Chưa xác định root cause; không rewrite CodeMirror/save authority để che lỗi. Resume check là
phase `editor-chrome` ở trên, rồi full `main`, với cùng Chrome/platform và đủ dung lượng.

Các lượt FAIL giữ nguyên trong log:

- `/tmp/vink-213-baseline.log`: dừng có chủ ý do build debug gây áp lực đĩa.
- `/tmp/vink-213-baseline-small.log`: FAIL lockfile khi bắt đầu đổi manifest quá sớm; đã giữ patch
  và trả source baseline để lấy run trước sửa riêng.
- `/tmp/vink-213-before.log`: baseline main FAIL caret; ảnh/metadata trước sửa giữ tại
  `target/verify-web/1794759707545538000-41816/`.
- `/tmp/vink-213-quick.log`: FAIL generated catalog/icon drift; đã sửa source JSON + generator.
- `/tmp/vink-213-quick-2.log`: tokens/fmt/server clippy PASS, WASM clippy FAIL `ENOSPC`; các bước sau NOT_RUN.
- `/tmp/vink-213-web.log`: source #213 main FAIL `ENOSPC` khi ghi ảnh sau khi đã qua Desk,
  timestamp và pagination; auth/endorsement của lượt này NOT_RUN. `contexts.json` của run
  `1794760453317817000-58313` bị mất vì write thiếu đĩa; không dùng nó làm nguồn geometry.
- `/tmp/vink-213-desk.log`: toàn bộ phần Desk qua, public-search FAIL vì thiếu catalog ở fixture
  thủ công; đã seed đúng catalog qua CLI, không thay assertion.
- `/tmp/vink-213-public.log`: bắt được regression byline time 174,23px trong track 249px ở 320/200.
  Sửa đúng CSS `time { display: block }` trong date track. `/tmp/vink-213-desk-2.log` vẫn nhận
  stylesheet cũ trong profile đã mở; fresh profile `/tmp/vink-213-public-2.log` PASS public-search.
- CMP: `/tmp/vink-213-cmp.log` FAIL JBR25/Gradle version parser; `cmp-24.log` FAIL offline cache
  thiếu Kotlin plugin. `cmp-online.log` PASS bằng JDK24 và dependency pin của repo.

## Self-review ảnh và geometry

Baseline là regression input, `fidelity: NO_APPROVED_REFERENCE`; không dùng ảnh Drive lịch sử làm
golden. Đã mở 16 ảnh trước và 16 ảnh sau: `desk-long-{320,390,834,1440}-{100,200}-{light,dark}`,
viewport cao 900 CSS px, DPR 1; 200% là root font-size 32px, không browser zoom hay OS font scale.
Nguồn sau cuối: `target/issue213/final-desk/screenshots/contexts.json` (250 records).
Cả 16 PNG sau cuối có SHA-256 giống hệt các PNG sau đã mở của run `1794760453317817000-58313`;
metadata cuối hợp lệ và được đọc riêng, không suy geometry từ file mất của run cũ.

| Cùng fixture/state (CSS px) | Search y trước → sau | Hàng đầu cao trước → sau |
| --- | --- | --- |
| 390 / 100 / light | 394,97 → 140,80 | 248,95 → 105,98 |
| 320 / 200 / dark | 1074,67 → 405,16 | 1275,34 → 295,16 |
| 834 / 100 / light | 367,56 → 140,80 | 163,17 → 105,98 |
| 1440 / 100 / light | 338,97 → 140,80 | 105,98 → 105,98 |

Search y là input rect trong viewport ở scrollY=0; common axis kiểm theo search shell/heading/list,
không nhầm left của input (có leading icon) với left của shell. Bản trước có focus còn trên resume
ở một số ca; bản sau park pointer ngoài controls và blur để đo row mặc định. Title hai dòng vẫn có
full text trong AX/DOM; focus mở đủ, row touch/keyboard mở đúng editor. Đã mở thêm ảnh last-row
320/390 chữ200 (focus nằm trên bottom nav), hai timezone và toolbar/editor narrow200.
Nav wrapping hai dòng ở 200% được giữ; không giảm font/target hoặc ép nowrap.

## Auth và endorsement chạy riêng

Lệnh gộp `cargo xtask verify web --scenario auth --scenario endorsement` FAIL do `ENOSPC` khi
Cargo build dependency `rustls`, trước khi vào runner; hai scenario của **lệnh đó** NOT_RUN.
Để hoàn tất phần độc lập, chạy trực tiếp cùng browser phases trên các binary đã build trong
cùng checkout (Rust không đổi sau `d4f5038`), cùng restricted App/Identity roles và schema,
local fixture/native Identity/local-outbox như setup của `xtask/src/web.rs`. Không gỡ assertion.

| Phase hiện hữu / log | Kết quả |
| --- | --- |
| `--phase auth --base http://127.0.0.1:3015 --outbox /tmp/vink213-outbox --state target/issue213/final-auth/state.json` | PASS 79 assertions; App không fixture, Identity email-code thật; account A/B, protected drafts, reader lifecycle và stale answers, logout/expiry/limiter |
| `--phase endorsement --base http://127.0.0.1:3014 --author-base http://127.0.0.1:3013 --state target/issue213/final-endorsement/state.json` | PASS 48 assertions; hai fixture servers/DB thật; moderation, double tap một command, không optimistic, confirmed/check, lost ACK exact replay, stale order reconciliation, guest/unknown, public counts, Pen Flight 20px/targets 48px |

Cả hai dùng `node apps/vink-web/e2e/browser-smoke.mjs --chrome <Chrome>`; log lần lượt
`/tmp/vink-213-final-auth.log`, `/tmp/vink-213-final-endorsement.log`. Đây là phase PASS riêng,
không full lane PASS hoặc production auth acceptance. Desk phase cuối PASS 569 assertions.
Đã mở ảnh `lve-signed-out-reason`, `discussion-lve-local-postgres-390-200-light` và
`discussion-lve-local-postgres-1440-dark`: nhãn Thấm cố định, check riêng, lý do guest và icon giữ nguyên.

## Kiểm kê thuật ngữ giữ lại

`rg` trên source/test/catalog hiện hành của `apps/` và `packages/` không còn “Đánh giá cao”/
“đánh giá cao”. Những nơi còn lại được giữ có chủ đích:

| Nhóm nguồn | Lý do |
| --- | --- |
| `decisions/*.toml`, DecisionDelta 10/11 và CHOICE lịch sử trong ADR LVE | Bản nhập/trích dẫn chủ sản phẩm được pin; delta thuật ngữ mới ở cuối ADR, không viết lại payload hoặc signoff |
| `prd/vision.md`, mục RANK/TOP trong `prd/requirements.md`, `docs/architecture/discovery-ranking.md`, reference `vink-shape/domain-lenses.md` | Thuật ngữ của policy/OPEN O-07 và kế hoạch ranking 10/11, không active action copy hoặc ranking runtime mới |
| `docs/architecture/work-taxonomy.md` | Diễn giải/tradeoff lịch sử về revision anchor và count; không nhãn control hiện hành |
| `docs/development/*endorsement*evidence.md`, `pen-flight-r2-evidence.md`, `docs/design/brand/vpen-r1/evidence/*.log` | Evidence lịch sử giữ SHA, câu chữ và kết quả lần chạy gốc |

Không đổi Hữu ích, enum/API/storage identifiers, version/CAS, icon SVG, policy vote hoặc ranking;
không metrics API, SQL/counter, tracking, native product app. Không tick/close issue #213 hay issue khác.

Own-content/no-write và HTTP `403 self_endorsement` đã PASS trong phần tác giả của
`/tmp/vink-213-web.log` trước ENOSPC (dòng 408–414); không gán các assertion đó cho phase reader.

Preview: `CARGO_BUILD_JOBS=1 ... target/debug/xtask ui-preview leptos verify` dùng cùng CLI
không đổi source ở `xtask/` (tránh Cargo launcher đang lỗi thiếu đĩa). Generator và CSR preview build
PASS, prerequisites PASS; media hydration fixture build FAIL `ENOSPC` tại `tachys`; browser preview
assertions phía sau NOT_RUN. Log `/tmp/vink-213-preview.log`. Không coi build là preview verify PASS.

CMP focused cuối: `JAVA_HOME=<Temurin24> ./gradlew :ui:desktopTest --tests vink.ui.VinkToggleTest
:patterns:desktopTest --tests vink.patterns.EndorsementTest --no-daemon` PASS, 18 tasks
(6 executed, 12 up-to-date); log `/tmp/vink-213-cmp-final-online.log`. Lượt `--offline` trước
đó FAIL vì thiếu `ui-test-junit4:1.9.0`, không lỗi test. Android/iOS, OS IME và screen reader NOT_RUN.
Diff/whitespace, `cargo fmt --all -- --check`, bốn `node --check`, generator check PASS;
436 local Markdown targets có thật, AGENTS 17.203 bytes <24.000. Không sửa YAML metadata.

Kiểm kê consumer: recipe Web `screen.desk` được bỏ resume và lời claim limit100; field/caption
ghi rõ là specimen không nối tìm/phân trang. Recipe CMP giữ khối cũ với nhãn lịch sử và
không claim là bố cục Web/native hiện hành. `DraftResume`/`DraftMetadata` API giữ tương thích;
không mount trong Desk Web, DTO version/CAS và metadata editor không bị xóa.

## Source và kiểm nguồn cuối

Code/fixture cuối: `f537b4c54c844279c5ae8271304e14d710f043e4`; commit tiếp theo chỉ ghi báo cáo này.
`cargo xtask verify quick` cuối PASS **toàn lane** trên source này (build một job, debug/incremental
tắt như trên), gồm generator, expressions manifest, fmt, clippy server + hydrate WASM + preview CSR,
literary doctests, toàn bộ unit/contract và repository checks. Log `/tmp/vink-213-quick-final.log`.
Lượt quick trước (`/tmp/vink-213-quick-3.log`) cũng PASS; giữ nguyên các FAIL tài nguyên đã liệt kê.
Fresh fetch cuối vẫn `origin/develop@97b29c30a87b5403759b2219ab0cdc710fb64cbd`; issue body
vẫn cập nhật lần cuối `2026-11-15T14:17:28Z`, comments rỗng, không thay checklist GitHub.

## Preview và consumer CMP cuối

Lượt canonical cuối `/tmp/vink-213-preview-final.log`: generator, CSR build và media-hydration
fixture build PASS; Node helper tests 7/7 PASS; browser smoke FAIL `WebAssembly compilation aborted:
Network error`. Không đổi host/runner trong repo để báo xanh.

Chạy **cùng** `preview-smoke.mjs`, catalog và assets vừa build qua Python HTTP loopback 3023
để tách boundary host. Media hydration và các kiểm component/state/navigation/endorsement,
contrast, no-overflow mọi scenario ở 320/390/834/1024/1440 text100/200 và font self-host PASS;
script sau đó FAIL `ENOSPC` khi ghi capture `field.radio--default--light--1024.png`. Vì vậy
**preview verify vẫn FAIL**, các capture/check sau điểm dừng NOT_RUN; không suy toàn suite PASS.
Log `/tmp/vink-213-preview-http.log`, ảnh ở `target/issue213/preview-http/`. Đã mở full-page ảnh
`screen.desk--default--light--390` và `composition.endorsement--default--light--390`; đây là gallery
full-page, không dùng để kết luận geometry overlay theo viewport. `ToggleAction` có specimen
large count 1.234.567 trong matrix hiện hữu; không tuyên bố mọi count cực đại đã được visual QA.
Khác biệt HTTP host là evidence chẩn đoán; chưa kết luận hoặc sửa root cause canonical network error.

`JAVA_HOME=<Temurin24> ./gradlew :diagnostics:compileKotlinDesktop --no-daemon` PASS, 12 tasks;
log `/tmp/vink-213-diagnostics.log`. Đây là compile consumer mẫu, không native runtime hay capture CMP.

## Đối chiếu AC và kết quả năm chiều

Checklist authority duy nhất vẫn ở issue #213; bảng này chỉ ánh xạ evidence, không tick/close.

| AC | Source/run và kết quả |
| --- | --- |
| AC-01 | PASS scoped Desk: không resume/Recent1, first-load failure, query miss/empty, giữ data khi refresh lỗi, create vẫn có đúng gate; chỉ head không lọc/part1 rỗng mới first-use |
| AC-02 | PASS patterns unit + Desk browser: full Unicode/AX, focus/touch/href đúng, không version trên row; real saved DTO vẫn version2/CAS |
| AC-03 | PASS active copy scan + Web/CMP tests/catalog; inventory lịch sử phía trên, icon và Hữu ích không đổi |
| AC-04 | PASS scoped author + reader + pure/preview assertions: action names, unknown/pending/uncertain/replay/order/counts, self/guest reasons; full preview result FAIL riêng như trên |
| AC-05 | PASS timestamp unit, hai device zones, UTC→local day, invalid/future, raw datetime/title và public SSR/no-JS/hydration |
| AC-06 | PASS `draftPagination` trong main trước ENOSPC: real 250 corpus, both sorts, 24/page, 240/part, append/restart/refresh/query A→B→A; final quick paging/read-owner units và Auth real accounts/session fences PASS |
| AC-07 | PASS toolbar/nav specified 320/390 text200, editor identity/text/selection/undo/composition giả lập và recovery trước điểm dừng; FAIL broader inherited caret 390×420 text100; phần editor sau đó NOT_RUN |
| AC-08 | PASS scoped self-review 16 before/after pairs + metadata/axis/last-row; không approved reference, native/OS IME/screen reader NOT_RUN |
| AC-09 | PASS current docs/spec/copy/recipe/fixtures + quick final; không fake metrics, policy đổi hoặc sửa evidence lịch sử. Full main/preview FAIL không bị che |

| Chiều | Kết quả theo đúng boundary |
| --- | --- |
| Functional | PASS Desk 569, Auth 79, endorsement 48 và full quick; full main FAIL tài nguyên/caret, không full Web lane PASS |
| Geometry | PASS list/search common axis, clamp/focus, targets/last row, public byline sau fix; FAIL caret resize kế thừa y<0 |
| Visual | PASS self-review ảnh đã mở, 100% gọn hơn; 200% cho phép cuộn và wrap; `NO_APPROVED_REFERENCE` |
| Accessibility/Responsive | PASS Chrome AX names/full title, focus, contrast, root text100/200; OS screen reader/native/real IME NOT_RUN |
| Interaction | PASS scoped search/composition/create/GET recovery, Thấm ACK/replay/session; FAIL inherited caret resize; phần sau điểm dừng NOT_RUN |

Các phase thủ công dùng DB test có state sau lần chạy (endorsement work=1); để chạy lại dùng
harness canonical tạo fresh scratch hoặc một DB test mới, không reset DB dev. Không có kết luận
SLA, production auth, native product app, real AI provider, full formal/model proof hoặc screen reader.

Bàn giao reviewer độc lập: review net diff với base `97b29c3`; chú ý first-use/part boundary,
input/read owner, timestamp hydration và byline width, copy contract hai renderer. Tái hiện caret
bằng phase `editor-chrome`, đối chiếu baseline trước khi quy thành regression #213. Khi có dung lượng
ổn định, chạy lại `cargo xtask verify web --scenario main --scenario auth --scenario endorsement`
và `cargo xtask ui-preview leptos verify`; giữ mọi assertion. Giữ PR draft để reviewer quyết định
phần gate còn thiếu; không merge/deploy, không mở việc #214 hay hàng đợi UI cũ.

Kết thúc: đã dừng các App/Identity/Python fixture servers và cluster PostgreSQL do task tạo;
source, data test, outbox, logs và captures được giữ. Source code cuối `f537b4c`; source runtime
App/Rust giống `d4f5038`, JS phases ghim `ddf95a4`, CSS hash đã xác nhận ở trên. Native diagnostics
compile tại source cuối; báo cáo này là delta documentation cuối cùng.
