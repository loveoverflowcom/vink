# UI-REPAIR-12 và kiểm lại UI-REPAIR-13 — preview trung thực

Ngày 03/11/2026. Đã fetch và fast-forward từ checkout cũ tới
`develop@f8eaaddef18a13bfa4e324e970886eec7e7a36a3`; không triển khai trực tiếp từ
snapshot phát hiện `31a62b1`. Nhánh `codex/ui-preview-theme-selection`, phần code
và coverage tại `c6ce94d1cbf9a4bee0e32acef022404185de8ec4`.
Các lane local được khởi chạy trên patch của base rồi commit nguyên các input
code/test thành `ef2d721b289f8240b32d5be9b6b6ed05269b0b70`. Đưa lên qua kết nối
GitHub tạo commit khác metadata nhưng cây mã giống hệt: cả hai có tree
`16cabf2de66f758cf6e7b6b7268b01fd681fe205`. Phần tài liệu tiếp theo không đổi input
runtime. CI của PR kiểm đúng head cuối và là gate trước merge.

## Finding và ranh giới

- **UI-REPAIR-12: CONFIRMED.** Web `screen.me` dùng signal search rỗng làm value
  của RadioGroup, nên không có radio được chọn; CMP dùng `theme = "system"`.
  Replay Chrome trên bản chưa sửa FAIL đúng assertion “exactly one System radio
  selected initially”; đã mở ảnh baseline 1440px. Primitive so value với từng
  option, nên đây là khác biệt cả selection lẫn native semantics, không chỉ tên biến.
