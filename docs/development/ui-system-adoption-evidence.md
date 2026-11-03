# UIS-05 — adoption màn Vink và composition Web/CMP

**Ngày:** 03/11/2026 (UTC+07) · [Issue #23](https://github.com/loveoverflowcom/vink/issues/23) ·
nhánh `codex/ui-system-screen-adoption`, nền bắt đầu `develop@25e3d10` đã chứa predecessor #33/UIS-04; tích hợp `develop@516a28e`
(#35 chỉ đổi skills/checks, không đổi UI).
Nguồn là diff của PR này; không chấp nhận D-xx hay nghiệm thu toàn bộ WR/UX/COM từ migration UI.

## LayoutSpec r1 và delta v4

Spec đã trình bày trong chat trước sửa UI. Nguồn current:
[index](https://drive.google.com/file/d/1cNYQh-iukYP-nP0cphLOGdywROAmtkSr/view),
[amendment v4](https://drive.google.com/file/d/1PQeLmEbwkNiFquI-Qgb6gaEV3i0OZWSn/view),
ảnh overview sáng v4 đã mở; [public reading](../design/public-reading.md) và
[Bàn viết](../design/README.md#dashboard-bàn-viết-8). Giữ logo Enso #10 của app.

```text
Screen #explore — đọc và tìm trong danh sách
  #explore-heading → #public-search → #public-results (finite stream)
  #context — gợi ý viết; desktop cạnh luồng, màn hẹp sau luồng
Screen #reader — #reader-return → #reader-header (title/byline) → public text
Screen #desk — heading/create → #resume → #draft-tools → #draft-results
Screen #editor — toolbar/status → title → một CodeMirror host
  Overlay — preview hoặc giữ chữ/lưu và rời/bỏ có xác nhận
Screen #me — identity thực có → appearance
```

Delta r1a được trình bày trước patch: giảm padding `DraftResume` từ `Space::S6` sang
`Space::S3` ở cả hai renderer để giữ nguyên các từ “Đường”/“Những” ở 320px/chữ 200%;
không đổi cấp tiêu đề, fixture hay oracle. Rà ảnh r1b khôi phục padding `Space4` và
màu tiêu đề của `DraftRow` trong pattern, bỏ skin input trùng ở app; metadata dùng weight caption.

Light/dark cùng hierarchy, chữ 200%, title/tên Việt dài, thơ giữ khổ/thụt lề. Một cuộn chính;
toolbar editor nằm trong flow trên mobile. Controls wrap; đổi rail/theme không remount editor.
Dialog nhận focus, giữ Tab/Escape, trả focus không cuộn mất vị trí. Save chỉ được xác nhận bởi
`EditorSession`; dialog nhận intent từ `LeaveGuard`. Không thêm guard hay body authority khác.

| Vùng | Trước → sau | Adapter/chủ trạng thái | Bất biến và đường kiểm |
| --- | --- | --- | --- |
| Explore | local avatar/byline/search/button/empty → `WorkCard`/`WorkByline`, `TextField`, `Button`, `StatePanel`, `ListEnd` | `api`, `public_list`, `ui/adapters` | max12, tìm loaded cards, URL/query/anchor, không manuscript N+1; browser `publicReading` |
| Reader | local header → `ReaderHeader`/`AuthorByline`/`Heading`/`Link` | `PublicReads`/publication resource | public snapshot/SSR/meta/404/503, không nháp; SSR + browser |
| Bàn viết | resume/rows → `DraftResume`/`DraftRow`; search/sort/actions/notices → shared primitives | `DraftListView`, `CreateState`, API | max100, newest accessible resume, no double POST/blind retry/late navigation; `draftDashboard` |
| Editor | local buttons/title/notices/dialog chrome → `EditorToolbar`, `Button`, `TextField`, `InlineNotice`, `Dialog` | `EditorSession`, `LeaveGuard`, `BodySource` | một EditorView; latest snapshot/save/version, selection/undo, failure/conflict/export/leave; `mainPhase` |
| Tôi | local avatar/radio/buttons → `Avatar`, `RadioGroup`, `Text`, `Button` | `Appearance`/shell | unknown account truthful, storage fallback, zero POST/PUT; `adaptiveShell` |
| Cộng đồng | chưa có runtime → compositions ở hai preview | fixture adapter, không service | controlled value, Chỉ chia sẻ, pending/auth/uncertain không gửi, không false success |

Primitive → token mapping ở [components](../../packages/ui-system/shared/components.md),
[states](../../packages/ui-system/shared/states.md), [navigation](../../packages/ui-system/shared/navigation.md).
Các module composition và boundaries ở [vink-patterns](../../packages/vink-patterns/README.md).

**Supported:** current public/draft DTOs, timestamps UTC, manual save, appearance. **Fixture-only:**
feedback intent/items/participants, sample request phases, controlled native text. **Deferred:**
spotlight/curation/genre/excerpt/profile/bookmark/feedback backend (#14 B/C), production auth,
native app. Production bỏ vùng thiếu owner; không nhân bản feed, fake count/photo/activity hay login.

## Môi trường và kiểm chứng

macOS arm64; Rust 1.99.1, wasm-bindgen 0.2.129, Node 26.12.0; Chrome headless, DPR 1.
PostgreSQL 17.11, cluster tạm riêng trên loopback 55432; `verify web` tạo/xóa database scratch.
JDK 17.0.19, Gradle 8.14.3, Kotlin 2.2.20, CMP 1.9.0/Material3 1.8.2.
Tokens 0.4.0 không đổi giá trị; catalog 0.5.0 thêm 6 recipe (49 entry, 42 kịch bản màn/góp ý).

| Lệnh đã chạy | Kết quả | Boundary và kết quả chính |
| --- | --- | --- |
| `cargo xtask verify quick` | PASS | fmt, Clippy workspace SSR + Web hydrate/preview CSR wasm32, 175 unit/contract tests + 8 compile-fail doctests, repository/links/boundaries; chạy lại sau tích hợp `516a28e` |
| `VINK_TEST_DATABASE_URL=postgres://vink@127.0.0.1:55432/postgres VINK_CHROME='/Applications/Google Chrome.app/Contents/MacOS/Google Chrome' cargo xtask verify web` | PASS | App/SSR/API thật + PostgreSQL scratch + Chrome, cả main và after-restart; 96 capture context; fixture HTTP chỉ ở các ca list/state/fault được ghi rõ |
| `VINK_CHROME='/Applications/Google Chrome.app/Contents/MacOS/Google Chrome' cargo xtask ui-preview leptos verify` | PASS | 169 link kịch bản chính xác, reflow 320/390/834/1440px × 100/200%, light/dark, controlled feedback/blocked intents, 536 captures; không service |
| `VINK_UI_NATIVE_TARGETS=1 ANDROID_HOME=/Users/manh.pd1/Library/Android/sdk JAVA_HOME=$(/usr/libexec/java_home -v 17) cargo xtask ui-preview cmp verify` | PASS | common/JVM + actual renderer; 344 gallery PNG/context và 72 consumer PNG/context riêng; Android `:ui`/`:patterns` compile, iOS arm64 compile, common tests trên iOS simulator arm64 (gồm `ParticipantsTest` 1/1) |
| `cargo xtask ui-tokens --check`, `git diff --check`, `node --check` hai smoke scripts | PASS | source/generated khớp; không whitespace/lỗi cú pháp JS |
| `cargo xtask verify postgres` | NOT_RUN | Không đổi SQL, query/store/contract; bằng chứng DB ở web chỉ cho luồng app đã chạy |

`ScreenPreviewTest` 2/2 kiểm controlled value + selection qua theme/pending/uncertain và Sharing
không có form. Rust patterns 2 test kiểm escape/no invented profile và Sharing; existing
`EditorSession`/`LeaveGuard` không đổi. Browser xác nhận zero POST/PUT khi dùng utility, list không
N+1 manuscript, current snapshot/version, unknown-create reconciliation, late response, conflict,
preview/export, giữ EditorView/selection/undo, save-and-leave chỉ sau ack. Hồi quy mới phát sự kiện
`close` cũ khi dialog vừa mở lại: ý định hiện tại vẫn còn và lưu/rời thành công. Trả focus giữ cuộn.

Các lần FAIL trong quá trình sửa (đã khắc phục trước kết quả trên): thiếu feature `FocusOptions`
của preview wasm; độ sâu kiểu của composition (đặt giới hạn compiler 256); selector đo contrast
cũ; từ Việt bị bẻ ở resume 320px/chữ 200%; `close` queued hủy leave intent. Một lần preview drift
bị chặn đúng khi catalog đang được sinh; lần cuối check/build/scenario chạy sau generation xong.
Không nới oracle, thay fixture hay bỏ required check để đạt PASS.

### Ảnh và đối chiếu đã mở

[Bảy ảnh giữ trong PR + manifest hash/context](evidence/uis-05/README.md) là render nguyên bản;
phân biệt seed thật, HTTP fixture và CMP sample. Không có baseline ảnh trước patch nên chỉ đối
chiếu bản sau với spec/reference. Cỡ chữ, màu và avatar dùng roles hiện hành của UI system;
không khôi phục V/ink-v2 hoặc copy palette từ prototype. Không thêm spotlight/genre/social sidebar
vì thiếu owner, đây là delta capability có chủ ý so với v4. CMP là sample consumer; chưa có shell
hay router app native production.

Các ảnh đã mở thêm (tên nằm trong `target/verify-web/screenshots/`, ngoài bảy ảnh lưu trong PR):

- `explore-1440-100-light`, `explore-1024-200-dark`, `explore-834-100-dark`,
  `explore-390-200-light`, `explore-320-200-dark`.
- `reader-1024-100-light`, `reader-834-200-light`, `reader-390-100-dark`,
  `reader-1440-200-dark`, `reader-body-320-200-light` (thơ/indent/literal markup).
- `desk-dashboard-320-200-light`, `desk-dashboard-1440-dark` (đã mở lại sau sửa padding hàng),
  `1280-preview-long-title-dark`, `320-me-dark-text200-bottom`, `320-me-dark`.

Preview đã mở: Leptos `screen.explore--default--light--1440` và
`composition.feedback--default--dark--390`; CMP gallery `screen.reader--feedback--light--tablet`;
consumer CMP `screen.explore` 1280dp sáng/100%, `screen.desk` 360dp tối/200%,
`composition.feedback` 834dp sáng/100%, `screen.editor` 360dp tối/200%.
Consumer native: density 1, font scale 1/2, padding 16dp, OS/keyboard inset 0; gallery có chrome
workbench nên host width không bằng width specimen. `screen-contexts.json` ghi consumer width
thật, `contexts.json` ghi gallery host. Không coi mọi ảnh được chụp là đã mở.

Tương phản app đã đo riêng trên DOM resolve: tối thiểu shell 5.79/8.23, desk 5.39/5.93,
Explore 5.43/5.93, reader 6.72/10.83 (sáng/tối), đều ≥4.5. Ảnh 200% cho thấy compact rail có thể
wrap nhãn trong từ ở 96px (style UIS-04 có sẵn); toggle vẫn thấy/hoạt động, không tràn ngang.
Chưa nhận đây là nghiệm thu screen reader hay typography của mọi tổ hợp.

Self-review lần theo base/head, DTO adapters, all callers, dialog close/focus, submit guards và
fixture boundaries; đã sửa các finding trên. Không có reviewer độc lập trong lượt này. Cluster
PostgreSQL tạm đã dừng/xóa sau web PASS; dữ liệu dev có sẵn không bị dùng hay sửa.

## Giới hạn

IME chỉ mô phỏng qua CDP, chưa có OS IME/paste Word/Docs, Safari/Firefox, screen reader, mobile
thật. CMP/JVM render off-screen không chứng minh Android/iOS UI, lifecycle, keyboard/insets hay
native editor parity. Compile/native common tests được ghi riêng. Không thay query/store/contract,
nên PostgreSQL lane riêng không bắt buộc; browser vẫn chạy với PostgreSQL thật. Auth/provider,
Discourse/publication/AI nằm ngoài phạm vi; không có gửi hoặc lưu feedback production.
