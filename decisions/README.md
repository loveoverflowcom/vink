# Kho quyết định Vink

[Record TOML](decision-delta-20261110.toml) lưu các quyết định, đề xuất và câu hỏi từ
DecisionDelta 10/11/2026 §2–4. [Bảng kiểm kê và bàn giao](../docs/development/decision-registry-rollout.md)
ghi phạm vi đã chuyển, nguồn còn dùng và các điểm cần chốt. PRD vẫn sở hữu yêu cầu và tiêu chí
nghiệm thu. ADR sở hữu các quyết định kiến trúc ngoài phạm vi nhập. Các registry phát triển ghi
mức triển khai và bằng chứng.

## Tra cứu theo phạm vi công việc

```bash
cargo xtask decision --help
cargo xtask decision query --domain publication
cargo xtask decision query O-03
cargo xtask decision query USER-08 --history
cargo xtask decision query USER-08@r1
cargo xtask decision query D-07
cargo xtask decision diff USER-08@r1 USER-08@r2
cargo xtask decision refs
cargo xtask decision validate
```

- `query ID` trả `current` cho mục đã nhập và `head` cho revision mới nhất của luồng ghi. Đọc
  `record.status` trước khi dùng nội dung. `OPEN` là câu hỏi. `DEFERRED` ghi việc hoãn. Bản
  `DESIGN_PROPOSED` chứa phương án để review. Chỉ `PRODUCT_CONFIRMED` ghi hướng sản phẩm đã chốt
  trong phạm vi nhập.
- `query ID@rN` và `--history` đọc được revision lịch sử hoặc pending. `pending_only` nghĩa là
  ID mới đang ở bước đề xuất. `related_only` liệt kê các mục có nhắc đến D-ID hay requirement ID.
  Ví dụ `D-07` liên quan cả `USER-03` và `USER-05`. Tra ADR hoặc PRD để đọc chính ID đã yêu cầu.
  Tra `PRODUCT-DECISIONS-20261110` để xem toàn bộ nhóm đã nhập.
  `unknown` là kết quả tra cứu hợp lệ và cho biết kho chưa có ID hoặc revision đó.
- Khi bàn giao, lưu Git SHA, `snapshot_sha256`, `ID@rN` và `source` từ kết quả. `query` không
  đối số xuất toàn bộ revision dưới dạng JSON. Dữ liệu xuất được dựng lại mỗi lần từ TOML.
