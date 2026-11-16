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
| `cargo xtask verify web --scenario endorsement` | PASS sau cleanup được ủy quyền; real PostgreSQL + Chrome, 13 assertions #226 PASS |
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

Năm chiều cho regression: Functional PASS SSR/unit/browser; Geometry và
Accessibility/Responsive PASS theo assertions scenario; Interaction PASS cho node/focus,
ACK/reconcile và keyboard. Visual NOT_RUN (không mở capture mới); UI mới của #226,
OS IME/native/screen reader vẫn NOT_RUN. Không dùng baseline cũ làm PASS mới.

## Tiếp tục sau cleanup được ủy quyền

Không có Cargo/rustc/App đang chạy trước cleanup. Giữ hai PostgreSQL cũ port 55435/55499,
worktree #204, source và toàn bộ `target/verify-web`. Chỉ gỡ Cargo cache `target/debug`
và `target/wasm32-unknown-unknown` không hoạt động của checkout gốc, cùng hai đường dẫn
binary hỏng trong clone review. Dung lượng trống tăng từ 703 MiB lên khoảng 2,9 GiB;
sau rebuild còn khoảng 2,4 GiB. Không dọn dữ liệu không thể tái tạo.

Lượt `web-clean` chạy với cùng source/tests (head `74b86c8`, chỉ docs khác `28c8773`),
`CARGO_BUILD_JOBS=1`, Chrome và npm cache riêng như trên: **PASS** toàn scenario endorsement.
Đã kiểm executable App là Mach-O arm64 hợp lệ; migrate scratch thành công. 13 assertions
#226 PASS, real writes/double tap/lost ACK/replay/409/keyboard/401 PASS, không console error.
Full Web vẫn NOT_RUN trong lượt review. Không thay source sau test, không cần chạy lại
quick đã PASS cho cùng code/tests. Commit tiếp theo chỉ cập nhật evidence.

Raw evidence: `target/verify-web/1794817043323714000-29887`; log
`/tmp/vink-review-242-web-clean.log`. Đối chiếu cả status và SHA-256 của 30 file thay đổi
trong worktree #204: giống trước cleanup. Cluster review được dừng sau test; giữ data/log.
UI mới/paired capture, CMP/native, OS IME và screen reader: NOT_RUN vì không triển khai UI.
GitHub CI: NOT_RUN theo yêu cầu; không deploy.

## Tích hợp

[PR #245](https://github.com/loveoverflowcom/vink/pull/245) chứa lịch sử #242 cùng sửa đổi
review, đủ điều kiện tích hợp phần spec/tests sau browser PASS. Chỉ merge #245 (squash),
rồi đóng #242 là superseded; không merge cả hai. #226 vẫn mở, toàn bộ UI AC chưa nghiệm thu.
Continuation triển khai UI bắt đầu từ develop sau khi phần chuẩn bị được tích hợp.
