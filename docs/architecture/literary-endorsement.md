# Literary endorsement — quyết định của lát LVE-PR-01

**Ngày:** 05/11/2026. **Nguồn:** [#73](https://github.com/loveoverflowcom/vink/issues/73),
LVE-PR-01 do người dùng giao thực thi; baseline `develop@be2b69e490d7637f6021c4b97062ed3859e09189`.
Đây là lựa chọn triển khai có phạm vi **pure core PR1**, không phê duyệt toàn bộ epic, UI hay
production policy. Nội dung lõi liên quan trong snapshot Drive r1/issue được supersede bởi
[PRD canonical](../../prd/requirements.md#12-literary-endorsement-lve) và tài liệu này;
phần ngoài lát vẫn là đề xuất ở epic. Không đổi nghĩa các D/INV/COM hiện hành.

## Hợp nhất sau #78/#79

Theo yêu cầu merge PR #80, slice này tiếp nối lõi #78 đã vào develop. Chỉ giữ
`decide_endorsement` với accepted order/mutation version/receipt riêng và revision-specific target.
Hai module `work.rs`/`comment.rs` cùng model/counterexample cũ của #78 được thay bằng seam/model
hiện hành; requirements/registry URL cũ chuyển hướng về một owner. Đây là delta chủ ý: CHOICE-02 B
và CHOICE-09 A của #80 thay C/B tạm thời của #78. Không có transaction/UI consumer cũ cần migrate.
Giữ nguyên mọi production source/fixtures và harness sửa fence reaction/settings của #79.

## Review và phản biện trước khi triển khai

Bảng dưới ghi findings trên baseline ban đầu trước #79. #79 đã sửa riêng reaction/settings;
phần hợp nhất giữ nguyên các sửa đó, không dùng findings cũ để đảo chúng.

| Finding | Source/symbol | Hậu quả | Phương án | Quyết định slice |
| --- | --- | --- | --- | --- |
| No-op chỉ trả quan sát, không fence lệnh đến muộn | `discussion/reaction.rs::ActorReaction::desire`, #75 | OFF@1 đến trước không chặn ON@1 cũ | A: bắt client serialize; B: mọi acceptance có order + receipt | B, không reuse reducer reaction có semantics khác |
| Publication generation nằm trên hàng pointer có thể bị xóa | migration literary core `publications` | Delete/reinsert có thể reset generation; không là anchor bền | A: work + observed revision; B: immutable revision; C: stale/reconfirm | B cho work; không dùng generation/moderation_version làm content revision |
| Bình luận chưa có content revision/edit route | `lifecycle.rs`, migration `comments` | Không thể coi moderation_version là phiên bản văn bản | Original comment identity; hoặc thêm lifecycle edit | Identity cho original root/reply; edit UNSUPPORTED trong slice |
| `comments_open` là quyền tạo text | ADR discussion §4, COM-07/08 | Reuse gate reaction nhập ngầm policy đóng vote | Capability độc lập hoặc chặn vote khi đóng | Độc lập; close/reopen không đổi endorsement |
| Chỉ backend có consumer nghiệp vụ dự kiến | domain, application/ports | Shared crate/framework tăng surface chưa có lợi ích | Module local hoặc crate khi có consumer thứ hai | Module `domain::endorsement`, không thêm service/port giả |
| Receipt submission có CHECK/FK chỉ cho submit | migration discussion | Đổi chuỗi operation không đủ để dùng cho vote | Bảng/protocol riêng ở PR SQL | PR1 trả write intent; chưa có persistence/HTTP caller |

Code reaction và các yêu cầu đã chốt giữ nguyên. Một model PASS không chốt sản phẩm; phạm vi
của các lựa chọn dưới đây là contract để PR2/PR3 review/consume, không phải production acceptance.

## Lựa chọn và alternatives

| ID của #73 | Chọn cho PR1 | Lý do / đánh đổi / evidence cần thêm |
| --- | --- | --- |
| CHOICE-01 | A — một tín hiệu tích cực mỗi actor/target/anchor | Nhỏ, không có nhu cầu facets đã xác định. B tăng policy/schema; C appraisal riêng để sau. Work “Đánh giá cao”, comment “Hữu ích”; không điểm chuyên môn. Count không nhân vì reason. |
| CHOICE-02 | B — work theo `RevisionId` của publication; comment theo original `CommentId` | `revisions(work_id,id)` có FK, UPDATE bị trigger chặn. Rewrite tạo anchor mới với count riêng; A thừa kế ghi nhận qua rewrite, C cần content-change policy chưa có. Cùng revision được đăng lại giữ ghi nhận; không phụ thuộc số lần publication delete/reinsert. Không tự chuyển vote sang bản mới. |
| CHOICE-03 | B ở **core ordering/retry** — accepted-command order + receipt exact payload | Every accepted intent, kể cả same-state, tăng order. Mutation version riêng. A nhỏ hơn nhưng buộc client serialize mới tránh OFF/no-op trace; không dùng A để giảm invariant. Hình dạng HTTP POST/PUT vẫn OPEN cho PR API; contract identity/order/receipt phải giữ. |
| CHOICE-08 | A — chặn self endorsement | Không có utility self-mark riêng với bookmark. Work author vẫn có thể vote comment của người khác trên bài mình. |
| CHOICE-09 | A — độc lập quyền gửi text | COM-07 đóng root/reply create; không làm người viết tác phẩm có quyền xóa vote. Close, retry, reopen giữ cùng rules. Visibility/authority vẫn thắng. |
| CHOICE-11 | A — owner-local module trong `vink-app` | Chưa có consumer Rust thứ hai; reuse IDs, placement và lifecycle predicate thật; không tạo crate chỉ để gọi “abstract”. |

**Còn OPEN:** CHOICE-04 storage/FK; 05 aggregate/read strategy; 06 ranking; 07 icon;
10 client confirmation; 12 voter visibility/consent. Core chỉ trả count và trạng thái riêng của actor,
không có voter list; không được suy quyền công khai danh tính. Facets, chapter target, sửa comment,
account ban/removal/quarantine/retention, rate limit và auth authority chưa được triển khai.
D-05/08/12 giữ trạng thái hiện hành. Ranking/optimistic UI không nằm trong PR1.

## Contract của production seam

`services/vink-app/src/domain/endorsement.rs` là owner implementation. Hàm
`decide_endorsement(server_actor, command, facts)` chỉ nhận input, không I/O/clock/random,
không nhận actor/count từ client DTO. Không có route/store thực sự gọi hàm trong PR1.

- `EndorsementTarget::{Work {work,revision}, Comment {work,comment}}` giữ work/root/reply riêng.
  `EndorsementAnchor` là revision bất biến hoặc original comment. Root/reply dùng cùng luật,
  không tự cộng count lên root/work. Comment constructor kiểm placement thuộc thread;
  adapter phải chứng minh `thread.work_id == work`, revision thuộc work từ FK/locked lookup.
- `EndorsementState::INITIAL`: absent = OFF, mutation version 1, accepted order 1.
  `StoredEndorsement` bind actor + full target/anchor. Sau remove **giữ hàng OFF và order**;
  không delete/reset/reuse identity. Mỗi trạng thái lưu có `1 <= version <= order <= i64::MAX`.
- `EndorsementCommand` = key + full target/anchor + expected accepted order + Select/Remove.
  Key UUID từ caller, namespace `(server actor, endorsement operation, key)` trên mọi đích.
  Receipt giữ exact command (bao gồm expected token) và committed state, không chỉ desired bool.
- `Accepted(write)` luôn là **lệnh mới vừa được chấp nhận**: order +1; nếu đổi selected thì
  mutation version +1 và delta Add/Remove, nếu cùng trạng thái thì version giữ và delta None.
  No-op về selection **vẫn phải ghi fence và receipt**. Key mới không là retry.
- `Replay {committed,observed}` là **lệnh lịch sử đã commit**, không ghi lại/count lại và không
  có nghĩa selected hiện tại giống lịch sử. Receipt cũ phải có order <= current; bằng order phải
  cùng state. Payload khác cùng key → `KeyReused`; facts sai actor/target/key → `InvalidFacts`.
- `Conflict {observed}` chỉ quan sát trạng thái của đúng actor/đích, không nhận lệnh mới.
  Caller reconcile, **không tự refresh expected token rồi phát lại ý định cũ**. Sau conflict,
  muốn thể hiện ý mới phải do consumer/người dùng xác định; không hứa latest user intent xuyên
  thiết bị khi chưa có global creation ordering. Bảo đảm là first accepted CAS wins, stale không thắng.
- Refusal: authority unavailable, target unavailable, own content, invalid facts, key reuse,
  token exhaustion. Không ghi gì trên conflict/refusal/replay. Count là `i64 >= 0`, checked arithmetic;
  underflow/overflow là lỗi projection/integrity khiến **rollback toàn transaction**.

**Eligibility/effectiveness:** Select (kể cả selected=true cùng trạng thái) cần capability hiện
hành, target đúng scope, work đang public đúng revision hoặc comment present+approved trên work
public, và actor khác tác giả target. Bản revision cũ không được “reconfirm” bằng same-state ON;
phải chọn target của revision mới. Không có stale flag vì mỗi revision là relation riêng.
Selected lưu là lịch sử source; effective/public chỉ khi anchor hiện hành và target còn visible.
`public_count` trả `None` khi không visible, không giả 0. Comment pending/rejected/removed không lộ count.

**Own-removal ngoại lệ riêng tư:** actor có capability hiện hành được Remove đúng retained record
của mình dù target hidden/withdrawn/missing/old revision. Không có own record thì missing/hidden
vẫn `TargetUnavailable`. Response không trả author/text/count hay xác nhận target cho người khác.
Receipt Select cũ trên target hiện không visible không replay ra ngoài; Remove có own row thì replay
được. Capability bị thu hồi chặn cả remove/replay; khôi phục quyền cần authority thật, không fixture.

## Transaction preconditions cho PR2/PR3

Đây là nghĩa vụ **chưa có implementation/evidence**. Không copy luật vào HTTP/SQL như reducer thứ hai.

1. Resolve principal/capability phía server. Serialize với revoke; tải work/publication/thread/comment
   từ owner App, đúng thứ tự khóa hiện tại: publication → thread → target. Trường hợp own removal
   target đã mất cần khóa record actor/target và retained identity mà không lộ target. Thiết kế khóa
   bổ sung phải giải quyết absence races, republish/withdraw và deadlock trong PR SQL.
2. Serialize state và receipt namespace: actor+target+anchor có unique/CAS; actor+operation+key unique
   **trên mọi đích**. Concurrent cùng key khác target phải có một winner; không chỉ khóa target.
   Receipt/state đọc cùng transaction; không load từ cache/client. Chưa có receipt retention policy:
   giữ receipt/fence, không expire rồi tuyên bố historical replay vẫn được nhận diện.
3. Recheck target visibility, exact revision/work binding, capability tại durable boundary, rồi gọi
   hàm thuần. Với `Accepted`, ghi retained row (kể cả delta None), checked count hoặc source-derived
   aggregation theo CHOICE-05 và receipt atomically. Projection không là ACL/publication authority.
4. Chống reset khi remove/recreate, publish delete/reinsert, account purge/restore. UUID và immutable
   revisions không được tái sử dụng; comment body edits cần revision/anchor policy mới. Schema hiện tại
   chỉ chặn UPDATE revision, chưa chứng minh lifecycle purge/reuse; comment body không có SQL immutable
   trigger. Không mở write/edit integration trước khi migration/adapter bảo vệ các preconditions này.
5. Response/readback phân biệt accepted/replay/conflict; private actor state `no-store`, public count
   không có viewer identity; current visibility override receipt/projection. Retention/privacy/ranking
   vẫn cần quyết định riêng. Unit/model/Kani không chứng minh các nghĩa vụ SQL/auth/browser này.

## Trace đối kháng

| Input theo thứ tự đến | Kết quả contract |
| --- | --- |
| absent OFF/v1/order1; B=OFF@1 đến trước A=ON@1 | B accepted unchanged: v1/order2, delta0 + receipt; A conflict, không ON |
| ON@1 commit; OFF@2 commit; replay ON cùng key/payload | committed ON/v2/order2; observed OFF/v3/order3, không ghi |
| ON cũ với key khác sau OFF | Conflict; không tự lấy order hiện tại phát lại |
| Remove rồi Select lại | Giữ hàng OFF; order tăng tiếp, không reset về initial |
| selected=true của revision cũ; publication sang revision mới | ON target cũ unavailable; target revision mới độc lập; Remove own cũ vẫn được |
| target bị ẩn/rút; own Remove | Chỉ own retained row + current authority; public count None |
| work/comment/actor binding sai | InvalidFacts, không mutation; HTTP mapping privacy-safe cần PR API |
| order/version/count hết miền | Refuse/checked count error, không wrap hay partial commit |

[Registry, model/proof evidence và gap](../development/endorsement-core.md) là owner duy nhất
traceability LVE; không gán proof lõi cho SQL, HTTP, UI hay toàn epic.
