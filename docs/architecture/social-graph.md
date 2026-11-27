# Social Graph · hợp đồng liên miền r1

**Ngày 18/11/2026 · #288.** [PRD §15](../../prd/requirements.md#15-hồ-sơ-tác-giả-và-theo-dõi-social-graph)
sở hữu luật sản phẩm. Tài liệu này quy định miền nào quyết định một sự thật và thời điểm sự thật
đó có hiệu lực. [HTTP Social r1](../../services/vink-app/contracts/http/v1/social-graph.md) quy định
dữ liệu nào được ra khỏi server. [Bàn giao và truy vết](../development/social-graph.md)
ghi các issue tiêu thụ r1 và phép kiểm còn phải chạy.

| Chiều | Trạng thái tại r1 |
| --- | --- |
| Sản phẩm | R1–R9 `PRODUCT_CONFIRMED` theo [R10 §2](https://github.com/loveoverflowcom/vink/issues/284). SHA-256 body gốc 17/11 theo #288 là `3b076a69470a3e0d8c0b386eefd4831bccb37ad3665e17a68c9724c0cbc02973`. Cập nhật đúng phạm vi issue con ngày 18/11 ở hai bảng dưới. |
| Kiến trúc | `DESIGN_PROPOSED`. D-05@r1 `PROVISIONAL`, O-12@r2 `PRODUCT_CONFIRMED`. Các proposal Social và USER-01@r2 chờ signoff đúng revision/hash theo VDR-DECISION-SIGNOFF@r1. [Kho TOML](../../decisions/README.md#social-graph) |
| Triển khai | Xem [registry Social](../development/social-graph.md) cho mức triển khai và bằng chứng từng lát. |
| Bằng chứng | Đối chiếu source tại develop `e0551edf08d4d44903f35717acffa6da70ce4bf3`. Kiểm tài liệu/registry theo SHA ở PR #288. Các phép kiểm Social model/PG/HTTP/Web là mục tiêu cho child. |

<!-- vdr: active USER-01@r1 -->
<!-- vdr: active D-05@r1 -->
<!-- vdr: active O-12@r2 -->
<!-- vdr: active VDR-DECISION-SIGNOFF@r1 -->

## Ranh giới sở hữu

| Sự thật | Owner duy nhất và điểm nối hiện hữu | Giá trị Social sử dụng |
| --- | --- | --- |
| Credential, subject, session, disable | [Identity](../../services/vink-identity/contracts/rpc/v1/) | Xác minh phiên. App chỉ dùng kết quả resolve. Username, email, mật khẩu, subject và cookie nằm ngoài projection Social. |
| Binding, PrincipalId, standing, bút danh | [App Principal](../../services/vink-app/src/persistence/principal.rs) | Binding bất biến nối subject với đúng PrincipalId. PenName trim/NFC, 1–80 ký tự, cho trùng. Profile dùng lại bút danh ở Principal. |
| Địa chỉ, hồ sơ và quyền từng trường | App Profile (#290/#292) | Current Handle, alias và tombstone chung namespace. Profile cung cấp ref công khai theo PrincipalId bên trong App, kể cả khi chưa có bài. |
| Cạnh, thứ tự lệnh và Mutual | App Graph (#294) | Cạnh có hướng giữa hai PrincipalId khác nhau. Followers là cạnh vào, Following là cạnh ra. Mutual là hai cạnh ngược chiều còn hiệu lực. Mọi list/count lấy từ cùng nguồn cạnh. |
| Bài và mốc lần đầu công khai | [Publication](../../services/vink-app/src/persistence/publication.rs) | Quyết định WorkId/revision/generation đang đọc được, trang thay thế, mốc đầu và provenance. Discovery tiêu thụ các fact này. |
| Attribution bình luận | [Discussion](../../services/vink-app/src/persistence/discussion.rs) | Từ author của root/reply **và của bình luận được trả lời**, lấy projection Profile hiện hành tại lúc đọc, theo visibility của Discussion. |
| Danh sách bài/tác giả và gợi ý | [App Discovery](../../services/vink-app/src/persistence/discovery.rs) | Tìm tác giả bắt đầu từ hồ sơ đủ điều kiện. Query Feed kết hợp Graph và Publication. Chỉ dùng dữ liệu công khai cho candidate và thứ tự gợi ý. |
| Hiển thị, focus, phiên và chuỗi trang | [Web](../../apps/vink-web/src/read_lifecycle.rs) | Render các projection server cấp. Từ card/Reader/comment đi đến ref có thật. Trở về từ Reader theo mode/query và anchor còn hợp lệ. |
| Xác minh/điều phối xóa hai DB | Account/Identity (SOC-DEP-01) | Cấp operation có thể replay cho từng owner. App giữ fence và trạng thái áp dụng bền. Đóng E2E cần evidence của Account. |

## Provisioning và địa chỉ tác giả

Điểm ghi hiện hữu là transaction tạo principal + binding. Tiêu chí cấp Profile là binding tài
khoản đã xác minh và bút danh hợp lệ. Ghi Profile, current Handle sinh sẵn và trạng thái setup
cùng giao dịch, hoặc hoàn tất backfill của binding cũ trước khi mở đọc. Hai lần provision cùng
subject trả cùng principal đã thắng. Seed, fixture, principal kỹ thuật hoặc tên cũ sai chuẩn được
phân loại trước backfill. Chuẩn hóa PenName qua parser của App, giữ binding/ownership, ghi số
được cấp, bỏ qua và cần xử lý. Chạy theo batch có checkpoint, chạy lại được, so với tài khoản
đủ điều kiện sau khi writer mới đã hoạt động. Chỉ mở đường đọc sau khi đối soát được mỗi binding đủ điều kiện với đúng một Profile và một current Handle. Lượt GET không sinh tên hay sửa dữ liệu.

[#480](https://github.com/loveoverflowcom/vink/issues/480) gộp UI Handle vào Chỉnh hồ sơ.
`GET/POST /api/v1/me/handle` và receipt giữ version của Handle, tách khỏi `PUT /api/v1/me/profile`.
Một lần ghi ở bên này không thay version hoặc nháp phía kia. URL cũ chỉ chuyển hướng; contract
`/@handle` cho current, alias, tombstone và missing giữ nguyên.
[LayoutSpec](../design/profile-inline-handle-480.md).

```text
chưa cấp ── provision hợp lệ ──► current H0, chờ setup, còn 2
  chờ setup ── xác nhận giữ H0 ──► đã setup, current H0, còn 2
  chờ setup ── xác nhận H1 mới ──► đã setup, current H1, H0 là alias, còn 2
  đã setup  ── đổi thành công  ──► current mới, current cũ là alias, còn 1 rồi 0
  mọi trạng thái đã cấp ── xóa ──► mọi tên đã dùng là tombstone vĩnh viễn
```

“Dùng địa chỉ này” kết thúc setup ở **commit thành công**, kể cả khi giữ H0. Sau setup,
ghi lại cùng tên là no-op. Hai principal không thể giữ cùng khóa chuẩn hóa giữa ba trạng thái.
Kiểm grammar, từ dành riêng, độ dài và dạng chuẩn ở ingress và tại ràng buộc lưu trữ. Tên gợi ý
chưa commit không giữ chỗ vô hạn. Sinh với số lần thử hữu hạn, hết ngân sách thì trả lỗi có
thể thử lại, không để principal/alias/quota dở dang. Chuẩn hóa khóa trước kiểm trùng cả khi
backfill. #290 chốt grammar chi tiết trong giới hạn ASCII của PRD.

Đổi tên là một commit gồm current mới, alias của tên cũ, quota, version và kết quả lệnh.
Mỗi alias tra cùng chủ đến current **tại thời điểm đọc**. Sau H0 → H1 → H2, H0 và H1 đều
301 thẳng tới H2. `Location` là đường dẫn nội bộ do server dựng, chỉ mang query đã cho phép.
Fragment do browser xử lý. Trả `no-store` cho redirect tới khi chứng minh được hành vi
revalidate qua hai lần đổi và xóa. Tombstone trả trang không còn hồ sơ, không có Location,
tên hoặc ref của chủ cũ. Kiểm cả GET, HEAD và URL đã từng redirect. Alias làm lộ mối liên hệ
với địa chỉ cũ trong thời gian tài khoản còn hoạt động.

Nếu người đổi bút danh còn giữ bài, card/Reader/comment dùng ref của cùng PrincipalId.
Với dữ liệu cũ thiếu binding/ref đã xác thực, hiển thị tên được phép dạng chữ, không tạo
link bằng suy đoán từ tên trùng hoặc id kỹ thuật. Với tài khoản đã xóa, dùng projection
đã xóa ở mọi adapter, kể cả client chưa nhận diện kiểu ref mới.

## Lệnh có thứ tự và mất phản hồi

Mỗi lệnh riêng có actor do phiên xác định, scope đích, command ID, ý định tường minh, version
mong đợi và nội dung gắn với cùng ID. Lệnh Follow/Unfollow đặt trạng thái đích. Cùng ID/cùng
nội dung trả lại kết quả đã ghi trong scope có quyền. Cùng ID khác nội dung hoặc version cũ
của một lệnh mới trả conflict. Đọc receipt cũng cần quyền hiện tại. Receipt cho biết **lệnh nào
đã được chấp nhận**. Đọc lại trạng thái hiện hành cho biết **nút nào hiển thị bây giờ**.

Ví dụ A gửi Follow B ở version 0, commit thành version 1 rồi mất ACK. Sau khi đối soát, Unfollow
từ version 1 thành version 2. Retry lệnh Follow đầu chỉ tra kết quả cũ, kể cả hiện không còn
cạnh. ACK version 1 về muộn không thay nút của version 2. Cùng version ở hai tab chỉ có một
thay đổi thắng. No-op không tăng count/quota. Nếu hết thời hạn tra receipt, trả kết quả chưa
xác định và trạng thái hiện tại, vẫn dùng version/order bền để ngăn replay cũ.
Áp cùng nguyên tắc cho setup/rename, quyền Profile và xóa.

**Tại commit.** Trong App, các writer cạnh lấy khóa principal của **cả hai đầu** theo một thứ tự
toàn cục (ví dụ PrincipalId tăng dần), sau đó khóa scope Profile/namespace hoặc cặp Graph và
các projection/counter liên quan theo thứ tự cố định. Suspend, rename và xóa phải phối hợp cùng
fence. Kiểm standing, capability của actor, lifecycle/eligibility đích, version, quyền và
receipt **sau khi chờ khóa**. Hiệu ứng, version/order và receipt cùng commit hoặc cùng rollback.
Không giữ khóa App qua RPC Identity. Xóa đặt fence ngắn trước rồi dọn theo batch, không giữ
khóa một đầu rồi chờ đầu kia theo thứ tự ngược. Kiểm bằng hai kết nối PG (kể cả A→B và B→A).

## Publication, feed và dữ liệu cũ

Ngày đầu là lúc **lần đầu thực sự thiết lập public pointer thành công** cho WorkId. Publication
ghi fact đúng một lần cùng transaction đó. Tạo snapshot, kết quả duyệt chưa đặt pointer,
transaction rollback và thời điểm Follow đều không tạo mốc. Lấy metadata của revision hiện
đang đọc được. Sửa, rút rồi đăng lại, private/public, re-review, admin/evaluator/appeal không
đổi mốc. Cả [writer tác giả](../../services/vink-app/src/persistence/publication.rs) và
[writer kiểm duyệt](../../services/vink-app/src/persistence/moderation_write.rs) hiện ghi lại
`publications.published_at`. Rút còn xóa hàng. Rà cả [seed](../../services/vink-app/src/dev_seed.rs)
và [legacy bootstrap](../../services/vink-app/migrations/20261116000000_work_publication_persistence.sql).

| Fact Publication bàn giao | Cách xử lý của #291/#296 trong đợt mở thử |
| --- | --- |
| Biết chắc chưa từng public | Chưa có mốc. Commit public đầu tạo `exact`. |
| `exact` | Có chứng cứ lần đầu thực sự public. Feed nhận khi pointer/authority hiện hành còn cho đọc. |
| `legacy` | Có dấu vết với nguồn nhưng chưa đủ chứng minh lần đầu. Lưu nguồn, loại khỏi đợt mở thử. |
| `unknown` | Thiếu bằng chứng, kể cả bài cũ đã rút mất pointer. Loại khỏi đợt mở thử. Tái đăng không biến `legacy/unknown` thành `exact`. |

Backfill idempotent theo WorkId, báo số từng nhóm và nguồn. Dữ liệu lịch sử chưa rõ vẫn có
thể đọc qua các bề mặt hiện hành theo Publication. Khi query Feed, lọc membership của viewer,
tác giả hợp lệ, quyền đọc hiện tại và provenance trước ORDER/LIMIT. Chỉ một dòng mỗi WorkId,
thứ tự `first_published_at DESC` và WorkId làm khóa phụ cố định cho cả truy vấn lẫn cursor.
Chỉ lấy metadata, tiêu đề/thẻ hiện hành, ngày đầu hiển thị cạnh tác giả. Không tải manuscript
để lọc hoặc truy vấn từng dòng. Kiểm kế hoạch query trên dữ liệu lệch. Bình luận, Follow và
“Thấm”/“Hữu ích” không sinh item.

Trên Web, [thanh mode Khám phá](../design/redesign-r4/layout-spec.md#4-khám-phá) tiêu thụ
`Session::profile_scope` và feature flag như `FollowingFeed`. Cookie presence không cấp scope.
Tín hiệu đổi phiên fence ngay scope cũ. Refresh cùng phiên giữ hành vi của Session owner.
Đích trực tiếp vẫn do `FollowingFeed` diễn giải trạng thái phiên và do HTTP kiểm quyền mỗi lần đọc.

## Xóa, hạn chế và tương thích

1. Account xác minh yêu cầu, cấp operation ID/generation, chốt trạng thái đang xóa và quy tắc
   retry/retention giữa hai DB (DEP-01). App lưu fence bền của Principal trước khi dọn. Từ fence,
   đọc mới đánh giá chủ là đang thu hồi, chặn ghi Social vào/ra, loại khỏi query công khai/cá nhân.
   Identity xử lý credential/session ở DB của mình. Binding có trigger cấm UPDATE/DELETE.
   Không xóa cascade principal hoặc chuyển ownership. [Migration binding](../../services/vink-app/migrations/20261107000000_principal_bindings.sql)
2. Graph kết thúc cạnh vào và ra, đối soát count/list/Mutual cả hai phía. Profile giữ tombstone
   tất cả tên, dừng public ref, ảnh và biến thể. Search/gợi ý/feed loại chủ. Publication rút
   pointer, URL bài theo trang thay thế của owner. Discussion giữ text của bình luận công khai
   hợp lệ theo quyền đọc, thay mọi attribution, kể cả reply context, thành “Tài khoản đã xóa”.
   Kết quả public không mang id/handle/link/ảnh/tên cũ qua metadata, hydration hay cache.
3. Mỗi owner ghi checkpoint/receipt cùng hiệu ứng. Crash sau một bước hoặc mất ACK thì tiếp tục
   đúng operation. Trong lúc dọn, projection phải đọc fence hoặc tạm đóng đường đọc chưa hiểu
   fence (kể cả count dẫn xuất). Account chỉ coi là hoàn tất sau khi đối soát các owner ở cả
   hai DB. Lỗi dependency là chưa hoàn tất. DEP-02 do Account/Identity cung cấp capability
   riêng nếu triển khai hạn chế đăng/bình luận. Guard full suspension/disable vẫn áp nguyên vẹn.

**Mở dần và quay lui.** Thêm cấu trúc tương thích trước, nâng mọi writer (kể cả seed/worker),
chạy backfill có thể tiếp tục, so số lượng và provenance, rồi mới mở endpoint/consumer.
Nếu writer cũ có thể bỏ qua quota, fence hoặc mốc đầu, dừng write liên quan trong lúc chuyển.
Kiểm client cũ × provider mới và ngược lại. Khi client/SSR/asset/cache cũ còn phát giá trị
đã chuyển `self` hoặc link đã thu hồi, tắt đường đọc đó cho cả phiên bản cũ trước khi bật
quyền mới. Backout tắt consumer trước rồi dừng ghi mới, giữ nguyên reservations/alias/tombstone,
quota, cạnh/order/receipt, quyền riêng, checkpoint và fact first-public. Chỉ mở lại sau đối
soát và revalidate. Proposal đổi bằng revision kế tiếp, giữ lịch sử và signoff cũ.

## Lựa chọn sản phẩm và phụ thuộc

Chủ sản phẩm quyết định các SOC-OPEN ở R10 §6/§11. Bảng giữ cả hai phương án của nguồn và
cập nhật đã có của child. Điểm mở chỉ chặn hành vi khác nhau giữa hai cột.

| ID | Phương án A | Phương án B | Trạng thái và phần việc bị ảnh hưởng |
| --- | --- | --- | --- |
| SOC-OPEN-01 | Dialog chung xác nhận từng trường, nêu giá trị sắp công khai | Cùng luồng gộp các thay đổi chủ đã chọn, liệt kê chính xác từng trường sắp công khai | `PRODUCT_OPEN` · #289/#292, chỉ luồng chuyển sang everyone. DOB luôn cần hành động xác nhận và USER-01@r2 có hiệu lực. |
| SOC-OPEN-02 | Xác nhận tường minh cả khi giữ gợi ý | Chờ lần đầu đặt tên riêng mới kết thúc setup | `PRODUCT_CONFIRMED` A theo lời chủ ở [#290 r2](https://github.com/loveoverflowcom/vink/issues/290), 18/11. #290 kiểm cả giữ và sửa tên rồi còn 2 lượt. |
| SOC-OPEN-03 | Alias đã nghỉ vĩnh viễn không trở lại current | Chỉ chính Principal cũ được dùng lại, tính một lượt đổi bình thường sau setup | `PRODUCT_OPEN` · #290, chỉ lệnh/UX reclaim. Người khác vĩnh viễn không được cấp tên này. |
| SOC-OPEN-04 | Avatar chữ theo phạm vi sản phẩm v1 | Upload có storage, thay/xóa ảnh gốc, thumbnails và quyền đọc bytes | `PRODUCT_OPEN` · #292/#298. #298 chỉ kích hoạt khi chọn B. A điều chỉnh nghiệm thu media. |
| SOC-OPEN-05 | Exact Handle, tìm bút danh/Handle hiện tại, alias chỉ resolve ở URL | Thêm tìm alias về đúng một Principal, nêu rõ liên hệ lịch sử | `PRODUCT_CONFIRMED` A, mode **Tác giả** riêng trong Khám phá theo [#295](https://github.com/loveoverflowcom/vink/issues/295), 21/11/2026. |
| SOC-OPEN-06 | Tập xếp hạng công khai trung tính, CTA theo viewer | Sau xếp hạng mới lọc mình/đã Follow bằng quyền của viewer | `PRODUCT_CONFIRMED` A theo [#295](https://github.com/loveoverflowcom/vink/issues/295), 21/11/2026. Phần Theo dõi dùng Graph #294 theo phiên. |

**Phụ thuộc Account/Identity**

| ID | Đầu vào bên ngoài (`EXTERNAL_DEPENDENCY`) | Phần bị chặn |
| --- | --- | --- |
| SOC-DEP-01 | Account/Identity cung cấp xác minh, trạng thái xóa, retention, điều phối và evidence hai DB. | E2E xóa #297. Adapter Social có thể kiểm riêng. |
| SOC-DEP-02 | Account/Identity cung cấp capability hạn chế chỉ đăng/bình luận. | Claim runtime về mức hạn chế này ở #294/#297. |

## Lựa chọn kỹ thuật theo owner

Xuất phát từ sáu `TECHNICAL_OPEN` ở [R10 §11](https://github.com/loveoverflowcom/vink/issues/284).
Child ghi lựa chọn, phương án bỏ và bằng chứng ở đúng owner. CHOICE-04 có cập nhật sau R10.

| ID · owner | A / B, đánh đổi | Phép kiểm phân biệt và tình trạng |
| --- | --- | --- |
| CHOICE-01 · Profile #290 | Một registry với trạng thái current/alias/tombstone, hoặc một bảng giữ chỗ unique và các ref current/history riêng (thêm FK/join). | PG unique xuyên ba trạng thái, rename/delete race và rollback cùng quota. Mở. |
| CHOICE-02 · Graph #294 | Giữ desired state/version kể cả khi unfollow (giữ hàng lịch sử), hoặc cạnh sống + thứ tự/receipt bền (thêm lưu trữ và dọn). | PG thử ABA, mất ACK, đổi ý, retry cũ, xóa và restart. Mở. |
| CHOICE-03 · Graph #294 | Batch aggregate theo index trên cạnh, hoặc materialized counter cùng transaction có kiểm/rebuild. | Đo query plan, skew, contention. Kiểm exact delta qua no-op/rollback/xóa và rebuild bằng nguồn cạnh. Mở. |
| CHOICE-04 · Publication #291 | Fact bền mỗi WorkId ghi cùng public pointer lần đầu, hoặc derive từ lịch sử event đầy đủ, có provenance và chi phí đọc hữu hạn. | A đã chọn trong [#291 r2](https://github.com/loveoverflowcom/vink/issues/291). PG kiểm mọi writer, backfill, race, rút/tái đăng. Lịch sử chỉ xác thực mốc cũ khi đủ bằng chứng. |
| CHOICE-05 · App/Web #292, #296 | SSR public + overlay/Feed no-store sau xác thực, hoặc SSR cá nhân cô lập theo request (có chi phí xác thực và cô lập hydration). | HTTP/SSR hai phiên, trước 304. Browser kiểm logout A→B→A, Back/focus và response muộn. Owner #292/#296 chọn SSR công khai và đọc riêng sau hydrate. |
| CHOICE-06 · Discovery #296 | Live membership mỗi trang, restart local sau mutation (bài mới trên cursor đợi refresh), hoặc cursor gắn graph revision, đổi graph thì reject/restart (tốn cập nhật revision). | Thử follow/unfollow/withdraw giữa hai trang, tie, đổi viewer/mode. Cả hai kiểm lại quyền đã thu hồi. [Owner #296](../development/social-graph.md#đang-theo-dõi-trong-khám-phá-296) chọn membership live. |
