# Đề xuất và Top 100 — đặc tả đích

**Ngày:** 10/11/2026 · **Nguồn:** `PRODUCT-DECISIONS-20261110`
([DecisionDelta](product-decisions-20261110.md)), USER-12/13 · **Yêu cầu:** RANK-01..04, TOP-01..04,
UX-02, LVE-FR-05/06 · **Quyết định:** D-14, D-18.

**Trạng thái:** `TARGET_BEHAVIOR`, **chưa có implementation**. Chủ sản phẩm đã chốt *có* hai chế độ
và các ràng buộc ở mục 2; **công thức, trọng số, cửa sổ thời gian và timestamp chưa được chốt**.
`recommend.v1` và cách phân trang dưới đây là `DESIGN_PROPOSED` để đặc tả và kiểm thử được, không
phải tuning production. Đặc tả thực thi:
[`discovery.qnt`](../../services/vink-app/behavior/target/discovery.qnt).

## 1. Hiện trạng (`CURRENT_IMPLEMENTATION`)

`GET /api/v1/works` sắp theo `published_at DESC, work_id` với keyset trên một index, lọc theo chữ
và thẻ catalog ([discovery](../development/discovery-api.md)). Không có ranking, đề xuất hay top-N;
`WorkCard` không mang số vote; tổng “Đánh giá cao” được đếm khi đọc, chỉ cho revision đang công
khai (`persistence/endorsement.rs`).

## 2. Luật đã chốt

| Luật | Đề xuất | Top 100 |
| --- | --- | --- |
| Tập ứng viên | Bài đang đọc công khai được (mục 3) | Như bên trái, **sau đó** áp bộ lọc |
| Khóa xếp hạng | Vote hợp lệ **kết hợp** độ mới | Vote hợp lệ giảm dần; độ mới **chỉ** là tie-breaker |
| Giới hạn | Không | Tối đa 100 bài của tập kết quả, phân trang bên trong |
| Vote | “Đánh giá cao” của **bài**. Không dùng “Hữu ích” của bình luận; không dùng vote của nội dung không đủ điều kiện công khai | Như bên trái |
| Không được | Ưu tiên theo tác giả, độ dài bài, “điểm văn học AI”; sửa thông thường để bump | Lấy 100 bài toàn cục rồi mới lọc |
| Đơn điệu | Cùng tuổi: thêm vote không làm điểm giảm. Cùng vote: bài mới hơn không bị điểm thấp hơn. Tie-breaker ổn định | Tie-breaker xác định |
| Quyền | Mỗi lần đọc kiểm quyền hiện hành | Snapshot xếp hạng không vượt quyền đọc hiện hành |

## 3. Tập đủ điều kiện và đầu vào

