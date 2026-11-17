# Audit Empty State Web/CMP — 17/11/2026 (#258/#259/#261)

Base `develop@6287fbfa2817fd457f3b93e710463a7a9cc7e84f`, branch `codex/empty-state-audit`.
Máy macOS 16 GiB, Chrome qua CDP của `apps/vink-web/e2e/browser-smoke.mjs`, PostgreSQL 17
scratch, `vink-app serve` profile `local` với principal fixture (không phải đăng nhập thật).
Đây là audit có giới hạn: chỉ đổi những lỗi có bằng chứng; artwork vẫn **NO_APPROVED_REFERENCE**.

## Kết luận theo phát hiện

| Phát hiện | Phân loại | Nguồn bằng chứng | Kết quả |
| --- | --- | --- | --- |
| Cụm Empty bám ngay dưới controls, phần trống dồn xuống dưới (Desk 1440×900 lệch −42/−48px, Explore/thẻ lệch −18…−75px ở 32/32 ô) | CONFIRMED · P2 | Browser, đo rect thật ([before](before/)) | Sửa: chuỗi flex cột chỉ khi có `.vk-empty-state`, `margin-block: auto` |
| Desk NoResults bị gỡ rồi gắn lại khi refresh cùng query (vi phạm UIS-03 `Empty + Loading`, LayoutSpec #259) | CONFIRMED · P2 (layout jump) | Browser, `MutationObserver` ghi `removedNodes` | Sửa: bỏ điều kiện `!loading`, dùng chung `empty_head` |
| CMP diagnostic: ảnh căn giữa nhưng câu/CTA lệch trái, câu dùng tông mặc định thay vì muted như Web | CONFIRMED · P2 (parity) | Capture Desktop cũ `empty-webp-cmp-261/review-sheets` | Sửa trong `:diagnostics`, không mở rộng API `:ui` |
| Xóa tìm kiếm từ NoResults có thể thoáng hiện first-use với trang lọc cũ | NOT_REPRODUCED | Browser: observer ghi cả node thêm/gỡ trong một task, GET bị giữ | Giữ làm regression guard |
| Class `.vk-desk-empty` chỉ còn để ẩn icon CTA | Dọn dẹp | Source | Chuyển selector sang `.vk-empty-state__actions` |
| Loading/lỗi/401/403/cursor hỏng/đổi phiên hiển thị như Empty | ALREADY_FIXED | Phase `desk-empty`, `public-search` hiện có | PASS, không đổi |
| Asset: 3 WebP canonical, không bản sao/orphan | ALREADY_FIXED | `verify.mjs`, `git ls-files` | PASS |

## Lệnh và kết quả

| Lệnh | Kết quả |
| --- | --- |
| `node apps/vink-web/assets/empty/verify.mjs` · `node --test apps/vink-web/assets/empty/verify.test.mjs` | PASS: no-results 20.070 B, no-public-works 16.444 B, first-draft 19.190 B; 512×427, VP8 + alpha; 3/3 test |
| `./tools/local-budget run -- cargo xtask build web` (before và after) | PASS |
| `browser-smoke.mjs --phase desk-empty`, code develop + CSS develop, chế độ soft cho check `audit` | 2 FAIL refresh-remount, 4 FAIL lệch tâm ở 1440 ([log](before/desk-empty-soft.log)) |
| `browser-smoke.mjs --phase public-search`, CSS develop, soft | 32/32 FAIL lệch tâm ([log](before/public-search-soft.log)) |
| `browser-smoke.mjs --phase desk-empty`, after, strict | PASS, exit 0; \|Δ\| ≤ 0,2px ở 320/390/834/1440 × sáng/tối × 100/200% ([log](after/desk-empty.log), [geometry](after/desk-empty-geometry.json)) |
| `browser-smoke.mjs --phase public-search`, after, strict | PASS, exit 0; Δ = 0px ở 32 ô Explore + thẻ ([log](after/public-search.log)) |
| `cargo xtask ui-preview cmp verify` | PASS: 105 test Desktop, 0 lỗi, có assertion tâm mới ([tóm tắt](after/cmp-verify-summary.log)) |
| `./tools/local-budget run -- cargo xtask verify quick` | Lần 1 FAIL repository check (`;` trong comment CSS mới bị đọc như khai báo); sau khi sửa comment: PASS toàn lane, exit 0, wall 166 s. Hai lần browser after chạy trước bản sửa comment, khác biệt chỉ nằm trong comment |

Phép đo: cụm = đỉnh ảnh → đáy actions; vùng = sau margin của control nhìn thấy cuối cùng phía trên
slot → đáy content box `.vk-main` (Desk trừ thêm thông điệp riêng tư, Explore trừ margin của
`#explore`). Chiều cao viewport của runner là 900px. Khi nội dung cao hơn viewport (320/390 ở 200%)
không còn khoảng trống thừa, cụm về căn trên và cuộn theo trang.

## Ảnh đại diện (renderer thật)

- Desk 1440 sáng: [before](before/desk-art-first-draft-1440-light-100.png) · [after](after/desk-art-first-draft-1440-light-100.png)
- Explore 1440 tối: [before](before/empty258-explore-dark-1440-100.png) · [after](after/empty258-explore-dark-1440-100.png)
- CMP Desktop Desk/no-results 320 sáng: [after](after/cmp-screen.desk--no-results--320--light--100.png)

## Ma trận trạng thái

| Trạng thái | Web | CMP diagnostic | Ranh giới |
| --- | --- | --- | --- |
| Loading | PASS: không artwork, không CTA tạo | PASS: skeleton, không artwork | Browser / Desktop test |
| Empty (first-use) | PASS: chỉ head không lọc, không continuation, không lỗi | PASS | Browser / Desktop |
| NoResults | PASS: xóa query là hành động thật | PASS | Browser / Desktop |
| Ready | PASS: không mount artwork | không áp dụng | Browser |
| Error / 401 / 403 | PASS: nhánh lỗi riêng, không artwork | PASS | Browser / Desktop |
| Retained + refresh | PASS: giữ Empty, `aria-busy`, nhãn bản đã tải | PASS: scenario `refresh`/`stale` không artwork | Browser / Desktop |
| Pending / uncertain create | PASS: không POST lại, phiên cũ không điều hướng | PASS: CTA disabled | Browser (transport fixture) / Desktop |

## Chưa chạy / ngoài phạm vi

- Android API24–27 và API28+, iOS Simulator, TalkBack, VoiceOver, fontScale/Dynamic Type, IME OS
  thật, thiết bị thật: **NOT_RUN**. Capture Desktop chỉ là DESKTOP_PASS, không phải NATIVE_PASS.
  Issue #261 giữ OPEN.
- Forced colors và reduced motion: không đổi trong audit này, không chạy lại riêng.
- Nhãn CTA CMP khác Web ở hai ô vẫn là khác biệt có chủ ý đã ghi trong
  [LayoutSpec #261](../../../design/empty-artwork-cmp-261.md).
