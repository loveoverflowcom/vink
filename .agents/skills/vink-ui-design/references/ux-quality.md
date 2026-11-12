# Chất lượng UX: rubric, mốc thị giác và vòng review có điểm dừng

Do `vink-ui-design` sở hữu, lập ngày 12/11/2026 cho
[#174](https://github.com/loveoverflowcom/vink/issues/174). Tài liệu nối các owner đã có thành
một đường **ý định → triển khai → bằng chứng → review → sửa có giới hạn**. Nó không chép lại nội
dung của họ, không là thang điểm thẩm mỹ và không tự duyệt thiết kế.

## Mỗi quy tắc có một nơi định nghĩa

| Cần gì | Owner duy nhất |
| --- | --- |
| Ý định bố cục, vùng, state, mốc tham chiếu và invariant của một tác vụ | [LayoutSpec](../../vink-layout-spec/references/layout-spec.md) |
| Hợp đồng tương tác: trigger, commit/cancel, focus, IME, reduced motion | [Interaction UX](interaction-ux.md) |
| Token, chữ, màu, hình, motion baseline | [Hệ thiết kế](design-system.md), `packages/ui-system/shared/tokens.json` |
| Ảnh, metadata, số đo anchor, overlay | [Contract evidence](../../vink-ui-capture/references/evidence-contract.md) |
| Finding, severity, confidence, coverage | [Component audit](../../vink-ui-inspector/references/component-audit.md) |
| Sửa đúng owner, check theo rủi ro, bàn giao | [Repair và handoff](../../vink-ui-fix/references/repair-and-handoff.md) |
| Lệnh, lane, readiness theo target | [Development guide](../../../../docs/development/README.md), [qualification](../../../../docs/development/ui-system-qualification-evidence.md) |
| Mốc thị giác đã duyệt | [Sổ mốc thị giác](../../../../docs/design/visual-anchors.md) |

Tài liệu này chỉ sở hữu bốn thứ: **rubric** (§1), **phân loại mốc thị giác** (§2), **ngân sách
vòng sửa** (§3) và **bảng kết quả theo chiều** (§4). Sửa nhỏ chỉ cần đọc dòng rubric liên quan
và §4; không nạp toàn bộ cho một lần đổi câu chữ.

## 1. Rubric theo loại trải nghiệm

Chọn các dòng áp dụng cho vùng đang làm; không chấm một điểm chung cho cả màn. “Không áp dụng”
là câu trả lời hợp lệ. Rubric không tạo ngưỡng số mới: chỉ dùng số mà owner đã công bố (tương
phản và vùng chạm trong [EXTENDING](../../../../packages/ui-system/EXTENDING.md) và
[components](../../../../packages/ui-system/shared/components.md), giá trị trong `tokens.json`)
hoặc invariant mà LayoutSpec của tác vụ ghi kèm đơn vị, dung sai và lý do. Tỷ lệ khoảng trắng,
line-height hay bề rộng dòng lấy từ một ví dụ không phải luật toàn ứng dụng.

### 1.1 Tác vụ và phân cấp

| Câu hỏi | Đạt | Không đạt | Cách kiểm |
| --- | --- | --- | --- |
| Nhìn vào có biết đang ở đâu và việc tiếp theo? | Dialog bỏ chữ: tiêu đề nêu hậu quả, nội dung nói phần nào bị ảnh hưởng | Tiêu đề “Xác nhận” chung chung; lý do nằm trong tooltip hover | Mở ảnh đúng state; đọc theo thứ tự cây của spec |
| State này có đúng một điểm nhấn chính? | Một hành động chính nổi hơn; hành động phụ tonal/text | Hai nút cùng mức nhấn; hành động phá hủy không phân biệt được với lựa chọn an toàn | So với `Hierarchy` của spec. Geometry đạt không trả lời câu này |
| Hành động, nội dung và metadata có tách tầng? | Tên bài > tác giả > ngày/nhãn; trạng thái có chữ và icon | Byline, nhãn và CTA cùng cỡ, cùng màu; trạng thái chỉ bằng màu | Ảnh hai theme; role màu đã resolve |
| Control có trông thao tác được và có tên? | Nút có nhãn tiếng Việt ngắn; vùng chạm theo contract component | Icon trần không tên; link trông như chữ thường; disabled không có lý do nhìn thấy | Source và cây accessibility, không chỉ ảnh |

### 1.2 Bố cục, chữ tiếng Việt và surface

Hướng thị giác là Lavender hiện hành trong [hệ thiết kế](design-system.md); không lấy palette
hay nhịp của sản phẩm khác.

| Câu hỏi | Đạt | Không đạt | Cách kiểm |
| --- | --- | --- | --- |
| Khoảng cách có nhóm theo nghĩa? | Khoảng trong nhóm nhỏ hơn khoảng giữa các nhóm; bước lấy từ scale token | Mỗi chỗ một khoảng tùy ý; mọi vùng thành card/pill ngang mức nhấn | Đối chiếu token; đo anchor khi nghi ngờ |
| Chữ Việt có nguyên dấu và xuống dòng tự nhiên? | Dấu chồng và dấu nặng không bị cắt; tên dài xuống dòng; khổ thơ giữ nguyên | Chiều cao cố định cắt dấu; `break-all`; ellipsis che thông tin thiết yếu | Mẫu có dấu tổ hợp NFC/NFD trên render thật với font đã tải, không phải fallback |
| Màu và surface có theo vai trò? | Nền phân tầng theo tier; lỗi dùng role lỗi kèm chữ | Tertiary dùng làm lỗi; viền trang trí thường trực; hex riêng ở consumer | Role token; tương phản đo trên màu đã resolve |
| Mật độ có hợp tác vụ? | Reader/editor yên; danh sách khám phá hữu hạn | Bảng số liệu lấn vùng đọc; mọi dòng đều có huy hiệu | Bảng Expression trong [SKILL](../SKILL.md) |
| Focus và lỗi có chỗ để hiện? | Vòng focus không bị clip; lỗi cạnh field, có tóm tắt khi form dài | Vòng focus bị `overflow` cắt; lỗi đẩy hành động chính ra ngoài tầm với | Ảnh state focus/error và thao tác bàn phím |

### 1.3 Luồng thật

Các câu dưới đây cần thao tác hoặc test trên đúng host; một PNG không trả lời được. Chi tiết
hợp đồng nằm ở [Interaction UX](interaction-ux.md).

| Câu hỏi | Đạt | Không đạt |
| --- | --- | --- |
| Có phản hồi ngay khi thao tác? | Pressed/focus/pending xuất hiện trước khi có kết quả | Nút không đổi gì cho tới khi request xong; bấm lần hai gửi hai lần |
| Chờ, lỗi và phục hồi có tách nhau? | Loading khác empty khác error; thử lại giữ chữ đã nhập | Lỗi trông như “không có nội dung”; retry xóa bản nháp |
| Back/cancel có một đường rõ? | Escape, bấm ngoài (khi cho phép), nút Hủy và Back hệ thống đóng lớp trên cùng trước | Chỉ đóng được bằng một cử chỉ; Back rời cả trang khi dialog còn mở |
| Chữ, caret và vị trí cuộn có được giữ? | Đổi theme, refresh ngầm hay đóng overlay không làm mất chữ, caret, scroll | Remount input; nhảy về đầu trang; cướp focus |
| Có thao tác thay thế và reduced motion? | Mọi cử chỉ có nút/phím tương đương; vòng lặp dừng ở khung có nghĩa | Chỉ vuốt mới làm được; chờ animation mới commit |
| Ranh giới IME/native có được nêu? | Báo riêng: composition giả lập, IME hệ điều hành, TalkBack/VoiceOver | Suy native PASS từ JVM Desktop hoặc từ ảnh |

### 1.4 Hai mục tiêu: 100% và 200%

- **100% (cỡ chữ mặc định) là mục tiêu bố cục:** nhịp, cân bằng và phân cấp theo spec và mốc.
- **200% là mục tiêu accessibility/reflow, nhưng vẫn phải rõ ràng và có hierarchy:** thứ tự đọc
  giữ nguyên, hành động chính vẫn nhận ra được, không tràn ngang, không chồng chữ, không mất
  thông tin hay hành động. Cuộn dọc dài hơn là chấp nhận được. “Không crash” chưa đủ.
- Khi nội dung ở 200% dài hơn khung (dialog, sheet, panel cuộn riêng), phần mở đầu vẫn phải
  thấy được lúc mở: người dùng đọc câu hỏi hoặc tiêu đề trước rồi mới tới hành động. Focus ban
  đầu không được kéo vùng cuộn qua khỏi tiêu đề; mọi hành động vẫn tới được bằng cuộn.
- Không giảm font scale, cắt chữ, thu nhỏ ảnh hoặc ẩn nội dung để ảnh 200% giống ảnh 100%.
- Báo hai mục tiêu riêng. Nhịp ở 200% kém cân đối hơn 100% chưa là lỗi; mất hành động, mất
  phân cấp hoặc phải cuộn ngang ở 200% là lỗi.
- “Chữ 200%” của preview Web là root font-size, của CMP là multiplier của host; cả hai không
  thay browser zoom, Dynamic Type hay OS font scale. Ghi đúng cơ chế đã dùng.

## 2. Mốc thị giác có provenance

Ba loại nguồn, không thay thế nhau:

| Loại | Dùng để | Không chứng minh | Phải ghi |
| --- | --- | --- | --- |
| **Concept** | Định hướng hình ảnh và ý tưởng | Implementation, tính khả thi từng pixel, tương phản | Nguồn, ngày, ai chọn, phần do công cụ tạo sinh |
| **Style anchor** | Ngôn ngữ thị giác đã duyệt cho một archetype: nhịp, phân cấp, tầng nền | Giống pixel ở màn khác hay scenario khác | Người duyệt và lời duyệt, revision/SHA, renderer, theme, font thật, constraints, phạm vi áp dụng và không áp dụng |
| **Regression baseline** | Bắt thay đổi ngoài ý muốn ở đúng scenario, renderer và môi trường | Chất lượng thẩm mỹ; renderer hay môi trường khác | Scenario ID, môi trường render, hash; lý do và người duyệt mỗi lần thay |

- Mốc chỉ là `approved` khi [sổ mốc thị giác](../../../../docs/design/visual-anchors.md) ghi
  như vậy kèm lời duyệt của chủ sản phẩm. Mọi trường hợp khác là `candidate` hoặc `unknown`.
  Trong manifest capture, `approved` ứng với `references[].authority = confirmed`.
- Agent đề xuất ứng viên và nêu lý do; chủ sản phẩm duyệt. Không tự gọi screenshot hiện tại là
  golden, không chọn “ảnh đẹp nhất” thay người dùng, không dùng ảnh render bằng font fallback
  làm mốc.
- Anchor của màn khác chỉ hướng dẫn nhịp và phân cấp. So theo vùng và nguyên tắc; không so pixel.
- Thay baseline cần lý do, người duyệt và giữ bản cũ trong lịch sử. Hết ngân sách sửa không
  phải lý do để nhận ảnh mới làm baseline.
- Chưa có mốc được duyệt vẫn review được theo LayoutSpec và rubric. Khi đó ghi
  `fidelity: NO_APPROVED_REFERENCE` cùng giới hạn; không dựng approval thay người duyệt.

## 3. Vòng xử lý có ngân sách và điểm dừng

1. **Chốt tiêu chí của lượt:** spec/revision, dòng rubric, ma trận scenario, mốc và trạng thái
   duyệt. Tiêu chí giữ nguyên trong lượt; đổi tiêu chí là mở lượt mới.
2. **Chụp và đo** bằng runner hiện có ([vink-ui-capture](../../vink-ui-capture/SKILL.md)).
3. **Review** ([vink-ui-inspector](../../vink-ui-inspector/SKILL.md)): mở ảnh và metadata trước
   khi đọc lời tự đánh giá của người sửa.
4. **Chọn nhóm lỗi ưu tiên:** mức độ cao trước, gom theo cùng nguyên nhân đã xác minh.
5. **Sửa đúng owner** ([vink-ui-fix](../../vink-ui-fix/SKILL.md) và renderer).
6. **Chạy lại cùng scenario** rồi đối chiếu từng finding: `fixed`, `still`, `new`, `regressed`.

**Ngân sách (chính sách pilot, có thể đổi sau khi có số đo):** tối đa **hai vòng sửa có tái
render** cho một phạm vi. Dừng sớm hơn khi có một trong các dấu hiệu:

- một finding đã `fixed` quay lại, hoặc sửa A làm hỏng B rồi ngược lại (dao động);
- số finding mức Cao trở lên không giảm sau một vòng;
- tiêu chí hoặc mốc bị đổi giữa lượt để kết quả dễ đạt hơn.

Khi dừng: giữ phiên bản tốt nhất **đã kiểm** (commit/SHA và bằng chứng của nó), giữ lịch sử
findings và delta, báo phần còn lại là FAIL hoặc gap. Hết ngân sách không thành PASS, không tự
nâng trần, không tự nhận baseline mới và không reset công việc đã làm. Phân loại điểm dừng:

- `NEEDS_HUMAN_DESIGN_DECISION`: spec và mốc mâu thuẫn, hai phương án đều hợp spec, lựa chọn
  thuần thẩm mỹ, hoặc cần duyệt mốc. Nêu đúng câu hỏi và các phương án đã thử.
- `BLOCKED`: thiếu runtime/thiết bị, lỗi công cụ, hoặc lỗi thuộc owner khác. Nêu prerequisite
  và lệnh tiếp tục.

Sửa nhỏ (padding, câu chữ, một control) dùng một vòng với check nhỏ; mở rộng ma trận theo rủi ro.

**Mức độc lập của review:** cùng một agent tách bước sửa và bước review thì ghi `self-review`.
Chỉ gọi `independent` khi người hoặc agent khác, không dùng chung kết luận của người sửa, thực
hiện review. Reviewer không ghi PASS chỉ vì người sửa báo PASS.

## 4. Bảng kết quả theo chiều

Bàn giao UI tách ít nhất năm chiều. Mỗi ô ghi scenario/target, kết quả theo
[verification guide](../../vink-engineering/references/verification.md)
(`PASS`, `FAIL`, `NOT_RUN`, `UNSUPPORTED`) và giới hạn của review.

| Chiều | Trả lời câu hỏi | Bằng chứng tối thiểu | Không suy ra từ |
| --- | --- | --- | --- |
| Functional | Tác vụ chạy đúng nghĩa sản phẩm | Test hoặc thao tác trên đúng target | Ảnh |
| Geometry | Invariant của spec có giữ | Số đo anchor hoặc assertion của runner | Cảm giác “trông ổn” |
| Visual | Rubric §1.1–1.2 và mốc (nếu có) | Ảnh sạch đã mở, đúng scenario | Geometry đạt; ảnh chưa mở |
| Accessibility/Responsive | 200%, màn hẹp, tên control, focus, tương phản | Ảnh 200%/hẹp, tương phản đã đo, cây accessibility | Ảnh 100%; tên token |
| Interaction | Hợp đồng tương tác | Thao tác hoặc timed frames trên đúng host | Một PNG; Desktop thay native |

`UNSUPPORTED` cần lý do năng lực thật của target; thiếu thiết bị là `NOT_RUN`. Geometry `FAIL`
vẫn phải mở ảnh để chẩn đoán: nó chỉ chặn kết luận đạt, không cấm reviewer xem ca lỗi. Tài liệu
hoặc skill sửa xong không đồng nghĩa giao diện đã được nghiệm thu.

## 5. Mức bắt buộc hiện tại

Review theo tài liệu này đang ở mức **khuyến nghị có gap hiển thị** (pilot #174): thiếu
bằng chứng thì ghi `NOT_RUN`/gap ngay trong bàn giao, không chặn merge tự động và không được ghi
thành PASS. Mở rộng thành gate bắt buộc theo vùng UI là quyết định của chủ sản phẩm sau khi
pilot có số liệu về lỗi bắt được, bỏ sót và chi phí; xem
[báo cáo #174](../../../../docs/design/ui-ux-quality-174.md).
