# Ngân sách HTTP lưu nháp — #273

Owner: `PUT /api/v1/drafts/{work_id}`, `http/api.rs`, `http/contract.rs` và Web
`pages/desk.rs` → `api.rs`. Base: `develop@9640bdef4b570fe0fc76e90e538de947c2428614`.
Đây là sửa F1 của [baseline #229](long-document-budget.md), không thay baseline lịch sử.

## Quyết định kỹ thuật và phạm vi

**#273 CHOICE-01 A:** `DefaultBodyLimit::max(8 * 1024 * 1024)` chỉ gắn vào method PUT
lưu nội dung nháp. PUT tags giữ mặc định Axum 2 MiB; nhóm Auth giữ cap riêng 32 KiB. Chọn guard sẵn có vì
nó giới hạn cả body có `Content-Length` lẫn chunked trước khi deserialize JSON; không
cần parser/streaming framework mới. Body được buffer hữu hạn, không có streaming domain
validation. Không có thay đổi migration, hash, revision, cache, autosave hay quyền.

- Sản phẩm: `USER-09@r1 PRODUCT_CONFIRMED`; `WR-04 DEFERRED`; WR-01/02/03 giữ nguyên.
- Kiến trúc: đây là lựa chọn transport được issue giao agent chọn; không ký hoặc chấp nhận
  `VDR-DOCUMENT-SOURCE-FORMAT` hay `VDR-CONTENT-STORAGE`.
- Triển khai: `persistence-api` và consumer `web`; nội dung v1 vẫn NFC, tối đa 200.000 scalar;
  title vẫn trim + NFC, tối đa 200 scalar. Không có native consumer mới.
- Evidence thực thi, SHA và giới hạn: [bản ghi #273](evidence/draft-http-273/README.md).

## Phép tính byte từ serializer thật

Web `SaveDraftRequest` → gloo-net `RequestBuilder::json` → `serde_json::to_string` → UTF-8
fetch body. Test dùng chính DTO Web và serializer này rồi deserialize bằng DTO App.
Đây là **compact JSON UTF-8**; ký tự không ASCII không tự đổi thành `\uXXXX`.

Gọi `N ≤ 200.000` là số scalar sau NFC, `L` là số line, `B` là số block:

1. Mỗi line hợp lệ chứa ít nhất một scalar không trắng; mỗi block chứa ít nhất một line.
   Vì vậy `B ≤ L ≤ N`. Empty document được phép và có chi phí cố định.
2. NFC và đầu vào có cùng NFD. Mỗi scalar đầu vào tạo ít nhất một scalar NFD;
   mỗi scalar NFC tạo tối đa **4** scalar NFD. Regression vét toàn bộ Unicode scalar
   bằng bảng của `unicode-normalization` đang pin xác nhận mốc 4. Do đó đầu vào có
   tối đa `4N` scalar, kể cả NFD tiếng Việt/Greek, không chỉ đo ký tự đã NFC.
3. Một scalar JSON qua serde_json cần tối đa 6 byte (`\u00xx`), UTF-8 tối đa 4;
   quote/backslash/tab cần 2. Bound bảo thủ là `24N` byte nội dung chuỗi.
   Control bị domain từ chối vẫn nằm trong bound này.
4. Dấu ngoặc, key `lines`, quote và comma của document tốn không quá `15N + 32` byte.
   Trường hợp nhiều block một ký tự tốn framing nhiều nhất. Title đã trim thêm tối đa
   `4 × 6 × 200 = 4.800` byte; envelope, schema và version `i64` được dành **128 byte**
   (bao gồm 32 byte cố định của document).
5. Tổng bảo thủ: `(15 + 24) × 200.000 + 4.800 + 128 = 7.804.928 B`, nhỏ hơn
   `8.388.608 B` (8 MiB). `tiny_blocks200k` riêng document compact là 3.200.031 B;
   title và envelope được đo riêng, không nhầm document bytes thành request bytes.

Web trim title ngay trước gửi, đúng với `Title::parse`; không đổi live input hoặc buffer.
Việc này tránh lượng whitespace title vốn bị domain bỏ đi trở thành chi phí vô hạn.
Các client khác có thể gửi title còn padding, JSON pretty/escape khác trong cap; **không hứa
nhận padding, whitespace hay escape tùy ý vô hạn**. Nếu vượt cap, client có thể gửi compact
UTF-8 với title trim mà giữ nguyên nội dung được domain cho phép. Không cắt chữ để vừa cap.

## Lỗi, quyền và tài nguyên

Guard chạy khi `Json` đọc body. Body vượt cap, JSON malformed/unknown field vẫn đi qua hợp đồng
hiện hành: `422 validation_failed`, `field: body`, `Cache-Control: no-store`; domain invalid là
`field: document/title`. Không đổi thành 413 hoặc đổi dạng lỗi mà consumer chưa biết. Guard
không gọi save use case nên không đổi text/version/updated_at. Khi thiếu principal, handler trả
401 trước khi đọc body. Chỉ sau đó mới chạy extractor JSON với cap riêng. Origin, actor, ownership và CAS
giữ thứ tự/kết quả hiện hành (`403`, `401`, `404`, `409`); principal standing vẫn được fence tại
commit. Không coi test header principal là bằng chứng đăng nhập production.

8 MiB là cap **mỗi request body**, không phải trần RSS/process/concurrency. Deserialize có
allocation cho các `Vec`/`String`, domain normalize và SQLx serialize tiếp; dữ liệu cố ý có nhiều
line rỗng có overhead lớn hơn số byte wire. Regression gần cap với khoảng 2,8 triệu line rỗng
đo đường này và bị từ chối, không ghi DB. `Content-Length` và chunked đều kiểm limit−1/limit/
limit+1; không tin riêng header. Xem RSS sample và timing local trong evidence, không dùng chúng
làm SLA hay bằng chứng load/concurrency production. Không thêm global resource policy ở issue này.

## Kiểm chứng và tái lập

Giữ `CARGO_INCREMENTAL=0`, `CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_PROFILE_TEST_DEBUG=0` khi so kết quả.
Dùng cluster local scratch riêng; `DATABASE_URL`/`VINK_TEST_DATABASE_URL` không trỏ production.

```sh
./tools/local-budget run -- cargo test --locked -p vink-app --test document_budget
./tools/local-budget run -- cargo test --locked -p vink-app --test postgres draft_http_budget -- --nocapture
./tools/local-budget run -- cargo test --locked -p vink-app --test postgres long_document_http_boundaries
./tools/local-budget run -- cargo xtask verify quick
./tools/local-budget run -- cargo xtask verify postgres
./tools/local-budget run -- cargo xtask build web
# Browser ownership chưa được local-budget hỗ trợ: chạy lane nguyên bản tuần tự,
# với CARGO_BUILD_JOBS=2 và RUST_TEST_THREADS=1; không gán SOFT_MANAGED cho browser.
VINK_CHROME='/Applications/Google Chrome.app/Contents/MacOS/Google Chrome' \
  VINK_DOCUMENT_BUDGET_OUTPUT=/tmp/vink-273-browser \
  cargo test --locked -p vink-app --test postgres document_budget::draft_http_budget_browser -- --ignored --exact --nocapture
cargo xtask verify web --scenario main
```

Non-ignored loopback tests dùng TCP/HTTP/1 thật và PostgreSQL thật: 7 corpus seeded ở 200k,
thêm 200k block với NFD bốn scalar, quote, backslash, supplementary; title NFD 200 scalar;
readback exact document/title/version + hash v1 tính từ dữ liệu DB. Mỗi stale/foreign/anonymous/
bad-Origin/malformed/oversize/domain-invalid refusal được đối chiếu durable state đầy đủ.
Hash lịch sử #229 vẫn được golden regression bảo vệ. Browser phase sẵn có kiểm manual save,
NFC readback, title trim trên chính request Web và title gốc trong live input, giữ full buffer khi 422/409, preview/selection và 3 corpus lựa chọn;
không phải toàn bộ benchmark storage #229.

## Tích hợp #285 và rollback

Đối chiếu PR #285 tại `fd5a922dd2b81d24381120ea64e041b9c1be41ea`: `http/api.rs` vẫn giống base,
chưa có cap riêng. #273 không cần migration/hash/title authority của #285 để sửa đường v1.
PR #285 thay hoàn toàn shape thành `{format, source}`; **không giữ v1/v2** khi tích hợp.

Người tích hợp phải cập nhật lại DTO/title optional, corpus, pure hash golden, HTTP tests,
`document_budget.rs`, phase browser, contract/docs và tính lại bound cho source nguyên văn
(raw byte cap, CR/LF/tab/escape, title sinh từ Markdown). Bound cho blocks ở trên **không chứng minh**
source canonical đã đạt. Chạy lại quick, corpus, PG/HTTP và browser trên head tích hợp; không chỉ
resolve conflict rồi kế thừa PASS của hai PR. `VDR-DOCUMENT-SOURCE-FORMAT@r2` còn PENDING;
không thay revision bất biến, rehash lịch sử hoặc tự tạo signoff.

Rollback #273 là revert thay đổi method cap và consumer trim; không có migration/DB rewrite.
Nội dung đã lưu vẫn hợp lệ theo domain, nhưng tiny_blocks lớn lại không lưu được qua HTTP sau
rollback: thông báo hạn chế đó, không truncate hoặc đánh dấu saved khi nhận lỗi. Không deploy,
merge hoặc kiểm GitHub CI trong lượt triển khai này.
