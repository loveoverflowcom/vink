# Requirements nền tảng cho Vink

**Phiên bản:** 0.2 · **Ngày:** 30/10/2026 · **Trạng thái:** Đề xuất phạm vi và tiêu chí nghiệm thu, chưa phải checklist đã PASS.

## 1. Mục tiêu của MVP

Chứng minh một vòng lặp hoạt động: **tác giả viết và lưu nháp tin cậy → gửi một snapshot để đăng → được kiểm duyệt → người đọc đọc và phản hồi → tác giả sửa hoặc viết tiếp**. Một tuyển tập nhỏ giúp người mới khám phá nội dung; công cụ vận hành đủ để xử lý lỗi và khiếu nại.

Requirements dưới đây triển khai [Vision](vision.md) và phương án B đề xuất trong [Tech Stack](../docs/architecture/tech-stack.md). Nếu đổi kiến trúc, yêu cầu sản phẩm vẫn giữ; acceptance về ranh giới Discourse được thay bằng kiểm chứng tương đương của owner mới.

## 2. Phạm vi và mức ưu tiên

| Mức | Ý nghĩa |
| --- | --- |
| Must — Pilot | Bắt buộc trước khi thử với nhóm được mời; người duyệt có thể giữ vai trò chính, AI hỗ trợ |
| Must — Beta | Bắt buộc trước khi mở public beta hoặc mở chức năng tương ứng |
| Should | Tăng giá trị nhưng có thể lùi mà không làm sai lời hứa cốt lõi |
| Later | Có hướng phát triển, không thuộc cam kết MVP |

Pilot ưu tiên tác phẩm một phần. Mô hình phải giữ định danh tác phẩm/đơn vị văn bản ổn định để bổ sung chương; UI nhiều chương cơ bản là Must khi chủ sản phẩm chọn nó làm tính năng của public beta. Không ép một bài thơ hiển thị như tiểu thuyết.

Các vai trò tối thiểu: khách đọc, tác giả, người duyệt chính sách, biên tập Tạp Chí và quản trị hệ thống. Một người có thể giữ nhiều vai trò trong pilot; quyền thực thi vẫn phân biệt theo hành động. Quyền biên tập, trust level hoặc có tài khoản đăng nhập không tự cấp quyền xuất bản thay tác giả.

## 3. Viết và bản thảo

| ID | Ưu tiên | Yêu cầu | Nghiệm thu tiêu biểu |
| --- | --- | --- | --- |
| WR-01 | Must — Pilot | Nháp riêng có autosave và trạng thái lưu rõ | Refresh giữ nguyên dữ liệu đã được server xác nhận; mất mạng hiển thị phần chưa lưu, cho retry/export; không tuyên bố đã lưu khi chưa thành công; tài khoản khác/khách không đọc được nháp qua UI, API, SSR hoặc URL đoán |
| WR-02 | Must — Pilot | Soạn và render đúng tiếng Việt, thơ, đoạn văn | Bộ mẫu dấu tiếng Việt, IME, khổ thơ, xuống dòng và paste từ Word/Docs đi qua save → render → edit không mất cấu trúc; kiểm trên desktop và mobile mục tiêu |
| WR-03 | Must — Pilot | Snapshot gửi duyệt bất biến, working draft có version | Sửa tiếp tạo draft/snapshot mới; hai tab cùng lưu không âm thầm ghi đè; snapshot đã gửi có ID/hash ổn định, có thể đối chiếu lại |
| WR-04 | Must khi mở nhiều chương | Danh sách chương và trạng thái riêng từng chương | Thêm/sắp xếp chương nháp không làm lộ nội dung chưa đăng; URL/ID chương cũ giữ ổn định; trật tự công khai chỉ đổi qua hành động được phép |
| WR-05 | Must — Pilot | Xuất nội dung cơ bản theo định dạng có thể dùng lại | Tác giả có thao tác xuất văn bản của mình, giữ Unicode và ngắt dòng; không phụ thuộc Discourse topic ID làm khóa chính |
| WR-06 | Should | Xuất đầy đủ lịch sử và metadata | Có thể xuất revisions, metadata và schema version trong phạm vi quyền/retention; phân biệt dữ liệu tác giả với audit nội bộ |

