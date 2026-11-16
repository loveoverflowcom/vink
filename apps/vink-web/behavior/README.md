# Read lifecycle — SSOT hành vi của primitive Web

[`read-lifecycle.qnt`](read-lifecycle.qnt) mô tả `ReadOwner`, `ReadTicket`, `PageChain` từ
[#212](https://github.com/loveoverflowcom/vink/pull/212), hiện được dùng bởi danh sách Bàn viết,
thảo luận và queue kiểm duyệt. Owner implementation:
[`read_lifecycle.rs`](../src/read_lifecycle.rs); trạng thái quyết định:
[`VDR-WEB-READ-LIFECYCLE@r1`](../../../decisions/proposals/VDR-WEB-READ-LIFECYCLE.r1.toml)
**PENDING**, không phải signoff sản phẩm. Source đối chiếu: `develop@6e07806` ngày 16/11/2026.

PRD/TOML sở hữu quy tắc và trạng thái phê duyệt. Quint là oracle chuyển trạng thái của **primitive
đã chọn**; Rust là implementation. Không tạo model thứ hai cho mỗi consumer hoặc chép luật
moderation vào đây. [Registry đọc](../../../docs/development/public-read-freshness.md#lifecycle-web-202)
sở hữu adapter/data flow; [feature map](../../../docs/development/feature-map.md) sở hữu coverage.

## Phạm vi và giả định

- Một owner logic; scope `1/2` là hai identity query/session do adapter cấp, không là kiểm quyền.
  Mô phỏng ngẫu nhiên giới hạn 12 request, 3 ID hàng/cursor; generation và request ID tăng
  trong trace. Không kiểm tràn bộ đếm `u64`, tính ngẫu nhiên hoặc mọi lịch chạy.
- Ticket cũ có thể hoàn tất sau đổi scope A→B→A, invalidate, dispose, remount hoặc refresh
  supersede append. `RequestId` thật là opaque và duy nhất; replay ánh xạ ID model sang ticket
  Rust đã cấp, không gán giá trị vào atomic production.
- `change_scope` chỉ đổi owner; **không tự xóa PageChain/DTO**. Adapter phải xóa private facts
  khi đổi session/quyền. Remount trong shell tạo owner/chain mới và giữ ticket cũ để thử ABA.
- `accept_page` không finish flight; adapter có thể lookup tiếp trước `finish`. Model không
  cam kết accept head đúng một lần nếu caller gọi lặp trước finish. Lỗi transport được biểu
  diễn bằng finish không có page accept, nên không tiến cursor.
- Không mô hình hóa HTTP, DTO schema, IndexedDB/epoch, `freshness::Store/Coordinator`, clock,
  backoff, authority fingerprint, DOM/focus hoặc SQL. Model không chứng minh thu hồi nội dung
  đa tab, quyền server, durable ACK hay giao dịch. Các boundary đó giữ tests riêng.

## Properties và scenario

| Property | Điều kiện cần giữ | Kịch bản đạt tới / độ nhạy |
| --- | --- | --- |
| `ticketFence` | Chỉ flight hiện hành đúng scope/generation của owner còn sống được nhận page | A→B→A, dispose/remount cùng scope và generation, invalidate, refresh thay append; mutant bỏ fence nhận ticket trước remount ở bước 4 |
| `pageIntegrity` | Append đúng cursor; không trùng ID trong/giữa trang; next không quay vòng hoặc đi kèm trang rỗng | overlap, duplicate nội bộ, next bằng input/cursor cũ, input lệch; mutant bỏ validation nhận trang trùng ở bước 5 |
| `rejectedKeepsChain` | Từ chối không đổi IDs/cursor/pages/generation | Mọi page bị từ chối trong hai scenario |
| `headReplacesTail` | Head thành công chỉ giữ IDs mới, pages=1, tăng chain generation | Refresh rồi append lại một ID đã bị bỏ khỏi head cũ; Restart với trang cuối rỗng |

Hai scenario lawful có **52 bước** (21 fence + 31 paging). JSON sinh từ ITF thật bởi
[`target/verify.mjs`](../../../services/vink-app/behavior/target/verify.mjs), dùng chung pin Quint
**0.33.1**, TypeScript backend, seed **41**, **1.000 samples × tối đa 40 bước**. Không JS reducer.
Hai counterexample có 4 + 5 bước, runner đòi lỗi **Invariant violated** và trace đúng shape;
lỗi cú pháp hoặc chọn 0 bước không được tính là negative control.

[`read_lifecycle_replay.rs`](../src/read_lifecycle_replay.rs) feed command vào `begin`,
`change_scope`, `invalidate`, `dispose`, `accept_page`, `finish` thật; so outcome và toàn bộ
owner/chain/tickets sau mỗi bước. Cả hai counterexample phải bị production từ chối và không đổi
state ở bước cuối. Nhãn `PURE_CORE_CONFORMANCE` chỉ áp cho seam này.

```sh
# Lane hiện hữu, dùng cả local và CI; bắt buộc chọn đủ hai replay có tên.
cargo xtask verify discussion
# Khi chỉ kiểm replay đã sinh (không phải full lane):
cargo test --locked -p vink-web --lib --features ssr -- read_lifecycle::
# Sau thay đổi model có chủ đích, review cả fixture diff; không sửa expected bằng tay:
node services/vink-app/behavior/target/verify.mjs --write
```

`--write` dựng lại cả nhóm model dùng runner này, không chỉ Web. Giữ các fixture khác nguyên
nếu policy không đổi. Run evidence của lượt bổ sung nằm tại
[đối soát 16/11](../../../docs/development/verification-reconciliation-20261116.md).
Apalache/exhaustive checking, PostgreSQL và browser không được suy từ model/replay PASS.
