# Điểm Vink — model và vectors (#644)

| File | Nội dung | Kiểm |
| --- | --- | --- |
| `reputation.qnt` | Một entitlement, hai anchor, số đếm hiện hành của người nhận và hai máy credit Q41 (tuần, tháng) với đồng hồ độc lập; kỳ 0 là kỳ một phần trước activation. Bất biến `reputationSafe`: net mỗi kỳ ∈ {−1, 0, 1}, net trọn đời ∈ {0, 1}, chỉ credit đầu tiên là tăng trưởng, số đếm bằng trạng thái effective, xóa là terminal, không credit trước kỳ đầy đủ đầu tiên. | `cargo xtask verify discussion` (runner `../target/verify.mjs`) |
| `reputation-vectors.json` | Năm kịch bản: oracle tuần/tháng, vòng đời park/retire/xóa, maturity, OFF trước ngưỡng rồi ON, baseline activation. | Replay `domain::reputation::replay::quint_reputation_inputs_replay_through_production` |
| `reputation-relike-counterexample.json`, `reputation-anchor-counterexample.json` | Mutant: Thấm lại ở kỳ sau vẫn được credit; anchor thứ hai tính thêm một đơn vị. | Replay `…counterexamples_are_refused_by_production` |
| `formula-vectors.json` | 2 063 vector `formula_version 1` (32 vector SR-01, lũy thừa của 11, mẫu có seed tới 2^40 − 1), sinh bằng Decimal 60 chữ số. | `domain::reputation::tests::formula_reproduces_every_sr01_golden_vector_exactly` |

Sinh lại vectors công thức: `python3 -I services/vink-app/bench/reputation-sr01/qualify.py --vectors > services/vink-app/behavior/reputation/formula-vectors.json`.
Sinh lại fixture model: `node services/vink-app/behavior/target/verify.mjs --write`, rồi review diff.

Phạm vi: chỉ pure core. Không có SQL, worker, đồng hồ hay HTTP; nguồn và thứ tự sự kiện thật thuộc #645/#646.
