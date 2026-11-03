# UI-REPAIR-03 — Bàn viết: thẻ viết tiếp và danh sách nháp

Ngày: 03/11/2026 (UTC+07). Nền `develop@cda45bc`, sau
[PR #42 — UI-REPAIR-02](https://github.com/loveoverflowcom/vink/pull/42).
Nhánh `codex/desk-resume-density`. Phạm vi trình bày Desk, giữ WR-01–WR-03
hiện có và INV-08; không hoàn tất autosave, auth hay publication.

## Nguồn và nguyên nhân

[Brief thiết kế](https://drive.google.com/drive/folders/1uMb10V-oRIbvZ7dBUwAfIEQ8v3qpG9WH),
[addendum navigation/identity](https://drive.google.com/file/d/1XnilHnCE8n28EeLw8_UdaMVWGjr0lEcB/view)
và [ảnh chữ 200% được giao](https://drive.google.com/file/d/1KZqRHCw-xxqHIQTzD-oNwuvv5JcLaEXo/view)
đã được đối chiếu theo phạm vi Bàn viết. Không mang feedback mẫu vào app.
Ảnh so trước/sau bên dưới là capture mới từ source nền và patch, cùng fixture;
không coi concept hoặc ảnh Drive cũ là build của patch.

`DeskIndex → DraftListView → adapters::draft_summary → DraftResume/DraftRow`.
`GET /drafts` trả nháp được phép truy cập theo cập nhật mới nhất; resume lấy phần
tử đầu tiên. `visible_drafts` chỉ chiếu danh sách đã tải theo query/sort. Adapter
giữ title (tên trống thành “Chưa đặt tên”), version, updated_at và href từ work ID.
Không tải manuscript để dựng dashboard.

`DraftResume` trước sửa dùng H2/Headline, wrap toàn bộ title, link thường và surface
xếp dọc. Margin giữa heading/resume/list cộng thêm chiều cao. Lịch sử UIS-05
(`a2a1fdd`) ghi nhận việc đưa composition vào pattern dùng chung; không tìm thấy
lý do sản phẩm yêu cầu title resume phải là Headline. Nguyên nhân thuộc pattern
và page spacing, không phải dữ liệu/API hay quy tắc chọn nháp.

## LayoutSpec r2 — đã trình bày trước patch tương ứng

```text
#desk
  #heading — H1 Bàn viết + tạo bản thảo hiện có
  #resume
    .vk-draft-resume — container theo bề rộng thực
      Surface primary
        #resume-content
          nhãn Trở lại trang đang viết
          H2 / Title — toàn bộ title từ DraftSummary
          mở/thu gọn tiêu đề khi layout gọn và title dài
          phiên bản + ngày cập nhật thật
        Viết tiếp — Link primary tới cùng work ID
  #drafts
    H2 Bản thảo gần đây
    tìm + sắp xếp + tải lại, tự xuống hàng
    phạm vi đã tải + số kết quả
    các hàng nháp, tên đầy đủ + metadata + mở bản thảo
  #privacy-note — nghĩa riêng tư hiện có
```

Trước → sau: title không giới hạn chiếm thẻ → kiểu Title và tối đa ba dòng ở
layout gọn. Ngưỡng hơn 60 Unicode scalar chỉ quyết định có disclosure, không cắt
chuỗi, chuẩn hóa Unicode hay sửa DTO. Title ngắn luôn hiển thị toàn bộ.
Nút thật có `aria-expanded`; Enter/Space mở nguyên H2 tại chỗ, giữ focus và href.
CSS đọc trực tiếp trạng thái trên nút, tránh một bản sao trạng thái bị cũ sau refresh.

Container từ `38rem` trở lên đặt CTA bên cạnh nội dung, hiện nguyên title và ẩn
disclosure không cần thiết. Ngưỡng rem theo text scale; thiếu chỗ thì trở lại cột
dọc. CTA luôn ở ngoài phần thu gọn và giữ link semantics (history, modifier click).
`Surface`, `Heading`, `Link`, `Button` và semantic tokens giữ owner hiện có; class
directive thêm class, không thay thế class primitive. `Button.expanded` là prop
optional nhỏ để đặt aria lên nút có focus thay vì wrapper chứa ghi chú; nút khác
không nhận `aria-expanded` khi không được truyền.

390/100% cần thấy CTA và công cụ danh sách trong viewport đầu. 320/200% cho cuộn
trang tới mọi nội dung và focus CTA phía trên bottom bar; không thu nhỏ chữ để
nhét toàn trang. Hai theme cùng cấu trúc. Không thêm overlay, cuộn riêng hay state
lưu. Query/sort/refresh/count giữ nguyên; chỉ khoảng cách page và leading row đổi.

## Kiểm chứng và giới hạn

Kịch bản được mở rộng trong `draftDashboard`, không thêm runner: 100 nháp với title
dài NFD, title trùng nhưng work ID riêng, một nháp tên trống/từ Unicode liền, rỗng,
không kết quả, 503/retry giữ query/sort, 401 và 403 riêng. Keyboard Enter/Space/Tab,
AX tree, contrast và geometry được kiểm riêng. Ma trận viewport cao 900 CSS px:
1440/1024/834/390/320 × light/dark × root text 100/200%, DPR 1.

Tạo nháp vẫn một POST; mất ack sau commit thật cần GET đối chiếu, không replay.
GET lỗi không mở khóa, phản hồi muộn không kéo từ route khác về editor. Mở nháp,
sửa title, lưu vào PostgreSQL thật và trở lại phải thấy cùng work và version 2.
Các regression editor/session/dirty guard hiện có tiếp tục là boundary kiểm riêng.

Các lần kiểm trung gian đã giữ FAIL: class/aria gắn sai owner và thuộc tính trạng
thái forwarded cũ sau refresh đều đã sửa. Oracle chiều cao title mở cũng được sửa:
dấu NFD có thể vượt line box vài px khi `overflow: visible`; kiểm bỏ clamp và cho
chữ tràn tự nhiên, thay vì yêu cầu `scrollHeight == clientHeight` không đúng với font.
Không thay kỳ vọng về title, href, version hoặc khả năng truy cập đầy đủ.

Kết quả chạy cuối và ảnh cùng fixture được ghi dưới đây. `AX tree` không phải một
lượt sử dụng screen reader; root text scale không phải browser zoom. NOT_RUN:
thiết bị thật, Safari/Firefox, OS IME, screen reader. Không đổi API, schema, auth,
EditorSession, body editor, privacy hay publication authority.

## So cùng fixture trước/sau

Đơn vị CSS px, cuộn về đầu trang. Title dài NFD, work ID, version và ngày fixture
giống nhau. Khác biệt focus trong ảnh được giữ nguyên; không mask hoặc đổi baseline.

| Kịch bản | Cao thẻ trước → sau | Đầu danh sách trước → sau | Danh sách sớm hơn |
| --- | ---: | ---: | ---: |
| 1440/100%/light | 221,6 → 174,2 | 383,2 → 311,8 | 71,4 |
| 390/100%/light | 403,5 → 294,2 | 623,1 → 489,8 | 133,3 |
| 320/200%/dark | 2681,7 → 873,5 | 3261,0 → 1396,8 | 1864,2 |

| Kịch bản | Trước | Sau |
| --- | --- | --- |
| Desktop 1440/100%/light | [PNG](evidence/ui-repair-03/before-desk-long-1440-100-light.png) | [PNG](evidence/ui-repair-03/after-desk-long-1440-100-light.png) |
| Mobile 390/100%/light | [PNG](evidence/ui-repair-03/before-desk-long-390-100-light.png) | [PNG](evidence/ui-repair-03/after-desk-long-390-100-light.png) |
| Mobile 320/200%/dark | [PNG](evidence/ui-repair-03/before-desk-long-320-200-dark.png) | [PNG](evidence/ui-repair-03/after-desk-long-320-200-dark.png) |

Ảnh bổ sung: [danh sách khi cuộn](evidence/ui-repair-03/after-desk-list-320-200-dark.png),
[tiêu đề mở đầy đủ](evidence/ui-repair-03/after-desk-title-expanded-320-200-dark.png),
[focus Viết tiếp trên bottom bar](evidence/ui-repair-03/after-desk-continue-320-200-dark.png).
Chữ 200% vẫn cần cuộn; option select dài có thể bị giới hạn bởi vùng hiển thị native,
nhãn và giá trị đầy đủ vẫn thuộc control. Không xem một viewport là toàn bộ nội dung.

Đã mở 80 captures trước/sau qua 20 sheet (mỗi sheet: đầu trang/danh sách × 100/200%),
cộng 8 captures cho title mở, focus, rỗng/lỗi/no-results, một nháp trống/từ liền.
Sheet chỉ ghép PNG nguyên kích thước với nhãn, không chỉnh ảnh gốc. Geometry, context,
SHA-256 source và inventory đã mở trong [geometry.json](evidence/ui-repair-03/geometry.json).
Toàn bộ PNG/context của bản cuối ở `target/verify-web/screenshots`; CI lưu
`browser-evidence` theo retention hiện có. Bằng chứng không khẳng định đã kiểm mọi
pixel của cả 100 hàng.

Đối chiếu spec r2: heading/create, resume, tools/list và privacy giữ đúng thứ tự DOM.
Wide có CTA bên cạnh, narrow có disclosure thật; title/metadata không chồng CTA.
Các controls xuống hàng, count/scope luôn hiện. Theme không thay nghĩa hay dữ liệu.
Self-review source/callers và render không còn finding cần sửa trong phạm vi; đây
là self-review, không phải approval độc lập.

Kết quả local trên macOS ARM64, Rust 1.99.1, Node 26.12.0, PostgreSQL 17.11 tạm
trên loopback và Chrome headless 156:

- **PASS** `cargo xtask verify quick`: fmt/clippy SSR/hydrate/preview, unit/contract/
  doctest, token drift và repository/link/asset/boundary guards. Có regression prop
  `Button.expanded` trên đúng phần tử nút và mặc định không có aria disclosure.
- **PASS** `cargo xtask verify web`: server/PostgreSQL/browser thật, ma trận và các
  scenario nêu trên; SSR/privacy, editor save/reload/conflict, guard, restart/export
  cũ cũng đạt. Biến tiến trình chỉ trỏ cluster tạm và Chrome đã cài; không ghi cấu
  hình hoặc credential vào repo.
- **PASS** contrast chữ Desk ≥4,5 từ màu resolve: min light **5,39**, dark **5,93**.
- **NOT_RUN** local lane PostgreSQL riêng: không đổi store/transaction; web dùng
  database thật cho save/create/reconciliation. Kết quả CI được ghi tại PR.
