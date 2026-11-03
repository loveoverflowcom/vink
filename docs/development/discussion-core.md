# Core cấu trúc bình luận hai tầng — D2T-01, lát cắt 1

**Baseline:** `develop@8045d45e554e0a01f11fd252ac681f8eb9d33a56`, ngày 03/11/2026.
Đã đồng bộ tiếp `develop@69057433d74ee0cf7c7b4c9bdddfe740398d1412` (#48) trước bàn giao;
giữ các thay đổi StatePanel/recovery, focus capture, skill CMP và workflow của upstream.
**Phạm vi:** phần cấu trúc có thể kiểm độc lập của [#41](https://github.com/loveoverflowcom/vink/issues/41).
Không hoàn tất issue và không thay owner thảo luận. [ADR đang đề xuất](../architecture/discussion-ownership.md)
giữ gate chọn owner trước persistence [#43](https://github.com/loveoverflowcom/vink/issues/43).

## HOW và WHY

[`DiscussionThread`](../../services/vink-app/src/domain/discussion.rs) là state thuần của một
thread do caller chọn, với hai lệnh tạo root/reply. ID do caller cung cấp qua nominal types
`ThreadId`/`CommentId`; core không sinh random, đọc clock, gọi I/O hoặc nhận actor từ DTO.
`RootCommentId` chỉ được lấy từ placement hợp lệ; không có constructor raw hoặc serde cho
placement và không có setter đổi thread/root/context sau tạo.

Tạo A, trả lời A bằng B rồi trả lời B bằng C: B và C đều có root A, B có context `None`, C có
context `Some(B)`. `create_reply(id, target_id)` tự tra target trong thread; không nhận
`root_id` do client cung cấp. Root luôn là cấp 1, mọi reply là cấp 2. `reply_to` là ngữ cảnh
giữa các peer, không được render thành level 3. Context chỉ trỏ comment đã có trước khi tạo;
ID không được dùng lại nên cả cấu trúc và context không có self-edge/cycle trong lát cắt này.

Target chỉ có ở thread khác hoặc chưa có trả `TargetNotFound`; ID có rồi trả `CommentExists`
trước khi xét target. Cả hai lỗi giữ nguyên state. Việc lấy lại ID cũ không reparent root/reply.
Đây không phải idempotency: retry cùng command vẫn lỗi duplicate, chưa có command key/receipt.

Giữ core trong app thay vì thêm crate/process vì chưa có consumer thứ hai hay bằng chứng cần
fault/scaling boundary mới. State dùng `Vec` và lookup tuyến tính, chỉ phục vụ quyết định cấu
trúc ở lát cắt này; không có public-list/ranking contract hoặc benchmark. Chưa có cách restore
raw records từ database, chưa request nào dùng core. Không được đưa aggregate volatile này
vào server rồi báo gửi thành công khi chưa có durable adapter.

## Ranh giới chưa được nối

- Caller phải chọn đúng thread/work/tenant, tải facts đáng tin và cấp ID; core chỉ biết state
  của một instance. Không chứng minh UUID duy nhất toàn hệ thống hay quyền actor.
- Trước commit, adapter phải kiểm lại authority/access/version; lookup trong snapshot không
  fence withdrawal/ban/callback chạy muộn. Schema/constraints và race tests PostgreSQL chưa có.
- Core không chứa body, author, structural removal, visibility, moderation, publish, edit,
  reaction, command receipt, time/rank, assets hoặc UI ack. Không có đường public read mới.
- Chưa chọn Discourse hay Vink làm write/moderation owner. `vink-app` sở hữu mã thuần này,
  không có nghĩa nó được quyền ghi một kho comments song song với Discourse.

## Registry traceability của #41

Đây là bảng mapping duy nhất cho D2T-INV trong repo. Nội dung yêu cầu đầy đủ do #41 sở hữu;
bảng chỉ dẫn tới owner và bằng chứng thực thi, không tạo bộ rule cạnh tranh. Requirements/ADR
sở hữu quyết định; model Quint sở hữu transition cấu trúc đã chọn; Rust hiện thực transition;
fixture sinh từ model là oracle để replay input qua Rust, không là implementation thứ hai.

| ID / nguồn | Model/action | Rust production / test | Kani | Boundary và gap |
| --- | --- | --- | --- | --- |
| D2T-INV-01 — #41, hai tầng | `structure.apply` với `op=root/reply`, `rootIsRoot`/`canonicalContext`/`noCycles` | `DiscussionThread::{create_root, create_reply}`, `reply_to_reply_keeps_one_root_and_flat_structure`, bounded enumeration và trace replay | NOT_RUN | Chỉ creation, chưa removal/restore/durable constraints |
| D2T-INV-02 — #41, cùng scope | Một thread là assumption của model; root/context cùng tập | Lookup `DiscussionThread::comment`, `a_foreign_or_missing_target_does_not_change_this_thread`, `target_selects_the_root_without_a_client_root_argument` | NOT_RUN | Thread instance và root bất biến; work/tenant binding, DB isolation chưa có |
| D2T-INV-03 — COM-03, INV-05 | Chưa có | Chưa có actor/access commit seam | NOT_RUN | Auth authority, generation và concurrent withdrawal chưa có |
| D2T-INV-04 — COM-02, MOD-02 | Chưa có | Chưa có content visibility/moderation/read projection | NOT_RUN | Pending/edit không public chưa được chứng minh |
| D2T-INV-05 — PUB-03 | Chưa có | Chưa có tombstone/removal | NOT_RUN | Root/reply/context deletion và privacy cần PR tiếp |
| D2T-INV-06 — COM-01/04 | Chưa có | Duplicate ID rejection không thay command idempotency | NOT_RUN | Command/payload receipt, lost ack và durable effect chưa có |
| D2T-INV-07 — MOD-02, INV-07 | Chưa có | Chưa có callback/job/generation seam | NOT_RUN | Stale/duplicate/out-of-order worker cần owner và fencing thật |
| D2T-INV-08 — COM-06 | Chưa có | Chưa có reaction/counter | NOT_RUN | Desired-state set/remove, cardinality và arithmetic chưa chọn |
| D2T-INV-09 — #41 | Chưa có | Không có rank/cursor/time projection | NOT_RUN | Thứ tự reply và root ranking không được suy từ vị trí Vec |
| D2T-INV-10 — #45 | Chưa có | Không đổi browser/native request state | NOT_RUN | Actor/session/draft snapshot ack cần consumer integration |
| D2T-INV-11 — #25 | Chưa có | Không có manifest/renderer reference | NOT_RUN | Google asset identity và text fallback thuộc #25/#45 |

Các symbol model, fixture và lệnh thực thi được chỉ mục ở
[owner-local behavior](../../services/vink-app/behavior/discussion/README.md).
Các hàng “Chưa có” vẫn là gap mở, không phải invariant đã PASS nhờ default state rỗng.

## Kiểm chứng theo lớp

Lệnh được sở hữu tại [development guide](README.md), runner ở `xtask`; CI gọi cùng lệnh.

1. Unit tests kiểm chain A/B/C/D, hai root, foreign/missing target, duplicate/self-reference,
   immutable placements và atomic reject. Compile-fail doctests kiểm type/serde boundaries.
2. Bounded enumeration gọi **Rust production** cho toàn bộ 20 lệnh mỗi bước, 4 ID và độ dài
   4: 160.000 chuỗi đủ bốn bước, 168.420 cạnh kể cả reject. Coverage assertions buộc đạt
   root/direct/contextual reply, duplicate/missing và chain có hai context hop. Đây là finite
   enumeration của code, không gọi là Kani hay proof cho mọi số comment.
3. Quint typecheck, deterministic scenarios, seeded simulation, model mutation counterexample
   và generated-trace drift là các check riêng. Trace replay chỉ đưa command input vào Rust,
   so outcome/state thực tế với oracle từ model. Simulation không là exhaustive model checking.
4. Kani **NOT_RUN**: release chính thức 0.68.0 cần `nightly-2026-08-21`; máy giữ duy nhất
   Rust stable 1.99.1 theo yêu cầu người dùng. Không cài compiler thứ hai hoặc thay proof bằng
   unit test rồi gọi tương đương. Muốn chạy cần môi trường proof riêng được cho phép, kiểm
   compatibility của crate và thêm harness/bounds/reachability gọi đúng production seam.
5. Apalache/bounded model checking, PostgreSQL/Discourse/API/browser integration **NOT_RUN**
   cho core mới. Không sửa các boundary đó trong PR này; model/core PASS không nghiệm thu chúng.

Nguồn toolchain Kani: [release 0.68.0](https://github.com/model-checking/kani/releases/tag/kani-0.68.0),
[compiler pin](https://github.com/model-checking/kani/blob/kani-0.68.0/rust-toolchain.toml) và
[setup](https://github.com/model-checking/kani/blob/kani-0.68.0/src/setup.rs).
Kết quả lệnh thực tế và nguồn đầu vào được ghi trong PR; sự có mặt của test/CI không tự là PASS.
