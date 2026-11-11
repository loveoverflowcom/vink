# Quyết định sản phẩm ngày 10/11/2026 — DecisionDelta và kế hoạch

**ID:** `PRODUCT-DECISIONS-20261110` · **Ngày:** 10/11/2026 · **Base thực thi:**
`origin/develop@f3af6005b09372cf699953c93f88e116a23e5466` · **Phạm vi:** tài liệu canonical, đặc
tả Quint và test vectors. **Không có runtime nào được triển khai trong lượt này**: không migration,
endpoint, UI, provider AI, email hay cấu hình production.

Chủ sản phẩm đã chốt 15 quyết định nghiệp vụ (`USER-01..15` trong prompt giao việc). Tài liệu này
là bản ghi của lượt đó: ánh xạ từng quyết định vào ID canonical, ghi luật cũ bị thay, hiện trạng
code, hành vi đích đã được mô hình hóa và phần còn mở. Các owner hiện hữu vẫn là nguồn chính:
[requirements](../../prd/requirements.md) giữ ID và acceptance, [decision log](decisions.md) giữ
D-ID; file này không thay chúng.

**Tra cứu sau #164.** [Kho quyết định TOML](../../decisions/README.md) sở hữu record của §2–4.
Giữ các bảng bên dưới làm snapshot 10/11, gồm hiện trạng code ghi ở thời điểm đó. Tra mục
hiện hành, lịch sử và đề xuất bằng `cargo xtask decision query`.

<!-- vdr: historical USER-08@r1 -->

## 1. Cách đọc trạng thái

| Nhãn | Nghĩa | Không có nghĩa |
| --- | --- | --- |
| `PRODUCT_CONFIRMED` | Chủ sản phẩm đã chọn hướng/ngữ nghĩa (10/11/2026) | ADR Accepted, production sẵn sàng |
| `DESIGN_PROPOSED` | Chi tiết do agent đề xuất để đặc tả/kiểm thử được | Lời chốt của chủ sản phẩm |
| `IMPLEMENTED` | Có code thật tại source được chỉ ra | Đã kiểm chứng ở mọi boundary |
| `VERIFIED` | Có lệnh đã chạy, gắn SHA và phạm vi | Chứng minh ngoài phạm vi đó |
| `DEFERRED` | Chủ động chưa làm ở giai đoạn này | Đã quyết định theo hướng nào |
| `OPEN` | Chưa đủ quyết định; có đề xuất tối thiểu | Chặn phần độc lập khác |

Mọi hành vi đích trong lượt này có trạng thái implementation **NOT_IMPLEMENTED**. Model Quint ở
[`behavior/target`](../../services/vink-app/behavior/target/README.md) chạy được nhưng không có
hàm Rust nào được replay; bảng yêu cầu → model → gap nằm ở
[registry](../development/product-decisions-registry.md).

## 2. DecisionDelta

