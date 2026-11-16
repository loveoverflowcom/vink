# #226 — baseline độc lập trước action surfaces

> Lịch sử bên dưới thuộc source `a330bb5`, không phải kết quả trên develop mới.
> Review trên `11c6a9c`: #228 đã tích hợp contract; xem [review](review-242.md).
> UI #226 vẫn NOT_IMPLEMENTED; AC-01..AC-11 giữ unchecked.

Base `a81333c71faf6033014a00ec26c9f7b90109d1d9`, Mac arm64, Rust 1.99.1.
Source/test commit `a330bb5604a4b8474e535f1a492bc856e53df698`; commit sau chỉ thêm evidence.
Kết quả, hash source/log và metadata năm ảnh đã mở: [verification.json](verification.json).
[Spec/owner map](../../../design/action-surfaces-reader-discussion-226.md).
Presentation #226 **NOT_IMPLEMENTED**: thiếu contract opt-in #224 trên actual base.
PR #228 lúc kiểm đang open draft, head `64d732aef1b0be6a5f72271eebdfe80a2bc636c0`.
Không fork primitive, không blanket CSS, không đổi backend/policy/CMP.

## Cleanup trước khi fetch

- Checkout chính sạch, branch #225 `20fa75b97190c9242ce7cd1f610aa405a56f93b6`, đã có remote.
- Worktree duy nhất ngoài checkout chính là `../vink-204`, HEAD `ef7da9d1981ef9c0864af8d9c26a8c5b6d919aa3`:
  có 30 mục source sửa/untracked chưa commit, không có bản sao tương đương được xác minh. Giữ nguyên;
  đối chiếu lại cuối lượt, status trùng trước cleanup.
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
harness chỉ sở hữu database ở cluster này. Sau Web PASS, đã dừng cluster riêng bằng
`pg_ctl -D /tmp/vink-226-pg/data stop -m fast`; giữ data/log, không dừng cluster cũ.

| Lệnh | Kết quả |
| --- | --- |
| `node --check apps/vink-web/e2e/endorsement-smoke.mjs` | PASS |
| `cargo test --locked -p vink-patterns --test reader_actions` | PASS, 3/3 tests |
| `cargo xtask verify quick` | PASS toàn lane với quyền Unix socket local |
| `cargo xtask verify web` | PASS toàn lane: main, discovery-corpus, after-restart, endorsement, review-withdrawn, auth, auth-password, publication-author |
| `cargo xtask ui-preview leptos verify` / `cmp verify` | NOT_RUN — không đổi renderer/CSS/CMP; qualification mới chờ #224 |
| GitHub CI | NOT_RUN — issue yêu cầu không xem/chờ/poll/rerun Actions |

Lượt test đầu FAIL do assertion mới giả thứ tự thuộc tính SSR; sửa test đọc opening tag độc lập
với thứ tự, đồng thời dùng oracle vi-VN `999.999` cho số lớn. Chạy lại 3/3 PASS, không sửa
code sản phẩm. Log thô ban đầu giữ ở `/tmp/vink-226-reader-actions-test.log`.
Lượt quick đầu cũng FAIL: 9 identity tests không bind được Unix socket do sandbox
(`PermissionDenied` tại `http/mod.rs:793`); các bước sau unit là NOT_RUN. Cùng source chạy lại
với socket local được phép thì toàn lane PASS, không skip test hoặc đổi assertions.
Logs: `/tmp/vink-226-quick.log` (FAIL), `/tmp/vink-226-quick-final.log` (PASS),
`/tmp/vink-226-web.log` (PASS). GitHub CI không được xem.

Web chạy Chrome 157.0.8103.78, Node 22.15.0, PostgreSQL 17, SSR + hydration debug,
local fixture và các phase Identity thật/local outbox. Endorsement phase giữ/bỏ ACK thật;
own-read 401 là interception, không phải một guest sign-in E2E. **13 assertion browser mới PASS**:
count giữ nguyên lúc pending On/Off và uncertain Off; work/comment giữ cùng node/focus qua
phase, count không thành interactive con; reconcile và conflict không thay target.
13 `lve_web_*` hiện hữu trong quick kiểm stale/late response và session fencing ở Rust boundary.

## Capture baseline và tự review

