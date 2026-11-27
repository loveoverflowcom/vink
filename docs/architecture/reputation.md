# Reputation V1 — đặc tả miền backend (đang review)

Nguồn quyết định: [VDR-REPUTATION@r1](../../decisions/proposals/VDR-REPUTATION.r1.toml)
và [VDR-REPUTATION-CONTRACT@r1](../../decisions/proposals/VDR-REPUTATION-CONTRACT.r1.toml)
(cả hai PENDING); yêu cầu [PRD §19](../../prd/requirements.md#19-điểm-vink-reputation-v1);
[EPIC #600](https://github.com/loveoverflowcom/vink/issues/600). Không phải ADR Accepted.

## Why / thuật ngữ

Đo sự ghi nhận của cộng đồng đối với sáng tác và nhận xét, không đồng nhất uy tín với chất lượng
văn học hoặc độ nổi tiếng. Impact, Contribution là hai dữ kiện thành phần độc lập; rating là
chỉ số tổng hợp không dự đoán xác suất thắng; period growth đo đóng góp mới hợp lệ theo kỳ.
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
  principal hoặc source đã xóa. Unlike/un-Ưng và re-like, suspension, admin reversal, delete work,
  comment edit: chi tiết product OPEN; không triển khai policy ngầm.
- Follow, saved, private views, content counts không đi vào rating hoặc growth.

## Period growth — không lấy đơn giản score cuối kỳ trừ đầu kỳ

Tuần bắt đầu thứ Hai 00:00, tháng bắt đầu ngày 1 00:00 theo Asia/Ho_Chi_Minh; biểu diễn khoảng
[start, end), UTC storage/instant chuẩn. Kỳ được gán theo thời điểm nghiệp vụ accepted có thẩm
quyền, không theo thời điểm worker xử lý; xử lý late events, checkpoint, boundary, replay.
Nguồn thời điểm/backdating/retention cụ thể phải audit. Thống kê uy tín cao là current rating
đủ điều kiện; tăng nhanh theo tuần/tháng là growth metric. Việc có thêm leaderboard theo kỳ
cho mức uy tín cao (snapshot/đỉnh) hay lịch sử công khai chưa được chốt.

Growth không được tăng do restore, backfill không đủ lịch sử, formula version switch hay lặp
on/off. Cần review trường hợp unlike trong kỳ đối với tương tác trước kỳ, re-like khác kỳ,
moderation correction của kỳ đã kết thúc, private/re-public và tài khoản xóa. Không cộng dồn
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
unavailable/coverage rõ, không tạo số giả.

Privacy: guest/public score và statistics chỉ dựa trên dữ liệu đủ điều kiện hiện hành;
không trả raw components. Thiết kế invalidation/generation guard fail closed cho mọi projection
đang lag, cache/ETag, kỳ cũ, pagination; kiểm private/hidden data differencing. Không sửa
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

Các chi tiết API ở [contract](../../services/vink-app/contracts/http/v1/reputation.md), trạng thái
và evidence ở [registry](../development/reputation.md).
