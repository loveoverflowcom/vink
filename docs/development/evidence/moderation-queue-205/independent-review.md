# Review độc lập PR #223 cho #205

Review đầu vào `cd511b8c97753b3676272cfdbf2bbf1cc86c2027`, base đã fetch
`6de515d799d60361224f4a29e63909b98bb33979`. Issue và PR không có comment tại lượt đọc.
Runtime/test sau sửa: `b9a1faaace7f112b18f7aea05b36f53d8f48b356`; commit evidence sau đó chỉ
đổi tài liệu/artifact. Review đầu vào độc lập với tác giả; các sửa đổi dưới đây do reviewer
tự kiểm. Không đọc/chờ GitHub CI, không deploy hoặc chạy lại hàng đợi UI đang tạm dừng.

## Findings và cách sửa

1. **P2 — Refresh giữ nhãn cũ của case cùng ID.** `QueuePanel` giữ link qua `For key=id` nhưng
   children lấy `r.label` một lần. Khi người khác nhận ca, tải lại vẫn hiển thị “Chờ nhận” dù
   response đã là `claimed`. Browser regression thêm trước sửa **FAIL** đúng assertion
   `refresh updates the state label of an existing case ID`, không có console error
   (`/tmp/vink223-web-before.log`). Nay nhãn đọc reactive từ rows hiện tại; browser kiểm cả
   nhãn “Đã có người nhận” lẫn giữ nguyên phần tử link. LayoutSpec/bố cục và form lý do giữ nguyên.
2. **P2 — Generic prepared plan có thể quét cả queue.** Diagnostic trên SQL đầu vào, dùng
   `PREPARE` rồi `SET plan_cache_mode = force_generic_plan`, quét 6.000 hàng và loại 5.900 hàng
   cho trang sâu, 5.999 hàng cho scope thưa. Các EXPLAIN cũ là custom plan nên không phát hiện
   rủi ro này. Không khẳng định production hoặc chế độ `auto` đã chọn generic plan. Bản sửa dùng
   `SET LOCAL plan_cache_mode = force_custom_plan` trong transaction đọc queue, theo mẫu discovery
   hiện có. Quyền và rows vẫn cùng snapshot; không đổi query/governance hoặc cấu hình session.
   Regression gọi mỗi production entrypoint tám lần trên một connection đặt sẵn force-generic,
   kiểm counter custom ≥ 8 / generic = 0 trong `pg_prepared_statements` và setting được trả lại
   sau thành công lẫn từ chối quyền.
3. **Giới hạn rollout đã xác minh.** Web tại base gọi `moderation_get::<Vec<Case/Appeal>>` và
   không đọc Link. Backend mới giữ dạng array nhưng tab cũ chỉ thấy trang đầu 50 mục; đây không
   phải tương thích chức năng đầy đủ. Đã sửa contract/registry cho đúng: server `/page` + migration
   trước bundle mới, rồi tải lại tab kiểm duyệt cũ trước khi tiếp tục duyệt. Nếu cần giữ tab cũ
   hoạt động xuyên rollout thì chưa đủ điều kiện deploy. Regression HTTP mới theo Link tới hết
   cả hai queue, xác minh tập ID, giới hạn và no-store. Caller runtime trong head dùng `/page`;
   các caller array còn lại trong repo là test fixture `review-withdrawn`/`publication-author`.

Không tìm thấy đường vượt quyền trong phạm vi đã kiểm: directory theo actor dùng cùng snapshot
với rows; domain oracle tạo scope; recusal/public-active được lọc trước `LIMIT+1`; lookahead không
lộ total hay mục ngoài quyền. Cursor ràng buộc actor/kind/limit, giữ vị trí khi anchor đã đóng.
ReadOwner/ReadTicket/PageChain, session generation, disposal và fingerprint quyền kiểm trước khi
nhận page. Detail/mutation có fence riêng, không lấy cached row làm thẩm quyền.

## Kiểm chứng của reviewer

Mac arm64, Rust 1.99.1, PostgreSQL 17.11, Chrome 157 headless/CDP, Node 22.15. Cwd `/tmp/vink-205`;
`CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0`. Cluster mới `/tmp/vink223-pg`, loopback 55523;
`DATABASE_URL` cho SQLx focused, `VINK_TEST_DATABASE_URL` cho xtask. Web thêm `VINK_CHROME` trỏ
Chrome Mac và `npm_config_cache=/tmp/vink205-npm`. Không dùng DB dev. LocalFixture không chứng minh
đăng nhập browser thật; queue transport fixtures được tách khỏi real review API flow.

