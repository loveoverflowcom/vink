# Kho quyết định Vink

[Bundle nhập DecisionDelta](decision-delta-20261110.toml) lưu các quyết định, đề xuất và câu hỏi
của DecisionDelta: §2–4 của lượt 10/11/2026 (`PRODUCT-DECISIONS-20261110`) và §7 quản trị kiểm duyệt
của lượt 11/11/2026 (`ADMIN-MODERATION-20261111`). [Bundle nhập sổ D/ADR](decision-log-20261111.toml)
(`DECISION-LOG-20261111`) lưu phần quyết định của D-02..D-15 và các `CHOICE` theo issue trong ADR
thảo luận, literary endorsement và phân loại. Mỗi revision sau đó gồm một proposal trong
`proposals/`, và chỉ có hiệu lực khi có signoff của chủ sản phẩm trong `signoffs/`.
[Bảng kiểm kê và bàn giao](../docs/development/decision-registry-rollout.md) ghi phạm vi đã chuyển,
nguồn còn dùng và các điểm còn mở. PRD vẫn sở hữu yêu cầu và tiêu chí nghiệm thu. ADR giữ thiết kế,
lý do và các quyết định ngoài phạm vi nhập. Các registry phát triển ghi mức triển khai và bằng chứng.

## Tra cứu theo phạm vi công việc

```bash
cargo xtask decision --help
cargo xtask decision query --domain publication
cargo xtask decision query O-05
cargo xtask decision query USER-08 --history
cargo xtask decision query USER-08@r1
cargo xtask decision query D-05
cargo xtask decision query D-07
cargo xtask decision query LVE-PR-01-CHOICE-02
cargo xtask decision query ADMIN-MODERATION-20261111
cargo xtask decision query DECISION-LOG-20261111
cargo xtask decision diff O-12@r1 O-12@r2
cargo xtask decision refs
cargo xtask decision validate
```

- `query ID` trả `current` cho revision đang hiệu lực và `head` cho revision mới nhất của luồng ghi.
  Revision hiện hành là revision mới nhất trong các revision nhập chưa bị thay hoặc proposal đã có
  signoff khác `REJECTED`. Số revision lớn hơn không tự thắng. Đọc `record.status` trước khi dùng
  nội dung: `OPEN` là câu hỏi, `DEFERRED` ghi việc hoãn, `DESIGN_PROPOSED` là phương án để review,
  còn `PRODUCT_CONFIRMED` là hướng sản phẩm đã chốt. Proposal chưa có signoff mang `PENDING`.
- `query ID@rN` và `--history` đọc được revision lịch sử, pending hoặc bị từ chối. `pending_only`
  nghĩa là ID chưa có revision hiệu lực và head đang chờ signoff. `no_current` nghĩa là signoff cuối
  đã từ chối. `group` liệt kê mọi revision một lượt nhập đã chứa, kể cả revision bị thay sau đó.
  `related_only` liệt kê các record hiện hành nhắc đến D-ID hay requirement ID. D-ID đã chuyển
  (`D-05`) trả `current`. D-ID mà record DecisionDelta sở hữu trọn vẹn (`historical_alias`) trả
  `related_only`. Ví dụ `D-07` trả cả `USER-03` và `USER-05`. `D-02` trả hai phần có trạng thái riêng
  là `D-02-CONTENT` và `D-02-DISCUSSION`. Requirement ID vẫn tra chính ở PRD. `unknown` là kết quả
  tra cứu hợp lệ và cho biết kho chưa có ID hoặc revision đó. Ví dụ `D-01` vẫn tra ở sổ D-ID.
- `CHOICE` theo issue luôn mang phạm vi lát trong ID, như `LVE-PR-01-CHOICE-02`, `TAG-PR-01-CHOICE-01`,
  `DISC-01-CHOICE-01` hay `DISC-04-CHOICE-02`. `query CHOICE-01` trả `unknown`.
- Khi bàn giao, lưu Git SHA, `snapshot_sha256`, `ID@rN` và `source` từ kết quả. `query` không
  đối số xuất toàn bộ revision dưới dạng JSON. Dữ liệu xuất được dựng lại mỗi lần từ TOML.
