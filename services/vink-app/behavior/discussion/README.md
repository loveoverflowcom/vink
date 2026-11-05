# Model discussion

Thư mục giữ bốn model Quint dùng chung một package pin và một runner: `structure.qnt` (cấu trúc
hai tầng, D2T-01), `lifecycle.qnt` (vòng đời bình luận, bật/tắt, authority kiểm duyệt và receipt
gửi bình luận, D2T-02/D2T-INV-06, [phần dưới](#model-vòng-đời-bình-luận-d2t-02)),
`reaction.qnt` (reaction desired-state, D2T-INV-08, [phần sau](#model-reaction-d2t-inv-08)) và
`endorsement.qnt` (ghi nhận văn học trên tác phẩm, bình luận gốc và phản hồi, LVE #73,
[phần cuối](#model-ghi-nhận-văn-học-lve-73)). Model LVE nằm ở đây để dùng chung pin Quint và lane
`discussion`; nó không làm tác phẩm thành một phần của thảo luận.

## Model cấu trúc

Lát cắt D2T-01 của [#41](https://github.com/loveoverflowcom/vink/issues/41), đối chiếu với
[core production](../../src/domain/discussion.rs) và
[phân tích ownership](../../../../docs/architecture/discussion-ownership.md).
Model không chọn discussion owner và không thay đổi trạng thái D-02.

### Phạm vi và giả định

`structure.qnt` mô tả **một thread ngầm định**, comment IDs hữu hạn `1..4`, hai lệnh tạo root
và reply, không có I/O. Root trỏ chính nó; reply luôn trỏ root của target hiện có. Context
trực tiếp đến root dùng sentinel `0`; context đến reply giữ ID của reply đó. Trong fixture,
`0` được đổi thành `null` để đối chiếu với `Option::None` của Rust.

Duplicate được xét trước missing target, kể cả khi cả hai cùng xảy ra. Lỗi giữ nguyên toàn
bộ placements; lệnh thành công chỉ thêm một placement. `previous` là snapshot của bước
trước để kiểm tính bất biến. `rank` là ghost insertion order, không phải field production:
root/context edges phải đi về rank nhỏ hơn, ngoại trừ root tự trỏ chính nó. IDs không cần
được tạo theo thứ tự số. `cursor` chỉ phục vụ scenario, bị chặn tại 10 để giữ miền hữu hạn.

Các invariant: `rootIsRoot`, `canonicalContext`, `noCycles`, `immutable`, `rejectionAtomic`,
`finiteDomain`, `resultMatchesInput`; `structureSafe` kết hợp chúng. Unique ID là tính chất
của map, còn invariant kết quả kiểm duplicate không được biến thành một create thành công.
Scenario chain thực sự tạo reply-to-reply và reply-to-reply-to-reply, nên normalization
không được kiểm bằng một antecedent không thể đạt tới.

**Không được chứng minh:** nhiều thread/tenant, auth, quyền truy cập, body, moderation,
visibility, sửa/xóa/tombstone, idempotent retry, reaction, ranking, subject mapping,
transaction/concurrency, PostgreSQL, Discourse, transport, UI hoặc liveness. Rust có thể kiểm
isolation giữa thread riêng; không gán bằng chứng đó cho model một thread này.

### Lệnh tái lập

Yêu cầu Node.js >=22 theo development guide; dependency npm pin chính xác Quint **0.33.1**,
có lockfile. Lệnh tổng hợp local/CI là `cargo xtask verify discussion` trong
[hướng dẫn phát triển](../../../../docs/development/README.md). Chạy riêng model từ repo root:

```sh
npm ci --ignore-scripts --no-audit --no-fund --prefix services/vink-app/behavior/discussion
node services/vink-app/behavior/discussion/verify.mjs
```

Runner luôn gọi package local và backend `typescript`, không tự tải evaluator Rust hoặc
Apalache. Runner thực hiện:

1. Typecheck toàn bộ model, gồm action mutation.
2. Random simulation `step`: seed `41` (Quint in ra `0x29`), **1.000 samples**, tối đa
   **40 transitions/sample**, kiểm `structureSafe` sau init và mỗi transition.
3. Chạy hai scenario xác định với 1 sample/seed 41 và xuất ITF thật: `chainStep` 10 transitions,
   `rootsStep` 5 transitions. So khớp fixture đã commit, khác biệt hoặc trace ngắn/rỗng là FAIL.
4. Chạy `mutatedChainStep` với `rootIsRoot`, tối đa 10 transitions. Action này giữ nguyên
   semantics còn lại nhưng cố ý đặt `root = target ID` thay vì root của target. Runner đòi
   exit 1, thông báo invariant violation, ITF status violation và một state thực sự có
   structural parent không phải root. Lỗi parse/type/runtime không được tính là PASS.

Lệnh trực tiếp tương đương cho random simulation (chạy tại thư mục này):

```sh
./node_modules/.bin/quint typecheck structure.qnt
./node_modules/.bin/quint run structure.qnt --backend=typescript --seed=41 \
  --step=step --max-steps=40 --max-samples=1000 --invariant=structureSafe --verbosity=1
```

Simulation là kiểm bounded random executions, **không phải exhaustive model checking hay
proof**. Apalache/model checking: **NOT_RUN**. Kết quả model và conformance Rust là hai loại
bằng chứng riêng, không thay thế constraints/transactions tại database.

### Fixture từ model và negative control

`fixtures.json` dùng format `vink-discussion-conformance-v1`, gồm version Quint và hai
scenario. Mỗi step chứa **command input**, outcome và toàn bộ **state output từ ITF**.
Node chỉ bỏ init/ghost fields, chuyển integer/sentinel và sort theo ID; không có reducer
JavaScript tái tạo expected state. Rust replay phải gọi `DiscussionThread::create_root` /
`create_reply` bằng command, dùng `Uuid::from_u128` cho IDs và một thread cố định (ví dụ 10),
rồi so outcome/placement quan sát được với state của model. `id == root` xác định root.
Không đưa expected placement vào core như một input.

- `chain_and_rejections`: chain `1 ← 2 ← 3 ← 4`, missing/self target, duplicate root/reply,
  duplicate-vs-missing precedence, từ chối reparent/cycle và duplicate self-reference.
- `separate_roots`: root 1 và 2; 3 trả lời 2; 4 trả lời 3 vẫn có root 2; từ chối chuyển 3
  sang root 1 vì ID đã tồn tại.
- `root-normalization-counterexample.json` giữ command, version và trace vi phạm đã chiếu
  từ ITF. Tại transition 6, tạo 3 trả lời 2 cho ra `root=2` trong khi placement 2 có `root=1`.
  Đây là negative control có nghĩa, không phải model lỗi cú pháp hoặc test bị bỏ qua.

Sau khi chủ động sửa model, review output rồi mới cập nhật hai artifact:

```sh
node services/vink-app/behavior/discussion/verify.mjs --write
```

Chế độ mặc định chỉ kiểm drift, không ghi lại fixture. ITF tạm có timestamp được chuyển
thành artifact ổn định; nếu lỗi, runner giữ ITF trong thư mục tạm và in đường dẫn để điều tra.

## Model vòng đời bình luận (D2T-02)

`lifecycle.qnt` mô tả **một tác phẩm công khai** có tác giả là principal 1, principals `1..3`,
comment IDs `1..3`, người đọc ẩn danh là viewer 0. Mỗi action là một commit đã tuần tự hóa: việc
khóa PostgreSQL (grant `FOR SHARE`, hàng thread, hàng comment) bảo đảm thứ tự đó là **giả định**
ở đây, bằng chứng nằm ở lane `postgres`. Cấu trúc root/context thuộc model cấu trúc; ở đây
`target` chỉ là “trả lời bình luận nào”. Server sinh ID mới nên `submit` chỉ chạy với ID chưa có.

Các action đi theo đúng thứ tự kiểm của store (`persistence/discussion.rs`):

| Action | Quyết định production được replay |
| --- | --- |
| `submit` | `decide_submission` (tác phẩm công khai): receipt của (actor, key) quyết định trước (`replayed` trả bình luận đầu, `key_reused` khi payload khác), rồi `closed`, `target_unavailable`, `created` pending v1 cùng receipt; bị từ chối thì không giữ receipt |
| `setMode` | `authorize_mode_change` rồi `DiscussionSettings::change_mode` |
| `removeOwn` | `view` của chính actor → `authorize_writer_removal` → `remove_by_author` |
| `moderate` | grant tại commit + `authorize_moderation` (tự né) → `ModerationAction::apply` |
| `grant`/`revoke` | Shell tổng hợp mô phỏng kết quả idempotent của lệnh vận hành (`granted`/`already_active`, `revoked`/`not_active`) |

Quyết định của chủ sản phẩm (03/11/2026) được model hóa: tác phẩm mới bắt đầu `open`; “tắt” chỉ
chặn lệnh gửi mới, không đổi hay ẩn bình luận nào (`scopedWrites`); tác giả tác phẩm dù có grant
vẫn tự né mọi bình luận trên tác phẩm của mình, không duyệt, từ chối, gỡ hay sửa được.

Invariant, gộp trong `lifecycleSafe`:

- `rejectionAtomic`: mọi lệnh bị từ chối hoặc no-op không đổi trạng thái nào.
- `scopedWrites`: lệnh bình luận chỉ đổi đúng một bình luận; `set_mode` không chạm bình luận;
  lệnh vận hành chỉ đổi grant.
- `premoderatedSubmission`: bình luận mới luôn pending v1 của actor, chỉ khi đang bật và target
  đang công khai.
- `modeAuthority`: chỉ tác giả tác phẩm đổi chế độ, với generation đã thấy, tăng đúng 1.
- `versionedLifecycle`: version không giảm, mỗi thay đổi tăng đúng 1, gỡ là terminal, quyết định
  gắn version cũ không có hiệu lực.
- `writeAuthority`: bình luận chỉ đổi do người viết tự gỡ hoặc do moderator có grant ở commit đó,
  không phải người viết và không phải tác giả tác phẩm (COM-08, MOD-02).
- `viewerProjection`: chỉ present + approved là công khai; người khác (kể cả tác giả tác phẩm)
  không thấy gì của bình luận pending/rejected/đã gỡ; người viết thấy trạng thái của mình.
- `idempotentSubmission` (D2T-INV-06): mỗi (actor, key) chỉ một hiệu ứng bền; bình luận chỉ được
  tạo dưới key mới và receipt ghi đúng payload (target, body) cùng bình luận đó; cùng key + payload
  trả lại bình luận đầu (kể cả khi đã tắt hoặc bình luận đã bị gỡ, không tạo lại); cùng key khác
  payload bị từ chối; receipt không bị ghi đè hay mất. `premoderatedSubmission` đặt receipt trước
  gate đóng.
- `finiteDomain`.

**Không được chứng minh:** auth authority (D-05), retention receipt, rút bài, lý
do kiểm duyệt, hàng chờ/phân trang, neo root ẩn và context không còn hiển thị (INV-05), nhiều tác
phẩm/thread, thu hồi đang chạy song song với quyết định (chỉ thứ tự đã tuần tự hóa), report/appeal,
mở lại sau khi gỡ, liveness của hàng chờ.

Runner chạy thêm cho model này:

1. Random simulation `step`: seed `41`, **1.000 samples**, tối đa **40 transitions/sample**, kiểm
   `lifecycleSafe` sau init và mỗi transition.
2. Ba scenario xác định xuất ITF thành `lifecycle-fixtures.json`
   (`vink-discussion-lifecycle-conformance-v1`): `moderation_authority` 26 transitions (grant,
   tự né của tác giả-moderator và của người viết, stale, retry, thu hồi trước commit, gỡ terminal),
   `comments_mode` 16 transitions (chỉ tác giả đổi, tắt chặn root/reply kể cả của tác giả,
   retry, tab cũ, bật lại, target pending, từ chối rồi duyệt lại) và `submission_retry` 12
   transitions (mất phản hồi rồi gửi lại, cùng key khác body/target, replay sau khi tắt, key mới bị
   chặn khi tắt không giữ receipt, replay không tạo lại bình luận đã gỡ, key theo từng actor).
   Runner đòi ba scenario cùng đạt đủ 18 outcome và 4 kiểu hiển thị, để invariant không đúng nhờ
   antecedent không đạt tới.
3. `mutatedRecusalStep` gọi `moderateWith(_, false)`: giữ nguyên mọi semantics khác nhưng bỏ tự né
   của tác giả tác phẩm. Runner đòi exit 1, ITF status violation và state thật trong đó tác giả
   (có grant) đã gỡ bình luận của người khác tại transition 3; lưu ở
   `author-recusal-counterexample.json`.
4. `mutatedReceiptStep` gọi `submitWith(_, false)` (bỏ kiểm receipt) với hai lần gửi cùng (actor 2,
   key 5, body 1). Runner đòi violation của `idempotentSubmission` tại transition 2, trong đó lần
   gửi lại tạo bình luận thứ hai; lưu ở `receipt-retry-counterexample.json`.

Rust replay `domain::discussion::lifecycle_replay` đưa **chỉ command** của từng step vào các quyết
định production theo thứ tự của store, rồi so outcome, bình luận được trả về, chế độ, generation,
grant, receipt, trạng thái từng bình luận và `view` của 4 người xem với state của model. Receipt
mang `PayloadDigest::of_submission` thật của (work, target, body); body 1 viết dạng NFD nên cũng
kiểm digest chuẩn hóa. Bình luận, receipt, settings và tập grant nằm trong shell bộ nhớ (được ghi
rõ trong test); khóa và độ bền thuộc lane `postgres`. Tạm bỏ `actor == work_author` trong
`authorize_moderation` làm replay đỏ ở `moderation_authority step 5`; tạm chuyển kiểm receipt
xuống sau gate đóng trong `decide_submission` làm replay đỏ ở `submission_retry step 5`.

Lệnh trực tiếp tương đương cho random simulation (chạy tại thư mục này):

```sh
./node_modules/.bin/quint typecheck lifecycle.qnt
./node_modules/.bin/quint run lifecycle.qnt --backend=typescript --seed=41 \
  --step=step --max-steps=40 --max-samples=1000 --invariant=lifecycleSafe --verbosity=1
```

## Model reaction (D2T-INV-08)

`reaction.qnt` mô tả **một tác phẩm công khai** của principal 1, principals `1..3`, bình luận 1
(người viết 2) và 2 (người viết 3), hai trong bốn loại production (`heart`, `moved`) là đủ để thay
thế. Bình luận chỉ còn ba trạng thái reaction cần: `pending`, `public`, `removed`; shell kiểm duyệt
tổng hợp duyệt/gỡ ở version hiện tại, tác giả đổi chế độ ở generation hiện tại (authority của hai
việc này thuộc `lifecycle.qnt`). Mỗi action là một commit đã tuần tự hóa; chưa có store reaction
nên khóa là **giả định** (#43).

`react` đi đúng `decide_reaction` rồi `ReactionTally::apply`. Semantics là **đề xuất** của
[registry](../../../../docs/development/discussion-core.md): desired state, một reaction mỗi
(người, bình luận), generation fence, giới hạn tương tác, count là projection.

Invariant, gộp trong `reactionSafe`:

- `reactionWritesAreApplied`, `scopedWrites`: chỉ reaction được áp mới đổi reaction/count, và chỉ
  của đúng (actor, bình luận); lệnh khác không chạm reaction.
- `noBlindToggle`: xin đúng reaction hiện có không đổi gì; reaction được áp luôn là reaction được xin.
- `generationFence`: mọi thay đổi cần generation actor đã thấy và tăng đúng 1.
- `interactionLimits`: chỉ bình luận đang công khai đổi được; thêm/đổi cần chế độ mở và actor
  không phải người viết; gỡ vẫn được khi đã tắt.
- `tallyMatchesReactions`: count mỗi loại đúng bằng số actor đang giữ loại đó, không âm.
- `noSelfReaction`, `hiddenCountsStayHidden` (bình luận ẩn không lộ count), `finiteDomain`.

**Không được chứng minh:** persistence/khóa/đồng thời, rate limit theo thời gian, catalog cuối,
reaction trên nhiều tác phẩm, rút bài, quan hệ với sticker (#25), UI/API.

Runner chạy:

1. Random simulation `step`: seed `41`, **1.000 samples**, tối đa **40 transitions/sample**, kiểm
   `reactionSafe`.
2. Scenario `desired_state` 23 transitions xuất `reaction-fixtures.json`
   (`vink-discussion-reaction-conformance-v1`): bình luận pending, tự thả, đặt, retry mất phản hồi,
   tác giả tác phẩm thả như người đọc, thay thế, tab cũ, gỡ, bản trùng muộn, gỡ hai lần, tắt (chặn
   thêm, retry vẫn `unchanged`, gỡ vẫn được), bật lại, gỡ bình luận (reaction còn lưu nhưng count
   ẩn). Runner đòi đủ 8 outcome, một lần thay thế ở generation > 1 và một bình luận đã gỡ còn count.
3. `mutatedToggleStep` (`desiredState = false`: xin lại reaction hiện có thì gỡ nó) phải vi phạm
   `noBlindToggle` tại transition 3 (`reaction-toggle-counterexample.json`);
   `mutatedUnfencedStep` (`fenced = false`) phải vi phạm `generationFence` tại transition 4, khi yêu
   cầu cũ ở generation 1 đưa `heart` trở lại sau lần gỡ (`reaction-unfenced-counterexample.json`).
   Runner kiểm nội dung counterexample, không chỉ exit code.

Rust replay `domain::discussion::reaction_replay` đưa chỉ command vào `decide_reaction`,
`ReactionTally::apply`, `ModerationAction::apply`, `change_mode` và `public_tally`, rồi so outcome,
chế độ, trạng thái/count từng bình luận, reaction của mọi (actor, bình luận) và count công khai với
state của model. Tạm bỏ kiểm `expected != self.generation` trong `ActorReaction::desire` làm replay
đỏ ở `desired_state step 7` (tab cũ).

```sh
./node_modules/.bin/quint typecheck reaction.qnt
./node_modules/.bin/quint run reaction.qnt --backend=typescript --seed=41 \
  --step=step --max-steps=40 --max-samples=1000 --invariant=reactionSafe --verbosity=1
```

## Model ghi nhận văn học (LVE, #73)

`endorsement.qnt` (policy `lve-core-v1`) mô tả **một tác phẩm** của principal 1 với revision 1 và 2
(`published = 0`: chưa/không còn công khai), đích 0 là tác phẩm, đích 1 là bình luận gốc của
principal 2, đích 2 là phản hồi phẳng của principal 3. Semantics và lựa chọn thuộc
[decision record](../../../../docs/architecture/literary-endorsement.md); registry ở
[lõi LVE](../../../../docs/development/literary-endorsement-core.md).

Khác các model trên, model có **client shell được gắn nhãn**: luồng 1..4 (luồng 4 là thiết bị thứ
hai của principal 3) gửi yêu cầu ở version đã học; mạng giao yêu cầu theo thứ tự bất kỳ, có thể giao
lại (`keep`, mô phỏng thử lại/bản trùng) và làm mất câu trả lời (`heard`); `read` là đọc lại trạng
thái riêng. Mỗi lần giao là một commit đã tuần tự hóa; khóa với publication/kiểm duyệt/chế độ là giả
định (chưa có store). Kiểm duyệt và chế độ là shell như `reaction.qnt`; `publish`/`withdraw` chỉ
dời con trỏ vì chưa có lệnh xuất bản.

Invariant, gộp trong `endorsementSafe`:

- `scopedWrites`, `acceptedVersionFence`: chỉ yêu cầu được chấp nhận mới ghi, đúng hàng của nó và
  số đếm của đích đó; mỗi lần chấp nhận cần version đang lưu và tăng đúng 1; reaffirm không đổi số đếm.
- `noRevival`: khi server đã xác nhận một yêu cầu của một luồng (chấp nhận hoặc thấy đã đúng), không
  yêu cầu nào luồng đó gửi trước có hiệu lực sau đó, dù giao đảo thứ tự, giao lại hay mất câu trả lời.
- `noSilentTransfer`: lựa chọn trên tác phẩm luôn là revision người đọc đã đọc.
- `eligibleWrites`: ghi cần đích công khai tại commit, actor không phải tác giả; lựa chọn mới trên
  bình luận cần chế độ mở.
- `countsMatchSelections`, `noSelfEndorsement`, `projectionsHideAndCountEffective` (đích ẩn không có
  số đếm hay trạng thái riêng; tổng số chỉ đếm lựa chọn hiệu lực), `finiteDomain`.

**Không được chứng minh:** nhiều tác phẩm/thread, persistence/khóa/đồng thời thật, auth, giới hạn
tần suất, lệnh xuất bản/rút bài, xếp hạng, cache, UI/API, ý định cuối giữa nhiều thiết bị (hợp đồng
không hứa điều đó).

Runner chạy:

1. Random simulation `step`: seed `41`, **1.000 samples**, tối đa **40 transitions/sample**, kiểm
   `endorsementSafe`.
2. Ba scenario xuất `endorsement-fixtures.json` (`vink-endorsement-conformance-v1`, kèm `policy`):
   `reorder_and_retry` 21 transitions (trace phản biện OFF/v1: bỏ@1 đến trước được reaffirm, chọn@1
   cũ đến sau là `stale`; mất câu trả lời, đọc lại, bản trùng, thử lại sau khi bỏ, thiết bị thứ hai,
   bản trùng cũ sau khi bỏ không gây ABA, tác giả), `revision_anchor` 21 (bản mới không nhận ghi
   nhận ngầm, xác nhận lại, rút bài, đăng lại revision cũ, yêu cầu bỏ bị từ chối khi ẩn được áp sau
   khi đăng lại) và `comment_targets` 28 (gốc chờ duyệt, người viết, gốc và phản hồi không cộng dây
   chuyền, tắt bình luận chặn lựa chọn mới nhưng cho reaffirm/bỏ, tác phẩm không bị chế độ chi phối,
   gỡ bình luận). Runner đòi đủ 15 outcome, đủ 4 trạng thái riêng, đúng hai bước phản biện và một lần
   ghi nhận được áp cho mỗi loại đích.
3. Negative control, kiểm nội dung counterexample: `mutatedUnfencedStep` (yêu cầu trùng trạng thái ở
   version hiện tại là no-op) vi phạm `noRevival` tại transition 5
   (`endorsement-unfenced-counterexample.json`); `mutatedResetStep` (xóa hàng khi bỏ chọn) vi phạm
   `noRevival` tại transition 6 (`endorsement-version-reset-counterexample.json`);
   `mutatedTransferStep` (bỏ kiểm revision đã đọc) vi phạm `noSilentTransfer` tại transition 4
   (`endorsement-transfer-counterexample.json`).

Rust replay `domain::endorsement::replay` đưa chỉ input vào `decide_work_endorsement`,
`decide_comment_endorsement`, `EndorsementCount::apply`, projection, `ModerationAction::apply` và
`change_mode`, rồi so outcome, version trả về, công khai, chế độ, trạng thái bình luận, mọi hàng khác
`ABSENT`, số đếm theo anchor, tổng số và trạng thái riêng của mọi (actor, đích). Fixture chỉ liệt kê
hàng khác `ABSENT` (không chọn, version 1), nên vẫn ghim mọi hàng.

```sh
./node_modules/.bin/quint typecheck endorsement.qnt
./node_modules/.bin/quint run endorsement.qnt --backend=typescript --seed=41 \
  --step=step --max-steps=40 --max-samples=1000 --invariant=endorsementSafe --verbosity=1
```
