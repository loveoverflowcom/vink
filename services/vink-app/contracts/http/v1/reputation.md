# Reputation API v1 — dự thảo, chưa triển khai

[PRD §19](../../../../../prd/requirements.md#19-điểm-vink-reputation-v1) ·
[đặc tả miền](../../../../../docs/architecture/reputation.md) ·
[registry](../../../../../docs/development/reputation.md). Route, DTO và mã lỗi được chốt ở REP-S06 theo caller
thật; trang này giữ các ràng buộc mà mọi lựa chọn phải thỏa.

## Năng lực

| Năng lực | Ràng buộc |
| --- | --- |
| Điểm hiện hành của một principal | Một số nguyên “Điểm Vink” (half-up một lần từ `score_micros`), `formula_version`, độ tươi và trạng thái. Đọc projection theo khóa chính. Baseline 1000 chỉ khi thật sự chưa có ghi nhận hợp lệ. |
| Top Rating (kỳ mở / kỳ đóng) | Người có ghi nhận hợp lệ (không chỉ baseline; người chỉ nhận Ưng được vào). Kỳ mở theo snapshot `as_of`; kỳ đóng theo điểm as-of end, không đỉnh, không điểm hiện tại. |
| Top Growth (tuần / tháng) | Điểm tăng trưởng chính xác 0,1 từ máy credit Q41; G > 0 và ≥ 3 actor net dương sau privacy/correction. Tuần và tháng độc lập. |

## Ràng buộc chung

- Top 100 mỗi bảng; trang mặc định 20, tối đa 100. Thứ tự: điểm canonical giảm dần, hòa thì PrincipalId
  tăng dần.
- Kỳ: [start, end) theo Asia/Ho_Chi_Minh; phản hồi mang snapshot id, generation, `as_of`, các version
  (formula/policy/metric), coverage và correction revision.
- Giữ kỳ mở + 52 tuần + 12 tháng đã đóng; kỳ ngoài phạm vi trả `expired`. Kỳ trước activation hoặc kỳ một
  phần trả `incomplete`, không 0.
- Cursor gắn snapshot, version và visibility generation; cursor cũ bị từ chối để client bắt đầu lại.
- Khách và thành viên thấy cùng dữ liệu công khai. Gate privacy/standing/deletion hiện hành chạy trước limit
  và cache; người nhận bị đình chỉ hoặc đã xóa bị bỏ khỏi mọi kỳ; snapshot không an toàn bị withhold.
- Phân biệt `disabled`, `unavailable`, `incomplete`, `expired`, `forbidden`, `deleted` với baseline thật.
- Không trả thành phần I/C, số actor, danh tính actor, tuổi principal hay bằng chứng correction.
- Không có route ghi công khai. Correction chỉ qua operator CLI (Q38); số Thấm/Ưng của nguồn không đổi.
- Không N+1, không rank toàn cục, không aggregate nguồn hay tính log lúc request.

Trạng thái: `NOT_IMPLEMENTED`; HTTP chưa chạy.
