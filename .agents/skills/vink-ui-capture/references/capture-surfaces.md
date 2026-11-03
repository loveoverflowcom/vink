# Ma trận hiện trạng Vink

Các đường dẫn dưới đây đã đối chiếu với `develop` tại `204d319` ngày 03/11/2026.
Đây là điểm khám phá, không đóng băng capability của các checkout về sau. Nguồn
lệnh là [development](../../../../docs/development/README.md); kiểm source/manifest
trước khi dùng, không đoán command cho host chưa có.

## Inventory theo owner

| Surface | Nguồn phải đọc | Bằng chứng cần giữ |
| --- | --- | --- |
| App Web | [routes](../../../../apps/vink-web/src/lib.rs), [shell](../../../../apps/vink-web/src/shell.rs), [navigation](../../../../apps/vink-web/src/navigation.rs), [feature map](../../../../docs/development/feature-map.md) | Route thực, entry/click-path, selected destination, back/return context, guard, overlay và responsive |
| Component/pattern | [UI system](../../../../packages/ui-system/README.md), [patterns](../../../../packages/vink-patterns/README.md), [components](../../../../packages/ui-system/shared/components.md), [states](../../../../packages/ui-system/shared/states.md), [navigation contract](../../../../packages/ui-system/shared/navigation.md) | API/source owner, trạng thái hỗ trợ, nơi dùng và khác biệt renderer có chủ đích |
| Preview/recipe | [catalog](../../../../packages/ui-system/shared/preview-catalog.json), [Leptos preview](../../../../packages/ui-system/leptos/README.md), [CMP](../../../../packages/ui-system/cmp/README.md) | Catalog version, entry ID, scenario ID, renderer, scope/theme, inspector/event log; mẫu không có service authority |
| Tokens | [tokens.json](../../../../packages/ui-system/shared/tokens.json), [schema](../../../../packages/ui-system/shared/tokens.schema.json), [shared](../../../../packages/ui-system/shared/README.md) | Version/hash, alias/role, generated derivatives, overrides, effective values + đơn vị |

Ở mốc trên, app có `/`, `/tac-pham/:id`, `/ban-viet`, `/ban-viet/:id`, `/toi` và
fallback. ID gợi ý: `explore.feed`, `reader.detail`, `desk.home`, `editor.draft`,
`me.utility`; xác nhận lại trước khi nhận vào registry. Không đặt tên `/toi` là
profile đăng nhập. Không tự thêm login/register/comments chỉ vì design có vẽ.

## Chọn coverage có thể kiểm đếm

Với yêu cầu toàn UI, mặc định:

- App: mọi route thực có default state ở light/dark, desktop 1440×900, tablet
  834×1112 và mobile-responsive 390×844 CSS px. Thêm 320px và text scale 200% cho
  các vùng rủi ro như rail/bar, reader/editor, dialog và chữ Việt dài. Ghi chiều
  cao thực, DPR và browser version; CSS px không phải pixel ảnh hay dp.
- Navigation: rail 220/96 nếu checkout hỗ trợ, bottom bar, đích đang chọn, reader
  return/back, Tôi overlay, resize và unsaved guard. Chụp trước/sau có ID liên kết;
  record state bị guard giữ lại cũng là bằng chứng, không coi click là đã tới đích.
- States: guest/local-fixture/auth thật nếu tồn tại; loading, empty, no-results,
  error, forbidden, offline, conflict/uncertain, dirty confirmation khi tái hiện
  an toàn được. Fault injection bằng test adapter/browser interception phải mang
  nhãn `simulated`; không gọi đó là lỗi backend thật.
- Preview: liệt kê toàn bộ catalog/scenario trên từng renderer; mặc định chụp
  mỗi scenario hỗ trợ ở light/dark với khung chuẩn của preview. Thêm compact,
  nested theme, text 200%, focus/pending/disabled/overlay khi liên quan. Không ép
  toàn bộ tích Descartes viewport × state × theme; ghi biến thể đã chọn và phần
  chưa chạy. Reuse ảnh runner hiện có sau khi xác minh context, không chỉ đếm PNG.
- Tokens: snapshot nguồn và catalog; chọn các role màu/chữ/spacing/shape/density/
  navigation/motion có liên quan để ghi resolved values trong từng renderer và
  theme đã chụp. Role chưa đo là `NOT_MEASURED`; source value không tự thành
  runtime resolved value hay contrast PASS.

Scope hẹp theo user được ưu tiên. Inventory ngoài scope giữ `OUT_OF_SCOPE`, không
phóng đại mẫu nhỏ hoặc ép chụp toàn repo khi chỉ giao một màn.

## Runtime và giới hạn ở mốc nền

Web app có runner [browser-smoke](../../../../apps/vink-web/e2e/browser-smoke.mjs)
dùng Chrome DevTools Protocol; không bắt cài Playwright để dùng skill. Kiểm
`contexts.json`, command và output thực của run trước khi nhập ảnh. Runner có
thể tạo scratch DB, ghi fixture và fault injection: đọc prerequisite, dùng local
đúng quyền, không trỏ vào dữ liệu thật chỉ để lấy ảnh.

Leptos preview có deep link `#<catalog-id>/<scenario-id>`, ảnh và `contexts.json`
qua runner [preview-smoke](../../../../packages/ui-system/leptos/preview/e2e/preview-smoke.mjs).
CMP ở mốc nền có Desktop preview và ảnh test off-screen. Ghi
`host=desktop`, `capture_mode=offscreen` khi đúng; không suy thành tương tác cửa
sổ desktop thật. Android/iOS của thư viện chỉ compile opt-in, chưa có product app.

Nếu checkout mới bổ sung diagnostic hosts, đọc manifest và docs mới rồi ghi
`surface_kind=diagnostic-host`, package/bundle, OS/device và build variant. Chỉ
chụp Android bằng emulator/device thật đang chạy host xác định (ví dụ `adb`
screencap); iOS qua simulator/host hỗ trợ (ví dụ `simctl` screenshot). Không dùng
ảnh Web hoặc CMP Desktop để lấp mục native. Có host nhưng thiếu SDK/emulator là
`BLOCKED`; chưa có host là `NOT_IMPLEMENTED`; nền tảng chủ ý không hỗ trợ là
`UNSUPPORTED` có source lý do.

Font tiếng Việt NFC/NFD, khổ thơ, safe areas, overflow và focus có thể ghi quan sát
trên ảnh/tương tác. OS IME, TalkBack/VoiceOver, predictive back, persist và lifecycle
cần bằng chứng runtime riêng; không nâng kết quả test off-screen thành nghiệm thu.
