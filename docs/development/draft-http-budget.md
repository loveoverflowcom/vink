# Ngân sách HTTP lưu nháp — #273

Owner: `PUT /api/v1/drafts/{work_id}`, `http/api.rs`, `http/contract.rs` và Web
`pages/desk.rs` → `api.rs`. Base: `develop@9640bdef4b570fe0fc76e90e538de947c2428614`.
Đây là sửa F1 của [baseline #229](long-document-budget.md), không thay baseline lịch sử.

## Contract tích hợp #285

Riêng PUT nội dung nháp có cap 8.388.608 byte ở `DefaultBodyLimit`. GET, PUT tags (2 MiB)
và Auth (32 KiB) giữ guard hiện hành. Handler kiểm principal trước khi đọc body. Client dùng
`SaveDraftRequest` → `gloo-net RequestBuilder::json` → `serde_json` compact UTF-8.
Title riêng của `default` được trim trước khi gửi và input giữ nguyên. Markdown bỏ trường
title. App dùng H1 đầu tiên của source. Source được lưu nguyên byte, kể cả CR/LF và NFD.

| Thành phần | Cận trên cho request hợp lệ |
| --- | ---: |
| Source tối đa 2.097.152 byte UTF-8. serde_json giữ nguyên Unicode. CR/LF/tab, quote và backslash mỗi byte thành hai byte. Các control khác không hợp lệ. | 4.194.304 B |
| Title sau trim tối đa 200 scalar NFC. Một scalar NFC phân rã thành tối đa bốn scalar NFD theo bảng của dependency. Mỗi scalar raw tạo ít nhất một scalar NFD. Dành sáu byte JSON cho mỗi scalar raw. | 4.800 B |
| Hai tên format, key, dấu JSON, quote và version i64, kể cả dấu âm | 128 B |
| **Tổng bảo thủ** | **4.199.232 B** |

Giới hạn domain là 200.000 scalar sau NFC (loại CR/LF) và 2 MiB source thô. Chúng độc lập
với cap 8 MiB trên toàn body. Padding, JSON pretty hoặc escaping khác vẫn phải nằm trong
cap transport. Body quá cap hoặc JSON không hợp lệ trả 422 `validation_failed` ở `body`.
Source quá giới hạn trả 422 ở `document`. Cả hai giữ nguyên durable draft. Auth, origin,
ownership và CAS lần lượt có các regression riêng. Cap áp dụng cho declared length và chunked,
không phải giới hạn RSS toàn process.

Bản đo #273 dùng shape blocks cũ (cận 7.804.928 B) và hash cũ. Các log tại
[evidence/draft-http-273](evidence/draft-http-273/README.md) giữ nguyên làm baseline lịch sử.
Xem [kiến trúc canonical và signoff](../architecture/document-source-format.md).

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

Loopback kiểm exact source, title và content hash sau GET/đọc store, cả Markdown,
NFD, CRLF ở đúng trần source, JSON escape và supplementary Unicode. Kiểm stale, foreign,
anonymous, bad Origin và refusal quá cap qua HTTP thật trên PostgreSQL scratch. Browser
so sánh source trong buffer, PUT và GET, title trong PUT với live title input và kiểm
conflict cùng domain refusal. Kết quả theo commit ở [registry #285](document-source-format.md).
