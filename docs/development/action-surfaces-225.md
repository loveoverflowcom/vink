# Action surfaces Bàn viết / Khám phá — #225 r2

Ngày 16/11/2026; base `6e07806aa87145d0e053f4061f47ccaf3830c14a`.
[LayoutSpec r2](../design/action-surfaces-desk-discovery-225.md),
[issue #225](https://github.com/loveoverflowcom/vink/issues/225),
[baseline chuẩn bị #240](evidence/action-surfaces-225/README.md).

## Phạm vi và trạng thái

Implementation `web`: tạo nháp Filled + Add (kể cả query miss), first-use vẫn một CTA trong
empty panel; Refresh Tonal opt-in; launcher Thẻ Tonal + FilterList. Helper “Khớp tất cả thẻ ·
Tối đa 4” luôn hiện và mô tả form; “Trang này: N tác phẩm” là live status của đúng trang, kể cả 0
và trang cuối. Error/stale/refresh và hành vi Apply/GET vẫn do owner hiện hữu quyết định.

CHOICE-01/02/03 chọn A trong scope kỹ thuật của issue; thiết kế vẫn `DESIGN_PROPOSED`,
`NO_APPROVED_REFERENCE`, không đổi quyết định sản phẩm hoặc authority. Không đổi Editor,
Timestamp, row metrics, Pen Flight, API/DB/cache/auth hoặc thêm model Quint.

Registry `shared/icons.json` thêm FilterList tại pin Material Icons
`10ecc14790612ca8fd164e3f4ffe9c16fc50d5b7`, Apache-2.0; SHA-256 SVG upstream
`da5f9ac466b2a296bdbbcee8eef2eaecf992207e83b8304451701e2e13a2bc0f`.
Generator sinh Web/CMP từ một nguồn; không đổi token/primitive/CSS. CMP không có consumer mới.

## Owner và kiểm chứng

| Claim | Owner | Kiểm tra |
| --- | --- | --- |
| Một create target có nền/Add; Refresh có nền, giữ guard | `desk.rs::CreateDraftButton` / `DraftListView` | `draftDashboard`: matrix, pending keys, uncertain/reconcile, no duplicate POST |
| Launcher có icon lọc đúng nghĩa, tên/target giữ nguyên | `discovery.rs::DiscoveryChrome`, icon registry | `discoveryReading`: glyph trang trí, nhãn/target, Escape/focus |
| AND/max4 luôn có trước/khi chọn và khi catalog lỗi | `FilterForm` | 0/1/4 thẻ, thử thẻ thứ 5, bỏ chọn, catalog 503, no-JS GET |
| Count chỉ nói trang, một live owner | `PublicResults` | SSR, trang rỗng và trang cuối 2 mục; không giấu dưới ngưỡng 12 |
| Apply, URL và history giữ nghĩa | owner GET/discovery hiện hữu | unchanged/changed Apply, q/tag/cursor, Back/Forward/reload, reader return |

## Bằng chứng r2 — Mac arm64, Chrome 157, PostgreSQL riêng

Runtime source: `c4c904675598c4dc643da4ff8cbc3da51add59ea`. Follow-up
`2b77b5a597638930a4b4158b9a53c0c86dadee81` chỉ thêm wait transition và phép đo tương phản
trong hai test `.mjs`; không thay app, bundle hoặc registry icon.

| Kiểm tra local | Kết quả / boundary |
| --- | --- |
| `cargo xtask ui-tokens` + `cargo xtask ui-tokens --check` | PASS; generator kiểm upstream hash và output Web/CMP |
| `cargo xtask verify quick` | PASS toàn bộ trên source runtime đã chốt; clippy, doctest, unit/contract, repo checks |
| `cargo xtask verify web` | PASS đầy đủ trên `c4c9046`; 65 assertion #225 PASS, bao gồm 61 ở main và 4 ở discovery corpus; real HTTP/PostgreSQL/Chrome, có ca transport fixture được gắn nhãn |
| Phase có sẵn `public-search`, script `2b77b5a` | PASS; server/bundle cùng runtime `c4c9046`, database capture riêng được migrate/dev-seed và catalog qua operator CLI; kiểm lại toàn bộ discoveryReading và capture đã đợi màu ổn định |
| `cargo xtask ui-preview leptos verify` | PASS đủ 5 bước; Chrome scenarios, 862 captures, không console error; không nhận mọi ảnh preview đã được mở |
| JS syntax, diff/links và review source | PASS; không thay authority hoặc state machine |

Các lệnh Rust dùng `CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0`.
Web dùng `VINK_CHROME=/Applications/Google Chrome.app/Contents/MacOS/Google Chrome` và cluster
throwaway tại loopback 55447. Phase cuối dùng `node apps/vink-web/e2e/browser-smoke.mjs --phase
public-search` với server loopback 55448 và `--state` trong bundle. Không đọc/chờ CI.

Full Web run: `target/verify-web/1794813144390366000-16352`; log local:
`/tmp/vink-225-ui-checks/{quick,web-r1,public-search-settled,preview}.log`.
Bundle chọn lọc:
`target/ui-evidence/2026-11-16T142616+0700_action-surfaces-ui-225_2b77b5a5/` có manifest,
hash, contexts, navigation graph, source snapshots và báo cáo. Không commit binary/log mới.

Baseline dùng 8 ảnh từ lượt full Web ngay trước triển khai (`2c8066c`, run
`1794811804624697000-97547`), đã xác nhận git tree bằng base `6e07806`; không dùng ảnh Drive cũ.
Đã mở 8 cặp trước/sau ở 320/390/834/1440, hai theme, chữ100/200 và 6 ảnh dialog 0/1/4 thẻ.
Các timestamp mẫu công khai đổi theo lần seed; không dùng pixel equality của timestamp làm tiêu chí.

## Review năm chiều — self-review, một lượt UI

| Chiều | Kết quả | Bằng chứng và giới hạn |
| --- | --- | --- |
| Functional | PASS Web | Một create/POST; uncertain + GET reconciliation; 0/1/4/max4, no-JS, catalog 503, empty/final-page count; URL/history/reader return |
| Geometry | PASS Web | Runner kiểm matrix 320/390/834/1440 × light/dark × chữ100/200: target ≥48 CSS px, full create label, icon trong launcher, không overflow |
| Visual | PASS trong scope | 22 ảnh đã mở: create là điểm nhấn của heading, Refresh tách được khỏi nền, icon lọc có nghĩa, helper/status gọn; `NO_APPROVED_REFERENCE` |
| Accessibility/Responsive | PASS kiểm tự động và ảnh | Tên/semantics, `aria-describedby`, một live owner, focus và reflow; helper/Apply contrast min 5.40:1 sáng, 7.68:1 tối; VoiceOver `NOT_RUN` |
| Interaction | PASS Chrome/CDP | Native Enter/Space/pointer, dismiss/focus, no duplicate submit, draft-vs-applied, history; IME giả lập, OS IME thật và native `NOT_RUN` |

Ảnh dialog 320/chữ200 là viewport `partial`: header/helper và lựa chọn đầu, không giả nhận thấy
toàn bộ vùng cuộn. Các nút phía dưới được đi tới trong test tương tác. Hai heading “Bộ lọc thẻ”
trong dialog là cấu trúc sẵn có của Dialog + FilterForm, giữ ngoài scope redesign.

Lượt đầu phát hiện ảnh dialog chụp giữa transition đổi theme; sửa harness đợi finite animations
và đo contrast, rồi mở lại 6 ảnh đã ổn định. Không sửa runtime hoặc đổi tiêu chí để hợp ảnh;
không ghi những frame chuyển màu đầu tiên là bằng chứng visual cuối. Không còn finding mới
trong phần delta. CMP chỉ được kiểm generator/source; không chạy renderer/native.

AC-01..06 và AC-08..09 được kiểm qua source/browser như bảng trên; AC-07 có paired evidence
và self-review, không phải owner signoff. Không tự tick/đóng issue hoặc nâng trạng thái thiết kế.
