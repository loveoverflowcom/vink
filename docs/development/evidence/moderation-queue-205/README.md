# Bằng chứng phân trang queue #205

Implementation: `3a6874ede9b2cc3953e137411000cb2030788e16`.
Base: `origin/develop@6de515d799d60361224f4a29e63909b98bb33979`; fetch lại trước bàn giao không đổi.
Dependency #200 (PR #210) và #202 (PR #212) đều nằm trong base. Không dùng feature branch làm base.

Các lệnh chạy tại `/tmp/vink-205`, trước commit trên `6de515d` + dirty source; **toàn bộ source
runtime/test đó được commit nguyên trạng thành `3a6874e`**, không đổi code sau khi chạy. Commit sau
chỉ bổ sung registry/evidence. `captures.json` giữ Git SHA gốc của harness, không viết lại provenance.
Môi trường Mac arm64, Rust 1.99.1, PostgreSQL 17.11, Chrome 157 headless, Node 22.15.
LocalFixture là principal test, không chứng minh đăng nhập browser thật.

## Lệnh và kết quả final

Mọi lệnh dùng `CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0`. Web dùng `VINK_CHROME` trỏ Chrome
Mac và `npm_config_cache=/tmp/vink205-npm`. Test PG riêng: loopback 55505; `DATABASE_URL` dành cho
focused SQLx, `VINK_TEST_DATABASE_URL` dành cho xtask, không dùng database dev.

| Lệnh | Kết quả | Boundary / artifact local |
| --- | --- | --- |
| `cargo xtask verify quick` | PASS toàn lane | `/tmp/vink205-quick-final3.log`: fmt, clippy SSR/hydrate/preview, doctest, unit/contract, repo links/decisions/boundaries |
| `cargo xtask verify postgres` | PASS toàn lane | `/tmp/vink205-postgres-final3.log`: App 192 PASS / 3 ignored; Identity 48 PASS; shared store 6 PASS; provisioning, runtime fences, auth, worker, backup/restore và cleanup scratch |
| `cargo test --locked -p vink-app --test postgres moderation_queue -- --nocapture` | PASS 3/3 focused | `/tmp/vink205-plans-final.log`, không gọi focused run là full lane |
| `cargo test --locked -p vink-app --lib moderation_queue` | PASS 3/3 focused; cũng được chạy trong quick | Cursor, strict query, provider envelope/legacy Link fixture |
| `cargo xtask verify web --scenario review` | PASS selected scenario | `/tmp/vink205-web-final.log`; `target/verify-web/1794791323300816000-11297/`; real API/PostgreSQL flow + transport fixtures có nhãn cho races/lỗi |
| `cargo xtask verify web` | FAIL baseline caret 390×420; later steps NOT_RUN | `/tmp/vink205-full-web-final.log`; [failure excerpt](full-web-failure.txt), run `1794791370268619000-10496`; không suy từ selected review thành full Web PASS |
| OS IME, screen reader, native host, production | NOT_RUN | CDP `Input.insertText`/key events không phải OS IME; không deploy |
| Quint / Kani | NOT_RUN | Không đổi pure governance decisions hoặc model; phân loại theo vink-behavior: thay cơ chế đọc và recovery, kiểm ở SQL/browser boundary |
| GitHub CI | NOT_RUN theo yêu cầu | Không đọc/chờ CI |

Full Web dừng tại `timed out waiting for reduced mobile viewport keeps the active caret visible
without fixed controls`: caret top -118,71875 / bottom -93,71875 ở height 420, scrollY 1865,
không có console error. Đây khớp baseline người dùng đã báo; source editor/bridge không đổi trong
#205. Không bỏ assertion, không rerun để chọn kết quả xanh và không nhận baseline mới. Các scenario
sau main trong full run NOT_RUN; selected review đã chạy độc lập và PASS ở cùng source.

## AC → evidence

| AC | Evidence / boundary | Kết quả |
| --- | --- | --- |
| AC-01 | `domain::moderation_queue`, provider/Web fixtures; SQLx corpus ties, traversal đủ tập; HTTP default 50 / max 100, legacy array + Link | PASS |
| AC-02 | SQLx đối chiếu `ModerationDirectory::resolve`; scoped/group/duplicate/expired/global/admin/recusal/suspended/private; eligibility trước LIMIT+1, không total | PASS |
| AC-03 | Rời nhóm, expiry, revoke grant, bỏ ghế giữa trang; fingerprint đổi dù 200; detail/command fences trong full PostgreSQL, browser clear rows/caps | PASS |
| AC-04 | Processed anchor không cản continuation; claimed case vẫn thuộc queue; target private bị loại; real review claim/conclude/appeal và lost-authority/version tests; refresh bỏ tail cũ, không replay mutation | PASS |
| AC-05 | Strict unknown/duplicate/malformed query, cursor sai actor/kind/limit/version; 422 validation với field; provider fixture parity, legacy Link, Web restart không giả empty | PASS |
| AC-06 | `ReadOwner`/`PageChain` unit tests + browser held append → refresh → late completion; session generation A→unknown→A, invalid cursor, partial grant loss, 403, busy settle | PASS trong boundary unit + browser transport fixtures |
| AC-07 | Real API no-store success/error, SSR không chứa snapshot; UI không persist queue/caps; online hint có backoff không mutation; reason giữ đúng DOM/text cả hai queue, admin denial gỡ controls | PASS trong boundary đã chạy |
| AC-08 | Corpus SQLx 6.000 works/cases/appeals; [raw plans](query-plans.json) cho sparse/global/deep; bounded rows và không N+1 từ App; migration index + upgrade/backout | PASS local PG |
| AC-09 | Browser continuation/error/permission recovery; 390/1280 × root text100/200, Enter/focus; bốn ảnh đã mở, inventory bên dưới | PASS bounded Web; OS IME / screen reader NOT_RUN |

