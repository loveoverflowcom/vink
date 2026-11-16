# #226 — baseline độc lập trước action surfaces

Base `a81333c71faf6033014a00ec26c9f7b90109d1d9`, Mac arm64, Rust 1.99.1.
[Spec/owner map](../../../design/action-surfaces-reader-discussion-226.md).
Presentation #226 **NOT_IMPLEMENTED**: thiếu contract opt-in #224 trên actual base.
PR #228 lúc kiểm đang open draft, head `64d732aef1b0be6a5f72271eebdfe80a2bc636c0`.
Không fork primitive, không blanket CSS, không đổi backend/policy/CMP.

## Cleanup trước khi fetch

- Checkout chính sạch, branch #225 `20fa75b97190c9242ce7cd1f610aa405a56f93b6`, đã có remote.
- Worktree duy nhất ngoài checkout chính là `../vink-204`, HEAD `ef7da9d1981ef9c0864af8d9c26a8c5b6d919aa3`:
  có source sửa/untracked chưa commit, không có bản sao tương đương được xác minh. Giữ nguyên.
  `git worktree prune --dry-run --verbose` không có entry stale; không worktree nào được xóa.
- Đọc process list: không Cargo/rustc/target server đang chạy. Giữ browser profile #204 và hai
  PostgreSQL sẵn có trên port 55435/55499; không reset/stop các tiến trình đó.
- Chuyển `target/verify-web` sang `/tmp/vink-226-cleanup-20261116/preserved-verify-web`;
  1.782 file có SHA-256 trước/sau trùng nhau. Manifest và status trước cleanup cùng thư mục.
- `cargo clean`: PASS, removed 8.486 files / 4,6 GiB. Chỉ dọn output checkout chính.
- Fetch `origin/develop`, fast-forward local develop `69f71f0` → `a81333c`, rồi tạo
  `codex/action-surfaces-reader-discussion`. Giữ nguyên branch #225 và source worktree #204.

## Regression và kết quả

Ba tests ở [reader_actions.rs](../../../../packages/vink-patterns/leptos/tests/reader_actions.rs)
chạy composition thật với 24 tổ hợp: 12 phase/count, 8 block/kind và 4 capability matrix.
SSR chỉ kiểm projection, không chứng minh thao tác. Browser bổ sung assertions DOM identity,
focus, noninteractive count và no optimistic count tại real-response holds của runner hiện có.
Không có assertion giả vờ surface mới đã tồn tại.

Build dùng `CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0`
để giữ disk, không tắt assertions. PostgreSQL test dùng cluster scratch riêng port 55526;
harness chỉ sở hữu database ở cluster này.

| Lệnh | Kết quả tại commit chuẩn bị |
| --- | --- |
| `node --check apps/vink-web/e2e/endorsement-smoke.mjs` | PASS |
| `cargo test --locked -p vink-patterns --test reader_actions` | PASS, 3/3 tests |
| `cargo xtask verify quick` | NOT_RUN — đang chạy, chưa có kết luận |
| `cargo xtask verify web` | NOT_RUN — chưa chạy |
| `cargo xtask ui-preview leptos verify` / `cmp verify` | NOT_RUN — không đổi renderer/CSS/CMP; qualification mới chờ #224 |
| GitHub CI | NOT_RUN — issue yêu cầu không xem/chờ/poll/rerun Actions |

Lượt test đầu FAIL do assertion mới giả thứ tự thuộc tính SSR; sửa test đọc opening tag độc lập
với thứ tự, đồng thời dùng oracle vi-VN `999.999` cho số lớn. Chạy lại 3/3 PASS, không sửa
code sản phẩm. Log thô ban đầu giữ ở `/tmp/vink-226-reader-actions-test.log`.

## Acceptance và năm chiều

Tất cả AC của issue vẫn unchecked; không sửa issue trong lượt này.

| AC | Evidence/gap |
| --- | --- |
| AC-01 | NOT_RUN: contained work Thấm chờ #224; CHOICE-01 A dự kiến, chưa có paired capture mới |
| AC-02 | NOT_RUN: CHOICE-02 A dự kiến; thiếu reply glyph đúng registry; Gửi/composer/Gỡ hiện hữu đã inventory |
| AC-03 | PASS tại SSR baseline: confirmed facts, count hidden/unavailable/known và block; UI sau surface chưa kiểm |
| AC-04 | Unit controller hiện hữu có session/stale/ACK fencing; browser assertions mới chờ chạy. Không kết luận UI mới PASS |
| AC-05 | PASS tại SSR baseline: bốn loại block và can_reply/can_remove root/flat replies; không kiểm quyền bền vững mới |
| AC-06 | Không đổi composer; coverage hiện hữu cho closed/typing/recovery/flat order. Browser baseline chờ chạy, OS IME NOT_RUN |
| AC-07 | NOT_RUN cho presentation mới: responsive/theme/focus/contrast/forced-colors phải kiểm lại sau tích hợp |
| AC-08 | NOT_RUN: chưa có before/after của surface mới; lịch sử own-content không thay eligible baseline |
| AC-09 | PASS source boundary của phần chuẩn bị: không fork/đổi primitive/backend/SQL/policy; CMP mới NOT_RUN |

| Chiều | Kết quả/boundary |
| --- | --- |
| Functional | PASS 3 unit render tests baseline; browser baseline chờ kết quả |
| Geometry | NOT_RUN cho surface mới |
| Visual | NOT_RUN cho surface mới; `NO_APPROVED_REFERENCE` |
| Accessibility/Responsive | SSR reasons/count semantics PASS; browser geometry/contrast chưa kết luận, screen reader thật NOT_RUN |
| Interaction | Browser baseline chờ chạy; OS IME/Android/iOS/TalkBack/VoiceOver NOT_RUN |

Self-review; chưa có vòng render/fix UI mới. Để tiếp tục, fetch base có contract #224 + reply
glyph đúng owner, rebase và áp recipe tại caller trong spec; chạy lại quick/web/preview và
paired capture trước/sau đủ eligible/own/guest, state và ma trận. Không merge/deploy/đóng issue.