| Quyết định | ID canonical | Luật cũ | Luật mới (`PRODUCT_CONFIRMED`) | Model / source bị ảnh hưởng | Hiện trạng code |
| --- | --- | --- | --- | --- | --- |
| USER-01 Chưa xử lý nhóm tuổi | D-11; vision §2, §12 | “Nhóm người trưởng thành được mời” là phương án giảm phạm vi (đề xuất) | `DEFERRED`. Không code/thiết kế age gate, ngày sinh, parental consent. Hoãn **không** phải quyết định mở cho mọi độ tuổi | Không model | Không có age gate, ngày sinh hay cổng mời trong code |
| USER-02 Hai mức riêng tư | D-06; AUD-01, AUD-02; PUB-01 | “Nháp riêng + public publication”, unlisted/nhóm kín “để sau” | Đúng hai audience: **Chỉ mình tôi** / **Công khai**. Audience tách khỏi trạng thái kiểm duyệt; công khai không bỏ kiểm duyệt; Approved không tự công khai bài chỉ lưu riêng | `publication.qnt` | `publications` chỉ là “có hàng hay không”; không có cột audience, không có lệnh publish |
| USER-03 Kiểm duyệt bài bằng trợ lý AI, hiện dùng mock | D-07; MOD-07; MOD-02, MOD-05 | Pilot: AI chỉ tư vấn, người duyệt quyết định | AI **có thể tự quyết định duyệt** theo policy. Lượt này chỉ đặc tả contract + mock scripted; không provider thật | `publication.qnt`, `evaluator-vectors.json` | Không có evaluator, port, job hay mock cho tác phẩm (`ai_moderation: not_implemented`) |
| USER-04 Bình luận cũng do AI kiểm duyệt | D-08; COM-02, COM-09 | Tiền kiểm bởi người có grant; ứng viên đăng-trước chỉ cho “người đáng tin” | Gốc và phản hồi đi qua evaluator: gửi → chờ đánh giá → kết quả hợp lệ → áp quyết định. Bỏ yêu cầu “chỉ người đáng tin mới được AI duyệt” khỏi phần đã chốt | `comment_edit.qnt` (cùng luồng quyết định) | `premoderation.v1` là policy hiệu lực duy nhất; `trusted-cohort.v1` chỉ shadow; classifier chỉ là một ô kiểu dữ liệu |
| USER-05 AI là luồng mặc định, không tự đặt SLA | D-07; MOD-03, MOD-08 | “Human review được vận hành”, “khung xử lý được thông báo” | Không mặc định có người trực. Timeout/lỗi/output hỏng/không đủ căn cứ: **không công khai**, giữ retry được hoặc “cần admin”. Không cam kết 24/48 giờ | `publication.qnt`, `appeal.qnt` | Chưa có |
| USER-06 Phản đối AI chuyển về admin, chưa gửi thật | D-19; MOD-04, MOD-10 | Appeal tới moderator, có màn vận hành | Đích xử lý cuối là chủ sản phẩm (admin). Lượt này chỉ đặc tả case + dialog; không gửi mail, không inbox. Phân biệt: mở dialog / tạo case / gửi / admin nhận / admin xử lý | `appeal.qnt` | Không có appeal, case hay admin |
| USER-07 Tác giả xóa bài: trang thay thế + bình luận qua link cũ | D-12; PUB-03, PUB-04, PUB-05 | Rút bài chặn đọc/ghi cả thảo luận cùng lúc | Tách A (tác giả xóa bài từng công khai: tombstone + bình luận hợp lệ), B (chuyển riêng tư), C (kiểm duyệt gỡ), D (chưa từng công khai). Ngoại lệ chỉ cho A | `tombstone.qnt` | Không có xóa bài. Work không có publication → thảo luận `404` |
| USER-08 Username + mật khẩu là chính | D-05; ACC-03 | Đề xuất A: mã email một lần (Provisional) | Username + password là đăng nhập chính; mã email có thể bổ sung sau. Username ≠ bút danh | Không model (auth thuộc D-05) | **Chỉ có mã email**; không có username, password hash hay Argon2 |
| USER-09 Bài ngắn và bộ sưu tập cá nhân | D-10, D-16; WR-04; COL-01..06 | Một phần trước, nhiều chương cơ bản ở beta | Mạng xã hội bài ngắn/tản văn; **không** nhiều chương. Bộ sưu tập cá nhân (hai audience), hiển thị = AND ba điều kiện | `collections.qnt` | Không có khái niệm bộ sưu tập |
| USER-10 Chưa có Tạp Chí | D-09; ED-02; vision §3 | Tạp Chí là một trong ba không gian, có acceptance pilot | `DEFERRED`, giữ lịch sử; không còn là gate của mạng xã hội bài ngắn | Không model | Không có curation |
| USER-11 Hashtag để lọc | D-15, D-17; HTG-01..06 | Chỉ có catalog curated (dạng bài ≤ 1, chủ đề ≤ 3) | Hashtag do tác giả dùng, để click/lọc/tìm; tách khỏi catalog curated; không index hashtag riêng tư | Không model (đặc tả văn bản) | Không có hashtag tự do |
| USER-12 Đề xuất = đánh giá cao + gần đây | D-14, D-18; RANK-01..04 | Ranking OPEN; “hệ recommendation riêng” là Later | Một chính sách kết hợp vote hợp lệ và độ mới. Công thức/trọng số **chưa chốt** | `discovery.qnt` | `GET /works` sắp theo `published_at`; không dùng count |
| USER-13 Top 100 theo đánh giá cao | D-18; TOP-01..04 | Không có | Chế độ riêng: lọc → sắp theo vote giảm dần → tối đa 100 → phân trang; quyền hiện hành thắng snapshot | `discovery.qnt` | Không có |
| USER-14 AI tự duyệt, admin cao hơn | D-19; MOD-09; INV-03 | Human override thắng callback | AI là actor dịch vụ có capability giới hạn; admin override trên phiên bản đang xét thắng callback AI muộn; admin không vượt privacy/audit | `publication.qnt`, `comment_edit.qnt`, `appeal.qnt` | Không có vai trò admin (chỉ grant moderator và operator CLI) |
| USER-15 Sửa bình luận công khai, có dấu đã sửa | D-08; COM-10; LVE-FR-04 | Chặn sửa bình luận trong pilot | Người viết sửa bình luận của mình; bản sửa có revision, đi qua kiểm duyệt; công khai có dấu “Đã sửa”; lưu không đổi nội dung không tạo dấu | `comment_edit.qnt` | Không có sửa, không có bảng revision body, không có cờ edited |

