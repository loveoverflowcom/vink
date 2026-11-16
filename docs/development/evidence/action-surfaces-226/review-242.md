# Review độc lập #242

Review base develop `11c6a9c` (gồm #228 contract và #244 Desk/Discovery), head gốc
`fd0c7808d94c48bbb99c8dcd6cd5c911ed87f34a`. Merge-base gốc `a81333c`.
Clone riêng bảo toàn checkout chính và worktree #204 có 30 mục chưa commit.
Merge lịch sử #242 vào base mới không conflict, gồm cả feature-map của #244.

## Findings đã sửa

- P3 — Assertions count mới dùng `startsWith('1')`, chấp nhận cả count 10/100 khi
  expected là 1. Đổi bốn assertion #226 sang so sánh chính xác span hiển thị count;
  giữ accessible count unit và kiểm tra node/focus hiện có.
- P2 — Handoff/spec hiện tại nói thiếu contract #224, khiến continuation bị chặn sai
  sau #228. Đã đối chiếu API Rust/CMP và cập nhật dependency, ghi AC-10/AC-11 và
  CHOICE-03 mới trong issue R2. Giữ nguyên evidence cũ với nhãn lịch sử rõ ràng.

`ActionSurface`/forwarding đã có; default Standard. `FilterList` có nhưng `Reply`
chưa có trong shared registry. Không fork primitive, không đổi renderer hoặc policy.
Không phát hiện regression sản phẩm từ diff spec/tests này; UI #226 vẫn
**NOT_IMPLEMENTED**, toàn bộ AC-01..AC-11 chưa nghiệm thu.

## Kiểm chứng lượt review

Source cuối cho code/tests: `28c877398c075bd1ecf76135b55a5ecd8f44794e`.
Commit sau chỉ ghi evidence. Build env: `CARGO_BUILD_JOBS=2` (lượt Web cuối = 1),
`CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0`.

| Lệnh / kiểm tra | Kết quả và boundary |
| --- | --- |
| `cargo test --locked -p vink-patterns --test reader_actions` | PASS 3/3, SSR composition |
| `cargo xtask verify quick` | PASS toàn lane, gồm repo checks/clippy/unit/contracts/doctests |
| `node --check apps/vink-web/e2e/endorsement-smoke.mjs` | PASS syntax |
| Negative oracle qua Node VM, actual 1/10/100/0 expected 1 | PASS: 10/100 bị từ chối; code cũ chấp nhận nhầm |
| Diff / 300 relative file links / JSON lịch sử | PASS |
| `cargo xtask verify web --scenario endorsement` | FAIL môi trường; assertions browser NOT_RUN |
| Full Web, preview Web/CMP | NOT_RUN; không đổi renderer/CSS/CMP |

Các lượt Web được giữ riêng: (1) artifact lock khi quick đang chạy; (2) preflight không
phát hiện Chrome; (3) npm EACCES cache dùng chung; (4) hết disk khi tạo archive Rust;
(5) linker LLVM báo hết disk, Cargo báo build finished nhưng binary App chỉ 3184 bytes,
không executable; migration không khởi chạy được (Permission denied). Không coi bước
artifact PASS của runner là binary hợp lệ. Cần đủ disk rồi rebuild App và chạy lại
scenario endorsement; không đổi quyền thực thi của binary hỏng để lách lỗi.

Lượt cuối dùng `VINK_CHROME=/Applications/Google Chrome.app/Contents/MacOS/Google Chrome`,
`npm_config_cache=/tmp/vink-review-242-npm`, PostgreSQL scratch riêng port 55542.
Đã nén 7 artifact build của clone review và đối chiếu SHA-256 trước khi gỡ bản thô;
bản phục hồi + manifest ở `/tmp/vink-review-242-binary-backup`. Không xóa source/evidence,
không sửa checkout gốc hoặc worktree #204. Cluster review đã dừng, data/log giữ lại.
Log và SHA-256: [review-verification.json](review-verification.json).

Năm chiều: Functional PASS tại SSR/unit, browser NOT_RUN; Geometry/Visual/
Accessibility-Responsive/Interaction mới NOT_RUN. Không dùng baseline cũ làm PASS mới.
UI mới/paired capture, CMP/native, OS IME và screen reader: NOT_RUN vì không triển khai UI.
GitHub CI: NOT_RUN theo yêu cầu; không deploy.

## Tích hợp

PR review mới chứa lịch sử #242 cùng sửa đổi review, là draft thay thế dự kiến.
**Chưa merge** vì browser chưa chạy được. #242 giữ draft để không thay đổi trạng thái
trước khi xác nhận PR thay thế an toàn; không merge cả hai. Khi có browser PASS,
merge PR review mới một lần rồi đóng #242 là superseded. Không đánh dấu #226 hoàn thành.
Continuation dùng nhánh review này hoặc develop sau khi phần chuẩn bị được tích hợp.
