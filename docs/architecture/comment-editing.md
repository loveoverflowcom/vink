# Sửa bình luận có kiểm duyệt và dấu “Đã sửa” — đặc tả đích

**Ngày:** 10/11/2026 · **Nguồn:** `PRODUCT-DECISIONS-20261110`
([DecisionDelta](product-decisions-20261110.md)), USER-04/15 · **Yêu cầu:** COM-02, COM-08, COM-09,
COM-10, LVE-FR-04 · **Quyết định:** D-08, D-13, D-14.

**Trạng thái:** `TARGET_BEHAVIOR`, **chưa có implementation**. Luật “đã chốt” là
`PRODUCT_CONFIRMED`; CHOICE-01 A và các chi tiết khác là `DESIGN_PROPOSED`. Đặc tả thực thi:
[`comment_edit.qnt`](../../services/vink-app/behavior/target/comment_edit.qnt). Quyền của tác giả
bài (COM-07/08, D-13) và cấu trúc hai tầng không đổi.

## 1. Hiện trạng (`CURRENT_IMPLEMENTATION`)

Bình luận **không sửa được**: không có use case sửa, bảng revision body hay cờ đã sửa;
`domain/discussion/lifecycle.rs` ghi “Editing does not exist: D-08 keeps public edits blocked”.
Người viết chỉ gỡ được bình luận của mình (`removal='author'`, body bị xóa). Quyết định kiểm duyệt
do người có grant thực hiện; “Hữu ích” neo ID bình luận vì bình luận bất biến
([registry LVE](../development/endorsement-core.md)).

## 2. Luật đã chốt

1. Người viết sửa bình luận/phản hồi **của chính mình**. Tác giả bài không sửa được bình luận của
   người khác chỉ vì sở hữu bài.
2. Bản sửa có version/revision và **đi qua kiểm duyệt** (evaluator, COM-09). Không ghi đè chữ công
   khai bằng nội dung chưa được duyệt.
3. Bản sửa được công khai mang dấu **“Đã sửa”**.
4. Lưu không thay đổi nội dung không tạo dấu sửa, không đổi thời gian.
5. Không mở công khai lịch sử chỉnh sửa.
6. Không coi vote cho nội dung cũ là vote cho mọi bản viết lại.

## 3. Mô hình hai bản (CHOICE-01 A, đề xuất)

Mỗi bình luận có các **bản** bất biến. Tại mỗi thời điểm:

| Vai | Ý nghĩa | Ai thấy |
| --- | --- | --- |
| Bản hiển thị (`shown`) | Bản đã được duyệt mà mọi người đang đọc | Mọi người có quyền đọc bình luận |
| Bản chờ (`candidate`) | Bản sửa mới nhất chưa có quyết định | Chỉ người viết (“Bản sửa đang chờ duyệt”) |

| Sự kiện | Bản hiển thị | Bản chờ | “Đã sửa” |
| --- | --- | --- | --- |
| Gửi bình luận | — | bản 1 | — |
| Bản 1 được duyệt | bản 1 | — | Không |
| Người viết sửa | bản 1 (không đổi) | bản 2 | Không |
| Bản 2 được duyệt | bản 2 | — | **Có** |
| Bản 2 bị từ chối | bản 1 (ở lại) | — | Không |
| Sửa tiếp khi bản 2 đang chờ | bản 1 | bản 3; bản 2 bị thay, kết quả của nó tới muộn không có hiệu lực | Không |
| Lưu đúng nội dung đang có | không đổi | không đổi | Không đổi; chỉ tăng version của ý định |
| Admin gỡ bản 1 khi bản 2 đang chờ | **không còn gì công khai** | bản 2 | — (hiện lại khi bản 2 được duyệt) |
| Người viết xóa / admin gỡ bình luận | không | không | Terminal; kết quả tới sau không hồi sinh |