Luật **không** đổi và được giữ nguyên trong mọi tài liệu: Vink sở hữu nội dung và thảo luận bằng
Rust/PostgreSQL; hai tầng bình luận; quyền kiểm tại server; tách private/public; tác giả bật/tắt
nhận bình luận (COM-07); tác giả không kiểm duyệt hay sửa lời người khác (COM-08); không Discourse
runtime; AI output là dữ liệu không tin cậy (MOD-05); không lấy “văn hay/dở” làm điều kiện an toàn
(MOD-01).

## 3. Lựa chọn triển khai (`DESIGN_PROPOSED`)

Không lựa chọn nào dưới đây là lời chốt của chủ sản phẩm; mỗi dòng ghi điều kiện xem lại.

| CHOICE | Chọn | Loại | Đánh đổi | Xem lại khi |
| --- | --- | --- | --- | --- |
| 01 Bản sửa bình luận đang chờ duyệt | **A** — giữ bản công khai đã duyệt; bản sửa là candidate riêng. “Đã sửa” gắn bản đang hiển thị | B — tạm ẩn nội dung trong lúc duyệt sửa | A giữ mạch thảo luận nhưng người viết thấy chữ cũ lâu hơn nếu evaluator chậm; bản cũ có thể bị admin gỡ trong lúc chờ (model có nhánh này: không còn gì công khai cho tới khi candidate được duyệt) | Policy yêu cầu rút bản cũ ngay khi người viết sửa; hoặc đo thấy thời gian chờ làm người đọc trả lời nhầm bản cũ |
| 02 Điểm đề xuất | **A** — `recommend.v1`: `score = H × ⌊log2(1 + votes)⌋ − tuổi`; thứ tự tương đương khóa `first_published_at + H × ⌊log2(1 + votes)⌋` | B — tổng vote và độ mới đã chuẩn hóa trong một cửa sổ | A: một tham số `H`, thứ tự không phụ thuộc đồng hồ (cursor không trôi theo thời gian), không có “vách” cửa sổ; đổi lại bậc thang theo lũy thừa 2. B dễ giải thích theo phần trăm nhưng cần hai trọng số + cửa sổ + trần vote, và thứ tự đổi khi bài rơi khỏi cửa sổ | Có dữ liệu thật để tuning; chủ sản phẩm muốn bài cũ nhiều vote không bao giờ vượt bài mới |
| 03 Phân trang Top 100 | **A** — snapshot hữu hạn ≤ 100 ứng viên chụp ở trang đầu, mang trong cursor; mỗi lần đọc kiểm lại quyền hiện hành; hết hạn → tín hiệu làm mới | B — ranking version đổi thì cursor hết hiệu lực | A không lặp/thiếu trong snapshot dù vote đổi, không cần bảng snapshot hay Redis (100 ID ≈ 2 KB); đổi lại cursor lớn và bài mới đủ điều kiện chỉ thấy sau khi làm mới. B buộc bắt đầu lại gần như mọi lần vì vote đổi liên tục | Cursor vượt giới hạn URL/HTTP đã đo; cần Top theo nhiều bộ lọc với snapshot dùng chung |
| 04 Nhận diện hashtag | **A** — trích từ nội dung bản công khai theo grammar rõ; index là projection dẫn xuất theo revision | B — danh sách hashtag có cấu trúc gắn với bài | A: một nguồn sự thật (chữ của revision bất biến), được kiểm duyệt cùng nội dung, không thêm store riêng cần đồng bộ, không tạo hệ tag thứ hai cạnh catalog. B sạch hơn cho UI nhưng cần version/kiểm duyệt riêng cho chữ tự do và dễ lẫn với thẻ curated | Editor cần chip hashtag có cấu trúc; hoặc hashtag phải đổi được mà không tạo revision |

