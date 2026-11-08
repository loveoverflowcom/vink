# Search / Surface R5.1 #90 — bằng chứng local

Ngày 08/11/2026; base `develop@6c97917b6a7aa4c51f3b2fe816462c8a0a47fc64`, branch
`codex/quiet-search-list-chrome`. [DecisionMap/LayoutSpec](../design/search-r5.md)
ghi choices và boundaries. Concept Drive là design-reference, không là screenshot app.

## Môi trường và fixture

macOS arm64; Rust 1.99.1, Node/Chrome headless cài trên máy; PostgreSQL 17.11 mới,
cluster tạm loopback55491 và database riêng `vink_r5_before`/`vink_r5_after`.
Dữ liệu seed có nhãn Mẫu và HTTP fixture hiện có trong runner: public12/draft100,
NFD, title dài/không khoảng trắng, rỗng/một/nhiều, errors và last-success. Không dữ
liệu hay auth production. Root text scaling100/200 khác browser zoom/thiết bị thật.
Build dùng `CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_DEV_INCREMENTAL=false` để vừa
ổ đĩa; không đổi optimization/debug assertions; npm cache riêng trong `/tmp`.

Baseline `desk-layout` đã PASS trên develop trước patch. Lần setup đầu nối nhầm
role của cluster cũ nên FAIL tại real authorized list; đã giữ log và chuyển sang
cluster fixture riêng, migrate/seed, chạy lại đầy đủ. Lần after đầu FAIL do selector
tooltip không được scope đúng sau dấu phẩy; đã sửa selector và chạy lại.

## Kết quả

Lệnh chạy từ repo root với các biến build trên; `VINK_CHROME` trỏ Chrome cài sẵn,
`VINK_TEST_DATABASE_URL` chỉ trỏ cluster fixture mới. Node22.15.0, Chrome156.0.8078.80,
DPR1, viewport height900 CSS px. Không có login production.

| Check | Kết quả / boundary |
| --- | --- |
| `cargo test --locked -p vink-ui` | PASS52 tests: native semantics/name/description, raw Unicode và shared support SSR. |
| `cargo xtask verify quick` | PASS lần cuối: token drift, fmt, SSR/hydrate/preview clippy, type-boundary doctests, unit/contract và repository checks. |
| `cargo xtask verify web` | PASS: SSR/public/private/no-JS, bundle secrecy, hydration, cache/reader return, Desk/editor save/conflict/leave, restart/export và endorsement writes trên scratch DB thật. |
| `cargo xtask ui-preview leptos verify` | PASS774 captures; shared primitives/recipe, tooltip hover/focus/Escape, native details keyboard/no-JS, reflow100/200 và gallery. |
| `node apps/vink-web/e2e/browser-smoke.mjs --base http://127.0.0.1:3052 --chrome <Chrome> --state /tmp/vink90-r5-final/smoke-state.json --phase desk-layout` | PASS: actual AX searchbox names, một settled live result (không mỗi phím), private query/sort/refresh, full-title và matrix320/390/834/1024/1440 × sáng/tối × chữ100/200. |
| Cùng runner, `--phase public-search` sau CSS cuối | PASS: toàn public matrix, native GET/q/Enter/no-JS/clear, CDP composition/caret/NFD/identity, reader return, focus/forced-colors/reduced-motion, finite scope và name-word integrity. |
| Focused reuse của preview runner trên build cuối | PASS13 captures: gọi lại `components` (gồm CDP touch trên details khi scripts disabled và unmount tooltip trước queued frame) và WorkCard cùng byline/title ở320/390/1440 × sáng/tối × chữ100/200. Runner tạm `/tmp/vink90-r5-preview-focused.mjs`, không thêm suite vào repo. |

Full web/preview PASS trước delta CSS cuối cho avatar WorkCard hẹp và guard nhỏ của
tooltip: queued placement bỏ qua owner/node đã dispose khi chuyển trang nhanh.
API/data/state giữ nguyên; focused public matrix kiểm CSS, preview13 kiểm lại cả
CSS và shared tooltip với hồi quy dispose tức thì. Quick cuối kiểm full source.
Desk evidence được reuse vì delta sau đó chỉ thuộc named WorkCard container. Không
tuyên bố đã chạy lại full web/preview trên một source khác mà không ghi boundary.

