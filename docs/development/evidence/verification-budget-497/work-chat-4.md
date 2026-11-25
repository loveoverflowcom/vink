# Kiểm tích hợp theo diff · lượt 4 của #496

Bản ghi đợt 4 GiB trên base `4d97d6a4d57eab22880f9a5c2eaba7897262c5e9`, đã chứa #509–#511. Các lỗi Editor/Desk dưới đây là kết quả của lượt đầu. Xem đối chứng, số đo 4.864 MiB và các blocker hiện tại trong [bàn giao cập nhật #512](work-chat-4-followup.md). [Draft PR](https://github.com/loveoverflowcom/vink/pull/512).

## Điều kiện đo

Mac arm64, RAM 24 GiB. Khóa admission Vink được giữ trước khi bật VM AppleHV riêng 6 GiB/2 vCPU. Mỗi lúc có một workload trong cgroup rootless 4 GiB, swap 0, CPU quota 1, Cargo jobs 1 và libtest threads 1. VM không có swap. Cargo/rustc 1.99.1, bindgen 0.2.129, Node 22.15.0, Chromium 156.0.8078.108, PostgreSQL 15.20. Dev/test debug=0, incremental=0. Registry, npm cache và toolchain scratch đã có. Target dành riêng cho lượt này ban đầu trống, volume còn hơn 23 GiB. Page cache của OS có thể ấm. Browser dùng profile và dữ liệu scratch mới; App, Identity (khi cần), PostgreSQL, Chrome và Cargo đều nằm trong cgroup. Container browser chỉ có mạng nội bộ. Chromium dùng chế độ headless CI có sẵn sau khi kiểm chroot mặc định thất bại.

Wall ở từng dòng bao gồm lệnh và các bước con tương ứng. RSS là `ru_maxrss` lớn nhất của child đã chờ, tính bằng KiB. `memory.peak` tính cả cgroup từ khi tạo container, gồm file cache. Hai số có phạm vi khác nhau. Lượt đầu phải bổ sung Git metadata cho source export và chuyển cache của Chromium ra ngoài checkout. Các ảnh bàn giao có SHA và trạng thái Git sạch trong manifest. [Mẫu stdout theo lượt](work-chat-4-samples.txt).

## Planner và selector

- `cargo xtask verify changed --base BASE_SHA --head HEAD_SHA --plan` kiểm hai cây commit. Với `--worktree`, xtask còn đọc index, thay đổi chưa stage và tệp mới ngoài ignore. Hai tên của rename và owner cũ của deletion đều được xét. Shared/unknown mở rộng sang các lane, gồm các fixture release opt-in. Người review đối chiếu thêm native, formal và gate theo AC.
- Static docs đã chạy qua selector thật với `--from-diff BASE_SHA HEAD_SHA committed`. Thêm Cargo.lock vào fixture làm lệnh đã pin bị từ chối. Chế độ worktree lập lại plan. Hai đầu trùng nhau và cây sạch báo không có coverage. Ba lượt lập plan cho một thay đổi docs mất 0,005 / 0,005 / 0,006 s, RSS 9.512–9.516 KiB, đo binary đã bootstrap.
- Bốn test planner kiểm ma trận input và workspace manifests, cây Git tạm, rename/delete/untracked, invalid ref và file index. Đối chứng bỏ dependency Expressions → Preview làm test thất bại với đúng owner; khôi phục rồi chạy lại. Review caller phát hiện fixture Desk rỗng còn được `passwordAuth` dùng trước sign-in; đã thêm selector password và regression. `verify web` mặc định bỏ hai fixture release, nên fallback rộng còn liệt kê chúng riêng.
- `cargo test --locked --offline -p xtask --bin xtask changed::tests`, `cargo test --locked --offline -p xtask --test scoped_cli`, clippy xtask all-targets với `-D warnings`, fmt, static docs/tokens và 16 test Node visual/appearance đã qua trong lượt này. [Hợp đồng lệnh và ví dụ](../../scoped-workflows.md).

## Kết quả sản phẩm và chi phí