| Command / boundary | Kết quả | Evidence |
| --- | --- | --- |
| `cargo test --locked -p vink-app --test postgres moderation_queue -- --nocapture`, đầu vào | PASS 3/3 | `/tmp/vink223-postgres-initial.log`; real PG + 6.000 targets |
| `cargo xtask verify web --scenario review`, thêm regression trước sửa nhãn | FAIL đúng nhãn cùng ID | `/tmp/vink223-web-before.log`; phần sau điểm lỗi NOT_RUN |
| Cùng command sau sửa nhãn | PASS selected scenario | `/tmp/vink223-web-fixed.log`, run `1794794603668275000-50413`; bốn PNG đã mở |
| SQLx focused sau sửa planning và thêm legacy traversal | PASS 4/4 | `/tmp/vink223-postgres-focused-final.log`; runtime sau đó pin ở `b9a1faa`, trước hai chỉnh test về bound vòng lặp/auto-deref |
| `cargo xtask verify quick` tại `b9a1faa` | PASS toàn lane | `/tmp/vink223-quick-final2.log`; lần đầu dừng Clippy auto-deref trong test, đã sửa, giữ log `quick-final.log` |
| `cargo xtask verify postgres` tại `b9a1faa` | PASS toàn lane | `/tmp/vink223-postgres-final.log`: App 193 PASS / 3 ignored, Identity 48 PASS, shared store 6 PASS, runtime/provisioning/auth/backup và cleanup scratch |
| `cargo xtask verify web --scenario review` tại `b9a1faa` | PASS selected scenario | `/tmp/vink223-web-final.log`, run `1794795521936857000-52349`; không console error |
| Full Web mới, OS IME, screen reader, native, deployed rollout | NOT_RUN trong lượt review | Không suy PASS từ selected scenario |
| Quint/Kani | NOT_RUN | Không đổi pure governance/model; sửa ở SQL/browser boundary |
| GitHub CI | NOT_RUN theo yêu cầu | Không đọc/chờ |

Full Web của tác giả vẫn **FAIL** caret 390×420, các bước sau **NOT_RUN**. Đã đối chiếu với
[baseline/head #222](../../desk-author-feedback.md#kiểm-mới-của-reviewer) và
[#220](../../appearance-220-review.md): cùng caret top `-118.71875`, bottom `-93.71875`, toolbar
static và không console error. #220 runtime `17d6344` nằm trong base hiện tại. Diff #205 không
đổi editor/caret predicate; CSS chỉ thêm selector riêng queue. Đây là evidence lỗi có sẵn,
không chỉ lời báo của tác giả. Không đổi/bỏ assertion hoặc nhận full Web PASS.

[Tóm tắt log và assertions](review-checks.txt) giữ các kết quả cuối cùng cùng failure trước sửa.

## Query plans

[Raw plans + SQL](review-query-plans.json) lưu forced-generic trước sửa và custom sau sửa trên
cùng corpus tổng hợp. Các lệnh EXPLAIN đo SQL production; regression production entrypoint
riêng chứng minh `SET LOCAL` thực sự chi phối prepared statements và được rollback.

| Plan | Trước: forced generic (ms / shared hits) | Sau: custom (ms / shared hits) |
| --- | --- | --- |
| Work sparse (1/6.000) | 0,636 / 200 | 0,022 / 9 |
| Work deep (sau 5.900) | 0,801 / 794 | 0,068 / 310 |
| Appeal deep (sau 5.900) | 0,663 / 450 | 0,033 / 157 |

Custom work global first: 0,066 ms / 310 hits; appeal first: 0,038 ms / 157 hits. Đây là local
hot-cache measurements, không SLA; query cost vẫn phụ thuộc selectivity/grants, không hằng số
cho mọi corpus. Scope actor-specific không thêm N+1 theo từng case.

## UX và phần còn lại

Theo [LayoutSpec hiện hành](../../../design/moderation-queue-pagination.md), sửa nhãn reactive
trong một vòng; không redesign. Bốn ảnh final đã mở: [390/100](review-moderation-queues-390-100.png),
[390/200](review-moderation-queues-390-200.png), [1280/100](review-moderation-queues-1280-100.png),
[1280/200](review-moderation-queues-1280-200.png). Run `1794795521936857000-52349`,
[metadata + hashes](review-captures.json). `text200` là root font-size, không phải OS scaling/zoom.

| Chiều | Kết quả / giới hạn |
| --- | --- |
| Functional | PASS browser nhãn cùng ID, continuation/refresh/restart, real review flow; transport errors/races dùng fixture |
| Geometry | PASS controls trong 390/1280 và text100/200; không cuộn ngang |
| Visual | PASS trong LayoutSpec trên PNG đã mở; `NO_APPROVED_REFERENCE` |
| Accessibility/Responsive | PASS tên control, Enter, focus recovery, narrow/200%; screen reader NOT_RUN |
| Interaction | PASS link identity, unsent Vietnamese reason DOM/text, reverse completion/session/permission fences; OS IME NOT_RUN |

Decision lookup trên checkout giữ `ADM-01@r1 PRODUCT_CONFIRMED`, `O-18@r1 OPEN`; snapshot
`cef5662c19d3db1219bab14d197b5413f203aa816c99eaa11e8f2c31688091c2`. Không nâng architecture
status. Comment queue đã paging; author appeal/revision history vẫn ngoài scope. Giữ #205 mở,
không coi merge là nghiệm thu các boundary chưa chạy hoặc cho phép production rollout.

Worktree chính, dirty #204 và untracked probe #209 giữ nguyên; không xóa cache/evidence/DB cũ.
Khoảng 10 GiB trống trong lượt review. Không khởi động hàng đợi UI tạm dừng.
