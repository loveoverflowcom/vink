# Reputation V1 — đặc tả miền backend (đang review)

Nguồn quyết định: [VDR-REPUTATION@r1](../../decisions/proposals/VDR-REPUTATION.r1.toml)
và [VDR-REPUTATION-CONTRACT@r1](../../decisions/proposals/VDR-REPUTATION-CONTRACT.r1.toml)
(cả hai PENDING); yêu cầu [PRD §19](../../prd/requirements.md#19-điểm-vink-reputation-v1);
[EPIC #600](https://github.com/loveoverflowcom/vink/issues/600). Không phải ADR Accepted.

## Why / thuật ngữ

Điểm Vink = Reputation Rating, Điểm tăng trưởng = Growth Score (không phải rating delta).
Hai thành phần nội bộ Ảnh hưởng/Đóng góp (Impact/Contribution), không public raw; không Elo,
không quyền ACL/ranking tác phẩm. WorkId dedup actor qua eligible revision anchors; superseded
khác hidden/vi phạm. Contribution actor+CommentId; comment edit deferred COM-10/LVE-FR-04.
Current eligibility theo Q13/Q20/Q21: lifecycle remove neutral growth; actor deletion DEL-05,
recipient deletion purge, full suspension recipient ẩn mọi kỳ; actor suspension không cascade.
Q30 principal age; Q32 source receipt transitions and provenance, no current-age fallback.
Q11 comment-only eligible; baseline-only không Top Rating.

## Q41 — invariant credit (chọn, sửa Q07/Q12/Q36)
Current selection/eligibility tách community credit; per entitlement có 2 state machines độc lập week/month:
UNSEEN(0) -> ACTIVE(1) khi first eligible effective community ON trong covered period (+1);
ACTIVE -> WITHDRAWN_PENDING(0, withdrawal period) khi voluntary effective OFF còn eligible (-1);
ON lại cùng withdrawal period -> ACTIVE (+1 bù, không fresh credit);
qua boundary vẫn OFF -> BURNED(0 terminal); ON/OFF sau burn = 0.
Baseline/preactivation hoặc held maturity -> BURNED, current-only. No-op/duplicate/retry/rollback/stale và mất một anchor khi còn anchor hợp lệ không sinh delta.
Net credit cuối - đầu kỳ = -1/0/+1 cho community transitions; lifetime net 0 hoặc 1 trước weight. Week/month độc lập, không cộng tuần suy tháng. Accepted event-time [start,end) Asia/Ho_Chi_Minh, đúng end thuộc kỳ mới; late/replay theo authoritative order và coverage.
Neutral lifecycle (privacy/withdrawn/hidden/delete/DEL-05/formula/maturity/backfill/correction) không giả ON/OFF: park credit khi tạm ineligible, restore không grant mới; OFF khi ineligible retire/burn không trừ; terminal invalidation/deletion retire không resurrect. End-start oracle áp community state, không lấy effective eligibility làm credit. Privacy/deletion/verified correction có thể remove/restate historical contributions separately, versioned; không giữ unsafe signal chỉ để bảo toàn growth.
Oracle: tuần 1 +1, tuần 2 -1, tuần 3 relike 0, tuần 4 unlike 0; nếu OFF/ON khác tuần cùng tháng thì month net 0. Model thêm same-period loops, multiple anchors, hidden/restore/retire, threshold/boundary race, late/retry/rollback, expiry.

## Q42 — maturity (chọn)
Durable bounded job một lần/actor, effective_at=created_at+7 ngày, recheck held eligible facts as-of threshold và lifecycle/order fences; current rating được ghi nhận, growth burned cả lịch. OFF trước threshold không maturity; first real eligible ON sau threshold nếu chưa được ghi nhận có thể credit theo Q37. Không dùng processing time; no-op/race/replay không mất/nhân/hồi sinh; không network Identity. Kỳ chuyển tiếp Q33 độc lập tuần/tháng.

## Q43 — Top Growth (chọn)
Net weighted growth >0 AND >=3 distinct eligible actors mỗi actor có net-positive weighted community credit cho recipient trong kỳ, sau privacy/correction. Nhiều entitlement cùng actor chỉ count 1; unlike-only, zero, neutral/maturity/backfill/burned không count. Không expose actor IDs/count. Ngưỡng 3 là heuristic chưa có thực nghiệm, không bảo đảm k-anonymity/anti-collusion; test differencing. Không thêm gate cho Top Rating.

## Q44 — snapshot (chọn có điều kiện correctness)
Open top-k immutable snapshot generation/as_of, target 5–10 phút đo thực tế (không SLA); profile PK projection gần realtime. Closed top100+50 reserve/table/period, không dense member history; retain minimized ledger/checkpoint đủ exact as-of reconstruction toàn eligible population (kể cả member không biến động). Reserve chỉ optimization, không guarantee: >50 excluded, restore/outsider/correction đổi ordering hoặc thiếu candidates -> offline refill/rebuild từ toàn population, không false-complete/stale/unsafe. Nếu thiếu evidence mark incomplete/unavailable. Current privacy/standing/deletion gate trước limit/cursor, invalidate/withhold unsafe snapshot/cache ngay; cursor bound snapshot/version/visibility generation, reject/restart stale. Closed ordinary later score không đổi as-of-end; authoritative late/correction có version. Q15 retain 52 closed weeks/12 closed months + open; source/fence/audit retention tách, minimize/purge.

## Operator correction Q38

Authorized operator CLI, single anchor/entitlement or bounded actor-recipient set at watermark,
not blanket ban on future interactions. Evidence/reason, dry-run/explicit apply, audited
idempotent invalidation fence. Reputation-only, source counts unchanged. Recompute current and
affected retained periods from all eligible as-of facts; corrected revision/as_of, privacy wins.
If evidence missing withhold, no false rank. No admin UI/automatic detection.

## Retention — Q15

Public statistics retention = kỳ mở + 52 tuần lịch đã kết thúc gần nhất + 12 tháng lịch
đã kết thúc gần nhất, theo boundary Q04. Đây là số kỳ, không phải sliding 364/365 ngày;
expiry theo boundary riêng tuần/tháng. Không truy vấn kỳ ngoài phạm vi; dữ liệu hết hạn hoặc
chưa có coverage khác 0 thật. Purge projection/aggregate hết hạn bằng tác vụ bounded,
idempotent; late replay không tái công bố kỳ hết hạn. Không hứa đủ lịch sử trước activation.
Privacy, deletion và correction có thẩm quyền có thể yêu cầu sửa/ẩn/xóa sớm hơn retention.

Không dùng TTL này cho current facts, entitlement, idempotency fence, ledger, audit và checkpoint
một cách máy móc: đặc tả minimization, nguồn rebuild/anti-replay và purge theo luật deletion.
Chỉ lưu dữ liệu cần thiết, không suy quyền lưu raw history/PII vô hạn; đánh giá storage, WAL
và khả năng dựng lại khi compact/purge, kể cả đối với entitlement cũ vẫn hiệu lực.

## Scientific review — selected direction, qualification pending

[SR-01](../development/reputation-science.md) synthetic comparisons; Q25–Q28 choose
60:40 normalized log R=1000+400*(0.6*f(I)+0.4*f(C)), f(x)=ln(1+x/k)/ln(1+reference/k).
Initial candidate k=10/reference=100 each component; qualify numeric constants and canonical precision
locally; half-up integer display once, rank unrounded, no business cap but checked numeric bounds.
G=2.4*dI+1.6*dC exact tenths. No prod access. 7-day gate and >=3 actors heuristic not empirical optimum.
Exact signoff and evidence required before runtime.

## How (candidate, không khóa schema)

Nguồn authority App accepted transition -> durable effect cùng transaction -> worker batch
-> pure Rust policy/formula -> projection current và kỳ -> indexed read API. Event/dirty queue,
entitlement/ledger, versioned cutover, storage/retention là lựa chọn kỹ thuật mở.
Không mặc định dùng stream Notifications (coverage/retention khác); không thêm Follow producer
cho V1. Không external infra nếu chưa có evidence. Audit source owners Endorsement, publication,
discussion, principal standing/deletion, outbox; giữ lock order, fencing, idempotency và bounded
batch. Version policy/formula/schema độc lập; đổi formula không tự tạo growth; replay không
double count. Q18/Q32/Q33 proof-backed current backfill, no preactivation history, first full week/month independently, baseline burned. Q44 snapshot and minimized exact as-of reconstruction source; retention and correction qualified, no dense history or request-time rank.

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
| REP-INV-07 | Q41 credit machine per calendar -1/0/+1, burned boundary fence, no repeated debit/credit; neutral and correction separate. |
| REP-INV-08 | Kỳ đã đóng có coverage/version/retention rõ; full suspension và deletion/privacy hiện hành thắng dữ liệu lịch sử; không fake history. |
| REP-INV-09 | Top rating theo current khi kỳ mở, rating as-of end khi đóng; không dùng peak, không đổi theo score hiện tại sau close; late/privacy correction theo policy. Member đủ điều kiện chỉ nhận Ưng không bị loại vì không có bài. |
| REP-INV-10 | Lifecycle remove nội dung đổi current eligibility, không tự ghi âm community growth; correction/privacy/deletion có thẩm quyền thắng lịch sử. Đình chỉ actor không cascade thu hồi endorsement trước đó còn hợp lệ; DEL-05/invalidation vẫn áp dụng. |
| REP-INV-11 | Chỉ phục vụ kỳ mở và 52 tuần/12 tháng đã đóng gần nhất có coverage; expiry/purge/replay không hồi sinh kỳ hết hạn, không phá current facts hoặc anti-replay; không giữ PII trái deletion. |
| REP-INV-12 | Q30/Q42 age 7d, maturity current-only at threshold; no-op/retry/race no fake credit, source unchanged. |
| REP-INV-13 | Top Rating valid-only, Top Growth positive and >=3 distinct net-positive actors; current authority before limit, no raw actor data. |
| REP-INV-14 | Proof-backed current, baseline burn, first full periods, no fake history/zero. |
| REP-INV-15 | Operator correction authority, audit/fence/idempotency, source count isolation, full-population as-of correction. |
| REP-INV-16 | Reserve exhaustion/outsider restore/correction cannot false-complete; exact rebuild, snapshot cursor and visibility fence. |

Các chi tiết API ở [contract](../../services/vink-app/contracts/http/v1/reputation.md), trạng thái
và evidence ở [registry](../development/reputation.md).
