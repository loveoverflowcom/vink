# Bản đồ miền Vink và điểm cần lưu ý khi định hình (`vink-shape`)

Dùng các ID dưới đây để tra tại [kho TOML](../../../../decisions/README.md) ở đầu công việc.
Phần tóm tắt cung cấp bối cảnh để đặt câu hỏi. CHOICE của lượt 10/11 có ID đầy đủ dạng
`PRODUCT-DECISIONS-20261110-CHOICE-NN`.

Tài liệu này chỉ dẫn thẳng tới nơi sở hữu từng quy tắc trong repo để tránh tạo bản sao thứ hai của
các bất biến (`INV-*`, `D-*`, `O-*`). Luôn đọc nguồn gốc trước khi kết luận một điểm là đã chốt hay
còn mở.

## 1. Xuất bản, audience và xóa bài / tombstone (`AUD`, `PUB`)

- **Nguồn sở hữu:** [PRD §14.1](../../../../prd/requirements.md#141-audience-xuất-bản-và-xóa-aud-pub),
  [xuất bản và kiểm duyệt](../../../../docs/architecture/publication-moderation.md),
  [DecisionDelta](../../../../docs/architecture/product-decisions-20261110.md),
  [publication-core](../../../../docs/development/publication-core.md),
  [publication-runtime](../../../../docs/development/publication-runtime.md).
- **Đã chốt (`PRODUCT_CONFIRMED`):**
  - Đúng hai audience: **Chỉ mình tôi** và **Công khai** (`AUD-01`).
  - Audience tách khỏi trạng thái kiểm duyệt; `Approved` không tự công khai bài đang “Chỉ mình tôi”;
    ý định công khai gắn đúng revision/generation (`AUD-02`, `PUB-01/02`, `INV-01/02`).
  - Bốn trường hợp tách biệt (`PUB-04/05`): **A** tác giả xóa bài từng công khai (hiện trang thay thế
    “Nội dung đã được tác giả xoá” + giữ bình luận đã duyệt); **B** tác giả chuyển riêng tư (`404`,
    ẩn cả thảo luận cho tới khi công khai lại); **C** admin/kiểm duyệt gỡ (`404`); **D** chưa từng
    công khai rồi xóa (`404`).
- **Điểm còn `OPEN` cần hỏi khi chạm tới:**
  - `O-03`: Quyền ghi trên trang tombstone (bình luận/phản hồi mới, “Hữu ích” mới, tự gỡ bình luận).
    *Lưu ý:* `domain::publication` (`#157`) đã cài baseline kỹ thuật (chặn bình luận mới, cho phép
    người viết tự gỡ), nhưng chủ sản phẩm chưa chốt `O-03` khi mở API/UI tombstone.
  - `O-04`: Bình luận đang chờ duyệt lúc tác giả xóa bài công khai có tiếp tục được duyệt để hiện
    trên tombstone hay đóng băng?
  - `O-05`: Bài từng công khai, chuyển sang “Chỉ mình tôi” rồi mới xóa có để lại tombstone không?
    (Baseline hiện tại: không — `DeletionProvenance::Hidden`).
  - `O-08`, `O-09`, `O-10`: Purge vật lý, thông tin hiển thị trên tombstone ngoài câu thông báo, và
    khả năng khôi phục bài đã xóa/gỡ.

## 2. Kiểm duyệt AI, quản trị viên và phản đối (`MOD-01..10`)

- **Nguồn sở hữu:** [PRD §14.2](../../../../prd/requirements.md#142-kiểm-duyệt-bằng-trợ-lý-ai-quản-trị-viên-và-phản-đối-mod-0710),
  [publication-moderation.md](../../../../docs/architecture/publication-moderation.md),
  [moderation-core](../../../../docs/development/moderation-core.md),
  [discussion-post-review](../../../../docs/development/discussion-post-review.md).
- **Đã chốt (`PRODUCT_CONFIRMED`):**
  - Trợ lý AI là luồng kiểm duyệt mặc định cho cả bài viết và bình luận; quản trị viên đứng trên AI
    và quyết định override trên phiên bản đang xét thắng callback AI đến muộn (`MOD-07..09`, `INV-03`).
  - Lưu nháp riêng không bao giờ gửi dữ liệu sang AI (`MOD-05`, `INV-08`).
  - Timeout, lỗi provider hoặc output sai định dạng không bao giờ tự duyệt (`MOD-08`); không tự cam
    kết SLA 24/48 giờ và không báo “đã gửi cho quản trị viên” khi kênh thông báo chưa kết nối (`MOD-10`).
- **Điểm cần làm rõ khi mở runtime/UI/provider:**
  - Dữ liệu nào được gửi tới AI provider (chỉ snapshot gửi duyệt, loại bỏ thông tin định danh nào,
    chính sách lưu/xóa bên ngoài).
  - Trần retry, xử lý hàng đợi `NeedsAdmin`, và `O-15` (quyết định của admin trong luồng phản đối).

## 3. Thảo luận hai tầng, sửa bình luận và ghi nhận văn học (`COM`, `LVE`)

- **Nguồn sở hữu:** [PRD §12](../../../../prd/requirements.md#12-literary-endorsement-lve),
  [PRD §14.3](../../../../prd/requirements.md#143-bình-luận-com-09-com-10),
  [discussion-ownership](../../../../docs/architecture/discussion-ownership.md),
  [comment-editing](../../../../docs/architecture/comment-editing.md),
  [literary-endorsement](../../../../docs/architecture/literary-endorsement.md),
  [endorsement-core](../../../../docs/development/endorsement-core.md).
- **Đã chốt (`PRODUCT_CONFIRMED`):**
  - Bình luận hai tầng (gốc + phản hồi phẳng) do Vink sở hữu trong PostgreSQL; tác giả bài bật/tắt
    nhận bình luận nhưng không sửa/xóa lời người khác (`COM-07/08`).
  - Người viết sửa bình luận của mình qua kiểm duyệt, không ghi đè chữ đang công khai trong lúc chờ,
    có dấu “Đã sửa” khi công khai, lưu không đổi chữ không tạo dấu (`COM-10`).
- **Điểm còn `OPEN` / `DESIGN_PROPOSED` cần lưu ý:**
  - `CHOICE-01` (giữ bản công khai cũ trong lúc chờ duyệt bản sửa) và `O-16` (hủy bản sửa đang chờ).
  - `O-06`: Neo “Hữu ích” theo revision bình luận (`LVE-FR-04` đề xuất) so với code hiện tại đang neo
    theo `CommentId` (vì bình luận hiện chưa cho sửa).
  - `O-07`: Ảnh hưởng của việc đăng revision bài viết mới lên điểm xếp hạng dựa trên “Đánh giá cao”.

## 4. Tài khoản và xác thực (`ACC-03`, `D-05`)

- **Nguồn sở hữu:** [PRD §14.6](../../../../prd/requirements.md#146-tài-khoản-acc-03),
  [ADR D-05 §12](../../../../docs/architecture/auth-authority.md#12-đổi-hướng-10112026-username--mật-khẩu-là-chính),
  [auth-web](../../../../docs/development/auth-web.md),
  [`services/vink-identity/src/password.rs`](../../../../services/vink-identity/src/password.rs).
- **Đã chốt (`PRODUCT_CONFIRMED`):**
  - Username + mật khẩu là phương thức đăng nhập chính; username khác bút danh công khai (`ACC-03`).
  - `O-12@r2` (11/11/2026): mật khẩu chuẩn hóa NFKC, 10–128 ký tự, mỗi code point một ký tự;
    `Password::parse` đã theo. Copy lỗi trên UI phải nói “ký tự”, không nói byte.
- **Điểm cần làm rõ khi làm UI/policy:**
  - `O-11`: Luồng khôi phục tài khoản qua email đã xác minh (tùy chọn) và cách chuyển tiếp tài khoản
    chỉ-email trong môi trường thử nghiệm.

## 5. Bộ sưu tập, phân loại, hashtag và khám phá (`COL`, `TAG`, `HTG`, `RANK`, `TOP`)

- **Nguồn sở hữu:** [PRD §13](../../../../prd/requirements.md#13-phân-loại-tác-phẩm-tag),
  [PRD §14.4–14.5](../../../../prd/requirements.md#144-bộ-sưu-tập-cá-nhân-col),
  [collections](../../../../docs/architecture/collections.md),
  [work-taxonomy](../../../../docs/architecture/work-taxonomy.md),
  [hashtags](../../../../docs/architecture/hashtags.md),
  [discovery-ranking](../../../../docs/architecture/discovery-ranking.md).
- **Đã chốt (`PRODUCT_CONFIRMED`):**
  - Bộ sưu tập cá nhân cho bài của chính tác giả, hai audience, hiển thị là phép **AND** của
    `membership ∧ can_read_collection ∧ can_read_post`, không lộ bộ sưu tập riêng tư trên trang bài
    công khai (`COL-01..06`).
  - Hashtag tách biệt hoàn toàn với catalog dạng bài/chủ đề do curator quản lý; chỉ index hashtag của
    bài đang công khai (`HTG-01..05`).
  - `Đề xuất` kết hợp “Đánh giá cao” và độ mới; `Top 100` lọc trước rồi lấy tối đa 100 bài theo
    “Đánh giá cao” giảm dần (`RANK-01..04`, `TOP-01..04`).
- **Điểm còn `OPEN` / `DESIGN_PROPOSED`:**
  - `O-01` (một bài thuộc một hay nhiều bộ sưu tập), `O-02` (thứ tự bài trong bộ sưu tập).
  - `CHOICE-04` (trích hashtag từ nội dung vs trường riêng) và `O-13` (trần 10 hashtag/bài: bỏ qua phần
    dư hay báo lỗi khi gửi).
  - `CHOICE-02` / `O-14` (công thức `recommend.v1`, tham số `H`, mốc thời gian độ mới) và `CHOICE-03`
    (cơ chế snapshot cursor của `Top 100`).
