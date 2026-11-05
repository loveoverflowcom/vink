# Ghi nhận văn học (LVE) — decision record lõi

**Ngày:** 05/11/2026 · **Policy:** `lve-core-v1` · **Epic:** [#73](https://github.com/loveoverflowcom/vink/issues/73)
· **Trạng thái:** các lựa chọn dưới đây được **chọn cho lát lõi** (pure Rust + Quint + Kani) với bằng
chứng; chủ sản phẩm chưa xác nhận hệ quả sản phẩm của CHOICE-02, 08 và 09. Không lựa chọn nào tự
thành Accepted vì đã được viết hoặc kiểm chứng.

Yêu cầu và ID: [PRD LVE](../../prd/literary-endorsement.md). Code, bằng chứng và gap:
[registry](../development/literary-endorsement-core.md). Code owner:
[`domain/endorsement`](../../services/vink-app/src/domain/endorsement.rs). Quyết định này không đổi
D-01..D-13, INV-01..08, COM-07/08 hay semantics reaction hiện có.

## 1. Lựa chọn

| CHOICE | Trạng thái | Lựa chọn | Lý do và phương án bị loại | Bằng chứng |
| --- | --- | --- | --- | --- |
| 01 Ý nghĩa/cardinality | Chọn cho lõi | **A**: một tín hiệu dương mỗi (actor, đích), chọn hoặc không | Đủ cho pilot, ít giải thích. B (facet lý do) vẫn OPEN như phần mở rộng: facet không được đổi `Endorsement` hay số đếm. C (thẩm định có cấu trúc) để sau | Unit test, `countsMatchSelections` |
| 02 Anchor qua các bản | Chọn cho lõi | **C**: quan hệ theo tác phẩm, ghi **revision người đọc đã đọc**; chỉ hiệu lực (được đếm) khi revision đó là publication hiện hành; bản mới làm ghi nhận cũ thành “bản trước”, không đếm, cho tới khi người đọc ghi nhận bản mới | A chuyển ngầm ghi nhận sang chữ mới (vi phạm LVE-UC-10) trừ khi thêm luật hiệu lực, tức là C. B cần một hàng và một version cho mỗi revision nên mỗi người có nhiều chuỗi fence; tổng hiệu lực giống C. Không dùng `publications.generation` (xóa/chèn lại con trỏ có thể bắt đầu lại) hay `moderation_version` (không phải revision chữ). Bình luận không sửa được (D-08) nên chính bình luận là anchor | `noSilentTransfer` + counterexample `endorsement-transfer`; Kani work admission |
| 03 Thứ tự/thử lại | Chọn cho lõi | **B′**: version của ý định được chấp nhận. Mỗi yêu cầu mang đúng version hiện tại được chấp nhận và tăng version 1, **kể cả khi trùng trạng thái** (reaffirm). Không có receipt: thử lại được nhận ra bằng hiệu ứng (`AlreadyCurrent`) | A (CAS, no-op khi trùng trạng thái) thua trace phản biện kể cả khi client tuần tự hóa: ACK mất, yêu cầu cũ còn trên mạng, đọc lại không chặn được nó. B đầy đủ với receipt không cần cho bất biến của lõi; giữ OPEN nếu cần audit “lệnh của tôi đã được chấp nhận chưa” | `noRevival` + counterexample `endorsement-unfenced`, `endorsement-version-reset`; Kani hai bước |
| 04 Lưu trữ/FK | OPEN (lát PostgreSQL) | — | Lõi đòi: FK đúng loại đích, unique (actor, đích), version ≥ 1, **giữ hàng sau khi bỏ chọn** | §4 |
| 05 Tổng hợp/read model | OPEN | — | Delta theo anchor và `EndorsementCount` không âm/tràn dùng được cho đếm-khi-đọc, counter giao dịch hoặc projection outbox | Kani counts |
| 06 Xếp hạng | OPEN | — | Lõi không sắp xếp gì; phản hồi giữ thứ tự thời gian | — |
| 07 Icon | OPEN | — | Không thuộc lõi | — |
| 08 Tự ghi nhận | Chọn cho lõi | **A**: tác giả tác phẩm/người viết bình luận bị từ chối mọi ý định trên nội dung của mình, nên không bao giờ có hàng của họ | B (đánh dấu riêng) chưa có utility khác Lưu bài | `noSelfEndorsement`, Kani admission |
| 09 Tắt bình luận | Chọn tạm cho lõi | **B**: tắt chặn ghi nhận **mới** trên bình luận; gỡ và reaffirm vẫn được; không ẩn gì; ghi nhận tác phẩm không bị ảnh hưởng; mở lại không đổi dữ liệu | Tôn trọng quyền kiểm soát duy nhất của tác giả với hoạt động thảo luận (COM-07) mà không ẩn nội dung; nới về A sau dễ hơn siết lại. Không suy từ reaction: reaction là đề xuất riêng | `eligibleWrites`, scenario `comment_targets`, Kani comment admission |
| 10 Xác nhận phía client | OPEN | — | Lõi đặt nghĩa vụ client ở §5 | — |
| 11 Owner thuần | Chọn | **A**: module `domain/endorsement` trong `vink-app` | B (crate dùng chung) chưa có consumer Rust thứ hai | `verify quick` kiểm import boundary |
| 12 Hiển thị người ghi nhận | Hệ quả của lõi | Chỉ tổng số + trạng thái riêng của chính người đó; lõi không có projection danh sách người ghi nhận | B (ghi danh tự nguyện) OPEN; không tự đưa người dùng hiện có vào | — |

Mọi thay đổi ý nghĩa (ví dụ coi hai revision cùng `content_hash` là cùng anchor, hay đổi CHOICE-08/09)
là một policy version mới, không sửa `lve-core-v1` tại chỗ.

**Hệ quả CHOICE-02 C cần chủ sản phẩm xác nhận:** đăng bản sửa lỗi chính tả làm số đếm của bản mới
bắt đầu từ 0, người đã ghi nhận thấy “bạn đã đánh giá cao bản trước”. Đăng lại đúng revision cũ
khôi phục hiệu lực mà không ghi gì. Coi bản tương đương nội dung là cùng anchor là refinement OPEN.

## 2. Semantics lõi (`lve-core-v1`)

Một yêu cầu gồm trạng thái mong muốn (chọn/bỏ; với tác phẩm kèm revision đã đọc) và `expected` —
version mà client đã thấy. Actor và đích do server lấy từ session và route, không lấy từ body.

**Version của ý định được chấp nhận.** Người chưa từng thao tác có `Endorsement::ABSENT`
(không chọn, version 1). Kết quả của `Endorsement::transition`:

| Kết quả | Khi nào | Ghi? | Client hiểu |
| --- | --- | --- | --- |
| `Changed` | `expected` = version, lựa chọn khác | Có: lựa chọn mới, version + 1, delta số đếm | Ý định mới được chấp nhận |
| `Reaffirmed` | `expected` = version, lựa chọn đã đúng | Có: chỉ version + 1, số đếm không đổi | Ý định mới được chấp nhận; mọi yêu cầu cũ hơn bị chặn |
| `AlreadyCurrent` | `expected` ≠ version, lựa chọn đã đúng | Không | Trạng thái đang quan sát đã thỏa yêu cầu; **không** phải chấp nhận mới, không chặn gì |
| `Stale` | `expected` ≠ version, lựa chọn khác | Không | Xung đột: một ý định được chấp nhận sau góc nhìn của client; cần đối soát từ `current` |
| `VersionExhausted` | `expected` = version = `i64::MAX` | Không | Lỗi nội bộ, không bao giờ là thành công |

Phân biệt bốn khái niệm: **trạng thái đang quan sát** (lựa chọn + version trả về), **ý định vừa được
chấp nhận** (`Changed`/`Reaffirmed`), **command lịch sử đã commit** (không lưu riêng: không có
receipt; một lần thử lại chỉ thấy hiệu ứng) và **xung đột cần đối soát** (`Stale`,
`RevisionChanged`). Server không bao giờ tự cập nhật `expected` để phát lại một ý định cũ.

**Đủ điều kiện, theo thứ tự** (`decide_work_endorsement`, `decide_comment_endorsement`):

1. Đích phải đúng hàng server đã nạp (tác phẩm; bình luận trong thread của đúng tác phẩm) và đang
   công khai: tác phẩm có publication hiện hành; bình luận đã duyệt, còn hiện diện, tác phẩm công
   khai. Ngược lại `TargetUnavailable` cho **mọi** ý định, kể cả bỏ chọn: không lộ đích ẩn, ghi nhận
   đã lưu vẫn còn nhưng không hiệu lực/không hiển thị, có lại hiệu lực khi đích công khai trở lại.
   Phản hồi dưới gốc đã ẩn vẫn ghi nhận được khi chính phản hồi còn công khai.
2. Tác giả/người viết của đích: `OwnWork`/`OwnComment` (CHOICE-08 A).
3. Tác phẩm, ý định chọn: revision đã đọc khác publication hiện hành → `RevisionChanged`
   (không ghi). Bỏ chọn không cần revision.
4. Bình luận, lựa chọn **mới** khi tác giả đã tắt bình luận → `CommentsClosed` (CHOICE-09 B).
5. `Endorsement::transition` quyết định theo version.

**Projection.** `Summary::Unavailable` cho đích ẩn, không bao giờ là `Count(0)`; tổng số tác phẩm
chỉ đếm lựa chọn ở revision hiện hành. `OwnView` (`Unavailable`, `NotEndorsed`, `Endorsed`,
`EndorsedEarlierRevision`) chỉ dành cho chính actor, không vào cache dùng chung. Số đếm là projection:
không ACL, xuất bản, kiểm duyệt, tuyển chọn hay xếp hạng nào đọc nó.

## 3. Trace phản biện

| Trace | Kết quả theo `lve-core-v1` | Bằng chứng |
| --- | --- | --- |
| OFF/v1; A = chọn@v1 tạo trước nhưng đến muộn; B = bỏ@v1 đến trước | B `Reaffirmed` → version 2; A `Stale`. Nếu no-op không tạo fence (CAS thuần), A thắng | Scenario `reorder_and_retry` (giao stamp 2 rồi stamp 1); counterexample `endorsement-unfenced`; Kani hai bước |
| Ý định ngược chiều từ góc nhìn cũ | `Stale` với trạng thái hiện tại; không ghi | Unit test, Kani transition |
| Thử lại sau khi bỏ chọn | Thử lại lệnh bỏ: `AlreadyCurrent`; bản trùng của lệnh chọn trước đó: `Stale` | `reorder_and_retry` |
| Xóa/tạo lại hàng làm reset version (ABA) | Cấm: bỏ chọn giữ hàng với version mới. Mutation xóa hàng làm bản trùng cũ áp lại | counterexample `endorsement-version-reset` |
| Đã chọn nhưng revision cần xác nhận lại | `EndorsedEarlierRevision`, không đếm; thử lại trang cũ → `RevisionChanged`; chọn bản mới → `Changed` (delta chuyển 1 từ bản cũ sang bản mới) | `revision_anchor`, unit test work |
| Trùng lặp với khóa khác | Không có khóa: hai yêu cầu cùng version và cùng lựa chọn → một `Changed`/`Reaffirmed`, một `AlreadyCurrent`; tối đa một hiệu ứng | `reorder_and_retry` (bản trùng), Kani |
| Đích ẩn/rút bài | Mọi ý định `TargetUnavailable`; tổng số và trạng thái riêng `Unavailable`; yêu cầu bỏ đã bị từ chối khi ẩn có thể áp sau khi đăng lại | `revision_anchor`, `comment_targets` |
| Sai binding tác phẩm/bình luận | `TargetUnavailable` | Unit test, Kani admission (identity khác nhau) |
| Hai thiết bị của cùng người | Ai được chấp nhận trước ở một version thắng, bên kia `Stale`; **không** hứa “ý cuối” theo đồng hồ | `reorder_and_retry` (luồng 4) |
| Cạn version/số đếm | `VersionExhausted`; `CountError::{Underflow, Overflow}` làm rollback cả giao dịch | Kani transition, Kani counts |

## 4. Hợp đồng cho lát API và PostgreSQL

Lát sau gọi đúng các hàm trên trong transaction; không viết luật thứ hai trong handler hay SQL.

1. Actor là `PrincipalId` từ resolver phía server; production trả `401` tới khi có auth authority (D-05).
2. Thứ tự khóa thống nhất với adapter thảo luận: hàng `publications` (FOR SHARE) → với bình luận:
   `discussion_threads` → hàng bình luận (FOR SHARE) → hàng ghi nhận của (actor, đích) (FOR UPDATE,
   hoặc chèn-giành rồi khóa).
3. Nạp facts từ chính các hàng đó: `WorkFacts { work, author: works.owner_id, published:
   publications.revision_id }`; `CommentFacts { work: thread.work_id, comment, work_public,
   writer, state, settings }`; hàng hiện tại hoặc `Endorsement::ABSENT`.
4. Chỉ khi `Transition::write()` có giá trị: ghi có điều kiện `version = current.version` (UPDATE)
   hoặc INSERT khi chưa có hàng (`ON CONFLICT DO NOTHING`, xung đột thì đọc lại và quyết định lại).
   Nếu chọn counter giao dịch, áp `delta()` cho anchor bị bỏ và anchor được thêm trong cùng transaction;
   `CountError` làm rollback toàn bộ.
5. **Không DELETE hàng khi bỏ chọn.** Chỉ xóa tài khoản/purge đích mới được xóa hàng, sau khi không
   còn yêu cầu nào của principal/đích đó có thể được nhận.
6. Đề xuất ánh xạ HTTP (lát API quyết định và giữ mã hiện có): `Changed`/`Reaffirmed` → `200`
   `accepted: true`; `AlreadyCurrent` → `200` `accepted: false`; `Stale` → `409 version_conflict`
   kèm trạng thái riêng hiện tại; `RevisionChanged` → `409 revision_changed` kèm revision đang công
   khai; `TargetUnavailable` → `404 not_found`; `OwnWork`/`OwnComment` → `403`; `CommentsClosed` →
   `409 comments_closed`. Phản hồi riêng tư dùng `no-store`, tách khỏi tổng số công khai.

## 5. Nghĩa vụ của client và giới hạn của hợp đồng

- Gửi `expected` = version cuối cùng đã học cho (session, đích). Mỗi (session, đích) chỉ một yêu cầu
  đang bay; thao tác tiếp theo được gộp thành trạng thái mong muốn mới nhất, gửi sau khi yêu cầu
  trước có kết quả, với version từ kết quả đó.
- Mất kết quả: gửi lại **đúng** yêu cầu (cùng `expected`) hoặc đọc lại trạng thái riêng trước khi
  gửi yêu cầu kế tiếp. `Stale`/`RevisionChanged`: hiển thị trạng thái hiện tại, không tự gửi lại ý
  định cũ với version mới.
- Chỉ áp kết quả có version lớn hơn version đang hiển thị, để ACK đảo thứ tự không kéo UI lùi.
- Hợp đồng không cho biết ý định cuối giữa nhiều thiết bị; không có receipt nên server không trả lời
  “lệnh cụ thể này đã từng được chấp nhận chưa”. Reaffirm ghi một hàng/version dù trạng thái không
  đổi; giới hạn tần suất là việc của policy chống lạm dụng (LVE-UC-15), chưa có.

## 6. Phát hiện ngoài phạm vi

`ActorReaction::desire` (reaction, đề xuất #41) và `DiscussionSettings::change_mode` (COM-07) kiểm
“trùng trạng thái” trước version, nên một yêu cầu trùng trạng thái không tạo fence: hai tab của cùng
người có thể gặp đúng trace phản biện ở trên. Lát này **không đổi** hai hành vi đó (AC-P1-06); cần
quyết định riêng cho owner của chúng.
