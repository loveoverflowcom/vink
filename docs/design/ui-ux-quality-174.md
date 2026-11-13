# #174 — Chất lượng UI/UX: tài liệu, skills và pilot

**Ngày:** 12/11/2026 (UTC+07) · **Base:** `origin/develop@e54db0d6f3e1b1caae901cbff31027cb3d49d757`
· **Nhánh:** `codex/ui-ux-quality-docs-skills` · **Issue:**
[#174](https://github.com/loveoverflowcom/vink/issues/174) (EPIC, vẫn mở sau lát này).

Lát tài liệu ban đầu cập nhật skills UI hiện hữu (W1–W4) và chạy một pilot hẹp trên dialog/sheet
ở preview Leptos và CMP Desktop (W5). Nó **không** đổi giao diện sản phẩm, không thêm runner,
không duyệt mốc thị giác và không nghiệm thu native. Bốn chiều trạng thái giữ tách nhau:
quyết định sản phẩm (không đổi), quyết định kiến trúc (không đổi), mức triển khai (tài liệu và
skills) và bằng chứng (các lượt chạy ghi ở §5).

**Cập nhật tiếp theo:** [vòng sửa 1](#10-vòng-sửa-1-f-01-và-các-gap-bằng-chứng) trên
`5221871923ca2935988a225d827e1c4ed9c234c0` sửa F-01 và mở rộng nhỏ hai runner hiện hữu.
§1–9 giữ lịch sử của pilot đầu; trạng thái hiện tại và phần còn mở nằm ở §10.

## 1. Vì sao và làm thế nào, qua một tác vụ cụ thể

Tác vụ: *“Dialog xác nhận xóa bản nháp trông kém hài hòa so với concept, nhất là khi tăng chữ.”*

| | Trước lát này | Sau lát này |
| --- | --- | --- |
| Ý định | LayoutSpec có `Hierarchy` nhưng không nói concept là loại mốc gì, phần nào phải giống | Spec ghi `Intent`, `References` (concept ≠ style anchor ≠ baseline, kèm trạng thái duyệt) và `Invariants` có đơn vị |
| Bằng chứng | Một ảnh dialog ở chữ 100%; “PNG tồn tại” dễ bị coi là đủ | Ảnh sạch 100% **và** 200%, metadata đúng lượt; số đo chỉ cho anchor mà invariant cần, ghi `measured`/`derived`/`unavailable` |
| Review | Người sửa tự báo “đạt”; không tách geometry với thị giác | Inspector mở ảnh trước khi đọc tự đánh giá, ghi `self-review`, báo năm chiều; geometry đạt không suy ra phân cấp đạt |
| Sửa | Chỉnh tới khi “trông ổn”, có thể sửa đi sửa lại | Tối đa hai vòng có tái render, đối chiếu `fixed/still/new/regressed`, dừng khi dao động; hết vòng không thành PASS |
| Kết luận | “Đã sửa, test pass” | Bảng năm chiều theo scenario/target, 100% và 200% báo riêng, `NO_APPROVED_REFERENCE` nếu chưa có mốc |

## 2. Inventory và source map (W1, AC-01)

Đọc tại `e54db0d`. Cột “Delta” là thay đổi của lát này; mỗi quy tắc có một nơi định nghĩa, nơi
khác chỉ link.

| Owner | Đã có | Lệch hoặc thiếu | Delta |
| --- | --- | --- | --- |
| [layout-spec.md](../../.agents/skills/vink-layout-spec/references/layout-spec.md) | Cây vùng, `Hierarchy`/`Responsive`/`States`/`Interaction`/`Compare`, bản rút gọn | Không có chỗ ghi loại mốc, phần phải giống/được đổi, invariant có đơn vị | Thêm `Intent`, `References`, `Invariants`, mục “Mốc tham chiếu và invariant”; vẫn là Markdown, không DSL |
| [interaction-ux.md](../../.agents/skills/vink-ui-design/references/interaction-ux.md) (#172) | Hợp đồng 9 yếu tố, bản 4 dòng, nguyên tắc motion/gesture/focus/IME | Debounce “300–500ms” và timeout “10–30s” viết như hằng số nhưng không có owner; loading gate có nguồn token nhưng không dẫn | Sửa cách diễn đạt ba dòng đó; không làm lại nội dung #172 |
| [component-audit.md](../../.agents/skills/vink-ui-inspector/references/component-audit.md), [inspector](../../.agents/skills/vink-ui-inspector/SKILL.md) | Inventory, chiều kiểm, severity/confidence, so sánh reference | Finding thiếu owner/giả thuyết và metric; không tách geometry với visual; không có bảng kết quả; mức P0–P3 của capture chưa nối với tên mức | Chuỗi finding đầy đủ, hai ca dễ nhầm, bảng năm chiều, `self-review`, ánh xạ P0–P3 |
| [evidence-contract.md](../../.agents/skills/vink-ui-capture/references/evidence-contract.md), [capture](../../.agents/skills/vink-ui-capture/SKILL.md) | Manifest format 1, `capture_status`, `inspected`, focused capture, `references[].authority` | Không có hợp đồng cho số đo theo anchor và overlay | Mục “Số đo anchor optional”, tương thích format 1; ghi khả năng thật của từng runner |
| [repair-and-handoff.md](../../.agents/skills/vink-ui-fix/references/repair-and-handoff.md), [fix](../../.agents/skills/vink-ui-fix/SKILL.md) | Bug contract, owner, lane, trước/sau cùng scenario | Không có ngân sách vòng sửa, dấu hiệu dao động, cách dừng | Mục “Vòng sửa có điểm dừng”, `REFERENCES`/`FINDINGS` trong mẫu bàn giao |
| [design-system.md](../../.agents/skills/vink-ui-design/references/design-system.md), [SKILL](../../.agents/skills/vink-ui-design/SKILL.md) | Token, chữ Việt, state, a11y, motion | Không có rubric đạt/không đạt; 100% và 200% chưa tách mục tiêu; concept/anchor/baseline chưa phân loại | [ux-quality.md](../../.agents/skills/vink-ui-design/references/ux-quality.md) mới: rubric, mốc, ngân sách, bảng năm chiều |
| [EXTENDING.md](../../packages/ui-system/EXTENDING.md) | Admission, ngưỡng tương phản, checklist component | Checklist không nối với tiêu chí review | Một câu link tới rubric; không chép checklist |
| [prompt-template.md](../../.agents/skills/vink-prompt/references/prompt-template.md) | `LAYOUT_SPEC`, ACCEPTANCE/VERIFY cho interaction | Không truyền tiêu chí quyết định, loại mốc, ngân sách | `UX_CRITERIA` và hai dòng ACCEPTANCE/VERIFY |
| [.agents/skills/README.md](../../.agents/skills/README.md) | Routing interaction (#172) | Chưa có đường từ ý định tới review và sửa có giới hạn | Mục “UI quality loop” với năm tình huống |
| `vink-cmp-mobile`, `vink-leptos-web`, `vink-maintain-verification`, `vink-prompt` | Ranh giới renderer và evidence theo target | Chưa dẫn tới tiêu chí chung; giới hạn của số đo từng renderer chưa nêu | Mỗi skill một đoạn ngắn |
| [#24](https://github.com/loveoverflowcom/vink/issues/24) | Qualification, completeness của capture, host/frame/specimen, focused capture | Vẫn mở (AC-07…AC-19) | **Không sửa, không tick.** Gap tooling mà pilot gặp được nêu ở §6 để bàn giao cho owner đó |

**Chưa kiểm chứng tại lát này:** hai ảnh dialog/sheet do người dùng báo (không có metadata về
renderer, cỡ chữ hay revision nên không coi là lỗi trên `develop` hiện tại); chính sách hai vòng
sửa mới được thử bằng một tình huống dao động trên giấy và chưa qua một vòng sửa thật; số đo
anchor trên CMP (`unavailable`); mọi ranh giới native, OS IME và screen reader.

**Ranh giới với #172 và #24:** #172 sở hữu nội dung interaction (không viết lại); #24 sở hữu
runner capture, geometry/completeness và qualification theo target. Lát này sở hữu phương pháp,
quy tắc review, kết nối skills và pilot chứng minh dùng được.

## 3. Lựa chọn trong lát này (CHOICE của #174)

Chủ sản phẩm giao agent tự chốt các điểm cần thiết trong phiên 12/11/2026. Các lựa chọn dưới đây
là **chính sách pilot có thể đảo lại**, ghi ở tài liệu này và ở
[ux-quality.md](../../.agents/skills/vink-ui-design/references/ux-quality.md); chúng không được
nhập vào [kho quyết định TOML](../../decisions/README.md) và không là signoff cho một mốc thị
giác hay một gate bắt buộc.

| ID | Chọn | Lý do và điều kiện xem lại |
| --- | --- | --- |
| CHOICE-01 | **A — khuyến nghị, gap hiển thị** | Chưa có số liệu false positive và chưa có mốc được duyệt. Xem lại khi có vài pilot nữa và runner đo được anchor của lớp phủ |
| CHOICE-02 | **A — một style anchor cho archetype pilot** | Mới có một ứng viên (`ANCHOR-CAND-01`, trạng thái `candidate`) trong [sổ mốc](visual-anchors.md). Việc duyệt ảnh cụ thể vẫn thuộc chủ sản phẩm |
| CHOICE-03 | **A — đo anchor chọn lọc bằng probe/runner sẵn có** | Bốn anchor của dialog đủ giải thích mọi finding hình học của pilot; không cần sidecar chung |
| CHOICE-04 | **A — tối đa hai vòng sửa có tái render**, dừng sớm khi dao động | Dễ hiểu cho phạm vi nhỏ. Lát này chưa chạy vòng sửa thật nào nên chưa có số liệu để đổi |

Giả định giữ nguyên từ issue: không áp dụng tỷ lệ khoảng trắng 35%, line-height 1.35/1.2, “x bằng
tuyệt đối” hay thời gian chụp từ bản góp ý làm tiêu chuẩn.

## 4. LayoutSpec của pilot

Dùng để review hai specimen sẵn có; không phải thiết kế mới và không đổi UI.

```text
LayoutSpec: pilot dialog/sheet r1 — proposed (agent, 12/11/2026), chỉ dùng để review
Context: người viết xác nhận một hành động hoặc nhập nhanh trong lớp phủ; specimen
  `state.dialog/default|sheet` (Leptos) và `composition.interaction/default|sheet` (CMP); chữ [Mẫu]
Intent: dialog/sheet; điểm tập trung là câu hỏi ở tiêu đề, rồi tới hành động an toàn
Sources: develop@e54db0d; feedback.rs, vink-ui.css, VinkFeedback.kt, VinkActions.kt,
  ui-interaction-foundation.md
References: không có mốc được duyệt (ANCHOR-CAND-01 là candidate)

Overlay #overlay
  Region #scrim — che nền; bấm ngoài theo policy của từng specimen
  Region #panel — nền surface-container-high, cuộn riêng khi nội dung dài
    Region #title — câu hỏi hoặc tên tác vụ (heading)
    Region #content — hậu quả, field hoặc lựa chọn
    Region #actions — hành động an toàn trước, hành động chính/phá hủy sau

Hierarchy: #title đọc trước; đúng một hành động nổi nhất; hành động phụ là tonal hoặc text
Responsive: dialog giữa khung, sheet neo đáy ở màn hẹp
Interaction: theo interaction-ux.md và ui-interaction-foundation.md (Escape/bấm ngoài/Back,
  focus ban đầu vào lựa chọn an toàn, trả focus về trigger)
Invariants (viewport-css, CSS px hoặc dp; dung sai 0, không có ngoại lệ):
  INV-1 #title nằm trọn trong vùng nhìn của #panel ngay khi mở
  INV-2 #panel nằm trọn trong viewport
  INV-3 #actions nằm trong #panel hoặc tới được bằng cuộn của #panel
  INV-4 không cuộn ngang ở cấp trang
Compare: 100% — nhịp và phân cấp; 200% — đọc được câu hỏi lúc mở, tới được mọi hành động,
  không cuộn ngang. Hai mục tiêu báo riêng.
```

## 5. Pilot (W5)

### Môi trường và lệnh

Linux x86-64, 22 CPU, Temurin 17.0.20.1, Node 24.22.3, Google Chrome 157.0.8103.78. Checkout
`e54db0d6f3e1b1caae901cbff31027cb3d49d757`; các lượt chạy diễn ra khi cây làm việc đã có sửa đổi
**chỉ ở tài liệu** (`.agents/skills`, `docs/`, `packages/ui-system/EXTENDING.md`).
`git status --porcelain` trên `packages/ui-system/{shared,leptos,cmp}` và
`packages/vink-patterns/{leptos,cmp}` rỗng; vì vậy `focus-manifest.json` ghi `dirty: true` với
`source_fingerprint` `97a3004b…0484f` của nguồn UI không đổi.

| Lệnh (cwd = repo root) | Kết quả | Thời gian đo | Ranh giới |
| --- | --- | --- | --- |
| `VINK_UI_FOCUS_CAPTURE=1 VINK_CHROME=<wrapper --no-sandbox> cargo xtask ui-preview leptos verify` | PASS: 4/4 test clip, 658 assertion, 1.233 PNG, 814 context, focus 36/36 (66 tile) | 16 phút 40 giây (một lượt, cache build ấm) | Chrome headless, preview mẫu; wrapper chỉ để Chrome chạy trong sandbox của công cụ |
| `cargo xtask ui-preview cmp verify` | PASS: `:ui` 43, `:diagnostics` 6, `:preview` 48 test, 0 skip; 668 PNG capture | 7 phút 32 giây (một lượt) | JVM + Compose Desktop off-screen; không Android/iOS |
| `cargo xtask ui-preview leptos run` + thăm dò trong browser pane | 5 trạng thái lớp phủ được đo | Không đo | **Thăm dò thủ công**, Chrome 156 giả lập mobile, DPR 2; không phải output của runner |
| Android/iOS diagnostic, OS IME, TalkBack/VoiceOver | NOT_RUN | — | Không thuộc lát này; kết quả 12/11 ở [qualification](../development/ui-system-qualification-evidence.md#interaction-foundation-12112026) không được chạy lại |

Hai con số thời gian là số đo một lượt trên máy này, không phải SLA.

### Ảnh đã mở

Ảnh nằm ở [evidence/ui-ux-quality-174](evidence/ui-ux-quality-174/README.md) kèm hash; ảnh CMP
và ảnh raw của Web giữ nguyên tên runner.

| Target | Đã mở | Ghi chú |
| --- | --- | --- |
| CMP Desktop | 8/8 `interaction--{default,sheet}--{light,dark}--360--text{100,200}.png` (360×800, density 1) | 5 ảnh được lưu làm bằng chứng |
| Web, runner | `state.dialog--default--open--{light,dark}--390.png` (390×6000), xem qua bản cắt vùng panel | Bản cắt là ảnh phái sinh, tên có `--crop` |
| Web, thăm dò | 4 screenshot browser pane: dialog 320×700 và 390×844 ở chữ 200%, sheet 320×700 ở 100% và 200% | JPEG đã thu nhỏ; số đo ở [probe-measurements.json](evidence/ui-ux-quality-174/web/probe-measurements.json) |

Chưa mở: 1.231 PNG còn lại của preview Leptos và các capture CMP khác; `focus-manifest.json` vẫn
ghi `inspected: false` cho cả 36 record và không được sửa.

### Findings

Review là `self-review` (cùng agent chạy và đọc), `fidelity: NO_APPROVED_REFERENCE`.

| ID | Vùng / scenario / target | Quan sát | Evidence / số đo | Tác động | Mức độ / độ chắc chắn | Owner hoặc giả thuyết | Thay đổi nhỏ đề xuất | Kiểm lại |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| F-01 | `#title` của `state.dialog/default`, Web, 320×700 và 390×844, chữ 200% (root 32px), light | Dialog mở ra đã cuộn tới hành động; tiêu đề nằm ngoài vùng nhìn (320) hoặc bị cắt mép trên và nút “Xóa” nằm dưới mép (390) | Đo: 320 — `#panel` y=32 cao 636, `scrollTop` 658/658, `#title` y=−578; 390 — `scrollTop` 53, `#title` y=27 < 32, “Xóa” y=828 > 812. Ở chữ 100% INV-1 đạt | Người dùng thấy “Giữ bản nháp / Xóa” mà không thấy câu hỏi xóa cái gì | **Cao** trong điều kiện này / quan sát: cao, nguyên nhân: vừa | Giả thuyết có nguồn: `focus_initial` trong `packages/ui-system/leptos/src/feedback.rs` gọi `focus()` lên hành động đầu tiên nên trình duyệt cuộn panel | Giữ focus an toàn nhưng không để nó kéo cuộn (hoặc focus tiêu đề khi nội dung dài hơn panel); cần chọn một hướng | INV-1 ở 320/390, chữ 200%, hai theme; chạy lại assertion Tab/Escape/trả focus |
| F-02 | `#panel` cùng scenario với F-01 | Lề và padding nhân theo chữ: cột nội dung còn khoảng 160–173/320 CSS px; tiêu đề 7 dòng, “Giữ bản nháp” gãy 3 dòng (cao 167,6), hai nút khác bề rộng | Đo: margin 32px, padding 48px, `#title` rộng 172,7; nút 172,7×167,6 và 154,5×89,2 | Nhịp và phân cấp ở 200% kém rõ; phải cuộn 1.294px nội dung trong khung 636px | Vừa / cao | Đã đọc: `.vk-ui-dialog` và `.vk-ui-dialog__panel` trong `vink-ui.css` dùng `--vk-space-4`/`--vk-space-6` theo rem | Là lựa chọn thiết kế (`NEEDS_HUMAN_DESIGN_DECISION`): có giới hạn lề/padding của panel ở màn hẹp khi chữ lớn hay không | Cùng phép đo; so số dòng tiêu đề và chiều cao cuộn |
| F-03 | Nút text “Chuyển vùng [Mẫu]”, CMP Desktop, `interaction--default--light--text200`, `interaction--sheet--light--text100` (mờ hơn ở dark) | Nút text có nền xám như nút đặc/disabled; ở `default--light--text100` thì không | Ảnh đã mở; vị trí nút trùng vị trí trigger vừa được click. Anchor bounds: `unavailable` | Reviewer dễ đọc thành hai nút đặc cạnh tranh, sai với `Hierarchy` | Thấp cho sản phẩm, Vừa cho độ tin của ảnh / quan sát: cao, nguyên nhân: vừa | Giả thuyết: con trỏ giả lập còn hover tại chỗ trigger, và `VinkButton` (VinkActions.kt) dùng `Button` không truyền `elevation` nên trạng thái hover vẽ bóng dưới container trong suốt. Sidecar không ghi vị trí con trỏ | Ghi hoặc dời con trỏ trước khi chụp (owner capture, #24); xét bỏ elevation cho biến thể Text/Outlined (owner `:ui`) | Chạy lại `ui-preview cmp verify`, mở lại ba ảnh |
| F-04 | Sheet, CMP Desktop, `interaction--sheet--{light,dark}--text200` | Ảnh chỉ có phần cuối nội dung và nút đóng; không thấy tiêu đề và field; sheet phủ hết 360×800, không còn scrim | Ảnh đã mở; sidecar ghi `state: open, close action scrolled into view`. Đã đọc: `#actions` nằm trong cùng `Column` cuộn với nội dung (VinkFeedback.kt) | Ảnh là `partial` cho review tiêu đề/phân cấp ở 200%. Về UX: nút đóng trong panel ở cuối vùng cuộn và không còn vùng bấm ngoài | Vừa / vừa (một vị trí cuộn, native NOT_RUN) | Capture: cần tile đầu vùng cuộn (#24). UX: hành động ghim hay cuộn theo nội dung là lựa chọn thiết kế đã ghi trong interaction foundation | Thêm tile ở đầu vùng cuộn; nếu muốn đổi hành vi thì `NEEDS_HUMAN_DESIGN_DECISION` | Mở đủ tile; thao tác đóng trên host native |
| F-05 | Capture dialog mở của preview Leptos | Ảnh raw 390×6000 đặt dialog giữa canvas 6.000px (tâm panel khoảng y≈3.000, suy từ pixel) vì capture đổi chiều cao viewport theo document; runner không có ảnh dialog mở ở 200% và không có ảnh sheet mở | Ảnh đã mở; `contexts.json` chỉ có `state.dialog--default--open--{light,dark}--390` | Không đánh giá được vị trí panel trên màn hình thật; F-01 không thể bị runner hiện tại bắt | Vừa (chất lượng bằng chứng) / cao | `packages/ui-system/leptos/preview/e2e/preview-smoke.mjs` (`capture`) | Phần mở rộng nhỏ nhất ở §6 | Chạy `ui-preview leptos verify`, đọc số đo trong context |
| F-06 | Nút “Giữ bản nháp”, Web, 390, chữ 100%, light | Nền tonal của nút gần trùng nền panel nên nút đọc như nút text; “Xóa” là điểm nhấn duy nhất | Bản cắt đã mở; tương phản chưa đo | Không cản tác vụ; nhãn vẫn nhận ra | Đề xuất / vừa | Role màu tonal trên `surface-container-high` | Không đề xuất sửa khi chưa có mốc được duyệt | Đo tương phản đã resolve nếu muốn theo đuổi |

Không finding nào được sửa trong lát này: epic không ngầm giao sửa giao diện, và F-02/F-04 cần
quyết định thiết kế. Số vòng sửa đã dùng: 0/2.

### Kết quả theo năm chiều

**Web — preview Leptos, `state.dialog` default và sheet**

| Chiều | Scenario / target | Kết quả | Giới hạn |
| --- | --- | --- | --- |
| Functional | Ba kịch bản dialog render đúng link; hết phiên giữ chữ (Chrome headless 157) | PASS | Specimen mẫu, không phải app |
| Geometry | INV-1…4, 320×700: chữ 100% dialog và sheet | PASS | Thăm dò thủ công; runner: NOT_RUN vì không đo anchor của lớp phủ |
| Geometry | INV-1, chữ 200%: dialog 320×700 và 390×844 | FAIL (F-01); sheet 320×700 PASS | Chỉ theme light; root font-size, không phải browser zoom |
| Visual | Dialog mở 390, light/dark, chữ 100% (ảnh runner, xem bản cắt) | PASS, kèm F-06 là đề xuất | `self-review`; `NO_APPROVED_REFERENCE`; vị trí trên viewport không đánh giá được (F-05) |
| Accessibility/Responsive | Dialog 320 và 390, chữ 200% | FAIL (F-01, F-02); sheet 320 PASS | Dark ở 200%, tương phản và screen reader: NOT_RUN |
| Interaction | Tab ở trong dialog, Escape, bấm ngoài theo policy, trả focus (assertion của smoke, chữ 100%) | PASS | Bàn phím giả lập; ở chữ 200% NOT_RUN; lượt thăm dò mở bằng script nên không kết luận về trả focus |

**CMP — Desktop off-screen, `composition.interaction` default và sheet**

| Chiều | Scenario / target | Kết quả | Giới hạn |
| --- | --- | --- | --- |
| Functional | Mở → đóng → trigger nhận lại focus, 8 tổ hợp theme/chữ/kịch bản | PASS | JVM; không có cửa sổ hay bàn phím hệ điều hành |
| Geometry | INV-1…4 | NOT_RUN | Sidecar không có bounds theo anchor (`unavailable`); thuộc #24 |
| Visual | Dialog 360, chữ 100%, light/dark | PASS | `self-review`; `NO_APPROVED_REFERENCE` |
| Visual | Dialog light chữ 200%, sheet light chữ 100% | FAIL với tư cách bằng chứng (F-03) | Chưa tách được lỗi ảnh với lỗi component |
| Accessibility/Responsive | Dialog chữ 200%: tiêu đề 3 dòng, hành động xếp dọc và thấy được | PASS | Multiplier của preview, không phải OS font scale |
| Accessibility/Responsive | Sheet chữ 200% | Chưa kết luận: ảnh `partial` (F-04) | Cần tile đầu vùng cuộn |
| Interaction | Chạm, Back, IME, TalkBack/VoiceOver trên Android/iOS | NOT_RUN | Desktop không thay native |

### Đối chứng âm

Mỗi dòng là một lỗi có thật hoặc một bản sao bị sửa có chủ đích, và phép kiểm đã bắt nó.

| Loại lỗi | Dữ liệu | Phép kiểm | Kết quả |
| --- | --- | --- | --- |
| Ảnh thiếu vùng | 36 record của `focus-manifest.json` | So `bounds_css` của specimen với chiều cao ảnh raw `document-capped` | 12/36 record (đều 390, chữ 200%; specimen bắt đầu ở y≈10.641, ảnh raw cao 6.000) không chứa specimen; ảnh focus/tile của chính các record đó thì đủ |
| Ảnh thiếu vùng | CMP `interaction--sheet--*--text200.png` | Mở ảnh và đối chiếu ba vùng của spec | Thiếu `#title` và field (F-04); đếm PNG hay kiểm kích thước không bắt được |
| Metadata sai | Sidecar CMP 8 record; bản sao đổi `heightDp` 800→640 | So header PNG với `widthDp/heightDp × density` | Dữ liệu thật: 8/8 khớp; bản sao: phát hiện lệch |
| Metadata không tương thích | Manifest focus 36 record; ba bản sao đổi theme, đổi `text_scale`, đổi hash và chiều cao | Tính lại SHA-256 và kích thước 66 tile; đối chiếu `surface` với theme, `root_font_size` với `text_scale`, viewport, font đã tải | Dữ liệu thật: 0/36 lỗi; cả ba bản sao bị bắt đúng loại (`theme/surface`, `text_scale/root_font`, `pixels`+`sha256`) |
| Invariant hình học bị vi phạm | Web, dialog ở chữ 200% | INV-1 trên số đo thật | FAIL ở 320 và 390, PASS ở chữ 100% và ở sheet |
| Invariant hình học bị vi phạm | Test `specimenClips` của runner | `node --test` trong `ui-preview leptos verify` | 4/4 PASS, gồm ca bounds NaN/rỗng/ngoài document phải ném lỗi |
| Hình học đạt, phân cấp chưa đạt | CMP `interaction--default--light--360--text200.png` | Mở ảnh, so với `Hierarchy` của spec | Không vùng nào tràn hay bị cắt trên ảnh, nhưng hành động phụ hiện như nút đặc thứ hai (F-03). Lưu ý: hình học ở đây là quan sát trên ảnh vì anchor CMP `unavailable` |

Các phép kiểm metadata là lệnh một lần của reviewer (đọc header PNG, `sha256`); không thêm runner
vào repo.

## 6. Bài học đưa ngược về tài liệu và gap bàn giao

Đã cập nhật trong chính lát này:

- **Rubric §1.4 và ví dụ invariant của LayoutSpec:** nội dung dài hơn khung ở 200% vẫn phải thấy
  phần mở đầu lúc mở (từ F-01).
- **Capture SKILL:** ghi vị trí con trỏ/hover và vị trí cuộn; lớp phủ neo viewport chụp ở viewport
  thật; nội dung dài cần tile hoặc ghi `partial` (từ F-03, F-04, F-05).
- **Contract evidence:** thao tác kích hoạt bằng script không dùng để kết luận về trả focus hay
  hover (từ chính lượt thăm dò: mở bằng `click()` rồi Escape không trả focus về trigger, trong
  khi assertion của smoke dùng bàn phím thì đạt).
- **Skill Leptos:** chữ 200% của preview là root font-size; capture full-page không cho biết vị trí
  lớp phủ.

Gap tooling, **không** làm trong PR tài liệu; bàn giao cho owner runner trong phạm vi #24:

1. Preview Leptos: chụp lớp phủ đang mở ở viewport thật (không đổi chiều cao theo document) cho
   `state.dialog` default và sheet tại 320/390 × chữ 100/200 × light/dark, ghi bốn anchor
   (`#panel`, `#title`, `#actions`, viewport) cùng `scrollTop` và kết quả INV-1…4 vào context của
   capture. Đây là phần mở rộng nhỏ nhất làm cho F-01 lặp lại được bằng runner.
2. CMP: bounds theo anchor trong sidecar của `InteractionCaptureTest`, vị trí con trỏ, và tile đầu
   vùng cuộn cho sheet ở chữ 200%.
3. Overlay phái sinh (vẽ bounds lên bản sao ảnh sạch): chưa tạo trong pilot, NOT_RUN.

Lỗi giao diện tìm được (F-01, F-02, F-03 phần component, F-04 phần UX) thuộc owner `:ui`/Leptos
`vink-ui` và cần phiên sửa riêng theo vòng có ngân sách.

## 7. Acceptance của #174 theo bằng chứng

Checkbox trên issue không được tick trong lát này; bảng dưới là đề xuất trạng thái để chủ sản phẩm
đối chiếu.

| AC | Trạng thái đề xuất | Bằng chứng / giới hạn |
| --- | --- | --- |
| AC-01 | Đạt | §2; không sửa #24, không viết lại #172 |
| AC-02 | Đạt | Rubric có ví dụ đạt/không đạt và cách kiểm; LayoutSpec có bản dài và rút gọn; không ngưỡng thẩm mỹ chung |
| AC-03 | Đạt về quy tắc | Ba loại mốc, quy tắc duyệt, sổ mốc. **Chưa có mốc `approved`** |
| AC-04 | Một phần | Hợp đồng số đo tương thích format 1; có số đo thật của Web ở dạng thăm dò. Overlay chưa tạo; CMP `unavailable` |
| AC-05 | Một phần | Chuỗi finding và chính sách dừng đã viết và áp dụng cho findings; tình huống dao động mới là ví dụ viết, chưa có vòng sửa thật |
| AC-06 | Đạt | Mục “UI quality loop” và năm tình huống; không thêm skill critic |
| AC-07 | Đạt | Bảng năm chiều dùng trong §5, 100% và 200% báo riêng, native NOT_RUN |
| AC-08 | Một phần — **không tick** | Web và CMP có lượt chạy thật, ảnh đã mở, metadata và đối chứng âm. Phần Web ở chữ 200% và sheet là thăm dò thủ công, chưa lặp lại được bằng runner; không có native |
| AC-09 | Đạt sau các lệnh ở §8 | Không thêm runner/CLI; lệnh dùng đúng checkout |
| AC-10 | Đạt | Tài liệu này; không tự duyệt mốc, không tick/đóng #24 hay #174 |

## 8. Kiểm chứng của thay đổi tài liệu

| Lệnh | Kết quả |
| --- | --- |
| `git diff --check` | PASS |
| `cargo xtask verify quick` | PASS cả 9 bước, gồm `repository checks (decisions, skills, links, boundaries, fonts, editor packages)`. Hai lượt đầu dừng vì ổ đĩa đầy (`No space left on device`, linker Bus error), không phải do nội dung; lượt thứ ba chạy sau khi dọn cache build cũ trong `target/debug` |
| GitHub CI | Không kiểm, theo `GITHUB_CI: NOT_REQUIRED` của issue |

`cargo xtask verify web`, `verify postgres` và các lane model không chạy: lát này không chạm app
Web, store hay nghiệp vụ.

## 9. Còn mở

- Chủ sản phẩm: duyệt hay bác `ANCHOR-CAND-01`; mức bắt buộc sau pilot (CHOICE-01); hướng xử lý
  F-02 và F-04.
- Owner runner (#24): ba gap ở §6.
- Owner `:ui`/`vink-ui`: F-01 và F-03, qua một vòng sửa có ngân sách để thử chính sách dừng bằng
  dữ liệu thật (phần còn thiếu của AC-05).
- Native: toàn bộ bảng năm chiều trên Android/iOS cho dialog/sheet ở cỡ chữ lớn.

## 10. Vòng sửa 1: F-01 và các gap bằng chứng

Ngày 12/11/2026, theo yêu cầu sửa F-01 và phần dang dở rồi tạo commit mới. Base
`5221871923ca2935988a225d827e1c4ed9c234c0`, cùng nhánh `codex/ui-ux-quality-docs-skills`.
Quyết định sản phẩm/kiến trúc không đổi; implementation thêm **web** ở `vink-ui::Dialog` và
capture/test ở preview Web + CMP Desktop. Không tạo app native hay tự duyệt anchor/gate.

### Tiêu chí cố định và thay đổi

LayoutSpec rút gọn đã trình bày trước patch: giữ `#panel → #title → #content → #actions`,
INV-1…4 của §4, fixture/copy/tokens hiện hữu, 320×700 và 390×844, chữ 100/200%, light/dark.
Dialog vừa khung vẫn focus hành động an toàn; khi nội dung cần cuộn thì focus heading có
`tabindex=-1`, mở ở đầu nội dung. `[autofocus]` rõ ràng của caller vẫn được ưu tiên. Escape,
outside-dismiss, thứ tự Tab và trả focus giữ hợp đồng hiện có. Cách chọn heading cho nội dung
dài phù hợp [WAI dialog pattern](https://www.w3.org/WAI/ARIA/apg/patterns/dialog-modal/#keyboardinteraction).
Spec sửa này chỉ đổi focus khởi đầu của Web; CMP giữ `initialFocus` và nút đóng cuộn theo nội dung.

- **F-01:** `focus_initial` ở `feedback.rs` đưa scroll về 0 (cả khi mở lại), chọn heading khi
  `scrollHeight > clientHeight`, gọi focus với `preventScroll`. Native `showModal()` đã chạy
  bước focus trước adapter nên chỉ đổi `focus()` sang `preventScroll` là chưa đủ.
- **F-05:** `overlay-capture.mjs` là module của smoke hiện hữu, chạy mặc định trong
  `ui-preview leptos verify`: 16 tổ hợp, ảnh viewport thật trước mọi corrective scroll,
  ảnh tại actions, DOM rect/font/scroll/focus, INV-1/2/4 lúc mở, INV-3 bằng Tab tới từng nút,
  Escape/trả focus, mở lại và bấm ngoài. Ảnh raw gallery cũ và `focus-manifest` giữ nguyên.
- **F-03 và F-04 phần capture:** `InteractionCaptureTest` đưa con trỏ ra ngoài trước chụp,
  giữ ảnh vừa mở, chụp từ đầu tới cuối scrollport bằng tile có overlap và kiểm không bỏ khoảng
  giữa. Sidecar giữ các trường cũ, thêm bounds/visible bounds, scroll range, pointer, hash,
  nguồn và ảnh overlay. `field-label` là node field tìm bằng accessible label trong semantics,
  không phải bounds của dòng nhãn được vẽ hay glyph.
  Không cần đổi elevation của `VinkButton` để có ảnh sạch.
- **Overlay:** vẽ bounds trên bản sao PNG sạch cùng lượt; Web dùng canvas tách khỏi DOM,
  CMP dùng `BufferedImage`. Check xác nhận capture không đổi geometry/scroll/focus.
  Ảnh sạch giữ nguyên byte và là evidence chính.

### Đối chứng trước/sau

Cùng Chrome headless, DPR 1, font pin, text scale bằng root font-size; khác với probe mobile
DPR 2 của §5 nên **không** dùng số của probe cũ làm baseline pixel. Before được chạy trên
UI chưa sửa; after là vòng sửa 1. SHA và fingerprint nguồn ở sidecar từng lượt trong
[evidence vòng sửa](evidence/ui-ux-quality-174/repair-1/README.md).

| Scenario light, chữ 200% | Trước: `scrollTop`, title y | Sau: `scrollTop`, title y | Kết luận |
| --- | --- | --- | --- |
| Dialog 320×700 | 658, −578 | 0, 80 | INV-1 được sửa |
| Dialog 390×844 | 201, −121 | 0, 80 | INV-1 được sửa |
| Sheet 320×700 | 108, 45 (panel y=105) | 0, 153 | Cùng nguyên nhân, được sửa |
| Sheet 390×844 | 0, 334,06 | 0, 334,06 | Không hồi quy |

Before: **6/16** ca FAIL (ba ca trên × hai theme), cả tám ca chữ 100% đạt. After:
**16/16** đạt các invariant và kiểm bàn phím/cancel/mở lại. Đây là đối chứng âm chạy trên
implementation thật, không chỉ mock DOM. Unit test geometry còn từ chối title bị cắt một
phần, bounds NaN/rỗng/thiếu, viewport bị cắt và overflow ngang.

### Findings sau vòng 1

Review: `self-review`, `fidelity: NO_APPROVED_REFERENCE`; tiêu chí và mốc không đổi giữa lượt.

| Finding | Delta | Kết luận / phần còn lại |
| --- | --- | --- |
| F-01 | **fixed** | Tiêu đề thấy trọn khi mở/mở lại, focus trong vùng nhìn; Tab tới được mọi nút ở 16 tổ hợp |
| F-02 | **still** | Lề/padding theo rem vẫn làm chữ 200% gãy nhiều dòng; lựa chọn spacing còn cần thiết kế, không đổi thành PASS nhờ geometry |
| F-03 | **fixed — capture** | Ảnh trung tính hết nền hover trên nút text sau pointer exit. Chưa kết luận chất lượng style hover trong tương tác thật |
| F-04 | **fixed — capture; still — UX choice** | Có ảnh vừa mở và tile phủ toàn vùng cuộn, thấy heading/field/nút đóng; lựa chọn ghim nút hoặc giữ cuộn vẫn mở |
| F-05 | **fixed** | Runner chụp/đo viewport thật, có dialog và sheet chữ 200% ở cả hai theme |
| F-06 | **still — đề xuất** | Không đổi màu tonal khi chưa có mốc được duyệt |
| Mới / hồi quy | Không thấy trong phạm vi đã kiểm | Không suy thành nghiệm thu toàn UI hay native |

Đã dùng **1/2 vòng sửa UI**; dừng vì các lỗi kỹ thuật được chọn đã đạt và phần còn lại cần
quyết định thiết kế. Không chạy thêm vòng chỉ để đổi thẩm mỹ. Đối chứng dao động dùng
`openingInvariants` chạy trên chuỗi artifact **before → after → before**: số ca lỗi
**6 → 0 → 6**, delta `fixed → regressed`; áp policy dừng và giữ nguồn after của vòng 1,
không đổi mốc hay đánh dấu đạt khi hết budget. Kết quả ở `bounded_review_replay` trong
[review.json](evidence/ui-ux-quality-174/repair-1/review.json). Đây là replay artifact mô phỏng
vòng kế tiếp, không phải sửa/render UI lần hai hay một hồi quy thật của source cuối.

### Kết quả theo năm chiều

| Chiều | Web preview, 16 tổ hợp | CMP Desktop, 8 tổ hợp | Giới hạn |
| --- | --- | --- | --- |
| Functional | PASS: mở, hủy, mở lại; không gọi dịch vụ | PASS: mở, đóng, trả focus | Specimen mẫu |
| Geometry | PASS INV-1…4; số đo lúc mở tách khỏi vị trí actions | PASS panel/title trong khung 8/8, tile đủ 8/8; overflow ngoài anchors: NOT_RUN | Node bounds không phải glyph/paint; không suy touch hit-test |
| Visual | PASS 100%; 200% hết mất tiêu đề nhưng FAIL về nhịp/spacing F-02 | PASS trên các ảnh đã mở: ảnh sạch, đầu/cuối sheet đọc được | `self-review`; không có anchor được duyệt |
| Accessibility/Responsive | PASS phần heading thấy được, reflow, keyboard của F-01; spacing F-02 còn | Đủ ảnh 100/200% cho heading/content/actions | Contrast mới, screen reader, OS text scale: NOT_RUN |
| Interaction | PASS Tab, Escape, outside policy, trả focus, mở lại | PASS close/return focus và cuộn bằng test semantics | Android/iOS, OS IME, TalkBack/VoiceOver: NOT_RUN |

### Kiểm chứng và acceptance còn lại

Lệnh, kết quả cuối, hash và danh sách ảnh đã mở được ghi tại
[evidence vòng sửa](evidence/ui-ux-quality-174/repair-1/README.md). `--overlays-only 1` là
chọn phạm vi của script smoke hiện hữu để tái hiện nhanh; lane đầy đủ vẫn chạy cùng ma trận.
`verify quick` đạt đủ 9 bước; `ui-preview cmp verify` đạt. Toàn bộ browser smoke Web chạy lại
trên bundle đã build đạt 660 assertion, 846 capture records và 1.297 PNG; lượt lane đầu bị
SIGTERM trong gallery được giữ riêng trong báo cáo, không gộp thành lane PASS.

| AC #174 | Delta của vòng sửa 1 |
| --- | --- |
| AC-04 | Đã có clean image + metrics + overlay cùng lượt ở Web và CMP; contract format 1 không đổi; glyph metric vẫn `unavailable` |
| AC-05 | Đã chạy một vòng sửa thực có baseline FAIL, after PASS và bảng `fixed/still/new/regressed`; điểm dừng/dao động không tự biến thành đạt |
| AC-08 | Web 200% và sheet đã lặp lại bằng runner; CMP có tile đủ vùng và metadata. Bằng chứng chỉ cho Web + CMP Desktop, không nghiệm thu native |
| AC-09/10 | Cập nhật owner commands/feature map/contract evidence, giữ lịch sử và ghi phần mở; không tick/đóng issue hoặc cập nhật #24 |

Còn cần chủ sản phẩm: `ANCHOR-CAND-01`, mức bắt buộc sau pilot, hướng spacing F-02 và UX của
F-04. Ranh giới thiết bị vẫn cần lượt Android/iOS/OS IME/screen reader riêng; việc sửa source
và test Desktop không thay những bằng chứng đó.

## 11. Vòng sửa 2: F-02, lề và padding của dialog/sheet ở chữ lớn (13/11/2026)

Nhánh `codex/ui-large-text-spacing`, base `origin/develop@7d9dc54f63a2b759cc5979007dd4f665fe0ec86f`. Chủ
sản phẩm yêu cầu “xử lý F-02”; quy tắc dưới đây do agent triển khai đề xuất. **Đã được chủ sản phẩm
duyệt ngày 13/11/2026** (lời duyệt nguyên văn: “Duyệt quy tắc”, trả lời đề nghị xác nhận quy tắc của
đúng PR này ở HEAD `84413ef15a3d89ba2efdc0c9ec1e239738deda05`), nên F-02 không còn
`NEEDS_HUMAN_DESIGN_DECISION`. Lời duyệt chỉ nói về **quy tắc spacing** (hai biến CSS dưới đây): nó không
duyệt ảnh nào làm mốc, nên mốc vẫn là `NO_APPROVED_REFERENCE` và sổ
[visual-anchors](visual-anchors.md) không đổi. Quy tắc đảo được bằng cách đổi hai biến CSS.

**F-02 được đo lại và hóa ra có phần khách quan.** Ngoài “cột hẹp”, ở dialog 320×700 chữ 200%
từ dài nhất (≈173 CSS px) rộng hơn cột còn lại (130 CSS px) nên **tràn qua padding bên phải**
(đo: trái 48, phải ≈ 5), và `Giữ bản nháp` xuống ba dòng trong một nút gần tròn. Đây là invariant
mới **INV-5**: tiêu đề và vùng hành động nằm giữa hai mép trong của panel (sau padding), dung sai
1 CSS px; mép trong tính từ `clientWidth` của dialog (đã trừ scrollbar) và padding tính toán thật.

```text
LayoutSpec (rút gọn): dialog/sheet dùng chung, chữ lớn trên màn hẹp
Intent: đọc câu hỏi, chọn hành động; không đổi cấu trúc, thứ tự hay CTA.
Regions (giữ nguyên): #panel > #title, #context, #content, #actions.
Rule (mới): margin = clamp(space-2, 5vw, space-4); gutter = clamp(space-3, 10cqi, space-6),
  dùng cho padding panel và padding ngang của nút trong #actions; dialog là size container.
Hierarchy/Interaction: không đổi (focus vào, trả focus, dismiss policy, cuộn: INV-1…4 giữ).
Invariants: INV-5 (mới); ở chữ 100% mọi số đo giữ bằng trước (padding 24 CSS px, cột 225/295 ở 320/390).
Compare: 320×700 và 390×844 × sáng/tối × 100%/200%, dialog và sheet.
```

Lý do chọn đơn vị: `5vw` và `10cqi` là cận theo bề rộng thật (viewport; bề rộng chính overlay) nên
không phụ thuộc cỡ chữ; hai token (`space-2/3` làm sàn, `space-4/6` làm trần) giữ giá trị cũ ở chữ 100%
trên mọi điện thoại từ 320 CSS px (margin 16, padding 24) và trên dialog rộng. Không thêm token.
Cách không chọn: giới hạn cứng bằng px (không theo cỡ chữ của người dùng), media query theo `rem`
(rem trong media query lấy cỡ chữ mặc định của trình duyệt, không theo cỡ chữ gốc đã đổi),
ép nút chiếm cả hàng (không giải quyết padding ngang của nút).

| Ca (light = dark), root 32px | Cột chữ trước → sau (CSS px) | Dòng tiêu đề | Số dòng nhãn nút | INV-5 |
| --- | --- | --- | --- | --- |
| Dialog 320×700 | 130 → 203 | 7 → 6 | 3 → 2 | **FAIL → PASS** |
| Dialog 390×844 | 200 → 254 | 6 → 4 | 3 → 1 | PASS → PASS |
| Sheet 320×700 | 194 → 244 | 4 → 3 | 1 → 1 | PASS → PASS |
| Sheet 390×844 | 279 → 300 | 2 → 2 | 1 → 1 | PASS → PASS |
| Cả bốn ca ở chữ 100% | 225/257/295/327 → giữ nguyên | giữ nguyên | giữ nguyên | PASS → PASS |

Số đo từ `overlay-contexts.json` (`measurements.inner/padding/setting`), viewport thật, DPR 1, Chrome
headless, chữ bằng root font-size (không phải zoom). Số dòng và chiều rộng cột được **ghi**, chỉ INV-5
được **kiểm**: không có ngưỡng thẩm mỹ chung. Chữ 200% ở 320 CSS px vẫn dùng nhiều dòng hơn 100%
(đó là bản chất của chữ gấp đôi), không còn tràn padding.

Ảnh đã mở (trước/sau cùng scenario, theme, viewport, font, scale; JPEG thu nhỏ trong
[repair-2](evidence/ui-ux-quality-174/repair-2/)): dialog 320×700 sáng chữ 200% — tiêu đề tràn sát mép
phải và “Xóa / bản / nháp” từng từ một dòng → hai lề cân, “Xóa bản” trên một dòng; dialog 390×844 tối
chữ 200% — “Giữ / bản / nháp” ba dòng trong nút gần tròn → một dòng. **fixed:** F-02 phần tràn
padding và nhãn nút; **still:** không (cột ở chữ 200% vẫn hẹp hơn chữ 100% theo bản chất); **new/
regressed:** không thấy ở 16 ca. Đã dùng 1/2 vòng sửa của lát này.

**Counterpart và consumer.** CMP `VinkFeedback.kt` đặt padding theo `VinkDimens` (dp, không nhân theo
cỡ chữ) nên không có hiện tượng này: **không áp dụng**, không đổi CMP. Consumer trong app dùng đường
chung: `tag-picker` (Khám phá) nhận quy tắc mới; `#draft-preview` và `#leave-editor` có kích thước và
padding do trang quyết định (`vink.css`, “Page-owned dialog sizing”, giữ nguyên) và chỉ nhận gutter
mới ở padding ngang của nút.

**Reproducer và kiểm chứng.** Trước khi sửa, cùng smoke (`--overlays-only 1`) với INV-5 chạy trên CSS
chưa đổi: **FAIL 2/16** (dialog 320×700 chữ 200%, sáng và tối); sau sửa **16/16** đạt INV-1…5, bàn phím,
hủy, mở lại và chính sách bấm ngoài. Unit test `overlay-capture.test.mjs` giữ số đo F-02 thật (cột
130, tiêu đề 172,7) làm ca bị từ chối và từ chối bounds thiếu/không hữu hạn.

| Lệnh (local, không dùng GitHub CI) | Kết quả |
| --- | --- |
| `node --test …/overlay-capture.test.mjs` | PASS (3 test) |
| `cargo xtask ui-preview leptos verify` | PASS — 838 capture; ma trận overlay 16 ca |
| `cargo xtask verify quick` | PASS (9 bước, gồm kiểm tra liên kết/skill/quyết định) |
| `cargo xtask verify web` | PASS — bảy phase. Lần chạy đầu có **một** lỗi ở `review-withdrawn` (“the case UI renders the submitted snapshot”, kiểm tra ngay sau khi nút nhận ca hiện, không chờ nội dung snapshot) không liên quan tới dialog; chạy lại nguyên lane PASS. Ghi nhận như một race của kiểm tra có sẵn, chưa sửa trong lát này |
| `ui-preview cmp verify`, host Android/iOS | **NOT_RUN** — không đổi CMP (padding dp, không nhân theo chữ); Desktop/Web không thay native |
| `verify postgres`/`discussion`/`kani` | **NOT_RUN** — không đổi backend |

| Chiều | Scenario / target | Kết quả |
| --- | --- | --- |
| Functional | mở, Tab trong dialog, Escape + trả focus, mở lại, bấm ngoài theo policy (16 ca) | PASS |
| Geometry | INV-1…5 ở 320×700 và 390×844, 100% và 200%, hai theme, dialog và sheet | PASS (trước sửa INV-5 FAIL 2/16) |
| Visual | 4 ảnh trước/sau đã mở; còn lại đọc qua số đo | Cải thiện đo được; `NO_APPROVED_REFERENCE`, không tự thành baseline |
| Accessibility/Responsive | reflow không cuộn ngang, nhãn nút không còn ba dòng ở 390; chữ bằng root font-size | PASS phần đo; OS text scale, screen reader, contrast mới: **NOT_RUN** |
| Interaction | bàn phím giả lập (CDP) trên Chrome headless | PASS; Android/iOS, OS IME, TalkBack/VoiceOver: **NOT_RUN** |

Giới hạn: số đo lấy từ Chrome headless có scrollbar cổ điển 15 CSS px nên dialog hẹp hơn điện thoại thật
có scrollbar chồng; ở chữ 100% quy tắc cho giá trị token cũ khi bề rộng overlay ≥ 240 CSS px (đã đo
ở 320/390). Hướng spacing đã được chủ sản phẩm duyệt (xem đầu mục); muốn đổi thì đổi
`--vk-overlay-margin` và `--vk-overlay-gutter` và chạy lại INV-5.

