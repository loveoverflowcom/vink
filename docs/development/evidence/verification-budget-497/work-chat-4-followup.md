# Đối chứng tích hợp Draft #512

## Cập nhật focus Desk

Trên base `4d97d6a4`, A `a8578e7b` có product của Draft `3bd55127` và regression mới. Khi đóng dock, regression chọn một link bản thảo trước khi callback phục hồi chạy. Chrome xác nhận link ban đầu đã nhận focus; A FAIL vì callback lấy mất focus sau paint. B `982e780596fc92f621834f3e26acbe23f1162bab` chỉ sửa hành vi phục hồi của Desk và thêm kiểm fallback khi quyền đọc bị thu hồi. [Hợp đồng focus Bàn viết](../../../design/redesign-r4/desk-moderation.md).

Trong callback, Desk kiểm lại route, phần tử đang giữ focus và các control hiện có sau khi render. Control đang dùng được giữ focus. Khi control bị gỡ hoặc ẩn, Desk chuyển về ô tìm kiếm, launcher hoặc tiêu đề theo trạng thái hiện hành. Khi nhận 401/403 và focus đang trong Desk, cùng cơ chế xử lý việc vùng riêng bị gỡ. Phép kiểm Escape, Back, Tab/Shift+Tab, WorkId, focus ring và title không bị clamp ở 390 px/200% vẫn chạy.

| Lượt Chrome, App và PG scratch | Wall (s) | Max RSS child (KiB) | Peak cgroup (byte) | oom_kill |
| --- | ---: | ---: | ---: | --- |
| A · Desk checks · FAIL tại regression mới | 18.030 | 1146348 | 4697227264 | 0→0 |
| B · Desk checks · PASS | 254.322 | 4112916 | 5100576768 | 0→0 |
| B · Desk full · PASS | 164.243 | 1146600 | 5100568576 | 0→0 |

Hai lượt B chạy toàn bộ Desk gồm Desk khách, bốn tổ hợp title 320/390 px ở 200%, hai theme, cùng fallback heading và focus khi search riêng bị thu hồi. Với danh sách rỗng, control đã ẩn rời AX tree, Tab từ heading vẫn vào Select. B build lại WASM và App sau thay đổi Rust. Digest danh sách package A là `4fdcca126d1464b4a63507f532c5a3f514e44a69966db976d151449d6c9531c5`, hai lượt B trùng `a50647d91d616e98f213385545fa64ad14d9ac7d98b396591fee9b242df0a0c7`. Cùng VM 6 GiB, một cgroup 4.864 MiB/1 CPU mỗi lượt, swap 0, Cargo jobs 1. Thời gian ở đây gồm những bước build khác nhau.

Ở ảnh 390 × 900, light/200%, viền focus màu tím bao quanh đúng link bản thảo. Tiêu đề xuống nhiều dòng và còn tiếp dưới mép màn hình. Các assertion Chrome kiểm đúng WorkId, style và clamp ở cả bốn tổ hợp. [Ảnh focus Desk](visual/desk-draft-focus-390-200.png) · [Manifest ảnh](visual/contexts.json).

**Trạng thái.** Blocker focus của Desk trên #512 đã có đối chứng FAIL/PASS. Planner, guard và hướng dẫn có evidence riêng ở các mục dưới. Epic #496 còn thiếu full Web union và phần runtime của Reader, Editor Format, Appearance theo từng biên đã ghi. App release native đã compile/link; WASM release/perf-probe trước đây FAIL do OOM, publish/browser release và 10 cold/10 warm NOT_RUN. Phần release cần chạy profile nguyên bản trên môi trường đủ RAM dưới ngân sách đã cấp. Các kết quả bên dưới mô tả lượt trước bản sửa focus.

## Đối chứng trước bản sửa focus

Fetch lại `origin/develop` cho `4d97d6a4d57eab22880f9a5c2eaba7897262c5e9`. Hai predicate gây lỗi có mặt nguyên văn trên develop và A. Product source của Editor/Desk, App và Leptos patterns giống nhau từ develop đến B. A là `3d69c648bb9521d2e98363983c061eadacdad68f`; B là `8bfacb47f42148a1338be8b5716203bf21b947c3`.

