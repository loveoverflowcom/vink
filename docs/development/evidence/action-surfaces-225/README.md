# #225 — Chuẩn bị spec/tests khi thiếu shared contract

Ngày 16/11/2026, Mac arm64. Branch `codex/action-surfaces-desk-discovery` từ
`origin/develop@69f71f0cb61228dbe5ceea775b3c34c237705922`.
[LayoutSpec r1 và inventory trước/sau dự kiến](../../../design/action-surfaces-desk-discovery-225.md).
Không có delta UI runtime trong lượt này; không nhận hoàn thành #225.

**Cập nhật review/rebase PR #240 (16/11/2026):** trên
`develop@2c55c2678729b4db39fadd9ce1bcfedfb4175770`, #228 đã merge và shared
`ActionSurface`/prop `TooltipIconButton.surface` đã có; glyph lọc/thẻ vẫn thiếu.
Các log, capture, hash và trạng thái dependency bên dưới thuộc lượt chuẩn bị trên `69f71f0`;
không dùng chúng làm kết quả kiểm thử của head sau rebase. Phạm vi vẫn chỉ spec/tests.

## Phụ thuộc và source

Đã fetch/pull fast-forward develop mới nhất trước tạo branch. Kiểm trực tiếp `action.rs`,
`support.rs`, `icons.rs` trên develop: chưa có `ActionSurface`, prop chuyển tiếp của
`TooltipIconButton` hoặc glyph lọc/thẻ phù hợp trong registry 23 glyph. PR #228 draft/open,
head `64d732aef1b0be6a5f72271eebdfe80a2bc636c0`; chưa merge/cherry-pick. Không suy contract vắng
chỉ vì PR mở. Issue #224/#225 đều chưa có comments khi đọc.

Phần được giao khi dependency thiếu là **spec/tests độc lập**. CHOICE-01 A (Filled + Add tạo)
và CHOICE-02 A (Refresh Tonal opt-in) còn dự kiến, cần paired capture khi có contract thật.
Không nâng một lựa chọn agent thành quyết định sản phẩm/ADR Accepted.

Owned hunks: hai browser test owner hiện có; LayoutSpec; mục liên kết trong discovery registry;
evidence này. Không sửa primitive/CSS/page, Editor/#217/#218, Dialog/Appearance, metrics, backend,
schema, cache hoặc auth. #229/draft #232 không là base. Không khởi chạy #226.

## An toàn workspace

- Ban đầu repo chính sạch trên `codex/long-document-budget@13ff80e1d0dff554037c75c5cfd763baa63068f9`.
  Branch và mọi branch cũ được giữ nguyên.
- Worktree `../vink-204` ở `ef7da9d1981ef9c0864af8d9c26a8c5b6d919aa3` có edits/untracked;
  không stash/reset/switch/clean worktree đó. Snapshot diff binary và untracked inventory đã lưu
  tại `/tmp/vink-225-preserved/`, ngoài Cargo target.
- Đọc tiến trình trên Mac trước clean: không Cargo/rustc/App/preview build đang dùng target.
  Chrome probe `/tmp/vink-sw-probe-204` và PostgreSQL công việc khác được giữ nguyên.
- `target` ban đầu 7,2 GB. Chuyển nguyên `target/ui-preview` (khoảng 2,3 GB, gồm ảnh/log/artifacts)
  và `target/verify-web` sang `/tmp/vink-225-preserved/`; rồi `cargo clean`: removed 8512 files,
  4.9 GiB. Không dọn source, node_modules, caches chung, database hay hàng đợi UI cũ.
- PostgreSQL test mới: `/tmp/vink-225-pg/data`, loopback 55445. Chỉ cluster này nhận scratch của
  lane Web; không kết nối/reset DB công việc khác. Initdb trong sandbox bị chặn shared memory;
  retry local ngoài sandbox thành công. Không thay config/dependency dự án.
- Sau kiểm thử: server fixture PID 96978/cổng 55446 đã dừng bằng SIGTERM, cluster #225 đã dừng
  bằng `pg_ctl -D /tmp/vink-225-pg/data -m fast stop`; data/log vẫn giữ. Binary diff #204 và
  untracked inventory trước/sau khớp nguyên vẹn; remote develop/#229/#224 đọc lại vẫn đúng SHA trên.

## Lệnh và kết quả