Một focused run gặp top target `-0.046875` CSS px do native scroll rounding; assertion
trên viewport edge nay dùng cùng tolerance1px với edge dưới. Không đổi layout để làm
xanh test, giữ log FAIL. Source review cũng phát hiện tên tác giả bị bó hẹp trong thẻ
tonal ở320/text200; named container ẩn avatar trang trí ≤18rem, và final matrix kiểm
những từ Việt ngắn không bị bẻ qua dòng. Một preview reuse tạm FAIL đường dẫn tương
đối do chạy ngoài repo; đã pin lại import/source URL, chạy lại PASS13.

## Hình học, màu và ảnh

Before/after cùng fixture12 public và100 drafts; baseline dùng cached build PR93
source-equivalent với develop6c97917 (hash6 owner search khớp bản đã qualify). Giữ
raw contexts, ảnh, log và production source fingerprint trong bundle local
`target/ui-evidence/search-r5-90/manifest.json`:84 capture được nhập,16 đã mở; source
fingerprint `548806246345549cdf2aaba2de1c14419054f49c59ae8c38bbd26cf5f9dc3a81`.
Các artifact có cờ inspected riêng;
không coi tất cả ảnh sinh ra là đã mở. Product screenshots được mở ở390 sáng,
320/chữ200 tối (cả tile byline/title/row), desktop1440 sáng/tối và focus/forced-colors.

| Geometry cùng fixture | Before → after (CSS px) |
| --- | --- |
| Explore390/text100, search + helper | 101.47 → 78.19 |
| Explore320/text200, search + helper | 348.53 → 163.97 |
| Explore390/text100, riêng shell | 61.09 → 56.00 |
| Explore320/text200, riêng shell | 122.19 → 83.19 |
| Desk320/text200, search + helper | 251.55 → 163.97 |
| Desk390/text100, inventory cũ (chưa kể count thứ hai) → inventory duy nhất | 36.38 → 18.19 |

Không ép mọi nội dung vào viewport khi chữ200. Native input vẫn một dòng, query dài
cuộn theo caret; placeholder có thể hẹp ở320/text200, native accessible name đầy đủ
vẫn có trong AX tree. Full titles/meta/actions có đường cuộn/focus và clearance.

Resolved text contrast tối thiểu: Desk sáng5.386/tối5.934; Explore sáng5.425/tối5.934.
Đã đo riêng placeholder trên field background: sáng6.283, tối10.004. Đây là các cặp
được đo trong Chrome, không là chứng nhận accessibility toàn app hoặc đo từ PNG.
Focus chỉ một perimeter ở input/clear/submit; forced-colors dùng system border/outline.

## Acceptance và phần còn lại

| AC | Evidence |
| --- | --- |
| 10,12,13 | Scope ngắn ngoài shell; một inventory/result; native sort + named refresh; single-link/full title; live status sau700ms ngừng gõ, composition không announce. State/error/retry/last-success có browser regression. |
| 11 | Shared support SSR + preview keyboard/hover/pointer persistence/Escape và native details no-JS/CDP touch. Rail exception và API cho #101/#67 có trong components contract. |
| 14,16 | Same-fixture captures/contexts, số đo trên, nguồn/choices/owner và gaps; source/diff review không thấy blocker còn lại trong scope. |
| 15 | GET/Enter/no-JS/clear và private Desk/reader return; CDP composition/caret/Unicode. Không nâng synthetic input thành OS IME. |
| 17,18,20 | Flat semantic surfaces, gap thay rules; byline/title cùng card; một-line Bar, hidden native names đúng AX, no info/help và conditional clear qua matrix. |
| 19 | Web qualification320–1440, root text100/200, hai theme, actual CSS/contrast/focus/forced-colors và trạng thái lỗi; boundary CDP/headless được giữ rõ. |

AC-04/AC-06 cũ vẫn chưa đủ: OS IME, zoom thật, mobile thật, nhiều browser và screen
reader NOT_RUN; CMP/native NOT_RUN và chưa có Bar/support parity. Cần runtime/thiết bị
tương ứng để nghiệm thu. Không đóng issue từ merge; không kiểm/chờ GitHub CI, không deploy.
