# Discovery theo chữ và thẻ — TAG-PR-06 (#100)

Baseline develop@bc926a8befd4c1ca2a19dece4bee9e782177ca22, 09/11/2026.
Scope UX-01/02, PUB-02/03. [LayoutSpec](../design/tag-discovery.md) được xuất trước code.
Không triển khai auth, publish/moderation hoặc app native; không deploy.

## HOW / WHY / TEACH

Native GET ghi q, các tag lặp và cursor opaque vào URL. App giữ riêng buffer đang gõ và URL
đã áp dụng. Chọn trong bộ lọc chỉ đổi draft; Áp dụng mới gửi GET. Checkbox dùng luật discovery
(tối đa 4 thẻ, AND), không áp cardinality của TagSelector bản nháp. TagChip, WorkByline,
AuthorByline, SearchField Bar và Dialog được dùng lại. Catalog gọi public read no-store.

SSR gọi PublicReadsAdapter → Vink::discover_works/catalog_tags trong tiến trình, cùng DTO API,
không HTTP tới chính mình hoặc Identity. Trình duyệt gọi /api/v1/works; READ-02 cache key mang
đủ q/tag/cursor qua API v1. Mỗi URL có scope và resource riêng trước khi I/O bắt đầu; thay URL thay ngay scope đang chờ.
Controls có DOM ổn định ngoài các vùng async; chỉ header/chips/kết quả thay khi dữ liệu về. Chữ đang gõ, focus và caret giữ nguyên. Response A không chặn hoặc đổi nhãn/kết quả B. Lỗi refresh cùng scope giữ
last-success có cảnh báo. 404 publication gỡ card khỏi mọi search đang giữ, kể cả unobserved,
và fence các ticket cũ để chúng không hồi sinh card đó.

Ví dụ: tìm mưa → thử chọn Thơ (URL chưa đổi) → Áp dụng → Trang tiếp → mở tác phẩm →
Về Khám phá. Link về mang đủ q, tag, cursor và neo hàng. Nếu từ trang thẻ, from_tag chỉ mang
slug để app xây /the/:slug; không có URL return tùy ý. Back/Forward/reload dùng cùng URL.
Khi hàng không còn, heading kết quả là focus fallback. Đổi q hoặc thẻ bỏ cursor.

Thẻ hiển thị trên card/reader là thẻ active của public snapshot từ provider. /the/:slug lấy
nhãn/mô tả/trạng thái từ resolution cùng response discovery, không tự đoán từ slug.
Alias/former name redirect tới slug canonical server cung cấp; thẻ unknown có SSR 404,
retired có lời giải thích và link kế nhiệm chỉ khi server cung cấp. Không dựng tổng số giả;
dòng trạng thái chỉ đếm tác phẩm trên trang.

## Owner và kiểm chứng

- [URL/query](../../apps/vink-web/src/discovery.rs): serialization once, safe local return, reset cursor.
- [Page glue](../../apps/vink-web/src/pages/discovery.rs): GET/Apply, states, picker/focus.
- [SSR adapter](../../services/vink-app/src/http/ssr.rs): public use cases trong tiến trình.
- [Cache](../../apps/vink-web/src/public_reads.rs), [store](../../apps/vink-web/src/freshness.rs):
  scoped snapshots và authority-removal fence.
- [Browser](../../apps/vink-web/e2e/discovery-smoke.mjs) và fixture
  [SQL publications](../../apps/vink-web/e2e/fixtures/discovery-publications.sql):
  dữ liệu công khai tổng hợp có nhãn trong database tạm của verify web, không chứng minh author publish.

Pure tests kiểm URL/keys và response đang bay sau removal. Browser main giữ regression editor,
discussion, privacy và READ-02; discoveryReading kiểm GET/no-JS, Apply, Escape/focus, matrix
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