- **Đủ điều kiện** = đúng điều kiện đọc công khai của bài tại lúc đọc: vòng đời đang hoạt động,
  audience công khai, có bản công khai đã được duyệt
  ([đặc tả xuất bản](publication-moderation.md#3-bốn-trục-độc-lập)). Bài riêng tư, chờ duyệt, đã
  xóa (kể cả tombstone) hay bị gỡ không bao giờ là ứng viên.
- **Vote hợp lệ** = tổng công khai của LVE cho **revision công khai hiện tại** của bài, đúng con
  số người đọc thấy. Bài đăng revision mới thì count bắt đầu lại theo luật LVE hiện hành (O-07).
- **Độ mới** (đề xuất, O-14) = thời điểm bài công khai **lần đầu** (`first_published_at`). Revision
  mới không đổi mốc này, nên sửa không thể bump.

## 4. CHOICE-02: điểm Đề xuất

Thời gian luôn là **đầu vào** của hàm thuần (`now`), không đọc đồng hồ ẩn. Tham số có tên version
và bounds; ví dụ dưới dùng số minh họa.

**A — log vote, suy giảm tuyến tính theo tuổi (`recommend.v1`, được chọn):**

```text
score(votes, first_published_at, now) = H × ⌊log2(1 + votes)⌋ − (now − first_published_at)
```

Mỗi lần `(1 + votes)` gấp đôi đáng giá `H` đơn vị thời gian. Vì `now` trừ như nhau ở mọi bài, thứ
tự bằng thứ tự của khóa `first_published_at + H × ⌊log2(1 + votes)⌋`: **không phụ thuộc đồng hồ**,
nên lưu/index được và cursor không trôi chỉ vì thời gian qua.

**B — tổng đã chuẩn hóa trong cửa sổ (bị loại):**

```text
score = w_v × min(votes, V) / V + w_r × max(0, 1 − tuổi / W)
```

Corpus minh họa (A: `H = 24h`; B: `W = 168h`, `V = 50`, `w_v = w_r = 0,5`):

| Bài | Tuổi | Vote hợp lệ | A | Hạng A | B | Hạng B |
| --- | --- | --- | --- | --- | --- | --- |
| P3 mới, nhiều vote | 3h | 12 | 72 − 3 = **69** | 1 | 0,611 | 2 |
| P2 cũ, nhiều vote | 72h | 40 | 120 − 72 = **48** | 2 | 0,686 | 1 |
| P1 mới, ít vote | 2h | 1 | 24 − 2 = **22** | 3 | 0,504 | 3 |
| P4 mới, zero-vote | 1h | 0 | **−1** | 4 | 0,497 | 4 |
| P5 hơn một ngày, zero-vote | 30h | 0 | **−30** | 5 | 0,411 | 5 |
| P6 rất cũ, nhiều vote | 200h | 40 | 120 − 200 = **−80** | 6 | 0,400 | 6 |

- **Unlike:** P1 mất vote duy nhất → A: −2, tụt xuống dưới P4 (−1); B: 0,494, cũng dưới P4.
- **Vote bị loại:** P3 còn 7 vote hợp lệ → A vẫn 69 (`⌊log2 8⌋ = 3`); còn 6 → `⌊log2 7⌋ = 2`, A = 45,
  xuống dưới P2. Bậc thang lũy thừa 2 là đánh đổi có chủ ý của A.
- **Khác nhau chính:** B giữ bài cũ nhiều vote ở mức sàn mãi mãi (P6 = 0,4, trên mọi bài zero-vote
  cũ hơn ~34 giờ) và xếp P2 trên P3; A để mọi bài chìm dần theo tuổi. B cần ba tham số + trần vote
  và thứ tự đổi khi bài ra khỏi cửa sổ; A cần một tham số.

**Chọn A** (`DESIGN_PROPOSED`). Cả hai thỏa các ràng buộc đơn điệu; A thêm tính “thứ tự không phụ
thuộc đồng hồ”. Tie-breaker: mới hơn trước, rồi ID lớn hơn trước. Không có thành phần tác giả, độ
dài hay điểm AI. Xem lại khi có dữ liệu thật hoặc khi chủ sản phẩm muốn dạng đường cong khác.

**Phân trang Đề xuất:** keyset theo (khóa, `first_published_at`, ID). Khóa đổi khi vote đổi, nên
**không hứa** không thiếu/không trùng giữa hai trang; client khử trùng theo ID; mỗi trang kiểm
quyền hiện hành.

## 5. Top 100 và CHOICE-03

Thứ tự `top.v1`: vote hợp lệ giảm dần → `first_published_at` mới hơn trước → ID lớn hơn trước.

**A — snapshot ứng viên hữu hạn (được chọn):** trang đầu tính, trong **một** lần đọc nhất quán,
danh sách có thứ tự của tối đa 100 ID thuộc `đủ điều kiện ∧ bộ lọc`, rồi trả trang đầu cùng cursor
mờ mang: tên policy, bộ lọc, thời điểm chụp, danh sách ID và vị trí. Không bảng snapshot, không
Redis: 100 ID là vài KB. Các trang sau đi theo **vị trí** trong danh sách đó và quyết định lại từng
bài theo quyền của lần đọc đó.

| Sự kiện giữa hai trang | Hành vi |
| --- | --- |
| Vote đổi, unlike | Thứ tự trong snapshot **không đổi**; số vote hiển thị là số hiện tại (có thể không còn giảm dần giữa các trang) |
| Bài bị xóa, chuyển riêng tư, bị gỡ, không còn khớp bộ lọc | Bài bị **bỏ khỏi trang**; không kéo bài thứ 101 vào thay; trang có thể ngắn hơn |
| Bài khác vừa đủ điều kiện hoặc vừa lọt top | Không xuất hiện cho tới khi người đọc làm mới |
| Cursor quá tuổi cho phép hoặc sai policy/bộ lọc | Tín hiệu `refresh_required` rõ; không tự nhảy về đầu |
| Tải trang kế | Không bài nào lặp trong cùng snapshot |

Cursor không phải authority: ID lạ hay bị sửa trong cursor vẫn phải qua kiểm quyền và bộ lọc hiện
hành; có thể ký (MAC) để tránh thứ tự tùy ý. Trần tuổi cursor do PR runtime chọn và đo.

**B — cursor hết hiệu lực khi ranking version đổi (bị loại):** đúng nhưng vote đổi liên tục nên
người đọc gần như luôn phải bắt đầu lại.

## 6. Bằng chứng và gap

`discovery.qnt` (cap = 3 thay cho 100, trang 2 bài): `servedIsReadableNow`,
`snapshotIsTopOfFilteredSet`, `topIgnoresRecency`, `noRepeatAcrossPages`, `recommendedIsOrdered`,
`moreVotesNeverLower`, `newerNeverLower`, `orderIgnoresClock`, `editNeverBumps`…; hai negative
control (chỉ kiểm quyền lúc chụp snapshot; lấy top toàn cục rồi mới lọc). Chi tiết:
[README target](../../services/vink-app/behavior/target/README.md),
[registry](../development/product-decisions-registry.md). Chưa có: cột/khóa xếp hạng, index, query
plan, API, cursor thật, UI; chưa có số đo với dữ liệu thật.
