# Lượt xem tác phẩm — registry triển khai và bàn giao

Registry này ghi mức triển khai, bản đồ từ quyết định sang issue và kịch bản nghiệm thu xuyên
suốt cho Lượt xem. Luật, ví dụ và vòng đời nằm ở [đặc tả nghiệp vụ](../architecture/work-views.md).
Yêu cầu có ID nằm ở [PRD §17](../../prd/requirements.md#17-lượt-xem-tác-phẩm-work-views).
Nguồn: [Epic #323](https://github.com/loveoverflowcom/vink/issues/323),
[#340](https://github.com/loveoverflowcom/vink/issues/340) và các issue con
[#341](https://github.com/loveoverflowcom/vink/issues/341),
[#342](https://github.com/loveoverflowcom/vink/issues/342),
[#343](https://github.com/loveoverflowcom/vink/issues/343).

## 1. Trạng thái bốn chiều

| Chiều | Trạng thái |
| --- | --- |
| Quyết định sản phẩm | VIEW-01..12 `PRODUCT_CONFIRMED` trong brief #340 ngày 21/11/2026. [VDR-WORK-VIEWS@r1](../../decisions/proposals/VDR-WORK-VIEWS.r1.toml) có [signoff](../../decisions/signoffs/VDR-WORK-VIEWS.r1.toml) `PRODUCT_CONFIRMED` ngày 21/11/2026, là revision hiện hành. OQ-1..OQ-4 `OPEN`. |
| Quyết định kiến trúc | CHOICE-01..06 `OPEN`. Chưa có `DESIGN_PROPOSED` nào. |
| Mức triển khai | `NOT_IMPLEMENTED` cho mọi VIEW-ID (bảng ở §3). |
| Bằng chứng | Kiểm tài liệu (§6). Không có runtime PASS. |

## 2. Snapshot quyết định

Tra bằng `cargo xtask decision` tại nhánh `docs/340-dac-ta-luot-xem`, base
`develop@9f68c513b5dae224b6a9f896d04bc9d1d035e1b8`, sau khi ghi signoff:
snapshot `8a322aed0446142317f78323e9bcb1748deae7e50e6cfbc892c360f2bc68aaaf`
(137 revision). Trước signoff, ngay sau khi tạo proposal, snapshot là
`176154f6ec916e58eb3e4d5db8419e7e256d8c2dfcc8c3b8a643e67708fa8330`. Snapshot đổi khi kho có
revision hay signoff mới; người tiếp tục tra lại.

| ID@rN | Trạng thái | SHA-256 payload | Vai trò |
| --- | --- | --- | --- |
| `VDR-WORK-VIEWS@r1` | `PRODUCT_CONFIRMED` (`current`, signoff 21/11/2026) | `cfb789cc2c5f7a99a2f8c8e24f0057815d736d7b02cdacde50d327c8a525e008` | Luật Lượt xem của PR này. |
| `O-17@r2` | `PRODUCT_CONFIRMED` | `cb30cb43040a3fad50bd4930096e537057a2f3c6037b387d8c332be497fa15aa` | Trang rút sau kiểm duyệt trả HTTP 200; không phải lượt. |
| `USER-02@r1` | `PRODUCT_CONFIRMED` | `2293bf58194809957b892b74bc3891d56a05998e7b97008ad1c8fbfd98fdc97b` | Hai audience. |
| `LVE-PR-01-CHOICE-02@r1` | `DESIGN_PROPOSED` | `766485bf6d2c6eabc3292921fc478d01a13376b46bed36bee9a00150a6ad6a03` | Thấm gắn `RevisionId`; Lượt xem cố ý khác (P-06). |
| `VDR-DECISION-SIGNOFF@r1` | `PRODUCT_CONFIRMED` | `add9c28677a66d759cc4c352eada0ebf92cc64dcc0849d489f2d89f16a022dc2` | Brief chưa tự là signoff của payload TOML. |
| `VDR-AUTHOR-WORK-FEEDBACK@r1` | `PENDING` | `4616ced97f723fac18cc31b1c11639d1eb7649b83340bb2d702d92999a3a14a7` | Mẫu “—” khác “0”; ẩn Thấm/Nhận xét khi riêng tư (OQ-1). |
| `O-08@r1` | `OPEN` | `410cb3637a3d013c49b9bd746ec8b5ddb292761fed6b987c3134b281c95b4375` | Purge vật lý của tác phẩm, tách khỏi luật xóa số liệu (OQ-2). |

## 3. Mức triển khai theo yêu cầu

| ID | Mức hiện tại | Issue sở hữu | Ghi chú |
| --- | --- | --- | --- |
| VIEW-01 | `NOT_IMPLEMENTED` | #343 (điểm phát), #341 (kiểm tại lúc ghi) | Điểm hiển thị có sẵn: `ReaderResponse::Published` → `PublicationView`. |
| VIEW-02 | `NOT_IMPLEMENTED` | #343 | No-JS không đếm theo luật. |
| VIEW-03 | `NOT_IMPLEMENTED` | #341 | Cửa sổ trượt neo tại lượt được tính. |
| VIEW-04 | `NOT_IMPLEMENTED` | #341 | Mã khách first-party, thời hạn 24 giờ. |
| VIEW-05 | `NOT_IMPLEMENTED` | #341 | So principal với chủ tác phẩm tại lúc ghi. |
| VIEW-06 | `NOT_IMPLEMENTED` | #341 (khóa ghi), #342 (đọc không reset) | Không có trong bản đồ của brief; xếp theo khóa dữ liệu. |
| VIEW-07 | `NOT_IMPLEMENTED` | #342 | Chỉ API tác giả; không API công khai. |
| VIEW-08 | `NOT_IMPLEMENTED` | #342 (đọc), #341 (không tăng khi hết công khai) | OQ-1 cho hàng riêng tư. |
| VIEW-09 | `NOT_IMPLEMENTED` | #342 | Mốc bật và nhãn “Tính từ”. |
| VIEW-10 | `NOT_IMPLEMENTED` | #343 (retry), #341 (idempotency) | Kỳ vọng khoảng 1 phút, không SLA. |
| VIEW-11 | `NOT_IMPLEMENTED` | #341 | Xóa khi xóa tác phẩm/tài khoản là target (OQ-2). |
| VIEW-12 | `NOT_IMPLEMENTED` | #342 (trạng thái dữ liệu), lượt thiết kế (hiển thị) | Copy cuối thuộc lượt thiết kế. |

## 4. Bản đồ P/CHOICE sang issue

| Issue | Kết quả | P-ID | CHOICE | Câu còn mở chạm tới |
| --- | --- | --- | --- | --- |
| [#341](https://github.com/loveoverflowcom/vink/issues/341) Ghi và chống trùng | Use case ghi, migration, adapter PostgreSQL, purge mã hết hạn | P-03, P-04, P-05, P-11 (thêm P-06 cho khóa `WorkId`; P-01/P-08 cho kiểm tại lúc ghi) | CHOICE-02, CHOICE-03, CHOICE-06 | OQ-2 (thiết kế để xóa theo `WorkId` làm được) |
| [#342](https://github.com/loveoverflowcom/vink/issues/342) Đọc theo quyền | Query/read-model, DTO, contract đọc của tác giả, trạng thái “0”/“—”/“Tính từ” | P-07, P-08, P-09 (thêm P-06 cho đọc cộng dồn; VIEW-12 cho trạng thái) | CHOICE-05 | OQ-1, OQ-4 |
| [#343](https://github.com/loveoverflowcom/vink/issues/343) Reader và kiểm xuyên suốt | HTTP thu nhận, tín hiệu Web tại điểm thân bài đã hiện, kịch bản browser cuối | P-01, P-02, P-10 | CHOICE-01, CHOICE-04 | OQ-3 |
| Lượt thiết kế (chưa có issue) | LayoutSpec vị trí số, copy “—”/“Tính từ”/độ phủ trong Bàn viết | VIEW-07, VIEW-12 | — | OQ-1 |

Thứ tự: #341 sau PR này; #342 sau #341 (dùng dữ liệu do đường ghi tạo); #343 viết tín hiệu sau
#341 và nghiệm thu xuyên suốt sau #342.

## 5. Kịch bản nghiệm thu xuyên suốt

Dùng làm oracle chung cho #341–#343. Dữ liệu thử: tác phẩm W công khai lần đầu sau mốc bật;
T là tác giả; A là thành viên; G là khách; D là admin. Thời điểm tính theo đồng hồ thử. Tổng là
số tác giả đọc được qua API của #342 sau khi chờ hội tụ (kỳ vọng tối đa khoảng 1 phút).

| Bước | Thao tác | Tổng mong đợi | Ví dụ, vòng đời | Lane |
| --- | --- | --- | --- | --- |
| 1 | T mở Bàn viết | 0 (số thật, không phải “—”) | L-02 | #342 HTTP, #343 browser |
| 2 | 08:00 A bấm thẻ W ở Khám phá, thân bài hiện | 1 | E-01 | #343 browser |
| 3 | A refresh, Back rồi Forward | 1 | E-14, E-15 | #343 browser |
| 4 | Gửi lại cùng mã sự kiện sau khi mất phản hồi; bấm đúp | 1 | E-27, E-28 | #341 PostgreSQL, #343 |
| 5 | G mở link chia sẻ W ở tab mới, rồi mở thêm tab thứ hai | 2 | E-05, E-07, E-29 | #343 browser; #341 đồng thời |
| 6 | T đăng nhập mở W ở Reader; mở nháp ở Bàn viết | 2 | E-22, E-23 | #341, #343 |
| 7 | D mở ca duyệt của W; rồi mở W ở Reader công khai | 2, rồi 3 | E-25, E-26 | #341, #343 |
| 8 | Trình duyệt tắt JavaScript mở W; bot tải W để tạo bản xem trước | 3 | E-31, E-06, E-18 | #343 |
| 9 | r2 thay r1; A mở lại W lúc 10:00 | 3, không reset | E-35, L-06 | #341, #342 |
| 10 | T chuyển W sang “Chỉ mình tôi”; A mở URL cũ | 3, Bàn viết vẫn hiện 3 (OQ-1) | E-12, L-07 | #342, #343 |
| 11 | T công khai lại; A mở W lúc 08:01 hôm sau | 4 | E-17, L-09 | #341, #343 |
| 12 | Làm lỗi nguồn số liệu khi đọc | “—” kèm lý do; tên, trạng thái, link sửa còn | L-05 | #342 |
| 13 | Làm lỗi đường ghi khi A mở W | Reader đọc bình thường; tổng không tăng hoặc tăng sau retry; không bao giờ +2 | E-32, E-33 | #343 |
| 14 | Tác phẩm khác V công khai trước mốc bật | Số kèm “Tính từ dd/mm/yyyy” | L-12 | #342 |

Bước nào chạm OQ-1..OQ-4 thì ghi kết quả quan sát là `OPEN`, không tính là PASS hay FAIL.

## 6. Bằng chứng của PR tài liệu #340

Chỉ kiểm tài liệu; không chạy runtime. Kết quả ghi ở mục cuối sau khi chạy trên nhánh.

| Phép kiểm | Mục đích |
| --- | --- |
| `git diff --check` | Không có lỗi khoảng trắng. |
| Kiểm link tương đối của các file đã sửa | Mọi đường dẫn và anchor nội bộ trỏ đúng. |
| `cargo xtask decision validate` | Kho TOML hợp lệ sau khi thêm proposal. |
| `cargo xtask decision query VDR-WORK-VIEWS` | `pending_only` trước signoff, `current` sau signoff. |
| `wc -c AGENTS.md` | Dưới 24.000 byte. |
| Đối chiếu P-01..P-11 của #323 | Mỗi câu có dòng VIEW-ID, dòng trong bảng quyết định và ví dụ. |

### Kết quả ngày 21/11/2026

Chạy trên cây làm việc của nhánh `docs/340-dac-ta-luot-xem` (base
`develop@9f68c513b5dae224b6a9f896d04bc9d1d035e1b8`), Linux, trước commit.

| Phép kiểm | Kết quả |
| --- | --- |
| `git diff --check` | Không lỗi. |
| Kiểm link tương đối và anchor kiểu GitHub cho `prd/requirements.md`, hai file `work-views.md`, `docs/README.md`, `docs/development/README.md`, `decisions/README.md` | 0 link hỏng. Script tạm, có ca đối chứng âm (anchor sai, file thiếu) được bắt đúng. |
| `cargo xtask decision validate` | 137 revision, snapshot `176154f6ec916e58eb3e4d5db8419e7e256d8c2dfcc8c3b8a643e67708fa8330`, 98 tham chiếu khai báo; review ngữ nghĩa là việc riêng. |
| `cargo xtask decision query VDR-WORK-VIEWS` | `pending_only`, head r1, SHA-256 `cfb789cc2c5f7a99a2f8c8e24f0057815d736d7b02cdacde50d327c8a525e008`. |
| `wc -c AGENTS.md` | 18.876 byte; không sửa AGENTS.md. |
| Đối chiếu 11 câu của #323 | Đủ, bảng dưới. |

Sau signoff (cùng ngày, trên cây làm việc sau commit `8c9cbf38830042e8b761cf709a80f9a4f36f33da`):

| Phép kiểm | Kết quả |
| --- | --- |
| `cargo xtask decision signoff VDR-WORK-VIEWS@r1 cfb789cc… INPUT` | `current: true`, `PRODUCT_CONFIRMED`. |
| `cargo xtask decision validate` | 137 revision, snapshot `8a322aed0446142317f78323e9bcb1748deae7e50e6cfbc892c360f2bc68aaaf`, 99 tham chiếu khai báo. |
| `cargo xtask decision query VDR-WORK-VIEWS` | `current`, r1, `PRODUCT_CONFIRMED`, SHA-256 không đổi. |
| `cargo xtask decision query VIEW-03` | `related_only` trả `VDR-WORK-VIEWS@r1`. |
| `cargo xtask decision refs` | Marker `active VDR-WORK-VIEWS@r1` ở đặc tả được nhận. |
| `git diff --check`, kiểm link, `wc -c AGENTS.md` | Không lỗi; 0 link hỏng; 18.876 byte. |

### Đối chiếu 11 câu của #323

| #323 | PRD | Bảng quyết định | Ví dụ, vòng đời |
| --- | --- | --- | --- |
| P-01 | VIEW-01 | Có | E-01, E-09..E-13, E-36 |
| P-02 | VIEW-02 | Có | E-01..E-08, E-18, E-31 |
| P-03 | VIEW-03 | Có | E-14..E-17, E-21, E-27..E-29 |
| P-04 | VIEW-04 | Có | E-19, E-20, E-30 |
| P-05 | VIEW-05 | Có | E-22..E-26 |
| P-06 | VIEW-06 | Có | E-35, L-06 |
| P-07 | VIEW-07 | Có | Cột “Ai thấy gì” |
| P-08 | VIEW-08 | Có | E-12, E-13, E-34, L-07..L-09 |
| P-09 | VIEW-09 | Có | L-12, L-13 |
| P-10 | VIEW-10 | Có | E-28, E-32..E-34, L-04, L-05 |
| P-11 | VIEW-11 | Có | L-10, L-11 |

### AC của #340

| AC | Phép kiểm | Kết quả, gap |
| --- | --- | --- |
| AC-01 | Mỗi P-ID dẫn về lời chốt 21/11/2026; OQ-1..OQ-4 ghi `OPEN` kèm ca bị ảnh hưởng | Đủ trong PRD §17 và đặc tả §3–§4. |
| AC-02 | Bảng ví dụ phủ đủ danh sách của brief | Đặc tả §6 (E-01..E-37). |
| AC-03 | Vòng đời theo trạng thái, độc giả đọc được khi thống kê lỗi, nhãn đúng độ phủ | Đặc tả §7, §8. Xóa tác phẩm/tài khoản và admin gỡ cả bài chỉ ở mức target. |
| AC-04 | PRD, đặc tả, proposal, registry trỏ đúng owner; bản đồ P/CHOICE | §4 của registry này. Proposal giữ `PENDING` tới lúc có lời duyệt. |
| AC-05 | Trình payload `VDR-WORK-VIEWS@r1` cho chủ sản phẩm; signoff chỉ khi có lời duyệt đúng ID@r1 | Chủ sản phẩm trả lời nguyên văn “Duyệt r1” cho câu hỏi duyệt `VDR-WORK-VIEWS@r1` (sha `cfb789cc…`) trong phiên #340 ngày 21/11/2026. Đã chạy `decision signoff` với đúng SHA-256 và trích lời đó. |

## 7. Điều kiện để bắt đầu việc tiếp theo

- **#341:** PR này đã merge vào `develop`, để mọi lát dùng cùng một revision đặc tả
  (`VDR-WORK-VIEWS@r1`, `PRODUCT_CONFIRMED`). Các P cần cho đường ghi (P-01, P-03..06, P-08, P-11)
  đều trong revision đó. OQ-1..OQ-4 không chặn #341.
- **Lượt thiết kế:** bắt đầu được sau khi PR này merge, dùng dữ liệu mẫu có nhãn. Cần câu trả lời
  OQ-1 trước khi chốt hàng Bàn viết của bài riêng tư.
- **#342, #343:** theo thứ tự ở §4.