Các lượt Rust dùng `CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0`
để vừa dung lượng Mac; không bỏ assertion. Browser dùng Chrome thật với CDP, CSS viewport,
text scale qua root font-size; không phải browser zoom, thiết bị native hoặc OS IME thật.

| Lượt | Kết quả quan sát | Giới hạn |
| --- | --- | --- |
| `node --check` hai script; `git diff --check` | PASS | Cú pháp/whitespace, không là browser PASS |
| `cargo xtask verify quick`, sandbox | FAIL | Unix socket bị OS chặn trong 9 identity tests; [log](quick.log) |
| quick local lần đầu | FAIL | Các test PASS, repository links trỏ README chưa tạo; [log](quick-local.log) |
| `cargo xtask verify quick`, local cuối | PASS | [Log cuối](quick-final.log), đầy đủ các bước repository/unit/contract/clippy |
| `cargo xtask verify web`, chưa chỉ Chrome | FAIL | Prerequisite không tìm thấy Chrome Mac; browser NOT_RUN; [log](web.log) |
| Web local lần đầu với Chrome/PG riêng | FAIL | Test mới chưa đợi catalog sau reopen; [log](web-local.log). Đã sửa harness |
| Phase `public-search` riêng | PASS | [Log](public-search-final.log); browser + API mẫu cục bộ, gồm no-JS. Bổ sung guard không vacuous của max4 được kiểm lại trong full lane cuối |
| `cargo xtask verify web --scenario discovery` | PASS | [Log](discovery.log); corpus SQL fixture, real pagination/reader return + transport race |
| `cargo xtask verify web`, local toàn bộ, source cuối | PASS | [Log cuối](web-final.log), tất cả bước trong verification summary PASS, exit 0 |

Không đọc/chờ/poll/rerun GitHub Actions, status hoặc logs CI.
Log Web cuối trong commit chỉ bỏ trailing whitespace ở 11 dòng để `git diff --check` sạch;
raw nguyên byte giữ tại `/tmp/vink-225-preserved/current-raw-logs/`, cả hai hash có trong provenance.

Các lượt thử harness giữ nguyên ở `public-search.log`, `public-search-r2.log`,
`public-search-r3.log`, `public-search-r4.log`: lần lượt chờ catalog thiếu, bộ đếm submit
trong Window / listener bubble không quan sát được Form, helper pointer chờ rAF khi JS bị tắt.
Đây là **FAIL của test mới trong quá trình phát triển**, không gán thành lỗi sản phẩm có sẵn.
Sửa bằng chờ checkbox draft hiện diện, binding CDP + capture listener, pointer đồng bộ cho no-JS;
không bỏ hoặc nới assertion một submit, max4, native GET. Không sửa UI qua các lượt này.

