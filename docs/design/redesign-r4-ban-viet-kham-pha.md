# R4 · Bàn viết + Khám phá — audit, redesign và bàn giao

**23/11/2026 · R4.2 · `DESIGN_PROPOSED` · `fidelity: NO_APPROVED_REFERENCE`.** Đối chiếu
`develop@7ca0dd0d3ecb4c5d5ae92e5a415f91d16d345298`. Phạm vi: `/ban-viet`, `/`, Navigation Rail,
thanh dưới và component dùng chung. Không sửa code UI, không tạo issue, không merge/deploy trong
lượt này. Mọi tên bài, bút danh, @handle, ngày và số Thấm/Nhận xét trong ảnh và canvas là
**dữ liệu mẫu (fixture)**.

- Canvas Claude Design (riêng tư, cần chủ sở hữu chia sẻ):
  <https://claude.ai/artifact/LgiXNfVcuY42xzAxsGcmQr> — 22 artboard: tổng quan, hiện trạng, A/B/C,
  bộ R4.1 đề xuất (Khám phá/Bàn viết sáng-tối, desktop-mobile, lọc, trạng thái, rail, 200%,
  component sheet).
- Canvas R3 cũ `U2EGFmygrgoT4hq5dHH1SP`: **không đọc được** từ tài khoản chạy lượt này (riêng tư);
  đã dùng nguồn từng khung trong [lavender-r3-336/canvas](lavender-r3-336/canvas/).
- Bằng chứng runtime (local, không commit): `target/ui-evidence/redesign-audit-20261123/` của
  worktree `vink-redesign-audit` — `manifest.json` + 126 PNG.

## 1. Nguồn và quyết định đã đối chiếu

Đọc AGENTS.md, skill index, `docs/design/{README, search-r5, desk-r6, desk-server-pagination,
discovery-chip-260, social-289, visual-anchors}.md`, Lavender R3, `ux-quality.md`, `tokens.json`
0.7.0, `vink-patterns`, `vink.css`, `pages/{desk,discovery}.rs`, `shell.rs`, `ui/adapters.rs`.
Issue và PR: #102, #191, #192, #201, #213, #214, #225, #258–#260, #308, #336, #351–#355, PR #394.

