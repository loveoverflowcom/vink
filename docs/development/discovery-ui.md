# Discovery theo chữ và thẻ — TAG-PR-06 (#100)

Baseline develop@bc926a8befd4c1ca2a19dece4bee9e782177ca22, 09/11/2026.
Scope UX-01/02, PUB-02/03. [LayoutSpec](../design/tag-discovery.md) được xuất trước code.
Không triển khai auth, publish/moderation hoặc app native; không deploy.

## Bộ lọc Web #603

[LayoutSpec hiện hành](../design/redesign-r4/layout-spec.md#4-khám-phá) thay picker nhiều thẻ
của #225/#260/#308/#404/#406. Chọn một link trong catalog thay toàn bộ điều kiện thẻ. Escape,
Back và Đóng trả focus về nút mở. Hàng chip lấy nhãn từ kết quả cùng query. Cache, API và URL
cũ vẫn giữ đủ tập AND. Catalog lỗi cho phép thử lại, catalog rỗng vẫn giữ lối gỡ thẻ đang áp dụng.
Trang thẻ xếp BackLink tới Khám phá, H1, SearchField “Tìm trong thẻ” rồi kết quả. Ở trang đầu
của thẻ hợp lệ rỗng và q rỗng, người đọc có một lối lên Khám phá. Khi có q, xóa thẻ giữ q
và xóa q giữ thẻ. Alias, thẻ không tồn tại, thẻ ngừng dùng và trang phân trang rỗng dùng
recovery riêng của owner dữ liệu.

### Các phiên bản trước #603

**Action surfaces #225 r2 (16/11/2026):** [LayoutSpec](../design/action-surfaces-desk-discovery-225.md)
và [registry triển khai](action-surfaces-225.md) ghi baseline lịch sử của launcher Tonal + FilterList, helper
“Khớp tất cả thẻ · Tối đa 4”, status “Trang này: N tác phẩm”. Giữ native GET/no-JS,
draft-vs-applied selection và Filled-khi-đổi/Tonal-khi-chưa-đổi của Apply đã có từ trước.
[Evidence chuẩn bị #240](evidence/action-surfaces-225/README.md) là baseline lịch sử riêng.

**Checkbox-chip #260:** recipe chỉ dành cho `.vk-discovery-option` dùng ba track đối xứng
cho mark/nhãn/end-slot, giữ nhãn đúng tâm và hình học ổn định khi chọn. Hai cột và native
Checkbox/GET được giữ; không đổi primitive Checkbox hay command FilterChip.
[LayoutSpec và lựa chọn](../design/discovery-chip-260.md) giữ anatomy/CHOICE;
[evidence](evidence/discovery-chip-260/README.md) ghi phép đo thực, đối chứng âm, matrix
dock/dialog và giới hạn kiểm chứng.

**Redesign r1 [#308](https://github.com/loveoverflowcom/vink/issues/308):** từ 760 CSS px, hàng công cụ ghim gồm Lọc thẻ, ô tìm và logo Vink. Trên điện thoại, kính lúp mở màn tìm toàn màn hình. Enter áp dụng từ khóa sau khi kết thúc composition. Xóa từ khóa giữ thẻ đang lọc. Khi quay về danh sách, focus trở về kính lúp và vị trí cuộn được phục hồi. Trước hydration, trang vẫn có form GET.
Danh sách một cột hiển thị tác giả, thẻ đầu tiên nếu có, nhan đề và trích đoạn từ revision đang công khai. Chỉ nhan đề và tên tác giả có đánh dấu từ khóa khớp. Trích đoạn giữ ngắt dòng thơ. Dialog
desktop và sheet mobile dùng cùng danh mục với `<details>` trước hydration. Một lượt đọc theo
scope URL, có thử lại khi lỗi. Danh mục rỗng ẩn nút và form lọc, các thẻ đang áp dụng vẫn gỡ được.
Đóng, Escape và Back bỏ lựa chọn thử. Áp dụng bật khi đã đổi lựa chọn, gửi GET giữ query và đưa
focus về vùng kết quả. Placeholder rỗng căn theo toàn vùng kết quả dưới hàng công cụ.

**Empty artwork #258:** [LayoutSpec R1](../design/empty-artwork-258.md),
[asset/pipeline owner](../../apps/vink-web/assets/empty/README.md) và
[evidence của lượt này](evidence/empty-webp-258/README.md). Composition app-owned
`ui::empty_state::EmptyState` dùng WebP đã pin; bỏ heading “Kết quả” và count 0 hiển thị,
đổi thành đúng một câu result status ở empty. Count của nonempty vẫn là số trang.
`community-stream` giữ tên vùng và focus-visible; unknown/retired tag và empty cursor page
không bị gọi là corpus rỗng. Desk #259 dùng cùng API/CSS/asset; chưa adopt trong #258.

**Quay lại từ Reader #236:** [LayoutSpec](../design/discovery-return-position-236.md) và
[bằng chứng trước/sau](evidence/web-perf-234-236/README.md). Router ghi fragment vào đúng entry
để Back, Forward và mở lại cùng tác phẩm đồng bộ URL. Kết quả đã settle phục hồi hàng hoặc
landmark; list refresh không lấy lại focus. Chạy `verify web --scenario return-position --release`
để kiểm keyboard và 50 vòng. Không dùng view giữ lại hoặc snapshot offset.

## HOW / WHY / TEACH

Native GET ghi q, các tag lặp và cursor opaque vào URL. App giữ riêng buffer đang gõ và URL
đã áp dụng. Chọn thẻ điều hướng ngay tới URL một thẻ. Trang dùng TagChip, SearchField Bar và Dialog. Thẻ tác phẩm lấy nhan đề làm tên truy cập của liên kết. Từ [#293](https://github.com/loveoverflowcom/vink/issues/293), thẻ có hai đích ngang cấp, không lồng nhau: liên kết nhan đề phủ cả thẻ để mở Reader, và bút danh có Handle do App cấp mở `/@handle`. DOM đặt heading trước dòng tác giả nên Tab đi nhan đề → bút danh, còn CSS hiển thị dòng tác giả phía trên như r1. Bút danh chưa đối chiếu trong cache hiện nhãn trung tính, không có link. Catalog gọi public read no-store.

SSR gọi PublicReadsAdapter → Vink::discover_works/catalog_tags trong tiến trình, cùng DTO API,
không HTTP tới chính mình hoặc Identity. Trình duyệt gọi /api/v1/works; READ-02 cache key mang
đủ q/tag/cursor qua API v1. Mỗi URL có scope và resource riêng trước khi I/O bắt đầu; thay URL thay ngay scope đang chờ.
Controls có DOM ổn định ngoài các vùng async; chỉ header/chips/kết quả thay khi dữ liệu về. Chữ đang gõ, focus và caret giữ nguyên. Response A không chặn hoặc đổi nhãn/kết quả B. Lỗi refresh cùng scope giữ
last-success có cảnh báo. 404 publication gỡ card khỏi mọi search đang giữ, kể cả unobserved,
và fence các ticket cũ để chúng không hồi sinh card đó.

Ví dụ: tìm mưa → chọn Thơ (URL có q và một tag) → Trang tiếp → mở tác phẩm →
Về Khám phá. Link về mang đủ q, tag, cursor và neo hàng. Nếu từ trang thẻ, from_tag chỉ mang
slug để app xây /the/:slug; không có URL return tùy ý. Back/Forward/reload dùng cùng URL.
Khi hàng không còn, landmark kết quả có tên là focus fallback (#258). Đổi q hoặc thẻ bỏ cursor.

Thẻ hiển thị trên card/reader là thẻ active của public snapshot từ provider. /the/:slug lấy
nhãn/mô tả/trạng thái từ resolution cùng response discovery, không tự đoán từ slug.
Alias/former name redirect tới slug canonical server cung cấp; thẻ unknown có SSR 404,
retired có lời giải thích và link kế nhiệm chỉ khi server cung cấp. Không dựng tổng số giả;
dòng trạng thái chỉ đếm tác phẩm trên trang.

## Owner và kiểm chứng

- [URL/query](../../apps/vink-web/src/discovery.rs): serialization once, safe local return, reset cursor.
- [Page glue](../../apps/vink-web/src/pages/discovery.rs): GET một thẻ, states, picker/focus.
- [SSR adapter](../../services/vink-app/src/http/ssr.rs): public use cases trong tiến trình.
- [Cache](../../apps/vink-web/src/public_reads.rs), [store](../../apps/vink-web/src/freshness.rs):
  scoped snapshots và authority-removal fence.
- [Browser](../../apps/vink-web/e2e/discovery-smoke.mjs) và fixture
  [SQL publications](../../apps/vink-web/e2e/fixtures/discovery-publications.sql):
  dữ liệu công khai tổng hợp có nhãn trong database tạm của verify web, không chứng minh author publish.

Pure tests kiểm URL/keys và response đang bay sau removal. Browser main giữ regression editor,
discussion, privacy và READ-02; discoveryReading kiểm GET/no-JS, chọn và thay thẻ, Escape/focus, matrix
320/390/834/1024/1200/1440 sáng/tối chữ100/200, tag detail và cursor lỗi. Phase discovery-corpus
dùng 14 publication tổng hợp để kiểm tìm ngoài trang đầu, hai thẻ AND, trang kế, reader return,
reload và race A/B bằng transport interception có nhãn.

## Evidence của lượt triển khai

| Kiểm tra local | Source đã kiểm | Kết quả và phạm vi |
| --- | --- | --- |
| `cargo xtask verify quick` | `de0696a` | PASS 9 bước: tokens, manifest, fmt, SSR/hydrate/preview clippy, doctest, unit/contracts và repo guards |
| `cargo xtask verify web` | `bfd055f` | PASS đủ 9 bước, PostgreSQL thật và Chrome: SSR/privacy/READ-02, editor/save/conflict/leave/restart/export, draft tags, discovery corpus và endorsement |
| `cargo xtask ui-preview leptos verify` | cây nguồn đã chốt ở `de0696a` | PASS 5 bước, 784 capture, mọi scenario 320/390/834/1024/1440 × text100/200, không lỗi console |
| `public-search` + `discovery-corpus` | `de0696a` | PASS browser thật sau sửa Unicode/heading: matrix 320/390/834/1024/1200/1440 × light/dark × text100/200; GET/no-JS, alias 308, unknown 404, cursor 422, avatar32/44; 14 publication fixture, AND/pagination, reader return, race A/B và focus/caret |
| `cargo xtask build` | `de0696a` | PASS binary và browser bundle |

Không chạy lại toàn bộ `verify web` sau `bfd055f`; hai phase discovery liên quan đã chạy lại
trên `de0696a`, cùng quick/build và preview. Sửa cuối gồm chuẩn hóa NFC/lowercase của URL thẻ,
heading chữ lớn và assertion tương ứng; không đổi persistence/domain/editor.
Ảnh desktop trang thẻ, mobile tối chữ200 và byline preview đã mở kiểm tra trực tiếp.
Log và ảnh của lượt này được giữ ngoài repo trong `evidence/vink-100` của work chat;
`REPORT.md` phân biệt bản đầy đủ trước sửa cuối với ảnh cuối đã chọn. Gallery preview thô đã
được dọn sau khi giữ log/metadata và 3 ảnh chọn lọc; không coi 784 capture là 784 ảnh đã mở.
Máy: macOS arm64, Rust 1.99.1, Node 22.15.0, PostgreSQL 17.11 tạm trên loopback.
Build giảm dung lượng: CARGO_INCREMENTAL=0, CARGO_PROFILE_DEV_DEBUG=0,
CARGO_PROFILE_TEST_DEBUG=0. GitHub CI không đọc/chờ.

OS IME thật, screen reader, thiết bị/virtual keyboard thật và Android/iOS/native service integration: NOT_RUN.
Các lane riêng `verify postgres|discussion|expressions|kani`: NOT_RUN trong lượt UI này;
không thay storage/domain/model. IME trong browser là event simulation, không phải phiên gõ hệ điều hành. Enter khi đang tổ hợp ở ô tìm
do `SearchField` dùng chung giữ (form không còn bản sao riêng); kiểm bằng phím tin cậy sau
`Input.imeSetComposition`, và đã chứng minh đối chứng âm (tắt guard dùng chung thì kiểm tra thất bại).
Catalog rename/retire trực tiếp khi đang mở trang chưa có phiên browser riêng.
Fixture SQL publication không chứng minh auth hoặc quy trình xuất bản production.