- Khi CLI lỗi hoặc không chạy được, nêu nguồn và SHA của dữ liệu đã đọc cùng phần chưa xác minh.
  [DecisionDelta tại commit nhập](https://github.com/loveoverflowcom/vink/blob/0c5bb275c15342607e6701f14019c07e269c2a68/docs/architecture/product-decisions-20261110.md)
  là bản đối chiếu lịch sử. Khi phạm vi hiện hành chưa xác định được, ghi `UNKNOWN` và tra owner
  của miền. Query và validate chạy được ở checkout nông. Lệnh migrate cần Git object của nguồn.

## Đề xuất một revision

Soạn input TOML tạm với các trường dưới đây. Mọi luận cứ cần lưu phải nằm trong trường dữ liệu.
Giữ nội dung riêng, thông tin xác thực và transcript ngoài record.

```toml
kind = "product"
domain = "publication"
title = "Quyền ghi trên trang thay thế"
resolution = """
Ghi phương án đề xuất và phạm vi áp dụng ở đây.
"""
rationale = "Ghi căn cứ và đánh đổi cần chủ sản phẩm xem xét."
requires = ["USER-07@r1"]
```

```bash
# ID mới thuộc namespace VDR-*
cargo xtask decision propose VDR-EXAMPLE new /tmp/decision-proposal.toml

# Bổ sung revision cho ID có sẵn
cargo xtask decision query O-03
cargo xtask decision propose O-03 'REV:SHA256' /tmp/decision-proposal.toml
cargo xtask decision diff O-03@r1 O-03@r2
```

Thay `REV:SHA256` bằng `head.revision:head.sha256` vừa tra. Nếu đã có thêm proposal,
so sánh với revision mới nhất tương ứng. Với yêu cầu cùng ID, giữ `kind` và `domain` của
revision đầu luồng hiện hành. ID chỉ dùng chữ ASCII viết hoa, chữ số và dấu gạch ngang,
tối đa 80 ký tự. Domain dùng tên kebab-case chữ thường, tối đa 48 ký tự.
`kind` nhận `product` hoặc `architecture`. `requires` là danh sách tùy chọn, mỗi mục ghim
`ID@rN` đang tồn tại. Có thể khai báo tùy chọn `work_audiences = ["private", "public"]` cho
quyết định sản phẩm miền `publication`. CLI đối chiếu đúng trường này với `USER-02`. Các mô tả
bằng văn xuôi và quan hệ nghiệp vụ giữa các miền cần đọc và review.

Lệnh ghi tạo file mới trong `decisions/proposals/` với trạng thái `PENDING`. Nếu mất kết quả lệnh
sau khi ghi, tra lịch sử để nhận diện revision đã xuất hiện. Khi base cũ, đọc head mới và đối
chiếu thay đổi trước khi thử lại. File được ghi và đồng bộ trước khi công bố bằng hard link
cùng thư mục. Hệ thống file cần hỗ trợ hard link. Cùng base/cùng ID chỉ một lệnh ghi thành công.
Trên hai worktree, Git có thể báo xung đột thêm cùng tên file. So sánh payload rồi tạo lại proposal
thứ hai từ head đã hợp nhất. Nếu tiến trình bị dừng đột ngột, có thể còn file `.tmp-*`.
Xóa file tạm sau khi xác nhận tiến trình ghi đã dừng.

## Định dạng và nguồn phê duyệt

- **Format v1.** TOML qua crate `toml` được khóa trong Cargo.lock, với kiểu Rust/Serde và kiểm tra
  ngữ nghĩa. Schema ghi gồm `version`, ID/revision, payload `content` và `base` khi có. Legacy có
  cấu trúc bundle riêng, kèm SHA, path và dòng nguồn. Field lạ, duplicate key/item, kiểu sai hoặc
  version khác v1 làm CLI dừng. Các field văn bản nhận Unicode và xuống dòng LF. Datetime, float,
  `inf`/`nan` và `null` nằm ngoài hợp đồng v1.
- **Tính ổn định.** Writer dùng thứ tự field của Rust và chuẩn hóa thứ tự các danh sách dạng tập.
  SHA-256 của proposal tính trên TOML render từ dữ liệu đã validate. Snapshot băm version, digest bundle nhập và các digest revision
  theo thứ tự ID và revision. Comment trong input là ghi chú tạm. Khi thay đổi serializer hoặc
  format, phải thử lại roundtrip, đối chiếu hash cũ và thêm đường nâng phiên bản nếu output đổi.
- **Lịch sử và phê duyệt.** Bundle nhập có pin SHA-256 độc lập trong xtask, đối chiếu được bằng
  `migrate --dry-run`. SHA của proposal dùng cho so sánh payload và chống ghi đè cùng base.
  Chủ sản phẩm còn cần chọn nguồn xác nhận cho revision mới. Workflow hiện ghi `PENDING` cả khi
  input đến sau một câu trả lời của chủ sản phẩm. Lưu căn cứ và nội dung chính xác để review.
  Các field tự khai như `approval`, `authority` và `status` bị từ chối trong proposal.
- **Ranh giới local.** Validator tin bản xtask đã review và snapshot legacy ghim trong tool.
  Người có quyền sửa cả tool và pin có thể thay đổi ranh giới đó. Thay đổi file proposal ở head
  bên ngoài CLI cần đối chiếu Git hoặc SHA đã ghi trong bàn giao. Quyền ghi local, Git merge
  và quyền phê duyệt sản phẩm là các quyền riêng. Kiểm thử hiện chứng minh hành vi khi có lỗi I/O
  và ghi cạnh tranh trên hệ thống file local. Độ bền qua mất điện và quyền cưỡng chế phía server
  chưa được kiểm chứng.

## Ghim tham chiếu

Đặt marker riêng một dòng trong Markdown thuộc `prd/`, `docs/` hoặc `.agents/skills/`
khi consumer cần ghim một revision cụ thể.

```text
<!-- vdr: active USER-08@r2 -->
<!-- vdr: historical USER-08@r1 -->
```

`active` phải trỏ đúng revision đang được tra cứu cho ID. `historical` được giữ ở revision cũ
có nguồn. Lệnh `refs` chỉ báo phạm vi các marker đã khai báo. Dùng `diff` rồi đối chiếu PRD,
spec, code và test có liên quan khi cập nhật pin. Các skill tổng quát dùng tra cứu theo miền
ở đầu công việc để tránh ghim một phiên bản cố định ở nhiều nơi.

## Dựng lại import

```bash
cargo xtask decision migrate --dry-run > /tmp/vink-decision-delta.toml
cmp /tmp/vink-decision-delta.toml decisions/decision-delta-20261110.toml
cargo xtask decision migrate
```

Dry-run đọc đúng Git object đã ghim và in kết quả ra stdout. Lệnh migrate cài bundle nếu thiếu
hoặc báo file hiện tại khớp. Lỗi khác biệt yêu cầu đối chiếu nguồn và pin trước khi sửa.