Không cam kết cộng tác thời gian thực hoặc offline-first trong pilot. Local recovery, nếu có, phải phân biệt với bản đã lưu server, tách theo tài khoản và có cách xóa trên thiết bị dùng chung.

## 4. Xuất bản và rút bài

| ID | Ưu tiên | Yêu cầu | Nghiệm thu tiêu biểu |
| --- | --- | --- | --- |
| PUB-01 | Must — Pilot | Công khai đúng snapshot, có ý định xuất bản của tác giả | Nháp chỉ lưu hoặc preview không xuất hiện công khai dù AI trả Approved; nút “Gửi để đăng khi được duyệt” lưu rõ ý định theo snapshot; publish kiểm lại quyền, trạng thái hiện tại và quyết định duyệt |
| PUB-02 | Must — Pilot | Bản đang công khai độc lập bản đang sửa | rev1 vẫn đọc được khi rev2 Draft/PendingAI/NeedsRevision/Rejected; chỉ transaction publish hợp lệ thay pointer; callback cũ không thay bản mới hoặc đảo quyết định của người duyệt |
| PUB-03 | Must — Pilot | Có cách rút bài và bảo vệ quyền ở mọi bề mặt | Ngay khi rút bài có hiệu lực ở Vink, public API/SSR/search/Tạp Chí không trả phần phải ẩn; remote cleanup có trạng thái riêng. Chỉ báo hoàn tất toàn bộ sau khi xác minh direct Discourse và snippet/cache trong phạm vi policy đã được thu hồi; lỗi giữ pending và cảnh báo, không khôi phục publication |

PUB-03 cần định nghĩa rõ phạm vi thu hồi trong hệ thống, thời hạn xử lý và cách xử lý lỗi đồng bộ. Không coi việc xóa card ở trang chủ là hoàn tất. Không hứa thu hồi các bản sao đã được người dùng lưu bên ngoài hệ thống.

## 5. Kiểm duyệt và AI

| ID | Ưu tiên | Yêu cầu | Nghiệm thu tiêu biểu |
| --- | --- | --- | --- |
| MOD-01 | Must — Pilot | Tách chính sách cộng đồng khỏi chất lượng văn chương | Bài an toàn nhưng viết yếu vẫn có thể đăng ở Xưởng; AI không từ chối vì không đạt điểm nghệ thuật, độ “chân thành” hoặc thiếu khả năng vào Tạp Chí |
| MOD-02 | Must — Pilot | Kết quả duyệt chỉ áp đúng đối tượng/phiên bản | Gắn revision/hash/policy/decision version; thử callback trùng, sai hash, trễ sau human override và sau rút bài: không có mutation trái phép |
| MOD-03 | Must — Pilot | Timeout/lỗi AI có human review được vận hành | Worker chết, hết retry hoặc quá deadline tạo/đánh dấu case thủ công; tác giả thấy trạng thái và khung xử lý được thông báo, người phụ trách thấy tuổi case; có watchdog, cảnh báo backlog và đối soát; không tự approve vì provider lỗi |
| MOD-04 | Must — Pilot | Giải thích và khiếu nại | Tác giả xem lý do có thể hành động, gửi appeal; quyết định mới ghi người thực hiện và căn cứ; duplicate appeal không tạo nhiều case; được mở lại khi có bằng chứng mới theo quyền moderator |
| MOD-05 | Must — Pilot | Phân quyền AI và dữ liệu đầu vào | Prompt injection trong truyện không được thực thi như chỉ thị; worker không tự mở URL/tệp hay dùng tool theo nội dung; output schema bị kiểm tra; nháp riêng không gửi provider khi chưa có hành động/đồng ý phù hợp |
| MOD-06 | Must trước tự động duyệt | Có bộ đánh giá tiếng Việt và chế độ thu hồi tự động hóa | Mẫu có thơ, phương ngữ, lời thoại, trào phúng và nội dung mơ hồ; đo sai lệch/false positives theo loại; ngưỡng cho phép được review; có thể tắt auto-approval mà vẫn giữ human queue |

