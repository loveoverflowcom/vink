# Social Graph · hợp đồng HTTP đề xuất r1

Từ bút danh trên WorkCard, Reader, bình luận hoặc ngữ cảnh trả lời, Web mở `/@handle` từ
ref công khai của đúng tác giả. Card có đích tác phẩm riêng để hai link đều dùng được bằng
bàn phím. Người chưa có bài vẫn có Profile. Các biểu diễn dưới đây là **hợp đồng đích của
#288**. Wire name, route API và fixture chạy thật thuộc provider ở PR triển khai. Owner
và chuyển trạng thái nằm ở [đặc tả liên miền](../../../../../docs/architecture/social-graph.md).

## Ma trận quyền và hành động

| Actor / standing | Hành động → đích | Quyết định của server tại lần đọc hoặc commit | Trạng thái/điều khiển ở Web |
| --- | --- | --- | --- |
| Khách, hoặc chưa có binding (`needs_principal`) | Đọc hồ sơ còn hiệu lực, bài công khai, list/count | Public projection theo từng trường, độc lập với cookie. Tìm/đọc không provision. | Đọc được qua SSR/no-JS. CTA dẫn tới đăng nhập/hoàn tất bút danh rồi quay lại đích nội bộ hợp lệ. |
| Chủ có phiên active | Đọc/sửa Profile của mình, giữ/đổi Handle | Own read theo session. Khi ghi, lấy principal từ phiên, so chủ, standing, lifecycle và version tại commit. | Tôi → Chỉnh hồ sơ, quyền từng trường, quota và phiên bản. Cấm self-follow. |
| Chủ active | Xem Following/Followers của mình | Đọc list và count của mình kể cả `self`, page thuộc scope own. | Có thể phân trang toàn bộ danh sách hợp lệ. |
| Thành viên active khác | Đọc Profile/list/count người khác | Public projection giống khách. Phần riêng chỉ trả quan hệ liên quan trực tiếp tới viewer và target. | Follow/Unfollow theo trạng thái đã xác nhận. Mutual chỉ cấp cho hai bên. |
| Thành viên active khác | Đặt Follow/Unfollow profile hợp lệ | Kiểm phiên, Origin, hai principal khác nhau, capability và lifecycle sau khóa. Cấm lấy follower từ body. Tác giả có 0 bài vẫn hợp lệ. | Pending tại nút, mất ACK → đọc receipt và trạng thái hiện tại. |
| Người thứ ba, kể cả admin chỉ có quyền moderation | Xem Mutual của hai người khác, sửa Profile của họ hoặc đọc trường self | Không có quyền Social tương ứng. Public views chỉ trả facts công khai. | Không có control/endpoint liệt kê Mutual người khác. |
| Thành viên active | Đọc Đang theo dõi / mở bài | Kiểm membership, Publication hiện tại và provenance trước sort/limit. Follow không mở nháp, private, pending, withdrawn hay revision cũ. | Chế độ trong Khám phá. Phân biệt chưa theo dõi ai, đã theo dõi nhưng chưa có bài đủ điều kiện, lỗi đầu trang và lỗi append. |
| Chỉ hạn chế đăng/bình luận (SOC-DEP-02) | Đọc, sửa Profile/quyền của mình, Follow và Feed | Được như active đối với các hành động Social khi có capability riêng có nguồn. Lệnh đăng/bình luận kiểm capability tại owner tương ứng. | Giữ cạnh và các control Social. Đây là trạng thái đích phụ thuộc Account. |
| Hết phiên, Identity disable hoặc App full suspension | Dùng own/viewer/Feed, ghi hoặc replay | Guard Identity/App hiện hành đóng quyền private/write. Public actor-free đánh giá riêng. Identity/DB unavailable trả lỗi riêng cho đường phụ thuộc. | Bỏ dữ liệu của phiên cũ, cung cấp trạng thái đăng nhập/hạn chế/lỗi phù hợp. |
| Mọi actor | Nhắm tới Profile đang xóa/đã xóa qua byline cũ | Kiểm fence trước resolve alias, cấp ref, bytes, đọc trang tiếp hoặc ghi cạnh. | URL hồ sơ cũ ngừng dẫn tới danh tính. Trang bài thay thế theo Publication. Bình luận hợp lệ giữ nhãn đã xóa. |

## Dữ liệu mỗi người được thấy

Quyền thuộc **từng trường**. `self` chỉ cấp cho chủ đã xác thực. Handle chuẩn là địa chỉ
của Profile đang hoạt động. Đổi bút danh không xoay địa chỉ.

