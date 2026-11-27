# Requirements nền tảng cho Vink

**Phiên bản:** 0.3 · **Ngày:** 30/10/2026, cập nhật 10/11/2026 · **Trạng thái:** Đề xuất phạm vi và tiêu chí nghiệm thu, chưa phải checklist đã PASS.

**Cập nhật 10/11/2026:** chủ sản phẩm chốt 15 quyết định nghiệp vụ
([DecisionDelta](../docs/architecture/product-decisions-20261110.md)). Yêu cầu mới có ID ở
[§14](#14-quyết-định-sản-phẩm-10112026); các dòng cũ bị thay đổi được ghi chú tại chỗ, không đánh
số lại và không xóa lịch sử. Mức triển khai từng phần và bằng chứng nằm ở [§14](#14-quyết-định-sản-phẩm-10112026) và
[registry phát triển](../docs/development/product-decisions-registry.md).

## 1. Mục tiêu của MVP

Chứng minh một vòng lặp hoạt động: **tác giả viết và lưu nháp tin cậy → gửi một snapshot để đăng → được kiểm duyệt → người đọc đọc và phản hồi → tác giả sửa hoặc viết tiếp**. Một tuyển tập nhỏ giúp người mới khám phá nội dung; công cụ vận hành đủ để xử lý lỗi và khiếu nại.

Requirements dưới đây triển khai [Vision](vision.md) và [Tech Stack](../docs/architecture/tech-stack.md). Từ 03/11/2026 Vink sở hữu cả bình luận ([ADR ownership](../docs/architecture/discussion-ownership.md)); acceptance từng gắn với Discourse được thay bằng kiểm chứng tương đương tại module thảo luận của Vink, không bỏ bảo đảm nào.

## 2. Phạm vi và mức ưu tiên

| Mức | Ý nghĩa |
| --- | --- |
| Must — Pilot | Bắt buộc trước khi thử với nhóm được mời; người duyệt có thể giữ vai trò chính, AI hỗ trợ |
| Must — Beta | Bắt buộc trước khi mở public beta hoặc mở chức năng tương ứng |
| Should | Tăng giá trị nhưng có thể lùi mà không làm sai lời hứa cốt lõi |
| Later | Có hướng phát triển, không thuộc cam kết MVP |

Pilot ưu tiên tác phẩm một phần. Mô hình phải giữ định danh tác phẩm/đơn vị văn bản ổn định để bổ sung chương; UI nhiều chương cơ bản là Must khi chủ sản phẩm chọn nó làm tính năng của public beta. Không ép một bài thơ hiển thị như tiểu thuyết.

**10/11/2026 (chủ sản phẩm):** Vink hiện là mạng xã hội các **bài viết ngắn/tản văn**; tác phẩm
nhiều chương **không triển khai** (WR-04 `DEFERRED`). Bài được tổ chức bằng bộ sưu tập cá nhân
(COL-01..06), không phải chương. Vai trò “biên tập Tạp Chí” và “người duyệt chính sách” dưới đây
được giữ làm lịch sử: Tạp Chí hoãn (ED-02 `DEFERRED`), kiểm duyệt mặc định do trợ lý AI thực hiện
và quản trị viên (chủ sản phẩm) là cấp cao hơn (MOD-07..10). **11/11/2026:** quản trị viên ủy
quyền cho moderator theo grant (MOD-11..15); moderator không phải vai trò ngầm định của tài khoản. Nhóm tuổi phục vụ chưa được quyết
định và không thuộc phạm vi hiện tại (D-11 `DEFERRED`).

Các vai trò tối thiểu: khách đọc, tác giả, người duyệt chính sách, biên tập Tạp Chí và quản trị hệ thống. Một người có thể giữ nhiều vai trò trong pilot; quyền thực thi vẫn phân biệt theo hành động. Quyền biên tập, trust level hoặc có tài khoản đăng nhập không tự cấp quyền xuất bản thay tác giả.

## 3. Viết và bản thảo

| ID | Ưu tiên | Yêu cầu | Nghiệm thu tiêu biểu |
| --- | --- | --- | --- |
| WR-01 | Must — Pilot | Nháp riêng có autosave và trạng thái lưu rõ | Refresh giữ nguyên dữ liệu đã được server xác nhận; mất mạng hiển thị phần chưa lưu, giữ buffer đang viết và cho `Thử lại`/`Lưu đè` cùng guard rời trang hai bước; không tuyên bố đã lưu khi chưa thành công; tài khoản khác/khách không đọc được nháp qua UI, API, SSR hoặc URL đoán. **16/11/2026 (#254):** bỏ toàn bộ nút tải bản thảo (`.txt`) và hướng dẫn ngắt dòng khỏi giao diện viết để giữ mặt viết yên tĩnh, vẫn giữ đầy đủ chống mất nội dung. **24/11/2026 (#475):** bỏ hộp thoại Xem trước bản nháp độc lập; lưu nháp vẫn riêng tư và không gửi duyệt |
| WR-02 | Must — Pilot | Soạn và render đúng tiếng Việt, thơ, đoạn văn | Bộ mẫu dấu tiếng Việt, IME, khổ thơ, xuống dòng và paste từ Word/Docs đi qua save → render → edit không mất cấu trúc; kiểm trên desktop và mobile mục tiêu. **24/11/2026 (#475):** giữ Markdown Live Preview trong mặt viết và xem trước kết quả đổi định dạng |
| WR-03 | Must — Pilot | Snapshot gửi duyệt bất biến, working draft có version | Sửa tiếp tạo draft/snapshot mới; hai tab cùng lưu không âm thầm ghi đè; snapshot đã gửi có ID/hash ổn định, có thể đối chiếu lại |
| WR-04 | Must khi mở nhiều chương — **`DEFERRED` 10/11/2026: không triển khai nhiều chương** | Danh sách chương và trạng thái riêng từng chương | Thêm/sắp xếp chương nháp không làm lộ nội dung chưa đăng; URL/ID chương cũ giữ ổn định; trật tự công khai chỉ đổi qua hành động được phép |
| WR-05 | Must — Pilot | Xuất nội dung cơ bản theo định dạng có thể dùng lại | Tác giả có thao tác xuất văn bản của mình, giữ Unicode và ngắt dòng; không phụ thuộc ID của hệ thống ngoài làm khóa chính |
| WR-06 | Should | Xuất đầy đủ lịch sử và metadata | Có thể xuất revisions, metadata và schema version trong phạm vi quyền/retention; phân biệt dữ liệu tác giả với audit nội bộ |

Không cam kết cộng tác thời gian thực hoặc offline-first trong pilot. Local recovery, nếu có, phải phân biệt với bản đã lưu server, tách theo tài khoản và có cách xóa trên thiết bị dùng chung.

**Chốt tiêu đề theo định dạng 16/11/2026 — #263:** Mặc định có tên riêng và body;
Markdown lấy H1 đầu tiên làm tên, không có H1/H1 đầu rỗng thì UI hiện “Chưa đặt tên”.
Không có hai nguồn tên writable; body Mặc định có dấu `#` vẫn là chữ, giữ tên riêng
đến conversion chủ động sang Markdown. Placeholder không vượt validation/publication;
giữ giới hạn tên hiện hành và nguồn khi lỗi. **17/11/2026 — #255:** thay hoàn toàn contract
document bằng một `{format, source}`, không duy trì v1/v2 song song. Đây là hướng
`PRODUCT_CONFIRMED`. Chủ sản phẩm duyệt [đúng kiến trúc r2](../decisions/signoffs/VDR-DOCUMENT-SOURCE-FORMAT.r2.toml)
và cơ chế dừng khi có revision cũ. Viết lại snapshot bất biến (WR-03) hoặc migration dữ liệu thật cần ủy quyền riêng. Bàn viết dùng đúng format nguồn và Reader hiển thị revision đã duyệt qua
[pipeline đọc #264](../docs/development/document-source-format.md#2-pipeline-đọc-web). Xem [contract và giới hạn cutover](../docs/architecture/document-source-format.md) và
[owner title, corpus và dependency #255](../docs/architecture/editor-title-authority.md).

## 4. Xuất bản và rút bài

| ID | Ưu tiên | Yêu cầu | Nghiệm thu tiêu biểu |
| --- | --- | --- | --- |
| PUB-01 | Must — Pilot | Công khai đúng snapshot, có ý định xuất bản của tác giả | Nháp chỉ lưu và mọi bản xem trước riêng đều không xuất hiện công khai dù AI trả Approved; nút “Gửi để đăng khi được duyệt” lưu rõ ý định theo snapshot; publish kiểm lại quyền, trạng thái hiện tại và quyết định duyệt. **10/11/2026:** ý định này là lựa chọn audience “Công khai” gắn đúng snapshot (AUD-01/02) |
| PUB-02 | Must — Pilot | Bản đang công khai độc lập bản đang sửa | rev1 vẫn đọc được khi rev2 Draft/PendingAI/NeedsRevision/Rejected; chỉ transaction publish hợp lệ thay pointer; callback cũ không thay bản mới hoặc đảo quyết định của người duyệt |
| PUB-03 | Must — Pilot | Có cách rút bài và bảo vệ quyền ở mọi bề mặt | Ngay khi rút bài có hiệu lực ở Vink, public API/SSR/search/Tạp Chí không trả phần phải ẩn; đọc/ghi thảo luận của tác phẩm bị chặn cùng lúc; bản sao ngoài Vink có trạng thái riêng. Chỉ báo hoàn tất toàn bộ sau khi xác minh snippet/preview/cache/thông báo trong phạm vi policy đã được thu hồi; lỗi giữ pending và cảnh báo, không khôi phục publication. **10/11/2026:** câu “đọc/ghi thảo luận bị chặn cùng lúc” nay chỉ đúng cho chuyển riêng tư và gỡ do kiểm duyệt; tác giả **xóa** bài từng công khai để lại trang thay thế và các bình luận hợp lệ (PUB-04/05) |

PUB-03 cần định nghĩa rõ phạm vi thu hồi trong hệ thống, thời hạn xử lý và cách xử lý lỗi đồng bộ. Không coi việc xóa card ở trang chủ là hoàn tất. Không hứa thu hồi các bản sao đã được người dùng lưu bên ngoài hệ thống.

**Nghiệm thu PUB-01 (#533, giai đoạn 1).** Bàn viết lưu riêng cả nháp trống. Khi gửi duyệt,
tác giả cần tiêu đề hiển thị được và thân bài có chữ hoặc số ngoài URL. Một chữ thơ là đủ.
Markdown lấy H1 đầu tiên làm tiêu đề và kiểm phần còn lại theo CommonMark. Máy chủ đối chiếu
revision đã chụp trước khi ghi ý định hoặc tạo việc duyệt. Dạng bài vẫn tùy chọn.
[Quy tắc và các ngưỡng chờ duyệt](../docs/architecture/publication-moderation.md#điều-kiện-gửi-duyệt-533).

## 5. Kiểm duyệt và AI

| ID | Ưu tiên | Yêu cầu | Nghiệm thu tiêu biểu |
| --- | --- | --- | --- |
| MOD-01 | Must — Pilot | Tách chính sách cộng đồng khỏi chất lượng văn chương | Bài an toàn nhưng viết yếu vẫn có thể đăng ở Xưởng; AI không từ chối vì không đạt điểm nghệ thuật, độ “chân thành” hoặc thiếu khả năng vào Tạp Chí |
| MOD-02 | Must — Pilot | Kết quả duyệt chỉ áp đúng đối tượng/phiên bản | Gắn revision/hash/policy/decision version; thử callback trùng, sai hash, trễ sau human override và sau rút bài: không có mutation trái phép |
| MOD-03 | Must — Pilot | Timeout/lỗi AI có human review được vận hành | Worker chết, hết retry hoặc quá deadline tạo/đánh dấu case thủ công; tác giả thấy trạng thái và khung xử lý được thông báo, người phụ trách thấy tuổi case; có watchdog, cảnh báo backlog và đối soát; không tự approve vì provider lỗi. **10/11/2026:** không mặc định có người trực và không tự đặt thời hạn xử lý; phần còn hiệu lực là “lỗi không bao giờ thành duyệt” và “giữ retry được hoặc cần quản trị viên” (MOD-08) |
| MOD-04 | Must — Pilot | Giải thích và khiếu nại | Tác giả xem lý do có thể hành động, gửi appeal; quyết định mới ghi người thực hiện và căn cứ; duplicate appeal không tạo nhiều case; được mở lại khi có bằng chứng mới theo quyền moderator. **10/11/2026:** đích xử lý là quản trị viên; kênh gửi chưa kết nối (MOD-10) |
| MOD-05 | Must — Pilot | Phân quyền AI và dữ liệu đầu vào | Prompt injection trong truyện không được thực thi như chỉ thị; worker không tự mở URL/tệp hay dùng tool theo nội dung; output schema bị kiểm tra; nháp riêng không gửi provider khi chưa có hành động/đồng ý phù hợp |
| MOD-06 | Must trước tự động duyệt | Có bộ đánh giá tiếng Việt và chế độ thu hồi tự động hóa | Mẫu có thơ, phương ngữ, lời thoại, trào phúng và nội dung mơ hồ; đo sai lệch/false positives theo loại; ngưỡng cho phép được review; có thể tắt auto-approval mà vẫn giữ human queue. **10/11/2026:** hướng sản phẩm là AI tự duyệt (MOD-07/08), nhưng **bật auto-approval production vẫn cần mục này**; lượt tài liệu/model không bật gì. **25/11/2026 ([#536](https://github.com/loveoverflowcom/vink/issues/536)):** chủ sản phẩm cho ngoại lệ bootstrap “Mặc định là bật cho mock đã, tôi sửa sau”, áp cho tác phẩm ở mọi môi trường kể cả VPS/production. Mock kịch bản duyệt mọi lần gửi công khai hợp lệ, không đọc nội dung và không phải AI hay kiểm duyệt nội dung. Operator chuyển `manual` hoặc dừng consumer bằng cấu hình. Admin dừng tự động (MOD-14) thắng cấu hình. Ngoại lệ không áp cho bình luận. Bật auto-approval bằng AI thật vẫn cần mục này |

Kết quả `NeedsRevision` liên quan tới chính sách phải khác góp ý writing coach. Góp ý văn phong tự nguyện không trở thành điều kiện bắt buộc để được đăng. Confidence do model tự báo không được coi như xác suất đã hiệu chuẩn.

Lịch sử moderation giữ kết luận, policy/model version, thời điểm và actor cần thiết để audit; retention và quyền xem phải được quyết định. Không lưu tràn lan toàn bộ prompt hoặc nội dung riêng vào logs vận hành.

## 6. Cộng đồng và thảo luận

| ID | Ưu tiên | Yêu cầu | Nghiệm thu tiêu biểu |
| --- | --- | --- | --- |
| COM-01 | Must — Pilot | Thread ổn định cho work/chapter | Xuất bản lại không tạo thread mới; retry/đồng thời không làm sinh nhiều thread hữu hiệu cho cùng tác phẩm (ràng buộc unique trong database Vink) |
| COM-02 | Must — Pilot | Tiền kiểm ở owner của bình luận trên mọi cửa đang bật | Tạo qua Web/native/API đều chưa public khi pending; không có đường email/import; nếu cho sửa bình luận công khai thì phải có gate tương ứng; test cả quyền staff/system đã được phép ngoại lệ. **10/11/2026:** hướng đích là evaluator AI quyết định thay cho người có grant (COM-09) và sửa bình luận được mở có gate (COM-10); code hiện tại vẫn là `premoderation.v1` |
| COM-03 | Must — Pilot | User actions thực thi đúng người và đúng quyền | Browser không thể đổi username/user ID để gửi thay tài khoản khác; account mapping lấy từ session; key không xuất hiện trong HTML, WASM, JS, logs hoặc lỗi gửi cho người dùng |
| COM-04 | Must — Pilot | Suy giảm chức năng rõ khi phần thảo luận lỗi | Bài công khai vẫn đọc được; bình luận hiển thị trạng thái phù hợp, không giả gửi thành công; retry không nhân đôi phản hồi; hoạt động trở lại có thể đối soát |
| COM-05 | Must — Pilot | Góp ý có ngữ cảnh và lưu bài | Tác giả chọn mục đích phản hồi; người đọc thấy tín hiệu đó; lưu/bỏ lưu rõ và idempotent theo [SAV-01..04 và SAV-INV-01..05](#18-lưu-bài-để-đọc-lại-saved-works); không dùng điểm sao làm cổng đăng bài |
| COM-06 | Should — Beta | Theo dõi tác giả và cảm nhận nhẹ | Theo dõi/bỏ theo dõi rõ, không tạo bản dữ liệu cạnh tranh ở hai owner; tín hiệu tương tác không tự quyết định quyền đăng hoặc tuyển chọn |
| COM-07 | Must — Pilot | Tác giả bật/tắt bình luận trên từng tác phẩm | Chỉ tác giả tác phẩm đổi được; thao tác idempotent và có version để tab cũ không đè lựa chọn mới (yêu cầu mang đúng version hiện tại luôn được chấp nhận và tăng version, kể cả khi chỉ giữ nguyên chế độ; chủ sản phẩm xác nhận 05/11/2026); tắt chặn bình luận/phản hồi mới tại commit, không xóa hay ẩn bình luận đã có (chủ sản phẩm xác nhận 03/11/2026; tác phẩm mới công khai mặc định bật); độc lập với quyền kiểm duyệt |
| COM-08 | Must — Pilot | Tác giả tác phẩm không sửa/xóa bình luận của người khác | Mọi đường ghi kiểm quyền tại server; tác giả tác phẩm chỉ gỡ bình luận của chính mình; gỡ/duyệt bình luận người khác thuộc authority kiểm duyệt hệ thống |

Ghi nhận văn học trên tác phẩm, bình luận gốc và phản hồi (“Thấm”/“Ưng”; đổi tên tương tác tác phẩm theo #213 ngày 15/11/2026 và tín hiệu bình luận “Hữu ích” → “Ưng” theo #359 ngày 20/11/2026) có PRD riêng với ID `LVE-FR/UC/INV`: [ghi nhận văn học](literary-endorsement.md). LVE kế thừa COM-03..08 và không đổi nghĩa của chúng; tín hiệu này không cấp quyền đăng, kiểm duyệt hay tuyển chọn.

Vink tự làm màn quản trị nhỏ cho bình luận (hàng đợi duyệt có version, lý do, báo cáo, khiếu nại) cùng màn cho tác phẩm, quyết định, appeal và curation. Discourse chỉ là tham khảo thiết kế (nguồn D3–D5 trong [Sources](../docs/references/sources.md)); không có Discourse Admin trong hệ thống.

## 7. Khám phá, tuyển chọn và trải nghiệm đọc

| ID | Ưu tiên | Yêu cầu | Nghiệm thu tiêu biểu |
| --- | --- | --- | --- |
| ED-01 | Must — Pilot | Trang chủ hữu ích khi ít nội dung | Với một tuyển tập nhỏ vẫn có lời dẫn, đề bài hoặc CTA tham gia phù hợp; không có thống kê giả, vùng trống dài hoặc feed vô hạn thiếu nội dung |
| ED-02 | Must — Pilot — **`DEFERRED` 10/11/2026: chưa có Tạp Chí/tuyển chọn** | Curation độc lập moderation và ghim phiên bản | Bài không được chọn vẫn đọc được ở Xưởng; pilot chỉ hiện tuyển chọn khi snapshot được chọn còn là publication hiện hành; đổi bản tạm ngừng mục tuyển chọn chờ biên tập; rút bài đóng hiển thị |
| UX-01 | Must — Pilot | Đọc dài trên desktop/mobile, M3 nhất quán | Giữ thơ/đoạn; không vỡ layout ở màn hẹp mục tiêu và khi tăng cỡ chữ; keyboard focus, screen-reader labels, tương phản sáng/tối được kiểm; public reader có nội dung HTML từ server. **#413 và #576 (26/11/2026).** Chỉ thành viên Active đã xác nhận chỉnh Giao diện trong Cài đặt tại `/toi`, với hai cỡ chữ `Mặc định` (100%) và `Lớn` (150%). Khi chưa có xác nhận Active, Web theo màu hệ thống, chữ 100% và rail theo ngữ cảnh. Giữ phóng chữ/trợ năng tới 200% của trình duyệt và kiểm hình học |
| UX-02 | Must — Beta | Tìm nội dung công khai bằng tiếng Việt | Bộ truy vấn có/không dấu, tác giả, tiêu đề và tags trả kết quả chấp nhận được; draft, revision chưa đăng và bài đã rút không lọt vào kết quả. **10/11/2026:** thêm lọc theo hashtag (HTG), Đề xuất (RANK) và Top 100 (TOP) trên cùng tập đủ điều kiện |
| UX-03 | Must — Pilot | Tìm lại tác phẩm và trạng thái của mình | Người dùng tìm được bản nháp, bài đã đăng, trạng thái duyệt và hành động sửa/khiếu nại; không phải đoán URL hoặc tìm trong thông báo cũ |
| UX-04 | Should — Beta | Hồ sơ nâng cao và thông báo cộng đồng | Có hồ sơ tác giả và đường xem reply/mention phù hợp; chưa cần inbox đa hệ thống hoàn chỉnh |

## 8. Xác thực, dữ liệu và vận hành

| ID | Ưu tiên | Yêu cầu | Nghiệm thu tiêu biểu |
| --- | --- | --- | --- |
| ACC-01 | Must — Pilot | Một định danh Vink ổn định và mapping có chứng cứ | Đổi email/bút danh không đổi tác giả của tác phẩm; account provisioning lỗi không làm tạo sai người; các session và database Cantos không được dùng chung |
| ACC-02 | Must — Pilot | Quyền và trạng thái khóa nhất quán | Đăng nhập không tự cấp quyền biên tập/admin; thử logout/ban/revoke qua Vink và mọi API đang bật; các trường hợp chưa đồng bộ có cơ chế đóng quyền phù hợp |
| ACC-03 | Must — Pilot (10/11/2026) | Đăng nhập chính bằng username + mật khẩu | Xem [§14.6](#146-tài-khoản-acc-03) và mức triển khai trong phần đầu §14 |
| OPS-01 | Must — Pilot | Giao dịch publication/outbox và job có thể phục hồi | Rollback không để pointer và event lệch; worker chết thu hồi lease được; reprocess không làm đăng lại bài đã rút hoặc phát lại quyết định cũ |
| OPS-02 | Must — Pilot | Backup/restore và export có phạm vi rõ | Có bản backup thử, phục hồi trên môi trường tách biệt và đối chiếu tác phẩm, revisions, publication, mapping và jobs; giữ rollback/recovery không làm mất quyền kiểm soát |
| OPS-03 | Must — Beta | Quan sát độ trễ, chi phí và khả năng vận hành | Có dashboard/log đủ biết tuổi hàng đợi, job lỗi, provider cost và backlog; người phụ trách và ngân sách được xác định; chưa có benchmark thì không gắn nhãn “đạt SLA” |

## 9. Các yêu cầu phi chức năng cần đặt ngân sách

Không đưa một số hiệu suất tùy ý thành lời hứa. Trước public beta, đo trên thiết bị/mạng mục tiêu và ghi ngưỡng được chấp thuận cho các mục sau:

| Hạng mục | Đại lượng cần đo | Điều kiện quyết định |
| --- | --- | --- |
| Reader | Thời gian tới văn bản, tải lỗi, byte ban đầu và tương tác | Tiêu chí SSR đang phát hành: đọc được văn bản trước interactivity. Delta SPA #504 chờ signoff r2: văn bản xuất hiện sau khi trình duyệt khởi động và API xác nhận quyền, shell có trạng thái tải/lỗi và hướng dẫn bật JS. Document tĩnh trả 200 và preview chung trước JS. [ADR](../docs/architecture/edge-frontend.md) |
| Editor | Độ trễ gõ/undo, autosave, tài liệu dài và memory | Không phá IME, selection, dấu hoặc trạng thái đã lưu |
| Moderation | Thời gian machine queue, human queue, tỷ lệ lỗi và đảo quyết định | Không đồng nhất model latency với tổng thời gian được đăng |
| Đồng bộ | Outbox age, webhook lag, topic duplicates, reconciliation lag | Có cách phát hiện và sửa trạng thái lệch |
| Thu hồi | Thời gian đóng public read, cache/snippet removal | Nêu rõ điểm hoàn tất và đường phục hồi khi một owner không phản hồi |
| Dữ liệu | RPO, RTO, retention, thời gian export | Có kết quả restore thực, bao gồm mappings và dữ liệu liên quan |
| Chi phí | Token/cost theo mục đích, retry, human minutes/case | Có quota, circuit breaker và đường chuyển xử lý thủ công |

## 10. Các tình huống nghiệm thu xuyên suốt

1. **Người mới viết thơ:** nhập bằng IME, xuống khổ, paste, lưu nháp, refresh, gửi để đăng; độc giả thấy đúng cấu trúc và tác giả thấy rõ trạng thái.
2. **Sửa sau xuất bản:** rev1 đang công khai; rev2 bị yêu cầu sửa; rev1 vẫn đọc được. Khi rev3 được duyệt, callback rev2 muộn không thay publication.
3. **Human override:** người duyệt đổi quyết định trong khi worker còn chạy; worker trả về chỉ ghi nhận kết quả lỗi thời, không đảo quyết định mới.
4. **Gửi bình luận không rõ kết quả:** bình luận đã commit nhưng phản hồi mất; gửi lại cùng khóa idempotency trả đúng bình luận cũ, không tạo bản thứ hai.
5. **Mọi cửa bình luận:** Web/native/API và tính năng edit đang bật tuân policy; đường không được hỗ trợ từ chối có chủ đích, không chỉ ẩn nút.
6. **Rút bài đã tuyển:** public read và tuyển tập không lộ nội dung đã bị rút; snippet thảo luận được xử lý; job/event cũ không tự khôi phục. (10/11/2026: “tuyển tập” hoãn; kịch bản xóa/riêng tư/gỡ nay theo PUB-04/05.)
7. **Account đổi thông tin:** đổi bút danh/email không đổi ownership; client giả `Api-Username` không gửi được thay người khác.
8. **AI hoặc phần thảo luận gián đoạn:** tác phẩm đã công khai vẫn đọc được; submissions không tự approve; người dùng biết nội dung chưa đăng/chưa gửi và có đường xử lý tiếp.

Đây là kịch bản nghiệm thu cần thực hiện khi có implementation, không phải kết quả thử đã chạy trong lần soạn tài liệu này.

## 11. Ngoài phạm vi hiện tại

Later: đồng biên tập thời gian thực; nhóm kín với ACL phức tạp; ghi chú bên lề tự re-anchor qua mọi lần sửa; chat/DM; hệ điểm/karmas; chợ bản quyền, paywall và chi trả tác giả; ứng dụng native; hệ recommendation riêng; chuyển thể audio tự động và SSO chung Cantos.

**10/11/2026:** thêm vào ngoài phạm vi hiện tại: tác phẩm nhiều chương, Tạp Chí/tuyển chọn biên
tập, age gate/ngày sinh/chính sách nhóm tuổi, audience thứ ba (unlisted, bạn bè, nhóm kín), bộ
sưu tập cộng tác, lịch sử chỉnh sửa bình luận công khai. “Hệ recommendation riêng” vẫn là Later ở
nghĩa cá nhân hóa; feed **Đề xuất** theo vote + độ mới (RANK) và **Top 100** (TOP) là phạm vi mới.

Không viết DDL cuối cùng, OpenAPI đầy đủ hoặc model Quint thực thi trong đợt requirements này. Các tài liệu đó cần theo sau quyết định phạm vi, editor schema, identity và integration spike.

Ngoại lệ được đề xuất cho **ngày sinh tùy chọn trên hồ sơ tác giả** nằm ở §15.
[USER-01@r2](../decisions/proposals/USER-01.r2.toml) còn chờ duyệt.

## 12. Literary endorsement (LVE)

Nguồn [epic #73](https://github.com/loveoverflowcom/vink/issues/73); canonical phần lõi được promote
ngày 05/11/2026 trong LVE-PR-01. [Quyết định slice](../docs/architecture/literary-endorsement.md)
chọn policy, alternatives và contract; [registry](../docs/development/endorsement-core.md) giữ
traceability/evidence/gaps. Snapshot handoff r1 không còn normative cho phần lõi đã promote.
Đây là yêu cầu và scope, không phải checklist production PASS. COM-07/08, MOD-01 và private/public giữ nguyên.

| ID | Yêu cầu canonical | Phạm vi PR1 |
| --- | --- | --- |
| LVE-FR-01 | Actor có capability thật chọn/bỏ một tín hiệu tích cực cho work theo revision, root hoặc flat reply; chặn self endorsement | Pure decision; authority thật chưa có |
| LVE-FR-02 | Aggregate có nguồn tách own state; unavailable khác 0 | Checked count + visibility seam; query/read adapter để sau |
| LVE-FR-03 | Desired state, không toggle; retry/duplicate/lệnh đến muộn không nhân hiệu ứng hoặc hồi sinh intent cũ | Every accepted intent tăng order, receipt exact payload; UI/session/ACK để sau |
| LVE-FR-04 | Truy vết văn bản được ghi nhận, không tự chuyển qua rewrite | Work neo immutable RevisionId; comment original identity, edit unsupported. 10/11/2026: khi mở sửa bình luận (COM-10), “Hữu ích” neo revision được ghi nhận (đề xuất, O-06) |
| LVE-FR-05 | Read/write/summary/cursor giữ visibility; không leak draft/pending/withdrawn | Guard/count projection + private own-removal; I/O để sau |
| LVE-FR-06 | Root ranking cần policy/capability thật; replies giữ chronology và hai tầng | Ranking OPEN, không đổi thứ tự discussion. 10/11/2026: xếp hạng **tác phẩm** theo “Đánh giá cao” có policy riêng (RANK/TOP); thứ tự bình luận không đổi |
| LVE-FR-07 | Endorsement độc lập emotion/bookmark/report; không nhập policy reaction, không đổi COM-07 | Module riêng; close text không chặn endorsement |
| LVE-FR-08 | UI có default/selected/pending/uncertain/rejected/unavailable/disabled-reason, keyboard/focus/text200%/reduced motion | Ngoài PR1, giữ yêu cầu epic |
| LVE-FR-09 | Canonical source, rebuild/reconciliation và policy abuse/retention có owner/version; RAM không là durable authority | Delta/checks + transaction contract; retention/account lifecycle OPEN |
| LVE-FR-10 | Fixture/skeleton không giả success; production cần auth/authority/persistence thật | Không route/store giả; epic AC-03 chỉ PARTIAL |

| Use case ID giữ từ #73 | Hành vi phải giữ |
| --- | --- |
| LVE-UC-01 | Khách đọc có visibility; không giả đăng nhập/vote thành công |
| LVE-UC-02 | Chọn work đúng revision, chỉ confirmed sau durable outcome/readback |
| LVE-UC-03 | Root/reply đổi đúng target, không cộng lên parent/work, giữ hai tầng |
| LVE-UC-04 | Bỏ/lặp thao tác không nhân delta; accepted same-state vẫn ghi fence |
| LVE-UC-05 | Pending serialize/coalesce/confirmation và ACK cũ không đè intent mới (UI sau) |
| LVE-UC-06 | Mất ACK là uncertain; cùng key/payload replay lịch sử tách current state |
| LVE-UC-07 | Multi-tab/máy: accepted-order CAS; stale không tự thắng; không hứa latest creation intent xuyên thiết bị |
| LVE-UC-08 | Đổi actor/session/route không áp ACK/read cũ vào scope mới (consumer sau) |
| LVE-UC-09 | Hide/delete/withdraw kiểm tại commit/read; own removal riêng tư không hồi sinh target |
| LVE-UC-10 | Publication revision mới có relation/count riêng; không tự chuyển endorsement |
| LVE-UC-11 | Chặn vote own target; work author không có quyền sửa/xóa vote của người khác |
| LVE-UC-12 | Đóng bình luận chỉ chặn text COM-07; endorsement theo capability/visibility riêng; reopen không reset |
| LVE-UC-13 | Root sorting/cursor/epoch/refresh cần policy; replies chronological (ngoài PR1) |
| LVE-UC-14 | DB/network/summary lỗi không biến thành 0/confirmed hay làm mất góp ý |
| LVE-UC-15 | Abuse/limits có owner, actor server-derived; không suy gian lận từ IP/đồng quan điểm (policy sau) |
| LVE-UC-16 | Accessible name/pressed/busy không chỉ màu/hover; focus ổn (UI sau) |
| LVE-UC-17 | Account removal/quarantine/rebuild không đếm trùng/hồi sinh vote; retention/effectiveness cần policy riêng |
| LVE-UC-18 | Không bắt viết luận để vote; facets/rubric nếu mở là tự nguyện, không là ACL |

Vote không là điểm nghệ thuật khách quan, danh hiệu chuyên môn hay quyền publish/moderation/curation.
Downvote, sao, leaderboard, AI chấm văn, native product app không được mở bởi slice này.

> **Cập nhật 18–20/11/2026 ([Epic #305 R3](https://github.com/loveoverflowcom/vink/issues/305), [#345](https://github.com/loveoverflowcom/vink/issues/345)):** phần danh sách người đã **Thấm** công khai có phân trang (`WorkId + RevisionId`) được bổ sung tại [§16 (`NOTIF-08`)](#16-thông-báo-cộng-đồng-và-danh-sách-thấm-công-khai-notifications-v1) và đề xuất [LVE-PR-02-03-CHOICE-12@r2](../decisions/proposals/LVE-PR-02-03-CHOICE-12.r2.toml) (`PENDING`; `LVE-PR-02-03-CHOICE-12@r1` `DESIGN_PROPOSED` vẫn hiệu lực đến khi có signoff). **Hữu ích** của bình luận bị loại khỏi phạm vi triển khai Epic Notifications (`REMOVED_FROM_SCOPE`) và giữ nguyên baseline hiện hữu. Lịch sử `LVE-FR-01..10` và `LVE-UC-01..18` giữ nguyên.

## 13. Phân loại tác phẩm (TAG)

Nguồn [epic #94](https://github.com/loveoverflowcom/vink/issues/94), lát lõi
[#95](https://github.com/loveoverflowcom/vink/issues/95) (TAG-PR-01, 08/11/2026).
[Quyết định slice](../docs/architecture/work-taxonomy.md) ghi thuật ngữ, CHOICE-01/02/03, đánh đổi và
điều kiện xem lại; [registry](../docs/development/taxonomy-core.md) giữ requirement → function → kiểm
chứng và gap. Đây là yêu cầu và scope, không phải checklist production PASS. Liên quan WR-01/03,
PUB-01..03, UX-02 và D-01/05/06/12; không đổi nghĩa của chúng hay của LVE/COM.

| ID | Yêu cầu | Phạm vi TAG-PR-01 |
| --- | --- | --- |
| TAG-FR-01 | Catalog do curator quản; mỗi thẻ có danh tính bền vững, nhãn, slug, facet, mô tả, trạng thái; tác giả chỉ chọn thẻ có sẵn | Pure decision + port; store ở #96; entrypoint curator (lệnh operator) và API ở #97 ([API](../docs/development/taxonomy-api.md)) |
| TAG-FR-02 | Hai facet đóng: dạng bài (≤ 1) và chủ đề (≤ 3 thẻ khác nhau sau alias/khử trùng); nháp không bắt buộc dạng bài | Hằng có lý do + type `Option`; bắt buộc khi gửi duyệt còn OPEN |
| TAG-FR-03 | Chữ tiếng Việt: NFC/NFD tương đương; slug giữ dấu; bỏ dấu chỉ để tìm ứng viên, không quyết định danh tính/alias | `TagLabel`, `TagSlug`, `SearchKey`, `TagQuery` |
| TAG-FR-04 | Alias do curator tạo, trỏ thẻ canonical đang hoạt động; không chuỗi, vòng, tự trỏ hay dangling | `decide_catalog_change` (AddAlias), `rank_matches` |
| TAG-FR-05 | Đổi tên giữ danh tính; slug cũ vẫn dẫn về thẻ; slug không bao giờ được gán cho thẻ khác | Rename + `resolve_slug`; unique index ở #96 |
| TAG-FR-06 | Ngừng dùng: không nhận dùng mới, giữ tham chiếu cũ (nháp giữ được, công khai không hiện), kế nhiệm chỉ là gợi ý | Retire + `decide_tag_change` + `public_tags` |
| TAG-FR-07 | Thẻ nháp riêng tư, có version; mọi lệnh được chấp nhận tăng version; lệnh cũ là xung đột kèm trạng thái quan sát, không ghi | `decide_tag_change`, `Vink::save_work_tags` |
| TAG-FR-08 | Thẻ công khai chỉ đến từ snapshot metadata bất biến được bind cùng publication; sửa thẻ không đổi hash v1, revision hay anchor “Đánh giá cao” | `MetadataSnapshot`, `MetadataDigest`, `public_tags`; binding/publish ở owner publication; chiếu lên card/publication và lọc theo thẻ ở #98 ([discovery](../docs/development/discovery-api.md)) |
| TAG-FR-09 | Thẻ không cấp quyền, không xuất bản, không tuyển chọn, không kiểm duyệt, không là điểm chất lượng | Outcome chỉ có metadata; không caller nào dùng thẻ làm ACL |
| TAG-FR-10 | Lỗi phân biệt: chưa xác thực, bị đình chỉ, không tìm thấy (gồm của người khác), xung đột, input sai, bị từ chối theo catalog, không khả dụng, lỗi nội bộ | `TagError`, `TagRejection`, `TagInput::field` |

| Use case | Hành vi phải giữ |
| --- | --- |
| TAG-UC-01 | Chọn/xóa thẻ nháp: lưu có version, thứ tự chủ đề của tác giả được giữ |
| TAG-UC-02 | Trùng/alias: chọn cùng thẻ hai lần (trực tiếp và qua alias) còn một danh tính, không tính hai lần |
| TAG-UC-03 | Chưa có thẻ: danh tính lạ bị từ chối rõ, không tạo thẻ, không có đề xuất giả |
| TAG-UC-04 | Thẻ ngừng dùng: không thêm mới (kèm gợi ý kế nhiệm), giữ được trên nháp đã có, ẩn khỏi bản công khai |
| TAG-UC-05 | Đổi nhãn/slug: danh tính giữ nguyên, URL cũ chuyển hướng, nhãn mới hiện ở mọi nơi |
| TAG-UC-06 | Sửa thẻ nháp không đổi thẻ đang công khai hay danh tính revision |
| TAG-UC-07 | Lệnh từ tab cũ: xung đột, không ghi, trả trạng thái hiện tại để đối chiếu |
| TAG-UC-08 | Người khác sửa: không phân biệt được với tác phẩm không tồn tại, không ghi |
| TAG-UC-09 | Tìm có/không dấu: `ma` tìm thấy `má`/`mà`/…; bản có dấu đúng xếp trước; chung khóa tìm không thành cùng thẻ |
| TAG-UC-10 | Mất ACK: gửi lại là xung đột; trạng thái quan sát cho biết mong muốn đã đúng, không giả là receipt |

| Bất biến | Thuộc tính |
| --- | --- |
| TAG-INV-01 | Slug → thẻ không bao giờ đổi; mỗi thẻ đúng một slug canonical; ghi slug chỉ cho chính thẻ được thay đổi |
| TAG-INV-02 | Alias chỉ được tạo cho thẻ canonical đang hoạt động; resolve không đi qua chuỗi |
| TAG-INV-03 | Thẻ nháp được chấp nhận: ≤ 1 dạng bài, ≤ 3 chủ đề khác nhau, đúng facet, đều có trong catalog, không trùng giữa hai ô |
| TAG-INV-04 | Thẻ ngừng dùng không bao giờ được thêm mới vào nháp |
| TAG-INV-05 | Lưu được chấp nhận khi và chỉ khi actor là chủ, đang hoạt động và expected = version hiện tại; version tăng đúng 1, không wrap |
| TAG-INV-06 | Search key trùng không bao giờ suy ra cùng danh tính |
| TAG-INV-07 | Công khai chỉ chiếu từ snapshot được bind; lưu nháp không ghi snapshot, binding hay revision |
| TAG-INV-08 | Thay đổi catalog chỉ bởi curator, có CAS version; mọi thay đổi được chấp nhận tăng version |

## 14. Quyết định sản phẩm 10/11/2026

Các record của `PRODUCT-DECISIONS-20261110` và `ADMIN-MODERATION-20261111` đã nhập, cùng các
revision có signoff về sau, tra tại [kho TOML](../decisions/README.md).
[DecisionDelta](../docs/architecture/product-decisions-20261110.md) lưu snapshot của lượt 10/11 và 11/11.

<!-- vdr: active USER-02@r1 -->
<!-- vdr: active USER-08@r2 -->
<!-- vdr: active O-05@r2 -->
<!-- vdr: active O-12@r2 -->

Cột “Trạng thái” dùng nhãn của tài liệu đó: phần **luật** là `PRODUCT_CONFIRMED` trừ khi ghi khác;
cột “Đề xuất UX/dữ liệu” luôn là `DESIGN_PROPOSED`. **Hiện trạng triển khai từng phần:** `#150`, `#151`, `#157` có lõi thuần + application planning cho publication, moderation evaluator/admin và tombstone lifecycle; `#158` có PostgreSQL + HTTP API v1 cho snapshot, `request-public`, `set-private` và đọc trạng thái của tác giả ([runtime](../docs/development/publication-runtime.md)); `#152` có backend native username/mật khẩu trong Identity + App ([ADR D-05 §12](../docs/architecture/auth-authority.md#12-đổi-hướng-10112026-username--mật-khẩu-là-chính)). [Auth Web](../docs/development/auth-web.md) có đăng ký, đăng nhập và thêm mật khẩu. Các phần còn lại (xóa/gỡ ở tầng SQL/HTTP/UI, worker/provider AI, bộ sưu tập, sửa bình luận, hashtag, xếp hạng, phản đối) chưa có runtime. **11/11/2026** (`ADMIN-MODERATION-20261111`,
[DecisionDelta §7](../docs/architecture/product-decisions-20261110.md#7-bổ-sung-11112026--quản-trị-kiểm-duyệt)):
admin tổng, grant moderator, nhận ca, phản đối cả quyết định của moderator, dừng tự động, điều tra
riêng và trang “Nội dung đã được ẩn” thay luật cũ của PUB-05 B, MOD-09 và MOD-10; có core thuần,
model và replay (chưa migration/API/worker/UI). Model
và gap từng action ở [registry](../docs/development/product-decisions-registry.md). Không dòng nào đổi nghĩa
của COM-07/08, MOD-01/05, INV-01..08.

### 14.1. Audience, xuất bản và xóa (AUD, PUB)

Hành vi chi tiết: [xuất bản và kiểm duyệt](../docs/architecture/publication-moderation.md).

| ID | Luật | Nghiệm thu tiêu biểu | Đề xuất UX/dữ liệu |
| --- | --- | --- | --- |
| AUD-01 | Bài viết có đúng hai audience: **Chỉ mình tôi** và **Công khai**. Không unlisted, bạn bè, nhóm kín hay ACL chia sẻ | API/schema không nhận giá trị thứ ba; không có đường đọc bằng “link bí mật” | Mặc định “Chỉ mình tôi”; bộ chọn hai lựa chọn có mô tả hệ quả |
| AUD-02 | Audience **không phải** trạng thái xuất bản: tách khỏi nháp/chờ duyệt/được duyệt/đã xóa. Chọn công khai không bỏ kiểm duyệt; Approved không tự công khai bài chỉ lưu riêng; ý định công khai của tác giả gắn đúng snapshot/version | Lưu riêng → evaluator không được gọi. Công khai snapshot r → chờ kết quả của r. Về “Chỉ mình tôi” rồi kết quả Approved tới → không công khai. Tab cũ không đảo lựa chọn mới hơn | Tác giả thấy riêng: audience, kết quả kiểm duyệt từng bản, bản đang công khai. Lưu `generation` của ý định audience |
| PUB-04 | Tác giả xóa bài **đang công khai**: bài rời Khám phá, tìm kiếm, hashtag, Đề xuất, Top 100, bài liên quan; người khác không đọc được nội dung; URL cũ mở trang thay thế “Nội dung đã được tác giả xoá”; bình luận hợp lệ đã có vẫn đọc được ở đó. **11/11/2026 (`O-05` đóng):** áp dụng cả khi xóa bài đang ở trang ẩn sau khi từng công khai (PUB-05/07); bài chưa từng công khai không để lại gì | Guest mở URL cũ: không tiêu đề/nội dung/preview; thấy bình luận đã duyệt, không thấy pending/rejected/removed của người khác. Không bình luận nào bị xóa theo bài | HTTP `410` kèm trang; `noindex`; xác nhận trước khi xóa nêu rõ không khôi phục được. Lưu thời điểm xóa; không purge tự động (O-08) |
| PUB-05 | Bốn trường hợp tách bạch: **A** tác giả xóa bài từng công khai (PUB-04); **B** tác giả chuyển riêng tư; **C** admin gỡ cả bài vì vi phạm; **D** chưa từng công khai. **11/11/2026:** B của bài **đang công khai** để lại trang **“Nội dung đã được ẩn”** ở URL cũ cùng bình luận đủ điều kiện (PUB-07); xóa từ trang đã ẩn giữ bình luận như A. C và D trả không tìm thấy | C/D: URL cũ trả không tìm thấy, không trả bình luận, kể cả với link/thông báo cũ. A/B: không tiêu đề, thân bài, preview hay listing. Preview/trích dẫn do hệ thống sinh từ bài không còn đọc được; chữ người khác tự viết trong bình luận không bị thu hồi hàng loạt | Quyền ghi trên trang thay thế/trang ẩn: không ghi mới (O-03). Thay luật cũ “B ẩn cả thảo luận” |
| PUB-06 | Gửi duyệt **theo phiên bản**. Tạo, lưu, sửa bài riêng tư không mở ca. Chỉ yêu cầu công khai của tác giả kích hoạt kiểm duyệt đúng snapshot. Bản đang đăng vẫn hiển thị khi bản sửa chờ/bị từ chối. Công khai lại chỉ dùng quyết định duyệt **còn hiệu lực** cho đúng snapshot và đúng phiên bản chính sách; policy đổi → lượt duyệt mới; quyết định của người được đưa lại cho người có thẩm quyền xem xét. Chuyển riêng tư đóng ca duyệt và phản đối của tác phẩm, giữ lịch sử quyết định | Kết quả AI hay kết luận người tới sau khi chuyển riêng tư: không tạo quyết định, không đổi bản công khai. Công khai lại bản đang chờ mà ca đã đóng: mở lượt mới. Bản được AI duyệt ở policy cũ: lượt AI mới; bản người duyệt ở policy cũ: về hàng chờ người | Tác giả thấy “Đang chờ kiểm duyệt” khi gửi lại; lịch sử giữ quyết định cũ kèm policy version |
| PUB-07 | Bài **đang công khai** chuyển “Chỉ mình tôi”: URL cũ chỉ hiện **“Nội dung đã được ẩn”** và bình luận đủ điều kiện; tiêu đề, thân bài, preview rút khỏi mọi đường đọc công khai, tìm kiếm, Khám phá, Đề xuất. Bài chưa từng công khai và bài bị admin gỡ trả không tìm thấy. Công khai lại thay trang ẩn bằng bài; xóa từ trang ẩn để lại trang thay thế của PUB-04 | Guest mở URL cũ của bài đã ẩn: không tiêu đề/nội dung, thấy bình luận đã duyệt; người viết thấy thêm bình luận chờ/bị từ chối của mình. Ẩn rồi xóa: bình luận vẫn đọc được. Ẩn rồi công khai lại: bài trở lại | HTTP `200` kèm `noindex` cho trang ẩn (đề xuất); trang ẩn không hiện tên tác giả, thẻ, hashtag (như O-09) |

### 14.2. Kiểm duyệt bằng trợ lý AI, quản trị viên, moderator và phản đối (MOD-07..15)

Theo [quyết định M01–M09 ngày 28/11](https://github.com/loveoverflowcom/vink/issues/639), tác giả đọc lời nhắn đã lưu của từng quyết định trong chi tiết riêng tư. Mock nói rõ chế độ thử nghiệm, kể cả khi duyệt. Lời nhắn bằng văn bản thuần, ưu tiên 1–3 câu. Bản duyệt cũ thiếu lời nhắn chỉ hiện trạng thái. Với bản bị từ chối thiếu lý do, thông báo rõ lý do chưa được lưu. Giữ nguyên căn cứ lịch sử và ẩn danh người duyệt.

M04 hoãn hậu kiểm bài cũ khi đổi provider. M08 hoãn đánh giá lời giải thích. M05/M06 dành cho AI tương lai: khi từ chối, nêu căn cứ, đoạn liên quan và hướng xử lý nếu xác định được; khi duyệt, cảnh báo nhạy cảm chỉ mang tính tham khảo. M09 dành cho Nhận xét tương lai: dùng chung cấu trúc kết quả, với chính sách, quyền và phiên bản đích riêng. Các quy tắc gửi bản mới và can thiệp có thẩm quyền tiếp tục áp dụng.

| ID | Luật | Nghiệm thu tiêu biểu | Đề xuất UX/dữ liệu |
| --- | --- | --- | --- |
| MOD-07 | Evaluator trả kết quả **có cấu trúc** gắn target, revision/content digest, policy version, evaluation/job identity, outcome và lý do. Chỉ kết quả hợp lệ, còn hiệu lực, đúng phiên bản và đủ authority mới tạo quyết định nghiệp vụ. Hiện tại: contract + mock scripted, **không** provider thật. Từ #536, mock `approve` chạy mặc định cả production theo ngoại lệ bootstrap ở MOD-06; lịch sử truy được job và kịch bản đã gắn | Sai revision, trùng, sai policy, sau khi target bị xóa/gỡ, sau admin override: không mutation. Output không hợp lệ không thành duyệt | Kết quả lưu append-only: target, digest, policy, job, outcome, mã lý do, thời điểm; không lưu prompt/nội dung riêng vào log |
| MOD-08 | AI là luồng duyệt **mặc định**; không mặc định có người trực; **không** cam kết thời hạn. Timeout, lỗi, output hỏng hoặc không đủ căn cứ: không công khai mới; giữ trạng thái retry được hoặc cần quản trị viên. Cờ “cần quản trị viên” không phải bằng chứng đã có người nhận | Mọi outcome khác Approve giữ nguyên thứ đang công khai. Không có chuỗi “đã chuyển cho người duyệt” khi chưa có kênh | Chữ trạng thái không nêu thời hạn. Có đếm số lần thử và trần retry (giá trị để PR runtime đo) |
| MOD-09 | Trợ lý AI là actor dịch vụ có capability giới hạn, không phải admin hay moderator. **Admin tổng** quản lý quyền kiểm duyệt, xử lý phản đối cuối cùng và vận hành chế độ duyệt tự động; **moderator** quyết định trong phạm vi grant (MOD-11). Quyết định của người (admin hoặc moderator) trên phiên bản đang xét **thắng** callback AI đến muộn hoặc retry; quyết định của phiên bản cũ không áp cho nội dung mới; callback không hồi sinh nội dung đã xóa/bị chặn. Người duyệt không vượt privacy/audit: không quyết định trên bản chỉ lưu riêng, không công khai thay tác giả | AI reject tới sau moderator/admin approve cùng revision: không đổi. Duyệt bản không có ý định công khai: không công khai | Quyết định lưu actor, căn cứ quyền (admin hoặc grant), lý do bắt buộc, revision/decision version được xét |
| MOD-10 | Chủ nội dung phản đối quyết định bất lợi của **trợ lý AI hoặc moderator**; đích cuối là quản trị viên. Một hồ sơ cho mỗi phiên bản quyết định bất lợi; gửi lại (cùng key hay key khác) trả hồ sơ đã có. Khi hồ sơ mở chỉ admin kết luận trên quyết định đó; giữ nguyên hay đảo đều ghi lý do và tạo phiên bản quyết định mới; kết luận admin là cấp cuối. Hiện tại **không** gửi mail/thông báo; phân biệt: mở dialog, tạo hồ sơ, gửi thông báo, admin nhận, admin xử lý | Phản đối trùng: một hồ sơ. Phản đối phiên bản quyết định cũ: `stale_decision`. Phản đối quyết định của admin: `final_decision`. Kết luận cho bản cũ chỉ công khai bản đó nếu ý định công khai hiện hành vẫn trỏ đúng bản. Không trạng thái nào báo “đã gửi” khi kênh chưa kết nối | Copy: “Cần quản trị viên xem xét. Kênh gửi yêu cầu chưa được kết nối; chưa có thông báo được gửi.” Thay luật cũ “chỉ phản đối quyết định của AI” |
| MOD-11 | **Ủy quyền.** Admin cấp/thu hồi quyền duyệt tác phẩm và quyền duyệt bình luận **độc lập**, cho cá nhân hoặc nhóm do admin quản lý; mỗi grant áp toàn Vink hoặc một `WorkId`, có thể hết hạn. Grant bình luận hiện hành (`vink-app moderator grant`) được kế thừa trong phạm vi bình luận. Quyền, tư cách tài khoản và thành viên nhóm được kiểm lại ở mỗi lần đọc hoặc xử lý. Người duyệt tự né nội dung của mình và bình luận thuộc bài do mình viết | Grant bình luận không duyệt được tác phẩm; grant cho bài W không duyệt được bài khác; grant hết hạn, bị thu hồi, rời nhóm hoặc tài khoản bị khóa: từ chối ở lần kiểm kế tiếp | Audit cấp/thu hồi ghi admin, grantee, capability, phạm vi, hạn, lý do |
| MOD-12 | **Nhận ca.** Mỗi ca có tối đa một người nhận; người nhận trả ca, ghi chú nội bộ hoặc chuyển lên admin. Người nhận mất quyền thực tế → ca về hàng chờ, trừ khi họ còn một grant hợp lệ khác. Admin nhận lại ca với lý do. Mọi kết luận kiểm phiên bản ca và phiên bản quyết định: hai thao tác đồng thời chỉ một được áp dụng | Hai moderator tranh ca: người sau `stale_case`. Thu hồi grant/rời nhóm khi đang xử lý: kết luận bị từ chối và ca về hàng chờ. Lịch sử phân biệt đã ghi nhận / đã có người nhận / đã có kết luận | Ghi chú nội bộ tách khỏi lý do gửi người viết; không hiển thị tên người nhận cho người viết |
| MOD-13 | **Can thiệp.** Moderator duyệt, từ chối hoặc ghi đè AI trong phạm vi grant; moderator tác phẩm bác bản đang đăng để ngừng hiển thị bản đó; chỉ admin gỡ cả bài (kết thúc quyền đọc qua URL cũ). Moderator bình luận ẩn và khôi phục **đúng bản đã duyệt**; gỡ của người viết và gỡ theo cơ chế cũ vẫn có hiệu lực; khôi phục dùng lại quy tắc hiển thị gốc và phản hồi hiện hành | Moderator gỡ cả bài: `not_admin`. Khôi phục sau khi bình luận đổi phiên bản hoặc người viết đã gỡ: từ chối | Lý do bắt buộc với từ chối/ẩn/gỡ |
| MOD-14 | **Dừng tự động.** Admin dừng AI áp quyết định: việc mới và việc AI đang chờ sang hàng chờ người có quyền; kết quả AI đang chạy mất hiệu lực. Bật lại chỉ áp cho lượt gửi mới; ca đã chuyển tiếp tục theo luồng người duyệt | Kết quả AI tới sau khi dừng: `automation_paused`, không mutation. Bật lại: job cũ đã chuyển không quay về AI | Trạng thái dừng/bật có phiên bản, actor và lý do |
| MOD-15 | **Điều tra riêng.** Một admin đang có quyền và không xung đột lợi ích mở hồ sơ khi có căn cứ vi phạm cụ thể, nguồn bằng chứng và đúng ID **bản từng công khai** của bài đã chuyển riêng tư. Mỗi lần đọc được kiểm quyền riêng và ghi audit trước khi trả đúng snapshot đó. Hồ sơ chỉ ghi kết luận và căn cứ; quyền kết thúc khi hồ sơ đóng hoặc điều kiện truy cập hết hiệu lực | Đọc sai bản, bản chưa từng công khai, hồ sơ đã đóng/hết hạn, admin mất quyền hoặc là tác giả: từ chối, không audit “đã đọc” | Mặc định quyền theo từng lần đọc; không có quyền đọc hàng loạt hay xuất bản thay tác giả |

### 14.3. Bình luận (COM-09..11)

Hành vi chi tiết: [sửa bình luận](../docs/architecture/comment-editing.md),
[admission](../docs/architecture/discussion-admission.md#8-hướng-đích-sau-quyết-định-10112026).

| ID | Luật | Nghiệm thu tiêu biểu | Đề xuất UX/dữ liệu |
| --- | --- | --- | --- |
| COM-09 | Bình luận gốc và phản hồi đi qua evaluator: gửi → chờ đánh giá → kết quả hợp lệ → áp quyết định. Không đăng trước kiểm sau; email đã xác minh không là “đáng tin”; timeout/lỗi không là duyệt. Không còn yêu cầu “chỉ người đáng tin mới được AI duyệt” | Bình luận mới không công khai trước khi có quyết định hợp lệ cho đúng nội dung đó | Người viết thấy “Đang chờ kiểm duyệt” trên bình luận của mình như hiện nay |
| COM-10 | Người viết sửa bình luận/phản hồi **của chính mình**; tác giả bài không sửa được bình luận người khác. Bản sửa có revision và đi qua kiểm duyệt; không ghi đè chữ công khai bằng nội dung chưa duyệt. Bản sửa được công khai có dấu **“Đã sửa”**; lưu không đổi nội dung không tạo dấu, không đổi thời gian. Không mở lịch sử chỉnh sửa công khai | Trong lúc chờ: người khác vẫn đọc bản đã duyệt. Bản sửa bị từ chối: bản cũ ở lại, không dấu. Hai tab: bản cũ hơn bị từ chối `stale`. Mất ACK: gửi lại cùng key trả kết quả cũ. Xóa trong lúc chờ: callback không hồi sinh | Người viết thấy “Bản sửa đang chờ duyệt”. Lưu revision bình luận bất biến, version của ý định sửa, receipt theo key. “Hữu ích” neo revision (O-06) |
| COM-11 | Trên trang ẩn (PUB-07) hoặc trang thay thế sau khi tác giả xóa (PUB-04), phần bình luận cho đọc, xử lý ca đã gửi và cho người viết tự gỡ. Chặn bình luận, phản hồi, bản sửa và tương tác mới. Kiểm duyệt bình luận dùng snapshot bình luận và giữ kín thân bài. Khi công khai lại, thông báo cho tác giả bài chỉ áp dụng với sự kiện mới sau lúc công khai, bỏ qua sự kiện phát sinh lúc ẩn | Gửi bình luận trên trang ẩn: `hidden_read_only`. Bình luận gửi trước khi ẩn, được duyệt lúc ẩn: hiện, không thông báo tác giả sau khi công khai lại | Ngữ cảnh evaluator của bình luận không chứa thân bài khi bài không công khai |

### 14.4. Bộ sưu tập cá nhân (COL)

Hành vi chi tiết: [bộ sưu tập](../docs/architecture/collections.md).

| ID | Luật | Nghiệm thu tiêu biểu | Đề xuất UX/dữ liệu |
| --- | --- | --- | --- |
| COL-01 | Tác giả tổ chức **bài của chính mình** vào bộ sưu tập cá nhân (như thư mục), không phải sách nhiều chương; không cộng tác, không quản lý bài người khác | Thêm bài của người khác bị từ chối; người khác không sửa được bộ sưu tập | Tên ≤ 80 ký tự, mô tả tùy chọn; tên không cần duy nhất |
| COL-02 | Bộ sưu tập có đúng hai audience: Chỉ mình tôi / Công khai | Như AUD-01 | Mặc định “Chỉ mình tôi” |
| COL-03 | Hiển thị là phép **AND**: `membership(bài, bộ sưu tập)` ∧ `can_read_collection(người xem, bộ sưu tập)` ∧ `can_read_post(người xem, bài)`; `can_read_post` xét audience, xuất bản/kiểm duyệt/lifecycle và quyền hiện tại | Bảng chân trị bốn dòng cho khách/người khác; chủ sở hữu đọc qua đường owner, không qua projection công khai hay cache dùng chung | — |
| COL-04 | Bài công khai nằm trong bộ sưu tập riêng tư vẫn đọc độc lập được, nhưng **không lộ** tên/ID/membership của bộ sưu tập đó. Số đếm chỉ đếm thứ người xem thấy | Trang bài không nêu bộ sưu tập riêng tư; count của bộ sưu tập công khai không tính bài ẩn | Số bài hiển thị = số bài người xem đọc được |
| COL-05 | Khi đọc bài thuộc bộ sưu tập được phép xem có link sang bài liên quan trong bộ sưu tập; từng đích qua cùng phép kiểm quyền; bài riêng tư/đã xóa không vào next/previous/related | Link chỉ dẫn tới bài người xem đọc được qua bộ sưu tập người xem đọc được | Thứ tự theo tác giả sắp (O-02) |
| COL-06 | Xóa bộ sưu tập hoặc bỏ membership không xóa bài; đổi audience bộ sưu tập không đổi audience từng bài; phân trang quyết định theo quyền của **lần đọc đó** | Sau mỗi thao tác, trạng thái bài không đổi; trang kế không trả bài vừa thành riêng tư | Bài thuộc nhiều bộ sưu tập (O-01) |

### 14.5. Hashtag, Đề xuất và Top 100 (HTG, RANK, TOP)

Hành vi chi tiết: [hashtag](../docs/architecture/hashtags.md),
[Đề xuất và Top 100](../docs/architecture/discovery-ranking.md).

| ID | Luật | Nghiệm thu tiêu biểu | Đề xuất UX/dữ liệu |
| --- | --- | --- | --- |
| HTG-01 | Tác giả dùng hashtag như mạng xã hội để người đọc click/lọc/tìm | Hashtag trong bài công khai là link tới danh sách bài cùng hashtag | CHOICE-04 A: trích từ nội dung theo grammar |
| HTG-02 | Hashtag **tách** khỏi catalog dạng bài/chủ đề curated: không áp “một dạng bài, ba chủ đề”, không tự chuyển catalog thành hashtag hay ngược lại | Hashtag trùng chữ với slug catalog không thành thẻ catalog; hai tham số lọc riêng | — |
| HTG-03 | Nhãn và danh tính tiếng Việt: NFC/NFD tương đương; bỏ dấu chỉ để **tìm ứng viên**, không đồng nhất hashtag | `#thơ` ≠ `#tho`; gõ `tho` gợi ý được `#thơ` | Danh tính = NFC + gập hoa/thường, giữ dấu |
| HTG-04 | Chỉ hashtag của bài đủ điều kiện công khai vào index, autocomplete, count và feed công khai | Hashtag của bài riêng tư/chờ duyệt/đã xóa không xuất hiện ở bất kỳ gợi ý hay số đếm nào | Index là projection theo revision công khai |
| HTG-05 | Index cập nhật khi bài đổi quyền, bị xóa hoặc đăng revision mới | Đổi sang riêng tư/xóa: bài rời trang hashtag cùng commit; revision mới: hashtag theo chữ mới | — |
| HTG-06 | Chưa làm UI hashtag hay search engine mới trong lượt này | — | Giới hạn 10 hashtag/bài (O-13) |
| RANK-01 | Feed **Đề xuất** kết hợp số “Đánh giá cao” hợp lệ và độ mới của bài. Công thức/trọng số/cửa sổ/timestamp **chưa chốt** | — | CHOICE-02 A `recommend.v1` |
| RANK-02 | Không dùng “Hữu ích” của bình luận làm “Đánh giá cao” của bài; không dùng vote của nội dung không đủ điều kiện công khai | Count lấy từ LVE của revision công khai hiện tại | — |
| RANK-03 | Cùng tuổi bài, thêm vote hợp lệ không làm điểm giảm; cùng vote, bài mới hơn không bị điểm thấp hơn; tie-breaker ổn định | Kiểm trên toàn miền nhỏ của model | Tie-break: mới hơn trước, rồi ID |
| RANK-04 | Không ưu tiên theo tác giả, độ dài bài hay “điểm văn học AI”; sửa thông thường không bump bài | Sửa bài không làm tăng thứ hạng | Độ mới = thời điểm công khai lần đầu (O-14) |
| TOP-01 | **Top 100** là chế độ riêng với Đề xuất: lọc tập đủ điều kiện, sắp theo “Đánh giá cao” hợp lệ giảm dần, tie-breaker xác định, phân trang trong tối đa 100 bài | Hai danh sách cho thứ tự khác nhau trên cùng corpus | — |
| TOP-02 | Độ mới không đưa bài ít vote lên trên bài nhiều vote; chỉ là tie-breaker | — | — |
| TOP-03 | Bộ lọc, visibility và eligibility áp **trước** khi lấy Top 100 | Bài đủ điều kiện khớp bộ lọc không bị mất vì 100 bài toàn cục | — |
| TOP-04 | Cursor/epoch có ngữ nghĩa rõ khi vote đổi, unlike, bài bị xóa/riêng tư, kiểm duyệt đổi, tải trang kế. Snapshot xếp hạng không vượt quyền đọc hiện hành; không hứa keyset thường tự bảo đảm không thiếu/trùng | Bài bị ẩn giữa hai trang không được trả; không bài nào lặp trong một snapshot | CHOICE-03 A; cursor hết hạn → “Danh sách đã thay đổi, tải lại” |

### 14.6. Tài khoản (ACC-03)

| ID | Luật | Nghiệm thu tiêu biểu | Đề xuất UX/dữ liệu |
| --- | --- | --- | --- |
| ACC-03 | **Username + mật khẩu** là đăng nhập chính. Mọi tài khoản chưa retired có đúng một email đã xác minh. Đăng nhập chỉ bằng mã email vẫn được hỗ trợ. Username đăng nhập **khác** bút danh công khai; không yêu cầu trùng nhau. Đổi hướng không được bỏ session/rate-limit/security guard hiện có | Đăng nhập bằng username + mật khẩu tạo cùng loại phiên như hiện nay; bút danh đổi không đổi username; lỗi đăng nhập không tiết lộ username có tồn tại | Username: ASCII thường `a–z 0–9 _ .`, 3–30 ký tự, duy nhất không phân biệt hoa thường, không hiển thị công khai. Mật khẩu: ≥ 10 ký tự, không luật ký tự bắt buộc, Argon2id. **11/11/2026 (`O-12@r2`, `PRODUCT_CONFIRMED`):** chuẩn hóa NFKC rồi đếm 10–128 ký tự, mỗi Unicode code point là một ký tự; `Password::parse` đã theo (trước đó kiểm 10–128 byte UTF-8). Ngày 13/11/2026 chủ sản phẩm xác nhận khôi phục qua email trong luồng riêng (#181); chi tiết [O-11@r2](../decisions/proposals/O-11.r2.toml) còn `PENDING`, hiệu lực kho vẫn `O-11@r1 OPEN` |

**Hiện trạng `persistence-api`.** Backend native có đăng ký và đăng nhập bằng username/mật khẩu
(#152). Người đã đăng nhập bằng mã email có thể thêm mật khẩu sau khi xác nhận mã mới gửi đến
địa chỉ đã xác minh. Cả hai phương thức tiếp tục đăng nhập vào cùng tài khoản và dùng chung
principal, bút danh và quyền với nội dung. Đăng ký mới yêu cầu phiên khách và đúng một địa chỉ email chuẩn hóa đã được xác minh trước khi tạo tài khoản. Mã chờ xác minh chưa cấp subject, principal hoặc phiên. Tài khoản chỉ dùng mã email vẫn hợp lệ.
[ADR D-05 §12](../docs/architecture/auth-authority.md#12-đổi-hướng-10112026-username--mật-khẩu-là-chính)

Web `/toi` dùng mật khẩu làm lựa chọn đầu, vẫn có mã email và hỗ trợ thêm mật khẩu trên cùng
tài khoản ([Auth Web](../docs/development/auth-web.md)). CMP và runtime khôi phục còn thiếu. [Đặc tả recovery #181](../docs/architecture/password-recovery.md) và [LayoutSpec](../docs/design/password-recovery.md) chuẩn bị luồng riêng, cùng account/principal/dữ liệu; #607 đã thay thế đề xuất email tùy chọn. Cách xử lý phiên sau reset vẫn chờ duyệt. `O-11@r1` vẫn `OPEN`, proposal r2 chưa có signoff. Production mặc định `VINK_AUTH_SIGN_IN=disabled` và D-05 vẫn `PROVISIONAL`.

### 14.7. Xóa tài khoản

Chủ sản phẩm xác nhận ngày 26/11/2026. [Quyết định nguồn #297](https://github.com/loveoverflowcom/vink/issues/297) · [Vòng đời](../docs/architecture/account-deletion.md)

| ID | Yêu cầu |
| --- | --- |
| DEL-01 | Xóa vĩnh viễn sau khi xác minh và xác nhận, thu hồi quyền và phiên ngay. |
| DEL-02 | Xác minh lại với mật khẩu hiện tại hoặc mã từ email đã xác minh của tài khoản. Mọi tài khoản chưa retired đều đã có email xác minh, kể cả khi bị đình chỉ. |
| DEL-03 | Username và email có thể được dùng cho đăng ký mới, với account/subject/PrincipalId mới. Handle và alias đã dùng giữ tombstone vĩnh viễn. |
| DEL-04 | Xóa tác phẩm, bản nháp, lịch sử chỉnh sửa và nội dung/ảnh thuộc sở hữu. Bài công khai về trang thay thế theo Publication. Bài bị kiểm duyệt gỡ giữ disposition chặt hơn. |
| DEL-05 | Thu hồi Thấm và Ưng do tài khoản thực hiện, đối soát số đếm và danh sách. |
| DEL-06 | Hủy nhận xét chờ duyệt và chặn quyết định đến trễ. Giữ chữ của nhận xét công khai hợp lệ dưới nhãn “Tài khoản đã xóa”, bỏ định danh công khai. |
| DEL-07 | Tài khoản bị đình chỉ vẫn được xác minh xóa qua luồng riêng. Giữ chứng cứ kiểm duyệt cần thiết. |
| DEL-08 | Dọn dữ liệu chính ngay nếu khả thi, tối đa 30 ngày. Giữ tối thiểu tombstone, binding bất biến, biên nhận chống replay và audit cần thiết. |

## 15. Hồ sơ tác giả và Theo dõi (Social Graph)

Từ bút danh cạnh bài viết hoặc bình luận, người đọc mở đúng tác giả ở `/@handle`, kể cả khi
có người trùng tên. Khách xem phần công khai. Thành viên có thể theo dõi rồi đọc bài của tác
giả trong **Khám phá → Đang theo dõi**. Chủ sửa hồ sơ và quyền từng trường ở **Tôi**.
Phạm vi này cụ thể hóa COM-06, UX-04 và ACC-01..03.

Bảng dùng các mục §2 của [Epic #284, R10](https://github.com/loveoverflowcom/vink/issues/284)
để truy từng luật sản phẩm R1–R9. Epic tổng hợp R1–R8 theo chủ đề và nêu đích danh ba chốt
ở R9 (giữ Identity, Handle ASCII và một lần setup + hai lần đổi). Các ID dưới đây chỉ yêu cầu
sản phẩm. [Hợp đồng liên miền](../docs/architecture/social-graph.md) và
[bảng nghiệm thu](../docs/development/social-graph.md) nối từng ID với owner, bất biến và phép kiểm.

| ID | Nguồn | Quy tắc sản phẩm đã chốt trong brief (`PRODUCT_CONFIRMED`) |
| --- | --- | --- |
| SOC-ID-01 | §2.1, ACC-01 | Mỗi tài khoản có một PrincipalId ổn định. Ownership và cạnh Theo dõi gắn với PrincipalId. |
| SOC-ID-02 | §2.1, R9, ACC-03 | Giữ Identity hiện hành. Username đăng nhập, bút danh và Handle độc lập với nhau. |
| SOC-ID-03 | §2.1 | Bút danh tiếng Việt sửa được, có thể trùng. Trên bài và bình luận còn công khai (kể cả ngữ cảnh trả lời), người đọc thấy bút danh hiện hành. Nội dung revision là bất biến. |
| SOC-ID-04 | §2.1 | Hoàn tất provisioning với binding và bút danh hợp lệ thì có đúng một hồ sơ tối thiểu. Đọc công khai chỉ tra cứu. |
| SOC-ID-05 | §2.1 | Địa chỉ chuẩn là `/@handle`. Người chưa có bài vẫn có hồ sơ, tìm được và nhận Follow. |
| SOC-ID-06 | §2.1 | Đổi email, credential, bút danh hoặc Handle vẫn giữ đúng người, tác phẩm và quan hệ cũ. Đổi bút danh giữ nguyên Handle. |
| SOC-HDL-01 | §2.2, R9 | Handle dùng chữ ASCII thường, số và dấu nối. Sinh từ bút danh chuyển ASCII, từ ngẫu nhiên chọn lọc và hậu tố ngẫu nhiên. Nguồn sinh loại trừ email, hash email và username. Setup một lần miễn lượt, sau đó tối đa hai lần đổi suốt đời. Retry, no-op, collision và lỗi không mất thêm lượt. Mọi tên từng dùng chỉ thuộc người ấy. Alias trả 301 thẳng đến tên hiện tại. Sau khi xóa tài khoản, tên thành tombstone. **24/11/2026 (#480):** trên Web, chính chủ thiết lập và đổi tại Tôi → Chỉnh hồ sơ, xác nhận tại hàng Địa chỉ tác giả. URL riêng cũ chuyển hướng về hàng này. |
| SOC-PRIV-01 | §2.3 | Tám trường có quyền `everyone`/`self` độc lập. Bút danh bắt buộc công khai. Avatar, bio, Followers list và Followers count mặc định công khai. Ngày sinh, Following list và Following count mặc định riêng. Bio và ngày sinh là tùy chọn. Muốn công khai đủ ngày/tháng/năm cần chính chủ xác nhận. |
| SOC-FOL-01 | §2.4, COM-06 | Follow/Unfollow tự do giữa hai tài khoản hợp lệ, kể cả tác giả chưa có bài. Cấm tự Follow. Mỗi cặp có hướng có tối đa một cạnh hiện hành. Mutual chỉ cấp trực tiếp cho hai bên. Follow giữ nguyên quyền đọc tác phẩm. |
| SOC-FEED-01 | §2.5 | Đang theo dõi là một chế độ Khám phá. Lấy cả bài có trước lần Follow, mỗi WorkId một lần theo snapshot hiện công khai. Thứ tự theo lần công khai đầu tiên, mới trước và có khóa phụ ổn định. Sửa, rút rồi tái đăng không đẩy bài lên đầu. Trên Web, đích Đang theo dõi chỉ hiện với phiên đủ quyền khi feed mở ([#621 · LayoutSpec](../docs/design/redesign-r4/layout-spec.md#4-khám-phá)). |
| SOC-FIND-01 | §2.6 | Tìm được đúng tác giả dù trùng tên hoặc chưa có bài. Gợi ý dùng hồ sơ/tác phẩm/thể loại/chủ đề công khai, với giới thiệu trung tính khi thiếu tín hiệu. Không có công tắc riêng để tắt gợi ý. |
| SOC-LIFE-01 | §2.7, ACC-02 | Hạn chế chỉ đăng/bình luận vẫn cho đọc, Follow/Unfollow, xem Feed, sửa hồ sơ/quyền và giữ cạnh. Full suspension/disable tiếp tục áp theo authority hiện hành. |
| SOC-LIFE-02 | §2.7 | Xóa vĩnh viễn chấm dứt cạnh cả hai chiều, cập nhật số đếm, dừng Profile và mọi địa chỉ từng dùng. Gỡ bài công khai về trang thay thế. Bình luận hợp lệ giữ lại mang nhãn “Tài khoản đã xóa”, kể cả trong ngữ cảnh trả lời, bỏ mọi liên kết định danh. |

Ngày 18/11/2026 chủ sản phẩm chốt mốc setup trong [#290 r2](https://github.com/loveoverflowcom/vink/issues/290).
Phần ghi vào [registry](../decisions/README.md#social-graph) còn `PENDING`. Riêng SOC-PRIV-01,
`USER-01@r1 DEFERRED` đang hoãn cả ngày sinh. Chỉ mở thu thập ngày sinh khi revision ngoại lệ
có hiệu lực. Age gate, nhóm tuổi và parental consent tiếp tục hoãn. Các phương án sản phẩm còn
mở, phụ thuộc Account và phạm vi từng lát nằm trong [hợp đồng](../docs/architecture/social-graph.md).

Ngoài roadmap Social hiện tại (`NOT_YET_PLANNED`) có Block, Remove Follower, Mute, duyệt người
theo dõi/tài khoản riêng, tắt nhận Follow, Follow kín toàn cục, bạn bè/chat/DM/danh bạ, nội dung
followers-only và xếp hạng bằng số follower. Follow Notifications thuộc Epic Notifications.
Native product app nằm ngoài phạm vi Epic Social.

## 16. Thông báo cộng đồng và danh sách Thấm công khai (Notifications v1)

Người dùng nhận thông báo gọn nhẹ cho tương tác hợp lệ và quyết định liên quan tới nội dung của
mình mà không bị ngập hay rò rỉ dữ liệu đã mất quyền; người đọc đủ quyền có thể xem danh sách đầy
đủ những người đang **Thấm** tác phẩm công khai. Phạm vi này cụ thể hóa `UX-04` và giữ nguyên
`PUB-03/05/07`, `COM-06/11`, `MOD-10..13` và `LVE-FR-01..10`, truy ngược từ
[Epic #305 R1/R2/R3](https://github.com/loveoverflowcom/vink/issues/305) (18/11/2026) và
[#345](https://github.com/loveoverflowcom/vink/issues/345). Đặc tả miền và UX hành vi bằng chữ nằm ở
[notifications.md](../docs/architecture/notifications.md), hợp đồng dữ liệu/sự kiện nằm ở
[HTTP/internal contract v1](../services/vink-app/contracts/http/v1/notifications.md), bảng truy vết
và bàn giao nằm ở [registry phát triển](../docs/development/notifications.md), và các proposal TOML
chờ signoff nằm ở [kho quyết định](../decisions/README.md#thông-báo-notifications-v1).

| ID | Nguồn R3 / liên kết | Quy tắc sản phẩm đã chốt trong brief (`PRODUCT_CONFIRMED`) |
| --- | --- | --- |
| NOTIF-01 | N1, N7, P-05 | Sáu nhóm loại ban đầu (bình luận gốc được duyệt, phản hồi được duyệt, quyết định kiểm duyệt tác phẩm, bình luận bị từ chối/gỡ do kiểm duyệt, Thấm tác phẩm, thông báo quản trị `TYPE_ONLY_NOW`) cùng phần mở rộng ngày 20/11/2026 cho **Ưng nhận xét** (`#365`, mở lại `P-08b`; producer, rollup và Gateway runtime đã có, UI thuộc `#364`). Thông báo Theo dõi (`Follow`) là `DEFERRED` chờ `#284` và lượt giao sau. Mặc định bật mọi loại đã triển khai (không bật loại reserved/deferred); v1 chưa có UI preference và không tự thêm loại mới. |
| NOTIF-02 | N2, N3, P-01B, COM-11 | Bình luận gốc gửi tác giả tác phẩm. Phản hồi giữ một tầng phẳng dưới root; khi A trả lời bình luận B dưới root của C, gửi cho người được trả lời trực tiếp (B, hoặc C nếu trả lời trực tiếp root) và chủ root (C), loại người gửi A và khử trùng theo `PrincipalId`. Tương tác chỉ phát khi nội dung đã duyệt và đủ điều kiện hiển thị; giữ `COM-11` (bình luận gửi trước lúc bài ẩn và được duyệt khi bài ẩn không phát bù thông báo tương tác sau khi tái công khai). Thông báo bất lợi do kiểm duyệt gửi riêng đúng người viết bất kể công khai hay không; tự gỡ bình luận của chính mình không tự gửi thông báo. |
| NOTIF-03 | N3, P-11a/b, P-12A, MOD-10..13, PUB-03/05/07 | Chỉ gắn nhãn/trạng thái “Cần xử lý” khi tác giả thực sự phải hành động (`NeedsAdmin` hoặc hết retry không phải yêu cầu tác giả sửa). Tác phẩm `approved → rejected → approved` phát thông báo duyệt mới đúng decision và revision (`approved → approved`, reaffirmation, no-op hoặc retry không tạo sự kiện mới). Người viết mở màn chi tiết quyết định riêng của chính mình theo đúng decision/revision (tác phẩm) hoặc comment/version (bình luận) cùng lý do được phép hiển thị; không phục hồi thân bình luận đã gỡ và không lộ ghi chú nội bộ hay danh tính người duyệt. |
| NOTIF-04 | N5, P-07A | Gom nhóm ngay khi ghi (`Write-time Rollup`), tối đa một nhóm chưa đọc còn hiệu lực cho cùng `group_key` (`recipient + loại + đích nghiệp vụ`: root theo tác phẩm, reply theo đích trả lời trực tiếp trong root, Thấm theo `WorkId + RevisionId`, quyết định gắn đúng revision/decision và nghĩa quyết định). Sự kiện mới cập nhật nhóm chưa đọc; sau khi `Read` tạo nhóm mới, không biến nhóm đã đọc thành chưa đọc. `actor_count` đếm số người khác nhau; `actors_preview` tối đa 3 người gần nhất chỉ là phần tóm tắt trong inbox. |
| NOTIF-05 | P-02B, P-03A | Thấm (và Ưng nhận xét theo `#365`) chỉ tạo tín hiệu thông báo lần đầu suốt vòng đời `(actor, loại, đích)`; hủy rồi chọn lại đối soát membership âm thầm, không tạo tín hiệu mới và không đánh thức badge/dòng đã đọc. Khi bỏ chọn hoặc mất hiệu lực trên nhóm chưa đọc, bỏ contribution, giảm `actor_count` và bù `actors_preview` từ các thành viên còn hợp lệ; khi còn 0 thì ẩn/hết hiệu lực nhóm (không giả `Read`). Nhóm đã đọc giữ lịch sử hợp lệ trong thời hạn lưu giữ nhưng luôn tuân quyền hiện hành. |
| NOTIF-06 | N6, P-04B, P-09A, P-10A | `Seen` cấp người dùng độc lập với `Read` cấp từng nhóm. Badge đếm số nhóm có cập nhật chưa `Seen`, chấp nhận trễ hội tụ hữu hạn để tiết kiệm tài nguyên và phải phân biệt trạng thái cũ (`stale`) hoặc không xác minh được (`unavailable`) với giá trị `0`. Chuông chỉ hiện số dương đã xác minh (tối đa `99+`), giữ số cũ còn hợp lệ trong lúc đồng bộ. Panel báo lỗi và cho thử lại khi chưa tải được. Mở chuông chỉ xác nhận `Seen` tới snapshot đã nhận, không tự `Read` các dòng. Bấm một dòng ghi `Read` cho đúng phiên bản đã thấy dù mở đích lỗi. “Đọc tất cả” chỉ áp dụng tới snapshot người dùng vừa xác nhận, không nuốt cập nhật đến sau. |
| NOTIF-07 | N4, K4, K5, PUB-03/05/07 | Thin Pointer: event và nhóm thông báo chỉ giữ định danh và metadata tối thiểu; App Gateway xác định chủ inbox từ phiên thật và kiểm quyền hiện hành + hydrate từ owner nghiệp vụ theo lô khi liệt kê, khi đọc preview/actor và khi mở đích. Rút/ẩn/gỡ bài, hold bình luận hoặc mất quyền phải chặn rò rỉ chữ, tên và avatar ở danh sách, preview, cache và đích mở; đổi tài khoản hoặc đăng xuất hủy dữ liệu và phản hồi muộn của phiên trước. Dòng bình luận gốc/phản hồi được duyệt có trích đoạn ngắn của bình luận hợp lệ mới nhất (quyết định 20/11/2026, `#365`): Gateway hydrate lúc đọc theo quyền hiện hành, không trích bình luận đã gỡ, đang hold, trên bài ẩn/rút hoặc khi người đọc mất quyền; Notification Service vẫn không lưu thân bình luận. |
| NOTIF-08 | P-08a, LVE-FR-02/05 | Danh sách đầy đủ người đang Thấm công khai theo `WorkId + RevisionId` có phân trang/tải thêm với thứ tự rõ và kích thước trang hữu hạn cho mọi người đủ quyền đọc tác phẩm/revision công khai hiện hành (bao gồm khách chưa đăng nhập). Dữ liệu lấy từ membership hiện hành của Endorsement và phép chiếu danh tính công khai qua Gateway, độc lập với `actors_preview`, `suppression`, `receipt` và tuổi xóa 180 ngày của Notifications; không tải toàn danh sách khi polling. |
| NOTIF-09 | N8, P-06A, P-13B | Nhóm inbox, preview và danh sách người tương tác thuộc Notifications có tuổi tuyệt đối tối đa 180 ngày từ lúc tạo nhóm (tương tác mới không gia hạn) và bị xóa sau tối đa 180 ngày. Biên nhận kỹ thuật/dấu chống báo lại có thời hạn độc lập khi thật sự cần bảo vệ `P-02B` hoặc khôi phục, nhưng không dùng để tái dựng inbox đã hết hạn và không xóa lượt Thấm đang còn hiệu lực ở Endorsement. Không backfill lịch sử trước mốc kích hoạt. |
| NOTIF-10 | P-14A, UX-01, UX-04 | Triển khai Web trước nhưng hợp đồng API, định danh đích nghiệp vụ, phân trang, lỗi, phiên bản và retry phải độc lập Leptos/DOM/HTML-only để CMP/mobile tái sử dụng. Tách biệt đường đọc Thấm công khai cho khách với đường inbox/ghi cần phiên thật. Giai đoạn Design/Web bảo đảm polling 30–60 giây cho count nhẹ, giảm/dừng khi ẩn/offline, backoff/resume, thao tác phím/chạm, trả focus và cỡ chữ 100%–200%. |

**Trạng thái bốn chiều và phạm vi hoãn của §16:**
- *Quyết định sản phẩm:* `N1–N8` và `P-01..P-14` (`NOTIF-01..10`) là `PRODUCT_CONFIRMED` trong brief Epic `#305` R1–R3 (18/11/2026); proposal [VDR-NOTIFICATIONS@r1](../decisions/proposals/VDR-NOTIFICATIONS.r1.toml) đang `PENDING` chờ signoff theo `VDR-DECISION-SIGNOFF@r1`.
- *Delta ngày 20/11/2026:* Chủ sản phẩm chốt “Có, báo Ưng” và trích đoạn từ Gateway theo [#365](https://github.com/loveoverflowcom/vink/issues/365); [VDR-NOTIFICATIONS@r2](../decisions/proposals/VDR-NOTIFICATIONS.r2.toml) giữ `PENDING`.
- *Quyết định kiến trúc:* hướng `K1–K6` (`ARCHITECTURE_DIRECTION_CONFIRMED` trong brief), delta Gateway `#348`/`#365` và mở rộng `P-08` cho Thấm nằm ở [VDR-NOTIFICATIONS-CONTRACT@r1](../decisions/proposals/VDR-NOTIFICATIONS-CONTRACT.r1.toml), [VDR-NOTIFICATIONS-CONTRACT@r2](../decisions/proposals/VDR-NOTIFICATIONS-CONTRACT.r2.toml) và [LVE-PR-02-03-CHOICE-12@r2](../decisions/proposals/LVE-PR-02-03-CHOICE-12.r2.toml) (đều `PENDING`; `LVE-PR-02-03-CHOICE-12@r1` `DESIGN_PROPOSED` vẫn đang hiện hành).
- *Mức triển khai* — App producer/relay (`#346`), Notification Service (`#347`), App Gateway inbox/quyết định riêng/Thấm công khai + trích đoạn nhận xét (`#348`/`#391`) ở mức `persistence-api`. Producer, rollup và DTO runtime cho **Ưng nhận xét** (`#365`) còn chờ; UI thuộc `#364`.
- *Bằng chứng* — [Registry Notifications](../docs/development/notifications.md) ghi đầy đủ bằng chứng PostgreSQL và HTTP theo từng ranh giới.
- *Hoãn (`DEFERRED`):* Follow Notifications (`#284`), gửi/soạn/sửa/rút/ủy quyền/broadcast thông báo quản trị (`P-05`), push OS, email thông báo, WebSocket/SSE, preference UI nâng cao, và mọi UI native/CMP (`P-14A`) đều `DEFERRED`.

## 17. Lượt xem tác phẩm (Work views)

Tác giả biết bài của mình được mở đọc ở mức nào qua một con số tham khảo **Lượt xem** tại Bàn
viết. Phạm vi này trả lời [Epic #323](https://github.com/loveoverflowcom/vink/issues/323)
(P-01..P-11) qua [#340](https://github.com/loveoverflowcom/vink/issues/340). Ngày 21/11/2026,
trong phiên làm #340, chủ sản phẩm trả lời “Theo khuyến nghị” cho bộ khuyến nghị r1
(`P-01..P-11`, [VDR-WORK-VIEWS@r1](../decisions/proposals/VDR-WORK-VIEWS.r1.toml), đã
[signoff](../decisions/signoffs/VDR-WORK-VIEWS.r1.toml) cùng ngày). Ngày 25/11/2026, trong phiên
làm [#323](https://github.com/loveoverflowcom/vink/issues/323) và `#340`, chủ sản phẩm chốt
delta hiệu lực mục tiêu: yêu cầu hiển thị thân bài tối thiểu theo ngưỡng cấu hình chung mặc
định 5 giây (`5.000 ms`), chốt `OQ-1..OQ-4`, bộ luật `UX-01..UX-10` (ID cục bộ Epic #323), giữ
riêng tư chỉ tác giả thấy, triển khai 2 PR (`#340+#343` rồi `#514`), và hai quyết định cho hàng
Bàn viết (`D-01`: thứ tự cố định **Thấm → Nhận xét → Lượt xem**, `D-02`: bỏ hẳn nút/link
“Đọc” / “Đọc bản công khai”). Delta nằm ở [VDR-WORK-VIEWS@r2](../decisions/proposals/VDR-WORK-VIEWS.r2.toml)
(`PENDING` chờ signoff đúng payload/SHA-256 theo `VDR-DECISION-SIGNOFF@r1`); `@r1` giữ nguyên
lịch sử đã signoff. Định nghĩa, bảng ví dụ và vòng đời theo trạng thái nằm ở
[đặc tả nghiệp vụ](../docs/architecture/work-views.md). Mức triển khai và bản đồ sang
`#341`–`#343` và `#514` nằm ở [registry phát triển](../docs/development/work-views.md).

Các ID dưới đây giữ nguyên WR-01, UX-01, AUD-01/02, PUB-04/05/07, MOD-05, INV-08 và `O-17@r2`
(trang rút trả HTTP 200; mã HTTP không phải bằng chứng đã xem).

| ID | Nguồn #323 | Quy tắc sản phẩm đã chốt (`r1` 21/11/2026 + delta 25/11/2026) |
| --- | --- | --- |
| VIEW-01 | P-01 B + delta 25/11 | Một lượt được cộng khi thân bài công khai đã hiện sau thao tác mở hợp lệ **và** hiển thị ở tab foreground/visible tối thiểu **ngưỡng cấu hình dùng chung mặc định 5 giây (`5.000 ms`)** (đồng hồ monotonic, cộng dồn trong cùng lần mở, tạm dừng khi tab ẩn, tiếp tục khi hiện; hủy/reset khi rời trang/unmount, đổi tác phẩm/revision thực sự, bài mất công khai hoặc đổi authority phiên đọc; đúng ngưỡng phát một ứng viên, dưới ngưỡng không gửi; không tính SSR HTML, hydration, tải trang, prefetch/prerender, re-render hay refresh nền; không yêu cầu đọc hết, cuộn hay bấm thêm). Bấm rồi hủy, tải lỗi và trang thay thế (trang ẩn PUB-07, trang rút `O-17@r2`, kể cả HTTP 200) đều không cộng. Tab mở nền chờ foreground và đủ ngưỡng (`OQ-3B`); trang ẩn/rút tự công khai lại do cập nhật nền không tự đếm mà cần mở mới (`OQ-3A`). Tên hiển thị là “Lượt xem”; không phải số người duy nhất hay số người đọc hết. Ngưỡng phải tách vào một nguồn cấu hình/constant dùng chung cho timer, giải thích UI và test. |
| VIEW-02 | P-02 B | Link nội bộ, URL trực tiếp, link chia sẻ và tab mới đều tính khi thỏa VIEW-01; mở bằng chuột, chạm hay Enter đều như nhau. Hover, prefetch, prerender và bot tạo bản xem trước link không tính. Trình duyệt tắt JavaScript không được đếm, và giới hạn này phải được ghi rõ cho tác giả. |
| VIEW-03 | P-03 C | Mỗi người đọc được tính tối đa một lượt cho mỗi tác phẩm trong cửa sổ 24 giờ trượt (neo tại lượt được tính; refresh là lần mở mới và vẫn bị dedup 24h; không cộng dồn thời gian giữa nhiều lần mở hay nhiều tab). Refresh, Back/Forward hay mở lại trong cửa sổ không cộng. Gửi trùng kỹ thuật (bấm đúp, gửi lại cùng sự kiện) không bao giờ thành lượt mới. |
| VIEW-04 | P-04 A | Khách chưa đăng nhập được tính. Chống trùng cho khách dùng mã ngẫu nhiên first-party có thời hạn cố định bằng cửa sổ VIEW-03 (không gia hạn khi dùng). Không dùng IP thô, không fingerprint. Chấp nhận đếm lại khi khách xóa cookie, đổi trình duyệt/máy hoặc đăng nhập giữa cửa sổ. Thành viên dùng `PrincipalId` từ phiên. |
| VIEW-05 | P-05 B | Không tính khi phiên đang đăng nhập là tác giả của bài. Màn kiểm duyệt, preview và nháp không bao giờ tính. Admin/moderator mở Reader công khai như độc giả thì được tính (trừ khi là tác giả bài). Tác giả đã đăng xuất được tính như khách; hệ thống không hứa nhận diện được tác giả trong trường hợp đó. |
| VIEW-06 | P-06 A | Số lượt cộng dồn theo `WorkId` qua sửa, đăng lại và revision mới. Khác với Thấm, vốn gắn với `RevisionId` đang công khai. |
| VIEW-07 | P-07 A + D-01/D-02 25/11 | Chỉ tác giả thấy số, riêng tư tại Bàn viết. Không có số công khai ở Reader, Khám phá hay hồ sơ, và không có danh sách người đã xem. **Delta 25/11/2026 (`D-01`/`D-02`):** hàng tác phẩm Bàn viết cố định thứ tự các metric được phép hiển thị là **Thấm → Nhận xét → Lượt xem** (Lượt xem cuối) và **bỏ hẳn nút/link “Đọc” / “Đọc bản công khai”** (không thay bằng icon, menu, CTA khác, click số liệu hay vùng giữ chỗ; giữ liên kết mở bản thảo/editor của hàng/nhan đề và giữ nguyên Reader công khai). |
| VIEW-08 | P-08 A + OQ-1 A + OQ-4 | Khi bài bị ẩn hoặc chuyển “Chỉ mình tôi” (trang ẩn, PUB-07), bị rút sau kiểm duyệt (`O-17@r2`), hay bị admin gỡ cả bài (`PUB-05 C`, **OQ-4** 25/11), tác giả vẫn thấy số Lượt xem cũ nếu còn quyền xem bài mình và số không tăng thêm (dù Thấm/Nhận xét giữ luật ẩn khi riêng tư theo `VDR-AUTHOR-WORK-FEEDBACK`, **OQ-1 = A**). Công khai lại thì đếm tiếp từ số cũ. Mở trang thay thế không tính. Nếu tác phẩm bị xóa thì xóa dữ liệu theo `VIEW-11`/`OQ-2`. |
| VIEW-09 | P-09 A | Đếm bắt đầu từ ngày bật tính năng. Bài đã công khai trước mốc (`counted_since != null`) mang nhãn “Tính từ dd/mm/yyyy” theo `Asia/Ho_Chi_Minh`. Không backfill vì chưa từng có nguồn dữ liệu (#214 loại tracking khỏi phạm vi). Không suy lượt xem từ Thấm hay Nhận xét. |
| VIEW-10 | P-10 A | Lượt xem là số tham khảo. Kỳ vọng tác giả thấy lượt mới trong khoảng 1 phút; đây là kỳ vọng, không phải SLA. Khi đạt ngưỡng, client gửi một mã sự kiện UUIDv7 và retry hữu hạn trong cùng tab với cùng mã. Đóng tab hoặc offline có thể làm mất lượt, nhưng không bao giờ cộng trùng. Lỗi thống kê không bao giờ cản người đọc mở, đọc, cuộn, bình luận hay quay lại. |
| VIEW-11 | P-11 B + OQ-2 | Chỉ lưu tổng theo tác phẩm, tổng theo ngày theo tác phẩm (không theo người, chưa có biểu đồ) và mã chống trùng/biên nhận đã băm, tự xóa khi hết cửa sổ VIEW-03. **OQ-2 (25/11):** PR đầu tiên đưa route xóa tác phẩm (`PUB-04/05`) hoặc xóa tài khoản (`SOC-LIFE-02`) vào runtime phải xóa dữ liệu lượt xem liên quan; không tự mở phạm vi xóa trong Epic #323 và không quyết định `O-08` purge vật lý thay owner. |
| VIEW-12 | UX-01..10 + D-01/D-02 | Chưa có số hoặc đọc số lỗi hiển thị “—” kèm lý do, khác với “0” thật (theo mẫu của `VDR-AUTHOR-WORK-FEEDBACK`). Số định dạng đầy đủ kiểu Việt (`1.234`), icon mắt Material Symbols (`visibility`), đặt cuối trong **Thấm → Nhận xét → Lượt xem**, giao diện trung tính R4/Tailwind. Tooltip/popover truy cập bằng chạm/bàn phím/screen reader giải thích ngưỡng cấu hình hiện hành (lấy từ cùng nguồn cấu hình), cửa sổ 24 giờ, khách theo trình duyệt, tự xem và trình duyệt tắt JavaScript; nhãn “Tính từ dd/mm/yyyy” theo `Asia/Ho_Chi_Minh`. Cập nhật khi tải lại/làm mới (không realtime, không polling, không biểu đồ/dashboard, không thông báo). |

**Trạng thái bốn chiều và phần còn mở của §17:**
- *Quyết định sản phẩm:* `VIEW-01..12` bản `r1` là `PRODUCT_CONFIRMED` ngày 21/11/2026
  ([VDR-WORK-VIEWS@r1](../decisions/proposals/VDR-WORK-VIEWS.r1.toml) +
  [signoff](../decisions/signoffs/VDR-WORK-VIEWS.r1.toml)). Delta ngày 25/11/2026 (`#323`/`#340`:
  ngưỡng 5s cấu hình tập trung, `OQ-1=A`, `OQ-2`, `OQ-3A/B`, `OQ-4`, `UX-01..10`, `D-01`/`D-02`
  và lộ trình 2 PR) được chủ sản phẩm chốt trên issue; proposal
  [VDR-WORK-VIEWS@r2](../decisions/proposals/VDR-WORK-VIEWS.r2.toml) giữ trạng thái `PENDING` theo
  `VDR-DECISION-SIGNOFF@r1` chờ signoff đúng payload/SHA-256.
- *Quyết định kiến trúc:* `#341` đề xuất `CHOICE-02 A`, `CHOICE-03`, `CHOICE-06 A` (`DESIGN_PROPOSED`);
  `#342` đề xuất `CHOICE-05 A` (`DESIGN_PROPOSED`); `#343` (PR1) đề xuất `CHOICE-01`, `CHOICE-04`
  và `CHOICE-POLICY` (chưa `Accepted`; [registry](../docs/development/work-views.md)).
- *Mức triển khai và bằng chứng:* xem [registry phát triển](../docs/development/work-views.md) cho
  đường ghi `#341`, đọc tác giả `#342`, thu nhận HTTP + tracker Reader `#343` (PR1) và giao diện
  Bàn viết `#514` (PR2). Xóa tài khoản tác giả (#550) đã xóa số liệu theo `OQ-2`; route xóa tác
  phẩm và admin gỡ cả bài là `CONDITIONAL_DEFERRED` cho owner Publication.
- *Ngoài phạm vi §17:* số công khai ở Reader/Khám phá/hồ sơ, danh sách người xem,
  dashboard/biểu đồ, xếp hạng, Notifications, CMP/native, route xóa tác phẩm và admin gỡ cả bài
  (`OQ-2` / `OQ-4` / `O-08`).

## 18. Lưu bài để đọc lại (Saved Works)

**Must — Pilot · COM-05.** Ngày 21/11/2026, chủ sản phẩm mô tả mục tiêu: “Lưu lại, giống nút lưu trên youtube này, lưu những bài viết mình muốn xem lại, và mở danh sách này ở trang cá nhân của mình.” Ngày 25/11, chủ sản phẩm duyệt D1–D12, vòng đời bên dưới và phạm vi hoãn. Các mã D trong mục này thuộc riêng Epic #384. [Nguồn lựa chọn 21/11 và lời duyệt tại §8](https://github.com/loveoverflowcom/vink/issues/384).

| ID · trạng thái sản phẩm | Quy tắc | Ví dụ nghiệm thu |
| --- | --- | --- |
| SAV-01 · PRODUCT_CONFIRMED | Một danh sách theo `WorkId` cho mỗi tài khoản. Được lưu bài công khai của chính mình (D8). Tối đa 10.000 quan hệ đang lưu, kể cả bài đang ẩn khỏi danh sách (D6). Mới lưu trước, khóa phụ ổn định (D7), 20 bài/lần bằng cursor và “Xem thêm” (D10). Không TTL (D12). Khác bộ sưu tập tác phẩm của tác giả (COL-01). | Lưu khi revision A đang công khai, mở lại sau khi B được công khai thì đọc B. Từ 9.999, hai lần lưu đồng thời hai bài mới chỉ một lần thêm thành công. |
| SAV-02 · PRODUCT_CONFIRMED | “Chỉ mình tôi”: chỉ chủ thấy tab “Đã lưu” trên hồ sơ của mình và đọc/sửa được danh sách. Khách và người khác mở `/@handle/da-luu` thấy cùng trang như URL không tồn tại. | So hồ sơ từ hai tài khoản: chỉ chủ thấy tab sau khi xác nhận phiên. Đọc HTML công khai hoặc đổi handle/ID trong request không tiết lộ quan hệ. |
| SAV-03 · PRODUCT_CONFIRMED | “Không hiện cho ai”: không có số lượt lưu của bài, kể cả cho tác giả, hoặc tổng số bài đã lưu trên UI. Không gửi thông báo khi lưu/bỏ lưu (D9), không làm tín hiệu Đề xuất, Top 100 hay xếp hạng khác, không gửi cho AI. | Lưu và bỏ lưu cùng bài: tác giả không thấy số hay sự kiện trong inbox; dữ liệu xếp hạng không thay đổi. |
| SAV-04 · PRODUCT_CONFIRMED | “Icon hiện sẵn”: duy nhất một cặp Thấm · Lưu ở cuối Reader, cùng kích thước, số Thấm cạnh chữ một dòng (D1/D5). Dùng token R4/Tailwind theo phạm vi nhóm (D4). Giữ nhãn Lưu, khi đã lưu có bookmark đặc, nền tonal, đổi hình, ✓ và trạng thái truy cập (D3). Chờ ACK tại nút, lỗi/mất ACK giữ giá trị đã biết, đọc lại (D11). | Khách mở sheet đăng nhập, quay lại đúng bài và focus nút, chỉ lần bấm tiếp mới gửi lệnh (D2). Hai tab gửi ý định trái nhau không thể dùng ACK trễ để đảo trạng thái hiện tại. |

### Vòng đời và quyền tại lần đọc/ghi

Quan hệ lưu giữ định danh và thời điểm lưu, không sao chép nhan đề, tên tác giả, thẻ hoặc trích đoạn. Chỉ cho lưu khi có publication công khai đọc được tại commit. Bỏ lưu chỉ cần quyền trên quan hệ của chính mình. Hạn chế đăng/bình luận vẫn được đọc danh sách và lưu/bỏ lưu. Đình chỉ chặn theo standing hiện hành, kể cả khi quyền đổi giữa lúc gửi và commit.

| Trạng thái tại lần đọc | Dòng của chủ và điều kiện kiểm chứng |
| --- | --- |
| Công khai, đọc được | Hiện thể loại, ngày lưu, nhan đề và tác giả từ biểu diễn được phép; mở Reader theo WorkId. |
| Chưa từng công khai | Không thể tạo quan hệ, kể cả với nháp của mình. |
| Từng công khai, tác giả chuyển riêng tư (PUB-07) | Chỉ hiện “Nội dung đã được ẩn” và Bỏ lưu. |
| Tác giả xóa (PUB-04), hoặc tài khoản tác giả bị xóa (SOC-LIFE-02) | Với người lưu khác, hiện “Nội dung đã được tác giả xoá” và Bỏ lưu theo trang thay thế. Hai loại dòng trung tính đều không có nhan đề, tác giả, thẻ, ngày lưu hay liên kết đọc. |
| Admin gỡ hoặc không tìm thấy | Bỏ qua dòng, không để ô trống hay tiết lộ lý do. Giữ quan hệ khi còn khả năng khôi phục. |
| Công khai lại, quan hệ vẫn còn | Dòng hiện lại với publication hiện tại, đúng thời điểm và vị trí lưu ban đầu. Nếu người dùng đã bỏ lưu thì không hiện lại. |
| Xóa tài khoản người lưu | Xóa mọi quan hệ **và fence** của người đó, cả quan hệ đã bỏ lưu. Quan hệ do người khác tạo tuân theo trạng thái của tác phẩm. |

### Bất biến

| ID · trạng thái sản phẩm | Quy tắc và ví dụ nghiệm thu |
| --- | --- |
| SAV-INV-01 · PRODUCT_CONFIRMED | Mỗi cặp (PrincipalId, WorkId) có tối đa một quan hệ đang lưu. Lưu lặp không nhân dòng, không làm mới ngày lưu. |
| SAV-INV-02 · PRODUCT_CONFIRMED | Chỉ chủ đọc được dữ liệu lưu của mình. Hai lịch sử lưu khác nhau cho cùng byte/ETag của bài công khai và hồ sơ công khai, không lộ dấu vết qua HTML, log, cache hoặc DTO chung. |
| SAV-INV-03 · PRODUCT_CONFIRMED | Mỗi lần chiếu một dòng phải kiểm lại quyền đọc. Ẩn bài giữa hai trang rồi tải trang kế tiếp thì không còn metadata cũ của bài đó. |
| SAV-INV-04 · PRODUCT_CONFIRMED | Generation của cặp tồn tại qua bỏ lưu, tăng cả khi nhận ý định mới trùng trạng thái. Retry lệnh lưu cũ sau lưu → bỏ lưu không tạo lại quan hệ. |
| SAV-INV-05 · PRODUCT_CONFIRMED | Lưu/bỏ lưu chỉ tác động danh sách của chủ. So trước/sau: audience, publication, kiểm duyệt, Thấm/Ưng, xếp hạng và thông báo đều độc lập. |

**DEFERRED** gồm thư mục/playlist, tìm kiếm trong danh sách, ghi chú, vị trí đọc dở, offline, chia sẻ/công khai, lưu bình luận và CMP/native. Trạng thái revision TOML, hợp đồng HTTP, LayoutSpec và mức triển khai tra tại [registry Lưu bài](../docs/development/saved-works.md).

## 19. Điểm Vink (Reputation V1)

[EPIC #600](https://github.com/loveoverflowcom/vink/issues/600) · Backend only. Năm lượt lựa chọn
nghiệp vụ 27/11/2026 được ghi tại [proposal VDR-REPUTATION@r1](../decisions/proposals/VDR-REPUTATION.r1.toml).
Lời chọn đã có; revision TOML còn `PENDING` (chưa signoff đúng payload/hash). Không đánh dấu active
hay kích hoạt public runtime. [Đặc tả](../docs/architecture/reputation.md) ·
[registry triển khai](../docs/development/reputation.md) ·
[hợp đồng API dự thảo](../services/vink-app/contracts/http/v1/reputation.md).

| ID · nguồn | Quy tắc nghiệp vụ đích | Ví dụ nghiệm thu |
| --- | --- | --- |
| REP-01 · REP-Q01 | Hai thành phần Impact (Thấm nhận trên tác phẩm) và Contribution (Ưng nhận trên nhận xét); không Follow, lượt xem, lượt lưu, số bài hay số bình luận. Một tổng rating, không phải Elo đối đầu. | Follow hoặc lưu bài không đổi điểm; Thấm và Ưng hợp lệ tác động đúng thành phần. |
| REP-02 · REP-Q02 | Ghi nhận Impact tối đa một đơn vị cho mỗi cặp người Thấm + WorkId qua các anchor revision còn đủ điều kiện. Không chuyển endorsement nguồn sang revision mới. | Một người Thấm revision A, sau đó B của cùng WorkId: không tăng Impact lần hai. Sửa bài bình thường không reset điểm tác giả. |
| REP-03 · REP-Q03/Q13/Q14 | Uy tín hiện hành: private/withdrawn, xóa hoặc admin gỡ nội dung loại điểm liên quan, không tự ghi giảm growth. Restore chỉ từ facts còn hợp lệ. Actor đình chỉ không tự thu hồi endorsement hợp lệ trước đó; DEL-05 và invalidation đã xác minh vẫn thu hồi. Privacy/deletion/correction thắng historical data. | Gỡ bài cũ giảm current rating, không tự trừ growth; khôi phục không tạo growth mới. Đình chỉ voter không cascade điểm tác giả khác; xóa tài khoản thu hồi theo DEL-05. |
| REP-04 · bổ sung lượt 1, REP-Q04/Q09/Q10 | Có thống kê uy tín cao và tăng uy tín nhanh theo tuần/tháng lịch (Asia/Ho_Chi_Minh), không rolling; backend đọc được các kỳ đã kết thúc trong retention REP-10. Top rating dùng điểm hiện tại khi kỳ mở, điểm as-of cuối kỳ khi kỳ đóng, không dùng đỉnh. | Hai sự kiện hai phía ranh giới thuộc hai kỳ khác nhau; không reset tổng điểm. Sau close điểm hiện tại đổi không tự đổi top rating kỳ cũ; coverage/correction/version rõ. |
| REP-05 · REP-Q05/Q07/Q12 | Tăng trưởng ròng từ đóng góp mới hợp lệ trừ thu hồi tương tác trong kỳ, kể cả tương tác nhận từ kỳ trước; khác delta tổng điểm. Bỏ rồi khôi phục cùng entitlement trong cùng kỳ thì bù trừ, không tạo credit mới; khác kỳ giữ khoản giảm kỳ trước, không cộng growth kỳ mới. Week/month tính độc lập. Loại lifecycle remove/restore nội dung, đổi công thức, backfill giả; correction/privacy/deletion có thẩm quyền tách riêng. Attribution phi tuyến/correction chi tiết OPEN. | Bỏ -30 tuần 1, Thấm lại tuần 2 cùng tháng: growth tuần -30/0, phần biến động tháng 0 (minh họa); rating hiện hành được khôi phục. |
| REP-06 · REP-Q06/Q08 | Tổng điểm và thống kê đủ điều kiện công khai kể cả khách; thành phần chưa công khai. Full suspension của người nhận điểm tạm ẩn điểm và loại khỏi mọi thống kê công khai, cả kỳ cũ. Giữ dữ liệu còn hợp lệ; khôi phục khi standing/eligibility cho phép, không reset hoặc tạo growth. | Guest có quyền xem dữ liệu đủ điều kiện; đình chỉ/ẩn/xóa trong khi worker lag không làm lộ projection cũ; hết đình chỉ có thể khôi phục số hợp lệ. |
| REP-07 · scope #600 | Độc lập Đề xuất, Top 100 tác phẩm, Thấm/Ưng nguồn, Follow, Lưu, AI, moderation và auth/ACL. Không ưu tiên tác phẩm hay mở quyền theo uy tín. | Thay đổi score/rollout không đổi kết quả RANK-04, TOP, SAV-03 và quyền. |
| REP-08 · scope #600 | Backend-only, projection/read API versioned; không tính lại toàn hệ thống mỗi read hoặc đưa phép tính nặng vào mutation nguồn. Mục tiêu hiệu suất phải đo trước khi public. | Score chưa sẵn sàng khác điểm 0/baseline; đọc có giới hạn, no N+1; benchmark baseline/candidate và kiểm rebuild/rollback. |
| REP-09 · REP-Q11 | Thành viên chỉ đóng góp nhận xét có score và được tham gia thống kê khi đủ eligibility; không bắt buộc có tác phẩm công khai. Ngưỡng eligibility khác còn OPEN. | Hai member đủ điều kiện có điểm tương đương, một người không có bài nhưng có Ưng: không loại vì không có WorkId công khai. |
| REP-10 · REP-Q15 | Retain kỳ đang diễn ra và 52 tuần/12 tháng lịch đã kết thúc gần nhất theo Q04; không phục vụ ngoài phạm vi, cleanup bounded. Không hứa đủ coverage trước activation. Retention thống kê không quyết định TTL của current facts/ledger/fence/audit; privacy/deletion bắt buộc. | Khi sang tuần mới, tuần cũ nhất ngoài 52 kỳ hết hạn; tháng xử lý độc lập. Replay không hồi sinh kỳ hết hạn, purge không phá current rating hoặc dedup. |

**OPEN:** công thức/trọng số/thang/rounding; eligibility mới, anti-abuse, lifecycle chi tiết
(comment edit/tombstone, account-deletion attribution, verified correction kỳ đóng,
attribution re-like); ngưỡng vào nhóm thống kê ngoài Q11, tie-breaker, retention nguồn/ledger/fence; backfill, freshness và rollout.
Các lựa chọn kỹ thuật có evidence thuộc từng slice. Không mặc định có leaderboard UI,
UI lịch sử hoặc API trả raw component. Backend historical statistics đã chọn theo Q09. Không decay trong phạm vi đề xuất V1.
