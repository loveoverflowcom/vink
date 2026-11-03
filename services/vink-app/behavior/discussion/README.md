# Model discussion

Thư mục giữ hai model Quint dùng chung một package pin và một runner: `structure.qnt` (cấu trúc
hai tầng, D2T-01) và `lifecycle.qnt` (vòng đời bình luận, bật/tắt và authority kiểm duyệt,
D2T-02, [phần dưới](#model-vòng-đời-bình-luận-d2t-02)).

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
| `submit` | `decide_submission` (không receipt, tác phẩm công khai): `closed`, `target_unavailable`, `created` pending v1 |
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
- `finiteDomain`.

**Không được chứng minh:** auth authority (D-05), receipt/idempotency (D2T-INV-06), rút bài, lý
do kiểm duyệt, hàng chờ/phân trang, neo root ẩn và context không còn hiển thị (INV-05), nhiều tác
phẩm/thread, thu hồi đang chạy song song với quyết định (chỉ thứ tự đã tuần tự hóa), report/appeal,
mở lại sau khi gỡ, liveness của hàng chờ.

Runner chạy thêm cho model này:

1. Random simulation `step`: seed `41`, **1.000 samples**, tối đa **40 transitions/sample**, kiểm
   `lifecycleSafe` sau init và mỗi transition.
2. Hai scenario xác định xuất ITF thành `lifecycle-fixtures.json`
   (`vink-discussion-lifecycle-conformance-v1`): `moderation_authority` 26 transitions (grant,
   tự né của tác giả-moderator và của người viết, stale, retry, thu hồi trước commit, gỡ terminal)
   và `comments_mode` 16 transitions (chỉ tác giả đổi, tắt chặn root/reply kể cả của tác giả,
   retry, tab cũ, bật lại, target pending, từ chối rồi duyệt lại). Runner đòi hai scenario cùng
   đạt đủ 16 outcome và 4 kiểu hiển thị, để invariant không đúng nhờ antecedent không đạt tới.
3. `mutatedRecusalStep` gọi `moderateWith(_, false)`: giữ nguyên mọi semantics khác nhưng bỏ tự né
   của tác giả tác phẩm. Runner đòi exit 1, ITF status violation và state thật trong đó tác giả
   (có grant) đã gỡ bình luận của người khác tại transition 3; lưu ở
   `author-recusal-counterexample.json`.

Rust replay `domain::discussion::lifecycle_replay` đưa **chỉ command** của từng step vào các quyết
định production theo thứ tự của store, rồi so outcome, chế độ, generation, grant, trạng thái từng
bình luận và `view` của 4 người xem với state của model. Bình luận, settings và tập grant nằm trong
shell bộ nhớ (được ghi rõ trong test); khóa và độ bền thuộc lane `postgres`. Tạm bỏ
`actor == work_author` trong `authorize_moderation` làm replay đỏ ở `moderation_authority step 5`.

Lệnh trực tiếp tương đương cho random simulation (chạy tại thư mục này):

```sh
./node_modules/.bin/quint typecheck lifecycle.qnt
./node_modules/.bin/quint run lifecycle.qnt --backend=typescript --seed=41 \
  --step=step --max-steps=40 --max-samples=1000 --invariant=lifecycleSafe --verbosity=1
```
