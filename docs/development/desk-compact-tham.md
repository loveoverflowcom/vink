# Bàn viết gọn và Thấm — #213

Nguồn yêu cầu: [issue #213](https://github.com/loveoverflowcom/vink/issues/213),
không có comment tại lần đọc đầu 15/11/2026. Base mới nhất lúc tạo branch:
`97b29c30a87b5403759b2219ab0cdc710fb64cbd`; branch `codex/desk-compact-tham`.
[LayoutSpec delta](../design/desk-server-pagination.md#delta-213--hàng-gọn-và-thời-gian).

CHOICE-01 A: thời gian tuyệt đối vi-VN/device zone sau hydration, fallback UTC có zone rõ
trên SSR/no-JS. CHOICE-02 B: DraftRow compose metadata riêng; DraftMetadata/editor giữ version.
Full Unicode title giữ trong DOM/accessibility, focus/hover hiện đủ, touch mở cùng href editor.
Không đổi DTO, SQL, CAS, policy vote, command/order/session hoặc icon Pen Flight.

Snapshot decision registry `cef5662c19d3db1219bab14d197b5413f203aa816c99eaa11e8f2c31688091c2`:
`LVE-PR-01-CHOICE-02@r1` và `LVE-PR-01-CHOICE-08@r1` vẫn `DESIGN_PROPOSED`;
đổi copy không nâng trạng thái quyết định. Mức triển khai: `web` và copy `cmp` patterns.

Bằng chứng đang được bổ sung từ local; không kế thừa PASS của #201/#202. Không đọc/chờ CI.
