# Review và bàn giao #414 · R4.3

**Base đọc source** `develop@2712145b08a6a4c7a12c1d57033ecd2a5bd33b26`.
[LayoutSpec hiện hành](layout-spec.md) · [HOW Bàn viết](how-ban-viet.md) ·
[HOW Khám phá](how-kham-pha.md) · [WHY](why-r4.md) · [Biên bản phản biện](review-independent.md)

[Comment kết quả cũ](https://github.com/loveoverflowcom/vink/issues/414#issuecomment-7102229465)
ghi nhánh local `docs/414-r4-phan-bien-sua`, commit `2c4f5fb`, chưa push. GitHub commit lookup và
branch search, clone/worktrees trên Mac cùng kết quả Work chat và Library trong phạm vi đã tìm
chưa cung cấp commit hay bản tài liệu gốc. Trạng thái bản đó là **unavailable**. Bộ tài liệu này
dựng lại phần có nguồn từ repo, PR và comment. CHOICE-01 B giữ audit lịch sử và dùng một LayoutSpec
hiện hành. Canvas/Drive/Stitch và ảnh lịch sử chưa mở trong lượt này.

## 1. Nguồn, trạng thái, delta và owner

| Nguồn | Lời chốt, đề xuất hoặc hiện trạng | Owner và phép kiểm |
| --- | --- | --- |
| [#401](https://github.com/loveoverflowcom/vink/issues/401), [#412](https://github.com/loveoverflowcom/vink/pull/412), [audit §8](../redesign-r4-ban-viet-kham-pha.md#8-sửa-theo-thiết-kế-hiện-hành-vs-cần-quyết-định) | Nền liền một màu, tìm dock, thẻ rời, tiêu đề nhỏ đậm. DQ-01/02/03/07/08/09/12 và tertiary phần Khám phá có nguồn. R4.3 `DESIGN_PROPOSED`. | #414 sở hữu đặc tả/handoff. #402 sở hữu token. Không thay giá trị trong patch docs. |
| [#415](https://github.com/loveoverflowcom/vink/pull/415), [#427](https://github.com/loveoverflowcom/vink/issues/427) | #402–#406 đã merge. H1 Khám phá, mode links, thẻ, sheet và rail từ R4.2. #427 giữ bốn acceptance, gồm `KP-R43-01..03` và `RAIL-R43-01`. | #403–#406 giữ cấu trúc. #427 kiểm reflow ở Lớn 150% sau #413, Tab/AX nhóm/menu, q/đóng dock và dấu hiệu active rail. |
| [#426](https://github.com/loveoverflowcom/vink/pull/426), [delta #408](https://github.com/loveoverflowcom/vink/issues/408#issuecomment-7102205637) | #407 excerpt, #408 Desk dock và recovery, DQ-06 giữ Select #409. Có mismatch giữa đề xuất đóng/xóa q, sort, focus và runtime Desk hiện hành. | #407 scoped domain, #408 Desk consumer, #409 create format. Dùng [HOW Desk](how-ban-viet.md) để chốt ca kiểm hiện hành trước khi sửa. |
| [#429](https://github.com/loveoverflowcom/vink/pull/429), [delta #410](https://github.com/loveoverflowcom/vink/issues/410#issuecomment-7102206518) | Thẻ, nhãn có cấu trúc, public link, metric 0/“—” đã vào develop. DQ-05 và phần “Chờ duyệt” DQ-10 còn `OPEN`. | #410 giữ topology và dữ kiện. Kiểm bản công khai khác tên nháp, 0/missing, target riêng. |
| [#430](https://github.com/loveoverflowcom/vink/pull/430), [delta #411](https://github.com/loveoverflowcom/vink/issues/411#issuecomment-7102207202) | Guest/first-use/error trên EmptyState, skeleton sau 300 ms. DQ-04 còn `OPEN`. Comment nêu đề xuất ẩn kính lúp và sort lúc kho rỗng. | #411 giữ runtime, #414 chỉ ghi drift. Kiểm 0 private GET cho khách, first-use và search miss tách, node/focus qua chuyển trạng thái. |
| [#413](https://github.com/loveoverflowcom/vink/issues/413), [delta #413](https://github.com/loveoverflowcom/vink/issues/413#issuecomment-7102207823) | Chốt bỏ Giao diện khỏi rail, 100/150 và normalize cũ 200. DQ-11 chỉ bị thay ở phần Giao diện. Source baseline còn rail và 100/200. | #413 sở hữu runtime; phối hợp #427 về cờ/modes/outlined. Chuyển Tôi từ editor qua guard. [#428](https://github.com/loveoverflowcom/vink/issues/428) sở hữu redesign editor. |
| [#431](https://github.com/loveoverflowcom/vink/pull/431), [registry](../../../decisions/README.md#nền-tảng-tailwind-css-v4-cho-web) | Web dùng compiler Tailwind sau #431. VDR nền tảng `PENDING`; không ràng buộc kiến trúc mới từ #414. | #417/#418 sở hữu pipeline và skills. Patch #414 chỉ nằm ở `docs/design`. |

## 2. Disposition F-01–F-06

| Finding | Kết quả ở owner #414 | Delta còn lại |
| --- | --- | --- |
| F-01 | **fixed**. Một [anatomy R4.3](layout-spec.md) cho thẻ, control, H1. Gắn banner lịch sử audit §5/§7. | Nếu asset đúng revision khả dụng, so vào spec và giữ provenance. |
| F-02 | **fixed docs**. [Spec shell](layout-spec.md#3-shell-rail-và-thanh-dưới) ghi #413 supersedes phần Giao diện, mức 100%/150% và stress 200%. | #403 AC-02/04 phải đọc với delta #413. Kiểm rộng/thu gọn/modal; sửa runtime tại #413. |
| F-03 | **fixed**. [WHY](why-r4.md) ghi KP-08 và tertiary Khám phá đã có nguồn sau audit; tertiary “Chờ duyệt”, DQ-04/05 vẫn mở. | Chỉ thay runtime tương ứng khi có lời chốt đúng phạm vi. |
| F-04 | **fixed docs / deferred asset**. Design index và audit dẫn spec mới. Dùng ID Drive `1aJ-i-Y6I4zP2ia0JqmTicUf8juKaWRO8` theo #408/#410/#411. | Sửa câu “chưa commit” và ID Drive ở handoff issue khi có grant tracking. Cần đúng revision và quyền khi dùng làm nghiệm thu visual. |
| F-05 | **fixed handoff**. #407 là excerpt domain. Check tối thiểu `cargo xtask verify unit app --filter excerpt` và đối chiếu test/source ở ETag/consumer nếu claim cần. | [#426](https://github.com/loveoverflowcom/vink/pull/426) ghi 8 unit test trên source domain tương ứng. Canvas hoặc ma trận visual rộng không phải prerequisite của pure-domain excerpt. |
| F-06 | **fixed docs / deferred independent**. Có hai HOW, WHY và [self-review](review-independent.md) với file/owner/recheck. | A3 review độc lập sau khi có delta mới. Kết quả review cũ cần artifact gốc mới đối chiếu được từng finding. |

## 3. Handoff theo lát

- #401 dùng spec ở đây, audit còn làm provenance; #402 tiếp tục là owner token và generated output.
- #403/#413 áp scope bỏ Giao diện ở mọi trạng thái rail; #404 giữ link, flag và q; #405
  giữ thể loại/ngày đầu/byline theo nguồn; #406 giữ draft → GET, max 4/AND. #427 phụ trách gap sau #415.
- #407 dùng kiểm domain phù hợp. #408 đối chiếu nút/Esc/Back, query hiện hành, sort ẩn, `#restart-drafts`
  ở cursor Invalid và focus khi phục hồi trước khi chốt patch thêm. #409 giữ Select cả lần đầu.
- #410 giữ facts/metric/tên và link đọc như #429. #411 dùng #430 làm baseline và tách câu hỏi
  DQ-04 chưa chốt khỏi phần EmptyState đã có. #428 giữ editor riêng.
- Giữ metadata rõ app 100%/150% và stress 200%. Không chuyển hàng loạt chữ “200%” trong ảnh/test
  lịch sử thành 150%. Thay đổi UI thực sự cần smoke nhỏ đúng owner và bảng năm chiều theo spec.

## 4. Acceptance #414

| AC | Trạng thái của delta |
| --- | --- |
| AC-01 | Có ma trận tại base và cách xử lý F-01..F-06, DQ có nguồn. |
| AC-02 | Desk và Explore có success/failure, caller/callee, trace SSR/hydration và HTTP/store. |
| AC-03 | WHY phân loại RECORDED/INFERRED/UNKNOWN, có lịch sử và điểm cần xem lại. |
| AC-04 | LayoutSpec hiện hành và design index có link. R4.1/R4.2, design asset chưa mở và #413 được đánh dấu. |
| AC-05 | Source token/PRD/TOML giữ owner và trạng thái. Delta này không đổi token, policy hay decision payload. |
| AC-06 | Self-review có disposition của finding source xác minh được. Bản review gốc và A3 review độc lập còn thiếu. |
| AC-07 | Kiểm nhẹ diff/links là phạm vi docs. Browser, asset/capture mới, IME và screen reader `NOT_RUN`. |
| AC-08 | Các file này chứa spec, source và delta theo owner. Final SHA và kết quả kiểm ghi ở PR; issue tracker giữ nguyên. |
