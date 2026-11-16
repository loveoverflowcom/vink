# Bàn viết empty WebP #259 — bằng chứng local trên Mac

**Kết quả cuối: Quick PASS; full browser phase `main` PASS, 5.818 assertions, browser/server exit 0.**
Không gọi outer xtask Web hoặc unselected Web PASS.

Base `develop@997bb2f5662a6417eebf9bc2d1d3891f925b48a0`, dependency #258/PR #266.
Production source: `f51e377`; các commit sau chỉ bổ sung/sửa test harness và evidence.
Test source cuối: `17fb74f` (production source vẫn `f51e377`).
[Source/runtime hashes](source-final.json), [scope inspection](scope-inspection.json),
[LayoutSpec](../../../design/empty-artwork-desk-259.md).

CHOICE-01 A: đổi trực tiếp hai empty branches trong `DraftListView`; input, requests, create,
focus và state tiếp tục thuộc owner cũ. Dùng đúng `EmptyArtwork::{FirstDraft, NoResults}` /
`EmptyState` của #258; không đổi composition, CSS, WebP bytes, manifest hoặc pipeline.
Editor từ `DraftEditor` tới cuối file byte-identical với base. PageChain tiếp tục từ chối
empty page có continuation; first-use cũng ghi rõ điều kiện không continuation.
Không có domain/admission/persistence transition mới: **Quint/domain NOT_APPLICABLE**.

## Checks đã tự chạy

| Command / boundary | Kết quả | Evidence |
| --- | --- | --- |
| `./tools/local-budget run -- cargo xtask build web` | PASS sau khi dùng cache npm riêng; final build peak RSS sum 2.205.008 KiB | [build](build-after.log) |
| `./tools/local-budget run -- cargo xtask verify quick` | PASS toàn lane: tokens/assets, fmt, clippy SSR/hydrate/preview, 32 doctest, SSR boundary, units/contracts, repository checks; peak RSS sum 2.287.072 KiB, wall 237,49s | [Quick](quick.log) |
| `node apps/vink-web/assets/empty/verify.mjs` và `node --test apps/vink-web/assets/empty/verify.test.mjs` | PASS ba original derivatives + ba positive/negative tests; cũng nằm trong Quick | [Quick](quick.log) |
| Existing CDP runner `--phase desk-empty`, final source `ed87f38` | PASS: artwork/geometry/network/fallback/focus và session fences; intercepted private-summary/auth fixtures, Chrome thật | [final focused](focused-final/browser.log), [32 geometry records](focused-final/desk-empty-results.json) |
| `cargo xtask verify web --scenario main`, source `ed87f38` | FAIL ENOSPC khi ghi capture; later steps NOT_RUN, exit 1 | [log](web-main-final.log), [exit result](web-main-final-result.json) |
| Existing browser runner `--phase main`, source `17fb74f`, previously built binary/bundle | PASS 5.818 assertions, browser/server exit 0; không gọi đây là xtask/full-Web PASS | [log](browser-main-r3/browser.log), [exit result + runtime hashes](browser-main-r3/result.json) |
| Node syntax, `cargo fmt --all --check`, `git diff --check`, relative links | PASS | Quick + final diff check |

CWD: `/Users/manhblue/Documents/Codex/2026-11-17/task-2/vink`; macOS 15.6 arm64,
Rust 1.99.1, Node 22.15.0, Chrome 157, wasm-bindgen 0.2.129, PostgreSQL 17.11 Homebrew.
Build/Quick qua resource-budget wrapper; `CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0
CARGO_PROFILE_TEST_DEBUG=0`, jobs/libtest ≤2. Browser lane là **UNSUPPORTED qua wrapper**,
chạy riêng với cùng giới hạn jobs/threads, không tự nhận đã tham gia host-user admission slot.
Npm cache riêng `/tmp/vink259-npm`; PostgreSQL scratch riêng loopback 55559, user `vink259`.
Không dùng production DB, không deploy VPS, không đọc/chờ CI.

## Acceptance và giới hạn

