# Bàn viết R6 — bằng chứng local #133

Base `e2294a2bc2ee68804f45cf3f51a5c3e494b651ce` (develop sau PR #136), branch
`codex/desk-r6-draft-hierarchy`, implementation `4183a45`.
[LayoutSpec và owners](../design/desk-r6.md). Chạy 09/11/2026 UTC
(10/11/2026 trên Mac): Rust 1.99.1, Node 22.15.0, Chrome 157.0.8103.40,
PostgreSQL 17.11 riêng ở loopback. LocalFixture chỉ cho phép ghi trong scratch DB.
GET fixture ba nháp là dữ liệu mẫu; các ca tạo/lưu/restart dùng server và DB thật.

## Đối chiếu cùng fixture

Ba nháp, tên đầu NFD “Những ngày mưa ở khuỷu đường cũ, nơi người trở về vẫn nhớ
tiếng Việt và những khổ thơ còn đang viết dở”, version 3, `2026-11-09T08:42:00Z`;
nháp sau là “Hiên nhà còn nắng” và “Bên kia ô cửa, nơi tiếng gió giữ lại những khổ
thơ chưa kịp đặt tên”. Cùng route `/ban-viet`, timestamps, viewport, theme và CSS
text scale. Ảnh trước/sau đã mở ở 390 sáng/100, 320 tối/200 và wide; thêm ảnh list,
A–Z ở 320/200, forced-colors focus và 1280/1440/chữ200 của app cuối.

| 390×900 sáng/chữ100 | Trước | Sau |
| --- | ---: | ---: |
| Khối resume | 294.17px | 230.17px |
| Mép trên input search | 466.77px | 394.97px |

Khối giảm 64px; search lên 71.8px. Tương phản chữ resolve tối thiểu 5.79:1 sáng,
8.16:1 tối (control disabled được miễn); không suy ra chứng nhận accessibility.

## Kết quả local

Dùng `CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_PROFILE_TEST_DEBUG=0`, tắt incremental để
hạn chế dung lượng; Chrome cấu hình rõ qua `VINK_CHROME`. `verify web` nhận
`VINK_TEST_DATABASE_URL` của cluster scratch riêng, tạo/migrate/seed rồi xóa DB test.

| Lệnh | Kết quả và ranh giới |
| --- | --- |
| `cargo xtask verify quick` | PASS — fmt, clippy SSR/hydrate/preview, doctest, unit/contract, tokens và repository checks |
| `cargo xtask verify web` | PASS — SSR/privacy, hydration, query/IME mô phỏng, sort, 100-item cap, focus/reflow, quyền/503/last-success, tạo mất ACK và GET reconciliation, lưu/reload/conflict, restart, discovery và endorsement regression |
| `cargo xtask ui-preview leptos verify` | PASS — cùng DraftResume/DraftRow trong specimen; 784 capture contexts, reflow 100/200%, theme và component checks |
| `node --check apps/vink-web/e2e/browser-smoke.mjs`, diff whitespace, tự review | PASS — không còn finding có thể hành động trong phạm vi |

Lượt trước đã FAIL ở typography literal (đổi sang tokens có cùng giá trị), Chrome
chưa cấu hình, selector contrast cũ, biến CDP redeclare và từ “thảo” bị bẻ khi thêm
icon vào CTA rỗng. Đã sửa và chạy lại đạt kết quả trên. Lượt `desk-layout` thủ công
thiếu catalog thẻ dừng ở phần public search; lane web cuối tự seed catalog và PASS.
CTA rỗng giữ toàn bộ measure của chữ; dấu cộng chỉ hiện ở action phụ trên heading.

| Acceptance | Bằng chứng |
| --- | --- |
| AC-01 | `draftR6Hierarchy`: resume ≤250, search lên ≥48; disclosure giữ focus/tên NFD; href đúng |
| AC-02 | `draftDashboard` + R6: full-title/single-link, metadata, equal inset, trục ngoài; reflow 320/chữ200 và A–Z |
| AC-03 | Matrix danh sách/quyền/loading/empty/503 và create uncertain; SSR chỉ shell, không nháp riêng |
| AC-04 | Ảnh app trước/sau đã mở; Enter/Tab qua CDP, contrast resolve, CTA cuối trên bottom nav |
| AC-05 | Một scope/status dưới search, số/cap thật, debounce/composition cũ; tools cùng hàng ở 390 và wrap khi hẹp |
| AC-06 | 1280/1440 × 100/200, primary resume, title đầy đủ; mở work ID thật và đọc version được ACK; không prefetch manuscript |

Evidence giữ tại thư mục local
`/Users/manhblue/Documents/Codex/2026-11-10/task/evidence/vink-133`:
`before/`, `after/` (cùng fixture), `web/` (R6, A–Z, focus, contrast), `preview/`
(Desk specimen + toàn bộ manifest), logs và `manifest.json` với hash source/ảnh.
Chỉ các ảnh nêu trên được mở; không gọi toàn bộ 784 capture contexts là đã review
thị giác. Output build và các capture gallery ngoài scope được dọn sau khi giữ
bằng chứng liên quan. Không kiểm/chờ GitHub CI; không deploy.

## Giới hạn

Text200 là tăng root font CSS; chưa kiểm zoom trình duyệt thật, Safari/Firefox,
mobile/IME hệ điều hành, screen reader có tên hoặc native runtime. Input/AX tree
qua CDP không chứng minh những lớp đó. `verify postgres`, Quint, Kani và CMP
NOT_RUN vì không đổi store/domain/contract hoặc native. Dropdown/chevron shared
vẫn thuộc #135.