- Khi CLI lỗi hoặc không chạy được, nêu nguồn và SHA của dữ liệu đã đọc cùng phần chưa xác minh.
  [DecisionDelta tại commit nhập](https://github.com/loveoverflowcom/vink/blob/af3c34bd624be18d404bd5789620759cae0b0ddb/docs/architecture/product-decisions-20261110.md)
  và [sổ D-ID tại commit nhập](https://github.com/loveoverflowcom/vink/blob/73ab92f0064674833ec31bc7533b210149b26b79/docs/architecture/decisions.md)
  là bản đối chiếu lịch sử. Record nhập từ sổ D/ADR ghi `source.path` và dòng nguồn tại commit
  nhập. Khi phạm vi hiện hành chưa xác định được, ghi `UNKNOWN` và tra owner của miền. Query và validate chạy được ở checkout nông. Lệnh migrate cần Git object của nguồn.

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
related_ids = ["PUB-04"]
```

```bash
# ID mới thuộc namespace VDR-*
cargo xtask decision propose VDR-EXAMPLE new /tmp/decision-proposal.toml

# Bổ sung revision cho ID có sẵn
cargo xtask decision query O-03
cargo xtask decision propose O-03 'REV:SHA256' /tmp/decision-proposal.toml
cargo xtask decision diff O-03@r2 O-03@r3
```

Thay `REV:SHA256` bằng `head.revision:head.sha256` vừa tra. Nếu đã có thêm proposal,
so sánh với revision mới nhất tương ứng. Với yêu cầu cùng ID, giữ `kind` và `domain` của
revision đầu luồng hiện hành. ID chỉ dùng chữ ASCII viết hoa, chữ số và dấu gạch ngang,
tối đa 80 ký tự. Domain dùng tên kebab-case chữ thường, tối đa 48 ký tự.
`kind` nhận `product` hoặc `architecture`. `requires` là danh sách tùy chọn, mỗi mục ghim
`ID@rN` đang tồn tại. `related_ids` là danh sách tùy chọn các requirement ID hoặc D-ID để truy vết.
Revision nhập mang sẵn liên kết này. Khi viết revision mới cho một ID có sẵn, chép lại liên kết
còn đúng để `query ACC-03` vẫn thấy record. Có thể khai báo tùy chọn
`work_audiences = ["private", "public"]` cho quyết định sản phẩm miền `publication`. CLI đối chiếu
đúng trường này với `USER-02`. Các mô tả bằng văn xuôi và quan hệ nghiệp vụ giữa các miền cần đọc
và review.

Lệnh ghi tạo file mới trong `decisions/proposals/` với trạng thái `PENDING`. Nếu mất kết quả lệnh
sau khi ghi, tra lịch sử để nhận diện revision đã xuất hiện. Khi base cũ, đọc head mới và đối
chiếu thay đổi trước khi thử lại. File được ghi và đồng bộ trước khi công bố bằng hard link
cùng thư mục. Hệ thống file cần hỗ trợ hard link. Cùng base/cùng ID chỉ một lệnh ghi thành công.
Trên hai worktree, Git có thể báo xung đột thêm cùng tên file. So sánh payload rồi tạo lại proposal
thứ hai từ head đã hợp nhất. Nếu tiến trình bị dừng đột ngột, có thể còn file `.tmp-*`.
Xóa file tạm sau khi xác nhận tiến trình ghi đã dừng.

## Ghi signoff của chủ sản phẩm

Theo [`VDR-DECISION-SIGNOFF@r1`](signoffs/VDR-DECISION-SIGNOFF.r1.toml), một proposal chỉ có hiệu
lực khi chủ sản phẩm duyệt, từ chối hoặc ủy quyền quyết định cho đúng revision đó. Agent chạy
`signoff` khi lời đó có trong phiên làm việc hiện tại hoặc trong brief/issue do chủ sản phẩm viết.
Không suy signoff từ việc chủ sản phẩm giao việc, từ một bản review hay từ lần merge trước đó.

```toml
status = "PRODUCT_CONFIRMED"
decided_on = "2026-11-11"
source = """
Trích nguyên văn lời của chủ sản phẩm, nơi nói (phiên, issue, brief) và phạm vi được quyết.
"""
```

```bash
cargo xtask decision query O-12@r2          # lấy sha256 của đúng payload đã review
cargo xtask decision signoff O-12@r2 SHA256 /tmp/decision-signoff.toml
```

- Quyết định sản phẩm nhận `PRODUCT_CONFIRMED`, `DEFERRED` hoặc `OPEN`. Quyết định kiến trúc nhận
  `DESIGN_PROPOSED`, `PROVISIONAL` hoặc `ACCEPTED`. Cả hai loại đều nhận `REJECTED`. Revision bị
  từ chối giữ lại lời từ chối và không bao giờ thành hiện hành.
- `decided_on` là chuỗi `YYYY-MM-DD`. TOML datetime nằm ngoài hợp đồng v1. Signoff không có
  trường lạ, và mỗi revision chỉ có một signoff.
- Signoff gắn với `ID@rN` và SHA-256 của payload. Khi payload proposal bị đổi, `validate` báo
  signoff lệch và revision phải được review lại. Không sửa signoff đã ghi. Muốn đổi kết quả, viết
  revision mới rồi signoff revision đó.
- Revision nhập giữ trạng thái của nguồn. Không thể signoff revision nhập. Muốn đổi luật đã nhập,
  viết proposal mới.

## Định dạng và nguồn phê duyệt

- **Format v1.** TOML qua crate `toml` được khóa trong Cargo.lock, với kiểu Rust/Serde và kiểm tra
  ngữ nghĩa. Proposal gồm `version`, ID/revision, payload `content` và `base` khi có. Signoff gồm
  `version`, ID/revision, `sha256`, `status`, `decided_on` và `source`. Legacy có cấu trúc bundle
  riêng, kèm nhóm nhập, SHA, path và dòng nguồn. Field lạ, duplicate key/item, kiểu sai hoặc version
  khác v1 làm CLI dừng. Các field văn bản nhận Unicode và xuống dòng LF. Datetime, float,
  `inf`/`nan` và `null` nằm ngoài hợp đồng v1.
- **Tính ổn định.** Writer dùng thứ tự field của Rust và chuẩn hóa thứ tự các danh sách dạng tập.
  SHA-256 của proposal tính trên TOML render từ dữ liệu đã validate. Snapshot băm version, digest
  hai bundle nhập, các digest revision theo thứ tự ID và revision, rồi digest signoff. Comment trong
  input là ghi chú tạm. Khi thay đổi serializer hoặc format, phải thử lại roundtrip, đối chiếu hash
  cũ và thêm đường nâng phiên bản nếu output đổi.
- **Lịch sử và phê duyệt.** Mỗi bundle nhập có pin SHA-256 độc lập trong xtask, đối chiếu được
  bằng `migrate --dry-run [GROUP]`. SHA của proposal dùng cho so sánh payload, chống ghi đè cùng
  base và gắn signoff. Các field tự khai như `approval`, `authority` và `status` bị từ chối trong proposal.
- **Ranh giới tin cậy.** Signoff là chứng thực, không phải xác thực. CLI và `verify quick` trên CI
  kiểm payload khớp hash, signoff không trùng và outcome hợp lệ theo loại quyết định. Chúng không
  biết ai đã viết file. Trust anchor là lời trích của chủ sản phẩm cùng lần merge vào `develop` do
  chủ sản phẩm thực hiện hoặc yêu cầu. Người có quyền ghi repo vẫn có thể tự viết signoff. Lịch sử Git
  và review PR là nơi phát hiện. Branch protection, GitHub App và chữ ký chưa được bật. Quyền ghi
  local, Git merge và quyền phê duyệt sản phẩm là các quyền riêng. Kiểm thử hiện chứng minh hành vi
  khi có lỗi I/O và ghi cạnh tranh trên hệ thống file local. Độ bền qua mất điện chưa được kiểm chứng.

## Ghim tham chiếu

Đặt marker riêng một dòng trong Markdown thuộc `prd/`, `docs/` hoặc `.agents/skills/`
khi consumer cần ghim một revision cụ thể.

```text
<!-- vdr: active O-12@r2 -->
<!-- vdr: historical USER-08@r1 -->
```

`active` phải trỏ đúng revision hiện hành của ID. `historical` được giữ ở revision cũ có nguồn.
Proposal pending hoặc bị từ chối không thể làm nguồn cho marker. Lệnh `refs` chỉ báo phạm vi các
marker đã khai báo. Dùng `diff` rồi đối chiếu PRD, spec, code và test có liên quan khi cập nhật pin.
Các skill tổng quát dùng tra cứu theo miền ở đầu công việc để tránh ghim một phiên bản cố định ở
nhiều nơi.

## Dựng lại import

```bash
cargo xtask decision migrate --dry-run > /tmp/vink-decision-delta.toml
cmp /tmp/vink-decision-delta.toml decisions/decision-delta-20261110.toml
cargo xtask decision migrate --dry-run DECISION-LOG-20261111 > /tmp/vink-decision-log.toml
cmp /tmp/vink-decision-log.toml decisions/decision-log-20261111.toml
cargo xtask decision migrate
cargo xtask decision migrate DECISION-LOG-20261111
```

Dry-run đọc đúng Git object đã ghim và in kết quả ra stdout. DecisionDelta đọc `af3c34b`, sau khi
#165 thêm §7; §2–4 giống hệt bản `0c5bb27` của lần nhập đầu. Sổ D/ADR đọc `73ab92f`, sau khi #166
merge kho TOML. Lệnh migrate cài bundle nếu thiếu hoặc báo file hiện tại khớp. Lỗi khác biệt yêu cầu
đối chiếu nguồn và pin trước khi sửa.

Importer sổ D/ADR (`xtask/src/decision/decision_log.rs`) chỉ chép trạng thái nguồn đã ghi tại commit
ghim. Mỗi câu của record là một ô bảng hoặc một trích dẫn mà `render` phải tìm thấy nguyên văn trong
khối nguồn đã khai báo. Trạng thái được trích trong `context.source_status`. Importer không nhận
`ACCEPTED` và không ghép trạng thái sai loại quyết định. Tiêu đề của `CHOICE` theo issue chỉ là nhãn.
Importer không là đường ghi quyết định: luật mới hoặc thay đổi nghĩa sau lượt 11/11 đi qua proposal
và signoff. Muốn nhập thêm một nguồn Markdown, cần một pin riêng, trích nguyên văn và review riêng.