| AC | Bằng chứng | Kết luận hiện có |
| --- | --- | --- |
| AC-01 | Shared owner/assets/style không đổi theo diff; đúng 20.070/19.190 bytes, 512×427, VP8+alpha/static/hash; HTTP MIME/bytes/hash và decode thật; tối đa một request mỗi transition first-use → miss → clear (cho phép decoded-cache reuse), không thêm request khi đổi theme/viewport/text scale | PASS |
| AC-02 | Held initial read; successful unfiltered head vs applied miss; error/401 ở dashboard; PageChain continuation rejection và retained stale qualifier; ảnh matrix đã mở | PASS focused + real paging trong main cuối |
| AC-03 | Một create target; shared recovery; raw main trước interruption đã kiểm một POST commit thật rồi mất ack, GET lỗi/success/reconciliation không replay; focused final kiểm late POST qua session | PASS main cuối + focused session fences |
| AC-04 | Search node/unapplied text/focus/caret/composition; clear focus; B/C account fixture xác nhận qua Session owner + BroadcastChannel rồi trả old GET/201 POST muộn; old reply không lộ/điều hướng; Editor byte-identical; SSR private payload assertion trong main | PASS cho các ca đã chạy; auth là transport fixture, không cookie/login thật |
| AC-05 | alt rỗng, một result status; blocked image naturalWidth=0; đúng một corrupt response inject; câu/action HTML còn dùng được; stale/error/create uncertainty warnings hiện hữu | PASS |
| AC-06 | 32 scenes mỗi before/after, 8 privacy screenshots mỗi run; 192px mobile / 240px desktop, body 16/32px; CTA/privacy scroll tới được trên nav; no horizontal overflow; 16 contact sheets đã mở và raw PNG đại diện | PASS, self-review; text200 là root font-size, không zoom |
| AC-07 | Owner docs/feature map/asset README cập nhật, scoped evidence giữ FAIL và NOT_RUN; một PR Web #259 sau #266; native #261 riêng | PASS local evidence/scope; draft PR được ghi trong issue live #259 |

Session fixture đi qua tab Session owner thật nhưng phản hồi auth/profile được intercept rõ ràng;
không giả đây là đăng nhập/cookie hoặc database authority test. Real PostgreSQL create/paging
thuộc `main`; browser image rendering và Node HTTP bytes là hai checks riêng. Browser main trực
tiếp dùng scratch DB riêng với credential local của task, không thay thế qualification runtime-role/
PostgreSQL integration lane. Setup catalog dùng đúng sáu tag/alias/description của `xtask/src/web.rs`.
[Script invocation được lưu](run-browser-main.py.txt); commands/cwd/runtime hashes và exit result
được ghi bởi supervisor. Full raw captures giữ local theo [artifact locations](raw-artifact-locations.json),
PR chỉ mang selected evidence để không nhân toàn bộ screenshot của các flow ngoài #259.

## Review hình ảnh và UX

[Before metadata](before/screenshots/contexts.json) / [After metadata](after/screenshots/contexts.json)
/ [contact-sheet inspection map](review-sheets.json). Mỗi sheet là bốn PNG nguyên nội dung:
trên trái light100, trên phải light200, dưới trái dark100, dưới phải dark200.
40/42 PNG của focused final **byte-identical** với after đã mở; hai ảnh blocked/corrupt còn lại
được chụp khi transition đã settle và đã mở riêng. [Hash đối chiếu](visual-equivalence.json).
Main cuối cũng có 40/42 ảnh byte-identical với after đã mở; hai fallback và hai ảnh recovery
mobile200 cuối đã mở riêng: [đối chiếu main](visual-equivalence-main.json),
[selected main metadata](browser-main-r3/screenshots/contexts.json).
Ảnh before không dùng để đo contrast; final harness đợi finite animations trước phép đo.

| Chiều UX | Kết quả và boundary |
| --- | --- |
| Functional | PASS focused + toàn phase browser main cuối, gồm real-create/paging/editor/storage |
| Geometry | PASS 32 scenes; sizing/type/overflow và recovery/privacy clearance đo trong browser |
| Visual | PASS self-review: bỏ panel/icon/serif heading lặp, image → một câu → action; không sửa fonts/tokens. Artwork vẫn **NO_APPROVED_REFERENCE** |
| Accessibility/Responsive | PASS alt/status/keyboard/DOM/reflow; contrast tối thiểu 6,72 light, 8,16 dark; named screen reader NOT_RUN |
| Interaction | PASS CDP input identity/caret/composition, clear, held reads, session generations và late create; OS IME/device NOT_RUN |

