# Literary endorsement — quyết định của lát LVE-PR-01

**Ngày:** 05/11/2026. **Nguồn:** [#73](https://github.com/loveoverflowcom/vink/issues/73),
LVE-PR-01 do người dùng giao thực thi; baseline `develop@be2b69e490d7637f6021c4b97062ed3859e09189`.
Đây là lựa chọn triển khai có phạm vi **pure core PR1**, không phê duyệt toàn bộ epic, UI hay
production policy. Nội dung lõi liên quan trong snapshot Drive r1/issue được supersede bởi
[PRD canonical](../../prd/requirements.md#12-literary-endorsement-lve) và tài liệu này;
phần ngoài lát vẫn là đề xuất ở epic. Không đổi nghĩa các D/INV/COM hiện hành.

> **Cập nhật 10/11/2026:** chủ sản phẩm chốt hai cách dùng “Đánh giá cao” của **tác phẩm** để xếp
> hạng: feed Đề xuất (vote + độ mới) và Top 100 (vote) — [đặc tả đích](discovery-ranking.md), D-18.
> Đầu vào là tổng công khai của revision hiện tại như tài liệu này định nghĩa; “Hữu ích” của bình
> luận không bao giờ là vote của tác phẩm; thứ tự bình luận không đổi. Hai hệ quả còn `OPEN`: count
> bắt đầu lại khi đăng revision mới ảnh hưởng thứ hạng (O-07), và “Hữu ích” khi bình luận được sửa
> (O-06, [đặc tả](comment-editing.md)). Không lựa chọn nào của lát LVE bị đổi; chưa có code xếp hạng.

**Record (11/11/2026):** phạm vi D-14 tra tại `D-14@r1`. Lựa chọn của #73 tra theo lát:
`LVE-PR-01-CHOICE-*`, `LVE-PR-02-03-CHOICE-*` (#81) và `LVE-WP-04-CHOICE-10`, đều `DESIGN_PROPOSED`.
Các bảng dưới giữ lý do và lịch sử lúc chọn. CHOICE-06 (ranking) chưa chọn. CHOICE-07 (icon) cần
rà vì registry runtime ghi đã gắn glyph Pen Flight R2 A.

<!-- vdr: active D-14@r1 -->
<!-- vdr: active LVE-PR-01-CHOICE-01@r1 -->
<!-- vdr: active LVE-PR-01-CHOICE-02@r1 -->
<!-- vdr: active LVE-PR-01-CHOICE-03@r1 -->
<!-- vdr: active LVE-PR-01-CHOICE-08@r1 -->
<!-- vdr: active LVE-PR-01-CHOICE-09@r1 -->
<!-- vdr: active LVE-PR-01-CHOICE-11@r1 -->
<!-- vdr: active LVE-PR-02-03-CHOICE-03@r1 -->
<!-- vdr: active LVE-PR-02-03-CHOICE-04@r1 -->
<!-- vdr: active LVE-PR-02-03-CHOICE-05@r1 -->
<!-- vdr: active LVE-PR-02-03-CHOICE-12@r1 -->
<!-- vdr: active LVE-PR-02-03-CHOICE-13@r1 -->
<!-- vdr: active LVE-WP-04-CHOICE-10@r1 -->

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

**Còn OPEN (tại PR1):** CHOICE-04 storage/FK; 05 aggregate/read strategy; 06 ranking; 07 icon;
10 client confirmation; 12 voter visibility/consent. Sau đó 04/05/12 chốt A ở [#81](#lát-api--postgresql-81),
10 chốt A ở [lát Web adapter](#lát-web-adapter-wp-04-06112026); 06 và 07 vẫn OPEN. Core chỉ trả count và trạng thái riêng của actor,
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

## Lát API + PostgreSQL (#81)

Chọn cho lát gộp LVE-PR-02/03 (05/11/2026), port lên seam trên; không đổi semantics lõi. Data flow,
chữ ký, lock order và bằng chứng: [runtime](../development/literary-endorsement-runtime.md).

| ID | Chọn cho lát | Lý do / phương án bị loại |
| --- | --- | --- |
| CHOICE-03 (HTTP) | `PUT …/endorsement/me` mang trạng thái mong muốn, `command_key` (UUID của actor, trên mọi đích) và `expected_order`; `200 accepted` / `200 replayed` (lịch sử `committed` tách trạng thái hiện tại), `409` conflict, `422` key reuse | POST lệnh cũng mang cùng key/order nên không bớt gì; không expose hai API song song |
| CHOICE-04 | A — `work_endorsements` (PK work, revision, actor; FK revision cùng tác phẩm), `comment_endorsements`, `endorsement_receipts` (đích typed nullable + CHECK đúng một đích); token `1 ≤ version ≤ order`, `order ≥ 2`; trigger cấm đổi danh tính, order không tăng, version giảm, DELETE/TRUNCATE; receipt append-only | Một bảng state chung thêm NULL/CHECK mà không bớt truy vấn; `(target_type, target_id)` không FK bị loại |
| CHOICE-05 | A — đếm hàng `selected` khi đọc, partial index | Counter giao dịch tuần tự hóa người ghi trên đích nóng; outbox chưa có nhu cầu. Số đo: [bằng chứng](../development/evidence/lve-postgres/README.md) |
| CHOICE-12 | A — chỉ tổng số + trạng thái riêng; không endpoint/truy vấn nào trả danh sách người ghi nhận | B cần owner consent/retention/withdrawal, chưa có |
| CHOICE-13 | A — `GET …/endorsements` (công khai, không resolve actor) tách `GET …/endorsements/me` (`401` cho khách), batch ≤ 100 bình luận của một tác phẩm, đều `no-store`; READ-01/02 không đổi | Gộp làm lỗi Identity che tổng số và buộc thêm trạng thái người xem vào biểu diễn công khai |

Precondition ở mục trên được hiện thực như sau: capability hiện hành = principal có hàng (chưa có cơ
chế cấm/thu hồi); khóa publication → thread → bình luận → advisory lock (actor, key) → hàng của actor;
receipt đọc trong cùng transaction; state + receipt ghi nguyên tử; hàng bình luận chỉ được đọc khi
bình luận thuộc tác phẩm trên route. Retention receipt, purge tài khoản và thu hồi capability vẫn OPEN.

**Rà soát domain ↔ API ↔ DB sau #81/#83:** không đổi semantics hay hợp đồng HTTP. Trạng thái lưu
phải là trạng thái lõi đạt được (`selected ⇔ version chẵn`): `EndorsementState::restore` và CHECK
`*_version_parity_check` (migration tiến `20261105120000`) cùng giữ, vì tổng số công khai là SQL không
qua hydrate. Ma trận, quyết định và OPEN-R4 (replay Select khi đích không còn visible):
[runtime](../development/literary-endorsement-runtime.md#rà-soát-domain--api--db-73-sau-8183).

## UI System — phần tái sử dụng (WP-04, 06/11/2026)

Lát này chỉ chuẩn hóa thư viện UI để lát adapter tiêu thụ; không nối endpoint, không chọn icon.
Contract: [components](../../packages/ui-system/shared/components.md#toggleaction--hành-động-bậttắt-đã-xác-nhận-lve-73),
bằng chứng: [UI evidence](../development/ui-system-endorsement-evidence.md).

| Vấn đề | Chọn ở UI System | Lý do / còn OPEN |
| --- | --- | --- |
| Primitive | `ToggleAction` chung (Leptos/CMP) + `EndorsementAction` ở patterns giữ copy LVE | Bookmark/reaction sau này dùng lại được; không đưa `command_key`/order/DTO vào UI |
| Hiển thị trạng thái | Chỉ giá trị owner đã xác nhận; pending/uncertain là ghi chú, dấu ✓ sau ACK/readback | Khớp hướng A của CHOICE-10; hướng B cần `toggle-contract-v2`. CHOICE-10 cấp sản phẩm vẫn chốt ở lát adapter |
| Bấm | Phát trạng thái mong muốn (`!confirmed`), không phát khi pending/uncertain/chặn/chưa rõ | LVE-FR-03; uncertain → “Kiểm tra lại” (Reconcile), không replay mù |
| Tên truy cập | Theo hành động: “Đánh giá cao”/“Bỏ đánh giá cao”, “Hữu ích”/“Bỏ đánh dấu hữu ích”; không `aria-pressed` | Copy khởi đầu #73; tránh đọc “Bỏ…, đã nhấn”; tên chứa nhãn (WCAG 2.5.3) |
| Số đếm | `CountFact::{Hidden, Unavailable, Known}`; unavailable “–”, không 0; mỗi đích riêng | LVE-FR-02, LVE-UC-03 |
| Lý do chặn | Chữ hiển thị: tự đánh giá (CHOICE-08 A), không có quyền, chưa đăng nhập, không khả dụng | Không có nút đăng nhập giả khi chưa có auth |
| Glyph | Pen Flight R2 A · Mềm (chốt 09/11/2026; tích hợp 10/11) | CHOICE-07: vector Vink-owned, 20px trang trí trong slot đầu; tiến trình/✓ giữ slot cuối. [Spec/nguồn](../design/pen-flight-r2.md) |
| Theme | Recipe `action.toggle.*` (token 0.5.0) | Quiet, borderless; selected dùng secondary-container đã đạt contrast |

[Registry, model/proof evidence và gap](../development/endorsement-core.md) là owner duy nhất
traceability LVE; không gán proof lõi cho SQL, HTTP, UI hay toàn epic.

## Lát Web adapter (WP-04, 06/11/2026)

Reader Web tiêu thụ `EndorsementAction` cho tác phẩm (sau văn bản) và cho từng bình luận gốc/phản
hồi phẳng. Owner, luồng và bảng ánh xạ: [runtime](../development/literary-endorsement-runtime.md#web-adapter-wp-04);
bằng chứng: [web evidence](../development/literary-endorsement-web-evidence.md).

| ID | Chọn | Lý do / phương án bị loại / còn OPEN |
| --- | --- | --- |
| CHOICE-10 | **A** — không optimistic: bấm chỉ đặt `Pending` (ghi chú “Đang gửi…”, MicroArc); giá trị và số đếm chỉ đổi theo ACK `accepted`/`replayed` (trạng thái hiện tại `endorsement`, không phải `committed`) hoặc đọc lại own state; mất phản hồi/timeout/`503`/`500` → `Uncertain` chặn bấm, “Kiểm tra lại” replay đúng lệnh (cùng key, order, trạng thái mong muốn, revision); own state chưa đọc/đọc lỗi → `Unknown`, “Kiểm tra lại” đọc lại | B (optimistic có nhãn) cần phase mới + `toggle-contract-v2`, rollback hiển thị và fence ý định mới nhất xuyên thiết bị mà lõi không hứa; lợi ích là độ trễ cảm nhận của một nút nhỏ, không đáng rủi ro “đã ghi nhận” giả. Đánh giá lại nếu đo được độ trễ ghi thật làm người đọc bấm lặp |
| Đối soát Uncertain | Replay cùng key thay vì chỉ đọc own state | Replay trả `replayed` khi lệnh cũ đã commit, nên biết chắc; đọc own state không phân biệt được lệnh của mình với lệnh khác và để lệnh cũ có thể commit sau khi màn đã hiện. Own-state read dùng cho trạng thái `Unknown`, không cho lệnh mất phản hồi |
| `409 version_conflict` | Hiện `endorsement` quan sát được nếu order không cũ hơn; trùng ý định → `Idle`, khác → `Failed` (bấm lại là ý định mới tại order mới) | Không tự gửi lại ý định cũ với order mới (hợp đồng API) |
| R7 (gỡ riêng tư trên đích ẩn/revision cũ) | Không có UI: đích ẩn không được render, action của revision cũ bị chặn `Unavailable`; gỡ hàng giữ lại chỉ qua API | Hiện đích đã ẩn để gỡ sẽ làm lộ đích; cần thiết kế riêng nếu sản phẩm muốn danh sách “ghi nhận của tôi” |
| Khách | Tổng số công khai + lý do bằng chữ “Cần đăng nhập để…” trên từng action; không gửi own read hay lệnh; không nút đăng nhập | D-05 chưa có auth authority; LVE-UC-01 |

**Giữ OPEN:** CHOICE-06 ranking (không sắp xếp theo ghi nhận; replies
giữ thứ tự provider), CHOICE-12 B ghi danh, OPEN-R4. Production vẫn `401` cho ghi (D-05); lệnh thật
chỉ chạy với LocalFixture loopback, có nhãn trên trang.
