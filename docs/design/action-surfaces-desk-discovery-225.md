# Action surfaces Bàn viết / Khám phá — #225

LayoutSpec r1 · 16/11/2026 · `DESIGN_PROPOSED` · `NO_APPROVED_REFERENCE`.
Đây là đặc tả và bộ kiểm hồi quy chuẩn bị cho [#225](https://github.com/loveoverflowcom/vink/issues/225),
chưa triển khai delta giao diện. Chủ sản phẩm đã giao thực thi; dependency thiếu thì làm spec/tests
độc lập, không nhập toàn bộ PR khác hoặc fork primitive.

## Trạng thái sau rebase để review PR #240

Đối chiếu `develop@2c55c2678729b4db39fadd9ce1bcfedfb4175770` ngày 16/11/2026:
PR #228 đã merge; `ActionSurface::{Standard,Tonal}` và prop `surface` của
`TooltipIconButton` đã có trong shared owner. Registry vẫn có 23 glyph và chưa có glyph lọc/thẻ.
Phạm vi PR #240 vẫn là spec/tests độc lập; chưa đổi các consumer hoặc hoàn thành #225.
Khi tiếp tục, dùng API shared hiện hành và xử lý glyph ở shared owner; không cần chờ lại #228.
Các lựa chọn presentation bên dưới vẫn dự kiến. Baseline và evidence cũ giữ nguyên SHA lúc chạy.

## Baseline và dependency tại lượt chuẩn bị `69f71f0`

- Branch `codex/action-surfaces-desk-discovery`, base `origin/develop` =
  `69f71f0cb61228dbe5ceea775b3c34c237705922`. Issue #225 và #224 chưa có comment khi đọc.
- [#228](https://github.com/loveoverflowcom/vink/pull/228) là draft/open tại head
  `64d732aef1b0be6a5f72271eebdfe80a2bc636c0`; không dùng làm base hoặc cherry-pick.
  Trạng thái PR không phải bằng chứng contract vắng: đã kiểm source `origin/develop` trực tiếp.
- `packages/ui-system/leptos/src/action.rs` có `ButtonVariant::{Filled,Tonal,Text,…}` và
  `IconButton`, nhưng chưa có `ActionSurface`. `src/support.rs::TooltipIconButton` chưa nhận
  hoặc chuyển tiếp surface. Cần contract opt-in có nền độc lập `pressed`, giữ state/name/focus
  và default. API `ActionSurface::Tonal` trong draft #228 chỉ là **interface ứng viên**, phải đọc
  lại API thật trên develop trước khi viết caller.
- `shared/icons.json` / `leptos/src/icons.rs` có 23 glyph, gồm Add và Refresh; chưa có glyph
  lọc/thẻ. Cần shared owner đăng ký glyph có nghĩa + provenance, generate Web/CMP và kiểm contract.
  Không dùng Explore (điểm đến), Search (tìm chữ), Check (đang chọn) thay nghĩa bộ lọc; không nhúng SVG
  hoặc CSS riêng tại page. #228 chưa tự giải quyết seam glyph này.
- Không sửa Editor trong `desk.rs`, Dialog, Appearance, row metrics, Thấm hoặc API/DB/cache.
  #229 / draft #232 không là base; không bắt đầu #226. Không thay trạng thái issue phụ thuộc.

## Bố cục dự kiến

```text
Screen #desk — tác giả tìm và tạo bản thảo
  Region #heading — Bàn viết; create có Add và container
  Region #drafts
    Region #draft-controls — SearchField; Select Sắp xếp; Refresh
    Region #draft-results — compact rows / trạng thái tác giả / số liệu readonly hiện hành
    Region #draft-continuation — tải tiếp / lỗi tiếp trang hiện hành
  Region #first-use — chỉ khi GET đầu không lọc xác nhận rỗng: CTA nằm trong empty panel

Screen #explore — người đọc tìm tác phẩm
  Region #public-search — native GET, q + hidden tag, giữ SearchField
  Region #applied-filters — launcher Thẻ Tonal + leading glyph thẻ/lọc đã đăng ký; applied links
  Region #community-stream — kết quả và link phân trang href thật
  Region #tag-dock OR Overlay #tag-picker
    Region #tag-form — checkbox dạng bài/chủ đề, tối đa 4, AND
    Action #apply — native submit riêng sau lựa chọn
```

| Action | Baseline trên develop | Target sau dependency |
| --- | --- | --- |
| Tạo, list có nội dung / query miss | Text + Add, một target | Filled + Add, giữ đầy đủ nhãn |
| Tạo lần đầu | Filled + Add trong panel, không CTA header | Giữ nguyên một target |
| Tạo pending / uncertain / rejected / reconciled | Guard và copy do `CreateState` sở hữu | Chỉ presentation; không replay POST |
| Refresh | Standard TooltipIconButton + Refresh | Tonal opt-in do shared wrapper nhận; không pressed giả |
| Search / Select | SearchField / Select native | Giữ anatomy, không bọc pill thứ hai |
| Mở bộ lọc | Tonal, nhãn Thẻ, không icon | Giữ Tonal, thêm glyph thẻ/lọc có nghĩa |
| Chọn thẻ | Native checkbox, selected cue | Giữ checkbox; không biến thành command |
| Áp dụng | Filled khi khác tập applied, Tonal khi cùng tập | Giữ đúng logic đã có, kể cả đổi thứ tự lựa chọn |
| Applied chip / phân trang / reader return | Link có href | Giữ link và GET/SSR/no-JS |

**CHOICE-01: A dự kiến** — Filled + Add ở header: tạo là hành động chính trong vùng heading;
danh sách vẫn là nội dung chính bên dưới. B (Tonal) dịu hơn nhưng chưa có paired capture chứng minh
đủ nhận diện trên surface cha. Chưa coi lựa chọn dự kiến là visual acceptance hoặc signoff.

**CHOICE-02: A dự kiến** — Refresh Tonal opt-in, Select giữ surface riêng. B (một group tonal
chứa Standard icon) tăng inset quanh Select đã có container. Chờ contract và width budget thật;
không dựng group thay thế chỉ để né dependency. Hai lựa chọn phải kiểm cùng fixture trước/sau
320/390/834/1440 × light/dark × text100/200, tối đa hai vòng sửa có capture.

Ở 100%, đọc theo heading → CTA → search/tools → tác phẩm; CTA nổi rõ mà list không thành bảng pill.
Ở 200%, full label và dấu tiếng Việt còn đọc/bấm được; tăng chiều cao/wrap, không co font/target.
Giữ một padding owner, một focus perimeter; glyph trong nút không có nền mini riêng. Target tối
thiểu 44 CSS px, giữ 48 ở control hiện dùng 48. DOM/focus order giữ nguyên khi breakpoint đổi.

## Interaction và invariants

- Pointer/Enter/Space dùng activation native đúng loại. Tạo gửi một POST; Pending/Unknown chặn
  lần tiếp theo. Reconcile chỉ GET, sau đó người dùng chủ động tạo thêm. Refresh/read không mutate.
- `draft_list`, `CreateState` và session scope vẫn sở hữu quyền/epoch/guards. Không thêm authority
  từ variant, không đưa private author facts vào SSR/public cache.
- Query buffer/input identity/caret không đổi khi variant đổi hoặc refresh nền. IME Enter không
  gửi partial text. Giữ error/stale/rejected/uncertain khác empty.
- Bộ lọc draft khác applied; đổi checkbox chưa đổi URL. Max4 AND; áp dụng giữ q/tag, reset cursor.
  Cùng tập applied trở về Tonal; Escape/dismiss trả focus trigger hợp lệ, điều hướng không kéo focus
  về trang cũ. GET, Back/Forward/reload, reader return giữ context.
- Không nested interactive; một native submit trong mỗi filter form, một create target ở state
  đã sẵn sàng. Native links/checkbox/select giữ vai trò, tên và href/name.
- Không overflow ngang ở 320 CSS px / chữ200; rect label và leading nằm trong target (dung sai
  1 CSS px do subpixel). Focus ring không clip; chụp cùng state/theme/scale, mở pixels thật.

## Evidence và đường tiếp tục

Ba ảnh Drive trong issue đã mở ngày 16/11: discovery 1440/light/100, 390/light/100 và
desk 390/dark/200. `BUILD_SHA_UNVERIFIED`, fixture 15/11, chỉ là **historical reference**;
ảnh Desk còn resume card, không khôi phục nó. Không ảnh nào là approved/golden/current runtime.

Tests độc lập mở rộng `e2e/browser-smoke.mjs` (Desk) và `e2e/discovery-smoke.mjs` (public search,
discovery corpus), không tạo phase/runner mới. Chúng bảo vệ semantics/guards/GET/Apply hiện có;
không gắn test PASS cho presentation chưa viết. Kết quả và checklist AC→evidence nằm ở
[bàn giao local](../development/evidence/action-surfaces-225/README.md).

Sau khi shared contract/glyph có trên develop: fetch, đối chiếu interface, tích hợp có chủ đích
trên branch này theo quyền phiên tiếp tục; sửa riêng `CreateDraftButton`, Refresh ở `DraftListView`
và launcher trong `DiscoveryPage`. Không cần đổi logic Apply. Dùng `cargo xtask verify quick`,
`cargo xtask verify web` và các phase hiện hữu `desk-layout` / `public-search` trên server mới build;
`discovery` kiểm corpus + cursor. Thêm assertion container/icon thật khi đã có implementation.
`ui-preview leptos verify` chỉ bắt buộc nếu shared pattern thực sự đổi; consumer không sở hữu nó.