| Trường | Mặc định | Chủ chỉnh quyền | Quy tắc đọc |
| --- | --- | --- | --- |
| Bút danh | everyone | Không | Bắt buộc và luôn công khai. |
| Avatar | everyone | Có | Chỉ trả phần công khai đã có. SOC-OPEN-04 bao gồm đường đọc ảnh nếu chọn upload. |
| Bio | everyone | Có | Tùy chọn, người ngoài chỉ nhận giá trị khi được phép. |
| Ngày sinh đầy đủ | self | Có, sau gate ngày sinh | Tùy chọn. Công khai cả ngày/tháng/năm cần xác nhận chủ động, gắn với đúng giá trị/trường ở phiên bản lưu (OPEN-01). |
| Followers list | everyone | Có | Cạnh vào. Có thể phân trang dù count riêng. |
| Followers count | everyone | Có | Chỉ cấp tổng khi quyền count cho phép, dù list đang ẩn. |
| Following list | self | Có | Cạnh ra. Việc A ẩn list của A vẫn cho phép A xuất hiện trong Followers công khai của B. |
| Following count | self | Có | Đánh giá độc lập với list và với Followers count. |

| Biểu diễn | Dữ liệu được phép | Loại trừ |
| --- | --- | --- |
| Public Profile / author ref | Bút danh, current Handle và href cùng origin, giá trị công khai, liên kết list/count có quyền và tác phẩm đang public. Ref dùng chung cho card/Reader/root/reply/context/search. | Trường private và cả dấu hiệu có giá trị private, quota, version lệnh, viewer, id credential/subject. Không ghép URL bằng bút danh. |
| Own Profile | Tên, giá trị đã lưu hoặc chưa điền, quyền từng trường, version Profile, trạng thái setup và quota Handle, list/count của chủ. | Dữ liệu xác thực. Chỉ trả qua scope riêng của đúng chủ. |
| Viewer đối với một Profile | Own relationship, capability đặt trạng thái, version/thông tin đối soát được phép, Mutual giữa viewer và chủ khi có đủ hai cạnh. | Cạnh riêng giữa hai người khác, count/list self của target. Không dùng để xếp hạng công khai. |
| Đã xóa | Discussion còn được đọc có nhãn “Tài khoản đã xóa”. Trang bài theo Publication. | Không có public ref, id người cũ, ảnh hoặc link trong row, reply context, SSR hay metadata. Không suy trạng thái xóa từ lỗi mạng. |

**Ví dụ dữ liệu minh họa.** An đã công khai số Followers và giữ Following riêng. Đây là
phần Profile trong public response. Nếu An có bio/ngày sinh ở chế độ self, public body,
header và metadata phải giống trường hợp chưa điền, khi các facts công khai khác bằng nhau.

```json
{"author":{"pen_name":"An","handle":"an-may-27","href":"/@an-may-27"},"followers_count":3}
```

Trong form riêng, An thấy cả nội dung bio và quyền của trường.

```json
{"profile_version":4,"bio":{"value":"Viết về miền sông nước","visibility":"self"},"handle_setup_complete":true,"remaining_handle_changes":2}
```

Cùng lúc, viewer Bình đã Follow An và An chưa Follow lại. Phiên Bình nhận trạng thái quan hệ sau.

```json
{"target_handle":"an-may-27","following":true,"mutual":false,"pair_version":1}
```

Sau khi xóa tài khoản An, bình luận còn hiển thị nhãn tác giả như sau.

```json
{"author":{"kind":"deleted","label":"Tài khoản đã xóa"}}
```

List được phép và đọc thành công mới có trạng thái rỗng. Count được phép và đúng bằng 0 mới
hiện số 0. UI có thể bỏ control khi server không cấp quyền đọc list. Với giá trị tùy chọn,
người ngoài nhận cùng cách biểu diễn khi chưa điền hoặc bị ẩn. Chủ phân biệt được chưa điền,
đang tải, lưu lỗi và xung đột ở phần riêng. Khi count là self, bỏ cả tổng trang, total_count,
link “trang cuối” và metadata tiết lộ aggregate. Người đọc vẫn có thể tự đếm list công khai
hoặc suy Mutual từ hai cạnh công khai. Server chỉ cấp Mutual trực tiếp cho hai người trong cặp.

## Thời điểm đọc, cache và ảnh

