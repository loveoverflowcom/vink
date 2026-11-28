# Điểm Vink (Reputation V1) — đặc tả miền backend

Nguồn quyết định: [VDR-REPUTATION@r1](../../decisions/proposals/VDR-REPUTATION.r1.toml) (sản phẩm,
`PRODUCT_CONFIRMED`) và [VDR-REPUTATION-CONTRACT@r1](../../decisions/proposals/VDR-REPUTATION-CONTRACT.r1.toml)
(kiến trúc, `Provisional`), signoff 28/11/2026. Yêu cầu: [PRD §19](../../prd/requirements.md#19-điểm-vink-reputation-v1) REP-01..14.
Tracker: [EPIC #600](https://github.com/loveoverflowcom/vink/issues/600). Mức triển khai và bằng chứng nằm ở
[registry](../development/reputation.md); trang này không phải ADR `Accepted`.

## 1. Thuật ngữ

| Thuật ngữ | Nghĩa |
| --- | --- |
| Điểm Vink (Reputation Rating) | Điểm tổng công khai R của một principal. Không phải Elo, không mang quyền. |
| Điểm tăng trưởng (Growth Score) | Tổng credit cộng đồng có trọng số trong một kỳ, G = 2,4 × dI + 1,6 × dC. Không phải R(end) − R(start). |
| Ảnh hưởng / Đóng góp (Impact I / Contribution C) | Số entitlement Thấm / Ưng đang hiệu lực của người nhận. Nội bộ, không trả qua API. |
| Entitlement | Đơn vị ghi nhận: (actor, WorkId) cho Thấm, (actor, CommentId) cho Ưng. |
| Anchor | Một nguồn đang làm entitlement có hiệu lực: một revision được Thấm, hoặc chính nhận xét được Ưng. |
| Community ON / OFF | Actor chủ động chọn / bỏ Thấm hoặc Ưng (version của dòng nguồn đổi). |
| Sự kiện trung tính | Thay đổi eligibility không do actor chọn: riêng tư, rút về, ẩn, xóa, DEL-05, đổi công thức, maturity, backfill, correction. |
| Người nhận | Chủ tác phẩm (Thấm) hoặc người viết nhận xét (Ưng). |

## 2. Công thức và độ chính xác (Q25..Q29, K2)

```text
f(x)  = ln(1 + x/10) / ln(11)            f(0) = 0, f(100) = 1, f(1200) = 2
R     = 1000 + 400 × (0,6 × f(I) + 0,4 × f(C))
score_micros = floor(R × 10^6)           canonical, xếp hạng
display      = floor((score_micros + 500000) / 10^6)   = half-up(R), làm tròn một lần
g_tenths     = 24 × dI + 16 × dC         G = g_tenths / 10
```

- I, C ∈ [0, 2^40). Ngoài biên là lỗi policy → `unavailable`; không cắt, không saturate.
- R chỉ là số nguyên khi f(I) và f(C) đều nguyên (I, C ∈ {0, 100, 1200, 13300, …}), nên R × 10^6 không
  bao giờ rơi đúng biên x,5 hay biên micro; hai dòng `score_micros` và `display` ở trên cho cùng kết quả
  với half-up trực tiếp trên R.
- Đường canonical dùng số học nguyên fixed-point xác định. SR-01 đo khoảng cách nhỏ nhất của R × 10^6
  tới số nguyên trên lưới 10^6 cặp là 4,8 × 10^-7, xấp xỉ sai số vài ulp của f64 ở R ≈ 1500; f64 chỉ
  dùng cho chẩn đoán.
- Xếp hạng: `score_micros` giảm dần, hòa thì PrincipalId tăng dần. Top Growth: `g_tenths` giảm dần, hòa
  thì PrincipalId tăng dần.
- Golden vectors: [SR-01D](../development/reputation-science.md#sr-01d--qualification-cho-formula_version-1),
  `services/vink-app/bench/reputation-sr01/results.json` và corpus
  `services/vink-app/behavior/reputation/formula-vectors.json`; owner Rust `domain::reputation::formula`.

| I | C | display | score_micros |
| ---: | ---: | ---: | ---: |
| 0 | 0 | 1000 | 1000000000 |
| 1 | 0 | 1010 | 1009539383 |
| 0 | 1 | 1006 | 1006359589 |
| 10 | 5 | 1096 | 1096430291 |
| 100 | 100 | 1400 | 1400000000 |
| 300 | 20 | 1417 | 1417005240 |
| 20 | 300 | 1339 | 1339091081 |
| 2^40 − 1 | 2^40 − 1 | 5241 | 5240936193 |

`formula_version`, `eligibility_policy_version` và schema version độc lập. Đổi công thức tính lại R từ
I, C mà không tạo tăng trưởng; đổi eligibility cần replay/rebuild từ nguồn.

## 3. Entitlement và anchor (Q02, Q19, Q23)

- Thấm neo `RevisionId` ở nguồn (LVE). Reputation gom theo (actor, WorkId): entitlement hiệu lực khi
  còn ít nhất một anchor hợp lệ. Revision bị thay thế bình thường vẫn hợp lệ; revision đặt vào
  `hidden_revision_id` hoặc bị xác minh vi phạm thì không.
- Ưng neo `CommentId`. Nhận xét bị gỡ (`removal`) hoặc mất quyền đọc làm entitlement ineligible theo
  Q13. Sửa nhận xét hoãn tới COM-10/LVE-FR-04.
- Mất một anchor khi còn anchor khác không sinh delta. Thêm anchor cho entitlement đã hiệu lực không
  sinh delta.
- Tự Thấm/Ưng đã bị nguồn từ chối; Reputation không tự cấp thêm quyền nào.

## 4. Eligibility, trưởng thành và backfill

Một entitlement **eligible** khi cùng lúc: actor đã trưởng thành (mục 4.1), có anchor hợp lệ, nội dung
còn công khai/đọc được, actor chưa bị xóa (DEL-05), không bị correction loại. Người nhận bị đình chỉ toàn
bộ hay đã xóa không làm entitlement mất hiệu lực trong I/C, nhưng bị loại khỏi mọi đầu ra công khai
(mục 7).

### 4.1 Trưởng thành (Q16, Q30, Q31, Q37, Q42)

- Ngưỡng: `principals.created_at + 7 × 24 giờ`, UTC, thời gian trôi. Không hỏi Identity.
- Trước ngưỡng, tương tác vẫn hợp lệ ở nguồn nhưng Reputation **giữ** (held).
- Tại ngưỡng, job maturity bền chạy một lần cho actor: mỗi entitlement đang ON và eligible as-of ngưỡng
  vào I/C (current-only); cả hai máy credit của nó chuyển thẳng `BURNED`.
- Entitlement đã OFF trước ngưỡng không được ghi. ON thật đầu tiên sau ngưỡng của entitlement chưa từng
  được ghi nhận là community ON bình thường (có thể +1).
- Thời điểm xử lý không quyết định kết quả: job chạy muộn vẫn dùng as-of ngưỡng; race giữa job và một
  ON/OFF được phân xử bằng thứ tự accepted.

### 4.2 Bằng chứng và backfill (Q18, Q32, Q33)

- Chỉ chuyển trạng thái accepted có thứ tự (`accepted_order`, `version`) và thời điểm accepted mới
  vào lịch sử. Không đếm receipt hay no-op trực tiếp; không thay bằng `updated_at` hay tuổi hiện tại.
  Thiếu bằng chứng thì loại và ghi coverage.
- Backfill đọc snapshot nhất quán của facts hiện hành, ghi I/C hiện hành, đặt watermark rồi catch-up.
- Không dựng kỳ trước activation. Kỳ tuần đầy đủ đầu tiên là tuần bắt đầu tại boundary ≥ thời điểm
  activation sẵn sàng; kỳ tháng đầy đủ đầu tiên tính riêng như vậy. Mọi credit đang có tại boundary
  đó là baseline `BURNED`. Kỳ một phần trước đó trả `incomplete`, không 0.

## 5. Máy credit Q41

Mỗi entitlement có hai máy độc lập, một cho tuần và một cho tháng. Credit chỉ đổi vì community ON/OFF.

| Trạng thái | Credit | Community ON (eligible) | Community OFF (eligible) | Qua boundary |
| --- | ---: | --- | --- | --- |
| `UNSEEN` | 0 | → `ACTIVE`, +1 | không đổi | không đổi |
| `ACTIVE` | 1 | no-op | → `WITHDRAWN_PENDING(kỳ hiện tại)`, −1 | không đổi |
| `WITHDRAWN_PENDING(p)` | 0 | trong kỳ p → `ACTIVE`, +1 (bù) | no-op | rời kỳ p → `BURNED` |
| `BURNED` | 0 | 0 | 0 | terminal |

- Baseline (tại activation) và held maturity vào thẳng `BURNED`; vẫn góp vào I/C hiện hành.
- Net credit trong một kỳ ∈ {−1, 0, +1}; net trọn đời ∈ {0, 1} trước trọng số.
- No-op, duplicate, retry, rollback, lệnh cũ (`accepted_order` không tiến) không sinh delta.
- Sự kiện thuộc kỳ theo thời điểm accepted trong [start, end) Asia/Ho_Chi_Minh; xử lý muộn không đổi
  kỳ của sự kiện.

### 5.1 Sự kiện trung tính

| Sự kiện | Hiện hành (I/C) | Credit |
| --- | --- | --- |
| Riêng tư, rút về, ẩn, admin gỡ (tạm thời) | trừ | park: giữ trạng thái, không ±1 |
| Khôi phục công khai | cộng lại | không cấp mới; tiếp tục từ trạng thái đã park |
| OFF trong lúc ineligible | không đổi | `ACTIVE` → retire/`BURNED`, không −1 |
| Tác giả xóa, DEL-05 actor, invalidation đã xác minh (terminal) | trừ | retire, không −1, không hồi sinh |
| Đổi công thức, maturity, backfill | tính lại | không ±1 |
| Privacy/deletion/correction có thẩm quyền | tính lại | có thể sửa lịch sử riêng, có correction revision |

Oracle bắt buộc cho S02: tuần 1 ON +1; tuần 2 OFF −1; tuần 3 ON 0; tuần 4 OFF 0. Nếu OFF và ON ở hai tuần
khác nhau của cùng tháng thì tháng net 0. Model thêm: vòng ON/OFF trong cùng kỳ, nhiều anchor, ẩn/khôi
phục/retire, race ngưỡng maturity và boundary, đến muộn/retry/rollback, expiry.

## 6. Thống kê và snapshot (Q06, Q09, Q10, Q15, Q17, Q22, Q29, Q43, Q44)

| Bảng | Ai được vào | Điểm dùng |
| --- | --- | --- |
| Top Rating kỳ mở | Có ít nhất một entitlement đã ghi nhận (không chỉ baseline 1000) | `score_micros` tại snapshot `as_of` |
| Top Rating kỳ đóng | Như trên, as-of end | `score_micros` as-of end boundary |
| Top Growth (tuần/tháng) | G > 0 và ≥ 3 actor eligible khác nhau, mỗi actor có net credit có trọng số dương cho người nhận | `g_tenths` của kỳ |

- Top 100 mỗi bảng; trang mặc định 20, tối đa 100.
- Snapshot kỳ mở bất biến theo `generation`/`as_of`, mục tiêu 5–10 phút đo thực tế, không SLA.
- Kỳ đóng lưu top 100 + 50 dự phòng; không lưu lịch sử dày từng thành viên. Ledger/checkpoint tối thiểu
  đủ dựng lại as-of chính xác toàn quần thể eligible, kể cả thành viên không biến động trong kỳ.
- Dự phòng chỉ là tối ưu: hơn 50 người bị loại, người ngoài được khôi phục, hoặc correction đổi thứ tự
  thì dựng lại offline từ toàn quần thể. Thiếu bằng chứng → `incomplete`/`unavailable`, không trả danh
  sách thiếu như đầy đủ.
- Điểm kỳ đóng không đổi theo điểm hiện tại; chỉ late event có thẩm quyền hoặc correction tạo version mới.
- Retention: kỳ mở + 52 tuần + 12 tháng đã đóng gần nhất, theo số kỳ; hết hạn → `expired`. Cleanup
  bounded, idempotent; replay không công bố lại kỳ hết hạn.
- Ngưỡng 3 actor là heuristic, không bảo đảm k-anonymity; S05/S06 phải test differencing.

## 7. Privacy và quyền (Q06, Q08, Q21)

- Gate hiện hành chạy trước limit/cursor/cache: người nhận bị đình chỉ toàn bộ, đã xóa, hoặc không còn
  hồ sơ công khai bị bỏ khỏi mọi kỳ, kể cả snapshot cũ. Snapshot/cache không an toàn bị withhold ngay.
- Cursor gắn snapshot id, version và visibility generation; cursor cũ bị từ chối để client bắt đầu lại.
- Xóa người nhận purge projection, index, cache cá nhân; audit và fence hợp pháp giữ tối thiểu.
- Không trả thành phần thô, số actor, danh tính actor, bằng chứng correction hay độ tuổi principal.
- Điểm không mở quyền, không ảnh hưởng Đề xuất, Top 100 tác phẩm, kiểm duyệt hay ACL.

## 8. Correction có thẩm quyền (Q34, Q38, Q40)

- Chỉ operator CLI; phạm vi là một anchor/entitlement hoặc tập actor–người nhận xác định tại một
  watermark. Không cấm trước tương tác tương lai.
- Bắt buộc bằng chứng và lý do; dry-run rồi apply rõ ràng; idempotency key; fence invalidation để event
  đến muộn không hồi sinh; audit.
- Chỉ tác động Reputation: số Thấm/Ưng công khai của nguồn giữ nguyên.
- Tính lại điểm hiện hành và mọi kỳ còn giữ bị ảnh hưởng từ toàn quần thể as-of, kèm correction
  revision; không giả community withdrawal. Thiếu bằng chứng thì withhold.
- Không tự trừ vì report, không UI admin, không tự phát hiện. Ledger actor–người nhận tối thiểu phục vụ
  correction; không dashboard tập trung offline ở V1.

## 9. Bất biến

| ID | Oracle |
| --- | --- |
| REP-INV-01 | Chỉ thay đổi accepted bền mới sinh effect; duplicate/retry/rollback/lệnh cũ không nhân hay hồi sinh effect. |
| REP-INV-02 | Mỗi (actor, WorkId) và (actor, CommentId) tối đa một entitlement hiệu lực; counter kiểm biên, không âm, không tràn. |
| REP-INV-03 | Cùng facts và version → cùng `score_micros`; unavailable không giả baseline. |
| REP-INV-04 | Boundary, xử lý muộn, replay, khôi phục và đổi version không tạo tăng trưởng giả. |
| REP-INV-05 | Quyền hiện hành thắng projection/snapshot/cache cũ; riêng tư, xóa, đình chỉ không rò rỉ. |
| REP-INV-06 | Đọc và batch bounded; không tính lại toàn cục lúc request; ranking tác phẩm và ACL độc lập. |
| REP-INV-07 | Máy Q41: net mỗi kỳ ∈ {−1, 0, +1}, trọn đời ∈ {0, 1}; `BURNED` terminal; sự kiện trung tính không ±1. |
| REP-INV-08 | Kỳ đóng có coverage/version/retention rõ; đình chỉ, xóa, privacy hiện hành thắng lịch sử; không lịch sử giả. |
| REP-INV-09 | Top Rating kỳ mở theo snapshot hiện hành, kỳ đóng theo as-of end; không dùng đỉnh; người chỉ nhận Ưng không bị loại. |
| REP-INV-10 | Lifecycle remove đổi hiện hành, không ghi âm tăng trưởng; đình chỉ actor không cascade; DEL-05/invalidation vẫn áp dụng. |
| REP-INV-11 | Chỉ phục vụ kỳ mở + 52 tuần + 12 tháng đã đóng có coverage; expiry/purge/replay không hồi sinh kỳ hết hạn hay phá dedup. |
| REP-INV-12 | Maturity 7 ngày current-only tại ngưỡng; no-op/retry/race không sinh credit; nguồn không đổi. |
| REP-INV-13 | Top Rating chỉ gồm người có ghi nhận; Top Growth G > 0 và ≥ 3 actor net dương; quyền chạy trước limit; không lộ dữ liệu actor. |
| REP-INV-14 | Backfill có bằng chứng, baseline burned, kỳ đầy đủ đầu tiên riêng mỗi lịch; không lịch sử hay số 0 giả. |
| REP-INV-15 | Correction có thẩm quyền, audit, fence, idempotent; số nguồn không đổi; dựng lại as-of toàn quần thể. |
| REP-INV-16 | Hết dự phòng/người ngoài khôi phục/correction không cho kết quả false-complete; dựng lại chính xác; cursor và visibility fence. |

## 10. Đường triển khai

Nguồn accepted (Thấm/Ưng, DEL-05, publication/lifecycle, nhận xét, standing) → outbox Reputation cùng
transaction → consumer `vink-worker` theo lô → quyết định pure trong `domain::reputation` → entitlement,
ledger, projection, snapshot → API đọc theo khóa chính và snapshot. Audit nguồn và chia lát ở
[registry](../development/reputation.md); hợp đồng đọc ở
[API dự thảo](../../services/vink-app/contracts/http/v1/reputation.md).
