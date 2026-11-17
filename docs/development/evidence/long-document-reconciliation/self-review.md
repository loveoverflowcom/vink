# Self-review reconciliation

Review pass riêng sau khi viết ma trận, cùng executor; không giả là independent reviewer khác.
Base đầu `6287fbf`, base follow-up `607ef83` sau #276/#277/#278/#279; Quick source `c2a3073`, focused source `11925bf`. Oracle trong test hiện có và docs/evidence của
reconciliation; không chép runtime/UI của PR upstream thành thay đổi của PR này.

| Rủi ro được challenge | Kết luận từ source/evidence |
| --- | --- |
| Merge/checkmarks thay runtime evidence | Mỗi AC có owner/path/test và bounds; ancestry chỉ kiểm tích hợp. #229 baseline, #230 projection, #231 RFC là ba deliverable khác nhau. |
| Benchmark save vẫn là payload lặp | `app_save_samples` reset baseline trước từng sample, title cố định, so full document trước/sau, version +1; 315 changed/105 noop đúng hashes. Hai negative controls và durable sample test đã chạy trên PG. |
| Projection bỏ integrity/authority | SQL summary NULL placeholder, shared restore invariants; commands ×2, moderation ×2, worker, appeals ×2 vẫn full loader. Public exact revision/hash trước conditional 304. Current test owners/API/DTO giữ byte identity. |
| Policy defect bị che hoặc dùng làm blocker vô hạn | Desired flow FAIL/constraint được giữ; method/migration giống base trước projection. Baseline full-flow NOT_RUN; #275 có migration/compatibility AC riêng. Không nới guard hoặc đánh dấu flow đó PASS. |
| RFC được nâng thành architecture Accepted | CLI storage current=null, head r1 PENDING. Không signoff/registry/model mutation. AC-05 nhận contract/plan và bounded probe, không chứng nhận production composite FK/GC/publish/upgrade. |
| Browser/raw được gán HEAD mới | Scene/timing lịch sử có SHA; new browser/benchmark NOT_RUN. Lượt đầu tại 6287fbf giữ riêng; corpus 3/PG 17/full Quick 13 mới tại c27286d trên develop92b2eb3. DocumentView/conversion functions giữ byte identity; toàn document.rs có delta export, không gán whole-file identity hoặc browser timing HEAD. |
| Duplicate ownership/framework | Tài liệu owner giữ chi tiết; biên bản mới chỉ reconciliation. `audit.py.txt` là raw-evidence one-shot, không command/framework sản phẩm. #238 vùng phụ và #264 format khác F3; inventory 272 items trước create. |
| Privacy/content leak | Chỉ synthetic corpus, test IDs, local trust-role URL không password; không manuscript/credential, production DB hoặc deployed claims. |
| Cleanup chạm phiên khác | Foreign worktrees/status readback giữ nguyên; target nguồn APFS clone giữ sạch, không clean. Chỉ target riêng task, exact process/lsof/provenance/recovery; data/log PG riêng giữ ngoài target. |
| Hạ verification để xanh | Các FAIL disk giữ nguyên. Final profile task-local có App/Web opt-level="z", UI preview1 + strip, debug/overflow checks và selectors/assertions giữ nguyên. Không dùng profile này đo latency. Chỉ kết quả exit/log thực của full Quick được phép gọi PASS. |
| Rollback/SSOT drift | Revert docs/evidence đủ; không migration/schema/hash/permission/transition mới. Các proposal storage/cache vẫn PENDING, WR-04 DEFERRED, Quint/Kani không bị sửa chỉ vì docs. |

Không phát hiện defect mới do diff reconciliation. Chỉ merge khi local required gates thực PASS;
Quick hoặc prerequisite FAIL thì PR draft/open, giữ chính xác blocker. Không dùng CI/Closes để
vượt nghiệm thu. PR #278 đã được chủ repo merge khi gate chưa biết; không sửa/push PR đó. Nhánh follow-up từ develop giữ checkpoint bytes; delta là oracle test 34 lines + docs/evidence; runtime/source/hash #279 và Reader/Back #280 không bị sửa. GitHub checklist/comment/closure phải fresh-read và readback sau merge follow-up.

Previous full run at11925bf observed PASS: fmt, registry127/88/snapshot8354df2c, corpus 3, PG 17 (2opt-in ignored), Quick 13. Known #275 FAIL 101 vẫn tách riêng. Hash v1 refactor hex được challenge bằng raw #249 bất biến, không chỉ round-trip hai kết quả của cùng implementation. Storage/source-format PENDING/current=null; no signoff.

Checkpoint review trước resume: #281 only PublicationPanel/UI smoke paths; all focused backend/codec/DTO/cache OIDs identical to11925bf. Latest Quick FAIL disk/doctest at c2a3073; later SSR/unit/repo NOT_RUN. Fmt/registry and3Clippy PASS. Keep PR draft; no merge/close. No upstream owner overwritten; no full result relabeled between source SHAs.

Resume review tại `dcd36ba6883b49bf764124e08e5d78278a485e48`: develop607ef83 không có delta mới; PR282 head khớp,
không có review thread mới. Fmt/registry/full Quick thực thi PASS cùng assertions/selectors.
Input OIDs so c2a3073 đồng nhất; focused PG/corpus giữ source11925bf với proof.
Không có runtime/schema/hash/authority/registry/model delta mới; không signoff PENDING.
Bổ sung raw mới và cập nhật nhãn current/historical, giữ toàn bộ raw FAIL.
Cho phép merge sau final diff/source freshness checks; issue closure phải fresh-read/readback sau merge.