“Đã sửa” nghĩa là **bản đang hiển thị không phải bản công khai đầu tiên**. Sửa trước khi bản đầu
được duyệt không tạo dấu (chưa ai đọc bản trước). Dấu không kèm lịch sử hay nội dung cũ.

## 4. Lệnh sửa và các tình huống đồng thời

Lệnh sửa mang `expected_version`, `command_key` và nội dung mới; thứ tự kiểm:

| # | Kiểm | Kết quả |
| --- | --- | --- |
| 1 | Actor là người viết | Không → `not_comment_writer` (kể cả tác giả bài) |
| 2 | Key đã dùng | Cùng payload → `replayed` (trả kết quả cũ, không ghi); khác payload → `key_reused` |
| 3 | Bình luận còn | Đã gỡ → `target_unavailable` |
| 4 | `expected_version` = version hiện tại | Không → `stale` (tab cũ) |
| 5 | Nội dung khác bản mới nhất | Giống → `reaffirmed`: tăng version (rào tab cũ hơn), không bản mới, không đánh giá, không dấu |
| 6 | — | `accepted`: tạo bản mới `pending`, tạo evaluation, tăng version; bản hiển thị **không đổi** |

- **Hai tab:** tab nào tới sau với version cũ nhận `stale`; không âm thầm ghi đè (cùng luật
  accepted-intent như COM-07 và LVE).
- **Mất ACK / ACK đảo thứ tự:** gửi lại cùng key trả đúng kết quả đã ghi của key đó, kể cả khi
  sau đó đã có bản sửa mới hơn; không tạo bản thứ hai.
- **Callback AI cũ:** kết quả cho bản đã bị thay → `superseded`; cho bản admin đã quyết định →
  `overridden`; cho bình luận đã gỡ → `target_gone`; không nhánh nào ghi gì.
- **Admin:** quyết định một bản được nêu tên. Duyệt chỉ công khai bản đó khi nó vẫn là bản mới
  nhất; quyết định về bản cũ không áp cho nội dung mới (`not_current`).

Kết quả evaluator theo cùng contract với bài: [mục 4 của đặc tả kiểm duyệt](publication-moderation.md#4-contract-evaluator-và-mock).

## 5. Quan hệ với “Hữu ích” (O-06, đề xuất)

Vote ở lại với bản mà nó được trao. Tổng công khai chỉ đếm vote của **bản đang hiển thị**; khi bản
sửa được công khai, tổng bắt đầu lại và vote cũ vẫn được lưu, không chuyển sang bản mới. Không vote
được cho bản đang chờ. Đây là cách đọc an toàn nhất của LVE-FR-04; phương án giữ vote qua bản sửa
cần chủ sản phẩm quyết định riêng.

## 6. Hướng đích cho kiểm duyệt bình luận (COM-09)

Bình luận gốc và phản hồi dùng cùng luồng: gửi → chờ đánh giá → kết quả hợp lệ → áp quyết định.
Điều này thay người-có-grant bằng evaluator ở vai trò quyết định mặc định, **không** biến thành
đăng-trước. Khác biệt với code hiện tại và đường chuyển tiếp nằm ở
[admission §8](discussion-admission.md#8-hướng-đích-sau-quyết-định-10112026).

## 7. Bằng chứng và gap

`comment_edit.qnt`: 14 invariant (`pendingNeverPublic`, `editedMarkTruthful`,
`unchangedSaveLeavesNoTrace`, `adminOverrideWins`, `votesStayWithTheirText`…), hai scenario và
negative control “bản sửa ghi đè chữ công khai”; chi tiết ở
[README target](../../services/vink-app/behavior/target/README.md) và
[registry](../development/product-decisions-registry.md). Chưa có: bảng revision bình luận, lệnh
sửa, evaluator, API, UI, khóa PostgreSQL cho race sửa × duyệt × gỡ. Khi PR runtime làm phần này,
model chuyển vào `behavior/discussion` cạnh `lifecycle.qnt` cùng Rust replay.
