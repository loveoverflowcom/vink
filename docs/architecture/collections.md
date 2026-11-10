# Bộ sưu tập cá nhân — đặc tả đích

**Ngày:** 10/11/2026 · **Nguồn:** `PRODUCT-DECISIONS-20261110`
([DecisionDelta](product-decisions-20261110.md)), USER-09 · **Yêu cầu:** COL-01..06, AUD-01/02 ·
**Quyết định:** D-10, D-16.

**Trạng thái:** `TARGET_BEHAVIOR`, **chưa có implementation** (code hiện không có khái niệm bộ sưu
tập nào). Luật “đã chốt” là `PRODUCT_CONFIRMED`; phần ghi “đề xuất” là `DESIGN_PROPOSED`. Đặc tả
thực thi: [`collections.qnt`](../../services/vink-app/behavior/target/collections.qnt).

## 1. Luật đã chốt

- Vink là mạng xã hội bài ngắn; **không** nhiều chương. Bộ sưu tập là nhóm/thư mục bài của **chính
  tác giả**, không biến thành sách nhiều chương, không cộng tác, không quản lý bài người khác.
- Bộ sưu tập có đúng hai audience: **Chỉ mình tôi** / **Công khai**.
- Xóa bộ sưu tập hoặc bỏ membership không xóa bài. Đổi audience bộ sưu tập không đổi audience bài.

## 2. Phép AND

```text
can_appear_in_collection(viewer, post, collection)
  = membership_exists(post, collection)
    AND can_read_collection(viewer, collection)
    AND can_read_post(viewer, post)
```

`can_read_post` là quyền đọc **hiện tại** của bài, do owner xuất bản quyết định (audience, kết quả
kiểm duyệt, vòng đời): không phải một boolean `public` chép sang bảng bộ sưu tập.

Với người khác và khách:

| Bộ sưu tập | Bài | Hiển thị trong bộ sưu tập | Bài đọc độc lập | Lộ tên/ID/membership bộ sưu tập |
| --- | --- | --- | --- | --- |
| Công khai | Công khai, đọc được | **Có** | Có | Có (đúng ý tác giả) |
| Công khai | Chỉ mình tôi / chờ duyệt / đã xóa | Không | Không | Bộ sưu tập thấy được, bài này thì không; số đếm không tính nó |
| Chỉ mình tôi | Công khai | Không | **Có** | **Không** |
| Chỉ mình tôi | Chỉ mình tôi | Không | Không | Không |

Chủ sở hữu đọc bộ sưu tập và bài riêng của mình qua **đường owner**, không qua projection công
khai hay cache dùng chung. Bài đã xóa không vào danh sách hay link của ai, kể cả chủ (đề xuất).

## 3. Bề mặt dẫn xuất

Mọi bề mặt tính từ cùng phép AND, cho đúng người xem:

| Bề mặt | Luật |
| --- | --- |
| Danh sách bài của bộ sưu tập | Đúng tập AND, theo thứ tự tác giả sắp |
| Số bài | = độ dài danh sách người xem thấy; không lộ số bài ẩn |
| Tên bộ sưu tập trên trang bài | Chỉ bộ sưu tập mà người xem đọc được **và** bài này hiện trong đó |
| Link bài trước/sau/liên quan | Chỉ tới bài qua được cùng phép kiểm, qua bộ sưu tập người xem đọc được |
| Phân trang | Cursor chỉ là **vị trí** (bài cuối đã thấy); mỗi trang quyết định lại theo quyền của lần đọc đó. Bài cursor rời bộ sưu tập → `cursor_invalid`, không đoán |

## 4. Lệnh (chỉ chủ sở hữu)

| Lệnh | Từ chối khi | Ghi chú |
| --- | --- | --- |
| Tạo bộ sưu tập | — | Mặc định “Chỉ mình tôi” (đề xuất) |
| Đổi audience | Không phải chủ → không tìm thấy | Không chạm bài |
| Thêm bài | Bài của người khác → `not_own_post`; bài đã xóa; đã là thành viên → `already_member` (idempotent) | Thêm vào cuối (O-02) |
| Bỏ bài | Không phải thành viên → `not_member` | Bài không đổi |
| Xóa bộ sưu tập | — | Không xóa bài nào |

Người khác không phân biệt được “bộ sưu tập riêng tư” với “không tồn tại”.

## 5. Đề xuất chưa phải quyết định

| Điểm | Đề xuất | Trạng thái |
| --- | --- | --- |
| Một bài thuộc nhiều bộ sưu tập | Có | O-01 |
| Thứ tự | Tác giả sắp; mặc định theo thứ tự thêm | O-02 |
| Dữ liệu | Tên ≤ 80 ký tự (không cần duy nhất), mô tả tùy chọn, audience, thời điểm tạo; membership (bộ sưu tập, bài, vị trí) duy nhất theo cặp | `DESIGN_PROPOSED` |
| Bộ sưu tập công khai không có bài nào người xem đọc được | Vẫn mở được, hiện trạng thái rỗng thật, số đếm 0 | `DESIGN_PROPOSED` |
| Bài bị xóa còn hàng membership | Giữ hàng, không hiển thị; dọn cùng chính sách purge (O-08) | `DESIGN_PROPOSED` |

## 6. Bằng chứng và gap

`collections.qnt`: 12 invariant (`closedCollectionLeaksNothing`, `listIsExactlyTheConjunction`,
`countsMatchTheList`, `badgesNeedAllThree`, `linksAreReadable`, `servedIsReadableNow`,
`collectionChangesKeepPosts`…), scenario bảng chân trị 36 bước và negative control đổi AND thành
OR (khách thấy bài công khai nằm trong bộ sưu tập riêng tư). Chi tiết:
[README target](../../services/vink-app/behavior/target/README.md),
[registry](../development/product-decisions-registry.md). Chưa có: schema, API, UI, kiểm quyền thật
tại PostgreSQL, cache/SSR.
