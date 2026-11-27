# Reputation V1 — đặc tả miền backend (đang review)

Nguồn quyết định: [VDR-REPUTATION@r1](../../decisions/proposals/VDR-REPUTATION.r1.toml)
và [VDR-REPUTATION-CONTRACT@r1](../../decisions/proposals/VDR-REPUTATION-CONTRACT.r1.toml)
(cả hai PENDING); yêu cầu [PRD §19](../../prd/requirements.md#19-điểm-vink-reputation-v1);
[EPIC #600](https://github.com/loveoverflowcom/vink/issues/600). Không phải ADR Accepted.

## Why / thuật ngữ

Đo sự ghi nhận của cộng đồng đối với sáng tác và nhận xét, không đồng nhất uy tín với chất lượng
văn học hoặc độ nổi tiếng. Impact, Contribution là hai dữ kiện thành phần độc lập; rating là
chỉ số tổng hợp không dự đoán xác suất thắng; period growth đo đóng góp mới hợp lệ theo kỳ, trừ các tương tác bị thu hồi trong kỳ kể cả nhận trước kỳ (REP-Q07).
PrincipalId là identity, WorkId là entitlement đích của Impact; RevisionId vẫn là anchor nguồn.
Không cấp quyền hay xếp hạng tác phẩm bằng score.

## Semantics đã được chọn và điểm mở

- Impact: một đơn vị (actor, WorkId) dù nhiều revision; các anchor nguồn giữ nguyên theo LVE-FR-04.
  Xác định effective entitlement từ tập anchor hợp lệ, không cộng lại khi thêm anchor thứ hai.
  Khi còn anchor hợp lệ, việc bỏ một anchor không làm mất entitlement. Audit policy historical
  anchor khi revision không còn hiện hành; không đánh đồng superseded với hidden/admin-removed.
- Contribution: Ưng theo đúng nguồn comment; không nhân điểm lên root, work hoặc parent.
  Comment edit/revision và tombstone hợp lệ cần review theo luật nguồn hiện hành.
- Private/withdrawn: loại phần score liên quan khỏi current eligibility; restore chỉ từ entitlement
  còn hợp lệ; không tăng growth. DEL-05 thu hồi endorsement của principal xóa. Không hồi sinh
  principal hoặc source đã xóa. Unlike/un-Ưng của entitlement còn hiệu lực làm giảm score và
  growth trong kỳ thu hồi kể cả endorsement trước kỳ. Re-like, admin reversal, delete work,
  comment edit: chi tiết attribution và lifecycle còn OPEN; không triển khai policy ngầm.
- Đình chỉ toàn bộ tài khoản nhận điểm: không public rating, không xuất hiện trong bất kỳ thống kê
  public nào (cả kỳ đã đóng); giữ facts còn hợp lệ, đánh giá lại trước khi khôi phục sau đình chỉ,
  không reset điểm, không ghi standing restore thành growth. Hạn chế riêng đăng/bình luận khác
  full suspension. Effect lên endorsement do actor bị đình chỉ tạo ra còn OPEN.
- Follow, saved, private views, content counts không đi vào rating hoặc growth.

## Period growth — không lấy đơn giản score cuối kỳ trừ đầu kỳ

Tuần bắt đầu thứ Hai 00:00, tháng bắt đầu ngày 1 00:00 theo Asia/Ho_Chi_Minh; biểu diễn khoảng
[start, end), UTC storage/instant chuẩn. Kỳ được gán theo thời điểm nghiệp vụ accepted có thẩm
quyền, không theo thời điểm worker xử lý; xử lý late events, checkpoint, boundary, replay.
Nguồn thời điểm/backdating/retention cụ thể phải audit. Thống kê uy tín cao là current rating
đủ điều kiện; tăng nhanh theo tuần/tháng là growth metric. Backend hỗ trợ thống kê kỳ hiện tại và kỳ đã kết thúc (REP-Q09), có retention và coverage;
định nghĩa top rating trong kỳ (điểm cuối kỳ/đỉnh), correction kỳ đã đóng và UI history còn OPEN.

Growth không được tăng do restore, backfill không đủ lịch sử, formula version switch hay lặp
on/off. Growth có dấu: bỏ một tương tác cũ đang đủ điều kiện làm giảm growth của kỳ thu hồi;
+80 điểm từ tương tác mới và -30 do bỏ tương tác tháng trước => +50 (ví dụ minh họa, không phải
formula). Thu hồi chỉ khi effective entitlement thực sự đổi (không giảm nếu còn anchor hợp lệ);
no-op/retry không trừ thêm. Re-like cùng/khác kỳ phải không tạo growth giả; bù trừ reversal và
cách gán kỳ cho correction cần review. Admin/private/delete lifecycle có thể làm đổi score hoặc
sửa/ẩn lịch sử vì privacy; không tự phân loại mọi biến động lifecycle là community growth. Không cộng dồn
các delta làm tròn theo thứ tự batch một cách tùy tiện: với hàm phi tuyến phải so sánh phép tính
trên net eligible facts/fixed reference hoặc attribution có tính tái lập; chỉ chọn sau fixtures
và owner review. Không công khai dữ liệu kỳ nếu delta có thể tiết lộ đóng góp private đã rút.

## Scientific review (NOT_RUN)

So sánh weighted log, căn bậc hai, piecewise diminishing returns; baseline 1000, scale, k,
weight I/C chỉ candidate. Test monotonicity, precision/rounding, extreme/saturation, skew,
sensitivity, nhóm chỉ viết/chỉ nhận xét/người mới, lợi thế người mới trong growth, tương quan
và abuse. Dữ liệu synthetic/authorized aggregate. Không gọi log là anti-abuse, không tuyên bố
khoa học hoặc benchmark chưa chạy. Owner duyệt formula và eligibility trước kích hoạt.

## How (candidate, không khóa schema)

Nguồn authority App accepted transition -> durable effect cùng transaction -> worker batch
-> pure Rust policy/formula -> projection current và kỳ -> indexed read API. Event/dirty queue,
entitlement/ledger, versioned cutover, storage/retention là lựa chọn kỹ thuật mở.
Không mặc định dùng stream Notifications (coverage/retention khác); không thêm Follow producer
cho V1. Không external infra nếu chưa có evidence. Audit source owners Endorsement, publication,
discussion, principal standing/deletion, outbox; giữ lock order, fencing, idempotency và bounded
batch. Version policy/formula/schema độc lập; đổi formula không tự tạo growth; replay không
double count. Backfill consistent watermark/catch-up nếu đủ source, nếu thiếu lịch sử thì
unavailable/coverage rõ, không tạo số giả. Giữ projection theo kỳ thưa nếu phù hợp, không mặc định
snapshot mọi thành viên mỗi ngày. Retention period/ledger, correction và version compatibility
phải được đặc tả và đo trước rollout; không hứa lưu vô hạn.

Privacy: guest/public score và statistics chỉ dựa trên dữ liệu đủ điều kiện hiện hành;
không trả raw components. Thiết kế invalidation/generation guard fail closed cho mọi projection
đang lag, cache/ETag, kỳ cũ, pagination; full suspension chặn public current và historical;
khôi phục sau standing review. Kiểm private/hidden data differencing. Không sửa
SOC-PRIV, DEL, LVE, RANK, TOP, SAV. Không quyền mới từ rating.

## Bất biến đề xuất để đặc tả/test

| ID | Oracle |
| --- | --- |
| REP-INV-01 | Accepted durable changes only; duplicate/retry/rollback/stale command không nhân hoặc hồi sinh effect. |
| REP-INV-02 | Unique eligible entitlement theo actor+WorkId, anchor provenance không đổi source; counter checked không âm/overflow. |
| REP-INV-03 | Current rating deterministic cùng facts + policy/version; unavailable không giả baseline. |
| REP-INV-04 | Period boundary, late processing, replay và restore/version changes không tạo growth giả; output theo policy đã duyệt. |
| REP-INV-05 | Quyền hiện hành thắng projection/cache cũ; deletion, private và moderation không rò rỉ. |
| REP-INV-06 | Batch/read bounded, no request-time global recompute; rank tác phẩm và ACL độc lập. |
| REP-INV-07 | Thu hồi tương tác cũ làm giảm growth tại kỳ thu hồi đúng một lần khi entitlement thực sự mất; re-like không tạo credit mới; formula/restore không tạo growth. |
| REP-INV-08 | Kỳ đã đóng có coverage/version/retention rõ; full suspension và deletion/privacy hiện hành thắng dữ liệu lịch sử; không fake history. |

Các chi tiết API ở [contract](../../services/vink-app/contracts/http/v1/reputation.md), trạng thái
và evidence ở [registry](../development/reputation.md).
