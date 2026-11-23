# Bàn viết — read-model tác giả và Thấm/Nhận xét (#214)

Lựa chọn read-model được đưa vào [VDR-AUTHOR-WORK-FEEDBACK@r1](../../decisions/proposals/VDR-AUTHOR-WORK-FEEDBACK.r1.toml), **PENDING**. Thấm theo revision; Nhận xét đếm public thread, không reset theo revision. Không chốt POLICY-01 B/C hoặc O-07.

Một lát dọc `persistence-api` + `web`, sau #213 tại base
`b28e4bdd42eb84c1f45a7ebff4b983cfbc9f9567`. Giữ nguyên API nháp cũ; không thêm tracking,
xếp hạng, native, danh sách người Thấm, kế thừa vote hoặc policy tích lũy.
[LayoutSpec](../design/desk-author-feedback.md) sở hữu bố cục.

## Quyết định và phạm vi

Tra CLI tại checkout: snapshot
`cef5662c19d3db1219bab14d197b5413f203aa816c99eaa11e8f2c31688091c2`.
`LVE-PR-01-CHOICE-02@r1` (`DESIGN_PROPOSED`, import `73ab92f`, payload
`766485bf6d2c6eabc3292921fc478d01a13376b46bed36bee9a00150a6ad6a03`) neo vào
`RevisionId` đang công khai; cùng revision đăng lại giữ số, revision mới có số riêng.
`O-07@r1` (`OPEN`, import `af3c34b`, payload
`321a00230042fb590ffaca83cefdb1360fef91a9a3564223c050d2221e8d5e3c`) chưa chốt
hệ quả qua bản sửa/ranking. PR này áp dụng baseline A của issue, POLICY-01 B/C vẫn OPEN;
không ghi signoff hay nâng architecture thành Accepted. Product/architecture, implementation
và bằng chứng chạy là bốn chiều riêng.
Publication lookup cùng snapshot: `USER-02@r1`, `ADM-06/07/08@r1` đều
`PRODUCT_CONFIRMED` (import `af3c34b`); `O-17@r2` `PRODUCT_CONFIRMED`, từ proposal
`decisions/proposals/O-17.r2.toml` và signoff tương ứng. [Provenance lookup](evidence/desk-author-feedback/decision-provenance.json)
ghim payload/source thật. Consumer giữ audience, lifecycle, intent/moderation và current pointer
riêng; không suy runtime cho các cơ chế chỉ mới có core hoặc proposal.

CHOICE-01 B: schema cho phép `works` không có `working_drafts` (không có FK ngược).
`GET /api/v1/author/works` trả một row/owned WorkId, kể cả không có draft. Tên lấy draft,
hoặc snapshot mới nhất, hoặc chưa đặt tên; thời điểm lấy draft, fallback ngày tạo work.
Không có draft thì không có `draft_version`/link sửa. API `/drafts` cũ giữ semantics/fixtures.
Cùng `DraftSearch` chuẩn hóa query, sort, owner-bound cursor, limit 1..100, `limit+1`;
namespace cursor riêng ngăn dùng nhầm cursor `/drafts` sang corpus tác phẩm và ngược lại;
Web dùng lại `DraftPages`/`ReadOwner`, 24/page, 240/part, không cache/coordinator thứ hai.

CHOICE-02 B: tối đa hai SELECT (aggregate chỉ khi có public anchor) trong cùng transaction `REPEATABLE READ, READ ONLY`:

1. Owner/query/order/keyset → page CTE MATERIALIZED → publication axes, intent moderation,
   latest moderation, current public anchor/title và diff nội dung/metadata; không body ra port.
2. Batch anchors của trang → hai aggregate độc lập (selected work endorsements của đúng
   revision; approved/present comments theo thread). Không fan-out, không loop query theo card.