Kết quả `NeedsRevision` liên quan tới chính sách phải khác góp ý writing coach. Góp ý văn phong tự nguyện không trở thành điều kiện bắt buộc để được đăng. Confidence do model tự báo không được coi như xác suất đã hiệu chuẩn.

Lịch sử moderation giữ kết luận, policy/model version, thời điểm và actor cần thiết để audit; retention và quyền xem phải được quyết định. Không lưu tràn lan toàn bộ prompt hoặc nội dung riêng vào logs vận hành.

## 6. Cộng đồng và Discourse

| ID | Ưu tiên | Yêu cầu | Nghiệm thu tiêu biểu |
| --- | --- | --- | --- |
| COM-01 | Must — Pilot | Thread ổn định cho work/chapter | Xuất bản lại không tạo thread mới; retry sau crash không làm sinh nhiều topic hữu hiệu cho cùng subject; có cách đối soát trường hợp Discourse đã tạo nhưng Vink chưa ghi mapping |
| COM-02 | Must — Pilot | Tiền kiểm ở owner của bình luận trên mọi cửa đang bật | Tạo qua Vink/native/API đều chưa public khi pending; đường email/import không dùng được tắt; nếu cho sửa bình luận công khai thì phải có gate tương ứng; test cả quyền staff/system đã được phép ngoại lệ |
| COM-03 | Must — Pilot | User actions thực thi đúng người và đúng quyền | Browser không thể đổi username/user ID để gửi thay tài khoản khác; account mapping lấy từ session; key không xuất hiện trong HTML, WASM, JS, logs hoặc lỗi gửi cho người dùng |
| COM-04 | Must — Pilot | Suy giảm chức năng rõ khi Discourse lỗi | Bài công khai vẫn đọc được; bình luận hiển thị trạng thái phù hợp, không giả gửi thành công; retry không nhân đôi phản hồi; hoạt động trở lại có thể đối soát |
| COM-05 | Must — Pilot | Góp ý có ngữ cảnh và lưu bài | Tác giả chọn mục đích phản hồi; người đọc thấy tín hiệu đó; lưu/bỏ lưu rõ và idempotent; không dùng điểm sao làm cổng đăng bài |
| COM-06 | Should — Beta | Theo dõi tác giả và cảm nhận nhẹ | Theo dõi/bỏ theo dõi rõ, không tạo bản dữ liệu cạnh tranh ở hai owner; tín hiệu tương tác không tự quyết định quyền đăng hoặc tuyển chọn |

Màn quản trị Discourse xử lý bình luận/reviewable. Vink cần màn nhỏ riêng cho tác phẩm, quyết định, appeal và curation; không coi toàn bộ phần này đã được Discourse Admin cung cấp. Review API/action/version và đường create/edit phải được kiểm trên release cụ thể, như các nguồn D3–D5 trong [Sources](../docs/references/sources.md).

## 7. Khám phá, tuyển chọn và trải nghiệm đọc

