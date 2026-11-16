# Reader và Nhận xét — action surfaces #226

Ngày 16/11/2026 · [Issue #226](https://github.com/loveoverflowcom/vink/issues/226).
Base review đã fetch: `11c6a9c` (gồm #228 và #244).
Baseline thực thi cũ: `a81333c71faf6033014a00ec26c9f7b90109d1d9`.
Đây là LayoutSpec và bản đồ tích hợp, **chưa triển khai presentation mới**.
Kết quả chạy, AC và điều kiện tiếp tục nằm ở [evidence](../development/evidence/action-surfaces-226/README.md).

## Ý định, trạng thái và nguồn

Người đọc tìm thấy Thấm sau khi đọc bài; trong cuộc trò chuyện, họ phân biệt được Hữu ích,
Trả lời, Gửi và Gỡ mà nội dung vẫn là trọng tâm. Yêu cầu kế thừa: `UX-01`, `COM-03/04/07/08`,
`LVE-FR-01/02/03/06/08`, `LVE-UC-02/03/05/06/08/11/12/14/16` trong
[PRD](../../prd/requirements.md). Không đổi quyền, dữ liệu, policy hoặc luồng gửi duyệt.

| Trục | Trạng thái của lát #226 |
| --- | --- |
| Sản phẩm | Yêu cầu nguồn #226; không thêm quyết định sản phẩm hoặc signoff |
| Kiến trúc/trình bày | `DESIGN_PROPOSED`; CHOICE bên dưới là khuyến nghị kỹ thuật chờ render |
| Triển khai | Presentation #226 `NOT_IMPLEMENTED`; Web/CMP hiện hữu giữ nguyên |
| Bằng chứng | Source discovery + regression độc lập; không suy nghiệm thu UI mới từ baseline |

Tra [kho TOML](../../decisions/README.md): `LVE-WP-04-CHOICE-07@r1` giữ Pen Flight R2 A · Mềm
20px trang trí; `LVE-PR-01-CHOICE-08@r1` chặn self endorsement. Cả hai record nhập vẫn
`DESIGN_PROPOSED`; không nâng thành `Accepted` hoặc quyết định mới của chủ sản phẩm.
Nhãn hiện hành là **Thấm / Hữu ích**, không đổi lại theo ảnh cũ.

`References: NO_APPROVED_REFERENCE`. Ảnh own-content lịch sử được issue dẫn là
`BUILD_SHA_UNVERIFIED`, không làm baseline eligible reader. Ảnh baseline của lượt thực thi
chỉ chứng minh UI cũ tại source được ghim; cần before/after cùng fixture khi tích hợp #224.

## LayoutSpec r2 — đồng bộ issue, chưa triển khai

```text
R0 Reader (nền đọc yên, không bọc bài trong pill)
├─ R1 bài: tiêu đề → byline → đoạn/khổ thơ giữ nguyên
├─ R2 work endorsement: Pen Flight · Thấm · count → note/reconcile khi có
└─ R3 Nhận xét
   ├─ R3a composer gốc hiện có: textarea → Gửi → trạng thái gửi
   └─ R3b mỗi root: byline → body → actions
      ├─ Trả lời (nếu can_reply) · Gỡ nhận xét của tôi (nếu can_remove) · Hữu ích
      └─ một danh sách replies phẳng, mỗi reply cùng anatomy; composer trả lời hiện có
```

Thứ tự composer/root giữ đúng host hiện tại, không tạo thêm CTA “Viết nhận xét”, menu hay overlay.
Count nằm trong đúng target; không thêm count chip hoặc nút tương tác con. Note nằm bên dưới
target và được liên kết qua description; shared guest/unavailable reason vẫn dùng chung khi
đủ facts đồng nhất. Không lặp lại live announcements.

| Lựa chọn #226 | Khuyến nghị và đánh đổi | Điều kiện xác nhận |
| --- | --- | --- |
| CHOICE-01 | **A dự kiến**: work Tonal opt-in, comment Standard; selected dùng check + trạng thái xác nhận hiện hữu. Giảm nền trong thread dài; hai context cần recipe rõ tại composition, không CSS ngoại lệ rải rác | Paired capture thread ngắn/dài, đủ quyền/blocked/Unknown/pending/selected ở hai theme, chữ 100/200%. Nếu comment khó nhận diện, so với B neutral contained cùng fixture |
| CHOICE-02 | **A dự kiến**: Trả lời Text + leading reply icon đúng registry, Gỡ giữ tertiary hiện hữu, Gửi giữ Filled. Tiết chế background; touch cần đủ target không dựa hover | Capture cùng capability matrix và kiểm keyboard/touch. Nếu affordance chưa rõ, so B Tonal với cùng input; không lấy số nút có nền làm thước đo |

CHOICE-03 vẫn OPEN theo issue R2: A là count read-only chỉ tại Work+OwnContent+Idle
với ownership đã resolve và handoff focus an toàn; B giữ toggle bị khóa và một visible
reason qua `blocked_description`. Ưu tiên B nếu không chứng minh được lifetime/identity.
Không tháo target vì late own-read, self-refusal, phase/ACK hay breakpoint; không giấu
pending/uncertain/reconcile, không giả count 0. Cần regression cho Known/Unavailable/Hidden,
session/revision switch, target đang focus và user đã chuyển focus sang field khác.

AC-11: đầu Nhận xét dùng nhãn tĩnh “Cũ nhất trước”, không chevron/menu giả. Chỉ giảm copy
open khi capability/composer rõ; giữ closed/unavailable/error/pending, author toggle và
announcements. Không sửa văn bản, khổ thơ, indentation/font hoặc timestamp/timezone.

Các lựa chọn **chưa phải lựa chọn có visual evidence**. Registry trên base và snapshot #228
đang đọc không có reply glyph (`IconName::Reply` không tồn tại). Không dùng `ArrowBack` hoặc
`Edit` thay nghĩa reply, không tự thêm icon vào file generated. Cần owner shared registry thêm
SVG/provenance rồi generate qua `cargo xtask ui-tokens`, trước adoption Trả lời.
Composer reader hiện đã có Gửi và leading `ArrowBack` được recipe CSS hiện hữu xử lý;
lượt này không đổi glyph/flow đó.

`Responsive`: 320/390/834/1440 CSS px, light/dark, chữ 100/200% qua root font-size.
Actions và reason dài wrap theo nghĩa, nhãn nguyên dấu, không ellipsis/shrink target.
100% ưu tiên nhịp đọc; 200% vẫn phân biệt hành động và giữ thứ tự, không cuộn ngang.
`Invariants`: mỗi endorsement chỉ một target; Web ≥44 CSS px và giữ ≥48 ở endorsement
hiện có, CMP ≥48dp; Pen Flight 20px; một focus perimeter; không nested interactive;
không thêm tầng replies. Contrast/forced-colors đo trên render, không suy từ tên token.

`Interaction`: pointer/Enter/Space một dispatch; pending/uncertain chặn lệnh mới. Chỉ ACK/readback
xác nhận mới đổi selected/count. Unknown không là Off; unavailable không là 0; hidden không có
count. Self-content/guest/forbidden/unavailable có lý do truy cập được. Không đổi style thành
`aria-pressed=true`. Giữ DOM node/focus khi phase đổi, dùng recovery riêng cho uncertain;
không tự cướp focus từ nút recovery. ACK/read cũ và session khác do controller hiện hữu fence.
Trả lời focus composer đúng root; thu ô giữ chữ và trả focus target hợp lệ. Giữ controlled
text/IME khi chờ/đóng bình luận; không dùng việc tác giả sở hữu bài để cấp quyền gỡ người khác.

## Bản đồ owner và tích hợp

| Vùng | Owner hiện hữu | Seam sau prerequisite |
| --- | --- | --- |
| R1/R2/R3 order | [pages/mod.rs](../../apps/vink-web/src/pages/mod.rs) `PublicationContent` | Giữ article → `WorkEndorsement` → `Discussion` |
| R2 | [pages/endorsement.rs](../../apps/vink-web/src/pages/endorsement.rs) `WorkEndorsement` | Truyền surface Tonal opt-in tại work caller; không đổi session/request |
| R2 + Hữu ích | [patterns lib.rs](../../packages/vink-patterns/leptos/src/lib.rs) `EndorsementAction`, `comment_endorsement` | Dùng prop từ #224, comment giữ default Standard; không thêm toggle thay thế |
| R3b actions/order | Cùng `lib.rs`: `DiscussionThread`, `discussion_comment` | Trả lời nhận leading reply glyph; giữ capability và flat provider order; không sửa selector Desk/Discovery #225 |
| R3a/reply composer | `FeedbackComposer` cùng `lib.rs`; [discussion.rs](../../apps/vink-web/src/pages/discussion.rs) | Giữ `tonal=true`, `#root-composer-send`, `#reply-composer-{root}-send`, thu ô/giữ chữ/recovery. Không tạo composer mới |
| Geometry | [patterns.css](../../packages/vink-patterns/leptos/patterns.css) `.vk-discussion-actions`, `.vk-endorsement`; [vink.css](../../apps/vink-web/style/vink.css) page glue | Ưu tiên shared surface; chỉ sửa wrap/spacing đúng composition nếu capture chứng minh cần |
| Confirmed state | [toggle.rs](../../packages/ui-system/leptos/src/toggle.rs), [endorsement_state.rs](../../apps/vink-web/src/endorsement_state.rs) | Consume, không fork; request key/order/session fence không thuộc thay đổi presentation |
| CMP counterpart | [Patterns.kt](../../packages/vink-patterns/cmp/src/commonMain/kotlin/vink/patterns/Patterns.kt) | `EndorsementAction` tồn tại độc lập. `DiscussionThread` có reply/remove, **chưa nhận CommentEndorsements như Web**. Ghi seam parity đúng scope; không giả có adapter native hoặc tự mở feature binding mới |
| Preview | [catalog](../../packages/ui-system/shared/preview-catalog.json) `screen.reader`, `composition.feedback` (`discussion-flat`, `discussion-anchor`), `composition.endorsement` | Fixtures diagnostic, không dữ liệu sản phẩm/CMP native |

## Dependency và cách tiếp tục

Review 16/11 trên `11c6a9c`: #228 đã merge tại `2c55c267`.
Rust `ActionSurface::{Standard,Tonal}`, prop `ToggleAction.surface` và
`EndorsementAction.surface` đã có; CMP cũng chuyển tiếp surface. Standard là mặc định.
`blocked_description` là DOM id của reason hiển thị, không phải text override.
#244 thêm `FilterList`; registry vẫn không có `Reply`. Đây là dependency riêng của leading
reply glyph, không chặn việc dùng surface cho work. Không fork primitive/CSS.

1. Tiếp tục từ develop sau PR review thay thế #242; không apply/merge #242 lần nữa.
2. Áp recipe tại caller theo contract hiện hữu; resolve reply glyph qua shared registry/provenance.
3. Giữ baseline trước sửa; chạy regression mới cùng `verify quick`, `verify web`,
   `ui-preview leptos verify`; chạy `ui-preview cmp verify` nếu đổi CMP.
4. Capture eligible + own-content + guest và các phase thực, paired before/after 16 tổ hợp
   viewport/theme/text scale ở trên. Review theo rubric tối đa hai vòng, ghi fixed/still/new/regressed.
   Full matrix không thay screen reader/OS IME/native target; báo riêng `NOT_RUN` nếu chưa thực hiện.

Không thêm model, migration, endpoint, provider, policy hay native product app.
