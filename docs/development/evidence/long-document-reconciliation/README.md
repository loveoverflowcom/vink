# Evidence đối soát #229–#231

**Source Quick mới: `dcd36ba6883b49bf764124e08e5d78278a485e48`**, base develop
`607ef83aa2824a16ec54a6f25bd10eb4c137d62f` sau #281. Corpus/PG tại `11925bfc3cfd901e6b5b7e2c46ec08fb8ee8957e`;
[OID proof](upstream-281.json) xác nhận toàn backend/codec/DTO/cache không đổi sau #281. Mac arm64 ngày 17/11/2026.
**LOCAL GATES PASS:** fmt, decision validate và full Quick đã chạy PASS tại source trên.
[Source/environment](resume-20261117T093542Z/source.json), [results](resume-20261117T093542Z/results.json),
[raw Quick](resume-20261117T093542Z/quick.log), [input reuse proof](resume-20261117T093542Z/input-reuse-proof.json).
Corpus 3 / PG 17 PASS tại11925bf giữ source thật và OID proof; không đo benchmark/browser mới.
GitHub là SSOT cho merge/checklist/closure; không suy trạng thái đó từ local PASS.
Hai benchmark PG opt-in giữ ignored; full Quick trước tại 11925bf có một scratch-lock test xtask vốn ignored.
[Verification](verification.json), [exit codes lịch sử c2a3073](gate-exit-codes.json), [ma trận24 AC](../../long-document-reconciliation.md).

Chủ repo đã merge checkpoint #278 (`d352dba`) khi Mac mất kết nối và gate cuối còn UNKNOWN.
Raw phục hồi xác nhận lượt đó FAIL disk. Nhánh follow-up mới
`codex/long-document-reconciliation-evidence` xuất phát từ develop đã merge, rồi lấy #279.
Sau đó tích hợp nguyên #280/#281 và kiểm delta. Không push/chỉnh PR #278. [Checkpoint](checkpoint.json) giữ nguyên bytes đã merge, là lịch sử
outage; các raw/results mới bổ sung ở đây. Trạng thái issue sống tại GitHub, không tại bundle này.

## Files và provenance

