# SR-01 — Điểm Vink V1: review khoa học và qualification tổng hợp

[#600](https://github.com/loveoverflowcom/vink/issues/600) · [PR #601](https://github.com/loveoverflowcom/vink/pull/601) ·
REP-S01. Quy tắc sản phẩm sở hữu ở [PRD §19](../../prd/requirements.md#19-điểm-vink-reputation-v1) và
[VDR-REPUTATION@r1](../../decisions/proposals/VDR-REPUTATION.r1.toml). Trang này ghi căn cứ phương pháp, các
so sánh dẫn tới lựa chọn Q25..Q28 và bằng chứng định lượng cho `formula_version 1`. Mọi dữ liệu ở đây là tổng
hợp (Q24); không có calibration trên người dùng thật, không đo PostgreSQL.

## Căn cứ phương pháp

- [JRC 10-step guide](https://knowledge-for-policy.ec.europa.eu/composite-indicators/toolkit_en/navigation-page/10-step-guide_en):
  khung lý thuyết, chọn dữ liệu, chuẩn hóa, trọng số/tổng hợp và sensitivity. Composite index không đo chất
  lượng văn học.
- [Stack Overflow reputation](https://stackoverflow.com/help/whats-reputation),
  [vote correction](https://stackoverflow.com/help/serial-voting-reversed),
  [removed account](https://stackoverflow.com/help/user-was-removed): cơ chế cộng/thu hồi và audit chỉ là
  tham chiếu; Vink không nhập quyền theo điểm, cap hay ngoại lệ deletion của nền tảng khác.

## SR-01A — So sánh hình dạng (lịch sử, dẫn tới Q26)

I, C là số entitlement hiệu lực, không phải số đếm thô của nguồn. Cùng chuẩn f(0) = 0, f(100) = 1, trọng số
Impact w = 0,6, R = 1000 + 400 × (w × f(I) + (1 − w) × f(C)):

- log: f(x) = ln(1 + x/10)/ln 11;
- shifted sqrt: f(x) = (√(1 + x/10) − 1)/(√11 − 1);
- piecewise: f(x) = [3·min(x,10) + 0,6·min(max(x−10,0),90) + 0,15·min(max(x−100,0),900) + 0,05·max(x−1000,0)]/84.

| Hồ sơ tổng hợp | I | C | Log | Sqrt | Piece |
| --- | ---: | ---: | ---: | ---: | ---: |
| Mới | 10 | 5 | 1096 | 1058 | 1114 |
| Chuyên sáng tác | 300 | 20 | 1417 | 1524 | 1394 |
| Chuyên nhận xét | 20 | 300 | 1339 | 1391 | 1320 |
| Cân bằng | 100 | 100 | 1400 | 1400 | 1400 |
| Lâu năm | 1000 | 600 | 1736 | 2408 | 1929 |
| Một chiều | 1000 | 0 | 1462 | 1938 | 1626 |
| Chỉ nhận xét | 0 | 300 | 1229 | 1315 | 1217 |
| Chưa có ghi nhận | 0 | 0 | 1000 | 1000 | 1000 |

Sqrt thưởng mạnh cho khối lượng lớn; piecewise dễ audit nhưng có bước gãy; log giảm dần đều. Chủ sản phẩm
chọn log (Q26) và 60:40 (Q25).

## SR-01B — Tăng trưởng tách khỏi hiệu số rating (dẫn tới Q28)

Cùng +10 I và +10 C từ mức đầu kỳ I = C = x:

| x đầu kỳ | Log marginal | Sqrt marginal | Piece marginal | Đơn vị cố định G |
| ---: | ---: | ---: | ---: | ---: |
| 0 | 116 | 72 | 143 | 40 |
| 10 | 68 | 55 | 29 | 40 |
| 100 | 15 | 25 | 7 | 40 |
| 1000 | 2 | 9 | 2 | 40 |

Hiệu số rating ưu ái người mới và khó bù trừ đúng khi có rút/khôi phục. G cố định (2,4 mỗi Thấm, 1,6 mỗi Ưng
ròng) không phụ thuộc thâm niên, dễ audit; đổi lại không bằng R(end) − R(start). Chủ sản phẩm chọn G (Q28) và
máy credit Q41.

## SR-01C — Eligibility và anti-abuse

Ngưỡng tuổi 7 ngày, dedup WorkId và hàm log không chống thông đồng: một nhóm nhỏ actor tập trung vẫn có thể
tạo cùng điểm như nhiều actor khác nhau. Gate ≥ 3 actor net dương của Top Growth (Q43) giảm rủi ro một
cặp thổi điểm nhưng không phải k-anonymity. Không tự trừ vì report; correction đi qua operator CLI (Q38).
S05/S06 phải test differencing trên dữ liệu ẩn và concentration trên fixture.

## SR-01D — Qualification cho `formula_version 1`

Script: [`qualify.py`](../../services/vink-app/bench/reputation-sr01/qualify.py) (Python stdlib, Decimal 60 chữ số, seed cố định).
Kết quả: [`results.json`](../../services/vink-app/bench/reputation-sr01/results.json). Chạy lại từ root:
`python3 -I services/vink-app/bench/reputation-sr01/qualify.py`. Hai lần chạy trên nhánh PR (base develop@b69f34bc) cho
cùng SHA-256 `1481f4f7dfaa83c05ceddbd11412ceb3253386c0f02caea251bdcfc9168d24d8`.

### Hình dạng

- f(0) = 0, f(100) = 1; f tăng ngặt và bước tăng giảm ngặt trên 0..10 000.
- Thấm hợp lệ đầu tiên +10 điểm hiển thị, Ưng đầu tiên +6.
- Ở biên I = C = 2^40 − 1, f ≈ 10,602 và R hiển thị 5241: không cần trần nghiệp vụ; `score_micros` nằm
  trong i64 với biên rất rộng.

### Độ chính xác

| Phép kiểm | Kết quả |
| --- | --- |
| f64 (`log1p`/`log`) so với Decimal trên lưới I, C ∈ 0..999 (10^6 cặp) | 0 sai lệch `score_micros`, 0 sai lệch hiển thị |
| R nguyên chính xác trên lưới | 4 cặp (I, C ∈ {0, 100}) |
| Khoảng cách nhỏ nhất khác 0 của R × 10^6 tới số nguyên | 4,837 × 10^-7 tại (314, 885) |

f64 qua được lưới với biên chỉ khoảng 2 ulp, và kết quả `ln` của libm có thể khác 1 ulp giữa nền tảng. Vì vậy
K2 chọn số học nguyên fixed-point xác định cho đường canonical; f64 chỉ dùng chẩn đoán. S02 phải tái tạo
đúng 32 golden vectors trong `results.json`, gồm 6 cặp có phần lẻ gần 0,5 nhất trên lưới 0..399
(ví dụ (153, 141) → `1460500004`, hiển thị 1461; (283, 65) → `1472499987`, hiển thị 1472).

### Phân phối tổng hợp

Ba quần thể 20 000 thành viên (seed 600..602): 55% không có tín hiệu, 15% chỉ nhận Ưng, 15% chỉ được Thấm,
15% cả hai; số đếm theo Pareto đuôi dày.

| Seed | Chỉ baseline | p50 | p90 | p99 | Max | Top 100: chỉ nhận xét / chỉ sáng tác |
| ---: | ---: | ---: | ---: | ---: | ---: | --- |
| 600 | 55,7% | 1000 | 1031 | 1161 | 1839 | 2 / 14 |
| 601 | 56,0% | 1000 | 1031 | 1153 | 1760 | 3 / 14 |
| 602 | 56,1% | 1000 | 1030 | 1149 | 1999 | 2 / 20 |

Hơn nửa quần thể ở đúng 1000, nên Top Rating phải loại người chỉ có baseline (Q17). Người chỉ nhận xét vẫn
vào Top 100 (Q11) nhưng ít hơn người chỉ sáng tác vì trọng số 60:40.

### Sensitivity

So với k = 10, reference = 100, w = 0,6 trên người có tín hiệu (min–max qua ba seed):

| Biến thể | Spearman | Trùng Top 100 |
| --- | --- | --- |
| k = 5 | 0,9997–0,9998 | 93–97 |
| k = 20 | 1,0000 | 94–97 |
| reference = 50 | 1,0000 | 100 |
| reference = 200 | 1,0000 | 100 |
| w = 0,5 | 0,9402–0,9430 | 90–94 |
| w = 0,7 | 0,9721–0,9730 | 93–97 |

- reference chỉ nhân cả hai f với cùng hằng số: không đổi thứ hạng, chỉ đổi thang hiển thị. Giữ 100 để
  “100 ghi nhận = +1 đơn vị chuẩn hóa” dễ giải thích.
- k đổi độ cong và đảo vài vị trí ở đầu bảng; k = 10 cho điểm mới tăng rõ (+10/+6) mà không làm vài nghìn
  ghi nhận vượt quá xa.
- Trọng số là lựa chọn giá trị, ảnh hưởng thứ hạng nhiều nhất; Q25 đã chọn 60:40.

Kết luận: k = 10, reference = 100 đủ điều kiện cho `formula_version 1`. Đây không phải tối ưu thực nghiệm; dữ
liệu thật có thể dẫn tới version mới qua proposal mới.

## Giới hạn

Không dùng dữ liệu người dùng, không fit tham số, không đo correlation thật hay PostgreSQL. Các phân phối
Pareto là giả định. Quint, Kani, PostgreSQL, HTTP và benchmark thuộc S02..S06.
