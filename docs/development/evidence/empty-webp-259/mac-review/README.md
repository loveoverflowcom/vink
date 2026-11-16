# Review độc lập PR #267 · Mac

Review source tại `07a840bb2217dad2d29ff79b0721abd8e1dc4e43`, so với latest fetched
`develop@997bb2f5662a6417eebf9bc2d1d3891f925b48a0` (merge #266 / dependency #258).
Reviewer khác agent triển khai. Đọc issue #259, comments (rỗng), PR metadata, AGENTS,
skill index, code review/Web/LayoutSpec/UX guidance và owners ngoài diff.

**Không phát hiện defect cần sửa. Independent browser `main` PASS: 5.818 assertions, browser/server exit 0.**
Không thay production code hoặc làm yếu assertions. CHOICE-01 A phù hợp: đổi hai
empty slots tại owner hiện hữu, không chuyển lifecycle hoặc input vào composition.

## Provenance và phạm vi

[Source integrity](source-integrity.json): 340 hash source khớp manifest của tác giả;
bốn artifact runtime cũng khớp. Không production delta sau `f51e377`; Editor từ
`DraftEditor` tới cuối file byte-identical với base. Shared composition, CSS, manifest
và hai WebP bytes không đổi. Không cần rebase khi base đã là latest develop.

Checkout mới `/Users/manhblue/Documents/Codex/2026-11-17/task-3/vink`.
macOS 15.6 arm64, Node 22.15.0, Chrome 157.0.8103.78, PostgreSQL 17 Homebrew.
Copy binary/site từ checkout tác giả sang target riêng, kiểm hash trước chạy; không rebuild.
PostgreSQL cluster riêng tại `../review-runtime/pgdata`, loopback `55567`, user `vink267`;
database mới cho mỗi attempt, App loopback `52567`, LocalFixture được ghi rõ trong log.
Không dùng database production, không chạm checkout/process/artifact phiên khác.
Browser lane UNSUPPORTED qua local-budget; chạy tuần tự với jobs/threads ≤2.

## Kiểm đã chạy và lịch sử

| Check | Kết quả và boundary |
| --- | --- |
| `node apps/vink-web/assets/empty/verify.mjs` | PASS codec/alpha/frame/bytes/hash của ba derivative dùng chung |
| `node --test apps/vink-web/assets/empty/verify.test.mjs` | PASS 3 tests, gồm animated/truncated/lossless/hash-drift negatives |
| `node --check` hai runner bị đổi; `cargo fmt --all --check`; `git diff --check` | PASS |
| Author resource-budget Quick | PASS được đối chiếu source/log; **không nói reviewer chạy lại Quick** |
| Independent existing runner `--phase main`, attempt 1 | **FAIL**, 2.079 assertions trước timeout chọn light ở `/toi`; server exit 0, không console error; phần #259 sau đó NOT_RUN. [Log](main/browser.log), [result](main/result.json), [ảnh lỗi đã mở](main/failure.png) |
| Diagnostic setup đầu | **FAIL** import do temporary copy sai depth; chưa chạy browser assertions. [Log](nav-probe/browser.log), [result](nav-probe/result.json) |
| Focused bottom-navigation diagnostic | PASS 540 assertions; light/dark radio state đúng, keyboard/AX/geometry và negatives. Chỉ thêm logging trước/sau chooseTheme vào temporary copy, không đổi app/helper/assertion. [Patch](theme-probe.patch), [log](nav-probe-r2/browser.log), [result](nav-probe-r2/result.json) |
| Existing unmodified runner `--phase main`, retry trên DB/Chrome mới | **PASS 5.818 assertions**, browser/server exit 0; [log](main-r2/browser.log), [result + runtime hashes](main-r2/result.json) |

Timeout đầu chưa tái hiện trong focused scenario hoặc full main retry. Đây là failure quan sát được trong
code helper/appearance không đổi từ base, chưa có bằng chứng là regression do #259;
không tự quy thành lỗi môi trường hoặc sửa sản phẩm theo giả thuyết.
[Supervisor đã dùng](run-review.py.txt); raw attempts giữ tại `target/mac-review-267/`.
Author outer xtask Web interruption/ENOSPC vẫn FAIL; không đổi thành full Web PASS.

## Đối chiếu acceptance

| AC | Source / oracle đã review |
| --- | --- |
| AC-01 | Một `EmptyState`/`EmptyArtwork` của #258; mapping đúng FirstDraft/NoResults, không fork bytes/CSS/pipeline; validator PASS |
| AC-02 | `ready`, unfiltered applied query, successful loaded head/part1, empty và không continuation mới first-use; error/auth/retained qualifier do owner cũ; PageChain từ chối empty continuation trước commit |
| AC-03 | Header create dùng `!first_use`, empty dùng cùng CreateDraftButton; Pending/Uncertain/Reconciled, disabled và reconciliation giữ nguyên; không thêm mount POST/retry |
| AC-04 | Một DraftListView/search owner qua state; read ticket + session scope generation fence và late create fence giữ nguyên; Editor byte-identical, SSR không private fetch |
| AC-05 | EmptyState không thêm status; `alt=""`, intrinsic dimensions, một câu và control HTML còn khi ảnh lỗi; warning/stale/uncertain độc lập với artwork |
| AC-06 | Đã mở 8 after sheets = 32 scenes ở 320/390/834/1440 × light/dark × text100/200; mở thêm before first-use 320 và miss390 để so đúng thay đổi; không global token/font edit |
| AC-07 | Scope chỉ Desk adoption + focused tests/docs; hai Web PR #266/#267, CMP riêng #261; giữ provenance FAIL và NOT_RUN |

AC-01..AC-07 **PASS trong phạm vi Web đã nêu**, kết hợp source inspection với independent main.
Network kiểm đúng HTTP MIME/bytes/hash và browser decode; blocked/corrupt giữ HTML/action;
create/lost-ack/GET reconciliation và cả hai traversal chạy với PostgreSQL thật; session auth
answers là transport fixtures đi qua Session owner, không là cookie/login qualification.
Không thay PostgreSQL runtime-role/integration lane bằng kết quả này.

[32 geometry records](main-r2/desk-empty-results.json), [42 capture contexts](main-r2/capture-contexts.json)
và [PNG hash equivalence](visual-equivalence.json): 40/42 ảnh mới byte-identical với after đã mở
(gồm toàn bộ 32 scenes); hai fallback khác hash được mở riêng. Mở thêm privacy320-dark200 và
privacy390-light200 của run mới; bốn PNG này được giữ trong thư mục `main-r2`.

## UX và giới hạn

| Chiều | Kết quả quan sát |
| --- | --- |
| Functional | PASS independent browser main: real PostgreSQL create/lost-ack/reconciliation, query/paging hai sort và các flow hiện hữu |
| Geometry | PASS independent 32 scenes: artwork 192/240 CSS px, text16/32px, không overflow; CTA/privacy cuộn trên nav |
| Visual | PASS độc lập trên 32 scenes đã mở: art → câu → action; bỏ panel/icon/heading lặp. `NO_APPROVED_REFERENCE`, không tự duyệt golden |
| Accessibility/Responsive | PASS Web alt/status/control, focused nav AX/keyboard và reflow; contrast min 6,72 sáng / 8,16 tối. Named screen reader NOT_RUN |
| Interaction | PASS input/caret/focus/composition simulation, clear, held reads, confirmed account change và late GET/POST fences; real OS IME NOT_RUN |

Native/CMP, real OS Telex/VNI/mobile keyboard, named screen reader, Safari/Firefox,
physical device và browser zoom **NOT_RUN**. Root-font text200 không phải browser zoom.
Quint/domain/persistence **NOT_APPLICABLE** vì không đổi transitions/API/SQL.
Không đọc/chờ CI, không deploy VPS. Merge/issue state được xác minh riêng sau review.

Capture contexts được lưu một record mỗi dòng; [receipt](metadata-compaction.json) xác nhận parsed JSON không đổi.