Corpus so sánh CHOICE-02 và grammar hashtag: [discovery-ranking](discovery-ranking.md),
[hashtags](hashtags.md).

## 4. Điểm còn `OPEN`

Mỗi dòng có đề xuất tối thiểu (baseline an toàn) đã dùng trong model; không dòng nào chặn phần
tài liệu/model khác.

| # | Câu hỏi | Đề xuất tối thiểu đang dùng | Tác động nếu đổi |
| --- | --- | --- | --- |
| O-01 | Một bài thuộc một hay nhiều bộ sưu tập? | Nhiều (membership là tập cặp bài–bộ sưu tập) | Chỉ-một cần ràng buộc unique và luật “chuyển” thay “thêm” |
| O-02 | Thứ tự bài trong bộ sưu tập | Thứ tự do tác giả đặt; thêm mới vào cuối | Sắp tự động theo thời gian bỏ được cột vị trí nhưng mất quyền sắp xếp |
| O-03 | Quyền ghi trên tombstone (bình luận/phản hồi mới, “Hữu ích” mới) | **Không ghi mới**; người viết vẫn gỡ được bình luận của mình; kiểm duyệt vẫn quyết định được | Mở ghi cần luật riêng cho thông báo, tác giả đã xóa không còn bật/tắt được |
| O-04 | Bình luận đang chờ khi bài bị tác giả xóa có được duyệt rồi hiện trên tombstone? | Có: bình luận theo vòng đời riêng của nó | “Đóng băng” cần mốc thời gian xóa và một trạng thái mới cho bình luận |
| O-05 | Xóa bài khi đang “Chỉ mình tôi” (từng công khai) có để lại tombstone? | Không: xóa không được nới rộng thứ đang hiển thị | Có tombstone sẽ làm bình luận đọc lại được sau khi tác giả đã ẩn |
| O-06 | “Hữu ích” khi bình luận được sửa | Vote ở lại với revision được vote (LVE-FR-04); tổng công khai chỉ đếm bản đang hiển thị | Giữ vote qua bản sửa cần chấp nhận “vote cho nội dung cũ là vote cho bản viết lại” |
| O-07 | “Đánh giá cao” khi bài đăng revision mới ảnh hưởng xếp hạng | Giữ luật LVE hiện hành: count theo revision công khai hiện tại, nên bản mới bắt đầu lại | Mang count qua bản sửa nhỏ cần định nghĩa “sửa nhỏ” và chống lạm dụng |
| O-08 | Retention/purge vật lý sau khi tác giả xóa | Không purge tự động; nội dung không còn đường đọc công khai | Purge cần thời hạn, phạm vi audit và xử lý backup |
| O-09 | Tombstone hiển thị gì ngoài câu thông báo | Không tiêu đề, nội dung, thẻ, hashtag, bộ sưu tập; không tên tác giả | Hiện tên tác giả/tiêu đề là đường đọc lại một phần nội dung |
| O-10 | Khôi phục bài đã xóa / bài bị kiểm duyệt gỡ | Không có trong model (terminal) | Cần transition có review và luật cho bình luận/vote cũ |
| O-11 | Khôi phục tài khoản khi dùng username + password | Email đã xác minh là kênh khôi phục **tùy chọn** (tái dùng hạ tầng mã email) | Không có kênh khôi phục: quên mật khẩu là mất tài khoản |
| O-12 | Quy tắc username và mật khẩu | Xem ACC-03 (ASCII thường, 3–30 ký tự, duy nhất không phân biệt hoa thường; mật khẩu ≥ 10 ký tự, Argon2id) | Username Unicode cần luật chống giả mạo ký tự |
| O-13 | Giới hạn hashtag mỗi bài | 10 hashtag khác nhau đầu tiên được index; phần còn lại là chữ thường | Từ chối khi gửi thay vì bỏ qua cần lỗi và copy riêng |
| O-14 | Tham số `H`, cửa sổ, timestamp độ mới | `H` có version và bounds; độ mới = thời điểm công khai lần đầu của bài | Dùng thời điểm revision hiện tại sẽ cho phép sửa để “bump” |
| O-15 | Ai được phản đối quyết định của admin | Không ai trong giai đoạn này: quyết định admin là cuối | Cần cấp xét lại thứ hai và người thực hiện |
| O-16 | Người viết rút bản sửa đang chờ | Chưa có lệnh riêng; sửa lần nữa thay candidate | Cần lệnh “hủy bản sửa” và trạng thái tương ứng |

