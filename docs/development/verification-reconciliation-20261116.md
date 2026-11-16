# Đối soát docs, quyết định và Quint sau các PR 15–16/11/2026

**Phạm vi:** 20 PR merge trên first-parent của `origin/develop`, từ sau `09ebede` đến
`6e07806` (PR #196–#240 trong bảng dưới; không phải mọi số PR trong khoảng).
Base đầy đủ: `6e07806aa87145d0e053f4061f47ccaf3830c14a`. Không nhập thay đổi dở dang
của checkout chính. Đây là đối soát source/contract/test và bổ sung evidence thuần, không phải
chạy lại các lần browser/deployment đã ghi trước đó.

Tái lập inventory: `git log --first-parent --format='%h %s' 09ebede..6e07806`.
Lần đọc TOML trước chỉnh có snapshot `cef5662c19d3db1219bab14d197b5413f203aa816c99eaa11e8f2c31688091c2`.
Các hash này ghim đầu vào, không xác nhận production. Lượt này không đổi runtime policy/UI/SQL.

## PR đã merge và phân loại tác động

| PR / merge SHA | Owner và hiện trạng tại base | Phân loại / phần cần giữ riêng |
| --- | --- | --- |
| [#196](https://github.com/loveoverflowcom/vink/pull/196) `b985e04`, [#197](https://github.com/loveoverflowcom/vink/pull/197) `a677b0d`, [#199](https://github.com/loveoverflowcom/vink/pull/199) `36a7e47`, [#206](https://github.com/loveoverflowcom/vink/pull/206) `d22647d` | Recovery/reflow Editor, Desk, Reader/Discovery; [editor](../design/editor-writing-reflow-193.md), [discovery](../design/tag-discovery.md) | Giữ outcome/guards; không cần Quint cho spacing/title disclosure. Browser evidence lịch sử không là native/OS IME acceptance |
| [#198](https://github.com/loveoverflowcom/vink/pull/198) `10805a7`, [#207](https://github.com/loveoverflowcom/vink/pull/207) `c019619` | [VPS test](test-container-evidence.md), [Cloudflare test](cloudflare-test-evidence.md) | Runbook/deployment lịch sử có pin riêng; không suy môi trường hiện chạy code `6e07806`, không deploy lại |
| [#210](https://github.com/loveoverflowcom/vink/pull/210) `ebbde22`, [#212](https://github.com/loveoverflowcom/vink/pull/212) `73fdbe0` | [READ-02](public-read-freshness.md): lỗi có kiểu, backoff, typed `/reader`, ticket/chain | Thiếu model primitive dùng chung → bổ sung model/replay Web bên dưới; freshness coordinator vẫn ngoài model |
| [#211](https://github.com/loveoverflowcom/vink/pull/211) `f22815e`, [#221](https://github.com/loveoverflowcom/vink/pull/221) `b28e4bd`, [#222](https://github.com/loveoverflowcom/vink/pull/222) `c3b6e7d` | [Paging](desk-pagination.md), [bỏ resume](desk-compact-tham.md), [author read-model](desk-author-feedback.md) | Web đã chuyển `/drafts` → `/author/works`, bỏ Recent/1; API legacy vẫn có. Thấm neo revision; Nhận xét neo thread public projection, không reset cùng revision |
| [#215](https://github.com/loveoverflowcom/vink/pull/215) `fa20513` | [IndexedDB](public-read-freshness.md#indexeddb-203) và `public-cache/indexeddb.js` | Adapter `web` có thật, rollout **tắt**; memory 15 phút khác proposal durable 24 giờ, không cold offline launch |
| [#216](https://github.com/loveoverflowcom/vink/pull/216) `ef7da9d`, [#219](https://github.com/loveoverflowcom/vink/pull/219) `97b29c3`, [#220](https://github.com/loveoverflowcom/vink/pull/220) `6de515d` | [Dialog](../design/dialog-anatomy-r1-208.md), appearance trong [feature map](feature-map.md) | Anatomy/focus/reflow giữ luật lưu/rời trang; dùng browser/preview tests hiện hữu, không thêm model policy |
| [#223](https://github.com/loveoverflowcom/vink/pull/223) `a3e6536` | [Queue tác phẩm/phản đối](moderation-runtime.md#10-hai-queue-hữu-hạn-205) | Đã migrate ReadOwner/PageChain; quyền và rows cùng snapshot PG. Legacy array chỉ tương thích body, tab cũ không theo Link |
| [#227](https://github.com/loveoverflowcom/vink/pull/227) `69f71f0` | [Save projection](../design/ui-editor-unified-save-217.md) | Một trạng thái chữ+thẻ, hành động thủ công có guard. Không autosave/device recovery; không dùng read model để nhận coverage mutation |
| [#228](https://github.com/loveoverflowcom/vink/pull/228) `2c55c26`, [#240](https://github.com/loveoverflowcom/vink/pull/240) `6e07806` | [Shared surfaces](../../packages/ui-system/shared/components.md), [#225](../design/action-surfaces-desk-discovery-225.md) | Primitive Web/CMP đã có; #240 chỉ spec/tests, UI adoption #225 chưa triển khai, CHOICE còn dự kiến |
| [#232](https://github.com/loveoverflowcom/vink/pull/232) `a81333c` | [Document budget](long-document-budget.md) | Benchmark/regression đã có. Document hợp lệ theo domain có thể vượt HTTP 2 MiB; cache 512 KiB phải miss. Không đổi giới hạn sản phẩm để hợp thức hóa gap; chưa chốt SLA/chunking/LZ4 |

## Quyết định TOML mới

Các record dưới đây ghi **lựa chọn kỹ thuật đã thấy trong source** thành proposal có thể review.
Tất cả là `architecture`, revision 1, **PENDING**; không có signoff, không thay record đã nhập
hoặc active marker. Merge của PR cũ không là lời duyệt đúng revision mới. Các trạng thái
`PRODUCT_CONFIRMED`/`OPEN` của yêu cầu liên quan và `DESIGN_PROPOSED` của ADR vẫn giữ nguyên.

| Proposal | Owner chi tiết / phạm vi |
| --- | --- |
| [VDR-WEB-READ-LIFECYCLE](../../decisions/proposals/VDR-WEB-READ-LIFECYCLE.r1.toml) | [READ-02 lifecycle](public-read-freshness.md#lifecycle-web-202), typed failure và read-only wake |
| [VDR-PUBLIC-SNAPSHOT-STORAGE](../../decisions/proposals/VDR-PUBLIC-SNAPSHOT-STORAGE.r1.toml) | [IndexedDB](public-read-freshness.md#indexeddb-203), transaction/epoch/quota/fallback và gate tắt |
| [VDR-DESK-PAGINATION](../../decisions/proposals/VDR-DESK-PAGINATION.r1.toml) | [Desk paging](desk-pagination.md), live keyset, API legacy và consumer hiện hành |
| [VDR-MODERATION-QUEUE-PAGINATION](../../decisions/proposals/VDR-MODERATION-QUEUE-PAGINATION.r1.toml) | [Moderation §10](moderation-runtime.md#10-hai-queue-hữu-hạn-205), authority snapshot và rollout |
| [VDR-AUTHOR-WORK-FEEDBACK](../../decisions/proposals/VDR-AUTHOR-WORK-FEEDBACK.r1.toml) | [Author feedback](desk-author-feedback.md), zero/null/unknown và hai loại số liệu khác anchor |
| [VDR-EDITOR-SAVE-PROJECTION](../../decisions/proposals/VDR-EDITOR-SAVE-PROJECTION.r1.toml) | [Editor #217](../design/ui-editor-unified-save-217.md), partial save, uncertainty và focus |
| [VDR-ACTION-SURFACES](../../decisions/proposals/VDR-ACTION-SURFACES.r1.toml) | [Shared contract](../../packages/ui-system/shared/components.md), opt-in neutral surface; #225 còn pending adoption |

`cargo xtask decision query ID` cho biết current/head và digest. `pending_only` không có nghĩa
runtime chưa tồn tại; ngược lại source có implementation không khiến proposal thành Accepted.
Xem [hợp đồng signoff](../../decisions/README.md#ghi-signoff-của-chủ-sản-phẩm) khi cần quyết định.

## Phần chưa rõ và điều kiện làm tiếp

| Phân loại | Điểm còn mở | Điều kiện / kiểm tra kế tiếp |
| --- | --- | --- |
| `open_product` | [VDR-PUBLIC-OFFLINE-AGE@r1](../../decisions/proposals/VDR-PUBLIC-OFFLINE-AGE.r1.toml) 24 giờ vẫn PENDING; logout/account switch/thiết bị dùng chung và UX expired chưa chốt | Owner review đúng revision và policy clearing trước bật gate; sau đó kiểm composition persistent offline UI, không chỉ adapter `storage` |
| `open_product` | READ-02 RAM `max_display_age=15 phút` còn giả định; thu hồi ở tab/thiết bị khác phụ thuộc revalidation | Chốt phạm vi hiển thị dữ liệu cũ khi triển khai offline; không thay bằng 24 giờ từ proposal pending |
| `open_product` | O-07 và POLICY-01 B/C của #214 chưa chốt kế thừa/tích lũy/ranking | Chỉ đọc counter baseline; hỏi owner khi làm thay đổi anchor hoặc inheritance |
| `open_technical` | Domain nhận document mà HTTP body cap từ chối; chi phí owner-state/DOM lớn | Dùng corpus #229 tái hiện ở HTTP/loader/render boundary; #230/#231 và thiết bị release trước ngân sách mới |
| `rollout_constraint` | Bundle moderation legacy bỏ qua `Link`, chỉ thấy 50 hàng đầu | Server/migration trước bundle; reload tab duyệt cũ. Nếu phải giữ tab cũ xuyên rollout, cần thiết kế tương thích bổ sung |
| `verification_gap` | Chưa model `freshness::Store/Coordinator`, IndexedDB transaction hay save reducer; model mới không phủ authority fingerprint | Giữ unit/browser/PG riêng; chỉ mở rộng model nếu có rủi ro state/race cụ thể, không sao chép governance |
| `implementation_gap` | #225 chưa UI adoption; autosave/device recovery, native product runtime chưa có tại base | Giữ feature/spec/evidence tách biệt, không đóng issue từ việc tests pass |

## Evidence của lượt này

Model mới: [owner Web](../../apps/vink-web/behavior/README.md), 52 bước lawful replay qua
production, 9 bước counterexample với bước vi phạm bị production từ chối. Dùng runner hiện hữu
`verify discussion`, chung pin Quint 0.33.1; không thêm generic lane hoặc package dependency.
Runner bắt buộc chọn đủ 21 replay App hiện hành và 2 replay Web mới.

Đầu vào: base ở đầu tài liệu + [SHA-256 model/fixture/Rust/runner](evidence/verification-20261116/source-manifest.json).
[Trích output kiểm tra](evidence/verification-20261116/checks.txt) giữ summary và số test thực tế;
chỉ bỏ log biên dịch/từng test để gọn. Môi trường macOS arm64, Rust 1.99.1, Node 26.12.0. Build dùng `CARGO_PROFILE_DEV_DEBUG=0`,
`CARGO_PROFILE_TEST_DEBUG=0`, `CARGO_INCREMENTAL=0`; không hạ assertions.

| Lệnh / boundary | Kết quả |
| --- | --- |
| `cargo xtask verify discussion` — model + pure production replay | **PASS** toàn lane: pinned models/drift/negative controls, 107 App tests và 4 Web read-lifecycle tests; 21 + 2 named replay bắt buộc được chọn |
| `cargo xtask verify quick` — static/unit/contract/repository | **PASS** toàn lane: token/manifest, fmt, 3 clippy, doctest, unit/contract và docs/skills/link/decision checks |
| `cargo test --locked -p vink-web --lib --features ssr -- read_lifecycle::` | **PASS** 4/4; 52 lawful steps + 9 counterexample steps, không transport/DB |
| `cargo xtask decision validate`, `cargo xtask decision refs`, `git diff --check`, JS syntax | **PASS**; 125 revisions, 79 declared refs, 7 proposal mới đều PENDING; rà cuối 779 local Markdown file targets và 8 source fingerprints |
| PostgreSQL/browser/CMP/Kani/Apalache/deployment/GitHub CI | **NOT_RUN** trong lượt này; không đổi runtime behavior/SQL/UI hoặc các harness Kani |

Snapshot TOML cuối: `6e77cb0a7f4eb67033c6e0df6d8a670a84eb61b3095a307ad0615b771c91dd9b`.
Các kết quả cũ trong từng owner giữ nguyên SHA, FAIL/NOT_RUN và môi trường ban đầu.
Model/replay mới không làm mới bằng chứng runtime hoặc chốt quyết định sản phẩm.