## Lựa chọn và số đo

CHOICE-01 A: envelope `{items,next_cursor,authority_key}` trên `/page`; route cũ giữ array hữu hạn
và Link. Client cũ parse được body, phải theo Link để đọc hết; Web mới cần server `/page` trước
khi phục vụ bundle. Không fallback 404/lỗi sang empty. CHOICE-02 B: actor-specific directory trong
cùng read-only repeatable-read snapshot, pure oracle tạo phạm vi SQL trước limit. Fingerprint là
hash phạm vi hiệu lực, không chứa danh sách work/total và không phải quyền có thể tái sử dụng.

| Plan (6.000 targets) | Execution ms | Rows tại Limit | Shared hit blocks |
| --- | ---: | ---: | ---: |
| Work global trước index | 3.966 | 51 | 521 |
| Work global final | 0.083 | 51 | 310 |
| Work global deep final | 0.085 | 51 | 310 |
| Work sparse final | 0.022 | 1 | 9 |
| Appeal first final | 0.040 | 51 | 157 |
| Appeal deep final | 0.046 | 51 | 157 |

Trước index global phải sort tập hàng mở. Partial index `(created_at,id)` mới loại sort đó;
sparse grants dùng index work/revision có sẵn, appeals dùng `work_appeals_open`. Số đo local/hot
cache trên corpus tổng hợp, không SLA hay benchmark production. Query còn phụ thuộc selectivity
và số grants của actor. Không claim query cost hằng số trên mọi corpus/quyền.

## UX self-review

[LayoutSpec](../../../design/moderation-queue-pagination.md); `NO_APPROVED_REFERENCE`.
Hai vòng sửa có render; vòng cuối ẩn status rỗng. Bốn PNG bên dưới được mở trực tiếp, không chỉ
xem metadata. `text200` là root font-size 200%, không phải browser zoom/OS font scale.

| Chiều | Kết quả | Evidence và giới hạn |
| --- | --- | --- |
| Functional | PASS | Real review flow + fixture pages/races/lỗi/denial |
| Geometry | PASS | Control rects trong 390/1280 CSS px; không cuộn ngang ở text100/200 |
| Visual | PASS trong LayoutSpec; fidelity NO_APPROVED_REFERENCE | PNG đã mở, Quiet Lavender/shared controls, không overlap nội dung; không tự nhận golden/style anchor |
| Accessibility / Responsive | PASS tên control, narrow/200%, Enter, focus recovery | Chưa chạy screen reader; CTA dưới fold được cuộn tới và click trên cả hai queue |
| Interaction | PASS browser programmatic | Held response, refresh supersede append, session invalidation, reason DOM/text retained; OS IME NOT_RUN |

[390 text100](moderation-queues-390-100.png) · [390 text200](moderation-queues-390-200.png) ·
[1280 text100](moderation-queues-1280-100.png) · [1280 text200](moderation-queues-1280-200.png) ·
[capture metadata](captures.json).

## Inventory và bảo toàn công việc

Comment queue đã có paging và không đổi. Author appeal history `/works/{work}/appeals`, revision /
decision history và chi tiết notes chưa được paging trong #205. Không private durable cache,
offline moderation, provider AI thật, native UI hoặc đổi luật governance. `ADM-01@r1` vẫn
PRODUCT_CONFIRMED; `O-18@r1` vẫn OPEN. Snapshot registry:
`cef5662c19d3db1219bab14d197b5413f203aa816c99eaa11e8f2c31688091c2`.

Cleanup trước build: xóa khoảng 13,4 GiB logical Cargo cache `vink-204/target/debug` và
`wasm32-unknown-unknown` sau khi xác nhận không có cargo/rustc đang dùng; giữ site/evidence,
source #204 chưa commit, dirty probe `vink-209-review-final`, mọi branch và database cũ.
Prune chỉ metadata #192/#195 trỏ thư mục không tồn tại. Dung lượng trước 5,5 GiB, ngay sau dọn
khoảng 16 GiB; sau final builds còn khoảng 9,7 GiB. Không xóa dữ liệu không thể khôi phục.

Không merge, deploy, đóng issue hay thay checklist/labels. Review source/diff là self-review.