- Mỗi page/GET/HEAD dùng một snapshot nhất quán của các fact dùng để cấp quyền, đọc cùng dữ
  liệu sẽ serialize. Request có snapshot trước commit đổi quyền có thể kết thúc theo quyền cũ.
  Snapshot lấy sau commit dùng quyền mới dù request đã vào hàng đợi trước đó. Nếu phải chờ khóa,
  đọc lại fact có thẩm quyền sau chờ. Client đã biết mutation/logout/fence loại cả response cũ
  đang bay. Bản sao đã nhận hợp lệ ngoài Vink không thể thu hồi.
- Resolve lifecycle, policy, scope và representation hiện hành **trước** `If-None-Match`.
  Chỉ cho 304 đối với đúng biểu diễn vẫn còn được đọc, validator phù hợp các byte thật.
  Nếu vừa giấu trường đã từng phát, trả body mới đã lọc. Nếu mất hẳn quyền, trả kết quả từ chối
  hoặc không còn tài nguyên. Mọi lỗi và phản hồi riêng dùng `no-store`. Public validator không
  phụ thuộc giá trị self, phiên hoặc private profile version. [READ-01](README.md#conditional-public-reads-read-01)
- HTML công khai, hydration JSON, title/description, OpenGraph, preloads, search, suggestion,
  list totals và cursor đều chỉ dùng projection public. CHOICE-05 cho phép public SSR + private
  overlay, hoặc authenticated SSR trong response riêng `private, no-store` cách ly theo request.
  Own/viewer và Feed không đi qua shared cache, IndexedDB hoặc `PublicKey::Works`. Đổi account
  kể cả A→B→A tạo generation mới. Bỏ request, page chain, receipt UI, dữ liệu form và Back cache
  của phiên cũ. Khi biết vừa thu hồi, xóa dữ liệu hiển thị liên quan ở cả tab, refetch khi
  focus/Back. Đường đọc mới có thể tiết lộ field phải revalidate trước khi hiển thị từ cache.
- Nếu chọn upload, cùng policy/lifecycle tại GET và HEAD cho URL cũ/mới, ảnh gốc, resize,
  thumbnail, CDN và validator. Đổi sang self, thay ảnh hoặc xóa tài khoản làm mất quyền với
  bytes cũ ở lần kiểm mới. URL đoán được, URL đã sao chép, redirect hoặc cache hit không được
  bỏ qua policy. Không phát URL bền truy cập thẳng storage. Placeholder, kích thước, alt text,
  response phụ và metadata cũng không báo ảnh riêng có tồn tại. Khi chưa bảo vệ được đường
  phát bytes thì giữ upload tắt. [Owner media #298](https://github.com/loveoverflowcom/vink/issues/298)

## Trang tiếp, lỗi và đối soát

Cursor bị ràng buộc với resource, target, scope public/own/viewer, mode, query, khóa sắp
và version contract. Với dữ liệu riêng, ràng buộc thêm viewer và phiên. Cursor opaque không chứa giá trị riêng
người nhận có thể giải mã. Mỗi lần đọc lại lọc quyền, lifecycle và membership hiện tại
trước LIMIT. Sai scope, hết hạn hoặc graph revision đã đổi theo CHOICE-06 thì bỏ cursor,
tải trang đầu. Với live membership, restart local khi xác nhận Follow/Unfollow, bỏ append
đang bay, refresh để thấy bài mới ở phía trên cursor. Trong cả hai cách, Back chỉ khôi
phục anchor còn được đọc. Trang lỗi không biến thành trang cuối rỗng.

| Kết quả quan sát | Ý nghĩa khi consumer xử lý |
| --- | --- |
| Thành công / no-op / replay cùng lệnh | Trả kết quả của ID đã ghi, version và trạng thái có quyền. Đối chiếu version mới nhất trước khi cập nhật nút/quota. |
| Input sai, Handle reserved/trùng, hết lượt | Lỗi trường hoặc giới hạn nghiệp vụ, transaction không trừ lượt. Giữ bản đang sửa và focus. |
| Chưa xác thực / không có quyền / target không còn đọc được | Dừng thao tác, xóa dữ liệu ngoài quyền. Đường public trả cùng footprint cho private/absent khi cần che sự tồn tại. |
| Conflict version / tái dùng command ID khác payload | Đọc lại owner/relationship và receipt. Giữ bản nháp của đúng phiên. Người dùng đối chiếu trước khi gửi ý định mới. |
| Mất ACK / timeout sau gửi | Có thể đã commit. Đọc kết quả theo command ID và state/version hiện tại. Retry cùng ID, cùng payload hoặc yêu cầu đối soát. Không gửi toggle mới. |
| Authority, DB hoặc mạng unavailable | Hiện lỗi và đường thử lại. Không giả chưa Follow, count bằng 0, tài khoản đã xóa hoặc đã lưu. |
