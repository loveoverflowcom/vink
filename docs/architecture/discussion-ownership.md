# Ownership của thảo luận hai tầng

**Ngày:** 03/11/2026 · **Trạng thái:** Đề xuất có phạm vi; chưa Accepted.
**Work item:** D2T-01, [#41](https://github.com/loveoverflowcom/vink/issues/41).
**Baseline khảo sát:** `develop@8045d45e554e0a01f11fd252ac681f8eb9d33a56`.

Tài liệu này so sánh owner trước khi triển khai persistence, không chuyển ownership production.
[D-02](decisions.md) vẫn là **Đề xuất ưu tiên**: Vink sở hữu tác phẩm, Discourse sở hữu
thảo luận. Chưa có discussion backend, kho comments, auth authority hay integration Discourse
được xác minh tại baseline. Việc thêm core Rust thuần không thay đổi những giới hạn đó.

## 1. Câu hỏi và phạm vi PR đầu

Người đọc trả lời một bình luận trong tác phẩm. Giao diện chỉ có bình luận gốc và một danh sách
phản hồi phẳng. Trả lời một reply vẫn thêm một reply cùng cấp; `reply_to` giữ ngữ cảnh, không
tạo thêm cấp cây. Thread gắn với work/chapter ổn định, không tự đổi khi xuất bản revision mới.

Cần trả lời hai câu hỏi riêng:

1. Hệ thống nào có quyền ghi comments/reactions và áp quyết định moderation cuối cùng?
2. Nếu Vink giữ quyền đó, module trong app có đủ không, hay cần process riêng?

PR đầu có thể xây và kiểm **core cấu trúc thuần, nằm trong owner hiện có `vink-app`**: kiểu ID,
root/reply, chuẩn hóa target và kiểm tham chiếu cùng thread/root. Đây là seam để application
thật sử dụng về sau, không phải reducer thứ hai chỉ dùng cho test. Core nhận facts/input tường
minh; không tự tải database, gọi Discourse, đọc đồng hồ, sinh ID hay nhận quyền từ client.

Vị trí của lát cắt là `services/vink-app/src/domain/discussion.rs`: `DiscussionThread` giữ
placements riêng, `create_root`/`create_reply` tra target ID trong chính thread. Chưa có request
production nào gọi core. Không có bodies, visibility, moderation, auth hay reactions trong
lát cắt này; unit tests và bounded enumeration chỉ kiểm những luật cấu trúc đã hiện thực.

**Không mở comment write store/API trong PR này.** Chưa tạo bảng comments, endpoint gửi bình
luận, worker moderation hoặc luồng public read. Kết quả core hợp lệ chỉ nói về cấu trúc trong
phạm vi facts được cung cấp, không có nghĩa actor đã được xác thực, comment đã commit hoặc
nội dung được phép công khai. Chọn owner vẫn là prerequisite của
[#43](https://github.com/loveoverflowcom/vink/issues/43), không được suy ra từ vị trí file Rust.

## 2. HOW: một thành công và một lần bị từ chối

Ví dụ dưới đây dùng ký hiệu, không phải HTTP contract đã triển khai:

```text
Thread T
  Root A
    Reply B: root=A, reply_to=None
    Reply C: root=A, reply_to=Some(B)
```

**Luồng hợp lệ ở core:** tạo A trong T; dùng target A đã được kiểm để tạo B; dùng target B
đã được kiểm để tạo C. Core chuẩn hóa C về root A và giữ B làm context. B và C cùng cấp cấu
trúc; không duyệt `reply_to` như cạnh cây. Quy ước cục bộ: trả lời trực tiếp root dùng
`reply_to=None`, trả lời reply dùng `Some(reply_id)`; không nhận thêm cách mã hóa root ID
trong context. Đây chưa phải cam kết về representation của provider API.

**Luồng bị từ chối ở core:** yêu cầu thêm C vào T nhưng target B chỉ tồn tại trong thread U,
hoặc dùng lại ID đã có. Core trả lỗi, không có trạng thái mới được chấp nhận. Root không là
input của lệnh reply: target trong T tự chọn root, nên không có tham số context/root giả để
ghi chéo hai root. Các test phải gọi đúng
seam production và kiểm trạng thái không đổi; dữ liệu expected không được đưa thẳng vào reducer.
Trường hợp self-reference/root trỏ reply cũng cần negative scenario tại chính ranh giới này.

**Race chưa được core cấu trúc giải quyết:** server tải B khi còn quyền, sau đó tác phẩm bị
rút hoặc actor bị khóa trước commit. Application/owner cuối cùng phải đọc lại authority và
access/version facts theo cơ chế transaction/fencing đã chọn. Đưa một snapshot hợp lệ vào
hàm thuần không chứng minh write muộn đã bị chặn. PostgreSQL, direct Discourse, cache/SSR và
notification cần kiểm riêng; trước khi có các gate này không bật gửi bình luận.

## 3. Phương án A: Discourse là owner duy nhất

- **Trải nghiệm:** Vink render hai tầng và context từ projection; Discourse sở hữu comment,
  revision, reaction được chọn, review/flag và mutation tương ứng. Rust giữ policy/validation,
  mapping và adapter, không được gọi proxy là owner dữ liệu Discourse.
- **Ranh giới:** Vink giữ subject → thread mapping, identity mapping và projection tối thiểu;
  không lưu một kho comments có quyền ghi cạnh tranh. Browser không quyết định actor hoặc
  root. Mọi tích hợp qua API/event có owner, không ghi chéo SQL vào Discourse.
- **Commit và recovery:** publication Vink và discussion Discourse không cùng transaction.
  Outbox/inbox, idempotency tại đích hoặc lookup/reconcile, subject generation và receipt phải
  xử lý cả tạo thành công nhưng mất ack. Local lock không fence được HTTP request cũ ở đích.
- **Tương thích và vận hành:** cần pin Discourse/plugin/config, bảo vệ mọi cửa đang bật
  (Vink/native/API, staff/system; tắt email/import nếu không hỗ trợ), giữ direct access trong
  phạm vi tiền kiểm/thu hồi. Vẫn vận hành hai hệ và bridge identity, nhưng tận dụng được staff
  tools nếu chúng thực sự đáp ứng nhu cầu.
- **Bằng chứng hiện có:** đây là hướng ưu tiên trong [tech stack](tech-stack.md), chưa phải
  kết quả runtime của Vink. S-02/S-03/S-04 chưa hoàn tất.
- **Bất định lớn nhất và phép thử phân biệt:** trên release/plugin pinned, thử create/edit
  pending qua từng cửa đang bật, reply-to-reply, direct read sau withdrawal, mất ack và replay
  generation cũ. Chỉ giữ phương án nếu các gate đạt và lợi ích staff tools bù được bridge.
- **Rút lui:** khi chưa có dữ liệu, bỏ adapter thử nghiệm không cần migrate comments. Sau khi
  có dữ liệu, tắt write tại một owner rồi export/đối soát theo kế hoạch chuyển owner ở mục 6;
  không cho Vink nhận write song song để che lỗi Discourse. Bài công khai vẫn đọc được, phần
  thảo luận báo gián đoạn trung thực theo COM-04.

## 4. Phương án B: Vink/Rust sở hữu comments và moderation

- **Trải nghiệm:** Vink kiểm soát trực tiếp hai tầng, revision, visibility, reactions và
  review. Discourse không còn là nơi ghi cùng một bình luận hoặc áp quyết định cạnh tranh.
- **Ranh giới:** domain thuần → application → persistence/API/event adapters thuộc owner.
  Subject/thread mapping, actor facts, revision/generation và idempotency có contract rõ.
  Không dùng DTO thay domain value; không tạo package contracts nghiệp vụ toàn cục.
- **Commit và recovery:** mutation + command receipt + outbox ghi cùng transaction của owner
  khi được thiết kế như vậy; database cần constraints và concurrency tests độc lập với Rust.
  Cùng command/payload chỉ một hiệu ứng, khác payload bị từ chối; timeout là uncertain, không
  tự tạo lại. Presence cấu trúc, visibility và moderation là các trục riêng.
- **Tương thích và vận hành:** giảm bridge comments nhưng Vink phải xây/vận hành phần cộng đồng
  từng kỳ vọng ở Discourse. Đây là thay đổi phạm vi D-02 và COM-01/02/04, không chỉ đổi một
  adapter. COM-03, MOD-02..05, PUB-03 và INV-05..08 vẫn giữ các bảo đảm liên quan.
- **Bằng chứng hiện có:** yêu cầu hai tầng và khảo sát Rust-owned core trong #41; kiến trúc
  app/worker và PostgreSQL theo owner đã có. Chưa có evidence cho comment transactions,
  moderation runtime, notification, hiệu năng hoặc đội vận hành các trách nhiệm mới.
- **Bất định lớn nhất và phép thử phân biệt:** làm một vertical slice được giới hạn sau khi
  chọn owner: pending → human decision đúng revision → đọc được phép → tombstone/withdraw;
  cố tình mất ack, race quyền và callback cũ. Review cả khả năng staff xử lý report/appeal và
  backlog, không chỉ latency hoặc test graph hai tầng.
- **Rút lui:** trước persistence có thể bỏ hoặc sửa core mà không migrate dữ liệu. Sau
  persistence, tắt write khi có lỗi không đồng nghĩa chuyển authority về Discourse; phải có
  mapping/export/retention và verification đã chuẩn bị trước. Giữ nguồn canonical để phục hồi.

### Trách nhiệm phải thay thế nếu rời Discourse

Đây là khoảng trống cần có owner và bằng chứng trước pilot, không phải các tính năng đang có
trong Vink. Không được âm thầm giảm yêu cầu để đạt một core demo:

| Trách nhiệm | Vink phải cung cấp hoặc chốt phạm vi thay thế |
| --- | --- |
| Tiền kiểm, sửa và quyết định người duyệt | Giữ pending khỏi mọi bề mặt; review queue, version guard, quyền staff/system, audit và công cụ vận hành theo COM-02/MOD-02 |
| Báo cáo, lý do, khiếu nại | Tiếp nhận report, xử lý abuse, giải thích và appeal có idempotency; người phụ trách, quyền mở lại và lịch sử quyết định theo MOD-03/04 |
| Chống lạm dụng và quyền cộng đồng | Rate limits, chống spam, suspension/ban, audit; trust signals nếu còn dùng phải có nguồn, policy và khả năng sửa sai |
| Thông báo reply/mention/review | Nguồn sự kiện, dedupe, read state và fan-out theo quyền; pending/withdrawn không lọt body/snippet/danh tính qua notification |
| Xóa, thu hồi, lịch sử và riêng tư | Tombstone không cascade mất replies ngang cấp; retention/purge, quote/context/cache/search và direct reads tuân PUB-03/D-12 |
| Khả năng phục hồi | Export/import IDs, account/thread mappings, backup/restore, reconciliation, metrics/cảnh báo và người vận hành; không replay mù side effects cũ |

## 5. Module-first hay process riêng là lựa chọn triển khai của B

**Khuyến nghị cho core đầu tiên:** đặt trong app hiện có, giữ seam thuần và không thêm process.
Điều này phù hợp D-03; không ngầm chọn B làm authority. Nếu A được chọn, chỉ các luật đã được
kiểm tương thích mới dùng trong policy/projection của adapter, không dùng core để override
quyết định Discourse.

| Bố trí | Lợi ích cụ thể | Chi phí/rủi ro | Bằng chứng cần trước khi đổi |
| --- | --- | --- | --- |
| Module trong `vink-app`, worker hiện có | Dùng một composition root, gọi nội bộ, ít deploy/contract mạng; authority app-local dễ kiểm tại commit | Scale/deploy/fault domain còn chung; cần bảo vệ boundary trong code | Baseline cho thử nghiệm; chưa có tải hoặc failure case buộc tách |
| Process `vink-discussion` riêng | Có thể cô lập tải ghi/worker, lỗi hoặc lịch deploy discussion | Thêm network auth, compatibility, retry, observability, vận hành và ranh giới nhất quán với publication/access | Đo được contention/scaling hoặc lỗi lan truyền cần isolation; chỉ rõ process mới giải quyết gì, cost và runbook rollback |

Nếu sau này chọn process riêng: service sở hữu contracts, migrations, database/schema và
runtime/migrator roles tách quyền; không đọc/ghi trực tiếp App hoặc Identity DB. Có thể dùng
container PostgreSQL hiện có với database/roles độc lập nếu phù hợp, không mặc định thêm
cluster/container. Event bất đồng bộ từ App không bảo đảm withdrawal tức thời; phải chốt
đường authority/fencing và phạm vi nhất quán trước khi bật write. Emoji/sticker là asset và
renderer theo [#25](https://github.com/loveoverflowcom/vink/issues/25), không cần emoji service.

## 6. Rollout, migration và rollback an toàn

1. **PR nền tảng:** core/model/tests chạy với input hữu hạn, không bật I/O comments. Không có
   dữ liệu discussion để migrate; rollback code không đổi owner hoặc dữ liệu production.
2. **Trước #43:** chọn chính xác một owner, cập nhật D-02 và requirement bị ảnh hưởng có chủ
   đích; chốt contract, authority/read/write gates và trách nhiệm vận hành. D-05/D-08/D-12
   không tự Accepted theo quyết định này. Thiếu auth authority thật vẫn chặn API write production.
3. **Pilot:** thêm adapter/schema chỉ cho owner đã chọn; kiểm boundary thật rồi mới mở luồng
   gửi có giới hạn. Có cách tắt write mà không giả ack thành công; không chuyển ngầm sang owner
   khác khi lỗi. Quyền đọc và thu hồi hiện tại luôn thắng cache/projection cũ.
4. **Nếu chuyển owner sau khi có dữ liệu:** kiểm inventory và export có version; bảo toàn IDs,
   subject/account mapping, Unicode, context/tombstones, review/audit và retention. Dừng writer
   cũ, xử lý lệnh uncertain/outbox đang bay, đối soát điểm chốt; import và kiểm quyền/đếm/hash
   phù hợp trước khi bật writer mới. Shadow read/projection không có quyền quyết định hay ghi.
5. **Sau khi writer mới đã nhận dữ liệu:** rollback cần đối soát delta bằng migration ngược đã
   thử hoặc phục hồi về điểm nhất quán có chính sách dữ liệu rõ; không đổi route về snapshot
   cũ rồi mất comments/decisions mới. Nếu chưa có đường phục hồi đáng tin, tắt write và giữ
   trạng thái gián đoạn, ưu tiên bảo toàn nguồn canonical thay vì dual-write.

## 7. Quyết định còn mở và evidence cần chốt

**Câu hỏi trước persistence:** giữ **A, Discourse là discussion write/moderation owner duy
nhất**, hay chọn **B, Vink là owner duy nhất**, chấp nhận đầy đủ trách nhiệm thay thế ở mục 4?
Nếu chọn B, khởi đầu module-first; chỉ tách process khi có bằng chứng ở mục 5. Quyết định phải
ghi người/quyền quyết định, phạm vi, ngày và evidence; merge core hoặc proof PASS không thay
cho lựa chọn này.

Hiện giữ D-02 là hướng ưu tiên; khảo sát B theo #41 bằng các lát thuần có thể đảo ngược. Kết
quả S-02 không đạt trên những cửa bắt buộc, hoặc chi phí bridge cao hơn lợi ích staff tools
được quan sát, là lý do xem lại A. Ngược lại, thiếu khả năng vận hành review/report/appeal/abuse
ở Vink là lý do chưa mở B, dù model cấu trúc đã đạt.

| Lát/evidence | Có thể làm độc lập ở PR đầu | Điều chưa được nghiệm thu |
| --- | --- | --- |
| Cấu trúc hai tầng | Rust seam thuần, positive/negative cases cho D2T-INV-01/02 và normalization | Same-scope theo facts không chứng minh auth, tenant isolation hoặc constraint DB |
| Model/proof/conformance | Model hữu hạn, harness gọi code thật, trace replay/guard sensitivity nếu runner đã triển khai; ghi riêng lệnh, version, bounds và kết quả | Không suy từ simulation sang exhaustive proof, từ model sang PostgreSQL/Discourse/browser hoặc từ PASS sang Accepted |
| Owner và persistence | So sánh và ghi gate trong tài liệu này | Acceptance chọn một owner của #41 vẫn mở; #43 chưa được mở persistence bằng sự tồn tại của core |
| Vòng đời và giao diện | Ghi rõ gap trong registry/traceability do #41 sở hữu | Các phần chưa có của D2T-INV-03..11; auth, moderation/withdrawal, idempotency, reaction/ranking, assets, UI và I/O cần các PR tiếp theo |

Không lập một oracle hay registry invariant thứ hai ở ADR này. Kết quả từng check thuộc tài
liệu evidence của implementation; tài liệu ownership không tự ghi PASS cho Kani, Quint,
PostgreSQL, Discourse hay browser.

## Nguồn và quan hệ tài liệu

- [#41 và comment bàn giao](https://github.com/loveoverflowcom/vink/issues/41#issuecomment-6598966340):
  yêu cầu hai tầng, một owner, phân biệt SSOT và các loại bằng chứng.
- [#43](https://github.com/loveoverflowcom/vink/issues/43): prerequisite ownership và phạm vi
  PostgreSQL/API; DDL comments chỉ áp dụng nếu chọn B.
- [Requirements](../../prd/requirements.md): COM-01..06, MOD-02..05, PUB-03.
- [Decision log](decisions.md): D-02/03/05/08/12, INV-05..08, S-02/03/04.
- [Tech stack](tech-stack.md): Hybrid và các trách nhiệm/boundary Discourse đang được đề xuất.
- [Ranh giới PostgreSQL/Identity](identity-postgres-boundaries.md) và
  [development](../development/README.md): hiện trạng owner, code và verification thực tế.