| Evidence | Nội dung và giới hạn |
| --- | --- |
| [verification.json](verification.json), [quick-run-source.json](quick-run-source.json), [focused-run-source.json](focused-run-source.json) | Command, source, flags/config hash, input tree OIDs và từng PASS/FAIL/NOT_RUN |
| [environment.json](environment.json), [github-inputs.json](github-inputs.json) | Host và fresh-read đầu phiên tại 6287fbf; không gán source mới cho lần chạy cũ |
| [fmt.log](fmt.log), [decision-validate.log](decision-validate.log), [corpus.log](corpus.log), [postgres-focused.log](postgres-focused.log), [quick.log](quick.log) | Raw corpus/PG tại 11925bf và Quick tại c2a3073; actual exits trong JSON; không benchmark mới |
| [audit.py.txt](audit.py.txt), [audit.json](audit.json), [audit.log](audit.log) | One-shot tính lại 51 checksums, 21 corpus, 420 saves, 768 storage samples/2runs, 13 projection checkpoints; ancestry và owner deltas |
| [decisions.json](decisions.json), [decision-query.log](decision-query.log), [storage query](storage-decision-current.json), [source-format query](source-format-decision-current.json) | Snapshot `8354df2c`; storage và source format đều PENDING/current=null; không signoff |
| [decisions-before-279.json](decisions-before-279.json), [upstream-delta.json](upstream-delta.json), [upstream-279.json](upstream-279.json) | Snapshot `9dbad856` trước #279 và delta nguồn #276/#277/#279; #280/#281 đã merge và được giữ nguyên; không lấy browser evidence upstream làm kết quả task này |
| [known-policy-reproducer.patch](known-policy-reproducer.patch), [log](known-policy-reproducer.log), [result](known-policy-reproducer.json), [PG excerpt](known-policy-postgres-excerpt.log) | Desired flow FAIL 101 tại6287fbf, đúng pending constraint; test tạm đã phục hồi byte-identical; owner #275 |
| [corpus-f4e917b-compile-fail.log.gz](corpus-f4e917b-compile-fail.log.gz) | Assertion mới dùng nhầm `str::len` cho `&String`; sửa closure tại c27286d, không bỏ assertion; corpus 3 sau sửa PASS |
| [quick-disconnected-final-disk.log](quick-disconnected-final-disk.log), [afb8a09 strip failure](quick-post-recovery-strip-fail.log.gz), [d352dba App failure](quick-d352dba-app-strip-disk.log.gz), [d352dba archive failure](quick-d352dba-web-archive-disk.log.gz) | FAIL môi trường được giữ; UI preview/App output hỏng không executable, hoặc archive hết đĩa; later steps NOT_RUN |
| [initial](quick-initial.log), [retry](quick-retry-disk.log), [global strip](quick-global-strip-disk.log), [package strip](quick-package-strip-disk.log.gz), [Web opt](quick-web-opt-disk.log.gz) | Các FAIL disk trước đó; không dùng partial PASS làm full Quick PASS |
| [preservation.json](preservation.json), [cleanup.json](cleanup.json), [disk-recovery.json](disk-recovery.json), [reconnect-recovery.json](reconnect-recovery.json), [post-merge-recovery.json](post-merge-recovery.json) | Process/lsof/dirty/provenance/recovery; chỉ target riêng task, giữ foreign worktrees/cache/DB |
| [size-profile-recovery.json](size-profile-recovery.json), [web archive recovery](web-archive-recovery.json), [App artifact](strip-artifact-recovery-app.json), [preview artifact](strip-artifact-recovery.json) | Đúng artifact lỗi/obsolete được dọn sau no-open-handles; source/lockfiles/raw/backup giữ ngoài target |
| [task-build-config.toml.txt](task-build-config.toml.txt), [before size](task-build-config-before-size.toml.txt), [before preview](task-build-config-before-preview.toml.txt) | Config task-local ngoài repo: App/Web opt-level="z", UI preview1, strip symbols; debug assertions/overflow checks=true |
| [compression script](compress-task-artifacts.py.txt), [latest compression](artifact-compression.json), [before #279](artifact-compression-before-source279.json), [before linker attempt](artifact-compression-before-linker.json) | Native filesystem compression giữ bytes/SHA/mode/mtime; từng lượt còn lại nằm trong manifest; cache nguồn không đổi |
| [linker-attempt.json](linker-attempt.json), [launcher diagnostic](linker-capability.log) | Wrapper variant NOT_RUN: launcher UNSUPPORTED 69. Tiny baseline probe không là AC evidence; wrapper bị bỏ, không bypass guard |
| [self-review](self-review.md), [manifest](manifest.sha256), [raw encodings](raw-log-encodings.json) | Review riêng của executor, hashes toàn bundle và byte-exact gzip round trip |

[Delta #280](upstream-280.json) / [delta #281](upstream-281.json) phân biệt các source.
[Git bundle phục hồi](local-recovery-snapshots.bundle) có [ref/prerequisite/checksum](local-snapshots.json),
đã `git bundle verify` PASS; giữ các source commit local cũ mà không push nhánh PR #278.
[Preflight75](quick-c2a3073-preflight-75.log) là NOT_RUN do memory warning, không compiler/test failure;
guard được giữ, retry khi normal. [Cluster riêng](own-resource-release.json) đã dừng sau khi
xác minh đúng data/port và không client khác; data/log giữ lại, foreign/system processes không bị dừng.

## Commands thực tế

Cwd là repository root. Cargo/Rust 1.99.1; PostgreSQL 17.11 riêng ở 127.0.0.1:55538.
Parent `task-8/.cargo/config.toml` là config tạm có copy ở trên; không đổi repo manifest hoặc
Cargo home của phiên khác. Không dùng profile này để đo latency. Không có rustc wrapper.

```sh
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0
export CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1
./tools/local-budget run -- cargo fmt --all -- --check
./tools/local-budget run -- cargo xtask decision validate
./tools/local-budget run -- cargo test --offline --locked -p vink-app --test document_budget
DATABASE_URL='postgres://manhblue@127.0.0.1:55538/postgres' \
VINK_TEST_DATABASE_URL='postgres://manhblue@127.0.0.1:55538/postgres' \
./tools/local-budget run -- cargo test --offline --locked -p vink-app --test postgres -- \
  long_document_ publication_state:: content_storage_rfc:: conditional_reads:: \
  document_source_compat:: --test-threads=1
./tools/local-budget run -- cargo xtask verify quick
python3 docs/development/evidence/long-document-reconciliation/audit.py.txt
target/debug/xtask decision query VDR-CONTENT-STORAGE
target/debug/xtask decision query VDR-DOCUMENT-SOURCE-FORMAT
```

Audit cần Git objects lịch sử ghi trong script (#232/#249/#248/#253 và baseline trước projection);
checkout shallow cần fetch các PR heads/history trước. Script là evidence one-shot, không CLI/framework
sản phẩm. Hash test file được ghim để audit vẫn chạy khi source commit của executor được squash.

Corpus3 gồm oracle mới cho 21 hash/byte/shape/envelope từ raw #249, boundary±1 và oversized cache
fallback. PG 17 gồm budget 4 (có 2 negative controls), projection 6, storage 2, conditional 3 và compat 2
của #279. SQLx tạo fresh databases trong cluster riêng. Scratch storage/compat constraints và commit
visibility không chứng nhận production composite FK, GC/publish races hoặc migration ứng viên.

## Giới hạn và phục hồi

Raw benchmark/browser giữ source đo của #229/#249/#248/#253. #277 đổi UI/export; #279 thêm nguồn/hash
v2 cô lập và chia sẻ helper hex. Writer/API/fixtures vẫn v1; oracle lịch sử chứng minh v1 không drift.
Không lấy current test timing, Chrome cũ hay probe SQL scratch làm release/production qualification.
Storage và source-format proposals cùng PENDING; giữ JSONB+TOAST v1, cache gate tắt, WR-04 DEFERRED.
Nếu writer/encoding đổi sau này, cần đo lại workload và gates B/C trước adoption.

Đĩa ban đầu khoảng 1,9GiB trống. Target riêng APFS-clone từ task-7 sạch tại 9c603b2; original cache
không bị clean. Các lần archive/strip hết đĩa và artifact data không executable giữ raw; không chmod
file hỏng để coi như PASS. Dọn đúng artifact task sau process/lsof, giữ source/lock/profile/raw/backup.
Nén native giữ byte/mode/mtime; những cache copy chưa rõ khả năng thu hồi được giữ. Profile cuối
opt-level="z" cho App/Web giảm footprint, debug/overflow checks và toàn bộ selectors/assertions vẫn giữ.
Launcher từ chối wrapper đã thử; không sử dụng hoặc lách guard đó.

Desired policy re-review vẫn FAIL: approved → private → policy mới → request-public revision cũ.
Method/migration byte-identical trước projection, baseline full flow cũ NOT_RUN. Không sửa guard,
applied migration hoặc đổi failure thành PASS; [#275](https://github.com/loveoverflowcom/vink/issues/275)
sở hữu additive migration/compatibility. F1 HTTP budget thuộc [#273](https://github.com/loveoverflowcom/vink/issues/273),
F3 nhiều paragraph thuộc [#274](https://github.com/loveoverflowcom/vink/issues/274).

**NOT_RUN:** full PG/Web lanes, benchmark/browser mới, mobile/OS IME/screen reader/Safari/Firefox,
total browser RSS/WASM peak, release/production/cold-cache, candidate GC/migration/upgrade,
Quint/Kani/CMP. PR chỉ tăng oracle test và docs/evidence; không đổi runtime/contract/transition/model.
Không đọc/chờ/poll/rerun GitHub CI, không deploy, không đổi production DB hay tự cấp signoff.

Blocker lịch sử trước resume (đã giữ raw): [raw FAIL](quick.log), [frozen FAIL](quick-c2a3073-doctest-archive-disk.log.gz), [recovery](c2a3073-resource-recovery.json). Sau phục hồi chỉ còn khoảng 615 MiB, ít hơn lượt 885 MiB vẫn thất bại. Fseventsd hệ thống quan sát khoảng 12 GB RAM; không kill hoặc can thiệp. Không còn thao tác thu hồi ngoại vi được phép; giữ worktree/evidence để resume khi tài nguyên đủ.

## Resume sau cleanup ngày 17/11

Parent bảo toàn evidence tại `/Users/manhblue/vink-cleanup-evidence/20261117-scheduled/`;
resume bắt đầu với khoảng6,4GiB trống. Không `cargo clean` hoặc xóa thủ công target thiếu CACHEDIR.TAG.
Không sửa/dừng process, cache, DB hay worktree khác. Cluster riêng55538 vẫn dừng, data/log giữ lại.
Lượt mới dùng cùng profile, launcher, suite và assertions; [raw/results](resume-20261117T093542Z/results.json)
ghi source, giờ, exit và disk/pressure. Các FAIL cũ ở root vẫn là FAIL lịch sử.