| ID | Ưu tiên | Yêu cầu | Nghiệm thu tiêu biểu |
| --- | --- | --- | --- |
| ED-01 | Must — Pilot | Trang chủ hữu ích khi ít nội dung | Với một tuyển tập nhỏ vẫn có lời dẫn, đề bài hoặc CTA tham gia phù hợp; không có thống kê giả, vùng trống dài hoặc feed vô hạn thiếu nội dung |
| ED-02 | Must — Pilot | Curation độc lập moderation và ghim phiên bản | Bài không được chọn vẫn đọc được ở Xưởng; pilot chỉ hiện tuyển chọn khi snapshot được chọn còn là publication hiện hành; đổi bản tạm ngừng mục tuyển chọn chờ biên tập; rút bài đóng hiển thị |
| UX-01 | Must — Pilot | Đọc dài trên desktop/mobile, M3 nhất quán | Giữ thơ/đoạn; không vỡ layout ở màn hẹp mục tiêu và khi tăng cỡ chữ; keyboard focus, screen-reader labels, tương phản sáng/tối được kiểm; public reader có nội dung HTML từ server |
| UX-02 | Must — Beta | Tìm nội dung công khai bằng tiếng Việt | Bộ truy vấn có/không dấu, tác giả, tiêu đề và tags trả kết quả chấp nhận được; draft, revision chưa đăng và bài đã rút không lọt vào kết quả |
| UX-03 | Must — Pilot | Tìm lại tác phẩm và trạng thái của mình | Người dùng tìm được bản nháp, bài đã đăng, trạng thái duyệt và hành động sửa/khiếu nại; không phải đoán URL hoặc tìm trong thông báo cũ |
| UX-04 | Should — Beta | Hồ sơ nâng cao và thông báo cộng đồng | Có hồ sơ tác giả và đường xem reply/mention phù hợp; chưa cần inbox đa hệ thống hoàn chỉnh |

## 8. Xác thực, dữ liệu và vận hành

| ID | Ưu tiên | Yêu cầu | Nghiệm thu tiêu biểu |
| --- | --- | --- | --- |
| ACC-01 | Must — Pilot | Một định danh Vink ổn định và mapping có chứng cứ | Đổi email/bút danh không đổi tác giả của tác phẩm; account provisioning lỗi không làm tạo sai người; các session và database Cantos không được dùng chung |
| ACC-02 | Must — Pilot | Quyền và trạng thái khóa nhất quán | Đăng nhập không tự cấp quyền biên tập/admin; thử logout/ban/revoke qua Vink và truy cập trực tiếp Discourse; các trường hợp chưa đồng bộ có cơ chế đóng quyền phù hợp |
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
4. **Topic-create không rõ kết quả:** Discourse tạo topic rồi worker crash; retry/reconcile xác định topic cũ và giữ một mapping hữu hiệu.
5. **Mọi cửa bình luận:** native/API/Vink và tính năng edit đang bật tuân policy; đường không được hỗ trợ từ chối có chủ đích, không chỉ ẩn nút.
6. **Rút bài đã tuyển:** public read và tuyển tập không lộ nội dung đã bị rút; snippet thảo luận được xử lý; job/event cũ không tự khôi phục.
7. **Account đổi thông tin:** đổi bút danh/email không đổi ownership; client giả `Api-Username` không gửi được thay người khác.
8. **AI và Discourse gián đoạn:** tác phẩm đã công khai vẫn đọc được; submissions không tự approve; người dùng biết nội dung chưa đăng/chưa gửi và có đường xử lý tiếp.

Đây là kịch bản nghiệm thu cần thực hiện khi có implementation, không phải kết quả thử đã chạy trong lần soạn tài liệu này.

## 11. Ngoài phạm vi hiện tại

Later: đồng biên tập thời gian thực; nhóm kín với ACL phức tạp; ghi chú bên lề tự re-anchor qua mọi lần sửa; chat/DM; hệ điểm/karmas; chợ bản quyền, paywall và chi trả tác giả; ứng dụng native; hệ recommendation riêng; chuyển thể audio tự động và SSO chung Cantos.

Không viết DDL cuối cùng, OpenAPI đầy đủ hoặc model Quint thực thi trong đợt requirements này. Các tài liệu đó cần theo sau quyết định phạm vi, editor schema, identity và integration spike.