| Cùng lệnh trên A và B | A | B | Dữ kiện |
| --- | --- | --- | --- |
| `verify web --scenario editor --capture full` | FAIL, 60,055 s | PASS, 28,016 s | Recovery thẻ trực tiếp trong bảng mở, nút enabled/visible, ngoài dialog và không có disclosure xen giữa. Sau click, 409 đối soát version 5, không ghi lần hai và giữ lựa chọn mới. |
| `verify web --scenario desk --capture full` | FAIL, 12,023 s | PASS, 164,168 s | Metric chỉ đọc, không có target lồng nhau. Mỗi hàng có một nút mở kiểm duyệt theo đúng WorkId; link đọc và editor độc lập. Cả Desk có principal và Desk khách đều hoàn tất. |

Cả bốn lượt dùng Chromium 156, PG 15.20, cùng fixture recipe và package đã xác minh. Digest danh sách SHA-256 là `4fdcca126d1464b4a63507f532c5a3f514e44a69966db976d151449d6c9531c5` ở cả bốn lượt. Thời gian FAIL kết thúc tại assertion; thời gian PASS bao gồm các bước sau và teardown. Đã mở ảnh Desk sáu hàng, 1440×900 light/100%, và leave guard Editor 320×800 dark/200%. Hành vi khi thử lại và kết quả ghi DB lấy từ thao tác và assertion thật.

