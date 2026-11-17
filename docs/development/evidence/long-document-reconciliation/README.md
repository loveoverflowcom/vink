# Checkpoint đối soát Long Document

Mac bị mất kết nối trong lượt full Quick cuối. Đây là summary từ output đã quan sát, **không phải raw log được tái tạo**. [checkpoint.json](checkpoint.json) chứa SHA/commands/results/limitations; [ma trận24 AC](../../long-document-reconciliation.md) dẫn tới raw historical evidence đã có trong develop. Full raw mới tại Mac chưa upload nên PR phải giữ draft.

## Đã quan sát

- PASS corpus3; focused real PostgreSQL/HTTP15 passed,2 opt-in ignored tại6287fbf.
- PASS fmt và decision validate;126 revisions/88refs, snapshot9dbad856…; current storage PENDING/current=null.
- PASS audit51 historical checksums,21 corpus/420 saves,768 storage samples,13 projection checkpoints, current fingerprints và merge ancestry. Đây không là benchmark mới.
- FAIL101 desired policy re-review flow, đúng pending constraint; test tạm phục hồi byte-identical, owner #275.
- Quick trước đó FAIL vì disk; last Quick tạif6b8846 đã qua fmt/3Clippy/doctest/SSR, unit/contract đang build lúc lần đọc cuối. **Final Quick UNKNOWN**, repository step/exit unknown sau disconnect.

## Tài nguyên và source còn ở Mac

Worktree: `/Users/manhblue/Documents/Codex/2026-11-17/task-8/vink-reconciliation`, branch `codex/long-document-final-reconciliation`, HEAD last observedf6b8846, dirty docs/evidence chưa local commit/push. Full bundle nằm dưới `docs/development/evidence/long-document-reconciliation`, gồm audit.py.txt/audit.json, decisions/environment/preservation, corpus/PG/Quick logs, các FAIL disk và policy patch/log/PG excerpt. Giữ các file đó; đừng thay bằng summary này nếu raw còn có thể phục hồi.

Target riêng được APFS clone từ task-7 clean tại9c603b2; chỉ target của task này đã được dọn/tái tạo sau process/lsof/provenance/recovery checks. Không clean foreign targets/worktrees hoặc DB. Cluster riêng `/tmp/vink-long-reconciliation-20261117/pg`, port55538, trạng thái cuối chưa biết; không stop/clean dựa vào PID cũ. Final build có thể vẫn chạy.

Final profile task-local ở thư mục cha task-8/.cargo/config.toml: App/Web strip=symbols,opt-level1; dev/test debug0,incremental0,jobs1,libtest1. Debug assertions/overflow checks/features/selectors/assertions giữ nguyên. Không dùng profile này đo timing. Corpus trước đó dùng launcher default2 libtest threads.

## Tiếp tục đúng một PR

1. Kết nối lại Mac; kiểm process/slot/disk, git status/HEAD và raw Quick. Không giả run đã chết hoặc đã PASS.
2. Bảo toàn local dirty docs và toàn bộ bundle ngoài target; fetch remote branch checkpoint trước commit/push. Reconcile local edits với checkpoint trên cùng branch bằng commit/merge/rebase có kiểm; không reset/stash/force-push, không tạo PR khác.
3. Upload raw, exact environment/profile, audit script/results và preservation records; không nhận summary là raw. Kiểm final Quick đủ mọi bước trên source phù hợp. Nếu chưa PASS, sửa prerequisite trong tài nguyên task rồi chạy đủ gates, không hạ assertions.
4. Fresh-fetch develop, đối chiếu owner delta; source inputs đổi thì chạy checks tương ứng. Không lấy PASS cũ cho owner đã đổi. Kiểm diff/links/JSON và pending decision references.
5. Self-review same PR; chỉ merge khi local gates PASS, xác nhận merge commit trong develop. Sau đó fresh-read issue/comments, giữ concurrent edits/wording, tick đủ evidence, comment SHA/PR/gaps/limits và close/readback khi đủ điều kiện. Không CI polling/rerun hoặc deploy.
