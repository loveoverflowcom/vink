# Khám phá và trình đọc công khai — #9, lớp đọc v3

**Ngày:** 01/11/2026 · **Nền:** `develop@c6a8a29` · **Phạm vi:** ED-01/UX-01 một phần;
giữ WR-03, INV-02/INV-08. Không nghiệm thu toàn bộ các yêu cầu hoặc S-01–S-05.

## Nguồn và quyết định triển khai

- [Issue #9](https://github.com/loveoverflowcom/vink/issues/9), gồm
  [cập nhật v3](https://github.com/loveoverflowcom/vink/issues/9#issuecomment-6556317033).
- [Issue #14](https://github.com/loveoverflowcom/vink/issues/14) phân chia A: đọc công khai,
  B: owner/contracts/quyền, C: tương tác thật. Thay đổi này chỉ thuộc A.
- [LayoutSpec v3](https://drive.google.com/file/d/1F2tieZPb2-PsR7Tn3vD0xvIlEwBQf2Ne/view),
  [ảnh sáng](https://drive.google.com/file/d/1zisVyoflOQo-d4sAa9g5gIkTXzCuwgUR/view),
  [reader](https://drive.google.com/file/d/1YpplBQpYfkZGlV_NTe6Mc83AJIectNqT/view).
  Đây là thiết kế có dữ liệu mẫu; không phải bằng chứng Vink runtime.

**HOW:** `/` → `Home`/`WorkCardView` → `/tac-pham/{id}` → `Reader`/`PublicationView`
→ `DocumentView`. SSR dùng `PublicReads` trong tiến trình qua adapter ở
[ssr.rs](../../services/vink-app/src/http/ssr.rs); browser dùng API v1 với DTO của consumer.
`Vink::public_works` giới hạn 12; `PgStore` chỉ join `publications` → revision được trỏ tới,
không join `working_drafts`. Thiếu publication trả 404; lỗi storage trả 503, không đổi thành rỗng.

**WHY có nguồn:** vision yêu cầu hữu ích khi ít nội dung. Cập nhật #9/#14 chọn luồng nhiều bài,
không tiếp tục spotlight đơn lẻ. Decision log tách nháp riêng với snapshot đang công khai.
**Lựa chọn triển khai:** giữ nguyên query/contract và thứ tự ngày đăng, trình bày thành luồng
hữu hạn, có điểm dừng và lời mời viết. Không tải manuscript riêng để bổ sung excerpt cho card.
Không đổi authority, state transition hay publication semantics; không cần thêm behavior model.

**Khoảng trống:** `WorkCard` chỉ có ID/title/byline/date; chưa có owner cho tiêu điểm biên tập,
excerpt, genre, intent, câu hỏi, counts, bookmark hay feedback. Các vùng tương ứng của v3 chưa
thể nối dữ liệu thật. B/C của #14 cần quyết định identity/discussion và hợp đồng do service sở hữu.

## LayoutSpec r1 — đã nêu trước lần sửa UI

Khách tìm bài để đọc; hành động chính là link “Đọc tác phẩm” có tiêu đề bài. Hành động phụ
là “Bắt đầu viết” tới Bàn viết, không tạo nháp hoặc xuất bản ngay.

```text
Screen #explore — /
  Region #explore-heading — Khám phá + lời dẫn v3
  Grid .vk-discover-grid
    Region #community-stream — tác phẩm công khai, theo ngày đăng
      Region #public-scope — tối đa 12 tác phẩm
      List .vk-public-list — tên tác giả, ngày UTC, title và link reader thật
      Region #feed-end — hết danh sách đã tải
    Aside #context
      Region #writing-prompt — “Một nơi bạn nhớ” + Bắt đầu viết
      Note .vk-reading-note — lời nhắc đọc kỹ
Screen #reader — /tac-pham/{id}, thuộc Khám phá
  Link #reader-return — về /
  Article #reader-paper — data-revision từ publication
    Header #reader-header — Bản công khai, title/byline/ngày UTC
    Body .vk-document — đoạn/khổ/dòng/thụt lề, text được escape
```

Desktop ≥1200px chia luồng/context khoảng 2.4:1; dưới đó context xuống sau danh sách,
không biến mất. Giữ breakpoint rail của #7: mở mặc định từ 1100px, gọn ở tablet/editor,
manual preference ghi đè. Đây là delta có chủ ý với mốc 1200px cho rail của prototype v3.
Màn 320/390px giữ một cột, lề của shell; tiêu đề và từ dài wrap, không ellipsis. Reader giới hạn
`--vk-measure` 42rem, không có card cho từng đoạn. Cuộn trang chính, không sticky/overlay mới;
bottom bar và safe-area do shell hiện có sở hữu. Focus theo DOM, ring `--vk-focus`, link ≥44px.

Sáng/tối cùng cấu trúc, dùng token sẵn có: surface-container-low cho danh sách,
tertiary-container/on-tertiary-container cho lời mời viết, primary cho link. Font tự host
Be Vietnam Pro/Literata. Không đổi logo, font asset hay appearance owner.

Loading có `role=status`; rỗng có lời giải thích và đường viết; unavailable có `role=alert`,
link tải lại thật và lối về Khám phá. Reader missing/unpublished là 404; dependency failure là
503. Không có dữ liệu cũ dưới trạng thái lỗi. Link đọc và nội dung metadata có sẵn trong HTML
server, khách không gặp login wall.

**Delta capability r1 so với mockup:** không spotlight khi chưa có cấu hình có nguồn; chưa có search,
intent filter, excerpt, genre, câu hỏi, số phản hồi, nút lưu/góp ý hay nhãn tuyển chọn/cá nhân hóa.
Ở r1 không có trạng thái bộ lọc để khôi phục; reader-return dùng `/`. Phần search/return được
mở rộng ở r2 bên dưới; các khoảng trống metadata và tương tác vẫn giữ nguyên.
Gợi ý viết là copy tĩnh do trang sở hữu, không tự nhận là một thử thách cộng đồng đang mở.

## Đường kiểm chứng

[Feature map](../development/feature-map.md) dẫn các assertions; [development guide](../development/README.md)
sở hữu lệnh và giới hạn lane. Các fixture nhiều tác giả/title dài/markup/đoạn dài chỉ được trả
bởi CDP trong browser test và đều có nhãn mẫu; không nhập chúng vào seed hoặc dữ liệu sản phẩm.

Kết quả chạy và giới hạn được bổ sung sau khi kiểm tra diff cuối; không suy ra PASS từ source
hoặc prototype.

## Bằng chứng local và review — 01/11/2026

[PR #16](https://github.com/loveoverflowcom/vink/pull/16), base/merge-base `c6a8a29`;
UI, Rust và CSS tại head ban đầu `4fea625`. Lượt tiếp theo chỉ sửa browser smoke và tài liệu.
macOS ARM64, Rust 1.99.1, Node 22.15.0, Chrome 156.0.8078.76 headless, DPR 1;
PostgreSQL 17.11 thật, UTF-8/locale C, loopback, database tạm của lane. Build và PostgreSQL
chạy trên ổ RAM tạm do ổ hệ thống thiếu chỗ; không dùng CI hoặc dữ liệu production.

- `PASS` — `cargo xtask verify quick`: fmt, clippy SSR/WASM, 54 unit/contract tests và
  repository checks; phần Rust/CSS không đổi sau lượt này.
- `PASS` — `cargo xtask verify postgres`: 12 tests với migrations thật; API/SSR sau save riêng
  vẫn trả đúng revision đã công khai, không lộ draft hoặc revision chưa được trỏ tới; rỗng 200
  tách khỏi outage 503. Config rejection và worker SIGTERM cùng qua.
- `PASS` — `cargo xtask verify web`, cả main và after-restart: public HTML/body/title/description/
  canonical trước JS; sửa nháp thật không đổi public DTO/SSR; missing/unpublished/invalid ID 404;
  hydration, link đọc/quay về, loading/rỗng/unavailable, reader 404/503; không N+1 ở danh sách.
  Giữ cả editor/save/conflict/leave/export/restart assertions và không có console errors.
- `PASS` — 40 tổ hợp Explore/reader × 1440/1024/834/390/320px × light/dark × root text 100/200%
  không tràn ngang; khổ/dòng/thụt lề giữ qua DOM, markup manuscript được render thành text.
  Tab đi từ link đọc sang action viết, rồi Shift+Tab trở lại với focus-visible.
- `SCREENSHOT_INSPECTED` — đã mở mẫu seed thật (một bài) ở Explore/reader 1440 sáng và 390 tối;
  mẫu nhiều bài/title dài ở 1440, 1024, 834, 390, 320; reader 320/chữ 200% và thân thơ 320/200%;
  empty/unavailable 320. Các ảnh mở là mẫu đại diện, không phải đã mở toàn bộ 40 tổ hợp.
  PNG và viewport/theme/font/scroll/user agent nằm trong `target/verify-web/screenshots/contexts.json`.
- Contrast từ `getComputedStyle`: 11 cặp Explore tối thiểu **6.28:1 sáng / 7.47:1 tối**;
  5 cặp reader **6.72:1 / 10.83:1**. JSON ở `target/verify-web/{explore,reader}-contrast-{light,dark}.json`.
  Chưa đo mọi hover/disabled hoặc mọi glyph của manuscript.

Đối chiếu vùng: heading/luồng hữu hạn/reader/đường về khớp spec r1. Bỏ spotlight, bộ lọc và
vùng tương tác là delta capability đã ghi trước code, không phải đã nghiệm thu B/C của #14.
Context xuống sau danh sách ở tablet/mobile; không chừa cột rỗng. Không thấy tràn/cắt dấu ở
các ảnh đã mở; độ tràn toàn trang còn có assertions riêng.

**Review:** đọc diff base/head, owner/caller của `PublicReads`, API, publication query,
`DocumentView`, shell và regression tests. Không tìm thấy lỗi production mới cần sửa trong
phạm vi này; không đổi authority, query, transport hay document schema. Đã sửa các giả định
có sẵn của smoke khi chạy trên macOS: giữ headless target visible/focused qua CDP (tab cập nhật
Chrome có thể giành active); chọn Cmd/Ctrl và phím đi cuối tài liệu theo platform; đợi focus
sau animation frame ở xác nhận bỏ chữ; đối chiếu tên tệp tương đương NFC trên HFS+, vẫn so
nguyên văn nội dung file. DevTools request có deadline 30s và download có sự kiện trạng thái;
không thay outcome mong đợi hoặc bỏ assertion để làm xanh. Đây là tự review cùng tác vụ,
không phải một GitHub approval độc lập.

Các lần chuẩn bị đầu bị lỗi dung lượng, cache npm có quyền sai và `RUST_LOG=warn` làm runner
không thấy worker ready; đã dùng build/cache tạm và filter log local rồi chạy lại. Các lỗi đầu
không được tính là PASS. Không thay cấu hình credential toàn máy; upload PR dùng GitHub connector
vì tài khoản Git CLI hiện tại không có quyền push repo này.

`NOT_RUN`: CI (không chờ theo yêu cầu), Safari/Firefox, screen reader có tên, browser zoom thật,
điện thoại/tablet và IME hệ điều hành thật. CDP viewport/root text/focus/key/composition không thay
các bằng chứng đó. Chưa có production auth, author publish/withdraw, moderation, curation,
full public search, bookmark, feedback hoặc Discourse; giữ quyết định D/S và yêu cầu ở trạng thái
đã ghi. B/C của #14 cần owner/contracts và authority thật trước khi nối tương tác.

## LayoutSpec r2 — tìm trong danh sách và giữ ngữ cảnh (#14, lát cắt A)

**Nền:** `develop@75ccb41c4bc4158c1897438e0316de8591fcc191`. Phạm vi UX-02 một phần,
giữ ED-01/UX-01 và INV-08. Spec rút gọn đã trình bày trong work chat trước sửa UI.

```text
Region #community-stream — giữ luồng đọc hiện có
  Region #public-scope — tiêu đề/tên tác giả, tối đa 12 tác phẩm mới công khai đã tải;
                         nói rõ không tìm toàn bộ thư viện
  Form #public-search — GET /, query q, có thể dùng trước JavaScript
    Field #public-query — nhãn “Tìm trong danh sách”
    Actions — Tìm; Xóa tìm kiếm khi URL có truy vấn
  Region #public-results — số khớp / số đã tải; list hoặc no-results
    Row #work-{id} — byline/title gốc; link reader mang q
Region #reader — cấu trúc văn bản và canonical hiện có
  Link #reader-return — /?q=…#work-{id}, chỉ đích nội bộ được dựng từ dữ liệu
```

Trước → sau: danh sách hiển thị tất cả và đường về cố định → form GET lọc sau khi bấm
“Tìm”, URL giữ chữ người dùng đã nhập, các kết quả giữ thứ tự provider. Không thêm sort,
intent filter, tags hoặc endpoint tìm kiếm. `search.rs` sở hữu comparison key dùng chung
với dashboard: NFC/NFD, có/không dấu, Đ/đ, hoa/thường và whitespace; chữ gốc không bị sửa.
`public_list.rs` sở hữu lọc title/author và link ngữ cảnh, không đọc nội dung manuscript.

Home/reader vẫn SSR async qua `PublicReads`; canonical luôn bỏ ngữ cảnh q/hash. Truy vấn
chỉ điều khiển projection của các public cards đã được phép đọc, không mở quyền đọc mới.
Khi click link thường, Home đánh dấu hàng ở history entry hiện tại, giữ nguyên history state;
Back/Forward và link quay về có thể khôi phục query/hash. Sau khi danh sách async đã mount,
vùng khớp hash được cuộn tới và focus về link đọc. Modifier/new-tab không sửa history trang cũ;
animation frame trễ kiểm lại route/hash trước khi chạm DOM. Không nhận `return_to` tùy ý,
không dùng localStorage/sessionStorage hay lưu chữ riêng tư cho ngữ cảnh này.

Màn hẹp và chữ 200% cho form/actions wrap; input co về `min-width: 0`, không đặt chiều cao
cố định. Giữ cuộn trang chính, không overlay/sticky mới; bottom bar/safe area vẫn do shell sở hữu.
`vk-field`, `vk-input`, nút tonal/text và focus ring dùng token hiện có, cùng thứ tự ở hai theme.
Nhãn liên kết bằng `for`, phạm vi qua `aria-describedby`, số kết quả bằng `role=status`.

Loading vẫn riêng; provider trả danh sách rỗng dùng empty kể cả khi có q; danh sách không rỗng
nhưng không khớp dùng no-results; lỗi phụ thuộc vẫn 503/alert, không thành “không tìm thấy”.
Link thử lại reload URL hiện tại nên giữ q. Reader 404/503 cũng giữ đích Khám phá nội bộ;
không render lại nội dung thành công cũ dưới lỗi. Không đổi provider/query/schema/auth.

**Đường kiểm:** `public_list::tests` kiểm matching/order/chữ gốc và encoding/đích nội bộ;
test dashboard cũ kiểm hồi quy helper; HTTP outage test kiểm 503 với q và canonical.
`publicReading`/`serverHtml` trong browser smoke có assertions cho SSR title/author/no-results,
hydrated GET, reload, Back/Forward, focus về hàng, retry giữ q và không fetch thêm manuscript.
Ma trận viewport/theme/root text và editor regression vẫn nằm trong lane web hiện có.
Các assertions mới không tự được coi là đã chạy; kết quả local của r2 ghi riêng khi kết thúc.

**Giới hạn:** đây là một phần A; A toàn phần chưa nghiệm thu lại, B/C vẫn pending.
UX-02 còn cần tìm toàn thư viện/tags và access/withdrawal theo hợp đồng thật; D-05/S-03,
D-08/S-02 cùng metadata/intent/bookmark/feedback không được giải quyết bởi thay đổi này.

### Kiểm tra local r2 — 01/11/2026

macOS ARM64, Rust 1.99.1, Node 22.15.0. `PASS`: `cargo xtask verify quick` (fmt,
clippy SSR/WASM, 56 unit/contract tests, repository checks); `node --check` cho browser smoke
và `git diff --check`. Bộ test dashboard cũ và hai test public-list mới đều qua. HTTP outage
test dùng router thật với database không kết nối được, xác nhận query được escape, canonical
không có query và trạng thái vẫn 503; đây không phải bằng chứng transaction PostgreSQL.

Ổ hệ thống gần đầy: các lượt đầu lỗi link/strip do thiếu đĩa; sau khi dọn cache build riêng
của task, chạy quick từ cache sạch với biến **chỉ cho tiến trình**: `CARGO_PROFILE_DEV_DEBUG=0`,
`CARGO_PROFILE_TEST_DEBUG=0`, `CARGO_PROFILE_DEV_STRIP=none`, `CARGO_PROFILE_TEST_STRIP=none`,
`CARGO_INCREMENTAL=0`, `CARGO_BUILD_JOBS=1`. Không sửa profile trong repo hay cấu hình host.
Các lần lỗi đầu không được tính là PASS; kết quả quick ở trên là lượt hoàn tất sau cùng.

`verify web`: **FAIL prerequisite**, `DATABASE_URL` chưa có. Doctor còn xác nhận thiếu
wasm-bindgen CLI 0.2.129; Chrome hiện có tại `/Applications/Google Chrome.app/Contents/MacOS/Google Chrome`.
Không cài service/package, không dùng VM/database Cantos. Full-app browser, hydration thực,
Back/Forward/reload/focus, ma trận 320–1440px/chữ 200%/hai theme, ảnh chụp/contrast mới và
editor regression: **NOT_RUN**. Các assertions tương ứng đã thêm vào smoke hiện có, chưa
được tính thành bằng chứng runtime. Cần PostgreSQL Vink độc lập có CREATEDB + `DATABASE_URL`
và CLI đúng pin để chạy lane web. CI không chờ theo yêu cầu; không bypass required checks.

Self-review diff và callers không phát hiện lỗi mới cần sửa trong phạm vi này; không có
GitHub approval độc lập. Đối chiếu vùng LayoutSpec r2 ở mức source: search trước kết quả,
phạm vi/các trạng thái rõ, URL chỉ trỏ về nội bộ; đối chiếu render và tương tác còn chưa chạy.
Không dùng bằng chứng runtime của r1 để nghiệm thu r2. A toàn phần và B/C #14 vẫn pending.
