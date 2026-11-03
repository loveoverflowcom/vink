# Drive protocol cho Vink UI evidence

## Đích đã kiểm tra ngày 03/11/2026

| Vai trò | URL/ID |
| --- | --- |
| CURRENT DESIGN root | [1miGZyG1oW7Dj2c9BUIekCuIJpXz_n3tR](https://drive.google.com/drive/folders/1miGZyG1oW7Dj2c9BUIekCuIJpXz_n3tR) |
| Current manifest | [00_START_HERE](https://drive.google.com/file/d/1cNYQh-iukYP-nP0cphLOGdywROAmtkSr/view) |
| Explore/Reader reference | [01_CURRENT — Khám phá v4](https://drive.google.com/drive/folders/1n8myZWf2NPMEzoC5-rRvLChY0o0_cGKr) |
| Desk/editor/Me reference | [02_CURRENT — Bàn viết](https://drive.google.com/drive/folders/1uMb10V-oRIbvZ7dBUwAfIEQ8v3qpG9WH) |
| Implementation root (đích ghi) | [03_IMPLEMENTATION — UI Captures · Web + CMP](https://drive.google.com/drive/folders/1spa8XFvX26LnJapd8nK7yJhtEHeCtE-D) |
| Runs (đích tạo run) | [1eCOyci9XObF-OmhIr_047RGLuHlbvOF2](https://drive.google.com/drive/folders/1eCOyci9XObF-OmhIr_047RGLuHlbvOF2) |
| Draft đầu vào, chỉ đọc | [00_CAPTURE_PROTOCOL_SUDOLANG.md](https://drive.google.com/file/d/1WZjF1854xqcxvsT9ejSf6mQid53yXGad/view) |

Readback lúc tạo skill xác nhận runs có parent implementation root và root xuất
hiện dưới CURRENT DESIGN; chưa chạy capture/upload hoặc xác nhận toàn bộ ảnh
reference. Tên/phiên bản này là locator có ngày, không phải baseline vĩnh viễn.
Mỗi run đọc lại current manifest, amendment và đúng asset; phân xử phạm vi theo
chỉ dẫn user hiện tại và nguồn confirmed tương ứng. Không auto chọn phiên bản
lớn nhất, không áp v4/v5 cho toàn app. Nếu không truy cập được hoặc parent sai,
báo gap, giữ local; không tự tạo root thay thế hoặc ghi vào My Drive/root.

Chỉ tạo/cập nhật trong implementation root đã được giao. Không sửa, rename,
move hay ghi đè current manifest, design folders/assets, draft protocol hoặc
`99_ARCHIVE`; không thay sharing/permission. Không upload skill/source repo hàng
loạt khi user chỉ giao UI evidence.

## Công cụ

Ưu tiên connected Google Drive; tìm capability theo tên/description rồi đọc schema
hiện có. Với plugin Google Drive của phiên tạo skill:

| Việc | Capability / shape đã thấy |
| --- | --- |
| Ground file/parent | `get_file_metadata(fileId, fields)`; đọc id/name/mimeType/parents/trashed/size/webViewLink và image metadata/checksum nếu tool hỗ trợ |
| Liệt kê | `list_folder(url, top_k)` nhận folder URL; nếu danh sách có thể bị cắt, dùng `search` với parent filter và `page_token` đến hết, không suy “không tồn tại” từ 100 mục đầu |
| Tạo thư mục | `create_folder(name, parent_folder)`; parent ID hoặc URL đúng schema |
| Upload file mới | `upload_file(file_uri=<absolute local path>, file_name, mime_type, parent_folder_id=<raw ID>)`; không truyền folder URL hoặc tên thư mục vào parent_folder_id |
| Cập nhật index cùng ID | `update_file(fileId, file_uri=<absolute local path>, mime_type="text/markdown")`; thay bytes của raw file, không dùng import Google Docs |
| Đọc lại nội dung | `fetch(url)` cho Markdown/JSON; raw image/file dùng raw fetch/download nếu cần, theo MIME metadata |

Schema/response có thể thay đổi, wrapper có thể đổi tên field hoặc bỏ checksum.
Không giả có CLI `drive`, OAuth token, signed upload hay batch API. Nếu connector
không đọc được local path, dùng cơ chế transfer được môi trường hỗ trợ; không
tạo credential hoặc upload URL công khai để lách. Giữ file và báo phần bị chặn.

## Giao dịch thực tế và retry

1. Dùng run_id đã có; tìm run folder trong đúng runs parent. Đọc metadata xác
   minh folder. Một folder khớp thì resume; nhiều folder cùng tên thì dùng ledger
   ID đã xác minh hoặc báo ambiguous, không chọn tùy ý. Chưa có mới tạo và readback.
   Tạo web/previews/cmp/sources/reports theo bundle, lưu ID cha từng thư mục.
2. Ledger local lưu artifact_id/path/hash + folder/file IDs, URL và kết quả từng
   call. Trước retry đọc lại remote object. Bỏ qua chỉ khi xác nhận cùng nội dung
   bằng checksum cùng thuật toán hoặc download so byte/hash; metadata name/size
   trùng đơn thuần chưa đủ. SHA-256 local luôn giữ; nếu remote chỉ có MD5 thì tính
   MD5 local để so, không đổi nghĩa trường local_sha256.
3. Upload ảnh/snapshot bằng MIME đúng; metadata readback xác minh parent, tên,
   type, byte size và kích thước ảnh nếu có. Dùng URL quan sát từ tool/readback,
   không tự dựng link rồi khai đã mở. Nếu tool không cho kiểm nội dung, ghi
   `integrity=metadata-only/unavailable`; vẫn có thể bàn giao với giới hạn này.
4. Timeout sau write là UNCERTAIN. Reconcile bằng ledger + tìm file trong đúng
   parent, kiểm nội dung trước khi upload lại. Tên trùng nhưng bytes khác không
   được ghi đè lịch sử: đổi run/capture ID hoặc ghi conflict để xử lý. Nếu không
   thể xác định kết quả, dừng artifact đó, giữ partial; không tạo liên tiếp bản trùng.
   Retry một lần sau khi đã kiểm và xử lý nguyên nhân tạm thời; lỗi lặp lại thì
   giữ FAILED/UNCERTAIN và tiếp tục artifact độc lập. Không retry vô hạn.
5. Sau ảnh/source, hoàn thiện report/graph và manifest từ ledger; report trỏ link
   đã xác minh, ghi cả item thất bại. Upload manifest sau cùng của run rồi fetch
   readback để kiểm run_id/SHA/record/links và tổng đếm. Final receipt local giữ
   manifest ID/checksum để tránh tự hash vòng; không xoá file phục hồi khi partial.

Upload status không chứng minh người khác có quyền đọc. Giữ sharing hiện tại;
báo giới hạn truy cập nếu cần, không tự public. `VERIFIED` metadata không đồng
nghĩa đã mở ảnh hoặc được design review thông qua.

## Canonical index và run history

Ground chính xác `00_SCREEN_INDEX.md` trong implementation root; đọc file ID,
parent, MIME, modified time và toàn bộ nội dung trước sửa. Chưa có mới upload
một raw Markdown file. Có nhiều file trùng tên thì báo ambiguous và giữ run riêng;
không tạo index thứ ba hoặc xoá file của người khác.

Index gồm metadata/scope/build; latest attempted/latest complete; bảng
`Screen ID | Surface/host | Route/scenario/state | Capture | Reference/version/authority | Review status/gap`;
graph navigation và reference có link thật + adjacency fallback; bảng component/
preview và tokens; coverage, blockers, findings nếu đã review; lịch sử trỏ tất cả
run/manifest/report đã có. Inventory preview không trở thành product navigation.

Read lại modified time/nội dung ngay trước update. Nếu có thay đổi thì đọc bản mới
và hợp nhất history theo run_id; không ghi đè bằng bản cache. Connector hiện không
cam kết compare-and-swap: tránh nhiều writer đồng thời. Nếu thấy xung đột lặp lại,
dừng cập nhật canonical, giữ run upload và báo index pending. Sau update fetch
lại kiểm history và links. Không gọi index update thành công khi readback khác
hoặc thiếu run; không giả bảo đảm atomicity mà tool không có.

LATEST dựa trên thời gian capture/build + scope, không dựa trên thời điểm upload
hay thứ tự hoàn tất retry. Không để run hẹp/partial che gap của run rộng/complete.
Review sau phải pin run_id và variant; link “latest” chỉ giúp khám phá.