| Selector hoặc test | Điều kiện và kết quả | Wall |
| --- | --- | ---: |
| `verify unit store` | Hai lần chạy đủ 5 test trên target đã có artifacts. RSS lần lượt 483.948 và 576.448 KiB. Cargo vẫn compile trong discovery. | 14,267 / 28,441 s |
| `verify postgres store --filter fresh_database_migrates_once_and_reruns_as_a_no_op` | PASS 1 test qua discovery, PG scratch và migration thật. | 5,172 s |
| `cargo test --locked --offline -p vink-app --test ssr_boundary` | PASS 1, ignored 1 opt-in. Phần lớn thời gian là compile/link. RSS 1.813.132 KiB. | 72,231 s |
| `verify web --scenario restart --capture full` | Cùng target ấm, dời riêng receipt rồi chạy cùng lệnh. Dev hydrate MISS và HIT đều PASS qua stop/start và kiểm nháp/tag đã lưu. RSS 4.111.640 / 1.146.488 KiB. | 22,425 / 15,657 s |
| `verify web --scenario editor-first-draft` | Cùng package ấm. Full và checks đều PASS hai format trên hai principal mới. Full tạo 2 PNG + 2 context + 2 summary. Checks tạo 0 PNG/context/summary. | 17,121 / 17,280 s |
| `verify web --scenario standalone --capture checks` | Prefix mới trên SHA Web cuối, MISS/HIT đều PASS với Identity/outbox thật. Build đầu của prefix và target ấm có chi phí khác nhau. | 68,140 / 23,494 s |
| CSR với riêng receipt bị dời trên target ấm | MISS/HIT đều PASS cùng SHA, flags và fixture. RSS 3.013.188 / 809.940 KiB. | 29,022 / 23,294 s |
| `verify web --scenario editor --capture full` | FAIL placement của thao tác thử lại sau khi lưu tag, sau các bước dirty editor/conflict. | 60,197 s |
| `verify web --scenario desk --capture full` | FAIL ở kiểm counter/read-only. | 45,042 s |
| `verify web` mặc định | Dừng trong READ-01. PASS prerequisites, package, secret scan, tag fixture, hai first-draft và các assertion Reader SSR/ETag/404, gồm trường hợp WASM lỗi. Các bước sau NOT_RUN. | 323,138 s |
| `verify web --scenario perf-baseline --release --capture full` | FAIL build `vink-web` feature `perf-probe`, profile `wasm-release`. rustc nhận SIGKILL, cgroup ghi `oom_kill=1`; RSS 4.068.488 KiB. Browser, publish, release HIT và 10 cold/10 warm journey NOT_RUN. | 494,369 s |

Store/SSR, cặp restart và full Web dùng `888b7534f39725da553b57221f8be78151aba65c`. Digest của danh sách checksum package restart là `4fdcca126d1464b4a63507f532c5a3f514e44a69966db976d151449d6c9531c5` ở cả hai lượt; đỉnh cgroup trong phiên 4.295.897.088 byte. CSR đầu tiên trên SHA này timeout lúc tài khoản thứ hai mở hồ sơ công khai (267,630 s), lần HIT kế tiếp PASS (22,064 s). Giữ cả hai kết quả; chênh lệch này không đo chi phí bindgen.

Lượt Web cuối dùng `6e38dc98c4d454e4d4b4613b61ed90049146dadf`. Cặp CSR có đối chứng kiểm đúng prefix SHA này. Danh sách SHA-256 của package có cùng digest `2d2bf171f6c65c0d3a22d20a936287d4b2129619244c62b7615b59d7efc181f9` ở cả hai lượt. Hai commit code tiếp theo chỉ thay đổi fallback/fixture mapping trong planner và test, không đổi bundle, Browser, App hay fixture. CSS luôn chạy token check (16 outputs), Tailwind và kiểm output; Editor luôn chạy typecheck, 30 title + 5 render corpus, outline, tài liệu 200k và bundle. Chưa thêm cache output cho hai owner khi chưa chứng minh đủ generator, dependencies/locks, env/toolchain và validator/corpus.