- **UI-REPAIR-13: finding lịch sử CONFIRMED, lỗi trên base hiện tại REJECTED.**
  [PR #54](https://github.com/loveoverflowcom/vink/pull/54) đã nối callback local
  của reader với callback composition tại `2edb2ea`, sửa replay tại `6fd48e6`,
  merge thành `f8eaadd`. Không sửa lại runtime góp ý đã được merge; chạy lại
  `screenPatterns` và `ScreenPreviewTest` trên nền mới, thêm ma trận capture hẹp.

HOW: RadioGroup Web dùng checked property/native radio và caller giữ value;
CMP dùng selectable group, Role.RadioButton và một value tại recipe. Web nay có
signal theme riêng, khởi tạo `system`; cả hai giữ selection trong instance và
khởi tạo lại khi specimen remount. Đổi theme của host/recomposition không remount.
Tìm kiếm vẫn có signal riêng. Không gọi Appearance của product, storage hay API.

WHY: UI-REPAIR-04 chỉ sửa Tôi/overlay trong product và đã ghi rõ không sửa recipe
preview; xem [nguồn quyết định và giới hạn](ui-repair-04.md). Product có authority
Appearance và device preference riêng; sample chỉ minh họa lựa chọn, không mang
authority đó sang gallery. Giữ primitives/tokens hiện hành, không thêm theme engine.

Đã đọc [#41](https://github.com/loveoverflowcom/vink/issues/41),
[#43](https://github.com/loveoverflowcom/vink/issues/43),
[#45](https://github.com/loveoverflowcom/vink/issues/45) và
[#25](https://github.com/loveoverflowcom/vink/issues/25), rồi đối chiếu source:
base đã chứa #49/#50, gồm Rust discussion core, models, application, PostgreSQL
và HTTP API. Chưa có auth authority hay UI bình luận production; không lặp lại
mô tả cũ rằng toàn bộ backend chưa tồn tại. Discourse không là runtime/store.
#25 giữ assets/renderer; #45 giữ UI integration với API thật.

Finding riêng cho #45 đã có trong #54: cả hai FeedbackComposer còn dùng Sharing
để ẩn form. Feedback intent và per-work comments mode là hai trục độc lập. PR này
không đổi pattern/toggle và không thêm assertion khẳng định Sharing khóa bình luận.

## LayoutSpec rút gọn — đã trình bày trước patch

```text
screen.me (default / auth-required / long)
  nhãn Mẫu, không kết nối dịch vụ
  H1 Tôi → avatar trung tính → trạng thái tài khoản và giới hạn đăng nhập
  Giao diện — RadioGroup / selectable group hiện có
    Sáng / Tối / Hệ thống
  sample-result hiện có

screen.reader/feedback, composition.feedback (kế thừa #54)
  nhãn Mẫu → nội dung/nhận xét cố định
  notice của error/uncertain nếu có
  composer → input đang soạn → action và lý do pending/disabled
  sample-result: intent mẫu, chưa gửi hoặc lưu
```

Không đổi cây vùng, thứ tự, copy hay token. Trước → sau của Tôi: không chọn mục
nào → Hệ thống được chọn; đổi value chỉ đổi selection trong sample. Cả hai theme
giữ cùng cây. Ma trận hẹp 320/390 CSS px hoặc dp, chữ 100/200%; nhóm radio và
composer reflow theo primitives, không cố định chiều cao hay giấu nhãn.

## Kiểm chứng và coverage

Runtime local: macOS ARM64; Rust 1.99.1, Node 22.15.0, Chrome
156.0.8078.80 headless, DPR 1; Microsoft JDK 21.0.7, Gradle 8.14.3, CMP JVM
off-screen. Rust dùng profile dev/test bỏ debug symbols và tắt incremental để
giảm cache; không sửa pin/dependency hoặc gates của repo.

| Check | Kết quả |
| --- | --- |
| Baseline `cargo xtask ui-preview leptos verify` + replay mới | FAIL đúng mặc định System; build/prerequisites PASS; phần sau NOT_RUN |
| `cargo xtask verify quick` | PASS: token drift, fmt, clippy SSR/hydrate/preview, doctest, unit/contract, repository checks |
| `cargo xtask ui-preview cmp verify` | PASS: 74 test JVM, không failure/error/skip |
| `cargo xtask ui-preview leptos verify` | PASS: 707 raw capture records và 172 specimen tiles cho 64 cấu hình hẹp; không lỗi console/exception; không tràn ngang 320–1440px, chữ 100/200%, mọi scenario |

Web `meSelection`: đủ ba scenario, mặc định một System; click đủ ba value;
ArrowDown vòng qua cả ba giữ focus đúng radio; OS theme/resize giữ node, selection
và focus; rời/quay lại reset System, độc lập search; host theme và local/session
storage không đổi. CMP replay kiểm ba scenario, role/selected duy nhất,
Enter/Space giữ focus, recomposition giữ lựa chọn, remount reset; preference host
và event log không bị sample thay đổi.

Web/CMP replay góp ý chạy trực tiếp recipe: một intent mỗi submit khả dụng,
thông báo “Mẫu · Intent gửi nhận xét, chưa gửi hoặc lưu.”; giữ text/focus/selection
qua status update và theme; Web thêm resize/Enter/Space và quan sát không request
mạng. Pending/auth-required/uncertain không phát intent; error giữ cảnh báo và
retry chỉ báo intent mẫu. Không mutation, count mới hoặc saved acknowledgment.

Capture matrix tập trung mỗi renderer có **64 cấu hình**: Tôi
default/auth-required/long, reader feedback, composition default/pending/error/
uncertain × 320/390 × light/dark × chữ 100/200%. Web giữ raw gallery và specimen
tiles đầy đủ từ geometry thật, không thu nhỏ hay cắt mất cuối màn; CMP chụp consumer
độc lập, density 1, padding 16dp, OS insets 0. Metadata nằm trong `contexts.json`
(Web), `screen-contexts.json` (CMP). Hình không tự chứng minh semantics hoặc focus.

Đã mở baseline và **40 cấu hình sau sửa** qua 18 contact sheets giữ tỉ lệ 1:1,
20 cấu hình mỗi renderer: cả tám cặp entry/scenario ở light/320/text200 và
dark/390/text200; thêm Tôi/default và composition/default ở dark/320/text100,
light/390/text100. Web nối đủ các tile liên tiếp; CMP chỉ bỏ khoảng nền trống cuối
ảnh. Nhãn, radio, nút và dấu tiếng Việt không clip trong mẫu đã mở; pending/error/
uncertain giữ thông báo đúng. Đây là review mẫu 20/64 cấu hình mỗi renderer,
không tuyên bố đã mở mọi ảnh. CMP tạo tổng cộng 511 PNG; Web giữ raw và tiles riêng.

Tự review hai hướng caller → primitive trên Leptos/CMP và diff cuối: không có
finding mới cần sửa trong phạm vi; lỗi Sharing đã ghi riêng cho #45 ở trên.
Chỉ recipe Web đổi runtime; không đổi patterns, catalog, product Appearance,
API, auth, storage hay theme engine. [Feature map](../development/feature-map.md)
ghi coverage và cách tái hiện, không suy kết quả chạy từ sự tồn tại của test.

Không commit PNG, log hay cache build. Theo yêu cầu người dùng, dọn artifact tạm
sau khi kiểm và ghi kết quả; CI giữ artifact theo retention của workflow hiện có.

Android/iOS runtime, OS IME, screen reader và cửa sổ Desktop thật **NOT_RUN**;
không cần native host cho thay đổi signal Web này. Không lấy off-screen/JVM hoặc
compile làm native acceptance; không nghiệm thu account, preference production,
comment gửi/lưu hay tích hợp UI #45 bằng preview.