[#214 AC-08](https://github.com/loveoverflowcom/vink/issues/214) giữ counters chỉ đọc và tránh nested targets. [#477 AC-02/03](https://github.com/loveoverflowcom/vink/issues/477) đưa kiểm duyệt lên mỗi card, được triển khai bởi #489. [#254 AC-05/06](https://github.com/loveoverflowcom/vink/issues/254) bỏ export và giữ retry/reconcile; [#475 AC-01..04](https://github.com/loveoverflowcom/vink/issues/475) đưa metadata và owner recovery hiện hữu vào native disclosure. Hai oracle ban đầu có từ `3e10fddfc` / `6bfbb63b2`, trước hai delta này. [Editor R4](../../../design/editor-r4.md) · [Desk R4.4](../../../design/redesign-r4/desk-moderation.md).

**Planner.** Fixture `author-works-smoke.mjs` có caller trong Desk nên chọn đúng `verify web --scenario desk`. CLI từ chối môi trường chuyển hướng checkout/index cho cả plan và `--from-diff`. Điều này tránh trường hợp Git đọc từ repository khác với root mà executor thực sự kiểm. Phép kiểm guard trên range của PR chạy từ checkout thật.

**Giới hạn máy.** Một slot, VM Vink 6 GiB/2 vCPU (guest MemTotal 6.034.772 KiB), cgroup 4.864 MiB, CPU quota 1, Cargo jobs 1 và swap 0. Headroom danh nghĩa trong guest khoảng 1.054.036 KiB ngoài cgroup. Trước setup, guest có 5.331.976 KiB MemAvailable và volume target Mac còn 26.691.900 KiB. Source, tài khoản và target đều riêng. Cgroup/guest/VM được kiểm trước mỗi phiên. Sampler đọc `memory.current`, `memory.events` và PSI hai giây một lần; RSS ghi theo child lớn nhất và `memory.peak` theo cả phiên.

Reader chọn riêng trên B qua toàn bộ READ-01, gồm wire 304 và giá trị mà client nhận. OOM kill xảy ra khi bắt đầu READ-02. Mẫu ngay trước đó 4.826.591.232 byte, sau đó 1.169.444.864 byte và `oom_kill` tăng từ 0 lên 1. Xtask của lượt này được dừng sau 45 giây không còn tiến triển. Wall 76,214 s.

## Reader và release

Commit `d6aded5132e18cf665d567d3cee911808c98e27b` tách ba tài liệu Reader độc lập sang ba browser context mới, vẫn dùng cùng App, PostgreSQL và package. Mỗi fixture tự thiết lập điều hướng và cache, giữ toàn bộ assertion, kiểm thêm console rồi đóng context. Sau đó fixture cũ tiếp tục trên trang ban đầu. Cả ba kiểm `readerBeforeHydration`, `conditionalPublicReads` và `tracePublicReadLifecycle` đều chạy xong. JSON SSR boundary và public-read lifecycle ghi PASS trên đúng SHA. Log còn ghi 304 trên wire, đọc cache qua client, trạng thái 404 tag lạ và 422 cursor lỗi. Reader sau đó bị OOM, 144,194 s, RSS child lớn nhất 1.813.068 KiB, cgroup peak 5.100.613.632 byte. Hai caller `readerPhase` và `mainPhase` cùng dùng helper này. Reader toàn phần và union còn PARTIAL.

Mac mất kết nối khi Discussion vừa bắt đầu. Sau khi kết nối lại, VM đã tắt. Log guest của Reader giữ các assertion, log điều phối trên Mac giữ số đo cuối. TSV và danh sách digest riêng của lượt Reader này trở lại dưới dạng tệp rỗng. Lượt ban đầu đã qua phép `cmp` mới bắt đầu Discussion. Sau khi phục hồi, băm lại toàn bộ package trên volume target và trong guest cho cùng digest danh sách `4fdcca126d1464b4a63507f532c5a3f514e44a69966db976d151449d6c9531c5`. Lượt Discussion dở không có kết quả; bảng bên dưới dùng lượt mới chạy trọn trên profile mới.

| Owner release, cùng phiên 4.864 MiB | Kết quả | Wall | RSS child lớn nhất |
| --- | --- | ---: | ---: |
| `cargo build --release --locked --offline -p vink-app --bin vink-app` | PASS native compile/link | 462,433 s | 3.043.036 KiB |
| `verify web --scenario perf-baseline --release --capture full` | FAIL build WASM thực, `--features perf-probe --profile wasm-release`, rustc nhận SIGKILL, oom_kill 0→1 | 136,128 s | 4.880.276 KiB |
| Publish WASM release, fresh/reused runtime, 10 cold/10 warm trong browser | NOT_RUN | — | — |

Peak cgroup tích lũy của phiên release là 5.100.273.664 byte. Số đo native App sau OOM có `oom_kill` giữ nguyên 1→1. Lượt 4 GiB trước đó cũng hết RAM ở WASM. Hai thời gian có trạng thái target và ngân sách khác nhau. Package dev còn trên volume chỉ có giá trị cho các phép kiểm dev.

## Scenario trên sản phẩm

Các lượt dưới chạy `cargo xtask verify web --scenario NAME --capture MODE` qua binary xtask đã build. Mỗi lượt dùng fixture và profile mới. Cột thời gian là wall của cả selector. RSS tính theo child lớn nhất; peak cgroup tích lũy của từng phiên có trong [mẫu stdout](work-chat-4-samples.txt). Đối chiếu danh sách digest package sau mỗi lượt. Hai ảnh đã mở, xác minh SHA và provenance: [Desk 1440](visual/verified-desk-1440.png) · [Editor 320/200](visual/verified-editor-320-200.png) · [manifest](visual/contexts.json).

| SHA | Scenario | Capture | Kết quả | Wall, s | RSS, KiB |
| --- | --- | --- | --- | ---: | ---: |
| `d6aded51` | discussion | full | PASS | 60.038 | 1,812,952 |
| `d6aded51` | storage | full | PASS | 18.046 | 1,146,524 |
| `d6aded51` | editor-format | full | FAIL | 54.117 | 1,146,268 |
| `d6aded51` | restart | checks | PASS | 144.145 | 2,708,744 |
| `d6aded51` | editor | checks | PASS | 26.020 | 1,146,496 |
| `d6aded51` | desk | checks | FAIL | 18.028 | 1,146,492 |
| `d6aded51` | appearance | full | FAIL | 62.095 | 1,146,524 |
| `d6aded51` | editor-chrome | full | FAIL | 58.071 | 1,146,364 |
| `9333c493` | editor-format | full | FAIL | 60.135 | 1,146,444 |
| `729cf385` | desk | checks | FAIL | 22.017 | 1,146,268 |
| `729cf385` | editor-format | full | FAIL | 78.150 | 1,146,444 |
| `729cf385` | appearance | full | FAIL | 71.098 | 1,146,560 |
| `729cf385` | editor-chrome | full | PASS | 132.122 | 1,146,376 |
| `763cce7d` | desk | checks | FAIL | 20.048 | 1,146,244 |
| `dec9dc90` | desk | checks | FAIL | 20.038 | 1,146,300 |
| `ab1ecd5a` | desk | checks | FAIL | 20.031 | 1,146,268 |

**Desk, capture checks.** Lỗi counter/review ban đầu đã được giải quyết ở B. Kiểm sâu hơn phát hiện một lỗi khác ở tiêu đề dài, 390 px/200%. Trace của Chrome ghi thứ tự: focus vào action cùng WorkId → callback WASM `pages::focus_recovery` đưa focus về `open-draft-search` → Shift+Tab bắt đầu từ launcher và kết thúc ở skip link `#noi-dung`. Các control liền kề trong DOM đúng là sort, link bản thảo, nút review và link kế tiếp. Các thử nghiệm focus có commit chẩn đoán riêng; code trên PR giữ nguyên yêu cầu keyboard focus, đúng WorkId, title không bị clamp, bổ sung precondition mở dock thực và chờ Back trả focus sau paint. Mode checks trên ab1ecd5a FAIL: keyboard reveals the entire title of the focused draft at 200% ({"href":"#noi-dung","focused":false,"overflow":"hidden","clamp":"2"}). Mode full trên ab1ecd5a NOT_RUN; đã có lượt full PASS trên 8bfacb47. Hai precondition mới về mở/đóng dock và focus sau paint đều đạt. Trace ở lượt này xác định contract cho bản sửa focus đã kiểm ở đầu báo cáo: giữ trả focus mặc định nếu người dùng còn ở control đóng hoặc body; nếu control khác đã nhận focus, đề xuất giữ đích mới và kiểm lại cả Escape/Back, error recovery, Tab/Shift+Tab. Caller và stack được giữ trong evidence ngoài target. Lệnh tái hiện trên bản cũ là `cargo xtask verify web --scenario desk --capture checks`.

**Editor Format.** Ở 1023 px fixture cũ dùng viewport 1024 px và đòi `position:sticky`. #475 quy định column theo bề rộng nội dung 50rem. Cả CSS và `observe_outline_layout` trên develop đã theo container. Bản sửa đo theo vùng Editor sau padding, đòi sidebar ở cạnh trang viết, kiểm dialog/role, focus, overflow, text scale và cả hai cực compact/docked. Sau khi sửa nốt cùng giả định ở preference đã lưu trên trang Tôi, lượt B qua 36 cảnh, preference thật, kiểm conversion, Markdown/title tới snapshot 422. Khi mở bản thảo Markdown dài trong `outlineOffsets`, cgroup OOM. Các assertion sau chưa chạy đủ. Tiếp tục trên môi trường đủ RAM bằng `cargo xtask verify web --scenario editor-format --capture full`.

**Editor Chrome.** Đã đối chiếu [quyết định Editor R4](../../../design/editor-r4.md) bỏ nút và modal preview độc lập. Oracle dirty cũ vẫn tìm `#preview` sau Save, trong khi oracle clean đã đòi control này vắng. Thử lại xác nhận Tab từ Save vào disclosure công cụ đang hiển thị, thứ tự tới title/body và toàn bộ scenario PASS.

**Appearance.** #477 chốt URL chuẩn `/ban-viet?review={WorkId}`. `LegacyDeskReview` redirect 308 ở SSR và replace khi điều hướng client. Lượt A đến Desk đã hydrate nhưng vẫn chờ URL cũ. Đã sửa oracle để đòi đúng Location/WorkId, URL sau điều hướng và active Desk trên rail/bottom bar. Probe HTTP trên App thật xác nhận 308 → URL chuẩn, HTTP 200, hai anchor Desk có `aria-current=page`. Lượt B bị OOM sớm hơn, trong nhóm route 404, trước assertion redirect mới. Các điều kiện hydrated của đoạn mới và cả Appearance cần tiếp tục bằng `cargo xtask verify web --scenario appearance --capture full`. [Contract URL cũ](../../../design/redesign-r4/desk-moderation.md).

Các phase dùng đúng loại fixture của owner, trong đó một số trạng thái danh sách Desk được cung cấp qua `DraftGate`. Không gộp các lượt selected thành một lần full union. Các terminal FAIL và counters OOM vẫn có trong [mẫu thô](work-chat-4-samples.txt).

## Tiếp tục từ Draft

Reader và WASM release đã chạm trần ở cả 4 GiB và 4.864 MiB. VM 6 GiB còn khoảng 1 GiB ngoài cgroup ở cấu hình sau. Chưa xác lập mức RAM tối thiểu đủ chạy. Cần một môi trường được cấp quyền, có chỗ cho target mới, bộ nhớ cao hơn cho profile này, hạn mức cứng bao toàn bộ process tree dưới 14 GB, một slot và đủ headroom cho hệ điều hành. Dùng Rust toolchain của repo, wasm32, bindgen 0.2.129, Node/npm, Chromium, PostgreSQL scratch, dev dependencies và các flags release hiện hành. Chạy từng lệnh tuần tự, lưu log và manifest trước khi clean.

```sh
cargo xtask verify changed --base 4d97d6a4d57eab22880f9a5c2eaba7897262c5e9 --head HEAD --plan
cargo xtask verify web --scenario reader --capture full
cargo xtask verify web
cargo xtask verify web --scenario perf-baseline --release --capture full
```

Để nghiệm thu release fresh/reused, dùng target riêng với receipt release ban đầu vắng, sau đó chạy lại chính lệnh perf-baseline với cùng SHA, flags, fixture và target ấm. Đối chiếu hash package, kết quả runtime và dữ liệu 10 cold/10 warm của từng lượt. Cặp dev hydrate đã chạy trên target ấm là 22,425/15,657 s; cặp dev CSR có đối chứng là 29,022/23,294 s. [Biên đo gốc và digest](work-chat-4.md). #509 chỉ đo package Cargo nhỏ. #511 chỉ đo warm hydrate HIT 4,77–4,93 s so với forced MISS 10,78–11,32 s.

## Đối chiếu issue

| Issue | Phạm vi đã kiểm và còn thiếu |
| --- | --- |
| #496 | PARTIAL. Đã có planner/guard, selector thực, kết quả Store, SSR, restart, CSR, Editor và Desk. Thiếu full Web và release/perf; số đo workspace và cross-worktree vẫn giới hạn như baseline. |
| #497 | Inventory, RSS, peak, cấu hình và cold/warm tách theo workload. Chưa có profile release browser hoặc số liệu đại diện mọi bước của workspace. |
| #498 | Giữ bằng chứng selector static/check/Store, 5 unit, PG scratch và SSR. Full quick và full postgres không được chạy lại ở scope này. |
| #499 | Hai oracle ban đầu Editor/Desk và lỗi focus thật ở Desk đã có đối chứng A/B. Bản sửa focus qua cả Desk checks và full, gồm phiên khách và fallback khi mất quyền. Union sạch end-to-end còn thiếu; direct monolithic main chưa nghiệm thu runtime. |
| #500 | Bindgen dev có receipt, digest và bằng chứng chạy sản phẩm fresh/reused. App native release build đạt. WASM release build FAIL tài nguyên, publish/runtime NOT_RUN. CSS/Editor tiếp tục chạy tokens, typecheck và corpus; chưa đủ closure để cache output. |
| #501 | Có fixture owner, range/worktree, negative control, mapping Desk và guard từ chối các biến chuyển hướng Git. CLI black-box chạy trên binary được build lại sau khi kiểm SHA input nguồn. |
| #502 | Admission, swap 0, target riêng và huỷ đúng nhóm child được kiểm. Mức máy hiện có vẫn chặn hai workload lớn. Tình trạng compiler cache/cross-toolchain theo baseline. |
| #503 | AGENTS, SSOT, skill/template dùng lệnh của xtask; có quy tắc provenance, stale binary khi export source, phân biệt cache HIT với kết quả runtime. |

[Ma trận AC và các phép kiểm ban đầu](work-chat-4.md#đối-chiếu-acceptance).
