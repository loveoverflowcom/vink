# Tôi / Tùy chọn — Surface R5 (#67)

Phần R4/R5 của [#67](https://github.com/loveoverflowcom/vink/issues/67), trên
`develop@b4134a7759dc693cf9781c5dfced5b815da5e838` sau PR #104.
Giữ các nghiệm thu R2; hình thức mới theo thiết kế được dẫn trong issue.

## Kết quả và lựa chọn

`Tôi → Tài khoản → Chế độ màu / trợ giúp → ba radio`.
Panel giữ `Đóng → Tài khoản → Chế độ màu / trợ giúp → radio → Mở trang Tôi`.
Tài khoản ghi “Chưa mở đăng nhập.”, không tạo login hoặc account giả.

Account/idle choices dùng surface-container, panel dùng surface-container-low,
selected dùng primary/on-primary kèm check. Bỏ shell lồng quanh choices, shadow
panel và rule footer. Không sửa tokens, shared renderer hoặc navigation #88.
Account có một inset ≤16px; chữ dùng trọn content box khi xuống dưới avatar.
Giữ container38em: chỉ ngang khi đủ chỗ, dọc ở panel/màn hẹp/chữ200, đủ nhãn.

CHOICE-06 B: dùng `SupportingDetails` native từ #90. Icon cạnh heading khi đóng;
mở help chiếm hàng đầy đủ bên dưới heading. Enter/Space giữ focus ở summary,
không có Escape riêng hoặc overlay lồng. Escape trong panel đóng modal cha và trả
focus theo shell. Radio fieldset/legend, bootstrap, `vink.ui.theme`, OS/storage
vẫn do owner hiện hữu giữ.

CHOICE-07 A: một disclosure “Bản thử nghiệm” ở footer. Nhãn Mẫu và thông báo định
danh thử tại thao tác thảo luận còn giữ nghĩa. Reader Tab qua disclosure trước
bottom navigation. Summary mới có scroll-margin theo chiều cao bar đã đo để
native focus không nằm dưới bar ở320/chữ200.

Focus selected dùng on-primary, hoặc HighlightText trong forced-colors, giữ một
outline flush2px. Đo tương phản vòng focus: sáng6,98:1, tối8,16:1,
forced-colors19,04:1. Tương phản chữ thấp nhất trong cặp đã đo: sáng5,79:1,
tối8,16:1. Không suy contrast từ ảnh concept.

## Kiểm chứng

| Kiểm tra local | Kết quả |
| --- | --- |
| `cargo xtask verify quick` | [PASS](../development/evidence/me-r5/quick-summary.txt) |
| Browser smoke `appearance` | [PASS](../development/evidence/me-r5/appearance-summary.txt) |
| Browser smoke `public-search` | [PASS](../development/evidence/me-r5/reader-summary.txt) |
| `cargo xtask verify web` | [PASS](../development/evidence/me-r5/web-summary.txt) |

Ma trận Web: 320/390/834/1024/1440, light/dark/system, root text100/200;
native Arrow/Space/Tab, first paint, OS/storage denied, focus/forced-colors;
panel cao360/900 một scroller và CTA cuối tới được; help keyboard/CDP touch/no-JS;
Escape/outside/Back/Forward/restore, SSR/privacy và dirty editor/save/conflict/
preview/export. Các lần FAIL được sửa trước kết quả cuối: fmt, Tab oracle còn
thiếu disclosure, rAF dùng nhầm cho no-JS, focus trùng fill, summary bị bar che.

[Manifest công khai](../development/evidence/me-r5/manifest.json) giữ hash source,
runner/PNG, viewport/theme/root-text/scroll và kết quả. Không chứa đường dẫn máy,
raw DOM hoặc link tệp riêng tư. Captures dùng fixture tổng hợp của repository,
account unavailable. Before/after `/toi` cùng fixture; panel chỉ dùng ảnh sau sửa,
không suy mức cải thiện từ nền dashboard baseline đã cache danh sách rỗng.

| Ca đã mở kiểm | Before | After |
| --- | --- | --- |
| Tôi390 sáng/chữ100 | [PNG](../development/evidence/me-r5/before-390-me-light-text100.png) | [PNG](../development/evidence/me-r5/after-390-me-light-text100.png) |
| Tôi390 tối/chữ100 | [PNG](../development/evidence/me-r5/before-390-me-dark-text100.png) | [PNG](../development/evidence/me-r5/after-390-me-dark-text100.png) |
| Tôi320 tối/chữ200 | [PNG](../development/evidence/me-r5/before-320-me-dark-text200.png) | [PNG](../development/evidence/me-r5/after-320-me-dark-text200.png) |
| Choices320 tối/chữ200 | [PNG](../development/evidence/me-r5/before-320-me-dark-text200-appearance.png) | [PNG](../development/evidence/me-r5/after-320-me-dark-text200-appearance.png) |

Panel1440: [PNG](../development/evidence/me-r5/after-panel-1440-dark.png).
Panel834×360/chữ200, help mở và CTA cuối:
[PNG](../development/evidence/me-r5/after-panel-834x360-text200-end.png).
Help: [PNG](../development/evidence/me-r5/after-panel-help-dark.png).
Chạm mô phỏng320/chữ200: [PNG](../development/evidence/me-r5/after-touch-help-320-text200.png).
Focus selected: [PNG](../development/evidence/me-r5/after-selected-focus-dark.png).

Root text/CDP không chứng minh zoom thật, OS IME, mobile hardware/soft keyboard,
screen reader, Safari/Firefox hoặc CMP/native; các lớp đó NOT_RUN. Không sửa API
CMP, không suy native từ Web. Giữ lịch sử/ticks R2 và scope chưa nghiệm thu mở.
Không kiểm/chờ GitHub CI và không deploy.
