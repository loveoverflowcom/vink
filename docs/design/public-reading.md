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

**Delta capability so với mockup:** không spotlight khi chưa có cấu hình có nguồn; không search,
intent filter, excerpt, genre, câu hỏi, số phản hồi, nút lưu/góp ý hay nhãn tuyển chọn/cá nhân hóa.
Không có trạng thái bộ lọc để khôi phục; reader-return dùng `/`, lịch sử do router hiện có sở hữu.
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
