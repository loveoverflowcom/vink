# Hashtag — đặc tả đích

**Ngày:** 10/11/2026 · **Nguồn:** `PRODUCT-DECISIONS-20261110`
([DecisionDelta](product-decisions-20261110.md)), USER-11 · **Yêu cầu:** HTG-01..06, UX-02,
TAG-FR-01..03 · **Quyết định:** D-15, D-17.

**Trạng thái:** `TARGET_BEHAVIOR`, **chưa có implementation**: không có hashtag tự do trong code;
chưa làm UI hashtag hay search engine mới trong lượt này. Luật “đã chốt” là `PRODUCT_CONFIRMED`;
grammar, giới hạn và cách lưu là `DESIGN_PROPOSED` (CHOICE-04 A). Không có model Quint riêng: phần
quyền của hashtag là hệ quả của “chỉ index bài đủ điều kiện” và được kiểm bằng unit + PostgreSQL
khi triển khai.

## 1. Hai hệ khác nhau

| | Thẻ catalog (hiện có, [TAG](work-taxonomy.md)) | Hashtag (mới) |
| --- | --- | --- |
| Ai tạo | Curator (lệnh operator), tác giả chỉ chọn | Tác giả tự viết |
| Giới hạn | Dạng bài ≤ 1, chủ đề ≤ 3 | **Không** áp giới hạn đó |
| Danh tính | ID bền + slug; alias/đổi tên/ngừng dùng do curator | Chính chuỗi chữ (mục 3) |
| Lưu ở | Thẻ nháp có version → snapshot metadata bất biến | Trong nội dung của revision |
| Dùng để | Facet có kiểm soát | Click/lọc/tìm như mạng xã hội |

Không tự chuyển catalog thành hashtag hay ngược lại. Hashtag trùng chữ với slug catalog **không**
trở thành thẻ catalog; hai tham số lọc tách nhau (`tag=` hiện có, `hashtag=` mới).

## 2. CHOICE-04: nhận diện

**A — trích từ nội dung theo grammar (được chọn).** Hashtag là chữ tác giả viết trong nội dung
bài. Nguồn sự thật duy nhất là văn bản của **revision bất biến**: hashtag được kiểm duyệt cùng nội
dung, có cùng digest và không cần store riêng phải đồng bộ. Index công khai là projection dẫn xuất
từ revision đang công khai.

**B — danh sách có cấu trúc gắn với bài (bị loại):** gọn cho UI nhưng là chữ tự do cần version và
kiểm duyệt riêng, dễ lẫn với thẻ catalog trong cùng mục “Thông tin tác phẩm”, và tạo hệ thứ hai
phải đồng bộ với nội dung. Không triển khai cả hai rồi đồng bộ bằng suy đoán.

Đối chiếu hiện trạng: nội dung là `Document` gồm các block văn bản thuần (đoạn văn, thơ), không
phải Markdown, nên `#` không có nghĩa nào khác; tiêu đề **không** được trích hashtag.

## 3. Grammar, nhãn và danh tính (đề xuất)

- **Nhận diện:** `#` đứng đầu block hoặc sau khoảng trắng/dấu mở ngoặc, theo sau là 1–50 ký tự
  thuộc chữ cái Unicode (kèm dấu kết hợp), chữ số hoặc `_`, trong đó có ít nhất một chữ cái.
  Hashtag kết thúc ở ký tự đầu tiên ngoài tập đó. `#123`, `a#b`, `##` không phải hashtag.
- **Chuẩn hóa:** văn bản đã NFC từ lúc lưu; NFC và NFD cho cùng hashtag.
- **Danh tính:** chuỗi NFC sau khi gập hoa/thường, **giữ dấu**. `#ThơTình` và `#thơtình` là một;
  `#thơ` và `#tho` là **hai** hashtag khác nhau.
- **Nhãn hiển thị:** đúng như tác giả viết trong bài đó.
- **Khóa tìm (bỏ dấu):** chỉ để tìm **ứng viên**: gõ `tho` gợi ý được `#thơ`, `#thở`, `#tho`; bản
  khớp dấu chính xác xếp trước. Chung khóa tìm không bao giờ gộp danh tính (cùng nguyên tắc với
  `SearchKey` của TAG-FR-03).
- **Giới hạn (O-13):** 10 hashtag khác nhau đầu tiên theo thứ tự xuất hiện được index; phần còn
  lại vẫn là chữ thường trong bài.

## 4. Phạm vi index và cập nhật

| Sự kiện | Index công khai |
| --- | --- |
| Lưu nháp, lưu bản riêng, bản đang chờ duyệt, bản bị từ chối | **Không** ghi gì |
| Bản r trở thành bản công khai | Thay toàn bộ hashtag của bài bằng hashtag của r, trong cùng transaction với pointer |
| Chuyển “Chỉ mình tôi”, tác giả xóa, kiểm duyệt gỡ | Xóa hashtag của bài khỏi index trong cùng transaction |
| Đăng revision mới | Như dòng “bản r trở thành bản công khai” |

Autocomplete, số bài theo hashtag và trang hashtag **chỉ** đọc index này và vẫn kiểm quyền đọc
hiện hành của từng bài khi trả kết quả, nên hashtag chỉ có trong bài riêng tư không thể xuất hiện
ở gợi ý, count hay feed. Tombstone không mang hashtag.

Lọc: `hashtag=<nhãn>` trên danh sách công khai, kết hợp AND với `q` và `tag` hiện có; Đề xuất và
Top 100 áp bộ lọc này **trước** khi xếp hạng ([Đề xuất và Top 100](discovery-ranking.md)).

## 5. Chưa quyết định / ngoài phạm vi

Hashtag trong bình luận; danh sách chặn hashtag; trang “hashtag thịnh hành”; đổi tên/gộp hashtag;
hashtag có cấu trúc trong editor (chip, gợi ý khi gõ). UI và search engine mới: chưa làm.

## 6. Gap

Chưa có: hàm trích thuần và test Unicode (NFC/NFD, hoa/thường có dấu, ranh giới từ, block thơ),
bảng projection + migration, ghi trong transaction publish (owner xuất bản chưa tồn tại), tham số
lọc, UI. Phụ thuộc PR xuất bản (kế hoạch ở [DecisionDelta §6.2](product-decisions-20261110.md#62-thứ-tự-pr-runtime-đề-xuất)).
