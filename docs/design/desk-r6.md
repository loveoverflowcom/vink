# Bàn viết R6 — Viết tiếp và danh sách nháp

Triển khai [#133](https://github.com/loveoverflowcom/vink/issues/133) trên
`develop@e2294a2` sau PR #136. Kế thừa search R5.1 và full-title/single-link của #91.
[Nguồn desktop R1.4](https://drive.google.com/file/d/1-SBgur-9EmDVKQ1CKLm7pyIMPJr_jDF7/view)
và [R6 mobile §1](https://drive.google.com/file/d/1S4xxyEpjtt4r6g9i8G8iJviWR_c_dQ8i/view).
R1.4 chọn Tạo bản thảo dạng chữ trên màn đã có nháp; trạng thái rỗng giữ CTA filled.
Dropdown/chevron dùng chung thuộc #135.

```text
Screen #desk — mở nháp cập nhật gần nhất hoặc tìm nháp đã tải
  Row #heading — H1 Bàn viết (UI); Tạo bản thảo (text, phụ)
  Region #resume — surface-container, bo 28px
    Column — Bản thảo gần nhất; tên Literata; phiên bản · ngày cập nhật
    Row — link Viết tiếp primary; Xem đủ tên/Thu gọn tên nếu cần
  Region #drafts — H2 Bản thảo (UI)
    Group — SearchField Bar; một dòng scope/kết quả
    Row #draft-tools — Sắp xếp; native Select; Tải lại
      Caption — Bỏ qua dấu khi sắp xếp (chỉ Tên A–Z)
    List #draft-results — một surface-container-low; hàng phẳng
      Link — tên đầy đủ; phiên bản/ngày; mũi tên trang trí
  Region — quyền riêng tư và ý nghĩa lưu nháp
```

CHOICE-01 A: link Standalone + recipe primary ở `DraftResume`; không thêm API
primitive. Link có href thật, target ≥48px và focus hệ thống ở forced-colors.
Nút disclosure giữ nguyên heading Unicode đầy đủ trong accessibility tree,
không thay DTO, không thay href. Ở measure ≥38rem, hiện toàn tên và ẩn disclosure.
Ở measure nhỏ, thu tối đa ba dòng; Viết tiếp đứng trước disclosure trong thứ tự Tab.
Hai thao tác cùng hàng khi vừa, tự wrap khi chữ lớn. Không có cuộn con hoặc overlay.

H1 Be Vietnam Pro 28/600, H2 danh sách 16/600; tên dùng role title Literata 22/600,
leading 1.3. Metadata role caption, “Cập nhật” vẫn ở tên truy cập của ngày. Dùng
semantic tokens hiện hành; không đổi palette hoặc renderer CMP.

Search, scope và list cùng trục ngoài. Ở measure ≥48rem, search và tools cùng cụm
hai cột; dưới ngưỡng này, tools theo sau scope. Nhãn sort và Select cùng hàng khi
vừa, wrap ở 320/chữ200. Native help tìm kiếm vẫn làm description nhưng được clip
khỏi phần nhìn để không lặp scope. Live status giữ debounce 700ms và bỏ qua
composition. Dưới cap: “Tìm trong {n} bản thảo đã tải.”; ở cap 100 thêm “gần đây”,
không tự nhận tổng kho nháp. Query: “{m} trong {n} bản thảo … đã tải.”

Resume vẫn lấy mục đầu của danh sách có quyền, độc lập query/sort. Mỗi hàng có
một href theo work ID, không tải manuscript trước và không ghi draft khi mở.
Loading, empty, 401/403, last-success khi 503, pending create, uncertain-create và
GET reconciliation dùng state authority cũ; SSR chỉ chứa shell/loading.

Owners: `pages/desk.rs`, app CSS, `vink-patterns` DraftResume/DraftRow và pattern CSS;
Leptos Desk specimen tiêu thụ cùng pattern. [Bằng chứng local](../development/desk-r6-evidence.md).
