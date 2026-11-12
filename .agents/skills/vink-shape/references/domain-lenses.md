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
  - Bốn trường hợp xuất bản/tombstone (`PUB-04/05`, `ADM-07/08`):
    - **A** tác giả xóa bài từng công khai: hiện trang thay thế “Nội dung đã được tác giả xoá” + giữ bình luận đủ điều kiện; chặn bình luận/đánh giá mới, cho phép tự gỡ bình luận cũ (`O-03@r2`).
    - **B** tác giả chuyển riêng tư: dùng trang “Nội dung đã được ẩn” (`AuthorHidden`), giữ bình luận đủ điều kiện; chặn ghi mới; xóa sau khi ẩn vẫn giữ bình luận qua URL cũ (`O-05@r2`). *(Lưu ý khoảng cách runtime: loader `#158` hiện vẫn build `hidden = None` nên `set-private` trả not found/404 ở HTTP cho tới khi hoàn thiện loader).*
    - **C** admin/kiểm duyệt gỡ: trả `404` (`PUB-06/07`, `WorkAccessPlan::ModerationRemoved`).
    - **D** chưa từng công khai rồi xóa: trả `404`.
- **Điểm còn `OPEN` cần hỏi khi chạm tới:**
  - `O-04`: Bình luận đang chờ duyệt lúc tác giả xóa bài công khai có tiếp tục được duyệt để hiện
    trên tombstone hay đóng băng?
  - `O-08`, `O-09`, `O-10`: Purge vật lý, thông tin hiển thị trên tombstone ngoài câu thông báo, và
    khả năng khôi phục bài đã xóa/gỡ.
  - `O-17`: Chính sách khiếu nại hoặc xem lại đối với tác phẩm bị ẩn/gỡ do vi phạm.

## 2. Kiểm duyệt AI, quản trị viên và phản đối (`MOD-01..15`, `ADM-01..15`)

- **Nguồn sở hữu:** [PRD §14.2](../../../../prd/requirements.md#142-kiểm-duyệt-bằng-trợ-lý-ai-quản-trị-viên-và-phản-đối-mod-0710),
  [publication-moderation.md](../../../../docs/architecture/publication-moderation.md),
  [moderation-core](../../../../docs/development/moderation-core.md),
  [decision-log-20261111](../../../../decisions/decision-log-20261111.toml),
  [discussion-post-review](../../../../docs/development/discussion-post-review.md).
- **Đã chốt (`PRODUCT_CONFIRMED` / core governance):**
  - Trợ lý AI là luồng kiểm duyệt mặc định cho cả bài viết và bình luận; quản trị viên đứng trên AI
    và quyết định override trên phiên bản đang xét thắng callback AI đến muộn (`MOD-07..09`, `INV-03`).
  - Lưu nháp riêng không bao giờ gửi dữ liệu sang AI (`MOD-05`, `INV-08`).
  - Timeout, lỗi provider hoặc output sai định dạng không bao giờ tự duyệt (`MOD-08`); không tự cam
    kết SLA 24/48 giờ và không báo “đã gửi cho quản trị viên” khi kênh thông báo chưa kết nối (`MOD-10`).
  - Quản trị viên can thiệp (`ADM-01..15`): override AI, ẩn/khôi phục bình luận (`COM-11`), gỡ bài vi phạm (`PUB-06/07`),
    quản lý ca điều tra và khiếu nại (đã cài đặt ở pure core `domain::moderation`, chưa có store/worker/UI).
- **Điểm cần làm rõ khi mở runtime/UI/provider (`OPEN`):**
  - Dữ liệu nào được gửi tới AI provider (chỉ snapshot gửi duyệt, loại bỏ thông tin định danh nào,
    chính sách lưu/xóa bên ngoài).
  - Trần retry, xử lý hàng đợi `NeedsAdmin`, `O-15@r2` (khiếu nại trực tiếp lên admin khi AI từ chối),
    và `O-17..19` (quy trình điều tra, ẩn tác giả vi phạm nhiều lần, escalation).

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
  [`services/vink-identity/src/password.rs`](../../../../services/vink-identity/src/password.rs),
  [`services/vink-app/contracts/http/v1/README.md`](../../../../services/vink-app/contracts/http/v1/README.md).
- **Đã chốt (`PRODUCT_CONFIRMED` / backend runtime):**
  - Username + mật khẩu là phương thức đăng nhập chính; username khác bút danh công khai (`ACC-03`).
  - `O-12@r2` (11/11/2026): mật khẩu chuẩn hóa NFKC, 10–128 ký tự, mỗi code point một ký tự;
    `Password::parse` đã theo. Copy lỗi trên UI phải nói “ký tự”, không nói byte.
  - Sau `#168`, tài khoản mã email đã xác minh có thể bổ sung username/mật khẩu trên cùng `(authority, subject)`
    qua endpoint backend (`POST /api/v1/auth/password/enroll/challenges`, `POST /api/v1/auth/password/enroll`,
    `GET /api/v1/auth/password/enroll`) với challenge proof tươi, giữ nguyên principal/bút danh/quyền hiện hữu.
- **Điểm cần làm rõ khi làm UI/policy (`OPEN` / Gaps):**
  - `O-11`: Luồng khôi phục tài khoản qua email đã xác minh (tùy chọn) vẫn còn `OPEN` và độc lập với enrollment đã có.
  - UI Web/CMP cho đăng nhập/đăng ký/enroll mật khẩu chưa có; production sign-in unlock (`VINK_AUTH_SIGN_IN` mặc định disabled) vẫn là khoảng cách runtime.

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
