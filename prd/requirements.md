# Requirements nền tảng cho Vink

**Phiên bản:** 0.3 · **Ngày:** 30/10/2026, cập nhật 10/11/2026 · **Trạng thái:** Đề xuất phạm vi và tiêu chí nghiệm thu, chưa phải checklist đã PASS.

**Cập nhật 10/11/2026:** chủ sản phẩm chốt 15 quyết định nghiệp vụ
([DecisionDelta](../docs/architecture/product-decisions-20261110.md)). Yêu cầu mới có ID ở
[§14](#14-quyết-định-sản-phẩm-10112026); các dòng cũ bị thay đổi được ghi chú tại chỗ, không đánh
số lại và không xóa lịch sử. Mọi yêu cầu §14 hiện **chưa có implementation**.

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
| WR-01 | Must — Pilot | Nháp riêng có autosave và trạng thái lưu rõ | Refresh giữ nguyên dữ liệu đã được server xác nhận; mất mạng hiển thị phần chưa lưu, cho retry/export; không tuyên bố đã lưu khi chưa thành công; tài khoản khác/khách không đọc được nháp qua UI, API, SSR hoặc URL đoán |
| WR-02 | Must — Pilot | Soạn và render đúng tiếng Việt, thơ, đoạn văn | Bộ mẫu dấu tiếng Việt, IME, khổ thơ, xuống dòng và paste từ Word/Docs đi qua save → render → edit không mất cấu trúc; kiểm trên desktop và mobile mục tiêu |
| WR-03 | Must — Pilot | Snapshot gửi duyệt bất biến, working draft có version | Sửa tiếp tạo draft/snapshot mới; hai tab cùng lưu không âm thầm ghi đè; snapshot đã gửi có ID/hash ổn định, có thể đối chiếu lại |
| WR-04 | Must khi mở nhiều chương — **`DEFERRED` 10/11/2026: không triển khai nhiều chương** | Danh sách chương và trạng thái riêng từng chương | Thêm/sắp xếp chương nháp không làm lộ nội dung chưa đăng; URL/ID chương cũ giữ ổn định; trật tự công khai chỉ đổi qua hành động được phép |
| WR-05 | Must — Pilot | Xuất nội dung cơ bản theo định dạng có thể dùng lại | Tác giả có thao tác xuất văn bản của mình, giữ Unicode và ngắt dòng; không phụ thuộc ID của hệ thống ngoài làm khóa chính |
| WR-06 | Should | Xuất đầy đủ lịch sử và metadata | Có thể xuất revisions, metadata và schema version trong phạm vi quyền/retention; phân biệt dữ liệu tác giả với audit nội bộ |

Không cam kết cộng tác thời gian thực hoặc offline-first trong pilot. Local recovery, nếu có, phải phân biệt với bản đã lưu server, tách theo tài khoản và có cách xóa trên thiết bị dùng chung.

## 4. Xuất bản và rút bài

| ID | Ưu tiên | Yêu cầu | Nghiệm thu tiêu biểu |
| --- | --- | --- | --- |
| PUB-01 | Must — Pilot | Công khai đúng snapshot, có ý định xuất bản của tác giả | Nháp chỉ lưu hoặc preview không xuất hiện công khai dù AI trả Approved; nút “Gửi để đăng khi được duyệt” lưu rõ ý định theo snapshot; publish kiểm lại quyền, trạng thái hiện tại và quyết định duyệt. **10/11/2026:** ý định này là lựa chọn audience “Công khai” gắn đúng snapshot (AUD-01/02) |
| PUB-02 | Must — Pilot | Bản đang công khai độc lập bản đang sửa | rev1 vẫn đọc được khi rev2 Draft/PendingAI/NeedsRevision/Rejected; chỉ transaction publish hợp lệ thay pointer; callback cũ không thay bản mới hoặc đảo quyết định của người duyệt |
| PUB-03 | Must — Pilot | Có cách rút bài và bảo vệ quyền ở mọi bề mặt | Ngay khi rút bài có hiệu lực ở Vink, public API/SSR/search/Tạp Chí không trả phần phải ẩn; đọc/ghi thảo luận của tác phẩm bị chặn cùng lúc; bản sao ngoài Vink có trạng thái riêng. Chỉ báo hoàn tất toàn bộ sau khi xác minh snippet/preview/cache/thông báo trong phạm vi policy đã được thu hồi; lỗi giữ pending và cảnh báo, không khôi phục publication. **10/11/2026:** câu “đọc/ghi thảo luận bị chặn cùng lúc” nay chỉ đúng cho chuyển riêng tư và gỡ do kiểm duyệt; tác giả **xóa** bài từng công khai để lại trang thay thế và các bình luận hợp lệ (PUB-04/05) |

PUB-03 cần định nghĩa rõ phạm vi thu hồi trong hệ thống, thời hạn xử lý và cách xử lý lỗi đồng bộ. Không coi việc xóa card ở trang chủ là hoàn tất. Không hứa thu hồi các bản sao đã được người dùng lưu bên ngoài hệ thống.

## 5. Kiểm duyệt và AI

| ID | Ưu tiên | Yêu cầu | Nghiệm thu tiêu biểu |
| --- | --- | --- | --- |
| MOD-01 | Must — Pilot | Tách chính sách cộng đồng khỏi chất lượng văn chương | Bài an toàn nhưng viết yếu vẫn có thể đăng ở Xưởng; AI không từ chối vì không đạt điểm nghệ thuật, độ “chân thành” hoặc thiếu khả năng vào Tạp Chí |
| MOD-02 | Must — Pilot | Kết quả duyệt chỉ áp đúng đối tượng/phiên bản | Gắn revision/hash/policy/decision version; thử callback trùng, sai hash, trễ sau human override và sau rút bài: không có mutation trái phép |
| MOD-03 | Must — Pilot | Timeout/lỗi AI có human review được vận hành | Worker chết, hết retry hoặc quá deadline tạo/đánh dấu case thủ công; tác giả thấy trạng thái và khung xử lý được thông báo, người phụ trách thấy tuổi case; có watchdog, cảnh báo backlog và đối soát; không tự approve vì provider lỗi. **10/11/2026:** không mặc định có người trực và không tự đặt thời hạn xử lý; phần còn hiệu lực là “lỗi không bao giờ thành duyệt” và “giữ retry được hoặc cần quản trị viên” (MOD-08) |
| MOD-04 | Must — Pilot | Giải thích và khiếu nại | Tác giả xem lý do có thể hành động, gửi appeal; quyết định mới ghi người thực hiện và căn cứ; duplicate appeal không tạo nhiều case; được mở lại khi có bằng chứng mới theo quyền moderator. **10/11/2026:** đích xử lý là quản trị viên; kênh gửi chưa kết nối (MOD-10) |
| MOD-05 | Must — Pilot | Phân quyền AI và dữ liệu đầu vào | Prompt injection trong truyện không được thực thi như chỉ thị; worker không tự mở URL/tệp hay dùng tool theo nội dung; output schema bị kiểm tra; nháp riêng không gửi provider khi chưa có hành động/đồng ý phù hợp |
| MOD-06 | Must trước tự động duyệt | Có bộ đánh giá tiếng Việt và chế độ thu hồi tự động hóa | Mẫu có thơ, phương ngữ, lời thoại, trào phúng và nội dung mơ hồ; đo sai lệch/false positives theo loại; ngưỡng cho phép được review; có thể tắt auto-approval mà vẫn giữ human queue. **10/11/2026:** hướng sản phẩm là AI tự duyệt (MOD-07/08), nhưng **bật auto-approval production vẫn cần mục này**; lượt tài liệu/model không bật gì |

Kết quả `NeedsRevision` liên quan tới chính sách phải khác góp ý writing coach. Góp ý văn phong tự nguyện không trở thành điều kiện bắt buộc để được đăng. Confidence do model tự báo không được coi như xác suất đã hiệu chuẩn.

Lịch sử moderation giữ kết luận, policy/model version, thời điểm và actor cần thiết để audit; retention và quyền xem phải được quyết định. Không lưu tràn lan toàn bộ prompt hoặc nội dung riêng vào logs vận hành.

## 6. Cộng đồng và thảo luận

| ID | Ưu tiên | Yêu cầu | Nghiệm thu tiêu biểu |
| --- | --- | --- | --- |
| COM-01 | Must — Pilot | Thread ổn định cho work/chapter | Xuất bản lại không tạo thread mới; retry/đồng thời không làm sinh nhiều thread hữu hiệu cho cùng tác phẩm (ràng buộc unique trong database Vink) |
| COM-02 | Must — Pilot | Tiền kiểm ở owner của bình luận trên mọi cửa đang bật | Tạo qua Web/native/API đều chưa public khi pending; không có đường email/import; nếu cho sửa bình luận công khai thì phải có gate tương ứng; test cả quyền staff/system đã được phép ngoại lệ. **10/11/2026:** hướng đích là evaluator AI quyết định thay cho người có grant (COM-09) và sửa bình luận được mở có gate (COM-10); code hiện tại vẫn là `premoderation.v1` |
| COM-03 | Must — Pilot | User actions thực thi đúng người và đúng quyền | Browser không thể đổi username/user ID để gửi thay tài khoản khác; account mapping lấy từ session; key không xuất hiện trong HTML, WASM, JS, logs hoặc lỗi gửi cho người dùng |
| COM-04 | Must — Pilot | Suy giảm chức năng rõ khi phần thảo luận lỗi | Bài công khai vẫn đọc được; bình luận hiển thị trạng thái phù hợp, không giả gửi thành công; retry không nhân đôi phản hồi; hoạt động trở lại có thể đối soát |
| COM-05 | Must — Pilot | Góp ý có ngữ cảnh và lưu bài | Tác giả chọn mục đích phản hồi; người đọc thấy tín hiệu đó; lưu/bỏ lưu rõ và idempotent; không dùng điểm sao làm cổng đăng bài |
| COM-06 | Should — Beta | Theo dõi tác giả và cảm nhận nhẹ | Theo dõi/bỏ theo dõi rõ, không tạo bản dữ liệu cạnh tranh ở hai owner; tín hiệu tương tác không tự quyết định quyền đăng hoặc tuyển chọn |
| COM-07 | Must — Pilot | Tác giả bật/tắt bình luận trên từng tác phẩm | Chỉ tác giả tác phẩm đổi được; thao tác idempotent và có version để tab cũ không đè lựa chọn mới (yêu cầu mang đúng version hiện tại luôn được chấp nhận và tăng version, kể cả khi chỉ giữ nguyên chế độ; chủ sản phẩm xác nhận 05/11/2026); tắt chặn bình luận/phản hồi mới tại commit, không xóa hay ẩn bình luận đã có (chủ sản phẩm xác nhận 03/11/2026; tác phẩm mới công khai mặc định bật); độc lập với quyền kiểm duyệt |
| COM-08 | Must — Pilot | Tác giả tác phẩm không sửa/xóa bình luận của người khác | Mọi đường ghi kiểm quyền tại server; tác giả tác phẩm chỉ gỡ bình luận của chính mình; gỡ/duyệt bình luận người khác thuộc authority kiểm duyệt hệ thống |

Ghi nhận văn học trên tác phẩm, bình luận gốc và phản hồi (“Đánh giá cao”/“Hữu ích”) có PRD riêng với ID `LVE-FR/UC/INV`: [ghi nhận văn học](literary-endorsement.md). LVE kế thừa COM-03..08 và không đổi nghĩa của chúng; tín hiệu này không cấp quyền đăng, kiểm duyệt hay tuyển chọn.

Vink tự làm màn quản trị nhỏ cho bình luận (hàng đợi duyệt có version, lý do, báo cáo, khiếu nại) cùng màn cho tác phẩm, quyết định, appeal và curation. Discourse chỉ là tham khảo thiết kế (nguồn D3–D5 trong [Sources](../docs/references/sources.md)); không có Discourse Admin trong hệ thống.

## 7. Khám phá, tuyển chọn và trải nghiệm đọc

| ID | Ưu tiên | Yêu cầu | Nghiệm thu tiêu biểu |
| --- | --- | --- | --- |
| ED-01 | Must — Pilot | Trang chủ hữu ích khi ít nội dung | Với một tuyển tập nhỏ vẫn có lời dẫn, đề bài hoặc CTA tham gia phù hợp; không có thống kê giả, vùng trống dài hoặc feed vô hạn thiếu nội dung |
| ED-02 | Must — Pilot — **`DEFERRED` 10/11/2026: chưa có Tạp Chí/tuyển chọn** | Curation độc lập moderation và ghim phiên bản | Bài không được chọn vẫn đọc được ở Xưởng; pilot chỉ hiện tuyển chọn khi snapshot được chọn còn là publication hiện hành; đổi bản tạm ngừng mục tuyển chọn chờ biên tập; rút bài đóng hiển thị |
| UX-01 | Must — Pilot | Đọc dài trên desktop/mobile, M3 nhất quán | Giữ thơ/đoạn; không vỡ layout ở màn hẹp mục tiêu và khi tăng cỡ chữ; keyboard focus, screen-reader labels, tương phản sáng/tối được kiểm; public reader có nội dung HTML từ server |
| UX-02 | Must — Beta | Tìm nội dung công khai bằng tiếng Việt | Bộ truy vấn có/không dấu, tác giả, tiêu đề và tags trả kết quả chấp nhận được; draft, revision chưa đăng và bài đã rút không lọt vào kết quả. **10/11/2026:** thêm lọc theo hashtag (HTG), Đề xuất (RANK) và Top 100 (TOP) trên cùng tập đủ điều kiện |
| UX-03 | Must — Pilot | Tìm lại tác phẩm và trạng thái của mình | Người dùng tìm được bản nháp, bài đã đăng, trạng thái duyệt và hành động sửa/khiếu nại; không phải đoán URL hoặc tìm trong thông báo cũ |
| UX-04 | Should — Beta | Hồ sơ nâng cao và thông báo cộng đồng | Có hồ sơ tác giả và đường xem reply/mention phù hợp; chưa cần inbox đa hệ thống hoàn chỉnh |

## 8. Xác thực, dữ liệu và vận hành

| ID | Ưu tiên | Yêu cầu | Nghiệm thu tiêu biểu |
| --- | --- | --- | --- |
| ACC-01 | Must — Pilot | Một định danh Vink ổn định và mapping có chứng cứ | Đổi email/bút danh không đổi tác giả của tác phẩm; account provisioning lỗi không làm tạo sai người; các session và database Cantos không được dùng chung |
| ACC-02 | Must — Pilot | Quyền và trạng thái khóa nhất quán | Đăng nhập không tự cấp quyền biên tập/admin; thử logout/ban/revoke qua Vink và mọi API đang bật; các trường hợp chưa đồng bộ có cơ chế đóng quyền phù hợp |
| ACC-03 | Must — Pilot (10/11/2026) | Đăng nhập chính bằng username + mật khẩu | Xem [§14.6](#146-tài-khoản-acc-03). Hiện trạng: chỉ có mã email một lần; **chưa có** username/mật khẩu |
| OPS-01 | Must — Pilot | Giao dịch publication/outbox và job có thể phục hồi | Rollback không để pointer và event lệch; worker chết thu hồi lease được; reprocess không làm đăng lại bài đã rút hoặc phát lại quyết định cũ |
| OPS-02 | Must — Pilot | Backup/restore và export có phạm vi rõ | Có bản backup thử, phục hồi trên môi trường tách biệt và đối chiếu tác phẩm, revisions, publication, mapping và jobs; giữ rollback/recovery không làm mất quyền kiểm soát |
| OPS-03 | Must — Beta | Quan sát độ trễ, chi phí và khả năng vận hành | Có dashboard/log đủ biết tuổi hàng đợi, job lỗi, provider cost và backlog; người phụ trách và ngân sách được xác định; chưa có benchmark thì không gắn nhãn “đạt SLA” |

## 9. Các yêu cầu phi chức năng cần đặt ngân sách

Không đưa một số hiệu suất tùy ý thành lời hứa. Trước public beta, đo trên thiết bị/mạng mục tiêu và ghi ngưỡng được chấp thuận cho các mục sau:

| Hạng mục | Đại lượng cần đo | Điều kiện quyết định |
| --- | --- | --- |
| Reader/SSR | Thời gian có nội dung, lỗi hydration, dung lượng tải | Đọc được phần văn bản cốt lõi khi interactivity chưa sẵn sàng |
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

Nguồn: `PRODUCT-DECISIONS-20261110`, [DecisionDelta](../docs/architecture/product-decisions-20261110.md).
Cột “Trạng thái” dùng nhãn của tài liệu đó: phần **luật** là `PRODUCT_CONFIRMED` trừ khi ghi khác;
cột “Đề xuất UX/dữ liệu” luôn là `DESIGN_PROPOSED`. **11/11/2026** (`ADMIN-MODERATION-20261111`,
[DecisionDelta §7](../docs/architecture/product-decisions-20261110.md#7-bổ-sung-11112026--quản-trị-kiểm-duyệt)):
admin tổng, grant moderator, nhận ca, phản đối cả quyết định của moderator, dừng tự động, điều tra
riêng và trang “Nội dung đã được ẩn” thay luật cũ của PUB-05 B, MOD-09 và MOD-10; có core thuần,
model và replay (chưa migration/API/worker/UI). **#150 có lõi publication thuần cho AUD/PUB-01/02 và #151 có lõi moderation evaluator + scripted mock cho MOD-07..09; chưa tích hợp runtime**; model
và gap từng action ở [registry](../docs/development/product-decisions-registry.md). Không dòng nào đổi nghĩa
của COM-07/08, MOD-01/05, INV-01..08.

### 14.1. Audience, xuất bản và xóa (AUD, PUB)

Hành vi chi tiết: [xuất bản và kiểm duyệt](../docs/architecture/publication-moderation.md).

| ID | Luật | Nghiệm thu tiêu biểu | Đề xuất UX/dữ liệu |
| --- | --- | --- | --- |
| AUD-01 | Bài viết có đúng hai audience: **Chỉ mình tôi** và **Công khai**. Không unlisted, bạn bè, nhóm kín hay ACL chia sẻ | API/schema không nhận giá trị thứ ba; không có đường đọc bằng “link bí mật” | Mặc định “Chỉ mình tôi”; bộ chọn hai lựa chọn có mô tả hệ quả |
| AUD-02 | Audience **không phải** trạng thái xuất bản: tách khỏi nháp/chờ duyệt/được duyệt/đã xóa. Chọn công khai không bỏ kiểm duyệt; Approved không tự công khai bài chỉ lưu riêng; ý định công khai của tác giả gắn đúng snapshot/version | Lưu riêng → evaluator không được gọi. Công khai snapshot r → chờ kết quả của r. Về “Chỉ mình tôi” rồi kết quả Approved tới → không công khai. Tab cũ không đảo lựa chọn mới hơn | Tác giả thấy riêng: audience, kết quả kiểm duyệt từng bản, bản đang công khai. Lưu `generation` của ý định audience |
| PUB-04 | Tác giả xóa bài **đang công khai**: bài rời Khám phá, tìm kiếm, hashtag, Đề xuất, Top 100, bài liên quan; người khác không đọc được nội dung; URL cũ mở trang thay thế “Nội dung đã được tác giả xoá”; bình luận hợp lệ đã có vẫn đọc được ở đó | Guest mở URL cũ: không tiêu đề/nội dung/preview; thấy bình luận đã duyệt, không thấy pending/rejected/removed của người khác. Không bình luận nào bị xóa theo bài | HTTP `410` kèm trang; `noindex`; xác nhận trước khi xóa nêu rõ không khôi phục được. Lưu thời điểm xóa; không purge tự động (O-08) |
| PUB-05 | Bốn trường hợp tách bạch: **A** tác giả xóa bài từng công khai (PUB-04); **B** tác giả chuyển riêng tư; **C** admin gỡ cả bài vì vi phạm; **D** chưa từng công khai. **11/11/2026:** B của bài **đang công khai** để lại trang **“Nội dung đã được ẩn”** ở URL cũ cùng bình luận đủ điều kiện (PUB-07); xóa từ trang đã ẩn giữ bình luận như A. C và D trả không tìm thấy | C/D: URL cũ trả không tìm thấy, không trả bình luận, kể cả với link/thông báo cũ. A/B: không tiêu đề, thân bài, preview hay listing. Preview/trích dẫn do hệ thống sinh từ bài không còn đọc được; chữ người khác tự viết trong bình luận không bị thu hồi hàng loạt | Quyền ghi trên trang thay thế/trang ẩn: không ghi mới (O-03). Thay luật cũ “B ẩn cả thảo luận” |
| PUB-06 | Gửi duyệt **theo phiên bản**. Tạo, lưu, sửa bài riêng tư không mở ca. Chỉ yêu cầu công khai của tác giả kích hoạt kiểm duyệt đúng snapshot. Bản đang đăng vẫn hiển thị khi bản sửa chờ/bị từ chối. Công khai lại chỉ dùng quyết định duyệt **còn hiệu lực** cho đúng snapshot và đúng phiên bản chính sách; policy đổi → lượt duyệt mới; quyết định của người được đưa lại cho người có thẩm quyền xem xét. Chuyển riêng tư đóng ca duyệt và phản đối của tác phẩm, giữ lịch sử quyết định | Kết quả AI hay kết luận người tới sau khi chuyển riêng tư: không tạo quyết định, không đổi bản công khai. Công khai lại bản đang chờ mà ca đã đóng: mở lượt mới. Bản được AI duyệt ở policy cũ: lượt AI mới; bản người duyệt ở policy cũ: về hàng chờ người | Tác giả thấy “Đang chờ kiểm duyệt” khi gửi lại; lịch sử giữ quyết định cũ kèm policy version |
| PUB-07 | Bài **đang công khai** chuyển “Chỉ mình tôi”: URL cũ chỉ hiện **“Nội dung đã được ẩn”** và bình luận đủ điều kiện; tiêu đề, thân bài, preview rút khỏi mọi đường đọc công khai, tìm kiếm, Khám phá, Đề xuất. Bài chưa từng công khai và bài bị admin gỡ trả không tìm thấy. Công khai lại thay trang ẩn bằng bài; xóa từ trang ẩn để lại trang thay thế của PUB-04 | Guest mở URL cũ của bài đã ẩn: không tiêu đề/nội dung, thấy bình luận đã duyệt; người viết thấy thêm bình luận chờ/bị từ chối của mình. Ẩn rồi xóa: bình luận vẫn đọc được. Ẩn rồi công khai lại: bài trở lại | HTTP `200` kèm `noindex` cho trang ẩn (đề xuất); trang ẩn không hiện tên tác giả, thẻ, hashtag (như O-09) |

### 14.2. Kiểm duyệt bằng trợ lý AI, quản trị viên, moderator và phản đối (MOD-07..15)

| ID | Luật | Nghiệm thu tiêu biểu | Đề xuất UX/dữ liệu |
| --- | --- | --- | --- |
| MOD-07 | Evaluator trả kết quả **có cấu trúc** gắn target, revision/content digest, policy version, evaluation/job identity, outcome và lý do. Chỉ kết quả hợp lệ, còn hiệu lực, đúng phiên bản và đủ authority mới tạo quyết định nghiệp vụ. Hiện tại: contract + mock scripted, **không** provider thật | Sai revision, trùng, sai policy, sau khi target bị xóa/gỡ, sau admin override: không mutation. Output không hợp lệ không thành duyệt | Kết quả lưu append-only: target, digest, policy, job, outcome, mã lý do, thời điểm; không lưu prompt/nội dung riêng vào log |
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
| ACC-03 | **Username + mật khẩu** là đăng nhập chính; mã email có thể bổ sung sau, chưa ưu tiên. Username đăng nhập **khác** bút danh công khai; không yêu cầu trùng nhau. Đổi hướng không được bỏ session/rate-limit/security guard hiện có | Đăng nhập bằng username + mật khẩu tạo cùng loại phiên như hiện nay; bút danh đổi không đổi username; lỗi đăng nhập không tiết lộ username có tồn tại | Username: ASCII thường `a–z 0–9 _ .`, 3–30 ký tự, duy nhất không phân biệt hoa thường, không hiển thị công khai. Mật khẩu: ≥ 10 ký tự, không luật ký tự bắt buộc, Argon2id. Email đã xác minh là kênh khôi phục tùy chọn (O-11, O-12) |

**Hiện trạng (IMPLEMENTED):** chỉ có authority native mã email một lần
([ADR D-05](../docs/architecture/auth-authority.md)); code đó không chứng minh username/mật khẩu
đã có. Lượt này không viết runtime auth và không xóa code mã email.
