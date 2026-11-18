# Bằng chứng #273 — HTTP draft-save

Base: `9640bdef4b570fe0fc76e90e538de947c2428614`; Mac, PostgreSQL 17 local scratch riêng.
Source/checks và logs được cập nhật sau lượt verification cuối. Không đọc/chờ/rerun GitHub CI.

Baseline regression `long_document_http_boundaries_preserve_durable_text_and_conflicts`
PASS tại base: tiny_blocks200k còn bị từ chối 422. Lần compile đầu FAIL do hết đĩa trước assertions;
sau kiểm session #260 idle, checkout sạch, không có process/open file dùng target/debug, chỉ dọn
artifact build đó và chạy lại PASS. Không xóa source/WIP, DB, evidence hay artifacts của #285.

Xem [owner, phép tính và tái lập](../../draft-http-budget.md). PASS đang phát triển chưa phải
chứng nhận head cuối hoặc dữ liệu/triển khai production.
