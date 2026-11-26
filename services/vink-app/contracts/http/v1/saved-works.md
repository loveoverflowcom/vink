# HTTP v1 · Danh sách Đã lưu của tôi

Hợp đồng đã triển khai ở mức `persistence-api` (#387). Vink App sở hữu quan hệ (PrincipalId, WorkId). Web dùng hai tuyến theo bài từ [Reader](../../../../../apps/vink-web/src/pages/saved_work.rs) và dùng cả tuyến danh sách từ [Hồ sơ Đã lưu](../../../../../apps/vink-web/src/pages/saved_profile.rs). [PRD §18 và bảng vòng đời](../../../../../prd/requirements.md#18-lưu-bài-để-đọc-lại-saved-works) · [phiên, Origin và lỗi v1](README.md) · [registry](../../../../../docs/development/saved-works.md).

## Tuyến và quyền

| Method và path | Input | 200 · JSON |
| --- | --- | --- |
| `GET /api/v1/me/saved-works/{work_id}` | WorkId hợp lệ | Trạng thái riêng `{"saved":false,"generation":1}` nếu chưa có fence. Nếu có fence, trả đúng trạng thái/generation hiện hành, kể cả sau bỏ lưu. |
| `PUT /api/v1/me/saved-works/{work_id}?expected_generation=G` | Đặt Saved, không body | `{"saved":true,"generation":2}` (ví dụ PUT mới tại G=1). |
| `DELETE /api/v1/me/saved-works/{work_id}?expected_generation=G` | Đặt NotSaved, không body | `{"saved":false,"generation":3}` (ví dụ DELETE mới tại G=2), kể cả khi bài không còn đọc được. |
| `GET /api/v1/me/saved-works` hoặc `?cursor=C` | Trang đầu bỏ cursor; trang sau dùng nguyên token server cấp | `{"items":[…],"next_cursor":null}` hoặc token chuỗi nếu còn dòng để đọc. Không có total/limit tùy chỉnh. |

App xác thực phiên v1 và tự giải PrincipalId cho **tất cả** các tuyến. Không có tuyến đọc theo handle hay PrincipalId do người gọi chọn. GET một bài chỉ hỏi về quan hệ của chủ, không tra hoặc tiết lộ bài đích có tồn tại hay không. Tài khoản chỉ bị hạn chế đăng/bình luận vẫn dùng được cả bốn tuyến.

GET chấp nhận điều kiện tùy chọn `X-Vink-Expected-Principal`. Web Hồ sơ đặt điều kiện này khi tải danh sách và đọc đối soát một cặp. Thiếu header vẫn đọc theo phiên. Header hợp lệ nhưng khác principal đã xác thực trả 403 trước khi lấy dữ liệu.

PUT/DELETE yêu cầu `Origin` khớp `VINK_PUBLIC_ORIGIN`. Client gửi thêm header `X-Vink-Expected-Principal` là PrincipalId của phiên lúc khởi tạo thao tác. Đây là điều kiện chống đổi tài khoản giữa hai tab, actor vẫn do server xác thực. Kiểm standing hiện hành tại commit bằng fence của App. Với PUT, kể cả PUT lặp, phải đọc được publication công khai tại commit, gồm bài của chính mình. Preview, nháp, bản riêng của chủ và trang thay thế đều không đủ điều kiện. DELETE chỉ cần standing và fence của cặp.

## Thứ tự ý định và ACK

`generation` là số nguyên dương do server cấp cho từng cặp. Client gửi đúng giá trị đọc được từ trạng thái riêng, dòng danh sách hoặc ACK. Cặp chưa từng nhận ý định bắt đầu ở 1, theo mẫu reaction của App. Không dùng đồng hồ thiết bị. Generation không được quay vòng hoặc reset khi tăng. Sau kiểm quyền, đối chiếu và ghi nguyên tử như sau.

| Điều kiện | Kết quả quan sát được |
| --- | --- |
| G bằng generation hiện tại | Chấp nhận, tăng generation đúng 1, **kể cả no-op mới**. Chỉ chuyển false → true mới lấy saved_at từ server và chiếm một suất. true → true giữ saved_at. true → false giải phóng một suất. false → false vẫn dựng/tiến fence. |
| G cũ hơn, trạng thái hiện tại đã bằng mong muốn | `200` với trạng thái/generation **hiện tại**. Không tăng fence, đổi saved_at hay tính suất lần nữa. ACK mô tả kết quả hiện có. |
| G cũ hơn, trạng thái trái mong muốn, hoặc G lớn hơn generation hiện tại | `409 version_conflict` kèm `field: expected_generation` và `current_version` là generation hiện hành. Không ghi. |

Kiểm cap trong cùng lần commit khi thêm membership. Tính tất cả quan hệ Saved, gồm cả dòng đang bị bỏ qua; fence NotSaved không chiếm suất. Khi đang đủ 10.000, PUT xác nhận lại một bài đã lưu vẫn được. Đang ở 9.999, hai lệnh đồng thời cho hai WorkId mới chỉ được cấp một suất. Lệnh bị từ chối không tiêu thụ generation. Giữ fence qua DELETE, chỉ xóa cùng dữ liệu của người lưu khi xóa tài khoản. PostgreSQL giữ generation và thời điểm lưu trên cùng một dòng. Bỏ lưu đặt thời điểm về NULL.

Ví dụ: PUT(1) → Saved/2; retry PUT(1) → Saved/2; PUT(2) → Saved/3 cùng saved_at; DELETE(3) → NotSaved/4. PUT(1) đến muộn → 409. DELETE(4) mới → NotSaved/5, PUT(4) đến sau → 409. Khi người dùng thực sự lưu lại bằng PUT(5) → Saved/6 với saved_at mới. DELETE(3) cũ không thể đảo lần lưu thứ hai (ABA).

Khi mất ACK, timeout, 503 hoặc conflict, client giữ trạng thái đã xác nhận, GET lại đúng cặp trước khi cho gửi ý định mới. Chỉ nhận phản hồi của đúng phiên, WorkId và thế hệ thao tác đang chờ. Không tự nâng G rồi phát lại lệnh trái với trạng thái đọc được.

## Phân trang và phép chiếu tại lần đọc

Sắp `saved_at DESC, work_id ASC` (thứ tự UUID), lọc theo chủ và quan hệ Saved. Cursor mờ, chống sửa, ràng buộc chủ, phiên bản thứ tự và biên khóa cuối. Trang kế tiếp chỉ lấy thời điểm nhỏ hơn biên hoặc cùng thời điểm với WorkId lớn hơn biên. Xóa dòng mang biên không làm hỏng cursor. Không phân trang bằng offset.

Mỗi request kiểm lại publication và quyền hiện hành trước khi đưa từng dòng vào trang. Bỏ qua Gone, tiếp tục quét đến 20 **dòng được hiện** hoặc hết nguồn. Xác định còn trang bằng dòng hiện được kế tiếp; cursor sau dòng cuối đã trả, không vượt qua dòng đầu trang sau. 21 dòng khả dụng xen với nhiều Gone phải trả 20 rồi 1, không lặp hoặc tạo trang rỗng chỉ vì Gone. Khi quyền thay đổi giữa hai request, trang sau có thể rỗng và kết thúc. Bài vừa lưu/khôi phục nằm trước biên sẽ thấy khi làm mới trang đầu.

| `items[].kind` | Các trường được phép ngoài `kind` | Khi nào trả |
| --- | --- | --- |
| `readable` | `work_id`, `generation`, `saved_at` (RFC 3339 UTC), `title`, `form` (nhãn hiện hành hoặc null), `author` theo [attribution v1](social-graph.md) | Publication hiện tại đọc được. Mở Reader theo WorkId; chỉ byline được phép mới có handle/link. |
| `hidden` | `work_id`, `generation` | Tác giả chuyển riêng tư sau công khai; DTO đủ cho Bỏ lưu. |
| `author_deleted` | `work_id`, `generation` | Tombstone do tác giả xóa bài/tài khoản tác giả bị xóa. |
| Gone (nội bộ) | — | Admin gỡ hoặc không tìm thấy: loại hẳn khỏi JSON. Quan hệ có thể khôi phục giữ saved_at. |

Mỗi lần truy vấn lấy metadata công khai từ nguồn hiện hành. Quan hệ/fence không lưu tiêu đề, thẻ, byline hay trích đoạn. Một bài đã lưu chuyển sang riêng tư giữa hai trang chỉ có thể xuất hiện như `hidden`; công khai lại khi còn quan hệ giữ saved_at. Xóa tài khoản người lưu xóa toàn bộ quan hệ và fence của họ.

## Lỗi và cache

Dùng envelope v1 `{"error":{"code":"…","message":"…"}}` với `field` và `current_version` khi quy định. UI chọn copy bằng code.

| HTTP | code | Điều kiện |
| --- | --- | --- |
| 401 | `unauthenticated` | Không có principal active khi xác thực, gồm phiên thiếu/hết hạn/bị thu hồi hoặc đình chỉ đã biết ở bước giải phiên. |
| 403 | `principal_suspended` | Standing fence phát hiện đình chỉ ở lúc commit. |
| 403 | `forbidden` | Origin ghi sai/thiếu hoặc principal kỳ vọng khác phiên. |
| 404 | `not_found` | WorkId sai dạng hoặc PUT tới bài thiếu/không công khai/không đọc được. Cùng biểu diễn cho các lý do không đọc được. |
| 409 | `saved_works_limit_reached` | Thêm quan hệ mới vượt 10.000. “Danh sách Đã lưu đã đầy. Bỏ lưu một bài để lưu thêm.” |
| 409 | `version_conflict` | Fence không cho áp dụng ý định; đọc lại theo bảng trên. |
| 422 | `validation_failed` | G thiếu, không dương, trùng hoặc sai dạng (`field=expected_generation`); cursor rỗng, sai hoặc của chủ khác (`cursor`); thiếu/sai dạng header principal (`expected_principal`); query lạ (`query`). |
| 503 | `dependency_unavailable` | Không xác định được phiên/quyền hoặc kết quả vì lỗi phụ thuộc; không suy thành “chưa lưu”. |

Mọi response của bốn tuyến, kể cả lỗi, gửi `Cache-Control: private, no-store`. Không thêm saved flag, count, fence, danh sách hay link riêng vào DTO/HTML/hydration payload công khai hoặc ETag READ-01. CSR standalone chỉ đọc lớp riêng sau khi xác nhận phiên cùng origin; SSR pre-cutover chỉ tải lớp riêng sau hydration. Không ghi dữ liệu vào cache READ-02, service worker, storage bền hoặc log chứa liên kết giữa người lưu và bài. Đổi principal/đăng xuất xóa state riêng, cursor và phản hồi đang bay. URL Web tab của khách/người khác dùng cùng trang 404 như URL không tồn tại. SSR dùng cùng status và HTML NotFound; nếu host CSR dùng SPA fallback, cả hai URL nhận cùng shell/status ban đầu rồi cùng NotFound sau khi router giải quyền. Không đưa `/@handle/da-luu` vào HTML công khai.
