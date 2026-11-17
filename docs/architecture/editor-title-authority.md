# Tiêu đề theo định dạng — #263

Phạm vi độc lập tại `develop@9e91534f302b08738492f42364010adc299c8678`,
17/11/2026. [Issue #263](https://github.com/loveoverflowcom/vink/issues/263)
sở hữu extraction/metadata; [#255](https://github.com/loveoverflowcom/vink/issues/255)
sở hữu envelope, format, canonical source và hash. Tài liệu này chuẩn bị corpus và
điểm nối; **không định nghĩa wire envelope hoặc kích hoạt Markdown trên document v1**.

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
| H1 vượt giới hạn tên | Lỗi validation | Giữ source trong editor; không truncate hay chọn H1 ngắn hơn ở phía sau. |
| AsciiDoc | `DEFERRED` | Hướng tương lai là document title trong header; không thêm parser hoặc tùy chọn dùng được. |

`Title::parse` hiện trim/NFC và giới hạn 200 Unicode scalar values sau NFC,
không phải 200 byte hay 200 grapheme. `RevisionContent::new` vẫn đòi tên khác rỗng.
NFC là projection metadata; raw source tương lai phải được #255 bảo toàn.
Document v1 hiện đã NFC hóa các dòng; không hứa phục hồi Unicode/blank lines đã mất.

Tại base trên, backend vẫn `SCHEMA_VERSION = 1`, `DocumentData { blocks }`
`deny_unknown_fields`; `Vink::save_draft` nhận tên riêng và document; không có format
selector trong App hoặc Markdown parser. `ContentHash::of` vẫn dùng domain separator
`vink-revision-content-v1` và length prefixes. `VDR-CONTENT-STORAGE@r1` vẫn `PENDING`.
Các regression của lát này bảo vệ hành vi đang có, không biến proposal thành Accepted.

## Corpus và điểm nối parser

Corpus thuộc [fixtures title-authority](../../services/vink-app/tests/fixtures/title-authority/README.md).
`legacy-v1.json` chạy qua application save → recording store → revision constructor,
với document dựng bằng hằng v1 riêng của test: nếu #255 đổi `SCHEMA_VERSION`, tương thích
v1 phải được giữ hoặc test fail, không âm thầm kiểm version mới (hash không chứa version).
`markdown-proposed.json` là các oracle **DESIGN_PROPOSED / extraction NOT_RUN**;
chúng chưa được parser Rust/TS nào thực thi và không phải hợp đồng API v2.
Thứ tự đề xuất: extraction/flatten → chuẩn hóa của `Title::parse` (trim, NFC) →
validation (ký tự cấm, >200 scalar). `expected_projection` ghi kết quả sau chuẩn hóa,
trước kết luận validation; raw source vẫn giữ nguyên, kể cả NFD.

Grammar tham chiếu là [CommonMark 0.31.2](https://spec.commonmark.org/0.31.2/),
đặc biệt [ATX](https://spec.commonmark.org/0.31.2/#atx-headings),
[Setext](https://spec.commonmark.org/0.31.2/#setext-headings) và container/code/HTML.
Đề xuất cho owner extraction sau khi #255 có contract: duyệt toàn AST theo source,
kể cả H1 trong blockquote/list; bỏ fence, indented code và raw HTML block;
flatten text/code/link label/image alt, decode entity/escape, bỏ inline HTML tags,
đổi soft/hard break thành một khoảng trắng trước validation. Các chi tiết flatten
và extension/dialect cần reconcile với provider #256 và Reader #264 trên cùng corpus,
không suy ra từ regex hoặc DOM/AST đã cắt partial. Raw HTML không được thực thi.

**CHOICE-01 chưa chọn:** A — server parser sinh projection, client chỉ preview;
B — client gửi projection/version và server recompute bằng cùng owner/dialect.
A ít trường tương thích hơn; B hữu ích nếu payload cần giữ client projection, nhưng
không bỏ server validation. Chọn sau khi có canonical source và phép đo save có giới hạn.
Không thêm dependency/parser chưa có consumer hợp lệ chỉ để làm corpus xanh.

**CHOICE-02 chưa chọn:** A — materialize metadata trong cùng save transaction/CAS;
B — bounded derived-on-read và publication metadata snapshot bất biến.
A phù hợp list/search hiện tại; B cần chứng minh read cost/consistency trước khi chọn.
Đây là lựa chọn kỹ thuật trong #263, không quyết định lại schema/hash của #255.

## Bản đồ tích hợp cần tiếp tục

| Owner/caller hiện hữu | Điều phải giữ hoặc bổ sung khi #255 sẵn sàng | Bằng chứng cần có |
| --- | --- | --- |
| `domain/document.rs`, `domain/revision.rs` | Dispatch theo format/version đã validate; v1 tiếp tục tên riêng/hash cũ; Markdown title là projection qua cùng `Title` validation. | Corpus Rust, golden v1/v2, no source mutation; không có constructor bypass. |
| `application/mod.rs::save_draft` | Server derive/validate một change từ source/format; không có hai writable tên. | Contract tests với client title sai, no/empty/long H1; invalid không chạm store. |
| `persistence/postgres.rs::save_draft`, migration owner #255 | Body/format/title/search key cùng CAS; giữ standing fence và phân biệt conflict/not-found. Tags vẫn có version riêng. | PostgreSQL scratch: stale saves, concurrent tags/title, rollback và lost ACK/retry. |
| `persistence/drafts.rs`, `domain/draft_query.rs`, discovery | List/search/sort lấy projection đúng version; placeholder chỉ phục vụ hiển thị/search key như hiện tại. | PG/API fixtures và paging với đổi/xóa H1; không đổi work ID/URL. |
| `persistence/publication.rs` | Capture source/title/metadata từ cùng saved version; snapshot cũ bất biến, hash không bị recompute theo parser mới khi đọc. | PG capture + draft đổi H1 + đọc lại publication cũ; legacy fixed hash. |
| `http/contract.rs`, `contracts/http/v1/` | Provider công bố version/validation/error field từ contract #255; consumer fixtures đồng bộ. | Provider/consumer fixture và HTTP tests, không tin tên client. |
| Web `desk_state.rs`, `pages/desk.rs`, `api.rs` | Default tên riêng; Markdown chỉ sửa H1; lỗi giữ raw buffer, save ticket và undo/IME. Không tự lưu/công khai. | Web save/reload, conflict, stale/lost ACK, nhập tiếng Việt và title lỗi. |
| Web `pages/mod.rs`, Reader #264 | Header/preview/list lấy đúng persisted/snapshot projection; không gọi parser mới để đổi tên publication lịch sử. | Browser + PG snapshot/Reader; responsive và screen-reader/IME tách evidence. |

## Điều kiện tiếp tục

1. Fetch lại `develop`, #255 body/comments/PR, xác nhận envelope/format/source capability
   và exact-revision schema/hash signoff nếu thay đổi đó đòi gate của registry.
2. Reconcile corpus đề xuất với dialect chung #255/#256/#264; triển khai pure extraction
   tại domain owner, chạy Rust/TS differential trên cùng input; giữ v1 guards.
3. Chọn CHOICE-01/02 bằng source/đo bounded; nối application/API/PG transaction trước
   title UI và consumers. Không dùng source text v1 làm Markdown giả.
4. Chạy quick, PG và Web đúng boundary rồi nghiệm thu từng AC. Checklist live ở
   issue #263; [bằng chứng lát độc lập](../development/evidence/editor-title-263/README.md)
   không thay checklist và không tuyên bố hoàn tất issue.
