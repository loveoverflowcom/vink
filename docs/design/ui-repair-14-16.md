# UI-REPAIR-14/15/16 — Khám phá, nhịp Reader và hàng bản thảo

Ngày 04/11/2026 (UTC+07). Nền và review snapshot:
`develop@27927c462244f2a70e36a686a37c8c95690775b6`. Theo yêu cầu của owner, ba prompt
(nhánh đề xuất `codex/ui-explore-discovery`, `codex/ui-reader-reading-rhythm`,
`codex/ui-draft-row-density`) được gộp vào một nhánh/PR `codex/ui-repair-14-16`.
Phạm vi ED-01/UX-01/UX-03 một phần, lát cắt A của
[#14](https://github.com/loveoverflowcom/vink/issues/14); giữ INV-02/INV-08, WR-01/WR-03.

## Nguồn đã mở

Đã tải và mở toàn bộ 16 ảnh được dẫn trong prompt:

| Surface | Design | Implementation lịch sử (`585ab7ed` hoặc trước) |
| --- | --- | --- |
| Khám phá | [desktop 1440](https://drive.google.com/file/d/1x2uKcTb5nj7qBTpfh4pR1QlCBw1kpLEb/view), [mobile](https://drive.google.com/file/d/1kEjk2Sb3lV1IdUhS_joP_BJmahKTL8eR/view), [No Results](https://drive.google.com/file/d/1BRkGzYAURwzSY95-O9BklHhPA5LI8YBy/view) | [1440](https://drive.google.com/file/d/14VCwu4wMCgYZXPWwUSnKhOO2uMBOTzWD/view), [390](https://drive.google.com/file/d/18nJLx-1yOqOzb3iT3U2sxEy3MaPwenYQ/view), [320/200%](https://drive.google.com/file/d/1fE8eetSVnr-bvaUX7BIiKEl6I1_D4ntQ/view) |
| Reader | [desktop](https://drive.google.com/file/d/1f71JwbsnVef1XXoSsmrGMbufvLqGI1ej/view), [mobile](https://drive.google.com/file/d/1kFN4SziGbm8yTO2fenvA1ahrerN6kdeG/view) | [1440](https://drive.google.com/file/d/1wtV_ME7S8EykTI4GXvwooyhUr0pzKNCc/view), [390](https://drive.google.com/file/d/1yPGkA8AuiCpi-UVRlTNcJmdWzGAG_CMs/view), [body 320/200%](https://drive.google.com/file/d/17Mz8Eo4WQ64iPRCBjpFgZZHjjzNsDKGy/view) |
| Bàn viết | [desktop](https://drive.google.com/file/d/1E8qM2tCN_vOnhR98KdcvtWNfQOtyefXh/view), [mobile](https://drive.google.com/file/d/1gn-8XS7j9hMpK9c6T13Df0_eU4T-HCb8/view) | [1440](https://drive.google.com/file/d/1Py3TcTFFIicLnSeaMLmAFj8h3_GYI7RN/view), [390](https://drive.google.com/file/d/1cLHvNr2-jxMlUAE9EHyOMhad6oRicFhp/view), [320/200%](https://drive.google.com/file/d/1KZqRHCw-xxqHIQTzD-oNwuvv5JcLaEXo/view) |

Ảnh implementation là baseline lịch sử nên không dùng để kết luận lỗi. HEAD được chụp lại
bằng CDP (Chrome 156 headless, DPR 1) trên server thật và PostgreSQL 17 tạm, cùng fixture cho
trước/sau: seed `[Mẫu] Mưa trên phố cũ` (thơ, dòng thụt lề), hai publication văn xuôi chỉ có
trong database chụp (`[Mẫu] Chuyện nhỏ ở ngõ chiều mưa` dùng đúng các đoạn văn của Design
Reader, và một title dài), cùng năm nháp tạo qua API (title dài, title rỗng, title ngắn).
Fixture này không vào seed hoặc repo. Ma trận: 320/390/834/1440 × light/dark × root text
100/200%: 96 tổ hợp trước (Explore, no-results, Reader thơ/văn xuôi, Bàn viết, danh sách) và
144 tổ hợp sau (thêm hàng nháp, no-results nháp, sort A–Z), 0px tràn ngang ở mọi tổ hợp (đo
bằng `getBoundingClientRect().right` so với `clientWidth`); xem
[before-report](evidence/ui-repair-14-16/before-report.json) và
[after-report](evidence/ui-repair-14-16/after-report.json).

## Finding trên HEAD và nguyên nhân

PR #44 đã sửa resume title; PR #40/UI-REPAIR-01/02 đã sửa byline, mật độ luồng, H1 reader hẹp.
Không sửa lại các phần đó. Finding còn lại sau khi chụp HEAD:

- **E1 CONFIRMED — phân cấp heading Explore.** H1 dùng `Headline` (1.75rem) trên rộng và
  `Title` dưới 760px, nên trên mobile “Khám phá” cùng cỡ H2 “Tác phẩm công khai”; trên desktop
  ô tìm kiếm cao 50px nặng hơn tiêu đề trang. Design và Bàn viết dùng tiêu đề trang Display.
- **E2 CONFIRMED — scope/result rời rạc.** Số kết quả “x trong y tác phẩm đã tải” là một dòng
  riêng dưới H2; Design đặt số đếm ở cuối hàng header của luồng.
- **E3 CONFIRMED — no-results ngược phân cấp.** `StatePanel` dùng H2 mặc định cỡ `Headline`,
  lớn hơn H2 của luồng chứa nó và ngang H1. Trên desktop panel còn bị kéo cao theo cột context.
- **R1 CONFIRMED — header Reader sát nhau.** Title → byline → thân bài gần như không có khoảng
  (0–8px); Design tách caption, title, byline rồi thân bài thành các nhịp rõ.
- **R2 CONFIRMED — return link.** Mũi tên “←” lấy font fallback nên nhỏ và dính vào chữ.
- **D1 CONFIRMED — DraftRow khó quét.** Mỗi hàng ba dòng xếp chồng (title, metadata,
  “Mở bản thảo →”), lặp link chữ ở mọi hàng; Design dùng hàng mark | title + metadata | mở.
- **REJECTED/không đổi:** typography thân Reader (Literata, `--vk-text-reading`, leading 1.85,
  `--vk-measure`, khoảng đoạn 1.4em) đã khớp nhịp Design khi so cùng nội dung; dòng/khổ thơ,
  thụt lề và text chữ được giữ. Không sửa `document.rs`.

## LayoutSpec r1 — đã trình bày trong work chat trước patch

```text
Screen #explore (/)
  #explore-heading — H1 Display ≥760px, Headline <760px; lời dẫn giữ nguyên
  #discovery-tools — form GET q, nhãn/help/Tìm/Xóa tìm kiếm giữ nguyên; tối đa 40rem
  #community-stream — header: H2 | .vk-public-scope (count cuối hàng; xuống dưới H2
                      khi vùng Explore ≤28rem, kể cả chữ 200%); DOM vẫn H2 → count
    #public-results — list hữu hạn + #feed-end, hoặc no-results
      no-results — StatePanel căn giữa, title cỡ Title, giữ #clear-public-results
  #context — prompt tĩnh + reading note, không đổi
Screen #reader (/tac-pham/{id})
  #reader-return — icon arrow-back (registry) + “Khám phá”, gap space-2
  #reader-header — caption → H1 → byline (grid gap); header → body space-8
  .vk-document — không đổi
Screen #desk → #draft-results > li
  DraftRow — mark Edit (aria-hidden) | title + metadata | “Mở bản thảo” + mũi tên
    row ≤30rem: nhãn chỉ còn cho AT, mark/mũi tên neo dòng đầu
    row ≤16rem (320px/200%): bỏ mark, mũi tên xuống sau metadata để title đủ rộng
```

Sau lần mở ảnh đầu, bổ sung trước patch tiếp: stream đặt `align-content: start` (no-results
không giãn theo context) và nhánh ≤16rem của DraftRow vì 320px/200% bẻ “Đường”, “Những”.
Không đổi cấu trúc khác, CTA, thứ tự đọc/focus, query, API, `EditorSession` hay quyền.

## Thay đổi

- [pages/mod.rs](../../apps/vink-web/src/pages/mod.rs): H1 Explore dùng `TypeRole::Display`;
  return link dùng `Icon` `ArrowBack` (decorative) thay ký tự.
- [vink.css](../../apps/vink-web/style/vink.css): header luồng dạng grid H2 | count với container
  query 28rem; no-results scoped; nhịp heading/tools; H1 hẹp dùng token Headline; return link gap.
- [vink-patterns](../../packages/vink-patterns/leptos/src/lib.rs) `DraftRow` và
  [patterns.css](../../packages/vink-patterns/leptos/patterns.css): hàng grid với container
  query theo độ rộng hàng; `ReaderHeader` dạng grid có nhịp. Chỉ token sẵn có
  (`secondary-container`, `primary`, `radius-md`, space); mũi tên tiến là `ArrowBack` lật ngang,
  không thêm icon asset. Gallery Leptos `screen.desk`/`screen.reader` dùng cùng pattern.

Accessible name của link hàng vẫn gồm title đầy đủ, phiên bản, ngày cập nhật và “Mở bản thảo”;
title wrap, không ellipsis; `<time datetime>` giữ timestamp. Mark là một dấu viết cố định,
không mang nghĩa thể loại hay trạng thái.

## Vùng Design thiếu API/contract — không làm

| Vùng trong Design | Lý do | Nơi xử lý |
| --- | --- | --- |
| Tiêu điểm/spotlight, chip lọc Gợi ý đọc/Mới đăng/intent, “Đọc theo mối quan tâm” | `WorkCard` chỉ có id/title/byline/ngày; chưa có curation, intent hay tags | B/C của [#14](https://github.com/loveoverflowcom/vink/issues/14) |
| Ô tìm toàn thư viện trên top bar | Search chỉ lọc ≤12 card đã tải | UX-02, #14 |
| Excerpt, thể loại, “phút đọc”, nút Lưu/bookmark trên card | Chưa có field/contract | #14 |
| Số góp ý, avatar người góp ý, “Từ những cuộc trò chuyện”, “Viết nhận xét” | Chưa có UI discussion production/auth | [#45](https://github.com/loveoverflowcom/vink/issues/45), [#43](https://github.com/loveoverflowcom/vink/issues/43) |
| Reader: “Tác giả mong đợi”, “Một điều còn ở lại”, composer, danh sách cảm nhận, Báo cáo | Author intent/discussion/moderation chưa nối | #45 |
| Reader: “Đọc tiếp một chút”, pull-quote trong thân | Không có related works; document schema v1 không có blockquote | contract riêng |
| Bàn viết: excerpt, số tiếng, thể loại, nhãn Công khai/Đang kiểm tra/Nháp riêng, bộ lọc Đã đăng/Chưa đăng, panel Góp ý, “Chưa lưu trên thiết bị này” | Draft card chỉ có title/version/updated_at; publication/moderation chưa có | WR-01/03, publication và #45 |
| Logo/rail “Vink · VIỆT INK” | Thuộc selected brand | [#34](https://github.com/loveoverflowcom/vink/issues/34) |

## Ảnh trước/sau (cùng fixture/viewport/theme/text)

| Vùng | Trước | Sau |
| --- | --- | --- |
| Explore 1440 sáng | [before](evidence/ui-repair-14-16/before--explore__1440__light__t100.png) | [after](evidence/ui-repair-14-16/after--explore__1440__light__t100.png) |
| Explore 390 sáng | [before](evidence/ui-repair-14-16/before--explore__390__light__t100.png) | [after](evidence/ui-repair-14-16/after--explore__390__light__t100.png) |
| Explore 320 chữ 200% | [before](evidence/ui-repair-14-16/before--explore__320__light__t200.png) | [after](evidence/ui-repair-14-16/after--explore__320__light__t200.png) |
| No-results 1440 sáng | [before](evidence/ui-repair-14-16/before--explore-noresults__1440__light__t100.png) | [after](evidence/ui-repair-14-16/after--explore-noresults__1440__light__t100.png) |
| No-results 390 tối | [before](evidence/ui-repair-14-16/before--explore-noresults__390__dark__t100.png) | [after](evidence/ui-repair-14-16/after--explore-noresults__390__dark__t100.png) |
| Reader văn xuôi 1440 | [before](evidence/ui-repair-14-16/before--reader-prose__1440__light__t100.png) | [after](evidence/ui-repair-14-16/after--reader-prose__1440__light__t100.png) |
| Reader văn xuôi 390 | [before](evidence/ui-repair-14-16/before--reader-prose__390__light__t100.png) | [after](evidence/ui-repair-14-16/after--reader-prose__390__light__t100.png) |
| Reader thơ 390 tối | [before](evidence/ui-repair-14-16/before--reader-poem__390__dark__t100.png) | [after](evidence/ui-repair-14-16/after--reader-poem__390__dark__t100.png) |
| Reader thơ 320 chữ 200% | [before](evidence/ui-repair-14-16/before--reader-poem__320__light__t200.png) | [after](evidence/ui-repair-14-16/after--reader-poem__320__light__t200.png) |
| Bàn viết 1440 toàn trang | [before](evidence/ui-repair-14-16/before--desk__1440__light__t100__full.png) | [after](evidence/ui-repair-14-16/after--desk__1440__light__t100__full.png) |
| Danh sách 390 sáng | [before](evidence/ui-repair-14-16/before--desk-list__390__light__t100.png) | [after](evidence/ui-repair-14-16/after--desk-list__390__light__t100.png) |
| Danh sách 834 tối | [before](evidence/ui-repair-14-16/before--desk-list__834__dark__t100.png) | [after](evidence/ui-repair-14-16/after--desk-list__834__dark__t100.png) |

Chỉ có ảnh sau (kiểm sau sửa, không phải so sánh trước/sau):
[hàng 390](evidence/ui-repair-14-16/after--desk-rows__390__light__t100.png),
[hàng 320/200% tối](evidence/ui-repair-14-16/after--desk-rows__320__dark__t200.png),
[hàng 1440/200% tối](evidence/ui-repair-14-16/after--desk-rows__1440__dark__t200.png),
[no-results nháp 390](evidence/ui-repair-14-16/after--desk-noresults__390__light__t100.png),
[sort A–Z 834 tối](evidence/ui-repair-14-16/after--desk-sort-title__834__dark__t100.png).

## Kiểm chứng local — 04/11/2026

Linux x86_64, Rust 1.99.1, Node 24.22.2, Chrome 156 headless, PostgreSQL 17.11 trong container
riêng của lần kiểm. Không chờ CI theo yêu cầu của owner.

- `PASS` — `cargo xtask verify quick`: tokens, fmt, clippy SSR/WASM/preview, doctests,
  unit + contract tests, repository checks (links, metadata, boundaries, fonts, editor packages).
- `PASS` — `cargo xtask ui-preview leptos verify`: 717 captures, không tràn ngang 320–1440px ở
  chữ 100/200%; đã mở `screen.desk` 390 sáng với DraftRow mới.
- `verify web` trên code cuối: lượt 1 (trước chỉnh icon mark) `PASS`; lượt 2 và 3 `FAIL` ở
  “new stanza typed with an indented line”; lượt 4 (log tạm, không commit) cho thấy đoạn gõ bị
  chèn ở đầu body thay vì cuối; lượt 5 `PASS` cả main và after-restart (SSR, hydration,
  publicReading/no-results, draftDashboard keyboard/sort/no-results, editor, restart, export).
  Trên source base `27927c4` cùng máy: 2/2 `PASS`. Bisect giữ CSS mới nhưng trả markup DraftRow
  cũ: main `PASS`, after-restart `FAIL` ở “unsaved stanza typed after the restart”, một bước nạp
  thẳng `/ban-viet/{id}` không render Explore/Reader/DraftRow. Kết luận có giới hạn: đây là race
  gõ CodeMirror ngay sau focus bằng script (`typeInBody`), không nằm trong code đã sửa; chưa
  chứng minh nguyên nhân và chưa sửa. Không đổi test hay editor để làm xanh.
- `SCREENSHOT_INSPECTED` — đã mở các ảnh trong bảng trên và các ảnh chỉ-sau; contrast của
  lane web: Explore tối thiểu 5.43:1 sáng / 5.93:1 tối, Reader 6.72:1 / 10.83:1, Bàn viết
  5.39:1 / 5.93:1 (cặp thấp nhất thuộc resume, không đổi ở PR này); hàng nháp: title 14.77:1 /
  13.95:1.

`NOT_RUN`: CI, Safari/Firefox, screen reader có tên, browser zoom thật, thiết bị/IME thật.
Root text 200% qua `documentElement.style.fontSize`, không phải zoom. Đây là tự review cùng tác
vụ, không phải approval độc lập.
