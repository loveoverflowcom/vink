# Ownership của thảo luận hai tầng

**Ngày:** 03/11/2026 · **Trạng thái:** **Đã chốt** phần owner bởi chủ sản phẩm ngày 03/11/2026.
**Work item:** D2T-01 [#41](https://github.com/loveoverflowcom/vink/issues/41), D2T-02
[#43](https://github.com/loveoverflowcom/vink/issues/43).
**Baseline khảo sát trước quyết định:** `develop@8045d45e554e0a01f11fd252ac681f8eb9d33a56`;
quyết định áp lên `develop@31a62b1` (sau [#49](https://github.com/loveoverflowcom/vink/pull/49)).

> **Cập nhật 10/11/2026:** owner, hai tầng, quyền bật/tắt và “tác giả không sửa/xóa lời người
> khác” **không đổi**. Hai điểm D-08 có hướng mới từ chủ sản phẩm: kiểm duyệt bình luận do trợ lý AI
> quyết định ([admission §8](discussion-admission.md#8-hướng-đích-sau-quyết-định-10112026)) và người
> viết được sửa bình luận của mình ([đặc tả](comment-editing.md)). D-12 tách xóa bài khỏi chuyển
> riêng tư: bài bị tác giả xóa để lại trang thay thế cùng bình luận hợp lệ
> ([đặc tả](publication-moderation.md#6-xóa-riêng-tư-gỡ-trang-ẩn-và-tombstone)). Tất cả là hành vi đích,
> **chưa có code**; tài liệu này tiếp tục mô tả hiện trạng.

**Record (11/11/2026):** quyết định owner tra tại `D-02-DISCUSSION@r1`. Quyền bật/tắt của tác giả
tra tại `D-13@r1`. Cả hai giữ trạng thái `PRODUCT_CONFIRMED` của nguồn. Tài liệu này giữ bố trí,
HOW, bất biến đã hiện thực và rollout.

<!-- vdr: active D-02-DISCUSSION@r1 -->
<!-- vdr: active D-13@r1 -->

## 1. Quyết định

1. **Vink tự xây hệ thống bình luận** bằng Rust core thuần và PostgreSQL của App. Module
   `discussion` trong `vink-app` là owner duy nhất ghi, đọc và kiểm duyệt bình luận.
2. **Discourse chỉ là nguồn tham khảo** về thiết kế, API, kiểm duyệt và trải nghiệm cộng đồng.
   Discourse không là dependency runtime, nơi lưu bình luận, nguồn xác thực hay plugin cần
   triển khai. Không có adapter, outbox đồng bộ hay mapping sang Discourse.
3. **Tác giả tác phẩm bật/tắt bình luận trên từng tác phẩm**, độc lập với quyền kiểm duyệt của
   hệ thống.
4. **Tác giả tác phẩm không sửa, xóa hay duyệt bình luận của người khác.**
5. **Giữ hai tầng:** bình luận gốc và một danh sách phản hồi phẳng; `reply_to` chỉ giữ ngữ cảnh.

Phương án trước đó (Discourse là owner duy nhất) được thay thế. [D-02](decisions.md) ghi phần
thảo luận là đã chốt; D-13 ghi quyền bật/tắt của tác giả. D-05 (auth authority), D-08 (tiền
kiểm/sửa công khai) và D-12 (thu hồi) **vẫn mở**: quyết định owner không tự chốt chúng.

## 2. Bố trí: module-first trong `vink-app`

Thảo luận là module trong App hiện có, dùng database `vink` và role runtime/migrator của App.
Không thêm process, cluster hay database mới. Ranh giới trong code:

| Lớp | Vị trí | Trách nhiệm |
| --- | --- | --- |
| Domain thuần | `services/vink-app/src/domain/discussion.rs` và `domain/discussion/` | Cấu trúc hai tầng, nội dung bình luận, trạng thái kiểm duyệt/hiện diện, chế độ bình luận của tác phẩm, quyền, hiển thị theo người xem, idempotency, phân trang |
| Application | `services/vink-app/src/application/discussion.rs` | Xác thực actor, validate input bằng domain, ánh xạ kết quả; port `DiscussionStore` |
| Persistence | `services/vink-app/src/persistence/discussion.rs` và migration | Khóa/transaction, ràng buộc toàn vẹn độc lập Rust, nạp facts rồi gọi quyết định thuần |

Domain không import Axum, SQLx, Leptos, đồng hồ hệ thống hay sinh ID ẩn. Adapter nạp facts
trong transaction rồi gọi đúng hàm quyết định production; store giả của test application chỉ ghi
nhận input, không viết lại luật.

**Khi nào tách process `vink-discussion`:** chỉ khi đo được contention/scaling hoặc lỗi lan
truyền cần isolation. Khi đó service sở hữu database/schema, migrations, contracts và roles
riêng, không đọc/ghi chéo database App; thu hồi tức thời cần đường kiểm quyền đồng bộ, không
chỉ dựa event. Không tách emoji/sticker thành service; asset thuộc
[#25](https://github.com/loveoverflowcom/vink/issues/25).

## 3. HOW: một thành công và một lần bị từ chối

```text
Tác phẩm W (đã công khai, bình luận đang bật)
  Root A (của Lan, đã duyệt)
    Reply B: root=A, reply_to=None      (Minh trả lời A)
    Reply C: root=A, reply_to=Some(B)   (Hà trả lời B, vẫn cùng cấp với B)
```

**Thành công:** Hà gửi `{target: B, body, command_key}`. Server lấy actor từ session, tải B
trong thread của W và khóa thread/publication. Quyết định thuần kiểm khóa idempotency, tác
phẩm còn công khai, bình luận đang bật, B đang hiển thị công khai; chuẩn hóa root = A và
context = B. C được ghi ở trạng thái **chờ duyệt** cùng receipt trong một transaction. Hà thấy
C là “đang chờ duyệt”; người khác và tác giả W chưa thấy C.

**Bị từ chối:** tác giả W tắt bình luận (generation 1 → 2) đúng lúc Hà gửi một bình luận mới.
Hai transaction tranh cùng khóa hàng thread: nếu tắt commit trước, lệnh của Hà thấy `Closed` và
không ghi gì; nếu lệnh của Hà commit trước, bình luận đã tồn tại và vẫn chờ duyệt. Không có trạng
thái nửa vời. Hà gửi lại cùng `command_key` sau khi mất phản hồi sẽ nhận lại đúng bình luận cũ,
không tạo bản thứ hai; cùng khóa nhưng nội dung khác bị từ chối.

## 4. Quyền của tác giả tác phẩm

| Hành động | Tác giả tác phẩm | Người viết bình luận | Người kiểm duyệt (authority hệ thống) | Người khác |
| --- | --- | --- | --- | --- |
| Bật/tắt bình luận của tác phẩm | Có | Không | Không qua quyền này | Không |
| Gửi bình luận/phản hồi khi đang bật | Có (như người đọc) | — | — | Có, nếu đã xác thực |
| Gỡ bình luận | Chỉ bình luận của chính mình | Có, bình luận của mình | Có, gắn version | Không |
| Sửa bình luận | Không (kể cả của người khác) | Chưa mở: D-08 chặn sửa công khai trong pilot (10/11/2026: hướng đích cho người viết sửa bình luận **của mình**, COM-10; chưa có code) | — | Không |
| Duyệt/từ chối | Không | Không | Có, gắn version; tự né bình luận của mình và tác phẩm của mình | Không |

**Người kiểm duyệt** là principal có grant đang hiệu lực, do operator cấp bằng
`vink-app moderator grant` và thu hồi bằng `revoke`. Mỗi quyết định kiểm lại grant trong chính
transaction ghi (khóa chia sẻ), nên lệnh thu hồi commit trước thì thắng, lệnh đang chạy dở thì chờ.
Tác giả tác phẩm dù có grant vẫn **tự né** mọi bình luận trên tác phẩm của mình (chủ sản phẩm
xác nhận 03/11/2026): không duyệt, từ chối, gỡ hay sửa bình luận của người khác trên tác phẩm đó,
để quyền kiểm duyệt không thành đường vòng vi phạm COM-08.

**Ngữ nghĩa “tắt” (chủ sản phẩm xác nhận 03/11/2026):** tắt là **ngừng nhận bình luận và
phản hồi mới**; bình luận đã duyệt vẫn đọc được, không bị xóa hay đổi trạng thái, và bật lại
không mất gì. Tác giả không dùng “tắt” để ẩn bình luận của người khác. Nếu sản phẩm muốn “tắt”
cũng ẩn toàn bộ thảo luận, đó là thay đổi trong `CommentsMode` và đường đọc, cần ghi lại ở đây.
Tác phẩm mới công khai mặc định **bật** bình luận (vẫn tiền kiểm; chủ sản phẩm xác nhận
03/11/2026).

Thay đổi chế độ dùng **generation của ý định được chấp nhận** (chủ sản phẩm xác nhận
05/11/2026, theo sửa fence accepted intent của PR #79): yêu cầu mang đúng `expected_generation`
hiện tại luôn được chấp nhận và tăng generation, **kể cả khi chế độ đã đúng**; generation khác mà
chế độ đã đúng thì không ghi (an toàn khi gửi lại); generation khác và chế độ khác bị từ chối. Nhờ
vậy một yêu cầu cũ còn trên đường đi không thể đè lựa chọn mới hơn của chính tác giả, kể cả khi
lựa chọn mới chỉ là “giữ nguyên”. Luật cũ (cùng chế độ là no-op ở mọi generation) để lọt trace:
đang tắt@g1, tab A gửi bật@g1 đến muộn, tab B gửi tắt@g1 đến trước không tăng generation, A khớp
g1 và mở lại bình luận.

## 5. Bất biến được hiện thực ở lát D2T-02a

ID và mapping đầy đủ ở [registry](../development/discussion-core.md#registry-traceability-của-41).
Tóm tắt phần lát này chịu trách nhiệm:

- Hai tầng được giữ ở **cả Rust và database**: discriminator bất biến, FK ghép tới root cùng
  thread, context chỉ trỏ reply cùng root, trigger chặn đổi placement. Test ghi SQL trực tiếp.
- Hai trục **hiện diện** (còn/đã gỡ bởi người viết/kiểm duyệt) và **kiểm duyệt**
  (chờ/duyệt/từ chối) tách nhau; hiển thị công khai là kết quả của cả hai cộng quyền đọc tác phẩm.
- Root đã gỡ/ẩn nhưng còn phản hồi hiển thị trở thành điểm neo không có nội dung/danh tính;
  context bị ẩn hiển thị “không còn hiển thị”, không gọi mọi trường hợp là “tác giả đã xóa”.
- Gỡ bởi người viết xóa nội dung khỏi database; gỡ bởi kiểm duyệt giữ nội dung cho audit nhưng
  không đường đọc nào trả về.

## Trách nhiệm phải tự xây

Rời Discourse nghĩa là Vink tự sở hữu các trách nhiệm dưới đây. Chúng là gap trước khi mở bình
luận công khai cho pilot, không phải tính năng đang có:

| Trách nhiệm | Vink phải cung cấp hoặc chốt phạm vi |
| --- | --- |
| Tiền kiểm và quyết định người duyệt | **Đã có (D2T-02c):** grant kiểm duyệt do operator cấp/thu hồi, kiểm lại trong transaction quyết định; tự né bình luận của mình/tác phẩm của mình; hàng đợi có version và `pending_total`; audit append-only gắn grant và lý do. **Còn thiếu:** màn vận hành (UI), cảnh báo tuổi case/backlog, người trực (MOD-03) |
| Báo cáo, lý do, khiếu nại | **Đã có:** lý do bắt buộc khi từ chối/gỡ, người viết thấy lý do từ chối. **Còn thiếu:** report, appeal idempotent, mở lại bình luận đã gỡ, thông báo khi bị gỡ (MOD-04) |
| Chống lạm dụng | Rate limit, chống spam, khóa/ban có authority và audit |
| Thông báo reply/mention/kết quả duyệt | Outbox/inbox, dedupe, read state; pending/đã gỡ không lọt qua thông báo |
| Xóa, thu hồi, riêng tư | Rút bài chặn đọc/ghi thảo luận ngay; retention/purge; quote/preview/cache theo PUB-03/D-12 |
| Phục hồi | Backup/restore database App gồm receipts; đối soát; không replay mù hiệu ứng cũ |

## 6. Rollout và rollback

1. **D2T-02a/b/c (PR hiện tại):** domain + application + migration/adapter PostgreSQL, HTTP API
   v1 + contract fixtures (đọc, gửi, gỡ của mình, bật/tắt), rồi authority + hàng đợi + route kiểm
   duyệt. Ở production mọi lệnh ghi (kể cả kiểm duyệt) trả `401` vì chưa có auth authority; chưa
   có UI.
2. **Trước khi mở ghi production:** auth authority thật (D-05), màn duyệt và người trực
   (MOD-03), report/appeal (MOD-04), UI (#45), giới hạn tần suất. Thiếu auth là blocker, không thay bằng
   tài khoản fixture.
3. **Rollback trước khi có dữ liệu thật:** gỡ route; migration giữ nguyên (không sửa migration
   đã áp dụng). Sau khi có dữ liệu: tắt ghi bằng cách đóng chế độ hoặc gỡ route, giữ dữ liệu
   canonical; không chuyển ngầm sang hệ khác, không dual-write.
4. **Đổi owner sau này** cần ADR mới, export có version giữ ID/Unicode/context/tombstone/audit,
   dừng writer cũ, đối soát lệnh chưa rõ kết quả rồi mới bật writer mới.

## Nguồn và quan hệ tài liệu

- [#41](https://github.com/loveoverflowcom/vink/issues/41),
  [#43](https://github.com/loveoverflowcom/vink/issues/43): yêu cầu hai tầng, D2T-INV-01..11,
  PostgreSQL/API.
- Quyết định của chủ sản phẩm ngày 03/11/2026 trong phiên triển khai D2T-02a.
- [Requirements](../../prd/requirements.md): COM-01..08, MOD-02..05, PUB-03.
- [Decision log](decisions.md): D-02, D-05, D-08, D-12, D-13; [tech stack](tech-stack.md) §9.
- Lịch sử so sánh Discourse/Vink trước quyết định nằm trong Git history của tài liệu này
  (`develop@31a62b1`).