Raw run: `target/verify-web/1794809876916630000-42551/`; PNG và `screenshots/contexts.json`
giữ local, không commit raw ảnh/log. Đã mở năm ảnh được liệt kê kèm SHA-256 trong JSON:
thread 390 light100, composer 390 light200, selected work Thấm 390 light100/dark200,
own-read 401 ở desktop. Đây là baseline trước presentation mới, không có after mới.

- Thread giữ root + replies phẳng; sample chữ có dấu còn nguyên. Hữu ích unavailable giữ
  dấu “–”, không thành số 0. Ảnh này là contract fixture, không chứng minh quyền ghi thật.
- Selected work Thấm có count 1/check từ ACK. Nền selected hiện hữu không chứng minh Off
  đã có Tonal opt-in; không dùng hai ảnh này để nghiệm thu AC-01.
- Composer chữ 200% đang focus field, nút Gửi chỉ hiện một phần ở vị trí cuộn đó.
  Đây là coverage một viewport, không kết luận mất đường tới nút; cần scroll/hitbox ở state
  thực khi qualification presentation mới, không suy từ ảnh này rằng mọi target đã đủ.
- Ảnh own-read 401 giữ count 1 và hiện lý do đăng nhập, trang vẫn ghi rõ LocalFixture.
  Không coi ảnh này là một phiên guest thật hoặc own-content baseline đã mở.

`self-review`, 0 vòng sửa UI (không đổi UI). `NO_APPROVED_REFERENCE`; chưa có capture mới
eligible Off/own-content/pending để so presentation, native/screen reader/OS IME chưa chạy.
Diff/whitespace, 295 relative links trong bốn tài liệu liên quan và JSON metadata đã kiểm hợp lệ.

## Acceptance và năm chiều

Tất cả AC của issue vẫn unchecked; không sửa issue trong lượt này.

| AC | Evidence/gap |
| --- | --- |
| AC-01 | NOT_RUN: contained work Thấm chờ #224; CHOICE-01 A dự kiến, chưa có paired capture mới |
| AC-02 | NOT_RUN: CHOICE-02 A dự kiến; thiếu reply glyph đúng registry; Gửi/composer/Gỡ hiện hữu đã inventory |
| AC-03 | PASS tại SSR baseline: confirmed facts, count hidden/unavailable/known và block; UI sau surface chưa kiểm |
| AC-04 | PASS baseline: 13 assertions browser mới và controller tests có session/stale/ACK fencing. Không kết luận UI mới PASS |
| AC-05 | PASS tại SSR baseline: bốn loại block và can_reply/can_remove root/flat replies; không kiểm quyền bền vững mới |
| AC-06 | PASS baseline Web cho closed/typing/recovery/flat order; không đổi composer. OS IME NOT_RUN |
| AC-07 | NOT_RUN cho presentation mới: responsive/theme/focus/contrast/forced-colors phải kiểm lại sau tích hợp |
| AC-08 | NOT_RUN: chưa có before/after của surface mới; lịch sử own-content không thay eligible baseline |
| AC-10 | NOT_RUN: self-view và CHOICE-03 bổ sung trong issue R2, chưa triển khai |
| AC-11 | NOT_RUN: header/order tĩnh bổ sung trong issue R2, chưa triển khai |
| AC-09 | PASS source boundary của phần chuẩn bị: không fork/đổi primitive/backend/SQL/policy; CMP mới NOT_RUN |

| Chiều | Kết quả/boundary |
| --- | --- |
| Functional | PASS baseline: 3 unit tests và full Web; UI mới NOT_RUN |
| Geometry | PASS baseline theo assertions runner hiện hữu; surface mới NOT_RUN |
| Visual | 5 ảnh baseline đã mở, coverage partial; surface mới NOT_RUN, `NO_APPROVED_REFERENCE` |
| Accessibility/Responsive | PASS baseline theo SSR reasons/count và assertions narrow/text200/keyboard runner; surface mới/forced-colors mới/screen reader thật NOT_RUN |
| Interaction | PASS baseline browser DOM identity/focus/ACK và programmatic composition; OS IME/Android/iOS/TalkBack/VoiceOver NOT_RUN |

Self-review; chưa có vòng render/fix UI mới. Để tiếp tục, fetch base có contract #224 + reply
glyph đúng owner, rebase và áp recipe tại caller trong spec; chạy lại quick/web/preview và
paired capture trước/sau đủ eligible/own/guest, state và ma trận. Không merge/deploy/đóng issue.
