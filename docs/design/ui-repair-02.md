# UI-REPAIR-02 — Khám phá v4: mật độ và phân cấp

Ngày: 03/11/2026 (UTC+07). Nền: `develop@0e69412`, sau PR #40
(UI-REPAIR-01). Nhánh: `codex/explore-v4-density`. Phạm vi ED-01/UX-01 một phần,
giữ UX-02 hiện có và INV-02/INV-08; chỉ lát cắt A của #14.

## Nguồn và nguyên nhân

- [Current design 02/11](https://drive.google.com/file/d/1cNYQh-iukYP-nP0cphLOGdywROAmtkSr/view),
  [Explore v4](https://drive.google.com/drive/folders/1n8myZWf2NPMEzoC5-rRvLChY0o0_cGKr),
  [amendment](https://drive.google.com/file/d/1PQeLmEbwkNiFquI-Qgb6gaEV3i0OZWSn/view).
- [Ảnh hiện trạng được giao](https://drive.google.com/file/d/14VCwu4wMCgYZXPWwUSnKhOO2uMBOTzWD/view)
  và overview sáng v4 đã mở. Đây là hai loại bằng chứng khác nhau: render app và concept.
- [#14](https://github.com/loveoverflowcom/vink/issues/14) vẫn tách A/B/C;
  [spec đọc công khai](public-reading.md) giữ lịch sử và hợp đồng tìm/return.

`Home → PublicListView → WorkCardView → WorkCard/WorkByline` chỉ trình bày
title/tên tác giả/ngày đăng từ public card. `q` GET → `visible_works` lọc cards đã tải;
`reader_href`/`explore_href` giữ query/anchor. SSR dùng `PublicReads` trong tiến trình,
browser dùng API v1; không có query/API/schema hoặc manuscript fetch mới.

Khoảng cách cũ cộng dồn ở heading, phạm vi, form và byline. `WorkByline` đặt ngày
sau cả khối avatar/tên nên ngày ở thấp hơn avatar; padding/margin tiếp tục đẩy title
xuống. Các vấn đề này thuộc layout/pattern, không phải thiếu dữ liệu nghiệp vụ.

## LayoutSpec r3 — đã trình bày trước patch

```text
#explore
  #explore-heading — H1 Khám phá + lời dẫn hiện có
  #discovery-tools
    #public-search — nhãn + input GET q + Tìm + Xóa tìm kiếm khi có q
      #public-query-help — phạm vi luôn hiện, nối aria-describedby từ input
  .vk-discover-grid
    #community-stream — H2 Tác phẩm công khai
      #public-results — số kết quả trong số đã tải
        .vk-public-list
          .vk-public-row — avatar | tên đậm / ngày UTC; title + Đọc tác phẩm
        #feed-end — điểm dừng hữu hạn
    #context
      #writing-prompt — gợi ý tĩnh Một nơi bạn nhớ + Bắt đầu viết
      .vk-reading-note
```

Heading dùng `Headline`, luồng dùng hàng phân cách, không có surface lớn bao từng
bài. Avatar/tên tiếp tục từ `AuthorByline`; pattern sắp tên và `<time>` vào cùng cột
bên cạnh avatar bằng grid. Initials lấy từ display name; tone trung tính vì provider
chưa cấp author ID. Không suy tên thành profile hoặc khóa danh tính.

Sau lần mở ảnh đầu, spec bổ sung trước patch tiếp: dưới 760px, H1 dùng size/leading/
tracking của `Title`, lời dẫn dùng `Label`. Vẫn tăng theo rem và giữ semantic H1;
giảm phần mở đầu tại 320px/chữ 200% mà không cắt hoặc giấu thông tin.

Search dùng nguyên `TextField`, `Button` và link; help do primitive nối với input.
Grid sắp nhãn → input/Tìm → clear nếu có → phạm vi. Vùng công cụ dưới 18rem thì
Tìm xuống hàng, vì 200% root text cần nhiều chỗ hơn. Không giấu phạm vi trong tooltip.

Luồng/context chia hai cột khi container đủ 56rem; cột phụ 14–18rem, còn lại cho
bài đọc. Khi thiếu chỗ, kể cả desktop chữ lớn, context theo sau luồng trong DOM.
Không sticky/overlay/cuộn riêng mới. Shell, logo selected #34, navigation order,
safe-area, focus ring và dirty guard giữ owner hiện có. Sáng/tối dùng semantic tokens
hiện hành, không đổi giá trị token hay font asset.

0 bài, 1 bài, nhiều bài, no-results, loading và unavailable giữ trạng thái riêng.
Gợi ý viết vẫn tới `/ban-viet`, không tạo nháp/đăng ngay. 404/503/canonical/public
SSR và privacy giữ nguyên hợp đồng. Link đọc có href thật trước JavaScript.

## Đối chiếu từng vùng với v4

| Vùng | Thực hiện | Điều chỉnh có chủ ý / hoãn |
| --- | --- | --- |
| Heading | Cỡ và khoảng cách gọn, giữ lời dẫn | Dùng Literata và font pipeline production hiện có |
| Công cụ | Search GET gọn, label/help rõ, clear bằng link thật | Tìm trong ≤12 cards đã tải; không thêm global search/chips |
| Luồng | Avatar initials, tên đậm, ngày ngay dưới tên, title và link đọc rõ | Hàng editorial thay card lớn; giữ thứ tự provider và endpoint |
| Context | Gợi ý viết nhỏ, chuyển sau luồng khi thiếu chỗ | Copy tĩnh, không tự nhận đề bài cộng đồng đang mở |
| Spotlight/excerpt | Chưa nối | Chưa có owner; không chọn tùy tiện một bài hay tải manuscript |
| Intent/câu hỏi/lưu/nhận xét/count/avatar stack/recent feedback | Hoãn #14 B/C | Cần contracts/quyền/identity/discussion và dữ liệu thật; không render affordance giả |
| Reader/Desk | Dùng chung byline gọn trong Reader; Desk không dùng selector này | Không thay authority, save state hoặc layout editor |

## Kiểm chứng

Runtime local: macOS ARM64, Rust 1.99.1, Node 26.12.0, PostgreSQL 17.11 tạm trên
loopback và Chrome headless 156, DPR 1, viewport cao 900 CSS px. Biến chỉ cho tiến
trình: `VINK_TEST_DATABASE_URL` trỏ cluster tạm của task và `VINK_CHROME` trỏ Chrome
đã cài. Không ghi credential hoặc cấu hình vào repo.

- **PASS** `cargo xtask verify quick`: fmt/clippy SSR/hydrate/preview, unit/contract/
  doctest, token drift và repository/link/asset/boundary guards.
- **PASS** `cargo xtask verify web`, gồm baseline trước sửa và patch cuối:
  server/PostgreSQL thật, public SSR/privacy/404/503/canonical, hydration, GET/query,
  title/author có/không dấu, Back/Forward/reload/return anchor/focus, retry và clear.
  Không fetch thêm list/manuscript khi tìm; 12/3/1 bài không trùng; 0 bài khác lỗi.
  Geometry kiểm input/nút không chồng, input có label và described scope, targets
  ≥44px, ngày căn dưới tên, title đầy đủ và link thật. Tại 320/390px/chữ 200%, link
  đọc và Tab tới gợi ý viết của một bài đều clear bottom bar. Các regression editor
  save/conflict/dirty guard/restart cũ tiếp tục đạt.
- **PASS** `cargo xtask ui-preview leptos verify`: shared pattern CSS qua preview
  thật, 642 captures, các scenario/reflow 100/200% và không có console errors.
  Không đổi API pattern hoặc CMP; không cần build native cho CSS Web.
- **SCREENSHOT_INSPECTED** 168 viewport captures qua 35 sheet có nhãn: Explore
  0/1/3/12 bài/no-results/unavailable × 1440/1024/834/390/320 × light/dark ×
  100/200%; thêm 28 ảnh cuộn byline/read/prompt màn hẹp và 20 ảnh Reader sau đổi
  shared byline. Mở riêng các ảnh trước/sau được dẫn dưới. Sheet chỉ thu nhỏ/ghép để
  review, không sửa PNG gốc hoặc dùng pixel diff để đánh giá mockup có dữ liệu khác.
  Ảnh/context đầy đủ ở `target/verify-web/screenshots`; CI lưu artifact
  `browser-evidence` theo retention 14 ngày hiện có.
- **PASS** contrast text Explore ≥4,5: min light **5,43**, dark **5,93** từ màu resolve;
  các cặp đã đo và source SHA-256 trong [geometry.json](evidence/ui-repair-02/geometry.json).
- **FAIL prerequisite** `cargo xtask doctor` lúc đầu: chưa cấu hình URL runtime/
  migrator và Chrome trong PATH. Đã dùng cluster tạm và Chrome bằng biến tiến trình
  để chạy lane web đạt; không biến doctor ban đầu thành PASS.

### So cùng fixture trước/sau

Đơn vị CSS px; tọa độ title đầu tính từ viewport khi cuộn ở đầu trang. Bài thật
là seed một publication; fixture 12 cards giữ cùng NFD title/tác giả/ngày/thứ tự.

| Kịch bản light | Trước | Sau | Sớm hơn |
| --- | ---: | ---: | ---: |
| 1440 / 100%, một publication thật | 502,9 | 362,8 | 140,1 |
| 390 / 100%, title fixture dài | 660,1 | 437,3 | 222,8 |
| 320 / 200%, title fixture dài | 2075,6 | 1585,5 | 490,1 |

| Kịch bản | Trước | Sau |
| --- | --- | --- |
| 1440 / 100% / light, một bài | [PNG](evidence/ui-repair-02/before-explore-real-1440-100-light.png) | [PNG](evidence/ui-repair-02/after-explore-real-1440-100-light.png) |
| 390 / 100% / light, title dài | [PNG](evidence/ui-repair-02/before-explore-390-100-light.png) | [PNG](evidence/ui-repair-02/after-explore-390-100-light.png) |
| 320 / 200% / light, title dài | [PNG](evidence/ui-repair-02/before-explore-320-200-light.png) | [PNG](evidence/ui-repair-02/after-explore-320-200-light.png) |

Ảnh sau sửa bổ sung: [một bài mobile dark](evidence/ui-repair-02/after-explore-real-390-100-dark.png),
[link đọc dài, 320/200%/dark](evidence/ui-repair-02/after-explore-loaded-read-320-200-dark.png),
[focus gợi ý viết, 320/200%/dark](evidence/ui-repair-02/after-explore-real-prompt-320-200-dark.png).
Title dài và chữ 200% vẫn cần cuộn; không cắt tên/title hoặc hạ text scale để nhét
mọi nội dung vào viewport đầu.

Self-review source/callers và render không tìm thấy regression trong phạm vi.
Selector byline chỉ chạm `WorkByline` của Explore/Reader và gallery; Desk rows,
resume/editor/session không khớp. Phạm vi, thứ tự DOM/CTA, hai theme và context
sau luồng khi thiếu chỗ khớp spec r3. Đây là self-review, không phải approval độc lập.

Giới hạn: root text 100/200% không phải browser zoom. CDP input không chứng minh
OS IME; Chrome headless không chứng minh điện thoại, Safari/Firefox hoặc screen reader.
B/C của #14 và nghiệm thu production tiếp tục chưa hoàn tất.
