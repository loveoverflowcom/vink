# Ghi nhận văn học (LVE) — lõi thuần, model, Kani và registry

**Owner:** module [`domain/endorsement`](../../services/vink-app/src/domain/endorsement.rs) trong
`vink-app` (CHOICE-11 A). **Lát:** PR1 của [#73](https://github.com/loveoverflowcom/vink/issues/73),
nền `develop@be2b69e`. Ý nghĩa và ID: [PRD](../../prd/literary-endorsement.md); lựa chọn, semantics và
hợp đồng cho lát sau: [decision record](../architecture/literary-endorsement.md).

Lát này chỉ có **quyết định thuần**: không bảng, store, route, contract HTTP hay UI nào dùng chúng.
Production vẫn chưa có auth authority. Epic AC-03 mới **PARTIAL**: seam production đã được kiểm,
chưa có transaction caller dùng nó.

## Code

| File | Vai trò |
| --- | --- |
| [`endorsement.rs`](../../services/vink-app/src/domain/endorsement.rs) | `POLICY`, `EndorsementVersion`, `Endorsement<A>` + `transition`, `Transition`, `EndorsementDelta`, `EndorsementCount`, `Summary`, `OwnView`; doctest ranh giới kiểu có lawful twin |
| [`endorsement/work.rs`](../../services/vink-app/src/domain/endorsement/work.rs) | `WorkFacts`, `WorkIntent`, `WorkRequest`, `WorkDecision`, `decide_work_endorsement`, `own_work_view`, `work_summary` (anchor `RevisionId`) |
| [`endorsement/comment.rs`](../../services/vink-app/src/domain/endorsement/comment.rs) | `CommentFacts`, `CommentIntent`, `CommentRequest`, `CommentDecision`, `decide_comment_endorsement`, `own_comment_view`, `comment_summary` (anchor `CommentAnchor`; gốc và phản hồi cùng luật) |
| [`endorsement/replay.rs`](../../services/vink-app/src/domain/endorsement/replay.rs) | Test replay input Quint qua các hàm trên |
| [`endorsement/proofs.rs`](../../services/vink-app/src/domain/endorsement/proofs.rs) | 5 harness Kani (`cfg(kani)`) |
| [`behavior/discussion/endorsement.qnt`](../../services/vink-app/behavior/discussion/endorsement.qnt) | Model Quint, scenario, negative control; [README](../../services/vink-app/behavior/discussion/README.md#model-ghi-nhận-văn-học-lve-73) |

Lõi tái dùng `PrincipalId`, `WorkId`, `RevisionId`, `CommentId`, `CommentState` và
`DiscussionSettings`; không đổi reaction, thảo luận, migration hay API hiện có.

## Registry truy vết LVE-INV

Quyết định thuộc PRD/decision record; model Quint sở hữu chuỗi bước đã chọn; Rust hiện thực; fixture
sinh từ model chỉ là oracle để replay input. “—” là chưa có bằng chứng ở lớp đó, không phải PASS.

| ID | Quint (`endorsement.qnt`) | Rust production / test | Kani | PostgreSQL / API | Gap còn lại |
| --- | --- | --- | --- | --- | --- |
| LVE-INV-01 | Một tác phẩm, ba đích cố định (giả định) | `decide_*` so `facts.work`/`facts.comment` với đích yêu cầu; `Endorsement<RevisionId>` ≠ `Endorsement<CommentAnchor>` | `work_admission_…`, `comment_admission_…`: identity khác → `TargetUnavailable` | — | FK/CHECK đúng loại (CHOICE-04) |
| LVE-INV-02 | `eligibleWrites` | Actor là tham số `PrincipalId` từ server; không DTO nào mang actor | Hai harness admission | — | Auth authority (D-05), khóa/cấm tài khoản, giới hạn tần suất |
| LVE-INV-03 | `countsMatchSelections`; một `Row` mỗi (actor, đích) | `Endorsement::selection` duy nhất | `transitions_…` | — | Unique key |
| LVE-INV-04 | `scopedWrites`, `acceptedVersionFence`; bản trùng/thử lại → `already_current` | `Transition::{Reaffirmed, AlreadyCurrent}`, `write()`, `delta()` | `transitions_…` | — | Không receipt (theo thiết kế) |
| LVE-INV-05 | `noRevival`; counterexample `endorsement-unfenced`, `endorsement-version-reset` | `Endorsement::transition`, `EndorsementVersion` | `a_confirmed_request_fences_…`, `transitions_…` | — | UPDATE/INSERT có điều kiện, không DELETE; race thật |
| LVE-INV-06 | `eligibleWrites`, `projectionsHideAndCountEffective`; rút/đăng lại/gỡ bình luận | `decide_*`, `own_*_view`, `*_summary` | Hai harness admission (view + summary) | — | Cache/event, cursor, đọc batch |
| LVE-INV-07 | `countsMatchSelections`; reaffirm không đổi số đếm | `EndorsementCount::apply`, `EndorsementDelta` | `counts_follow_deltas_…` | — | Rebuild/recount SQL, freshness (CHOICE-05) |
| LVE-INV-08 | `projectionsHideAndCountEffective` | `Summary::Unavailable`, `OwnView`; không có projection danh sách người ghi nhận | Hai harness admission | — | `no-store`, tách cache công khai/riêng |
| LVE-INV-09 | — | Lõi không sắp xếp | — | — | CHOICE-06 OPEN |
| LVE-INV-10 | Version trong mỗi kết quả (`answer`); client shell học version đơn điệu | Mọi `Transition` mang version | — | — | Consumer: fence session/route, áp kết quả đơn điệu, browser |
| LVE-INV-11 | — | Không module nào khác import `endorsement`; không quyết định nào đọc số đếm | — | — | Chưa có guard tự động; kiểm khi review |
| LVE-INV-12 | `noSilentTransfer` + counterexample `endorsement-transfer`; `noSelfEndorsement`; scenario tắt/mở bình luận | `POLICY`, `RevisionChanged`, `OwnWork`/`OwnComment`, `CommentsClosed` | Hai harness admission | — | Chủ sản phẩm xác nhận CHOICE-02/08/09 |

## Kiểm chứng theo lớp

Lệnh do [development guide](README.md) sở hữu; không có lane `verify votes`/`verify endorsements`.

1. **Domain thuần (`quick`):** 14 unit test trong `domain::endorsement` (version, transition, fence,
   thử lại, anchor revision, ẩn/sai binding, tự ghi nhận, tắt bình luận, số đếm) và doctest
   `compile_fail` + lawful twin. Không chứng minh transaction.
2. **Quint (`discussion`):** cùng runner `verify.mjs` với ba model thảo luận. Model có client shell
   được gắn nhãn: 4 luồng (người 3 có hai thiết bị), mạng giao theo thứ tự bất kỳ, có thể giao lại
   (`keep`) và mất câu trả lời (`heard`), đọc lại. Simulation ngẫu nhiên seed `41`, 1.000 mẫu, tối đa
   40 bước; ba scenario xác định `reorder_and_retry` (21), `revision_anchor` (21), `comment_targets`
   (28) xuất `endorsement-fixtures.json`; ba negative control (`mutatedUnfencedStep`,
   `mutatedResetStep`, `mutatedTransferStep`) phải tạo counterexample có nội dung đúng. Đây là
   simulation có giới hạn, **không phải model checking**; Apalache NOT_RUN.
3. **Replay (`discussion`):** `quint_endorsement_inputs_replay_against_the_production_decisions`
   chỉ đưa input vào `decide_work_endorsement`, `decide_comment_endorsement`,
   `EndorsementCount::apply`, projection, `ModerationAction::apply` và `change_mode`, rồi so kết
   quả, version trả về, hàng, số đếm, tổng số và trạng thái riêng từng bước; lane đòi test có tên
   này được chọn và đủ 15 outcome.
4. **Kani (`kani`):** 5 harness, 16 `cover!` phải SATISFIED, không vòng lặp nên không cần unwind:
   transition trên mọi version `i64` × lựa chọn; chuỗi hai yêu cầu của một luồng (giả định luồng chỉ
   biết version server đã tạo, theo thứ tự); admission tác phẩm/bình luận trên mọi binding, công
   khai, tác giả, revision, trạng thái bình luận × chế độ × generation; số đếm trên mọi `u32`.
   Identity trong 2–3 giá trị ký hiệu. Không chứng minh chuỗi dài, khóa, I/O.
5. **PostgreSQL, HTTP, browser:** chưa có code; NOT_RUN ở lát này.

**Độ nhạy đã kiểm khi phát triển (ghi trong PR):** cho `transition` chỉ chấp nhận khi lựa chọn
khác (no-op không fence) làm replay FAIL ở `reorder_and_retry` bước reaffirm và harness
`a_confirmed_request_fences_every_earlier_request_of_its_stream` FAIL; simulation ngẫu nhiên với
cùng mutation cũng báo vi phạm. Đây là kiểm tay, không phải bước của runner.
