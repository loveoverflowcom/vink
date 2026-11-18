# Selector định dạng bản thảo — #262

Base kiểm tra: `develop@e4066df9d7cad602a1822414abb418219c612e18`, ngày 17/11/2026.

> **Cập nhật 18/11/2026 (#256/#257/#262):** `SelectVariant::Inset16` đã được tích hợp vào
> luồng tạo nháp trên Bàn viết (`#create-draft-format`) và tác vụ đổi định dạng có chủ đích
> (`#editor-format-select` + `#format-convert-dialog` có xem trước / `Hủy` / `Áp dụng`); Bàn viết
> mở và soạn được cả `default` lẫn `markdown` (một `EditorView` Live Preview). Luồng đọc và
> công khai Markdown thuộc [#264](../development/document-source-format.md#2-pipeline-đọc-web).
> Xem LayoutSpec hợp nhất tại [editor-live-preview-256-257.md](editor-live-preview-256-257.md).

Ở trạng thái tài khoản có 0 tác phẩm, bộ chọn nằm ngay trước **Viết bản thảo đầu tiên**.
Hai control có thể xuống hàng khi chữ phóng lớn. Tab đi từ bộ chọn đến CTA và lựa chọn được
gửi trong cùng yêu cầu tạo nháp như ở danh sách có tác phẩm.

Nguồn yêu cầu: [issue #262](https://github.com/loveoverflowcom/vink/issues/262),
body/comments đọc trước triển khai. Không có PR mở lúc discovery. #255/#256/#263 còn mở,
không có comment triển khai; state issue chỉ là một dữ kiện, inventory source dưới đây mới
xác định prerequisite còn thiếu. Spec geometry được trình bày trước sửa UI.

## Quyết định và ranh giới triển khai

Product confirmed theo lời chủ sản phẩm được ghi trong issue: bản mới Mặc định có tên riêng
và body thuần; dấu `#` không tự biến thành Markdown. Markdown có title từ H1 đầu tiên,
không title input writable khác; AsciiDoc hoãn. Không thêm autosave hoặc lưu riêng offline.
Đây là delta hướng đích cho WR-01/02/03, không signoff schema/hash hay nâng proposal
`VDR-CONTENT-STORAGE@r1` khỏi `PENDING`.

| Owner | Source tại base | Capability của App / điều kiện tích hợp |
| --- | --- | --- |
| #255 source/format/hash | `services/vink-app/src/domain/document.rs`: `SCHEMA_VERSION = 1`, `DocumentData { schema_version, blocks }`, deny unknown fields; Web `contract.rs`/`document.rs` cùng v1 | Chưa có format discriminator/lossless raw source. Phải có owner contract, compatibility/hash signoff và checks PG trước create/open theo format. |
| #263 title | `domain/revision.rs` và `pages/desk.rs`: title riêng | Chưa có canonical Markdown title extraction/validation và same-save metadata. Sau PR #271 (`e947a42`), quy tắc/corpus/điểm nối nằm ở [owner title](../architecture/editor-title-authority.md); vẫn chưa có parser hay format-aware save. |
| #256 provider/mode | `apps/vink-web/editor/src/editor.ts` và bridge `src/editor.rs`: CM plain text | Chưa có Markdown provider/capability/mode; không dựng parser/provider thứ hai trong #262. |
| #264 Reader | Web `ReadingDocument`, public snapshot canonical | [Luồng đọc và kiểm chứng](../development/document-source-format.md#2-pipeline-đọc-web). |
| #262 presentation | `vink-ui::Select` + gallery `field.select` | Chỉ geometry opt-in và fixture callback, không format authority, persistence, conversion intent hay draft state mới. |

**CHOICE-01 A:** tái dùng native `Select` với chevron decorative, `appearance:none`, logical
padding. Ít lifecycle/focus owner hơn custom listbox; geometry được đo trong browser.
`SelectVariant::Standard` giữ fix #135 (18px icon, 12px inset, 38px text reserve).
`Inset16` thêm 16px icon, 16px inset, 52px reserve cho consumer tương lai.
Đây là renderer Web opt-in, không đổi CMP Select hoặc catalog scenarios dùng chung.

**CHOICE-02 còn OPEN cho integration.** Không có atomic source/format/title server seam tại
base nên không chọn in-place bằng một client label swap. New-draft conversion chỉ được bật
khi có idempotency/reconcile cho lost ACK và preview đầy đủ. Không converter trong lát này;
không đổi bản có chữ hoặc quảng cáo Markdown ready.

## LayoutSpec — geometry r1

Reference: concept R1 trong issue là `candidate / NO_APPROVED_REFERENCE`, không screenshot App.
Bản rút gọn chỉ triển khai control; layout editor tương lai giữ theo issue, chưa render ở App.

```text
Region #field-select-sample — gallery field.select, dữ liệu [Mẫu], không gọi service
  Field #genre-select — native Select Standard hiện hữu
  Field #format-select — native Select Inset16
    Label — Định dạng bản thảo
    Control — Mặc định / Markdown; chevron decorative click-through
    Help — [Mẫu] Chỉ thử control; không tạo, lưu hay chuyển đổi bản thảo.
```

Control dùng label/focus/disabled/error/theme roles hiện hữu. Stack xuống dòng theo nội dung,
không overlay tự dựng, cuộn document; giữ target tối thiểu 48px. Chữ 100% là layout goal,
200% là root font-size multiplier để kiểm reflow, không browser zoom. Fixed CSS-pixel inset
tránh tiêu tốn measure theo rem ở 200%; `padding-inline-end`/`inset-inline` hỗ trợ RTL.
Invariant đo từ border box select: icon 16×16px, cách inline-end 16px, computed trailing
padding 52px, content-to-icon gap 20px; dung sai ±1 CSS px của issue (oracle dùng ±0.5px).
Không clip option enabled, không horizontal document overflow.

Interaction đích: native keyboard/type-ahead, phím native mở menu, Escape giữ value/focus; pointer và
emulated touch trên tâm chevron đi tới chính select, không button lồng. Gallery callback chỉ
đổi fixture value và ghi event log. Invalid có error linked; disabled không nhận focus.
Native popup ngoài DOM; headless CDP chỉ kiểm hit target/focus/Tab/type-ahead/value.
Lặp open/close đã gây timeout `Runtime.evaluate`; không đưa probe popup vào smoke tự động.
`:open` transient trên Chrome/macOS trong probe không chứng minh popup OS mở bền vững.
Menu OS, VoiceOver và OS IME phải có evidence riêng.

## Đường tiếp tục sau prerequisite

1. Re-read develop và owners #255/#263/#256/#264. Consume đúng format/capability đã lưu,
   không infer từ v1 absence, filename hoặc dấu `#`; không dùng enum UI này làm wire format.
2. Tích hợp `Inset16` vào header `Back | format | save`; Mặc định có title+plain body,
   Markdown bỏ title input và chỉ hiện mode/outline đúng capability. Giữ một EditorView.
3. Chọn CHOICE-02 bằng server seam thật; preview source/title trước Apply. Cancel không edit,
   save, đổi dirty/history. Default→Markdown phải escape/transform `#` rõ trong diff;
   Markdown→Default phải công bố giữ source hay flatten, không bỏ heading/link/code/whitespace.
4. Chặn composition, pending, unresolved conflict; fence target/version/activation và ACK.
   Kiểm lost ACK/retry không nhân bản/apply hai lần. Nếu chưa fidelity, conversion có chữ vẫn unavailable.
5. Chạy scoped pure/PG và Web main + editor scenarios trên revision tích hợp; đo matrix cùng
   corpus/host theo issue. Chỉ sau đó nghiệm thu AC-01..06. Không kế thừa PASS từ gallery.

Lát presentation không thêm state transition nên không thêm Quint model song song.
Model/save/CAS hiện có không đổi. Kiểm PG và Web App cho conversion vẫn `NOT_RUN`;
[bàn giao evidence](../development/evidence/editor-format-picker-262/README.md) ghi check thực chạy.
