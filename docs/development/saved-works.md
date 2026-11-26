# Lưu bài · registry triển khai và bàn giao

| Chiều | Trạng thái sau #388 |
| --- | --- |
| Sản phẩm | SAV-01..04 (21/11), D1–D12, vòng đời và phạm vi DEFERRED (25/11) đã được chủ sản phẩm duyệt tại [Epic #384 §8](https://github.com/loveoverflowcom/vink/issues/384). [PRD §18](../../prd/requirements.md#18-lưu-bài-để-đọc-lại-saved-works) sở hữu quy tắc và ví dụ SAV-INV-01..05. |
| Registry quyết định | [VDR-SAVED-WORKS@r1](../../decisions/proposals/VDR-SAVED-WORKS.r1.toml) · `PENDING` · SHA-256 `7ebf41f88b6b3f91bf2aed1dd10d0f221b133ce30d55027e57d36d3d6e468b45`. Epic ghi “DUYỆT: Duyệt D1–D12 theo phương án đề xuất” và “DUYỆT: Duyệt quy tắc vòng đời dữ liệu”. Chưa có lời duyệt chỉ đích danh revision/hash này theo [quy tắc signoff](../../decisions/signoffs/VDR-DECISION-SIGNOFF.r1.toml). |
| Kiến trúc | [Contract v1](../../services/vink-app/contracts/http/v1/saved-works.md) là `DESIGN_PROPOSED` cho một owner trong Vink App. CHOICE-01 dùng một dòng bền cho mỗi cặp người lưu và tác phẩm. |
| Triển khai | [Domain Lưu bài](../../services/vink-app/src/domain/saved_work.rs) và [adapter PostgreSQL](../../services/vink-app/src/persistence/saved_works.rs) đạt mức `persistence-api`. Reader Web đạt mức `web` từ #388: [ReaderSave](../../apps/vink-web/src/pages/saved_work.rs), [reducer phiên](../../apps/vink-web/src/saved_state.rs), `SaveAction`/`ActionPair` trong [vink-patterns](../../packages/vink-patterns/leptos/src/lib.rs), trình diễn bằng dữ liệu Mẫu ở mục preview Leptos `composition.save-pair` (CMP `unsupported`). Tab Đã lưu (#389) chưa triển khai. [LayoutSpec](../design/saved-works.md) cùng [delta Pen Flight](../design/pen-flight-r2.md) mô tả đích. CMP/native thuộc DEFERRED. |
| Kiểm chứng | [#385](https://github.com/loveoverflowcom/vink/issues/385) có bằng chứng tài liệu và registry. [Các ca unit #386](../../services/vink-app/src/domain/saved_work/tests.rs) chạy bằng `cargo xtask verify unit app --filter domain::saved_work`. Các [test tích hợp #387](../../services/vink-app/tests/postgres/saved_works.rs) đi qua Axum và PostgreSQL. Kết quả, SHA và giới hạn bằng chứng được ghi ở [#387](https://github.com/loveoverflowcom/vink/issues/387). |

## Truy vết quyết định và phần cần chứng minh

| PR và owner | Luật / oracle cho lát tiếp theo | Ranh giới bằng chứng cần có |
| --- | --- | --- |
| PR1 · [#386](https://github.com/loveoverflowcom/vink/issues/386) · domain | SAV-01, SAV-INV-01/03/04, D6/D7/D8/D12. Đặt desired state; phân biệt lần mới, no-op mới, retry và stale. Mô phỏng lưu → bỏ lưu → lưu, stale trái chiều, đứng sát cap và toàn bộ bảng vòng đời. | Unit thuần; lấy [reaction generation](../../services/vink-app/src/domain/discussion/reaction.rs) làm mẫu, độc lập HTTP/SQL. |
| PR1 · [#387](https://github.com/loveoverflowcom/vink/issues/387) · App/PG/HTTP | SAV-01..03, SAV-INV-01..05, D6–D10/D12. Hai WorkId tranh suất cuối, đổi standing/quyền công khai trong lúc ghi, bỏ lưu bài không đọc được, xóa chủ và fence. Hai trang xen Gone/cùng thời điểm; đổi quyền trước trang sau. So byte/ETag công khai và header riêng; kiểm không phát thông báo/ranking. | PostgreSQL thật + API v1, contract fixture thành công/lỗi; kết quả retry không làm mới saved_at. |
| PR2 · [#388](https://github.com/loveoverflowcom/vink/issues/388) · Reader | SAV-04, D1–D5/D11. Một cặp bằng nhau, “Thấm 12” cạnh Lưu; sheet trả focus, không auto-save; no-op/ACK trễ, lỗi cap và mất ACK. | Web CSR standalone và SSR/hydration đang ship; bàn phím, screen reader, hai theme, reflow theo LayoutSpec. |
| PR3 · [#389](https://github.com/loveoverflowcom/vink/issues/389) · hồ sơ | SAV-01..04 và SAV-INV-01..05. Hành trình Lưu → Đã lưu → đọc publication mới → Back → Bỏ lưu. F5, yêu cầu trực tiếp, đổi phiên, phân trang, neutral/omit, NotFound tương đương với khách/người khác. | Web + hành trình tích hợp với API thật; đối chiếu DOM/SSR, public cache và quyền tại lần đọc. |

## Giao dịch và cursor của #387

**CHOICE-01.** Domain tăng generation cả khi ý định mới trùng trạng thái. Vì retry có thể đến sau khi bỏ lưu, PostgreSQL giữ generation cùng `saved_at` trên một dòng có khóa chính `(principal_id, work_id)`. Chỉ dòng có `saved_at` nằm trong index danh sách và chiếm suất. Khóa principal của người lưu tuần tự hóa cả hai WorkId tranh suất cuối. PUT khóa con trỏ công khai trước, rồi lấy khóa principal theo thứ tự UUID để việc xóa tài khoản tác giả và người lưu có cùng thứ tự với Follow. Domain quyết định kết quả sau khi adapter đọc lại quyền và đếm trong giao dịch. [Migration](../../services/vink-app/migrations/20261204000000_saved_works.sql) · [Domain](../../services/vink-app/src/domain/saved_work.rs).

GET danh sách dùng một snapshot PostgreSQL cho standing, retirement, publication, thẻ thể loại và byline. Truy vấn lọc Gone trước khi lấy tối đa 21 dòng hiện được. Cursor ký biên thời gian và WorkId bằng khóa ngẫu nhiên của cặp, gắn với chủ và thứ tự v1. Vì fence giữ khóa qua bỏ lưu, cursor vẫn xác minh được cả khi người dùng đã bỏ lưu hoặc lưu lại bài ở cuối trang trước. [Truy vấn](../../services/vink-app/src/persistence/saved_works/list.sql) · [Cursor](../../services/vink-app/src/persistence/saved_works/cursor.rs).

Khi App ghi fence xóa tài khoản hoặc thu hồi handle theo đường cũ, trigger dọn các dòng do tài khoản đó sở hữu ngay trong giao dịch. Những người từng lưu bài của tác giả đã xóa thấy hàng trung tính theo quyền hiện tại. [Account Deletion #297](../architecture/account-deletion.md) ghi fence sau xác minh. Bài bị kiểm duyệt gỡ tiếp tục bị loại trước khi chiếu hàng trung tính. SOC-DEP-02 vẫn sở hữu hạn chế riêng cho đăng bài và nhận xét.

**Bàn giao Web.** [PR #527](https://github.com/loveoverflowcom/vink/pull/527) đưa đặc tả và domain vào develop. #387 cung cấp API cho Reader #388 và hồ sơ #389. Sau khi mất ACK, consumer đọc lại đúng cặp của phiên hiện hành. [Hợp đồng và bảng lỗi](../../services/vink-app/contracts/http/v1/saved-works.md).

## Reader Web (#388)

**Cách làm.** Mỗi Reader có một owner cho cặp (phiên, WorkId): [`SaveController`](../../apps/vink-web/src/saved_state.rs) thuần và adapter [ReaderSave](../../apps/vink-web/src/pages/saved_work.rs). Adapter chỉ đọc `GET /api/v1/me/saved-works/{id}` trong trình duyệt, sau khi owner phiên của tab xác nhận principal (`profile_scope`). Vì vậy HTML SSR, artifact CSR và cache công khai READ-01/02 vẽ cùng một nút đang chờ cho mọi người. Bấm gửi `PUT`/`DELETE` kèm `expected_generation` vừa xác nhận và `X-Vink-Expected-Principal`. Hình, icon và `aria-pressed` chỉ đổi theo ACK hoặc lượt đọc, và generation mới hơn luôn thắng (SAV-INV-04).

| Tình huống | Hành vi |
| --- | --- |
| Đang chờ | Vòng chờ trong nút, “Đang lưu…”/“Đang bỏ lưu…”, giữ focus và giá trị đã xác nhận; bấm thêm không gửi lệnh mới. Một lượt làm mới nền đang bay bị bỏ khi người đọc bấm. |
| `409 version_conflict` | Giữ trạng thái chờ, đọc lại cặp; nếu trạng thái thật khác ý định thì báo lỗi cạnh nút và hiện trạng thái mới hơn. |
| Mất ACK, timeout, `503`, `5xx` | Giữ trạng thái đã biết, khóa bấm, “Chưa xác nhận được lượt lưu.” + “Kiểm tra lại”. Nút này chỉ đọc lại, không phát lại lệnh, rồi trả focus về Lưu. |
| `409 saved_works_limit_reached`, `404`, `401/403` | Giữ trạng thái cũ, báo đúng câu (hạn mức không nêu số). `401/403` nhờ owner phiên kiểm lại. |
| Đổi phiên A → B, đăng xuất | Scope mới xóa trạng thái, kết quả, liên kết và mọi phản hồi đang bay của scope cũ. |
| Khách, chưa có bút danh | Mở dialog Reader với mục đích “Đăng nhập để lưu bài”; xong thì trả focus về Lưu, đọc trạng thái của phiên mới, hiện “Bạn đã đăng nhập. Bấm Lưu để lưu bài này.” nếu chưa lưu. Không gửi lệnh lưu. |
| Tab hiện lại, khôi phục từ lịch sử | Đọc lại khi rảnh, tối đa một lần mỗi 15 giây, để hai tab không giữ trạng thái cũ quá lâu. |
| Trang ẩn, gỡ sau kiểm duyệt, đã xóa | Không có nhóm nút; chỉ trang công khai dựng `WorkEndorsement`. |

Dialog đăng nhập chuyển lên cấp trang Reader để composer và Lưu dùng chung, mỗi nơi gọi tự nêu điều kiện “owner riêng đã áp phiên mới”. `ToggleAction` có thêm các tùy chọn Web không đổi `toggle-contract-v1` ([components](../../packages/ui-system/shared/components.md)). Nhóm Thấm · Lưu chỉ đặt chữ `on-surface` và nền `action.neutral.container` bên trong nhóm, nên Ưng và các ToggleAction khác giữ nguyên token. Liên kết “Mở Đã lưu” lấy handle từ `GET /api/v1/me/profile` của đúng principal và trỏ tới `/@handle/da-luu` của #389. Ngày 26/11/2026 chủ sản phẩm chốt ba điểm mở của lát này (tên truy cập “Lưu” + `aria-pressed`, link tới trang #389 trả NotFound đến khi PR3 merge, Thấm của khách giữ lý do hiện có); chi tiết ở [LayoutSpec](../design/saved-works.md#cặp-thấm--lưu).

### Kiểm chứng #388

Base `f9c14d0b`, working tree của PR trước khi commit; code của commit giống hệt bản đã chạy. macOS, Chrome 158 headless, PostgreSQL 17.11 scratch. [Ảnh và cách đọc](../design/evidence/saved-works-388/README.md).

| Lệnh / lane | Kết quả | Ranh giới |
| --- | --- | --- |
| `cargo xtask verify check web --target all-targets`, `--target csr`, `--target hydrate` | PASS, 0 cảnh báo | compile native và WASM |
| `cargo xtask verify unit web` · `unit ui` · `unit patterns` | PASS (gồm 8 ca `saved_state`, SSR cặp nút, contract fixture, `SaveAction` và `ToggleAction` mới) | unit/SSR thuần |
| `cargo clippy` cho `vink-web`/`vink-ui`/`vink-patterns` (native, `-D warnings`) và `vink-web` WASM `hydrate`/`csr` | PASS native. WASM có một lỗi `type_complexity` sẵn có ở `work_view.rs` (#519, ngoài diff); bỏ riêng lint đó thì PASS | lint |
| `cargo xtask verify web --scenario standalone` | PASS. Hành trình Lưu có 21 assertion: sheet khách, pending, ACK, reload, không rò, mất ACK, hai tab, A → đăng xuất → B, Thấm, reduced motion và 24 tổ hợp hình học | browser + CSR + App API + Identity thật |
| `cargo xtask verify web --scenario auth` | PASS, gồm toàn bộ #328 và SSR của Lưu: `#reader-paper` và `#work-endorsement` của người đã lưu giống hệt của khách; hydration lưu/bỏ lưu tới API thật | browser + SSR/hydration + Identity thật |

**Năm chiều theo [UX quality](../../.agents/skills/vink-ui-design/references/ux-quality.md).**

| Chiều | Kết quả | Bằng chứng | Giới hạn |
| --- | --- | --- | --- |
| Functional | PASS | Lưu/bỏ lưu qua reload; sheet khách không tự lưu, trả focus; mất ACK đọc lại đúng trạng thái đã commit; tab cũ bị 409 rồi hiện trạng thái mới hơn; phản hồi muộn của A không hiện cho khách hay B | Trang Đã lưu (#389) chưa có nên “Mở Đã lưu” dẫn tới NotFound. Chưa kiểm lỗi hạn mức 10.000 trên browser (unit có) |
| Geometry | PASS | Cao 56 ± 0,5, hai nửa bằng nhau, khe 2, nhóm ≤ 400 px ở 100%. Xếp dọc đúng khi host < 20rem; không tràn ở 320/390/1024 × sáng/tối × 100/150/200/300%. Số 4 và 7 chữ số không tràn, 7 chữ số chỉ xuống dòng ở 200% | Số dài được thay trong DOM để đo |
| Visual | PASS có giới hạn | Đã mở 6 ảnh: nhóm nối, viên thuốc + ✓, sáng/tối, sheet | Nút chọn và chưa chọn chỉ khác 1,02:1 ở theme sáng (OPEN-2 đã chấp nhận); trạng thái dựa vào hình, icon đặc và ✓. Icon giữ 20–24 px nên nhỏ so với chữ 200% |
| Accessibility/Responsive | PASS có giới hạn | `aria-pressed` theo dữ liệu xác nhận, tên “Lưu” cố định, `aria-busy` khi chờ; ghi chú `aria-live=polite`; Enter/Space; link ≥ 44 px; reduced motion 0,01 ms | Chưa chạy VoiceOver/NVDA/TalkBack hay forced colors trên thiết bị thật |
| Interaction | PASS | Bấm lặp khi chờ không gửi lệnh thứ hai; “Kiểm tra lại” chỉ đọc lại rồi trả focus; Escape/Đóng trả focus Lưu; đăng nhập trong sheet trả focus Lưu | Safari/Firefox, IME và bàn phím mềm thật `NOT_RUN` |