Quyết định giữ nguyên (không mở lại trong R4): rail 220/96 px và mốc 760/1100; thứ tự Khám phá →
Bàn viết → Tôi; một logo V-pen chỉ trên `/` ở cuối hàng đầu (#102, #325); không khôi phục “Bản thảo
gần nhất”, không “Phiên bản N” trên hàng (#213); hai số riêng Thấm/Nhận xét của bản công khai, không
tổng “tương tác” (#214); Lọc thẻ là dialog/sheet, tối đa 4 thẻ, khớp tất cả, Áp dụng mới gửi GET
(#225, #260, #308); ảnh empty WebP và anatomy ảnh → một câu → hành động (#258, #259, #276); tìm,
sắp xếp, phân trang Bàn viết theo API #201; Tôi là link `/toi`, Giao diện là nút riêng (#394); nhãn
khách “Đăng nhập” (#337). Sổ mốc thị giác chưa có mốc `approved`.

Lưu ý drift tracker: #102, #192, #308 (DG-01…03) và #351–#355 còn mở với checkbox chưa tick dù PR đã
merge; R4 coi PR đã merge + source là hiện trạng.

## 2. Kiểm chứng

| Hạng mục | Kết quả | Bằng chứng / giới hạn |
| --- | --- | --- |
| `git fetch` + pin SHA | PASS | `7ca0dd0d3ecb`, worktree riêng nhánh `docs/ban-viet-kham-pha-redesign`, checkout chính không đổi |
| `cargo xtask doctor` | PASS | Cảnh báo: không có `.env`, không đặt `VINK_IDENTITY_SOCKET` (biến môi trường truyền tay) |
| `cargo xtask ui-tokens --check` | PASS | 15 file sinh ra khớp |
| `cargo xtask build web` (qua `tools/local-budget`) | PASS | 230 s; `CARGO_PROFILE_DEV_DEBUG=0` vì đĩa còn ~6–9 GB |
| `cargo xtask dev` | NOT_RUN | Thay bằng binary của `build web` chạy 4 instance trên PostgreSQL scratch (fixture tác giả, khách + Identity native, tác giả rỗng, DB rỗng) |
| `verify web --scenario main --scenario discovery` lần 1 | FAIL | Timeout “navigation route hydration” ở `/ban-viet` 759 px/200%/tối sau ~2.400 assertion đạt; load average máy 165 (McAfee, mobileassetd) |
| Cùng lệnh, lần 2 | PASS | main, discovery-corpus, desk-guest, SIGTERM; 6.775 assertion; load ~4. Kết luận lần 1 là môi trường, không phải hồi quy |
| Capture ma trận (101 cảnh) | CAPTURED, đã mở ảnh chính | 0 overflow ngang (oracle `visual-evidence.mjs`), 0 lỗi console; mạng: 503 giả lập có chủ đích, 401/403/404 của principal fixture ở feed/Tác giả |
| Khám phá lỗi SSR 503 | NOT_RUN | Cần làm DB tạm ngừng; không chụp, chỉ thiết kế |
| Feed Đang theo dõi thành viên, Theo dõi tác giả | BLOCKED | Principal `local-fixture` không có phiên Identity thật → 401/403; cần tài khoản native |
| Claude Design | PASS (dùng được) | Canvas tạo mới; không dùng design system có sẵn (Mystic Cosmos, Testing không phải Vink) |
| Safari/Firefox, thiết bị thật, OS IME, screen reader | NOT_RUN | — |

### Ma trận capture

Driver tạm trong scratchpad (không thêm vào repo) dùng Chrome 158 headless qua CDP, DPR 1, tái dùng
oracle `apps/vink-web/e2e/visual-evidence.mjs` (overflow, PNG metadata, source identity) và các
trường context của `browser-smoke.mjs`. Chữ 200% = preference `vink.ui.text=200` của app (root
font-size), không phải browser zoom. Theme = `prefers-color-scheme` giả lập, không override.

| Viewport | Chữ | Theme | Cảnh |
| --- | --- | --- | --- |
| 1440×900 | 100% | Sáng/Tối | Khám phá, Bàn viết (+ full-page), lọc, rỗng, không kết quả, khách, focus, rail thu gọn, Giao diện, Reader, Tôi |
| 834×900 | 100% | Sáng/Tối | Khám phá, Bàn viết, rail modal, focus rail |
| 390×844 | 100%, 200% | Sáng/Tối | Khám phá, Bàn viết, tìm mở, lọc, khách, lần đầu, lỗi 503 giả lập, đang tải (request giữ) |
| 320×800 | 200% | Sáng/Tối | Khám phá, Bàn viết, tìm mở, không kết quả, khách, lần đầu |

Dữ liệu: dev-seed + fixture SQL scratch (4 tác giả, hai người cùng bút danh “Hải Đường Nhỏ” khác
handle; 8 bài công khai, tiêu đề dài, thơ xuống dòng) + 6 bản nháp tạo qua `POST/PUT /api/v1/drafts`
và `request-public`/`set-private` thật (trạng thái “Bản nháp”, “Chỉ mình tôi · Bản mới đang chờ
duyệt”, “Chưa có bản công khai · Bản mới đang chờ duyệt”, “Đã đăng · Có bản sửa”, chưa đặt tên).

Số đo chính (từ `manifest.json`, `regions.firstWork.y` so với chiều cao dùng được):

| Cảnh | Hàng đầu (y) | Dùng được | Vào màn đầu? |
| --- | --- | --- | --- |
| Khám phá 390 / 200% | 516 | 700 | Có, ở 74% |
| Khám phá 320 / 200% | 567 | 656 | Chỉ mép nhan đề |
| Bàn viết 390 / 100% | 355 | 764 | Có |
| Bàn viết 390 / 200% | 719 | 700 | **Không** |
| Bàn viết 320 / 200% | 775 | 656 | **Không** |

## 3. Findings

Loại: **IM** implementation mismatch · **UX** UX/UI defect · **OP** design opportunity · **MF**
missing functionality. Không có P0 (không mất dữ liệu, không chặn tác vụ). Ảnh nằm trong
`screenshots/` của bundle; tên là ID cảnh.

| ID | Trang/Component | Evidence | Loại | Sev | Root cause | Đề xuất |
| --- | --- | --- | --- | --- | --- | --- |
| BV-01 | Bàn viết 390/320 · 200% | `desk.populated--320x800-text200-*`: hàng đầu y=775 > 656 | UX | P1 | Header chứa Select có nhãn xếp chồng + CTA; H2; tìm; nhãn Sắp xếp xếp chồng; tải lại xuống dòng (`desk.rs` `DeskWorks`/`CreateDraftControls`/`DraftListView`, `vink.css` 821–934) | Header chung; nút tạo tách đôi; công cụ một hàng; câu riêng tư xuống cuối; gutter 16 px |
| KP-01 | Khám phá 320/390 · 200% | `explore.populated--320x800-text200-*`: y=567; tab thành ba khối | UX | P1 | `.vk-explore-modes a { flex: 1 1 auto }` dưới 760 px làm mỗi tab một hàng; logo một hàng; thanh công cụ sticky | Logo + Lọc thẻ một hàng; nhóm chế độ gập thành nút menu khi không vừa; không sticky ở hẹp/200% |
| BV-04 | Hàng Bàn viết | `desk.populated--1440x900-text100-light` | UX | P1 | `AuthorWorkRow` đặt trạng thái và giờ cùng caption; dòng “Thấm 0 · Nhận xét 0” màu on-surface nổi hơn trạng thái | Nhãn trạng thái icon + chữ 600; chỉ “chờ duyệt / không được duyệt / đã gỡ / đã đăng” có nền role; số liệu một dòng có icon |
| X-01 | Hai trang | Trục trái Khám phá x=446 (48rem), Bàn viết x=318 (64rem) | UX | P1 | `.vk-discovery` 48rem, `.vk-desk--index` 64rem; H1 Khám phá ẩn, H1 Bàn viết UI font | Một trục 48rem, một hệ header (tiêu đề `type.page-title` trái, cụm nút icon phải) |
| KP-06 | Lọc thẻ cạnh ô tìm | Tương phản nền 1.02:1 (#eadcf3 vs #e8e0f0) | UX | P2 | Button Tonal (secondary-container) đặt cạnh field container-high | Lọc thẻ vuông bo 16 px, tertiary-container (chữ 12.6:1), có badge số thẻ |
| KP-02 | Tab chế độ | `explore.populated--320x800-*`: góc bo sai khi xuống dòng; mục chưa chọn 1.13:1 với nền | UX | P2 | Radius chỉ ở `first-child`/`last-child`; chọn chỉ bằng màu (không dấu chọn/đổi hình) | Button group nối liền M3E: chọn = pill primary + dấu chọn; chưa chọn = highest bo 8 px |
| KP-03 | Đầu trang Khám phá | Tất cả: H1 ẩn; Đang theo dõi/Tác giả: H1 hiện, tab ở trên H1 | UX | P2 | #308 r1 ẩn H1 ở chế độ Tất cả; `author_discovery`/`feed` có header riêng | Một anatomy cho ba chế độ (DQ-01) |
| KP-05 | Lọc đã áp dụng | `explore.filters-applied--1440x900-text100-light` | UX | P2 | `AppliedFilters` render trước `ExploreModes`; status căn phải | Thứ tự: tìm → chế độ → chip đã áp dụng + “Xóa lọc” + status trái; badge số trên Lọc thẻ |
| KP-08 | Hàng Khám phá | Không có ngày ở chế độ Tất cả | IM | P2 | `discovery.rs` `WorkCardRow`: `date = following.then(...)` dù `published_at` có trong DTO; canvas R3 có ngày | Dòng trên: “Thể loại · ngày công khai đầu” mọi chế độ (DQ-08) |
| KP-10 | Tìm mobile | `explore.search-open--320x800-text200-light`: placeholder bị cắt “Tìm tác ph…” | UX | P2 | Kính lúp mở tìm toàn màn (#325); placeholder dài | Giữ nút icon ở mọi cỡ màn, bấm mở search dock (DQ-02); placeholder ngắn “Tìm” ở 200% |
| KP-11 | Đang theo dõi (khách) | `explore.following-guest--*`: “Đăng nhập” là link chữ | UX | P2 | `feed` dùng `vk-ui-link` | Một hành động filled như Bàn viết khách |
| KP-12 | Chế độ Tác giả | `explore.authors--*`: hàng là thẻ tonal | IM | P2 | Trái quy tắc R3 “danh sách đọc phẳng” | Hàng tác giả cùng kiểu thẻ rời của Khám phá, byline #352 |
| BV-02 | Header Bàn viết | Select “Định dạng bản thảo” nhãn nổi cạnh “Tạo bản thảo” (#394) | UX | P2 | Hai control ngang hàng, lệch baseline H1 | Nút tạo tách đôi, định dạng trong menu (DQ-06) |
| BV-03 | Danh sách Bàn viết | Một khối `surface-container-low` bo 20, hàng cách 4 px cùng tông | UX | P2 | `.vk-draft-rows` (vink.css 933); Bàn viết chưa chụp lại sau R3 (#349) | Thẻ rời `bg-content`, bo đều 20, khe 8 px (R4.2) |
| BV-05 | Thời gian hàng | “Cập nhật 12:57 GMT+7 23/11/2026” lặp mọi hàng | UX | P2 | CHOICE-01 A #213 | “Sửa 12:57 · 23/11/2026”, bỏ zone khi trùng thiết bị (DQ-05) |
| BV-06 | Hàng công khai | Bốn dòng: trạng thái, số liệu, “Của bản đang công khai”, link | UX | P2 | `author_work_summary` tách `metrics`/`public_context`/`reader_href` | Một dòng “Bản công khai · Thấm · Nhận xét · Đọc bản công khai” |
| BV-08 | Chân danh sách | Ba ghi chú, hai cỡ body | UX | P2 | `DraftListView` continuation + `vk-desk-privacy` | Một câu caption cuối danh sách |
| BV-09 | Lần đầu | `desk.first-use--*`: tìm/sắp xếp/tải lại cho kho rỗng | UX | P2 | #201 r1 giữ ô tìm ở first-use | Ẩn công cụ khi kho rỗng (DQ-04) |
| BV-10 | Khách / phiên | `desk.guest--*`: thẻ tonal + ổ khóa, căn trên | UX | P2 | `DeskAccess` khác `EmptyState` | Anatomy EmptyState căn giữa; một filled “Đăng nhập” + text “Khám phá tác phẩm” |
| BV-11 | Gutter hẹp | Bàn viết 8 px ở 320 (vink.css 833); Khám phá 32 px khi 200% (rem) | IM | P2 | Hai cách tính gutter | `size.gutter-narrow` 16 px cố định |
| BV-12 | Tiêu đề trang | Bàn viết/Tôi Be Vietnam 600; Khám phá (khi hiện)/Reader Literata | UX | P2 | `#desk-title` ghi đè font UI (vink.css 905); Khám phá dùng `type.display` | Một vai trò `type.page-title`: Be Vietnam Pro 600, 24 px (mobile 22), cho Khám phá/Bàn viết/Tôi |
| NAV-01 | Chân rail | `nav.appearance-panel--*`: “Giao diện” có pill nền như đích đang chọn | UX | P2 | `NavUtility` dùng cùng indicator khi `expanded` | Tiện ích: viền 2 px quanh icon khi mở, không tô nền |
| KP-07 | Chip Lọc thẻ | Lưới hai cột, mục lẻ; chip chưa chọn giống nút | UX | P3 | CHOICE-02 A #260 | Chip theo bề rộng chữ, chọn = pill primary (DQ-07) |
| KP-09 | Trích đoạn | “…ven sông.…” | UX | P3 | `services/vink-app/src/domain/document/excerpt.rs:171` luôn thêm “…” | Không thêm “…” sau dấu câu kết thúc, hoặc thay dấu chấm |
| KP-13 | Không kết quả | Không nhắc từ khóa, không gợi ý chế độ Tác giả | OP | P3 | `PublicResults` empty copy | “Không có tác phẩm khớp “q”.” + “Tìm tác giả “q”” |
| KP-14 | Dialog Lọc thẻ | Tiêu đề focus có viền giống ô nhập | UX | P3 | `focus_title` + outline | Focus tiêu đề không vẽ viền khi mở bằng con trỏ (độ chắc: trung bình, chụp bằng `click()` script) |
| BV-07 | Hàng Bàn viết | Mũi tên trang trí mỗi hàng | UX | P3 | `AuthorWorkRow` `vk-draft-row__open` | Bỏ, hàng là link |
| BV-13 | Tải/lỗi | `desk.loading--*` chỉ chữ; `desk.error--*` panel nằm trên ô tìm | UX | P3 | `DeskLoading`, `DeskError` vị trí | Skeleton UIS-03; lỗi ở vị trí danh sách |
| BV-14 | Hàng chưa đặt tên | “Chưa đặt tên” cùng kiểu nhan đề | OP | P3 | — | Literata italic, on-surface-variant |
| BV-15 | Lượt xem | `views` có trong API tác giả (#342/#383), Web bỏ qua | OP | P3 | Chờ vòng design; CHOICE-05 A `DESIGN_PROPOSED` | Ngoài R4; cần quyết định riêng |
| NAV-02 | Tooltip toggle | `nav.toggle-tooltip--1440x900-text100-light` phủ “Khám phá” | UX | P3 | Tooltip đặt dưới toggle | Đặt bên phải toggle, cùng hàng |
| MF-01 | Sheet Lọc thẻ | Không có “Bỏ chọn” | MF | P3 | — | Nút text “Bỏ chọn” (client, không API) |

## 4. Phương án và lựa chọn

| | A · Refined Lavender | B · Editorial First | C · Expressive Functional |
| --- | --- | --- | --- |
| Khám phá | Giữ công cụ căn phải, H1 ẩn; tab tonal có dấu chọn; thêm ngày | Tiêu đề Literata 44 + dòng dẫn, cột 42rem, nhan đề 28, Tìm/Lọc là nút chữ | Thanh tìm 56 có nút lọc + số, nhóm nút đổi hình, hàng có lớp hover tonal |
| Bàn viết | Giữ header + Select, khối tonal; trạng thái có icon | Kiểu mục lục, cột ngày phải, công cụ chữ | FAB tách đôi, thanh công cụ liền khối, hàng thẻ, chip màu |
| Rủi ro | 200% vẫn hỏng, thiếu danh tính | Tìm chậm một bước, trạng thái chìm | Ồn, mọi hàng thành thẻ |

**Đề xuất R4.1** = phân cấp chữ của B cho Khám phá + cấu trúc A cho Bàn viết + từ C: nút tạo tách
đôi, nhóm nút, nhãn trạng thái. Sau góp ý của chủ sản phẩm ngày 23/11 (“Theme chưa được expressive…
tăng độ tương phản”), R4.1 áp các chiến thuật của M3 Expressive (m3.material.io/blog: M3E 13/05/2025,
I/O 2026, Compose Material 3 1.5.0 stable 18/11/2026): trộn hình tròn/vuông để nhấn hành động,
tương phản primary–tertiary–surface, button group nối liền, chip đổi hình, contained search,
danh sách phân đoạn cho vùng thao tác. Khám phá vẫn đọc phẳng theo R3.

**R4.2** (các góp ý tiếp theo cùng ngày; thêm: bỏ nút Tải lại khỏi thanh công cụ Bàn viết/Khám phá theo yêu cầu chủ sản phẩm — danh sách tự tải lại khi tab hiện lại và đã quá hạn (`read_events::observe` + `wake.due` ở `desk.rs`; READ-02 `freshness.rs` ở Khám phá), người dùng có thể tải lại bằng trình duyệt; vẫn giữ “Thử lại” trong panel lỗi tải (`DeskError`) và “Tải lại danh sách” trong panel tạo bản thảo chưa xác nhận (`CreateNotice`, `#reconcile-drafts`) vì đó là bước phục hồi, không phải công cụ thường trực; cùng PR phải đổi hai câu ở vùng phân trang đang trỏ tới nút này (`desk.rs` “Đang xem phần N. Chọn Tải lại để về đầu danh sách.” → nút chữ “Về phần đầu” chạy cùng `refresh`; bỏ câu “… Tải lại để xem các cập nhật mới nhất.”), và các smoke bấm `#refresh-drafts` (`browser-smoke`, `draft-pagination-smoke`, `editor-desk-regressions`, `desk-empty-smoke`, `auth*-smoke`, `author-works-smoke`) đổi sang phát `visibilitychange` hoặc nút “Về phần đầu”; tìm kiếm thu về nút icon mở search dock như Drive/Gmail/Mastodon, tiêu đề trang nhỏ mà đậm — Be Vietnam Pro 600 24/22 px, bảng text style trong component sheet; góp ý thứ hai, kèm ảnh nút vuông bo): (1) control “bán vuông” đối xứng — bo ≈29%
cạnh (56→16, 48→14, 40→12, avatar 32→10, badge 24→8), nút icon luôn vuông, bỏ mọi elip/viên thuốc
hẹp, nút tạo tách đôi thành hai khối độc lập cách 4 px; (2) bớt chữ — Thấm/Nhận xét chỉ còn icon
(Pen Flight thật từ `icons.json`) + số, icon 24–26 px; Lọc thẻ, Đọc bản công khai, Giao
diện là nút icon có tên truy cập; nhãn trạng thái một từ (DQ-12); (3) nội dung sáng hơn nền, nền liền một màu — theo góp ý của chủ sản phẩm (đối chiếu ảnh mẫu
Reply của M3): nền trang, Navigation Rail và thanh dưới dùng **một màu** (Sáng `surface-container`
#efe8f6; Tối `surface` #17121d), không tách rail thành tông riêng; thẻ/hàng/vùng đọc/sheet/menu dùng
`surface-container-low` ở cả hai theme (Sáng #f7f2fa, Tối #211b29) — sáng hơn nền đúng một bậc tonal,
tỉ lệ 1.09:1 / 1.10:1, bằng tỉ lệ thẻ/nền đo trên ảnh Reply, không trắng tinh (#fffaff cho 1.16:1 và
bị đánh giá chói, thiếu hài hòa). Avatar chữ và nút icon trong thẻ lên `surface-container-high`
(#e8e0f0 / #372e42) để không chìm vào thẻ; thẻ tài khoản ở chân rail dùng màu nội dung. Đích chọn
của rail giữ `primary-container`; ô tìm/control giữ `surface-container-highest`. Rủi ro cần đo
trên app: đích chọn rail trên nền mới chỉ khác 1.08:1 về độ sáng, phân biệt chủ yếu bằng sắc độ
lavender, chữ đậm và `aria-current` — nếu thử thấy yếu thì thêm icon filled cho đích chọn. Vì vậy Khám phá chuyển từ hàng phẳng sang thẻ sáng rời nhau — mỗi thẻ bo đều 20 px, cách nhau
8 px như Reply của M3, ranh giới tạo bằng bo góc và khe nền, không đường kẻ, thay quy tắc R3 “danh sách đọc phẳng” (cần chốt cùng DQ-01). Tham chiếu thêm Mastodon:
[design tokens 4.6](https://docs.joinmastodon.org/dev/frontend/design-tokens/) (tên theo
`text/bg/border` + `base/soft/softest`, quy tắc ghép cặp chữ–nền) và
[5.0 a closer look](https://blog.joinmastodon.org/2026/10/5.0-a-closer-look/) (cột nội dung sáng
nhất trên khung tô nhẹ, hàng tác vụ icon + số, thẻ tài khoản ở chân điều hướng, bo tròn đều).

## 5. LayoutSpec R4.1

```text
Shell (giữ #394)
  Rail ≥760: toggle 48 → Khám phá → Bàn viết … [chân] Giao diện (tiện ích) → Tôi
  Thanh dưới <760: Khám phá → Bàn viết → Tôi (pill 64×32, nhãn dưới)
PageColumn 48rem, căn giữa phần còn lại sau rail; gutter <760 = 16 px cố định
Khám phá /
  Header: H1 “Khám phá” (page-title BVP 600 24/22) | [🔍 mở dock] [Lọc thẻ, badge số] logo V-pen
  SearchDock (khi mở hoặc URL có q): ← | SearchField 56 bo 16 | Lọc thẻ — thay chỗ cụm nút
  ModeGroup (nav, link + aria-current): Tất cả | Đang theo dõi | Tác giả
  AppliedRow (khi có thẻ): input chip ×N · Xóa lọc · status “Trang này: N tác phẩm”
  Results: WorkRow × n, cách 40 (32 hẹp) · Pagination “Trang tiếp” medium 56
Bàn viết /ban-viet
  Header: H1 “Bàn viết” (page-title) + ổ khóa | [🔍] [Tạo bản thảo] [▾ định dạng] — không có nút Tải lại
  SearchDock (khi mở): ← | SearchField “Tìm theo tên”
  Status row: “N tác phẩm” | Mới nhất ▾
  DraftList phân đoạn: DraftRow × n (khe 2, bo 20/4) · Continuation · một câu ghi chú
```

| Vùng | Geometry | Phân cấp và chữ | Surface | Tương tác | Responsive / 200% | States |
| --- | --- | --- | --- | --- | --- | --- |
| Header | Hàng flex, wrap; khoảng dưới 24 | H1 `type.page-title` (Be Vietnam Pro 600, 24/22, tracking 0); cụm nút icon phải: tìm (mở dock), Lọc thẻ (Khám phá) hoặc tạo (Bàn viết); không có nút Tải lại | Nền trang | Logo là link “Vink — trang chủ”; SplitButton: trái = POST tạo, phải = menu | Hẹp: hành động xuống dòng dưới H1; 200% + <360: H1 Khám phá ẩn thị giác, logo + Lọc thẻ một hàng | Tạo: pending/uncertain/reconcile giữ StatePanel hiện có |
| ToolBand | Field 56, nút 56, gap 12 (8 hẹp) | Placeholder ngắn ở 200% | Field `surface-container-highest`; Lọc thẻ `tertiary-container`; nhóm Sắp xếp highest | Enter/composition như R5.1; Lọc thẻ mở dialog/sheet hiện có | Không sticky dưới 760 hoặc khi `data-text=200` | Có thẻ: badge `tertiary` |
| ModeGroup | Cao 48, khe 2, inner bo 8, outer pill | Nhãn 15/500, chọn 600 | Chọn `primary`/`on-primary` + dấu chọn; chưa chọn highest | Link thật; hover state 8%; focus 3/2 | Không vừa một dòng → `details/summary` “Tất cả ▾” | Feed tắt (`VINK_FOLLOWING_FEED`) → ẩn mục như hiện tại |
| WorkRow | Flat, max 680 | Dòng trên tertiary 13/600 → nhan đề Literata 26/23 → trích đoạn 17/1.6 → byline 600 + @handle | Không nền | Nhan đề là link duy nhất; byline một link (#352) | Byline xuống dòng tự nhiên; không `overflow-wrap:anywhere` cho từ thường | cached/refresh giữ hành vi `public_reads` |
| DraftRow | Padding 14/20, khe 2 | Nhan đề Literata 20/19; dòng trạng thái 13/600; giờ caption | `surface-container`; nhãn có nền chỉ cho chờ duyệt (tertiary-container), lỗi (error-container), đã đăng (success-container) | Một link editor; “Đọc bản công khai” link riêng ≥44 | Dòng trạng thái wrap | Copy trạng thái giữ nguyên `adapters.rs` |
| Empty/Guest/Error | Căn giữa vùng còn lại (#276) | Ảnh 192/240 → tiêu đề (tùy) → một câu → ≤1 filled | Nền trang | Filled vuông bo 16 | 200%: hành động tới được bằng cuộn | Khách Bàn viết: “Đăng nhập” + “Khám phá tác phẩm”, 0 request riêng tư |
| Rail | 220/96 giữ token; nền = nền trang | Mở rộng: đích 56 icon + nhãn ngang, thẻ tài khoản (avatar, bút danh, @handle, nút Giao diện). Thu gọn 96 (760–1099 hoặc khi người dùng thu): nút ☰ “Mở rộng điều hướng”, đích dọc 80 px (chỉ báo 56×32 bo 12 + nhãn 12 px), chân rail là thẻ dọc [Giao diện, avatar 44 “Tôi”] | Đích hiện hành `primary-container` (chỉ báo bao icon khi thu gọn) | Giao diện: `aria-expanded` → viền 2 px primary, không tô nền; tooltip bên phải toggle; avatar thu gọn có tên truy cập “Tôi — {bút danh}” | Artboard: Khám phá/Bàn viết 1440 (mở rộng) và 1024 (thu gọn), sáng/tối | — |

Motion: đổi hình (chip, nút nhấn, mục chế độ) dùng `duration.short/medium` + `ease.spatial`;
`prefers-reduced-motion` đổi tức thì. Không thêm vòng lặp.

## 6. Token delta (áp dụng từ token 0.8.0, #402)

| Token | Giá trị | Consumer | Lý do |
| --- | --- | --- | --- |
| `size.page-column` | 768 (48rem) | `.vk-discovery`, `.vk-desk--index`, Tôi | Một trục cho hai trang (X-01) |
| `size.gutter-narrow` | 16 px, không theo cỡ chữ | Main <760 | BV-11 |
| `radius.control-56/48/40` | 16 / 14 / 12 px | Button, IconButton, SearchField, ModeGroup, FilterChip | R4.2: bán vuông đối xứng thay pill/elip |
| `radius.card` | 20 px đều bốn góc; khe giữa thẻ 8 px | WorkCard, DraftRow | Thẻ rời như Reply của M3 (góp ý chủ sản phẩm), thay kiểu xếp liền 24/8 khe 4 px |
| `color.bg-page` (= `bg-nav`) / `color.bg-content` / `color.bg-inset` | Alias theo theme: page Sáng `surface-container` / Tối `surface`; content `surface-container-low` cả hai theme; inset `surface-container-high` | page: nền trang, Navigation Rail, thanh dưới; content: thẻ, hàng, vùng đọc, sheet, menu, thẻ tài khoản; inset: avatar chữ, nút icon trong thẻ | Nền liền một màu như Reply mẫu M3; nội dung sáng hơn một bậc (1.09/1.10:1), không trắng tinh; không màu mới |
| `type.page-title` | Be Vietnam Pro 600, 24/1.3 (mobile 22), tracking 0 | H1 Khám phá, Bàn viết, Tôi | BV-12: tiêu đề nhỏ mà đậm, một vai trò cho mọi trang công cụ; Literata để dành cho tên tác phẩm và nội dung |
| Role ở consumer | `field.container` dùng `surface-container-highest` cho search contained; `tertiary-container` cho Lọc thẻ, `tertiary` cho dòng thể loại/badge | Search Bar, Lọc thẻ, WorkRow | KP-06; không thêm màu (DQ-10) |

Tương phản tính từ hex token (Sáng/Tối): on-primary/primary 6.98/8.16; primary/nền 6.77/10.83;
on-surface/highest 11.87/9.00; highest/nền 1.33/1.70; on-tertiary-container 12.61/7.47;
tertiary/nền 6.08/10.61; on-surface-variant/highest 5.05. Cần đo lại trên app sau khi triển khai.

## 7. Current code → current design → proposed

| Vùng | Code (develop) | Thiết kế hiện hành | R4.1 |
| --- | --- | --- | --- |
| Khám phá đầu trang | H1 ẩn; Lọc thẻ · tìm 328 · logo căn phải; mobile kính lúp | #308 r1/#325: H1 ẩn, một hàng công cụ | H1 page-title hiện + [tìm] [Lọc thẻ] logo; tìm mở search dock ở mọi cỡ màn (DQ-01, DQ-02) |
| Chế độ | Segmented, chọn nền primary, bo sai khi wrap | #354/#355 thêm chế độ, không spec hình | Button group nối liền, gập thành menu (DQ-03) |
| Hàng Khám phá | R3 phẳng, không ngày (Tất cả) | Canvas R3 có ngày | Có ngày; nhan đề 26 |
| Lọc thẻ | Tonal pill; chip lưới hai cột | #260 CHOICE-01/02 A | Vuông bo tertiary + badge; chip theo bề rộng (DQ-07) |
| Bàn viết header | H1 UI 28/600; Select + Filled | #394 | Literata; SplitButton (DQ-06) |
| Danh sách | Khối tonal, mũi tên, 4 dòng cho bài công khai | #213/#214 | Danh sách phân đoạn, nhãn trạng thái, một dòng số liệu |
| Lần đầu | Có công cụ | #201 r1 giữ ô tìm | Ẩn công cụ (DQ-04) |
| Khách | Thẻ + ổ khóa | #308/#394 | EmptyState anatomy |
| Rail | Giao diện tô pill khi mở; tooltip phủ | #394 | Viền tiện ích; tooltip bên phải |

## 8. Sửa theo thiết kế hiện hành vs cần quyết định

**Sửa được ngay, không đổi quyết định:** KP-02 (bo/dấu chọn, không đổi cấu trúc), KP-05, KP-06
(chỉ đổi role ở consumer — vẫn cần DQ-10 cho tertiary), KP-08, KP-09, KP-11, KP-12, KP-14, BV-03,
BV-04, BV-06, BV-07, BV-08, BV-10, BV-11, BV-12, BV-13, BV-14, NAV-01, NAV-02, MF-01.

**Chủ sản phẩm trả lời ngày 23/11/2026 khi nhận PR lát 1–3 (#402–#404)** — trích lựa chọn của
chủ sản phẩm; chưa ghi vào registry `decisions/`:

- DQ-01: “Hiện tiêu đề” — tiêu đề “Khám phá” hiện ở chế độ Tất cả (thay #308 r1).
- DQ-03: “Button group + gập menu” — nhóm chế độ nối liền, chọn = primary + dấu chọn, gập thành
  menu khi không vừa một dòng; giữ tên Tất cả / Đang theo dõi / Tác giả.
- DQ-10: “Dùng tertiary” — Lọc thẻ là nút icon `tertiary-container`; dòng thể loại và nhãn
  “Chờ duyệt” vẫn thuộc lát 9.
- DQ-11: “Không, ẩn”, “với background của me / nút tôi khi có chữ, thì để background giống màu
  nền nhé” — không làm thẻ tài khoản ở chân rail; giữ Giao diện và Tôi, mục Tôi dùng nền trang.

**Cần chủ sản phẩm chốt (còn lại):** DQ-04 ẩn công cụ khi kho rỗng · DQ-05 định dạng giờ ·
DQ-06 định dạng trong menu nút tạo · DQ-07 chip theo bề rộng · DQ-08 ngày mọi chế độ · DQ-09
avatar bo vuông toàn app · DQ-10 phần dòng thể loại/“Chờ duyệt”. DQ-02 (search dock mọi cỡ màn)
và DQ-12 (ít chữ) đã được chọn trong vòng duyệt canvas (xem Epic #401).

## 9. Khả thi trên Leptos / UI System

- Có sẵn: `SearchField` (Bar), `Button`/`IconButton` (thêm biến thể shape), `FilterChip`,
  `StatusBadge`, `Skeleton`, `Divider`, `EmptyState` app-private, `Dialog` sheet.
- Thiếu: `Menu` (UI System chưa có) cho SplitButton; `ButtonGroup` dạng link (có thể là CSS trên
  `nav` + link, không cần primitive mới ở bước đầu). CMP chưa có parity; ghi `NOT_RUN` cho native.
- `details/summary` cho chế độ khi hẹp chạy không JS, hợp với SSR hiện tại.
- Thay đổi CSS phần lớn nằm ở `vink.css` + `vink-ui.css`; token qua `cargo xtask ui-tokens`.

## 10. Issue đã tạo (1 issue = 1 PR, thứ tự phụ thuộc)

Lập ngày 23/11/2026 theo yêu cầu chủ sản phẩm, tại `develop@e987255`.
Bộ thiết kế (27 PNG artboard, README, tài liệu này, nguồn canvas, 126 ảnh audit) nằm ở
[Drive · Redesign R4](https://drive.google.com/drive/folders/1aJ-i-Y6I4zP2ia0JqmTicUf8juKaWRO8)
(32/32 file, kích thước khớp bản local; chưa so checksum; có thể cần quyền). Epic
[#401](https://github.com/loveoverflowcom/vink/issues/401) giữ thứ tự, DQ và trích lời chọn hướng.

| # | Issue | Phụ thuộc | Findings |
| --- | --- | --- | --- |
| 1 | [#402](https://github.com/loveoverflowcom/vink/issues/402) Token R4.2 (UI System) | — | X-01, BV-11, BV-12, BV-03, KP-02 |
| 2 | [#403](https://github.com/loveoverflowcom/vink/issues/403) Rail mở rộng/thu gọn + thanh dưới | 1, DQ-11 | NAV-01, NAV-02 |
| 3 | [#404](https://github.com/loveoverflowcom/vink/issues/404) Khám phá: tiêu đề, search dock, Lọc thẻ, chế độ | 1, DQ-01/02/03/10 | KP-01/02/03/05/06/10 |
| 4 | [#405](https://github.com/loveoverflowcom/vink/issues/405) Khám phá: thẻ rời, ngày, tác giả, trạng thái | 1, #352, DQ-08/09 | KP-08/11/12/13 |
| 5 | [#406](https://github.com/loveoverflowcom/vink/issues/406) Khám phá: sheet Lọc thẻ | 1, 3, DQ-07 | KP-07/14, MF-01 |
| 6 | [#407](https://github.com/loveoverflowcom/vink/issues/407) App: trích đoạn “…” | — | KP-09 |
| 7 | [#408](https://github.com/loveoverflowcom/vink/issues/408) Bàn viết: header, dock, bỏ Tải lại | 1, 3 | BV-01/08/12, X-01 |
| 8 | [#409](https://github.com/loveoverflowcom/vink/issues/409) Menu + nút tạo tách đôi | 1, DQ-06 | BV-02 |
| 9 | [#410](https://github.com/loveoverflowcom/vink/issues/410) Bàn viết: hàng tác phẩm | 1, 7, DQ-05/10/12 | BV-03/04/05/06/07/14 |
| 10 | [#411](https://github.com/loveoverflowcom/vink/issues/411) Bàn viết: khách/lần đầu/tải/lỗi | 1, DQ-04 | BV-09/10/13 |

BV-15 (Lượt xem) ngoài R4. CMP chưa có parity; native ghi `NOT_RUN`.
