# Model discussion

Thư mục giữ năm model Quint dùng chung một package pin và một runner: `structure.qnt` (cấu trúc
hai tầng, D2T-01), `lifecycle.qnt` (vòng đời bình luận, bật/tắt, authority kiểm duyệt và receipt
gửi bình luận, D2T-02/D2T-INV-06, [phần dưới](#model-vòng-đời-bình-luận-d2t-02)),
`reaction.qnt` (reaction desired-state, D2T-INV-08, [phần sau](#model-reaction-d2t-inv-08)) và
`endorsement.qnt` (ghi nhận văn học trên tác phẩm, bình luận gốc và phản hồi, LVE #73,
[phần sau](#model-ghi-nhận-văn-học-lve-73)) và `admission.qnt` (admission theo policy có version,
shadow và hậu kiểm, #120, [phần cuối](#model-admission-và-hậu-kiểm-120)). Model LVE nằm ở đây để
dùng chung pin Quint và lane `discussion`; nó không làm tác phẩm thành một phần của thảo luận.

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
| `setMode` | `authorize_mode_change` rồi `DiscussionSettings::change_mode`: so generation trước, yêu cầu ở đúng generation được chấp nhận (`changed`, hoặc `reaffirmed` khi chế độ đã đúng) và tăng generation; generation khác: `already_current` khi chế độ đã đúng, ngược lại `stale` |
| `removeOwn` | `view` của chính actor → `authorize_writer_removal` → `remove_by_author` |
| `moderate` | grant tại commit + `authorize_moderation` (tự né) → `ModerationAction::apply` |
| `grant`/`revoke` | Shell tổng hợp mô phỏng kết quả idempotent của lệnh vận hành (`granted`/`already_active`, `revoked`/`not_active`) |

Quyết định của chủ sản phẩm (03/11/2026) được model hóa: tác phẩm mới bắt đầu `open`; “tắt” chỉ
chặn lệnh gửi mới, không đổi hay ẩn bình luận nào (`scopedWrites`); tác giả tác phẩm dù có grant
vẫn tự né mọi bình luận trên tác phẩm của mình, không duyệt, từ chối, gỡ hay sửa được.

Invariant, gộp trong `lifecycleSafe`:

- `rejectionAtomic`: mọi lệnh bị từ chối hoặc không được chấp nhận không đổi trạng thái nào.
- `scopedWrites`: lệnh bình luận chỉ đổi đúng một bình luận; `set_mode` không chạm bình luận;
  lệnh vận hành chỉ đổi grant.
- `premoderatedSubmission`: bình luận mới luôn pending v1 của actor, chỉ khi đang bật và target
  đang công khai.
- `modeAuthority`: chỉ tác giả tác phẩm ghi settings, với generation đã thấy, tăng đúng 1;
  `reaffirmed` giữ chế độ, `changed` đổi chế độ.
- `confirmedModeFence` (COM-07 “tab cũ không đè lựa chọn mới”, xác nhận 05/11/2026): biến ghost
  `modeFence` giữ generation cao nhất của một yêu cầu đã xác nhận (được chấp nhận, hoặc chế độ đã
  đúng) mà server từng cấp; một yêu cầu chỉ được chấp nhận khi generation của nó lớn hơn
  `modeFence`, nên yêu cầu cũ hơn không bao giờ áp sau một yêu cầu “giữ nguyên” đã xác nhận.
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
   `comments_mode` 18 transitions (chỉ tác giả đổi, tắt chặn root/reply kể cả của tác giả,
   retry `already_current`, tab B giữ tắt ở generation hiện tại `reaffirmed` rồi yêu cầu bật lại
   cũ hơn của tab A `stale`, tab cũ, bật lại, target pending, từ chối rồi duyệt lại) và `submission_retry` 12
   transitions (mất phản hồi rồi gửi lại, cùng key khác body/target, replay sau khi tắt, key mới bị
   chặn khi tắt không giữ receipt, replay không tạo lại bình luận đã gỡ, key theo từng actor).
   Runner đòi ba scenario cùng đạt đủ 20 outcome và 4 kiểu hiển thị, cùng một `reaffirmed` ngay
   trước yêu cầu cũ cùng generation bị `stale`, để invariant không đúng nhờ antecedent không đạt tới.
3. `mutatedRecusalStep` gọi `moderateWith(_, false)`: giữ nguyên mọi semantics khác nhưng bỏ tự né
   của tác giả tác phẩm. Runner đòi exit 1, ITF status violation và state thật trong đó tác giả
   (có grant) đã gỡ bình luận của người khác tại transition 3; lưu ở
   `author-recusal-counterexample.json`.
4. `mutatedReceiptStep` gọi `submitWith(_, false)` (bỏ kiểm receipt) với hai lần gửi cùng (actor 2,
   key 5, body 1). Runner đòi violation của `idempotentSubmission` tại transition 2, trong đó lần
   gửi lại tạo bình luận thứ hai; lưu ở `receipt-retry-counterexample.json`.
5. `mutatedNoopModeStep` gọi `setModeWith(_, false)`: luật cũ, xin đúng chế độ hiện tại là no-op ở
   mọi generation. Trace: tắt@1→2; tab B xin tắt@2 (`already_current`, generation vẫn 2); tab A xin
   bật@2 đến sau và được áp. Runner đòi violation của `confirmedModeFence` tại transition 3 (mà
   `modeAuthority` không bắt được); lưu ở `mode-noop-fence-counterexample.json`.

Rust replay `domain::discussion::lifecycle_replay` đưa **chỉ command** của từng step vào các quyết
định production theo thứ tự của store, rồi so outcome, bình luận được trả về, chế độ, generation,
grant, receipt, trạng thái từng bình luận và `view` của 4 người xem với state của model. Receipt
mang `PayloadDigest::of_submission` thật của (work, target, body); body 1 viết dạng NFD nên cũng
kiểm digest chuẩn hóa. Bình luận, receipt, settings và tập grant nằm trong shell bộ nhớ (được ghi
rõ trong test); khóa và độ bền thuộc lane `postgres`. Tạm bỏ `actor == work_author` trong
`authorize_moderation` làm replay đỏ ở `moderation_authority step 5`; tạm chuyển kiểm receipt
xuống sau gate đóng trong `decide_submission` làm replay đỏ ở `submission_retry step 5`; đặt lại
kiểm “trùng chế độ” trước generation trong `change_mode` làm replay đỏ ở `comments_mode step 8`.
`the_no_op_mode_counterexample_is_refused_by_production` đưa input của counterexample qua
production và đòi `changed`, `reaffirmed`, `stale` (bình luận vẫn tắt, generation 3).

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
(người, bình luận), generation của ý định được chấp nhận (yêu cầu ở đúng generation luôn được
chấp nhận và tăng, `reaffirmed` khi reaction đã đúng, không đổi count; theo sửa fence của PR #79 ngày 05/11/2026), giới hạn tương tác (mọi yêu cầu của người viết trên bình luận của
mình là `own_comment`), count là projection. `setMode` tổng hợp luôn ở generation hiện tại:
`mode_set` hoặc `mode_reaffirmed`.

Invariant, gộp trong `reactionSafe`:

- `reactionWritesAreApplied`, `scopedWrites`: chỉ yêu cầu được chấp nhận mới ghi reaction, chỉ
  `changed` đổi count, và chỉ của đúng (actor, bình luận); lệnh khác không chạm reaction.
- `noBlindToggle`: xin đúng reaction hiện có không đổi loại hay count nào (reaffirm chỉ tăng
  generation); reaction được áp luôn là reaction được xin.
- `generationFence`: mọi lần ghi cần generation actor đã thấy và tăng đúng 1.
- `confirmedIntentFence`: ghost `confirmed` giữ generation cao nhất của yêu cầu đã xác nhận (được
  chấp nhận, hoặc reaction đã đúng) của mỗi (actor, bình luận); chỉ yêu cầu ở generation lớn hơn
  mới được chấp nhận, nên yêu cầu cũ không áp sau một yêu cầu “giữ nguyên” đã xác nhận.
- `interactionLimits`: chỉ bình luận đang công khai ghi được, không bao giờ cho người viết; thêm/đổi
  cần chế độ mở; gỡ và reaffirm vẫn được khi đã tắt.
- `tallyMatchesReactions`: count mỗi loại đúng bằng số actor đang giữ loại đó, không âm.
- `noSelfReaction` (người viết không có reaction, kể cả hàng rỗng, trên bình luận của mình),
  `hiddenCountsStayHidden` (bình luận ẩn không lộ count), `finiteDomain`.

**Không được chứng minh:** persistence/khóa/đồng thời, rate limit theo thời gian, catalog cuối,
reaction trên nhiều tác phẩm, rút bài, quan hệ với sticker (#25), UI/API.

Runner chạy:

1. Random simulation `step`: seed `41`, **1.000 samples**, tối đa **40 transitions/sample**, kiểm
   `reactionSafe`.
2. Scenario `desired_state` 28 transitions xuất `reaction-fixtures.json`
   (`vink-discussion-reaction-conformance-v1`): bình luận pending, tự thả và tự gỡ của người viết,
   đặt, retry mất phản hồi (`already_current`), tác giả tác phẩm thả như người đọc, thay thế, tab
   cũ, gỡ, bản trùng muộn, gỡ hai lần (`reaffirmed`), tắt (chặn thêm, retry vẫn `already_current`,
   reaffirm và gỡ vẫn được), tác giả reaffirm chế độ, bật lại, tab B “không reaction”@1 đến trước
   rồi Heart@1 cũ hơn của tab A `stale`, gỡ bình luận (reaction còn lưu nhưng count ẩn). Runner đòi
   đủ 10 outcome, một lần thay thế ở generation > 1, một reaffirm khi đã tắt, một `reaffirmed` ngay
   trước yêu cầu cũ cùng generation bị `stale`, và một bình luận đã gỡ còn count.
3. `mutatedToggleStep` (`desiredState = false`: xin lại reaction hiện có thì gỡ nó) phải vi phạm
   `noBlindToggle` tại transition 3 (`reaction-toggle-counterexample.json`);
   `mutatedUnfencedStep` (`fenced = false`) phải vi phạm `generationFence` tại transition 4, khi yêu
   cầu cũ ở generation 1 đưa `heart` trở lại sau lần gỡ (`reaction-unfenced-counterexample.json`);
   `mutatedNoopStep` (`fenceReaffirm = false`: luật cũ, xin reaction đang có là no-op ở mọi
   generation) phải vi phạm `confirmedIntentFence` tại transition 3, khi tab B “không reaction”@1
   không tạo fence và Heart@1 cũ hơn được áp (`reaction-noop-fence-counterexample.json`);
   `generationFence` không bắt được trace này. Runner kiểm nội dung counterexample, không chỉ exit
   code.

Rust replay `domain::discussion::reaction_replay` đưa chỉ command vào `decide_reaction`,
`ReactionTally::apply`, `ModerationAction::apply`, `change_mode` và `public_tally`, rồi so outcome,
chế độ, trạng thái/count từng bình luận, reaction của mọi (actor, bình luận) và count công khai với
state của model. Tạm cho `ActorReaction::desire` chấp nhận mọi thay đổi bất kể generation làm replay
đỏ ở `desired_state step 8` (tab cũ); đặt lại kiểm “trùng trạng thái” trước generation làm replay
đỏ ở `desired_state step 11` (gỡ hai lần là reaffirm).
`the_no_op_reaction_counterexample_is_refused_by_production` đưa input của counterexample qua
production và đòi `moderated`, `reaffirmed`, `stale` (không reaction, generation 2, count 0).

```sh
./node_modules/.bin/quint typecheck reaction.qnt
./node_modules/.bin/quint run reaction.qnt --backend=typescript --seed=41 \
  --step=step --max-steps=40 --max-samples=1000 --invariant=reactionSafe --verbosity=1
```

## Model ghi nhận văn học (LVE, #73)

`endorsement.qnt` của PR #80 dùng cùng `verify.mjs`/pin 0.33.1 và lane discussion.
[Policy](../../../../docs/architecture/literary-endorsement.md) và
[registry, bounds, replay, negative control và gaps](../../../../docs/development/endorsement-core.md)
là canonical owners. Simulation seed41, 1000 samples ×40 transitions; scenario 33 inputs qua
Rust production; mutant bỏ no-op fence tạo OFF→old ON resurrection ở transition2.
Không exhaustive model checking. Giữ toàn bộ mode/reaction reaffirmation và negative controls #79.

## Model admission và hậu kiểm (#120)

`admission.qnt` (lát DISC-01, [quyết định](../../../../docs/architecture/discussion-admission.md))
mô tả **một thread công khai đang mở**, người viết `1..3`, bình luận `1..3` khi simulation, một
đồng hồ trừu tượng theo tick (attestation quá 2 tick là cũ). Mỗi action là một commit đã tuần tự
hóa; đọc standing/trust tại commit, khóa SQL, outbox, lease và retry của worker là **giả định**
(DISC-03/04). Duyệt/gỡ của moderator và người viết dùng lại vòng đời ở version hiện tại; authority thuộc
`lifecycle.qnt`. Model cho operator chọn `trusted-cohort.v1` làm policy hiệu lực **chỉ để kiểm ứng
viên** trước một delta D-08; production `PolicySelection` không biểu diễn được lựa chọn đó.

| Action | Quyết định production được replay |
| --- | --- |
| `submit` | `decide_admission` (premoderation hiệu lực, có/không shadow) hoặc `decide_initial_review(trusted-cohort.v1)`; `Admission::initial_state`, `post_review` |
| `moderate` / `remove` | `ModerationAction::apply` ở version hiện tại / `remove_by_author` |
| `post_review` | `decide_post_review` với ticket đã lưu; job luôn được ACK |
| `standing`/`attest`/`trust`/`thread`/`policy`/`tick` | Shell tổng hợp: chưa có authority trust, fence standing hay cấu hình policy |

Invariant, gộp trong `admissionSafe`: `admissionRecordsProvenance` (bình luận mới present, version
1, ghi policy hiệu lực; approved ⇔ `trusted_and_clear`), `refusalWritesNothing` (từ chối đúng khi
standing khác active, không ghi gì), `premoderationAdmitsPending`, `approvalNeedsEveryGuard` (duyệt
⇒ ứng viên hiệu lực, active, thread chuẩn, trust còn tươi, rules sạch/tươi/đúng payload, không cờ),
`shadowOnlyMeasures`, `ticketsOnlyForPublishFirst`, `postReviewFenced` (chỉ rút duyệt tại version
quan sát, không bao giờ duyệt, chỉ một bình luận), `removalTerminal`, `scopedVersionedWrites`,
`finiteDomain`.

Runner chạy: simulation `step` seed `41`, **1.000 samples × 40 transitions**; scenario `rollout` 45
transition xuất `admission-fixtures.json` (`vink-discussion-admission-conformance-v1`): shadow đo một
lần duyệt trên bình luận được lưu pending, đăng-trước, mọi lý do giữ lại, hậu kiểm `held`/`kept`/
`stale`/`removed`, đình chỉ/không rõ standing, kill switch giữ bình luận đã duyệt. Runner đòi đủ 13
reason/refusal, 9 outcome và hai điều kiện non-vacuity đó. Ba negative control (mỗi mutant giữ
nguyên semantics khác): `mutatedShadowStep` áp quyết định shadow → vi phạm
`premoderationAdmitsPending` tại transition 3; `mutatedStaleTrustStep` bỏ kiểm tuổi trust → vi phạm
`approvalNeedsEveryGuard` tại transition 6; `mutatedUnfencedStep` bỏ fence version của hậu kiểm sau
khi người từ chối rồi duyệt lại → vi phạm `postReviewFenced` tại transition 6. Runner kiểm nội dung
từng counterexample, không chỉ exit code.

Rust replay `domain::discussion::admission_replay` đưa chỉ command vào các quyết định trên rồi so
outcome, reason, shadow, bình luận (review, gỡ, version, policy, reason) và ticket với state của
model; một tick là `TRUST_MAX_AGE_MICROS / 2`. `the_admission_counterexamples_are_refused_by_production`
replay tiền tố hợp lệ của ba counterexample và đòi production không tới state của mutant.

```sh
./node_modules/.bin/quint typecheck admission.qnt
./node_modules/.bin/quint run admission.qnt --backend=typescript --seed=41 \
  --step=step --max-steps=40 --max-samples=1000 --invariant=admissionSafe --verbosity=1
```

**Không được chứng minh:** authority trust/standing thật, ngưỡng tuổi (ứng viên), rules/classifier
thật, nhiều thread, rate limit, provenance bền, outbox/lease/retry/dead-letter (xem `outbox.qnt` ở
phần sau, chỉ cho hạ tầng; hậu kiểm có hiệu lực chưa có), liveness của hàng đợi.

## Model outbox (DISC-04, #120)

`outbox.qnt` ([quyết định](../../../../docs/architecture/discussion-outbox.md)) mô tả **một event
nợ hai delivery** (hai consumer) và **hai worker** claim, chết, dừng quá lease rồi quay lại. Lease 2
tick, backoff 1 tick nhân đôi tới trần 2, tối đa 3 attempt, đồng hồ ≤ 8. Mỗi action là một commit
đã tuần tự hóa: claim, hoàn tất có fence (effect + outcome cùng transaction), crash (worker quên claim,
không ghi gì), tick, requeue của operator. Khóa hàng, `SKIP LOCKED`, savepoint, timeout, supervisor
và PostgreSQL là **giả định** ở đây và có bằng chứng ở lane `postgres`; deadline không đạt trong model
(unit/Kani phủ).

| Action | Quyết định production được replay |
| --- | --- |
| `claim` | `decide_claim` (đến hạn, lease hết hạn → reclaim, hết budget → dead letter) |
| `complete` | `decide_completion` với `HandlerResult::Applied/Retry/Fail` và generation worker nghĩ mình giữ |
| `requeue` | `decide_requeue` |
| `crash`, `tick` | Shell: bỏ claim của worker; tăng đồng hồ |

Invariant `outboxSafe`: `effectAtMostOnce`, `effectIffSucceeded`, `attemptsBounded`, `leaseShape`,
`holdsNeverAhead`, `fenceMonotone`, `successIsFinal`, `staleWritesNothing`, `scoped` (chỉ delivery
được nêu thay đổi), `finiteDomain`. Scenario `leaseScenario` (26 bước) đạt 12 outcome; fixture
`outbox-fixtures.json` được `outbox_replay::quint_outbox_inputs_replay_against_the_production_decisions`
replay. Negative control `mutatedUnfencedStep` bỏ fence generation: worker A quay lại sau khi B
reclaim và hoàn tất, effect bị áp lần hai (vi phạm `effectAtMostOnce` ở transition 6);
`the_unfenced_completion_counterexample_is_refused_by_production` chứng minh production trả `stale`.

```sh
node node_modules/@informalsystems/quint/dist/src/cli.js run outbox.qnt --backend=typescript --seed=41 \
  --step=step --max-steps=40 --max-samples=1000 --invariant=outboxSafe --verbosity=1
```

**Không được chứng minh:** transaction/khóa PostgreSQL, mạng, vòng đời process, nhiều event cạnh
tranh, deadline, thứ tự FIFO (không hứa), liveness (worker sống cuối cùng xử lý hết).
