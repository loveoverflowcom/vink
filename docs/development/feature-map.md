# Bản đồ tính năng và đường kiểm chứng Vink

**Đối chiếu mã nguồn:** 31/10/2026, nền `20635b45c856990638c06eaab011b9940da852dc`.
Bản đồ mô tả skeleton trong checkout này; lần lập bản đồ chỉ đọc source và assertions,
không chạy lại ứng dụng. Cột kiểm thử cho biết nơi có assertion, không phải kết quả `PASS`.
Khi code đổi, đối chiếu lại các hàng liên quan và ghi mốc mới; kết quả chạy phải gắn với
revision/diff, môi trường và lệnh của chính lần chạy đó.

[Hướng dẫn phát triển](README.md) sở hữu setup, cấu hình, lệnh và giới hạn các lane.
[Verification guide](../../.agents/skills/vink-engineering/references/verification.md) giúp
chọn lớp bằng chứng; [requirements](../../prd/requirements.md) sở hữu tiêu chí sản phẩm.
Bản đồ này chỉ giúp tìm luồng, thao tác và nơi kiểm chứng, không thay thế các nguồn đó.

## Điều kiện để đi qua các luồng

- Dùng đúng `VINK_PUBLIC_ORIGIN` của môi trường local theo hướng dẫn phát triển. POST/PUT
  từ origin khác bị từ chối; mở được trang chưa chứng minh thao tác ghi hoạt động.
- Fixture trong [dev_seed.rs](../../services/vink-app/src/dev_seed.rs) có tác phẩm
  `[Mẫu] Mưa trên phố cũ`, `SAMPLE_WORK = 33333333-3333-4333-8333-333333333333`.
  Seed chỉ nạp khi gọi rõ ràng và giữ nguyên hàng đã tồn tại; một database local đã sửa
  không nhất thiết còn đúng nội dung mẫu ban đầu. Dùng dữ liệu riêng cho thử nghiệm ghi.
- Bàn viết cần JavaScript/hydration và principal phía server. `VINK_DEV_PRINCIPAL` chỉ là
  fixture local trên loopback; chưa có màn đăng nhập hoặc nguồn xác thực production.
  Không cấu hình authority thì API nháp trả `401`, dù khung HTML vẫn mở được.
- Các đường dẫn dưới đây tương đối với origin đang chạy. Định tuyến web nằm ở
  [App](../../apps/vink-web/src/lib.rs), API ở
  [router](../../services/vink-app/src/http/api.rs); phương thức và lỗi do
  [contract v1](../../services/vink-app/contracts/http/v1/README.md) sở hữu.

## Điểm vào và trạng thái cần nhận biết

| Luồng | Đường đi / thao tác | Kết quả và trạng thái hiện có | Owner |
| --- | --- | --- | --- |
| Khám phá tác phẩm công khai | `/`, chọn thẻ trong “Tác phẩm công khai”; “Đọc” trên thanh điều hướng trở về `/` | Danh sách snapshot công khai; có trạng thái rỗng và lỗi dữ liệu. Chưa phải Tạp Chí tuyển chọn | `Home`, `WorkCardView` trong [pages/mod.rs](../../apps/vink-web/src/pages/mod.rs) |
| Đọc tác phẩm | `/tac-pham/{id}`, thử `SAMPLE_WORK` trên fixture nguyên trạng | HTML trước JavaScript có nội dung publication, metadata và canonical; không thấy phần nháp. Thiếu/chưa công khai trả `404`, dữ liệu không tới được trả `503` | `Reader`, `PublicationView` trong [pages/mod.rs](../../apps/vink-web/src/pages/mod.rs), [DocumentView](../../apps/vink-web/src/document.rs) |
| Mở và tạo nháp | “Bàn viết” → `/ban-viet` → “Tạo bản nháp mới” → `/ban-viet/{id}` | Tải danh sách riêng sau hydration; nút tạo bị khóa khi đang gửi, thành công mở nháp version 1. Có lỗi tải/tạo; không có publish ở đây | `DeskIndex`, `DraftListView`, `DeskError` trong [desk.rs](../../apps/vink-web/src/pages/desk.rs) |
| Viết và lưu nháp | `/ban-viet/{id}` → sửa “Tiêu đề”, “Nội dung” → “Lưu nháp”; có Enter ở tiêu đề và Ctrl/Cmd+S trong form | `unsaved` → `saving` → `saved` khi server xác nhận; `failed` hoặc `conflict` giữ nội dung trên trang. Lưu thủ công, chưa autosave | `DraftEditor`, `Editor` trong [desk.rs](../../apps/vink-web/src/pages/desk.rs), [EditorSession](../../apps/vink-web/src/desk_state.rs) |
| Xử lý xung đột | Mở cùng nháp ở hai tab, tab B lưu trước, tab A lưu bản cũ → “Lưu đè bằng nội dung đang viết” khi muốn giữ bản A | Save cũ nhận `409`, không ghi; bản A còn trong editor. Lưu đè dùng version được báo và vẫn có thể xung đột lần nữa. Muốn tải bản server thì sao chép nội dung cần giữ trước khi reload | `Editor`, `EditorSession`; owner/version được kiểm trong [PgStore](../../services/vink-app/src/persistence/postgres.rs) |
| Kiểm tra tiến trình | `/health/live`, `/health/ready` | Liveness tách khỏi kết nối/schema database; readiness không khẳng định có auth, moderation, Discourse hay job | [health.rs](../../services/vink-app/src/http/health.rs) |

Các điểm bám automation đã có trong Bàn viết: `data-testid=create-draft`, `save`, `save-status`,
`conflict`, `keep-mine`, `desk-error`; `input[name=title]`, `textarea[name=body]`.
`save-status` có `data-status` là `saved`, `unsaved`, `saving`, `failed`, `conflict`.
Đây là điểm tìm phần tử, không thay thế việc kiểm nội dung, version và dữ liệu bền vững.

