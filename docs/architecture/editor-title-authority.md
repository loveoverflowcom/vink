# Tiêu đề theo định dạng — #263

[Issue #263](https://github.com/loveoverflowcom/vink/issues/263) sở hữu extraction/metadata của
title; [#255](https://github.com/loveoverflowcom/vink/issues/255) sở hữu document canonical
`{format, source}` và hash ([kiến trúc](document-source-format.md)). Title authority nối domain → application → PostgreSQL → HTTP API v1. Web Bàn viết soạn cả
hai format (#256) và Reader đọc bản Markdown đã duyệt ([#264](../development/document-source-format.md#2-pipeline-đọc-web)).
Kiến trúc `VDR-DOCUMENT-SOURCE-FORMAT@r2` có trạng thái `ACCEPTED`.

Bàn viết hiện mở và lưu cả hai định dạng qua [Live Preview và bộ chọn](../design/editor-live-preview-256-257.md).
Khi API lưu nháp trả `422` ở trường `title`, Markdown hiện hướng dẫn sửa H1 cạnh vùng Nội dung
và Bàn viết Mặc định hiện lỗi ở ô Tiêu đề. Tác vụ **Sửa tiêu đề** đưa focus về trường cần sửa
và giữ buffer để tác giả tiếp tục. Phản hồi chỉ mô tả đúng lượt sửa đã gửi. Khi tiêu đề
còn lỗi, tác giả vẫn có thể lưu riêng thẻ. Bàn viết cho gửi lại phần chữ sau khi tác giả sửa.

## Quyết định sản phẩm và hiện trạng

Nguồn chốt: issue #263, lời chủ sản phẩm ngày 16/11/2026:
“Default thì cần có tên riêng và body. .md thì không”. Các quy tắc sau là
`PRODUCT_CONFIRMED`; trạng thái kiến trúc và triển khai được ghi riêng.

| Trường hợp | Authority của tên | Hệ quả |
| --- | --- | --- |
| Mặc định mới | Tên riêng | Dấu `#`, Setext hoặc HTML trong body vẫn là chữ. |
| Document v1 cũ | Tên riêng đã lưu | Không sniff Markdown khi mở/lưu. Conversion phải là ý định chủ động của tác giả. |
| Markdown được format contract nhận diện tường minh | H1 đầu tiên trong AST theo thứ tự source | Các H1 sau là nội dung; tên gửi từ client không là authority. |
| Không có H1 hoặc H1 đầu tiên rỗng | Projection rỗng | UI hiển thị “Chưa đặt tên”; không thêm placeholder vào source, metadata hoặc dùng nó để vượt `MissingTitle`. H1 đầu rỗng không được bỏ qua để lấy H1 sau. |
| Xóa H1 đầu | Tính lại từ source mới | H1 đầu còn lại thắng; nếu hết H1 thì projection rỗng. Không sửa source để giữ tên cũ. |
| Tên trích từ H1 đầu không hợp lệ, kể cả khi vượt 200 scalar | Lỗi validation của title | Khi lưu nháp qua API, lỗi 422 ở trường `title` nêu lý do và chỉ tới H1 đầu trong `document.source`. Bản đã lưu và version được giữ nguyên. Việc giữ buffer để tác giả sửa H1 trên Bàn viết thuộc #256. |
| AsciiDoc | `DEFERRED` | Hướng tương lai là document title trong header; không thêm parser hoặc tùy chọn dùng được. |

`Title::parse` trim, NFC, cấm ký tự điều khiển (kể cả tab) và U+2028/U+2029, giới hạn 200 Unicode
scalar values **sau** NFC (không phải byte hay grapheme). `RevisionContent::new` đòi title khác rỗng
cho mọi format: placeholder “Chưa đặt tên” chỉ là UI (`desk_title_key` dùng nó làm khóa sắp xếp như
trước), không vào source, metadata hay vượt `MissingTitle`.

## Triển khai

- **Một chính sách trích:** `domain::document::markdown::first_h1_projection` dùng `pulldown-cmark`
  0.13 với `Options::empty()` — CommonMark 0.31.2 lõi, không extension (không table,
  strikethrough, task list, heading attribute, math, footnote). Duyệt AST theo thứ tự source, kể cả
  blockquote/list; ATX và Setext H1; bỏ code fence/indented code, HTML block (`<h1>`) và `\#`.
  H1 đầu tiên quyết định; H1 đầu rỗng cho projection rỗng (không lấy H1 sau). Flatten text, code
  span, nhãn link, alt ảnh; entity/escape do parser giải mã; bỏ tag inline HTML; soft/hard break và
  tab thành một khoảng trắng; rồi `Title::parse`. Parser dừng ở cuối H1 đầu tiên. Không có regex.
- **Một cặp source/title:** `TitledDocument::author` (Default cần `title`; Markdown từ chối mọi
  `title` client) và `TitledDocument::from_stored` (Markdown tính lại H1 và phải bằng cột `title`).
- **`CHOICE-01` chọn A** — server sinh projection; client không gửi title Markdown. Không cần payload
  tương thích (B) vì không còn phiên bản cũ phải giữ. Parser CodeMirror ở #256 chạy cùng corpus với Rust.
  Corpus #264 có thêm Setext nhiều dòng trong trích dẫn để kiểm phép chiếu title và anchor.
- **`CHOICE-02` chọn A** — cột `working_drafts.title` vật chất hóa trong cùng `UPDATE … WHERE
  version = expected` với `document`, nên danh sách/tìm kiếm (`desk_title_key`,
  `revisions.title_search_key`) đọc đúng phiên bản. Capture snapshot đọc `title` + `document` từ
  cùng dòng nháp `FOR SHARE` và kiểm lại qua `from_stored`; sửa H1 sau đó không đổi snapshot.
- **Corpus:** [`default.json`](../../services/vink-app/tests/fixtures/title-authority/default.json)
  (12 ca, tên riêng thắng `#`/Setext/nested, NFD/emoji, 200/201, CRLF/dòng trống nguyên văn, hash
  golden) và [`markdown.json`](../../services/vink-app/tests/fixtures/title-authority/markdown.json)
  (30 ca, nay là oracle **thực thi** của parser Rust qua unit và application).

## Hiện trạng tích hợp

| Owner/caller | Hiện trạng sau #255/#263 | Bằng chứng |
| --- | --- | --- |
| `domain/document.rs`, `document/markdown.rs`, `revision.rs` | Một shape; title theo format; hash gồm format + title hiệu lực + source. | Unit + corpus 30/12 ca + golden hash. |
| `application/mod.rs::save_draft` | Document validate trước, rồi `TitledDocument::author`; lỗi không chạm store. | Application tests (recording store). |
| `persistence/postgres.rs`, `publication.rs`, `discovery.rs` | Title + document cùng CAS; snapshot title từ source đã capture; đọc kiểm `from_stored` + hash. | PG `canonical_document::*`. |
| `drafts.rs`, `author_works.rs`, discovery search | Đọc cột `title`/khóa sinh sẵn — cùng phiên bản với source. | PG drafts/author works/discovery hiện có. |
| `http/contract.rs`, `contracts/http/v1/` | `title` tùy chọn theo format; response `title` là title hiệu lực. | Provider/consumer fixture tests. |
| Web `pages/desk.rs` | Bàn viết dùng title draft mới nhất. Outline lấy H1 đầu từ buffer hiện tại. | CodeMirror corpus và Web main. |
| Web `pages/mod.rs` | Reader lấy title từ snapshot đã công khai. Nhân sự đọc snapshot qua API được cấp quyền. H1 đầu tiên của Markdown chiếm đúng vị trí nội dung, kể cả Setext hoặc H1 trong trích dẫn. | Corpus và Web review cùng PostgreSQL scratch. |

## Lịch sử

Corpus và quy tắc của lát độc lập (PR #271, #272) tại
[`6287fbf`](https://github.com/loveoverflowcom/vink/blob/6287fbfa2817fd457f3b93e710463a7a9cc7e84f/docs/architecture/editor-title-authority.md)
ghi `legacy-v1.json` và corpus Markdown `DESIGN_PROPOSED`; sau quyết định thay thế 17/11/2026 không
còn document v1 nên corpus legacy được chuyển thành `default.json`. Bằng chứng lịch sử ở
[evidence/editor-title-263](../development/evidence/editor-title-263/README.md).