## 5. Yêu cầu UX và dữ liệu bổ sung (`DESIGN_PROPOSED`)

Chủ sản phẩm cho phép bổ sung yêu cầu UX/dữ liệu hợp lý. Các mục này được ghi thành cột
“Đề xuất UX/dữ liệu” ngay cạnh requirement tương ứng ở
[requirements §14](../../prd/requirements.md#14-quyết-định-sản-phẩm-10112026), không phải quyết
định đã chốt. Tóm tắt:

- **Mặc định an toàn:** bài mới và bộ sưu tập mới bắt đầu ở “Chỉ mình tôi”; đổi audience là thao
  tác có xác nhận, nêu rõ hệ quả (công khai → sẽ được kiểm duyệt; riêng tư → link cũ ngừng hoạt
  động, bình luận ẩn cho tới khi công khai lại; xóa → không khôi phục được, bình luận đã duyệt còn
  đọc qua link cũ).
- **Trạng thái nói thật:** tác giả thấy riêng từng trục (audience, kết quả kiểm duyệt của từng
  bản, bản đang công khai). Chữ đề xuất: “Đang chờ kiểm duyệt”, “Chưa đánh giá được, sẽ thử lại”,
  “Cần quản trị viên xem xét”, “Không được duyệt” kèm lý do có thể hành động. Không hiển thị thời
  hạn xử lý khi chưa có cam kết.
- **Phản đối:** “Cần quản trị viên xem xét. Kênh gửi yêu cầu chưa được kết nối; chưa có thông báo
  được gửi.” (copy đề xuất, chưa dựng UI). Không hiện “đã gửi” khi mới chỉ lưu case.
- **Sửa bình luận:** người viết thấy “Bản sửa đang chờ duyệt” trên bản của mình; người khác chỉ
  thấy bản đã duyệt. “Đã sửa” không mở lịch sử.
- **Dữ liệu:** mọi quyết định kiểm duyệt lưu target, revision/digest, policy version, evaluation
  ID, actor (evaluator/admin), outcome, mã lý do và thời điểm; nội dung prompt/chữ riêng tư không
  vào log vận hành. Timestamp cần có: `first_published_at` (không đổi khi sửa), thời điểm đổi
  audience, thời điểm xóa. Tham số xếp hạng có tên version (`recommend.v1`, `top.v1`).
- **Danh sách hữu hạn:** Top ít hơn 100 bài không có ô trống hay số giả; hết danh sách nói rõ là
  hết; cursor hết hạn có lời mời làm mới, không tự nhảy về đầu.

## 6. Kế hoạch

### 6.1. Lượt này (đã làm)

1. Khảo sát owner docs, source và model tại HEAD thực thi; lập DecisionDelta (mục 2).
2. Cập nhật owner trước: [vision](../../prd/vision.md), [requirements](../../prd/requirements.md),
   [decision log](decisions.md); rồi tài liệu dẫn xuất: auth, admission, ownership, LVE, taxonomy,
   feature map, AGENTS và skill đang nhắc luật cũ.
3. Viết đặc tả đích theo owner:
   [xuất bản và kiểm duyệt](publication-moderation.md) (kèm contract evaluator, admin, phản đối,
   tombstone), [sửa bình luận](comment-editing.md), [bộ sưu tập](collections.md),
   [Đề xuất và Top 100](discovery-ranking.md), [hashtag](hashtags.md).
4. Sáu model Quint nhỏ + runner + vectors + negative controls ở
   [`behavior/target`](../../services/vink-app/behavior/target/README.md); nối thành một bước của
   lane `verify discussion` hiện có (không tạo lane mới).
5. [Registry](../development/product-decisions-registry.md): yêu cầu → model/invariant/scenario →
   gap, và bằng chứng đã chạy.

### 6.2. Thứ tự PR runtime đề xuất

Thứ tự theo phụ thuộc thật và owner của file; mỗi PR chuyển model của mình từ `behavior/target`
sang thư mục owner kèm Rust replay (xem README của thư mục đó). Không PR nào ở đây được triển khai
trong lượt này.

| # | PR | Phụ thuộc | Owner chính | Model chuyển sang replay |
| --- | --- | --- | --- | --- |
| 1 | Auth username + password (ACC-03) | D-05 hiện có | `services/vink-identity` (credential, Argon2id, rate limit), `services/vink-app/src/http/auth.rs`, `apps/vink-web/src/account.rs` | — (kiểm bằng PostgreSQL/binary thật) |
| 2 | Audience + publish intent + pointer (AUD, PUB-01/02) | 1 cho ghi production; lõi thuần làm được trước | `domain/` mới cho publication, `persistence`, migration | `publication.qnt` (phần author) |
| 3 | Evaluator port + mock + quyết định có version; admin operator (MOD-07..09) | 2 | `application` port, worker/outbox hiện có, `vink-app` CLI | `publication.qnt` (phần evaluator/admin) |
| 4 | Bình luận qua evaluator; delta admission (COM-09) | 3 | `domain/discussion/admission.rs`, migration bỏ CHECK premoderation-only | hợp nhất vào `admission.qnt` |
| 5 | Phản đối/escalation (MOD-10) | 3 | module case mới, CLI operator | `appeal.qnt` |
| 6 | Tác giả xóa bài + tombstone (PUB-04/05) | 2 | publication + `persistence/discussion.rs::readable_thread` | `tombstone.qnt` |
| 7 | Sửa bình luận (COM-10) | 4 | `domain/discussion/lifecycle.rs`, bảng revision bình luận | `comment_edit.qnt` → `behavior/discussion` |
| 8 | Bộ sưu tập (COL) | 2 | module `collection` mới | `collections.qnt` |
| 9 | Hashtag (HTG) | 2 | `domain/` grammar, projection khi publish, `persistence/discovery.rs` | — (unit + PostgreSQL) |
| 10 | Đề xuất + Top 100 (RANK/TOP) | 2, LVE hiện có | `domain/discovery.rs`, `persistence/discovery.rs` | `discovery.qnt` |
| 11 | UI Web, dashboard admin, provider AI thật, email admin | theo giao việc sau | `apps/vink-web`, hạ tầng | — |

4–10 độc lập với nhau sau khi 2–3 có; 1 có thể đi song song với 2 ở mức lõi thuần.

## 7. Bổ sung 11/11/2026 — quản trị kiểm duyệt

**ID:** `ADMIN-MODERATION-20261111` · **Nguồn:** brief của chủ sản phẩm ngày 11/11/2026 (“Vink
Admin — chốt PRD, SSOT và domain/core kiểm duyệt”) · **Mốc:** bình luận theo #120/#148, core tác
phẩm theo #151/#157, điểm nối runtime #158/#159. Luật ở bảng dưới là `PRODUCT_CONFIRMED`; tên kiểu,
mã từ chối và baseline kỹ thuật là `DESIGN_PROPOSED`. Lượt này có **PRD/SSOT, core Rust thuần,
model Quint và replay**; không migration, API/CLI, worker, design/UI.

### 7.1. DecisionDelta

| Quyết định | ID canonical | Luật cũ | Luật mới | Model | Core |
| --- | --- | --- | --- | --- | --- |
| ADM-01 Admin tổng và moderator | MOD-09, MOD-11 | Một vai trò admin; grant moderator chỉ cho bình luận | Admin tổng quản lý quyền, phản đối cuối cùng và chế độ tự động. Grant duyệt tác phẩm và grant duyệt bình luận độc lập; cá nhân hoặc nhóm do admin quản lý; toàn Vink hoặc một `WorkId`; có hạn. Grant bình luận hiện hành được kế thừa. Kiểm lại quyền, tư cách tài khoản, thành viên nhóm ở mỗi lần đọc/xử lý; tự né nội dung của mình và bình luận trên bài của mình | `governance.qnt` | `domain::moderation::authority` |
| ADM-02 Nhận ca | MOD-12 | Không có khái niệm ca | Một người nhận mỗi lúc; trả ca, ghi chú nội bộ, chuyển admin; mất quyền thực tế → về hàng chờ trừ khi còn grant hợp lệ khác; admin nhận lại có lý do; kết luận kiểm phiên bản ca + phiên bản quyết định | `governance.qnt` | `domain::moderation::case` |
| ADM-03 Can thiệp | MOD-13 | Chỉ admin quyết định tác phẩm; bình luận: duyệt/từ chối/gỡ (terminal) | Moderator duyệt/từ chối/ghi đè AI trong grant; moderator tác phẩm bác bản đang đăng; chỉ admin gỡ cả bài. Moderator bình luận ẩn và khôi phục đúng bản đã duyệt; gỡ của người viết và gỡ cũ vẫn hiệu lực | `publication.qnt`, `governance.qnt` | `WorkModeration::apply_human_decision`, `discussion::hide` |
| ADM-04 Phản đối | MOD-10 | Chỉ phản đối quyết định AI; quyết định admin là cuối (O-15) | Phản đối quyết định bất lợi của AI **hoặc moderator**; một hồ sơ mỗi phiên bản quyết định; khi mở chỉ admin kết luận; giữ/đảo đều có lý do, tạo phiên bản mới; admin là cấp cuối; bản mới có ca riêng; kết luận bản cũ chỉ công khai nếu ý định hiện hành trỏ đúng bản | `governance.qnt` (thay `target/appeal.qnt`) | `domain::moderation::appeal` |
| ADM-05 Dừng tự động | MOD-14 | Không có | Dừng: việc mới và việc AI đang chờ sang hàng chờ người; kết quả AI đang chạy mất hiệu lực. Bật lại chỉ cho lượt gửi mới; lịch sử phân biệt ghi nhận / có người nhận / có kết luận | `governance.qnt` | `domain::moderation::automation` |
| ADM-06 Gửi duyệt theo phiên bản | PUB-06 | Approved của bản cũ luôn dùng lại được; kết quả Approved tới sau khi chuyển riêng tư vẫn ghi quyết định (`approved_unpublished`) | Chỉ yêu cầu công khai mở ca; chuyển riêng tư **đóng** ca và phản đối, kết quả tới trễ không tạo quyết định; công khai lại dùng quyết định còn hiệu lực đúng snapshot + policy, policy đổi → lượt mới, quyết định người → người có thẩm quyền | `publication.qnt` | `WorkModeration::decide_author`, `JobState::Withdrawn` |
| ADM-07 Trang “Nội dung đã được ẩn” | PUB-05, PUB-07 | B: URL cũ không tìm thấy, ẩn cả thảo luận | Bài đang công khai chuyển riêng tư: URL cũ chỉ hiện “Nội dung đã được ẩn” + bình luận đủ điều kiện; rút khỏi đọc công khai, tìm kiếm, Khám phá, Đề xuất. Chưa từng công khai và bị admin gỡ: không tìm thấy | `tombstone.qnt` | `WorkAccessPlan::AuthorHidden` |
| ADM-08 Bình luận qua URL cũ | COM-11, PUB-04 | Tombstone chỉ sau khi xóa bài đang công khai; xóa khi đang ẩn không để lại gì (O-05) | Trang ẩn và trang thay thế: đọc, xử lý ca đã gửi, tự gỡ; chặn tạo mới/sửa/tương tác. Kiểm duyệt bình luận không thấy thân bài. Xóa từ trang ẩn giữ bình luận. Công khai lại: chỉ thông báo sự kiện mới | `tombstone.qnt` | `application::publication`, `Publication::should_notify_author` |
| ADM-09 Điều tra riêng | MOD-15 | Không có; admin không đọc bản riêng | Admin có quyền, không xung đột, căn cứ cụ thể, nguồn bằng chứng, đúng ID bản từng công khai; mỗi lần đọc kiểm quyền và audit trước khi trả snapshot; hồ sơ chỉ ghi kết luận và căn cứ; hết khi đóng hoặc điều kiện hết hiệu lực | `governance.qnt` | `domain::moderation::investigation` |
| ADM-10 Mặc định và phạm vi | — | — | Quyền điều tra theo từng lần đọc; ca nhận kết thúc bằng trả ca, mất quyền hoặc admin tiếp quản; giữ chính sách lưu dữ liệu hiện hành (O-08). Migration, API/CLI, worker, design/UI, báo cáo độc giả, quản lý policy, khóa tài khoản: đợt tích hợp sau | — | — |

### 7.2. Điểm `OPEN` bị thay hoặc thêm

| # | Thay đổi |
| --- | --- |
| O-03 | Mở rộng: “không ghi mới” áp cho cả trang ẩn (PUB-07); xử lý ca đã gửi và tự gỡ vẫn được (`PRODUCT_CONFIRMED`) |
| O-05 | **Đóng.** Xóa bài khi đang ẩn **sau khi từng công khai** giữ bình luận qua URL cũ (trang thay thế); bài chưa từng công khai vẫn không để lại gì |
| O-15 | **Đóng một phần.** Quyết định của moderator phản đối được; kết luận admin là cấp cuối. Cấp xét lại thứ hai trên kết luận admin vẫn không có |
| O-17 | (mới) Trang ở URL cũ sau khi moderator **bác bản đang đăng** của bài vẫn đang ở “Công khai”: đề xuất tối thiểu là không tìm thấy cho tới khi có bản được duyệt mới (bài không phải do tác giả ẩn). Đổi sang trang ẩn cần luật cho bình luận |
| O-18 | (mới) Thời hạn mặc định của grant/hồ sơ điều tra: không có giá trị mặc định; người cấp đặt hạn hoặc để không hạn |
| O-19 | (mới) Danh mục sự kiện thông báo cho tác giả: core chỉ có fence “sự kiện mới sau lúc công khai”; kênh thông báo chưa có |

### 7.3. Bàn giao

Chi tiết hành vi: [publication-moderation §5–§8](publication-moderation.md#5-quản-trị-viên-moderator-và-phản-đối).
Core và bằng chứng: [moderation-core](../development/moderation-core.md#admin-moderation-core--admin-core-01),
[publication-core](../development/publication-core.md#trang-ẩn-và-gửi-duyệt-theo-phiên-bản--pub-hidden-01),
[registry](../development/product-decisions-registry.md). Đợt tích hợp tiếp theo: migration cho grant,
nhóm, ca, phản đối, chế độ tự động, hồ sơ điều tra, `hidden`/`ever_public`/`public_epoch` của
publication và trạng thái job `withdrawn`; lệnh operator/API; worker áp kết quả với fence chế độ tự động.