## Assertion hiện có và phần còn thiếu

Nguồn kiểm trình duyệt là [browser-smoke.mjs](../../apps/vink-web/e2e/browser-smoke.mjs);
`serverHtml`, `mainPhase`, `afterRestartPhase` là các phần trong script, không phải lệnh CLI
mới. Nguồn kiểm PostgreSQL là [tests/postgres.rs](../../services/vink-app/tests/postgres.rs).
Chọn lệnh chạy từ hướng dẫn phát triển, không chạy thao tác phá dữ liệu trên database sử dụng thật.

| Kịch bản / điều cần quan sát | Assertion hiện có | Giới hạn cần giữ |
| --- | --- | --- |
| Nháp thay đổi nhưng người đọc vẫn thấy snapshot công khai, không có dòng riêng | `public_read_serves_the_published_snapshot_not_the_draft`, `server_html_has_public_content_and_never_private_drafts` (`postgres`); `serverHtml` (`web`) kiểm HTML reader và khung Bàn viết không có `DRAFT_ONLY_LINE` | Seed tạo publication; chưa chứng minh gửi duyệt/xuất bản/thu hồi theo ý định tác giả |
| Hydration và chuyển từ reader sang Bàn viết | `mainPhase` kiểm dấu hydration, giữ marker JS qua điều hướng, không có console error/exception | Một Chrome headless desktop; không bao phủ mọi link hoặc trình duyệt |
| Gõ tiếng Việt, giữ dòng/khổ/thụt lề, lưu rồi reload | `vietnamese_text_lines_and_stanzas_round_trip` (`postgres`); `mainPhase` so nội dung textarea, document server và version sau lưu/reload | CDP dùng `Input.insertText`, thử `imeSetComposition` có fallback; không phải IME hệ điều hành. Chưa kiểm Word/Docs paste, undo hay thiết bị thật |
| Chỉ báo đã lưu cho đúng nội dung được xác nhận | Unit tests của [desk_state.rs](../../apps/vink-web/src/desk_state.rs) kiểm phản hồi cũ, sửa tiếp khi request đang bay, một save đang bay và lỗi; `mainPhase` kiểm phiên bản 2 sau nút lưu | Smoke chưa kiểm phím tắt, Enter hoặc UI lỗi mạng; test thuần không chứng minh sự kiện DOM |
| Version cũ không ghi, giữ nội dung để người viết chọn | `stale_expected_version_conflicts_without_writing`, `concurrent_saves_of_one_version_have_exactly_one_winner` (`postgres`); `mainPhase` kiểm conflict và nút giữ bản local | Smoke mô phỏng bên ghi cạnh tranh bằng PUT trong cùng trang, chưa thao tác hai tab thật; test PostgreSQL chạy 8 save đồng thời |
| Quyền đọc/ghi riêng và lỗi API không bị gộp thành thành công | `another_actor_cannot_read_list_or_write_a_private_draft`, `api_distinguishes_outcomes_and_keeps_drafts_private` (`postgres`); `private_routes_are_closed_without_an_auth_authority`, `database_outage_is_reported_not_hidden` trong [http/mod.rs](../../services/vink-app/src/http/mod.rs) (`quick`) | Resolver header chỉ nằm trong test; fixture loopback không chứng minh session, logout/ban hoặc ACC-01/02 |
| Dữ liệu đã xác nhận còn sau restart, process dừng đúng | `confirmed_save_survives_a_new_connection_pool` (`postgres`); `afterRestartPhase` và orchestration trong [web.rs](../../xtask/src/web.rs) (`web`) | Không phải crash/replay/backup-restore; worker chỉ có lifecycle, không có job |
| Rời editor khi chưa lưu | Source `Editor` cài `beforeunload` cho đóng/reload tab | Smoke chưa có assertion cho cảnh báo này. Điều hướng nội bộ “Bàn viết”/“Đọc” chưa cảnh báo; không mô tả như đã bảo toàn mọi trường hợp rời trang |

## Công cụ và bằng chứng ngoài smoke

Web runner chuẩn bị database thử, server và Chrome; khi chạy tới bước tương ứng, nó ghi
`target/verify-web/server.log` và `smoke-state.json`. Thư mục này được làm mới ở lần chạy kế
tiếp; cần giữ bằng chứng có ích trước khi chạy lại nếu nhiệm vụ cần bàn giao. Script hiện chưa
tạo screenshot, video hoặc performance trace. Assertions DOM không chứng minh bố cục đẹp.

Với yêu cầu visual QA, dùng công cụ browser có sẵn để chụp **và mở** ảnh, ghi viewport/theme/
text scale; xem [vink-ui-inspector](../../.agents/skills/vink-ui-inspector/SKILL.md).
IME thật, accessibility với screen reader, mobile thật và hiệu năng cần phiên kiểm riêng có
môi trường và quan sát cụ thể. Không tự đổi chúng thành `PASS` vì lane `web` thành công.

Chưa có đường đi cho đăng nhập thật, gửi duyệt, moderation, publish/withdraw, tuyển chọn Tạp Chí,
bình luận Discourse, AI job, export và nhiều chương. Trạng thái chi tiết và điều kiện làm tiếp
ở [giới hạn skeleton](README.md) và [S-01–S-05](../architecture/decisions.md).

Khi một route, thao tác hoặc assertion đổi, cập nhật hàng liên quan cùng source owner;
[vink-maintain-verification](../../.agents/skills/vink-maintain-verification/SKILL.md) hướng dẫn
phân biệt tài liệu cũ, thiếu coverage và lỗi sản phẩm. Không cần cập nhật theo lịch cố định.