Cgroup ở full Web ghi `oom_kill=1`, memory PSI full avg10 42,81%, pids-limit không bị chạm. Sau khi Reader ngừng tiến triển, gửi TERM vào đúng xtask để dọn nhóm child. Lượt release lặp lại cùng cấu hình đã được dừng sau 44,062 s; không dùng checksum của package dev còn trên đĩa làm bằng chứng release. Full quick, full PostgreSQL, formal, native và compiler cache liên worktree chưa đo lại. [Baseline #509](README.md) chỉ đo package Rust nhỏ. #511 đo riêng warm hydrate dev HIT 4,77–4,93 s và forced MISS 10,78–11,32 s. Các số trên không xác lập tốc độ cho toàn workspace.

## Review UI trên ảnh thật

- Editor hiện trạng thái “Chưa lưu chữ”, nội dung tiếng Việt và vùng chọn trên nền tối ở 390 px. Fixture đặt theme và đợi màu ổn định, focus select trước thao tác mô phỏng thay đổi tag. Oracle kế tiếp yêu cầu `#tags-retry` nằm ngoài mọi `details,dialog`; production `work_info.rs` đặt nó trong `details#work-options`. Cần owner chốt vị trí action và kiểm lại nguyên assertion. [Editor 390 px](visual/editor-dark-390.png).
- Ở 320 px và text-scale 200%, hai ảnh hiển thị đúng lựa chọn “Mặc định”/“Markdown”, nút “Viết bản thảo đầu tiên” và điều hướng. Context xác nhận theme sáng/tối tương ứng, nguồn sạch và SHA-256 PNG. Đây là lane text-scale DOM/geometry của Chrome. [Mặc định](visual/first-draft-default-320.png) · [Markdown](visual/first-draft-markdown-320.png) · [Manifest ảnh](visual/contexts.json).
- Desk có 6 tác phẩm trong fixture. Ảnh 1440 px và caller `ReviewTrigger` xác nhận có nút “Xem kiểm duyệt” trên từng hàng. Oracle `author-works-smoke.mjs` cấm mọi `#draft-results button`. Cần phân xử phạm vi counter và vị trí action trước khi nghiệm thu. [Ảnh lỗi Desk](visual/desk-controls-1440.png).

## Các lệnh tiếp tục

Trên host đã có Chromium/Node/npm, bindgen 0.2.129, target wasm32, test PostgreSQL và admission chứa toàn bộ child, chạy từng selector. Với release hoặc full Reader, cần môi trường đã được phê duyệt và chứng minh đủ RAM cho profile/fixture gốc. Giữ nguyên assertion, release flags, secret scan và scope PG.

```sh
cargo xtask verify changed --base origin/develop --worktree --plan
cargo xtask verify static docs --from-diff BASE_SHA HEAD_SHA committed
cargo xtask verify web --scenario editor --capture full
cargo xtask verify web --scenario desk --capture full
cargo xtask verify web
cargo xtask verify web --scenario perf-baseline --release --capture full
```

## Đối chiếu acceptance

| Issue | Theo AC |
| --- | --- |
| #496 | AC-01 PARTIAL về chi phí đại diện; AC-02 có evidence selector + Store thật; AC-03 PARTIAL do Web; AC-04 PARTIAL vì output cache CSS/Editor và runtime còn gap; AC-05 có fixture, guard và negative control; AC-06 PARTIAL về cross-worktree/compiler cache; AC-07 có policy và tích hợp thực, giữ các gap dưới đây. |
| #497 | AC-01 inventory theo owner từ #509–#511 và plan hiện hành. AC-02 PARTIAL: có baseline target trống, cold/warm nhỏ, cặp restart warm có đối chứng; thiếu hồ sơ mọi bước trên cùng workload. AC-03 raw logs và số đo phân scope; không suy hiệu năng toàn workspace. |
| #498 | AC-01 static docs/tokens và compile/check selection. AC-02 Store 5 runnable, zero/ignored-match fail-closed trong #510. AC-03 selected Store thật, App scratch và migration trong Web. AC-04 giữ inventory quick/postgres và CLI negative, full quick/Postgres không chạy lại trong lượt này. |
| #499 | AC-01 có catalog, alias và plan. AC-02 restart PASS, Editor/Desk FAIL ở oracle cụ thể. AC-03 full union đã thử, dừng ở Reader sau oom_kill; phần sau NOT_RUN. AC-04 có PNG/manifest thật, đã đối chiếu full/checks ở trên; chưa có so sánh selected/full cùng coverage đã hoàn tất. |
| #500 | AC-01 có HIT/MISS và digest bindgen/static, npm install stamps. AC-02 PARTIAL: CSS/Editor luôn chạy gate; chưa chứng minh closure để cache output. AC-03 dùng lại negative corruption/rollback và lock của #511 trong đúng boundary. AC-04 hydrate restart và CSR đều PASS fresh/reused theo digest thực; release/perf-probe còn NOT_RUN ở runtime. |
| #501 | AC-01 pin SHA, range/worktree, rename/delete/untracked; fixture kiểm index không đổi. AC-02 ma trận mọi Rust owner, docs/CSS/SQL/contracts/locks/unknown. AC-03 test selector và executor thật với `--from-diff` trên SHA tích hợp; committed có thay đổi bị từ chối, worktree lập lại plan. AC-04 negative bỏ dependency thất bại đúng chỗ; không đổi full/formal/merge gate. |
| #502 | AC-01 admission giữ trước Cargo, giới hạn cgroup/slot đã kiểm. AC-02 PARTIAL: hai worktree nhỏ và debug0 ở #509, thêm số liệu trong một worktree tích hợp. AC-03 launcher có isolation/cancel từ #509, cross-toolchain/compiler cache PARTIAL. AC-04 không mở rộng support của launcher; VM riêng chứa cả PG/Chrome, local-budget macOS tiếp tục báo browser/CMP unsupported. |
| #503 | AC-01 central matrix và command contract, local docs/metadata/links/format kiểm đạt, AGENTS 20.845 byte. AC-02 có ví dụ docs, CSS, Rust, SQL, auth/shared và phân biệt edit/draft/review/integration. AC-03 provenance theo input, target, toolchain, fixture, claim; phân biệt HIT và kết quả assertion. AC-04 có nghiệm thu từng lát, log gốc, cặp có đối chứng và gap định danh; Epic còn mở. |