Metrics nằm trong savepoint: lỗi aggregate trả `tham/comments: null`, vẫn giữ title/state/link
editor. Từ #342, SELECT batch thứ ba đọc Lượt xem theo `WorkId` cho mọi thẻ của trang (không cần
public anchor) trong savepoint riêng; luật và bằng chứng ở
[registry Lượt xem §9](work-views.md#9-đọc-theo-quyền-342). Lỗi toàn page vẫn là lỗi đọc; không biến thành empty/zero. `None` publication khi paired
pointer lệch public projection là unknown. Không lộ manuscript, actor/voter ID hay toàn revisions.
Snapshot chỉ nhất quán trong một response; nhiều trang là live keyset, refresh lấy lại head.

`has_draft_changes`: so title và document JSON thật + ordered `(slot, tag_id)` với metadata
snapshot gắn revision. No-op save/version tăng không tạo marker. Legacy thiếu metadata có
kết quả unknown khi nội dung không khác; không đoán từ việc draft tồn tại hay so version với ID.

Nhận xét đếm root + flat reply `review='approved' AND removal IS NULL`, giống actor-free public
projection; reply dưới root ẩn vẫn hiện nên vẫn đếm. Anchor/placeholder không phải comment công
khai. Không tính own pending/rejected hoặc Hữu ích comment; đóng nhận xét không làm mất count.
Hai aggregate độc lập; loaded page length không phải tổng.

Owner chỉ từ Actor/session; cursor là vị trí, không quyền. Route nằm trong no-store layer cả lỗi.
Browser chỉ tải sau hydration; không serialize author facts trong SSR, không public/IndexedDB
cache. ReadOwner scope/generation chống late reply/ABA, xóa khi 401/403/đổi account/logout.
Same-account retained snapshot báo stale; reconnect không replay create/publish/endorsement.
Tên public khác tên draft có context rõ. [R4 #410](https://github.com/loveoverflowcom/vink/issues/410)
hiển thị mỗi tác phẩm trên thẻ riêng, chuyển các trạng thái thành tối đa ba nhãn có icon
và dùng icon kèm số cho Thấm/Nhận xét. Số chưa tải được có dấu “—” kèm tên chỉ số.
“Đọc” mở bản công khai qua một liên kết bên cạnh liên kết editor.

## Owners và kiểm chứng

- Application/port: `application/author_works.rs`, `ports::AuthorWorkStore`.
- SQL: `persistence/author_works.rs`; `author_page_sql` và `AUTHOR_METRICS_SQL` là SQL production
  được EXPLAIN trực tiếp; `AUTHOR_VIEWS_SQL` (#342) thuộc [Lượt xem](work-views.md#9-đọc-theo-quyền-342). Không thêm migration/index khi chưa có bằng chứng cần.
- HTTP provider: `http/author_works.rs`; fixture `author.works.page.json`.
- Web: `contract/author_works.rs`, `ui::adapters::author_work_summary`, `DeskIndex`,
  DTO-free `AuthorWorkRow`. Enum tương lai giữ Unknown, metric thiếu/âm không thành zero.
- PostgreSQL: `tests/postgres/author_works.rs` (corpus, actual public projection, read race,
  aggregate failure, PostgreSQL server statement log và EXPLAIN small/large).
- Browser: `authorWorks` trong `main`, phase focused `author-works`; paging main và auth
  đổi-account giữ gate ở endpoint thật mới. Fixture/race/geometry được ghi riêng với API thật.

Bằng chứng theo boundary và bảng UX năm chiều được ghi bên dưới; không dùng PASS của dependency thay cho lượt này.


## Đo PostgreSQL trên source triển khai

Source SQL `3e10fdd` (giữ nguyên tại `5787cd1`), test/measurement `5787cd1`, macOS aarch64, PostgreSQL 17.11 Homebrew;
scratch riêng localhost:55514, không database phát triển/production.
Focused `cargo test --locked -p vink-app --test postgres author_work -- --include-ignored --nocapture --test-threads=1`
(với `DATABASE_URL=<scratch>`, `VINK_PG_QUERY_LOG=<server-log>`) **PASS 5/5**.
Đo statement đọc từ log server PostgreSQL, không suy từ vòng lặp Rust: page size 1/24/100
đều 1 SELECT page + 1 SELECT metrics, với số public anchors cũng tăng theo page size. BEGIN/SET/savepoint/commit là control roundtrips cố định.
Measurement được ignore mặc định vì cần superuser local `SET log_statement` + đường log server;
regression metric failure không phụ thuộc logging. Measurement chạy tuần tự trên cluster scratch
riêng/quiescent để các log statement khác không xen giữa hai marker.

[EXPLAIN production SQL và log đếm SELECT](evidence/desk-author-feedback/query-plans.txt):
corpus owner 100 và 10.001, ties cùng timestamp, 10.000 comments dồn trên một public work,
page 24 + sentinel. Recent/Title owner lớn 6,594 / 7,263 ms; nhỏ 0,841 / 0,756 ms;
aggregate một anchor/10.000 comments 4,985 ms. Đây là một lần đo trên máy local, không SLO.
Owner filtering/key sort vẫn có scan/sort cả corpus owner; counts vẫn đọc các selected rows và
public comments thực tế. Không tuyên bố database scan bounded bằng page size hoặc cần cache/index
mới từ một mẫu này. Hai aggregate group riêng, không nhân số theo fan-out.

## Bằng chứng local và giới hạn bàn giao

Runtime/test source `5787cd1c81b3e5430129a9d293f92bc4aaf6571d`, base
`b28e4bdd42eb84c1f45a7ebff4b983cfbc9f9567` (merge #221). Các thay đổi sau mốc source chỉ
bổ sung tài liệu/ảnh bằng chứng. macOS aarch64; Rust 1.99.1; PostgreSQL 17.11; Node 22.15.0;
Chrome 157 headless; Leptos SSR + hydration dựng từ source mỗi lượt. Dùng
`CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0`; DB test riêng
`127.0.0.1:55514`, `VINK_TEST_DATABASE_URL=<scratch>`, Chrome trong `/Applications/Google Chrome.app`.

| Lệnh / boundary | Kết quả và evidence |
| --- | --- |
| `cargo xtask verify quick` | **PASS** toàn lane: lint SSR/WASM/preview, unit/contract, doctest và repository checks; `target/issue214/quick.log` |
| `cargo xtask verify postgres` | **PASS** toàn lane: store 6, App 189 + 3 measurement ignored, Identity 48; provisioning, binaries, worker, native auth, backup/restore và cleanup; `target/issue214/postgres-aggregate.log` |
| Focused PostgreSQL + ignored measurement theo lệnh ở trên | **PASS 5/5**, public anchors 1/24/100; log server và EXPLAIN production SQL; `target/issue214/postgres-measured-final.log` |
| `cargo xtask verify web --scenario main --scenario auth --scenario publication --scenario endorsement` | **FAIL main** tại caret sau resize 390×420, các phase sau **NOT_RUN** trong lượt này. Run `1794768507156240000-84545`, `target/issue214/web-main-final.log` |
| `cargo xtask verify web --scenario auth --scenario publication --scenario endorsement --scenario storage` | **PASS** cả 4 selector, kể cả native Identity/session thật, public IndexedDB và profile reopen. Run `1794768930677419000-1817`, `target/issue214/web-boundaries.log` |

Trong `main` cuối, `authorWorks`, `draftDashboard`, R6, timestamp và `draftPagination` đã PASS
trước lỗi editor. New API khớp Reader revision/count trên PostgreSQL thật; ma trận 16 cấu hình
320/390/834/1440 × light/dark × root text100/200; zero/large/unavailable/unknown, trạng thái bản
mới pending/rejected, public title khác draft, pointer+counts refresh, stable input/link/focus,
retained stale, 403 clear và 0 POST tạo nháp khi refresh đều được kiểm. Corpus HTTP thật 250 tác phẩm duyệt đủ
Recent/Title, lỗi cursor/retry, late query/append và simulated composition/selection vẫn PASS.
Auth chạy riêng đã giữ response 200 thật của A qua đổi phiên sang B: B tiến triển độc lập, không
còn title/row A và response A tới trễ không ghi đè. Storage phase xác minh public store/epoch,
không thay dữ liệu author thành public cache.

Điểm dừng main là `browser-smoke.mjs` tại assertion “reduced mobile viewport keeps the active
caret visible without fixed controls”: caret range y=-118,719..-93,719 CSS px, viewport height
420, toolbar static; không console exception. Đây là cùng lỗi baseline #221 đã được bàn giao;
PR này không sửa source editor. Không suy PASS cho phần sau điểm dừng hoặc full Web lane.
Ảnh/log lỗi vẫn ở run trên (`screenshots/editor-short-mobile-caret-failed.png`). Muốn nghiệm thu
main đầy đủ: xử lý/review lỗi caret ở owner editor rồi chạy lại **nguyên** selector main; không
bỏ assertion. Không mở hoặc khởi động lại hàng đợi UI cũ trong lượt này.

### AC → evidence (không tick checklist GitHub)

| AC | Bằng chứng / phần còn lại |
| --- | --- |
| AC-01 | PG owner corpus 175 + 30 foreign, cả works không draft; exact order/ties hai sort, cursor cross-owner/cross-route; legacy fixture/decoder; browser traversal thật 250 |
| AC-02 | Content/title/ordered tag comparison, no-op version, metadata-only edit; public R1 + pending R2 và private/withdrawn trong PG; enum unknown + pending/rejected UI trong contract/browser; lifecycle từ server, không thêm policy |
| AC-03 | PG same-revision republish fixture giữ số, revision swap bắt đầu số riêng; production endorsement on/off/replay; browser real Reader/Desk cùng revision/count; POLICY-01 B/C vẫn OPEN |
| AC-04 | Independent public HTTP projection + 9 review/presence combinations và reply dưới root ẩn; comments closed; count 10.000 vẫn đúng khi public page hữu hạn; không đếm placeholder/own overlay |
| AC-05 | PostgreSQL server log 1 page + 1 metrics SELECT khi public anchors tăng 1/24/100; production EXPLAIN small/large/skew; one row/work và hai aggregates riêng; browser không GET manuscript theo card |
| AC-06 | Lock-controlled pointer switch giữa page/aggregate trả R1+R1 count, read sau trả R2; nullable metric failure/savepoint; browser thay snapshot pointer/counts một lần và bỏ late replies theo ReadOwner |
| AC-07 | Actor-derived owner, 401/owner/cursor/unknown-param/SSR/no-store trong PG; auth thật A→B/late200/logout; 403 clear + 0 create POST trong browser; public storage selector PASS |
| AC-08 | Private/unknown ẩn số; real zero khác null/—; aggregate failure vẫn giữ draft version/title; public title/context rõ; readonly metrics và sibling links, editor node ổn định |
| AC-09 | PASS các kiểm scoped trên Chrome và ảnh đã mở; **main FAIL baseline caret**, OS IME/screen-reader/physical touch **NOT_RUN**; không tuyên bố nghiệm thu mọi target |
| AC-10 | quick/PG full PASS và browser scoped evidence ở trên; data flow/contracts/decision provenance cập nhật; **chưa full main PASS**, issue giữ mở để review/acceptance riêng |

### Self-review UI theo năm chiều

Không có style anchor được duyệt; `fidelity: NO_APPROVED_REFERENCE`. Tham chiếu lịch sử
Drive đã mở ở đầu lượt, không dùng làm golden. UI không có vòng sửa visual sau lần capture;
các lần chạy lại cập nhật assertion nhãn/owner metadata theo cấu trúc mới, không giảm tiêu chí.

| Chiều | Kết quả / giới hạn |
| --- | --- |
| Functional | **PASS** scoped #214: API/PG, lookup/traversal, state/metrics và recovery; **FAIL** aggregate main ở lỗi editor nêu trên |
| Geometry | **PASS** 16 cấu hình + kiểm R6/search/paging; không overflow ngang; last public-reader target 390/text200 ≥44px và nằm trên bottom nav |
| Visual | **PASS** self-review bốn ảnh source cuối đã mở bên dưới: tên > trạng thái > metrics/scope, Lavender và chữ Việt giữ hierarchy; không pixel-fidelity claim hoặc suy motion từ ảnh |
| Accessibility/Responsive | **PASS** reflow 100/200, full DOM title/AX label, focus, targets; các pair màu Desk được runner đo ≥4,5 (min light 6,28 / dark 9,85, disabled exempt). Chưa full accessibility audit; OS screen-reader/physical touch **NOT_RUN** |
| Interaction | **PASS** search submit/clear, simulated composition, anchor/recovery, refresh giữ input/link node và focus, auth late response và 403 clear; **FAIL** caret editor resize ở aggregate main; OS IME **NOT_RUN** |

Ảnh được chụp trong app Leptos thật với **author state/metrics fixture được ghi rõ**, không
thay bằng chứng DB. [Metadata/hash](evidence/desk-author-feedback/screenshots.json) ghim viewport,
theme, source SHA, thời điểm và review boundary. 200% là root font-size 32px, không browser zoom.

- [1440 light, text100](evidence/desk-author-feedback/author-works-1440-light-100.png)
- [834 dark, text100](evidence/desk-author-feedback/author-works-834-dark-100.png)
- [320 light, text200](evidence/desk-author-feedback/author-works-320-light-200.png)
- [390 dark, text200 — vùng metrics/phạm vi/link đọc](evidence/desk-author-feedback/author-works-metrics-390-dark-200.png)

Logs/captures trước được giữ, kể cả lỗi hết dung lượng và sandbox Unix-socket ở lượt build/check
đầu; chỉ debug artifacts có thể tái tạo được làm mới, không xóa evidence/worktree khác. Không CI
GitHub, merge, deploy, tick/close issue hoặc signoff sản phẩm trong lượt này. Reviewer độc lập
đối chiếu AC với source/evidence, giữ giới hạn main/OS IME và POLICY-01 B/C trước quyết định merge.

## Review độc lập PR #222 — 15/11/2026 UTC

Đầu vào được fetch và kiểm lại: `origin/develop@b28e4bdd42eb84c1f45a7ebff4b983cfbc9f9567`,
PR head `26b81df698433d2b0f81319fee4104dadab4148d`; issue #214 và PR chưa có comments.
Reviewer đọc diff, callers, predicate public discussion/endorsement, publication owner và
registry hiện hành; không lấy PASS của tác giả làm kết quả chạy mới. Registry lookup khớp
snapshot đã ghi ở trên: `O-07@r1` vẫn **OPEN**, anchor LVE vẫn `DESIGN_PROPOSED`.
Không có signoff B/C hoặc thay đổi luật tích lũy.

**F-01 — P3, fixed:** work riêng tư hợp lệ không có working draft/revision bị chiếu thành
“Bản nháp · Không có bản nháp để sửa”. Mapping ở `ui::adapters::author_work_summary` trước
đó chỉ xét generation/latest revision. Commit sửa
`194e986d0a10204db5e0b14b5adc3f82313df71c` đòi có draft thật trước khi dùng nhãn “Bản nháp”;
ca không có draft hiện “Chỉ mình tôi”, không link sửa hoặc metrics. Regression thêm vào
contract test **FAIL trước sửa**, **PASS 2/2 sau sửa**; browser bổ sung ca thứ sáu và assertion
nhãn/không có target giả. Không bỏ hoặc nới assertion cũ. Bản sửa nhỏ do reviewer tự kiểm
(`self-review`); review mã đầu vào là độc lập với chat triển khai.

Không còn finding mới chặn merge trong phạm vi #214. SQL chọn page trước aggregate, owner
filter, snapshot hai SELECT, revision anchor, nullable metrics, public visibility và session
fencing phù hợp baseline A. SQL/App/provider contract không đổi sau các kiểm PostgreSQL dưới
đây; bản sửa reviewer chỉ chạm Web mapping, regression và LayoutSpec. Việc merge không phải
nghiệm thu toàn bộ issue hoặc chốt policy/architecture.

### Kiểm mới của reviewer

Cùng máy macOS aarch64, Rust 1.99.1, PostgreSQL 17.11, Node 22.15.0 và Chrome 157 headless.
Các lệnh Cargo dùng `CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0`.
Cluster local riêng `127.0.0.1:55514`; không database production. Logs mới ở `target/review222/`.

| Lệnh / source / boundary | Kết quả mới |
| --- | --- |
| `cargo xtask verify quick`, đầu vào và mã sửa cuối `194e986` | **PASS** toàn lane cả hai lượt; `quick.log`, `quick-final.log` |
| `DATABASE_URL=<scratch> VINK_PG_QUERY_LOG=<server-log> cargo test --locked -p vink-app --test postgres author_work -- --include-ignored --nocapture --test-threads=1`, `26b81df` | **PASS 5/5** trên PostgreSQL thật; corpus/cursor/privacy, recount, snapshot race, metric failure và statement log; `postgres-focused.log` |
| `cargo test --locked -p vink-web --lib --features ssr author_work` | **FAIL** ca no-draft trước sửa, **PASS 2/2** sau sửa; `no-draft-red.log`, `no-draft-green.log`; full quick cuối cũng chạy regression |
| `cargo xtask verify web --scenario main`, baseline `b28e4bdd` | **FAIL** caret 390×420; run `1794770057833834000-47453`, `baseline-main.log` |
| Cùng nguyên selector `main`, head đầu vào `26b81df`, build lại SSR + WASM đúng checkout | **FAIL** đúng caret trên; run `1794770812943449000-78802`, `head-main-final.log`. #214, Desk/R6, search/paging 250 và composition simulation đã PASS trước điểm dừng |
| `cargo xtask verify web --scenario auth --scenario publication --scenario endorsement --scenario storage`, mã cuối `194e986` | **PASS** cả 4 selector trên App/Identity/PG thật; run `1794771703042773000-13941`, `web-boundaries-final.log` |
| `node apps/vink-web/e2e/browser-smoke.mjs --base http://127.0.0.1:55522 --chrome <Chrome> --state target/review222/author-final/state.json --phase author-works`, mã cuối `194e986` | **PASS**, gồm ca no-draft mới và 16 cấu hình. App/bundle từ build cuối ở trên, seed DB riêng `vink_review222_author`, runtime role hạn chế riêng; `author-final/browser.log`, setup/logs cùng thư mục, orchestration `/tmp/vink-review222-author.py` |

PostgreSQL log mới xác nhận page size/public anchors 1/24/100 đều **1 page SELECT + 1 metrics
SELECT**. EXPLAIN SQL production: owner 10.001 work Recent/Title 6,319 / 6,679 ms; owner nhỏ
0,869 / 0,777 ms; aggregate 10.000 comments 4,862 ms. Đây là số đo local một lượt, không SLO;
query vẫn scan/sort corpus và đọc count rows thật. Toàn lane PostgreSQL không chạy lại trong
phiên review (**NOT_RUN**); 5 focused tests là bằng chứng mới, full-lane PASS phía trên là lịch
sử của chat triển khai. Không có thay đổi SQL/provider giữa mốc PG và bản sửa cuối.

**Lỗi editor có sẵn, chưa sửa:** baseline và head đều có caret top `-118.71875`, bottom
`-93.71875` CSS px ở viewport height 420, toolbar `static`, không console error. Cả hai dừng
ở nguyên assertion “reduced mobile viewport keeps the active caret visible without fixed
controls”; ảnh hai lượt đã mở và lưu trong [metadata review](evidence/desk-author-feedback/review222/screenshots.json).
Không chạy lại toàn `main` sau bản sửa nhãn; final-source UI được kiểm bằng phase #214 và bốn
selector ở trên. Phần sau điểm dừng main vẫn **NOT_RUN**, không nhận full Web PASS. Không sửa
editor hoặc khởi động hàng đợi UI cũ; lệnh tiếp tục nghiệm thu là nguyên selector `main`.

Một lượt head đầu đã **FAIL do lẫn artifact baseline** khi reviewer dùng chung `target` giữa
worktree; không dùng làm bằng chứng source. Lượt clean host tiếp theo được dừng trước browser
vì WASM target chưa được clean. Reviewer gỡ symlink baseline, clean riêng host/WASM packages rồi
dựng lại đúng checkout; logs `head-main.log`, `head-main-rebuilt.log` vẫn giữ. Các lượt hợp lệ
được pin riêng ở bảng trên. Worktree baseline tạm đã dọn; DB, evidence và worktree có sẵn giữ lại.

### Kết quả UI sau sửa

Năm ảnh source cuối đã mở, dữ liệu trạng thái có nhãn fixture trong app Leptos thật; không
thay bằng chứng PostgreSQL. [Metadata/hash](evidence/desk-author-feedback/review222/screenshots.json)
pin `194e986`; text200 là root font-size, không browser zoom. `NO_APPROVED_REFERENCE`.

| Chiều | Kết quả / giới hạn |
| --- | --- |
| Functional | **PASS** scoped #214 trên mã cuối, regression no-draft, Reader/Desk anchor/count và real auth/publication/endorsement/storage. **FAIL** main trước sửa nhãn ở lỗi editor baseline |
| Geometry | **PASS** 320/390/834/1440 × light/dark × text100/200, không tràn ngang; target đọc cuối tới được trên bottom nav |
| Visual | **PASS** scoped, 5 ảnh cuối đã mở: title/status/count/scope rõ, private no-draft không mâu thuẫn. Bản sửa là self-review; không pixel-fidelity/design approval claim |
| Accessibility/Responsive | **PASS** reflow, native AX name/focus và targets trong các kiểm Chrome. OS screen reader, browser zoom và physical touch **NOT_RUN** |
| Interaction | **PASS** refresh giữ input/link/focus, stale/403 clear, readonly, late session reply; composition chỉ mô phỏng. Caret resize **FAIL** trên baseline/head đầu vào; OS IME **NOT_RUN** |

Ảnh cuối: [1440 sáng](evidence/desk-author-feedback/review222/author-works-1440-light-100.png),
[320 text200](evidence/desk-author-feedback/review222/author-works-320-light-200.png),
[834 tối](evidence/desk-author-feedback/review222/author-works-834-dark-100.png),
[metrics 390 text200](evidence/desk-author-feedback/review222/author-works-metrics-390-dark-200.png),
[không có draft](evidence/desk-author-feedback/review222/author-works-no-draft-390-dark-200.png).

Giữ issue #214 mở: AC-09/10 còn giới hạn full main/host nêu trên; POLICY-01 B/C và O-07 vẫn
**OPEN**, không đánh dấu tích lũy đã triển khai. Không tick/close issue, không xem/chờ/poll/rerun
hoặc chủ động kích hoạt GitHub CI, không deploy hoặc xử lý PR #220 trong phiên này.
