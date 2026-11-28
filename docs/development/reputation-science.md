# SR-01 — Reputation V1: scientific review và synthetic comparison

Ngày 27/11/2026 · [#600](https://github.com/loveoverflowcom/vink/issues/600) /
[PR #601](https://github.com/loveoverflowcom/vink/pull/601). Phần đầu REP-S01, không phải công thức
đã chốt, calibration thực nghiệm, Rust test, DB benchmark hay phê duyệt triển khai. Quy tắc sản phẩm
sở hữu ở [PRD §19](../../prd/requirements.md#19-điểm-vink-reputation-v1) và
[proposal](../../decisions/proposals/VDR-REPUTATION.r1.toml). Q16=B, Q17=A, Q18=A đã có lời chọn.

## Căn cứ phương pháp

- [JRC 10-step guide](https://knowledge-for-policy.ec.europa.eu/composite-indicators/toolkit_en/navigation-page/10-step-guide_en):
  theoretical framework, data selection, normalization, weighting/aggregation và sensitivity;
  không suy composite index là phép đo chất lượng văn học.
- [Stack Overflow reputation](https://stackoverflow.com/help/whats-reputation),
  [vote correction](https://stackoverflow.com/help/serial-voting-reversed),
  [removed account](https://stackoverflow.com/help/user-was-removed): cơ chế cộng/thu hồi và audit
  là tham chiếu, không nhập quyền theo điểm, cap hay ngoại lệ deletion của nền tảng khác vào Vink.
- Source inspection develop@91191186: persistence/endorsement.rs có accepted_order, version,
  updated_at khi UPDATE, require_active ở transaction; persistence/account_deletion.rs có
  accepted_at, reversal và standing effects. Chưa chứng minh updated_at là first endorsement event
  hoặc timestamp eligibility đầy đủ; chưa audit account-age authority, toàn bộ migration/history.

## SR-01A: current rating candidates

I, C là số effective eligible entitlements của Thấm/Ưng, không raw source counts, không Follow.
Giả định so sánh: baseline 1000, mốc 100 events mỗi thành phần = 1 đơn vị normalized, scale 400,
w = weight Impact, weight Contribution = 1-w.

R = 1000 + 400 * (w * f(I) + (1-w) * f(C)).

Ba hàm trên x >= 0, chọn cùng f(0)=0, f(100)=1 để không so sánh thang khác nhau:

- log: f(x)=ln(1+x/10)/ln(11).
- shifted sqrt: f(x)=(sqrt(1+x/10)-1)/(sqrt(11)-1).
- piece: f(x)=[3 min(x,10)+0.6 min(max(x-10,0),90)+0.15 min(max(x-100,0),900)
  +0.05 max(x-1000,0)]/84.

Không hardcode các hằng số minh họa vào domain. K=10, reference=100 như nhau ở đây chỉ để
isolate shape, chưa biết phân phối I/C thật, mốc cho mỗi thành phần có thể khác. Weights là
trade-off normative sau normalization; không đồng nhất weight với tỷ lệ 1 Thấm/1 Ưng mọi mức.
Scale 400 chỉ đơn vị hiển thị, không probability/Elo. Cả hai thành phần có thể thay thế phần nào
cho nhau; không thêm điều kiện phải viết bài. Round một lần tại boundary hiển thị, chưa chốt
precision, min/max hoặc tie trên unrounded score.

### Synthetic comparison w=0.6

| Profile giả lập | I | C | Log | Sqrt | Piece |
| --- | ---: | ---: | ---: | ---: | ---: |
| Mới | 10 | 5 | 1096 | 1058 | 1114 |
| Chuyên sáng tác | 300 | 20 | 1417 | 1524 | 1394 |
| Chuyên nhận xét | 20 | 300 | 1339 | 1391 | 1320 |
| Cân bằng | 100 | 100 | 1400 | 1400 | 1400 |
| Lâu năm | 1000 | 600 | 1736 | 2408 | 1929 |
| Nhiều tương tác một chiều | 1000 | 0 | 1462 | 1938 | 1626 |
| Chỉ nhận xét | 0 | 300 | 1229 | 1315 | 1217 |
| Chưa có ghi nhận | 0 | 0 | 1000 | 1000 | 1000 |

### Sensitivity của log theo weights

| Profile | 50:50 | 60:40 | 70:30 | 80:20 |
| --- | ---: | ---: | ---: | ---: |
| Mới | 1092 | 1096 | 1101 | 1106 |
| Chuyên sáng tác | 1378 | 1417 | 1456 | 1495 |
| Chuyên nhận xét | 1378 | 1339 | 1300 | 1261 |
| Cân bằng | 1400 | 1400 | 1400 | 1400 |
| Lâu năm | 1728 | 1736 | 1745 | 1753 |
| Nhiều tương tác một chiều | 1385 | 1462 | 1539 | 1616 |
| Chỉ nhận xét | 1286 | 1229 | 1172 | 1115 |
| Chưa có ghi nhận | 1000 | 1000 | 1000 | 1000 |

Ví dụ at 50:50, (300,20) và (20,300) bằng nhau; 80:20 chênh 234 điểm hiển thị. Tác giả nhiều
Thấm một chiều (1000,0) vượt profile cân bằng (100,100) khi 60:40 nhưng không ở 50:50.
Weights thay rank đáng kể, cần owner chọn giá trị sản phẩm sau khi calibrate.

### Sensitivity của normalized log (w=0.6, reference=100)

| k | Tăng khi I 0→1, C=0 (rounded delta) | Rating I=C=1000 |
| --- | ---: | ---: |
| 10 | 10 | 1770 |
| 100 | 3 | 2384 |
| 1000 | 3 | 3909 |

k nhỏ làm điểm mới tăng nhanh hơn và nén khoảng cách ở mức cao; k lớn giảm độ cong.
Thay reference hoặc k có thể đổi rank nếu hai thành phần có scale khác nhau. Không suy optimal từ 8 fixture.

## SR-01B: growth phải tách khỏi delta current rating

Q05/Q07/Q12/Q13/Q18: chỉ net eligible new community credits/withdrawals, bù trừ đúng entitlement
trong cùng kỳ, không restoration/cutover/backfill. Week/month độc lập. Candidate so sánh:
A: rating-based marginal growth dùng reference/baseline kỳ hoặc attribution cố định, xử lý phi tuyến
và reversal/correction; cùng lượng đóng góp có thể ưu ái người mới.
B: growth index fixed-unit G = 400*(w*dI/100 + (1-w)*dC/100), dI/dC là signed net growth units
đã xử lý Q12, không phải raw current counter delta. Reference ở đây minh họa, có thể khác I/C.
Với w=.6, mỗi 1 eligible net Thấm = 2.4 units, Ưng = 1.6 units. Tính precision trước khi làm tròn;
không gọi G là mức tăng đúng của R. Version G cùng period policy; không trộn formula versions.
So sánh cùng +10 I, +10 C từ baseline I=C=x (giả định không có lifecycle/restore/withdrawal):

| x đầu kỳ | Log marginal | Sqrt marginal | Piece marginal | Fixed-unit G |
| ---: | ---: | ---: | ---: | ---: |
| 0 | 116 | 72 | 143 | 40 |
| 10 | 68 | 55 | 29 | 40 |
| 100 | 15 | 25 | 7 | 40 |
| 1000 | 2 | 9 | 2 | 40 |

Candidate B dễ audit, công bằng hơn về cùng số đơn vị ghi nhận mới, không phụ thuộc seniority;
đổi lại không bằng delta rating, cần chốt tên/contract với owner. Candidate A đúng tinh thần rating
marginal nhưng phải chỉ rõ reference/ordering và bảo đảm bù trừ không tạo credit giả. Không áp dụng
log trực tiếp lên signed delta (âm có thể ngoài domain); không cộng delta làm tròn theo event.
Growth formula/scale chưa được chọn; positivity Q17 xác định theo precision contract được duyệt.

## SR-01C: eligibility and abuse — historical alternatives

Age-only vs activity; age gate/WorkId dedup/log not anti-collusion. Concentrated actors can
produce same rating as many distinct actors. Privacy differencing needs tests; no report-only
deduction. Initial alternatives retained in earlier commits.

## Final review (28/11/2026; supersedes candidate choices)

Q25–28 selected normalized log, 60:40, 1000/400, half-up once and canonical unrounded rank;
k=10/reference=100 synthetic candidate requiring numeric qualification. G=2.4*dI+1.6*dC exact tenths,
Q41 credit/burn machine not rating delta. Q30/42 7d principal age, held maturity current-only,
no fake legacy. Q43 >=3 distinct net-positive actors heuristic not k-anonymity. Q40 ledger + CLI
correction, no offline concentration report. Q44 snapshot/reserve/exact rebuild fallback.
Prior numeric tables remain historical comparisons, not open choices or signoff.

## Evidence và giới hạn

Calculated trực tiếp bằng JavaScript Number (IEEE-754), Math.round cho số dương minh họa;
8 deterministic profiles × 3 formulas và 4 log weights, 4 baseline growth comparisons,
3 k sensitivity, 30000 monotonic finite adjacent checks trên x=0..10000 (10,000 mỗi hàm)
và checks f(0)=0, f(100)=1. PASS trong phạm vi các checks này; không chứng minh toàn miền hoặc
deterministic Rust fixed precision. Reproduce theo biểu thức ở trên, không coi là production golden vectors.
Không dùng user data, không fit tham số, không đo correlation thực tế, không benchmark PostgreSQL.
Chưa chạy cargo decision validate, Quint, Kani, DB hoặc HTTP. Tách model/proof và runtime evidence.

Next S01: synthetic/local-only distributions, skew/sensitivity/precision/golden vectors, source coverage audit, privacy and state/correction/snapshot oracles. No prod access without separate grant. Exact payload/hash signoff pending, runtime NOT_RUN.