Lệnh phase riêng (sau build + migrate/seed/catalog qua CLI trên `vink_225_review` ở cluster #225):

```sh
node apps/vink-web/e2e/browser-smoke.mjs \
  --base http://127.0.0.1:55446 \
  --chrome '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome' \
  --state /tmp/vink-225-public-search-final/state.json --phase public-search
```

`VINK_TEST_DATABASE_URL=postgres://manhblue@127.0.0.1:55445/postgres`,
`VINK_CHROME=/Applications/Google Chrome.app/Contents/MacOS/Google Chrome` cho hai lệnh xtask Web.
URL trên chỉ là cluster throwaway mới tạo, trust loopback; không credential môi trường khác.

## Tests chuẩn bị và acceptance

Tests thêm vào phase hiện hữu, không tạo runner/phase song song:

- `browser-smoke.mjs::draftDashboard`: một native create + Add, label trong target, Select/Refresh
  đúng loại ở ma trận đang có; Enter/Space khi Pending không tạo POST thứ hai. Kế thừa các ca
  loading/first-use/query miss/error/stale/rejected/unknown/reconcile/pagination của runner.
- `discovery-smoke.mjs::discoveryReading` (phase `public-search` / `main` / `desk-layout`):
  target launcher >=48, đầy đủ capture 320/390/834/1440 × light/dark × text100/200;
  native GET/submit/checkbox, Space đổi một lần, max4, draft chưa đổi URL, quay về tập applied
  trở về Tonal, Enter một submit, dismiss/focus, no-JS tag Apply giữ q/reset cursor.
- `discoveryCorpus` (`verify web --scenario discovery`): Back/Forward giữ q/tag/cursor,
  set equality không phụ thuộc thứ tự chọn, Apply mới reset cursor, reader return/reload hiện có.

Source cuối có **51 assertions gắn #225 PASS trong phase main**, thêm **3 assertions gắn #225
PASS trong discovery-corpus** cùng các `waitFor` history/cursor. Main và discovery-corpus cuối
đều PASS. Hash của hai test files và UI source được pin trong `provenance.json`; không sửa test
sau lượt cuối. Đây là kết quả baseline với tests mới, chưa phải acceptance của UI delta.

| AC | Phần đã chuẩn bị | Kết quả / gap |
| --- | --- | --- |
| AC-01 | Spec create và first-use, kiểm một target/Add | Chưa đổi Text thành Filled; presentation NOT_IMPLEMENTED |
| AC-02 | Spec glyph/shared seam, test Apply và checkbox | Launcher chưa có glyph; presentation NOT_IMPLEMENTED |
| AC-03 | Spec grouping, test native controls và links | Refresh Tonal chờ #224; không fork CSS |
| AC-04 | Tests native pending keys + owner guard cases | PASS phần Desk đã chạy trong full lane; không đổi create state |
| AC-05 | Tests GET/no-JS/max4/history/cursor + regression hiện hữu | PASS public-search/discovery; OS IME thật NOT_RUN |
| AC-06 | Bổ sung capture matrix hiện hành | Ma trận UI sau triển khai NOT_RUN |
| AC-07 | Historical pixels đã mở, local logs, self-review | Before/after delta UI NOT_RUN vì chưa triển khai |
| AC-08 | Diff chỉ docs/tests | Source review; không sửa issue state/checklist |

## Năm chiều và blockers

| Chiều | Kết quả của delta UI #225 | Boundary |
| --- | --- | --- |
| Functional | UI delta NOT_RUN; baseline PASS theo checks nêu trên | Không nhận full lane PASS nếu có bước sau FAIL |
| Geometry | UI delta NOT_RUN; baseline target/reflow PASS | Native create/launcher >=48, label trong target; matrix tests hiện hữu |
| Visual | UI delta NOT_RUN; baseline đã mở | 9 current + 3 historical captures; CTA còn Text, launcher còn thiếu glyph đúng finding |
| Accessibility/Responsive | UI delta NOT_RUN; baseline checks PASS | 320/390/834/1440, light/dark, text100/200; tên/role/focus/contrast hiện có; screen reader NOT_RUN |
| Interaction | UI delta NOT_RUN; baseline checks PASS | Pointer native, Space checkbox, Enter submit, no-JS, history/cursor; OS IME/VoiceOver NOT_RUN |

[Provenance và SHA-256 source/ảnh](provenance.json) ghi UI source unchanged, dirty inputs chỉ
docs/tests và ảnh nào thực sự đã mở. 9 current captures trong PR đều từ lượt full Web cuối
`target/verify-web/1794808027834936000-2944`: Discovery ở 1440/light/100, 834/light/100,
390/dark/200, 320/light/200; Desk list 1440/light/100, 834/dark/100, 320/dark/200; Desk header
R6 1440/light/100 và 320/dark/200. List capture đã cuộn tới tools, còn R6 header capture cho thấy
CTA tạo. Không trộn fixture 24 tác phẩm với fixture R6 ba tác phẩm thành cặp before/after.
Desk dùng transport fixture `DraftGate`, không nhận số liệu đó là persistence thật.
Header/search/launcher còn đọc được, wrap có chủ đích; Text + Add và glyph thiếu vẫn đúng finding.
Không nhận đây là after/golden. Raw captures/contexts còn ở `/tmp/vink-225-public-search*`
và `target/verify-web/<run-id>/`; chỉ ảnh được chọn có bản sao trong PR.

Self-review: diff chỉ tests/docs, assertion bảo vệ hành vi quan sát được; không thêm default
primitive, không sửa fields/focus/row geometry hoặc dựng API giả. Hai theme và text200 đã mở
pixels; kiểm geometry không được dùng thay kết luận hierarchy của UI chưa triển khai.

Blocker tại lượt chuẩn bị: shared surface + wrapper/glyph chưa trên develop. Sau rebase, chỉ
glyph còn thiếu; shared surface/wrapper đã có theo ghi chú đầu trang. Khi tiếp tục: đối chiếu API mới,
triển khai đúng hunk consumer, thêm assertion background/icon, chụp/mở paired before/after cùng
fixture/state/theme/scale, chạy quick và Web. Không tự merge dependency hoặc tự đóng issue.