Historical attachment capture `8424a39...`, 2/66 INTERRUPTED trong issue vẫn là provenance partial;
không nâng nó thành full PASS. Ma trận mới này là run riêng trên Mac.
Independent work-chat review chưa chạy; PR phải giữ draft và không merge.

## Lịch sử FAIL / interruption

1. [Build baseline đầu](build-before.log): **FAIL** npm EACCES/EEXIST ở shared `~/.npm`.
   Không sửa quyền/xóa cache người dùng; [retry](build-before-retry.log) PASS với cache task riêng.
2. [Baseline](before/browser.log): PASS geometry/recovery trước sửa, 40 PNG. New artwork regression
   trên runtime cũ **FAIL đúng vì ảnh chưa tồn tại**: [negative control](negative-before/browser.log).
3. [Focused đầu](after-initial/browser.log): **FAIL** contrast đo giữa dark-theme transition
   (button ratio 1,03). Đợi finite animations, giữ tiêu chí ≥4,5 và không đổi product CSS;
   [focused sau](after/browser.log), [final focused](focused-final/browser.log) đều PASS.
4. [Main đầu](web-main.log): browser **FAIL `fetch failed`** sau 3.852 assertions; terminal session
   không còn (`Unknown process id`) và App server không còn sống, không có outer exit result.
   Không coi run này là full main PASS. Retry dùng task-owned supervisor ghi PID/exit JSON.
5. [Main xtask retry](web-main-final.log): **FAIL ENOSPC**, exit 1, khi ghi capture sau 2.326 assertions.
   Dọn compiler cache của chính #259; binary/bundle/evidence giữ nguyên hash
   ([cleanup](own-compiler-cleanup.json)). Không rebuild, chạy browser phase riêng trên artifact đã pin.
6. [Browser main đầu](browser-main/browser.log): **FAIL** oracle cố định đúng 3 request sau
   3.830 assertions; resource trace chỉ có hai WebP request, do ảnh đã decode có thể được dùng lại.
   Giữ đúng budget “không request thừa”, scope collector bằng current document loader và kiểm
   không có request mới qua toàn matrix. Không sửa app hoặc ép browser tải lại ảnh để đạt số đếm.
   [Lượt sửa oracle](browser-main-r2/browser.log) PASS toàn phần #259, sau đó FAIL assertion
   R6 one-head-GET do fixture Session/BroadcastChannel còn ảnh hưởng context kế tiếp.
7. Cô lập toàn `deskEmptyState` trong browser context riêng, dispose sau kiểm. Giữ nguyên
   assertions R6/paging. [Main r3](browser-main-r3/browser.log) PASS 5.818 assertions, gồm
   isolated #259, R6 one-head-GET, cả hai traversal thật, editor và public storage reopen;
   [supervisor](browser-main-r3/result.json) ghi browser/server exit 0.

## Bảo toàn máy và phần chưa chạy

[Preservation](preservation.json): 20 checkout có status/diff hashes trước/sau giống nhau.
Chỉ dọn `target/debug` và `target/wasm32-unknown-unknown/debug` có thể tái tạo của review #265
đã idle, sau khi kiểm worker/process; giữ source, capture/log, DB và worktree khác. Sau ENOSPC,
dọn thêm compiler cache của chính #259 và giữ nguyên binary/site/evidence; xem cleanup JSON.
Historical checkout #204 không có trong inventory lúc nhận task (evidence review #265 trước đó
cũng ghi đã vắng); các PG/Chrome process và artifacts #204 còn có đều không bị chạm.
Không tuyên bố đã xác minh một dirty checkout #204 không còn hiện diện.

**NOT_RUN:** unselected Web scenarios, real named OS Telex/VNI/mobile keyboard, VoiceOver/TalkBack,
Safari/Firefox, physical device/native/CMP, browser zoom và production deployment.
Không có quyết định sản phẩm/kiến trúc mới hay signoff; giữ status registry hiện hành.

Display copy của main log chỉ bỏ trailing spaces/tabs để qua whitespace check; raw log nguyên
byte giữ local, line count/verdict không đổi: [normalization receipt](log-normalization.json).

Capture contexts được xuất một record mỗi dòng để diff dễ review hơn; keys/values/schema không
đổi và PNG không sửa. Pretty raw JSON còn local: [compaction receipt](metadata-compaction.json).
