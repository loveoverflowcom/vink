# Evidence follow-up #229

Source measurement/code `4f2883630ff27f36c7549e036e6b554166a86692`, base develop
`ca9369653a516715fea95ca71fbd818d54c15b0a`. [Report/AC matrix](../../../long-document-budget.md#đối-soát-follow-up-229).
Các commit sau source này chỉ bổ sung docs/evidence. Không suy timing của #232 cho HEAD mới.
`manifest.json` pin SHA-256 từng file evidence; `verification.json` pin hash inputs test và kết quả.

| File | Phạm vi và kết quả |
| --- | --- |
| [environment.json](environment.json) | Source/base, host/toolchain, seed, cache/timer boundaries, command và hash executable; noise/giới hạn disk |
| [historical-audit.json](historical-audit.json) | 11 checksum lịch sử khớp; số case/scenes #232 được kiểm lại |
| [postgres.json](postgres.json) | Measurement v2, 21 case, CPU, 420 app saves, exact content checks, plans/WAL/relation sizes/history 1/10/100 |
| [compression.json](compression.json) | Scratch default/pglz/LZ4; same fresh corpus, raw samples, old/new compression |
| [browser.json](browser.json) | PASS 42 Reader/84 editor scenes; actual latest-develop UI, HTTP saves/rejections/conflicts, buffer/selection, oversized adapters/gate; V8/DOM proxy |
| [summary.json](summary.json) | 21/21 corpus exact equality với #232; 315 changed + 105 noop; median/max theo kind, không p95 |
| [pure-before.log](pure-before.log) | PASS 3 pure regressions ở base; production/corpus/pure inputs byte-identical với source measurement |
| [pg-focused-final.log](pg-focused-final.log) | PASS 4 checks trên executable final code; opt-in benchmark ignored ở command này |
| [benchmark-direct.log](benchmark-direct.log) | PASS full probe 570,45 s; SQLx cleanup warning được giữ; không che warning bằng test status |
| [build.log](build.log) | PASS `cargo xtask build` cho bundle/server hiện tại |
| [benchmark.log](benchmark.log) | FAIL môi trường: Cargo linker errno28 cho binary server trước benchmark; PG test executable vẫn build thành công |
| [quick-initial.log](quick-initial.log) | FAIL môi trường: hết đĩa khi tạo archive unit-test; sau đó dọn artifact/task scratch riêng và chạy Quick với jobs=1 |
| [quick.log](quick.log) | PASS đầy đủ `cargo xtask verify quick`; retry cuối dùng `CARGO_BUILD_JOBS=1` |
| [scratch-cleanup.log](scratch-cleanup.log) | Retry DROP thường đúng database của task sau process exit thành công, rồi stop cluster riêng; không FORCE |
| [owned-build-cleanup.json](owned-build-cleanup.json) | Chỉ artifact build thuộc checkout task này được xóa để giải phóng đĩa; không dọn phiên khác |
| [decision-user09.json](decision-user09.json), [decision-wr04.json](decision-wr04.json) | Registry query thực; WR-04 related-only, trạng thái DEFERRED ở PRD |
| [verification.json](verification.json), [manifest.json](manifest.json) | Source/test fingerprints, check results, byte/hash corpus equality và SHA-256 của evidence cuối |
| [initdb.log](initdb.log), [initdb-escalated.log](initdb-escalated.log), [pg-focused-initial.log](pg-focused-initial.log) | Lỗi shared memory sandbox/thiếu DATABASE_URL và setup retry; không coi là runtime regression |
| [contexts.json](contexts.json) | Metadata bốn ảnh poem_tabs-200000: capture diagnostic, chưa mở để kết luận visual; không dùng làm design reference |

Cwd: `/Users/manhblue/Documents/Codex/2026-11-16/task-5/vink`. Ngày 16/11/2026.

```sh
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=3
# DATABASE_URL phải là cluster local scratch có CREATEDB. Không dùng credential production.
cargo test --offline --locked -p vink-app --test document_budget
cargo test --offline --locked -p vink-app --test postgres long_document_ -- --test-threads=1
cargo xtask build
# Command Cargo của probe bị lỗi link server do đĩa đầy; executable integration đã build xong.
# Executable/list/hash/inputs đã đối chiếu trước khi dùng lệnh dưới:
VINK_DOCUMENT_BUDGET_OUTPUT=/tmp/vink-229-followup/run VINK_CHROME="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" \
  target/debug/deps/postgres-17b64a22f9443168 document_budget::long_document_budget --ignored --exact --nocapture --test-threads=1
# Scoped regression được chạy lại trên cùng executable sau probe:
target/debug/deps/postgres-17b64a22f9443168 long_document_ --test-threads=1
CARGO_BUILD_JOBS=1 cargo xtask verify quick
```

Lệnh tái tạo portable vẫn là Cargo probe ở report; tên executable trên là **lịch sử lần chạy**,
không API ổn định. Compiled executable được dọn sau khi ghi hash/evidence để đủ đĩa cho Quick.
Raw input fixture 26 MB chỉ tổng hợp ở `/tmp/vink-229-followup/run/fixtures.json`, không commit.
Log scratch được giữ; database benchmark đã DROP bình thường, cluster đã stop và data directory
riêng của task được dọn để đủ đĩa cho Quick. Không chạm cluster/data của phiên khác.

NOT_RUN: full PG/Web lanes, release/production, physical mobile, OS IME, screen reader,
Safari/Firefox, cold OS/PG cache, total RSS/WASM peak và Quint/Kani/CMP. Không đọc/poll GitHub CI.

Đã resume trên checkout Mac gốc; chỉ hoàn thiện docs/evidence sau source measurement. Quick
đã chạy xong trước khi tạm dừng, dùng lại vì code/test không đổi; final links/JSON/checksums
được kiểm riêng. Không nhập nháp dot, không gán Mac timing cho dot computer.
