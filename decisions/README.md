# Kho quyết định Vink

[Bundle nhập DecisionDelta](decision-delta-20261110.toml) lưu các quyết định, đề xuất và câu hỏi
của DecisionDelta: §2–4 của lượt 10/11/2026 (`PRODUCT-DECISIONS-20261110`) và §7 quản trị kiểm duyệt
của lượt 11/11/2026 (`ADMIN-MODERATION-20261111`). [Bundle nhập sổ D/ADR](decision-log-20261111.toml)
(`DECISION-LOG-20261111`) lưu phần quyết định của D-02..D-15 và các `CHOICE` theo issue trong ADR
thảo luận, literary endorsement và phân loại. [Bundle nhập lựa chọn phát triển](decision-dev-20261112.toml)
(`DECISION-DEV-20261112`) lưu 27 lựa chọn thiết kế từ các lát phân loại/khám phá (#96–#99, `TAG-PR-02..05`),
đường production Auth (§11, `D-05-C1..C5`), biểu tượng Pen Flight R2 (`LVE-WP-04-CHOICE-07`), đọc công khai
có điều kiện (`READ-01-CHOICE-01..04`, `READ-02-CHOICE-01..04`) và đọc thảo luận trên Web
(`DISC-WEB-CHOICE-01..03`). Mỗi revision sau đó gồm một proposal trong
`proposals/`, và chỉ có hiệu lực khi có signoff của chủ sản phẩm trong `signoffs/`.
[Bảng kiểm kê và bàn giao](../docs/development/decision-registry-rollout.md) ghi phạm vi đã chuyển,
nguồn còn dùng và các điểm còn mở. PRD vẫn sở hữu yêu cầu và tiêu chí nghiệm thu. ADR giữ thiết kế,
lý do và các quyết định ngoài phạm vi nhập. Các registry phát triển ghi mức triển khai và bằng chứng.

[#607](https://github.com/loveoverflowcom/vink/issues/607) xác nhận email bắt buộc khi tạo tài khoản. [VDR-AUTH-VERIFIED-EMAIL@r1](proposals/VDR-AUTH-VERIFIED-EMAIL.r1.toml) là bản ghi chờ signoff đúng revision/hash. O-11@r2 giữ nguyên lịch sử đề xuất recovery.

## Điểm Vink (Reputation V1)

[VDR-REPUTATION@r1](proposals/VDR-REPUTATION.r1.toml) ghi Q01..44 từ review 27–28/11/2026 của [Epic #600](https://github.com/loveoverflowcom/vink/issues/600);
[VDR-REPUTATION-CONTRACT@r1](proposals/VDR-REPUTATION-CONTRACT.r1.toml) ghi hướng backend đề xuất.
Cả hai `PENDING`, chưa signoff exact revision/SHA-256; hướng formula/eligibility/growth/snapshot đã chọn, numeric/technical qualification còn OPEN. [PRD §19](../prd/requirements.md#19-điểm-vink-reputation-v1),
[đặc tả](../docs/architecture/reputation.md), [registry](../docs/development/reputation.md).

## Social Graph

[VDR-DISCOVERY-FOLLOWING-VISIBILITY@r1](proposals/VDR-DISCOVERY-FOLLOWING-VISIBILITY.r1.toml)
ghi delta [#621](https://github.com/loveoverflowcom/vink/issues/621) cho thanh mode Khám phá.
Record chờ signoff đúng revision. [LayoutSpec hiện hành](../docs/design/redesign-r4/layout-spec.md#4-khám-phá)
quy định hình học và lối vào trực tiếp.

[#288](https://github.com/loveoverflowcom/vink/issues/288) đưa R1–R9 của Epic vào [PRD §15](../prd/requirements.md#15-hồ-sơ-tác-giả-và-theo-dõi-social-graph).
[Proposal sản phẩm](proposals/VDR-SOCIAL-GRAPH.r1.toml) ghi phạm vi và các SOC-OPEN.
[Proposal kiến trúc](proposals/VDR-SOCIAL-CONTRACT.r1.toml) ghi ownership và CHOICE-01..06.
[USER-01@r2](proposals/USER-01.r2.toml) chỉ đề xuất mở trường ngày sinh tự nguyện ở Profile.
Cả ba revision đang `PENDING`. Lệnh `cargo xtask decision query USER-01 --history` cho thấy
`USER-01@r1 DEFERRED` vẫn có hiệu lực. [Hợp đồng](../docs/architecture/social-graph.md)
ghi nguồn #290/#291 cho hai lựa chọn đã chốt trong phạm vi issue. Duyệt preview và merge tài liệu
không thay signoff đúng revision/hash theo [VDR-DECISION-SIGNOFF@r1](signoffs/VDR-DECISION-SIGNOFF.r1.toml).

[VDR-SOCIAL-GRAPH@r2](proposals/VDR-SOCIAL-GRAPH.r2.toml) bổ sung delta Web [#480](https://github.com/loveoverflowcom/vink/issues/480).
Thiết lập và đổi địa chỉ trong Chỉnh hồ sơ. Revision r2 còn `PENDING` theo cùng quy trình
signoff. [LayoutSpec delta](../docs/design/profile-inline-handle-480.md).

## Thông báo (Notifications v1)

[#345](https://github.com/loveoverflowcom/vink/issues/345) đưa R1–R3 của [Epic #305](https://github.com/loveoverflowcom/vink/issues/305)
vào [PRD §16](../prd/requirements.md#16-thông-báo-cộng-đồng-và-danh-sách-thấm-công-khai-notifications-v1),
[đặc tả miền](../docs/architecture/notifications.md),
[hợp đồng liên dịch vụ/Gateway](../services/vink-app/contracts/http/v1/notifications.md) và
[bảng truy vết](../docs/development/notifications.md).
[VDR-NOTIFICATIONS@r1](proposals/VDR-NOTIFICATIONS.r1.toml) tóm tắt phạm vi `N1–N8` và `P-01..P-14` (chỉ Thấm + danh sách công khai có phân trang; Hữu ích loại khỏi Epic #305).
[VDR-NOTIFICATIONS-CONTRACT@r1](proposals/VDR-NOTIFICATIONS-CONTRACT.r1.toml) ghi hướng kiến trúc `K1–K6` và phân giao `T-01..T-08` cho `#346`–`#348`.
[VDR-NOTIFICATIONS@r2](proposals/VDR-NOTIFICATIONS.r2.toml) và
[VDR-NOTIFICATIONS-CONTRACT@r2](proposals/VDR-NOTIFICATIONS-CONTRACT.r2.toml) ghi delta `#365`
(trích đoạn nhận xét và thông báo Ưng) cùng lựa chọn kỹ thuật `#348`; cả hai revision giữ `PENDING`.
[LVE-PR-02-03-CHOICE-12@r2](proposals/LVE-PR-02-03-CHOICE-12.r2.toml) đề xuất mở danh sách người Thấm công khai có phân trang (`WorkId + RevisionId`), giữ nguyên phần bình luận của `LVE-PR-02-03-CHOICE-12@r1` (`DESIGN_PROPOSED`, vẫn đang hiện hành khi `@r2` còn `PENDING`).
Cả ba proposal mới đang ở trạng thái `PENDING` và chờ signoff đúng revision/SHA-256 theo [VDR-DECISION-SIGNOFF@r1](signoffs/VDR-DECISION-SIGNOFF.r1.toml).

[VDR-NOTIFICATIONS-DESIGN@r1](proposals/VDR-NOTIFICATIONS-DESIGN.r1.toml) đề xuất badge đỏ
và đặt chuông dưới Bàn viết theo [LayoutSpec R4](../docs/design/notifications.md).
Revision này `PENDING` chờ signoff đúng revision/SHA-256. Phần mobile của r1 ghi thiết kế trước
[#608](https://github.com/loveoverflowcom/vink/issues/608).
[VDR-NOTIFICATIONS-MOBILE-NAV@r1](proposals/VDR-NOTIFICATIONS-MOBILE-NAV.r1.toml) ghi lời chốt #608
về bốn đích mobile và phạm vi thay thế. Bản ghi này chờ signoff revision/SHA-256 riêng với đề xuất
màu badge và vị trí rail của R4.

## Nhận xét R4

[VDR-DISCUSSION-R4@r1](proposals/VDR-DISCUSSION-R4.r1.toml) ghi bốn lựa chọn chủ sản phẩm chọn trong
phiên #359/#360 ngày 20/11/2026: vùng “Bạn đã gửi”, xác nhận gỡ tại chỗ, phản hồi chờ duyệt hiện cạnh
ô soạn (không hàng tạm trong nhánh) và câu “Bạn đã tắt bình luận…” theo `viewer_is_work_author`.
Chủ sản phẩm đã signoff đúng revision/SHA-256 ngày 20/11/2026
([signoff](signoffs/VDR-DISCUSSION-R4.r1.toml)); revision hiện hành là `PRODUCT_CONFIRMED`. Đổi nhãn “Hữu ích” → “Ưng” là delta thuật
ngữ trong [literary endorsement](../docs/architecture/literary-endorsement.md#delta-thuật-ngữ-20112026--359),
không có revision TOML riêng (như #213).

## Lưu bài để đọc lại

[VDR-SAVED-WORKS@r1](proposals/VDR-SAVED-WORKS.r1.toml) ghi SAV-01..04, D1–D12 và vòng đời từ [Epic #384 §8](https://github.com/loveoverflowcom/vink/issues/384). Revision còn `PENDING`. [Registry Lưu bài](../docs/development/saved-works.md) ghi hash và phạm vi signoff còn chờ.

## Mock evaluator bootstrap

[VDR-WORK-MOCK-EVALUATOR-BOOTSTRAP@r1](proposals/VDR-WORK-MOCK-EVALUATOR-BOOTSTRAP.r1.toml) ghi
ngoại lệ bootstrap của MOD-06 theo lời chủ sản phẩm trong [#536](https://github.com/loveoverflowcom/vink/issues/536)
ngày 25/11/2026. Mock kịch bản `approve` duyệt mặc định cho tác phẩm ở mọi môi trường, kể cả
production. Operator có opt-out `manual`, và admin dừng tự động thắng cấu hình. Revision còn
`PENDING`. [Vận hành và provenance](../docs/development/moderation-runtime.md#31-mock-bootstrap-và-công-tắc-vận-hành-536).

[VDR-WORK-MOCK-EVALUATOR-BOOTSTRAP@r2](proposals/VDR-WORK-MOCK-EVALUATOR-BOOTSTRAP.r2.toml) ghi M01–M09 của [#639](https://github.com/loveoverflowcom/vink/issues/639). Revision này chờ signoff riêng. #638 triển khai M01/M02/M03/M07.

## Bàn viết

[VDR-AUTHOR-WORK-FEEDBACK@r2](proposals/VDR-AUTHOR-WORK-FEEDBACK.r2.toml) ghi cách chiếu ngữ cảnh
kiểm duyệt cho [#614](https://github.com/loveoverflowcom/vink/issues/614), trạng thái `PENDING`.
[LayoutSpec Desk R5](../docs/design/redesign-r4/layout-spec.md#54-desk-r5-614) sở hữu bố cục.

## Lượt xem tác phẩm

[VDR-WORK-VIEWS@r1](proposals/VDR-WORK-VIEWS.r1.toml) ghi P-01..P-11 của
[Epic #323](https://github.com/loveoverflowcom/vink/issues/323) theo lời chốt ngày 21/11/2026
trong brief [#340](https://github.com/loveoverflowcom/vink/issues/340): định nghĩa một lượt, chống
trùng 24 giờ, khách, tác giả, cộng dồn theo `WorkId`, chỉ tác giả thấy số, lưu giữ và xóa. Chủ sản
phẩm đã signoff đúng revision/SHA-256 ngày 21/11/2026 ([signoff](signoffs/VDR-WORK-VIEWS.r1.toml));
revision này giữ làm lịch sử đã ký và vẫn hiện hành cho marker `<!-- vdr: active VDR-WORK-VIEWS@r1 -->`
cho tới khi revision kế tiếp được signoff.

[VDR-WORK-VIEWS@r2](proposals/VDR-WORK-VIEWS.r2.toml) (`PENDING`, chờ signoff đúng revision/SHA-256
theo [VDR-DECISION-SIGNOFF@r1](signoffs/VDR-DECISION-SIGNOFF.r1.toml)) ghi nhận các quyết định
delta ngày 25/11/2026 từ [#323](https://github.com/loveoverflowcom/vink/issues/323) và `#340`:
`VIEW-01/P-01` thêm ngưỡng hiển thị foreground tối thiểu cấu hình chung (mặc định `5.000 ms`, tách một
nguồn cấu hình/constant dùng chung), chốt `OQ-1 = A`, `OQ-2` (kích hoạt khi có route xóa),
`OQ-3A/B`, `OQ-4`, bộ luật `UX-01..10`, giữ số liệu riêng tư và hai quyết định Bàn viết `D-01`
(thứ tự **Thấm → Nhận xét → Lượt xem**, Lượt xem để cuối) và `D-02` (bỏ hẳn nút/link “Đọc” / “Đọc
bản công khai”, giữ link mở bản thảo/editor). Yêu cầu ở
[PRD §17](../prd/requirements.md#17-lượt-xem-tác-phẩm-work-views), luật và ví dụ ở
[đặc tả](../docs/architecture/work-views.md), mức triển khai và lựa chọn kỹ thuật ở
[registry](../docs/development/work-views.md).

## Nền tảng Tailwind CSS v4 cho Web

[VDR-WEB-TAILWIND-FOUNDATION@r1](proposals/VDR-WEB-TAILWIND-FOUNDATION.r1.toml) ghi bốn lựa chọn
kỹ thuật của [#417](https://github.com/loveoverflowcom/vink/issues/417) (`CHOICE-01 B` compiler pin
`4.3.5` trong `packages/ui-system/leptos/tailwind`, `CHOICE-02 A` không Preflight + `@layer` +
`source(none)` allowlist, `CHOICE-03 A` sinh `tailwind-theme.css` từ `cargo xtask ui-tokens`,
`CHOICE-04 B` xóa bảng màu/chữ mặc định + mở rộng guard `ui_qualification.rs` cho Rust UI và CSS giữ lại). Phạm vi triển khai hiện tại gồm 5 thư mục Rust UI,
4 stylesheet giữ lại và 2 entry CSS. Payload r1 giữ phạm vi ghi trong đề xuất ban đầu.
Proposal này ở trạng thái `PENDING` (chưa có signoff đúng revision/SHA-256). Chi tiết baseline,
inventory, pilot và bằng chứng nằm tại
[docs/development/tailwind-web-foundation.md](../docs/development/tailwind-web-foundation.md).

## Ranh giới Web và Preview

[VDR-WEB-PREVIEW-BOUNDARY@r1](proposals/VDR-WEB-PREVIEW-BOUNDARY.r1.toml) (`PENDING`)
ghi #479. [Owner và kiểm](../docs/development/web-preview-boundary.md).

## Giao diện Web (Shell & Cỡ chữ)

[VDR-WEB-APPEARANCE@r1](proposals/VDR-WEB-APPEARANCE.r1.toml) (`PENDING`, SHA-256
`e49d51487417906adb3e866e15cf63cd61a647942c534aafacb8928c1d7f7570`) ghi nhận quyết định ngày
23/11/2026 trong [issue #413](https://github.com/loveoverflowcom/vink/issues/413): bỏ nút
“Giao diện” và dialog `#profile-menu` khỏi mọi nav rail Web, giữ `/toi` làm điểm chỉnh Giao diện
duy nhất trên Web, rút nhóm Cỡ chữ thành hai lựa chọn `Mặc định` (100%) và `Lớn` (150%, tự quy
đổi giá trị cũ `vink.ui.text=200` sang `150` trong bootstrap trước CSS/first paint, theo lựa chọn kỹ
thuật `CHOICE-01 A`), và chỉ thay thế có phạm vi phần giữ nút Giao diện trên rail của
`#403`/`DQ-11` và `#209`. Theo `VDR-DECISION-SIGNOFF@r1`, proposal này giữ trạng thái `PENDING`
trong registry TOML và chưa gắn marker pin kích hoạt cho tới khi có file signoff trích đúng
revision/SHA-256.

[VDR-WEB-APPEARANCE@r2](proposals/VDR-WEB-APPEARANCE.r2.toml) (`PENDING`) ghi delta
[#576](https://github.com/loveoverflowcom/vink/issues/576): chỉ thành viên có phiên Active
đã xác nhận mới chỉnh Giao diện tại Tôi. Màu hệ thống, chữ 100% và rail theo ngữ cảnh được
dùng trước xác nhận phiên và khi mất xác nhận. [LayoutSpec Giao diện](../docs/design/me-preferences-r1-209.md)
ghi cách lưu trên thiết bị và kiểm chứng.

## Editor R4

[VDR-EDITOR-R4@r1](proposals/VDR-EDITOR-R4.r1.toml) ghi delta ngày 24/11 trong
[#475](https://github.com/loveoverflowcom/vink/issues/475) về việc bỏ hộp thoại xem trước bản nháp.
Proposal còn `PENDING` đến khi có signoff đúng revision theo
[VDR-DECISION-SIGNOFF@r1](signoffs/VDR-DECISION-SIGNOFF.r1.toml).
[VDR-EDITOR-R4@r2](proposals/VDR-EDITOR-R4.r2.toml) bổ sung focus cục bộ trên mặt viết theo
[#526](https://github.com/loveoverflowcom/vink/issues/526), trạng thái `PENDING`.
[LayoutSpec delta R4](../docs/design/editor-r4.md) ánh xạ hành vi vào Web.

## Nén asset Web production

[VDR-EDGE-ASSET-COMPRESSION@r1](proposals/VDR-EDGE-ASSET-COMPRESSION.r1.toml) ghi `CHOICE-01 A` của
[#487](https://github.com/loveoverflowcom/vink/issues/487): Worker production bỏ `no-transform` cho
asset `/_v/<sha>/pkg/` nén được để zone Cloudflare nén Brotli/gzip, giữ `no-transform` cho font,
ảnh và READ-01. Tiền nén lúc build (B) chuyển sang #491. Proposal đang `PENDING`. Số đo và giới hạn
nằm ở [evidence](../docs/development/evidence/web-compress-487/README.md).

## Frontend Web trên Cloudflare

[VDR-EDGE-FRONTEND@r2](proposals/VDR-EDGE-FRONTEND.r2.toml) đề xuất SPA Leptos CSR độc lập trên Pages, API cùng origin trên VPS và metadata chung trước JavaScript theo hướng mới của [#504](https://github.com/loveoverflowcom/vink/issues/504). Revision r2 đang `PENDING`. [Revision r1](proposals/VDR-EDGE-FRONTEND.r1.toml) lưu phương án SSR ban đầu.
[ADR](../docs/architecture/edge-frontend.md) · [route và bằng chứng](../docs/development/edge-frontend.md).

## Xóa tài khoản

[VDR-ACCOUNT-DELETION@r1](proposals/VDR-ACCOUNT-DELETION.r1.toml) ghi nhận DEL-01–08 từ
[#297](https://github.com/loveoverflowcom/vink/issues/297). Quy tắc nghiệp vụ đã chốt ngày
26/11/2026. Revision của registry đang `PENDING` theo [quy trình signoff](signoffs/VDR-DECISION-SIGNOFF.r1.toml).
[Đặc tả vòng đời](../docs/architecture/account-deletion.md).

## Tra cứu theo phạm vi công việc

[Đối soát PR 15–16/11/2026](../docs/development/verification-reconciliation-20261116.md#quyết-định-toml-mới)
ghi 7 proposal kiến trúc mới cho read lifecycle, storage, paging, author feedback, save projection
và action surfaces. Tất cả **PENDING**; implementation đã merge không tự cấp signoff.

```bash
cargo xtask decision --help
cargo xtask decision query --domain publication
cargo xtask decision query O-05
cargo xtask decision query USER-08 --history
cargo xtask decision query USER-08@r1
cargo xtask decision query D-05
cargo xtask decision query D-07
cargo xtask decision query LVE-PR-01-CHOICE-02
cargo xtask decision query ADMIN-MODERATION-20261111
cargo xtask decision query DECISION-LOG-20261111
cargo xtask decision query DECISION-DEV-20261112
cargo xtask decision diff O-12@r1 O-12@r2
cargo xtask decision refs
cargo xtask decision validate
```

- `query ID` trả `current` cho revision đang hiệu lực và `head` cho revision mới nhất của luồng ghi.
  Revision hiện hành là revision mới nhất trong các revision nhập chưa bị thay hoặc proposal đã có
  signoff khác `REJECTED`. Số revision lớn hơn không tự thắng. Đọc `record.status` trước khi dùng
  nội dung: `OPEN` là câu hỏi, `DEFERRED` ghi việc hoãn, `DESIGN_PROPOSED` là phương án để review,
  còn `PRODUCT_CONFIRMED` là hướng sản phẩm đã chốt. Proposal chưa có signoff mang `PENDING`.
- `query ID@rN` và `--history` đọc được revision lịch sử, pending hoặc bị từ chối. `pending_only`
  nghĩa là ID chưa có revision hiệu lực và head đang chờ signoff. `no_current` nghĩa là signoff cuối
  đã từ chối. `group` liệt kê mọi revision một lượt nhập đã chứa, kể cả revision bị thay sau đó.
  `related_only` liệt kê các record hiện hành nhắc đến D-ID hay requirement ID. D-ID đã chuyển
  (`D-05`) trả `current`. D-ID mà record DecisionDelta sở hữu trọn vẹn (`historical_alias`) trả
  `related_only`. Ví dụ `D-07` trả cả `USER-03` và `USER-05`. `D-02` trả hai phần có trạng thái riêng
  là `D-02-CONTENT` và `D-02-DISCUSSION`. Requirement ID vẫn tra chính ở PRD. `unknown` là kết quả
  tra cứu hợp lệ và cho biết kho chưa có ID hoặc revision đó. Ví dụ `D-01` vẫn tra ở sổ D-ID.
- `CHOICE` theo issue luôn mang phạm vi lát trong ID, như `LVE-PR-01-CHOICE-02`, `TAG-PR-01-CHOICE-01`,
  `DISC-01-CHOICE-01` hay `DISC-04-CHOICE-02`. `query CHOICE-01` trả `unknown`.
- Khi bàn giao, lưu Git SHA, `snapshot_sha256`, `ID@rN` và `source` từ kết quả. `query` không
  đối số xuất toàn bộ revision dưới dạng JSON. Dữ liệu xuất được dựng lại mỗi lần từ TOML.
- Khi CLI lỗi hoặc không chạy được, nêu nguồn và SHA của dữ liệu đã đọc cùng phần chưa xác minh.
  [DecisionDelta tại commit nhập](https://github.com/loveoverflowcom/vink/blob/af3c34bd624be18d404bd5789620759cae0b0ddb/docs/architecture/product-decisions-20261110.md),
  [sổ D-ID tại commit nhập](https://github.com/loveoverflowcom/vink/blob/73ab92f0064674833ec31bc7533b210149b26b79/docs/architecture/decisions.md)
  và các lát phát triển tại commit nhập `630e80a9a1b2790eb4f3c663afb7ad627163d3cb`
  là bản đối chiếu lịch sử. Record nhập từ sổ D/ADR/lát phát triển ghi `source.path` và dòng nguồn tại commit
  nhập. Khi phạm vi hiện hành chưa xác định được, ghi `UNKNOWN` và tra owner của miền. Query và validate chạy được ở checkout nông. Lệnh migrate cần Git object của nguồn.

## Đề xuất một revision

Soạn input TOML tạm với các trường dưới đây. Mọi luận cứ cần lưu phải nằm trong trường dữ liệu.
Giữ nội dung riêng, thông tin xác thực và transcript ngoài record.

```toml
kind = "product"
domain = "publication"
title = "Quyền ghi trên trang thay thế"
resolution = """
Ghi phương án đề xuất và phạm vi áp dụng ở đây.
"""
rationale = "Ghi căn cứ và đánh đổi cần chủ sản phẩm xem xét."
requires = ["USER-07@r1"]
related_ids = ["PUB-04"]
```

```bash
# ID mới thuộc namespace VDR-*
cargo xtask decision propose VDR-EXAMPLE new /tmp/decision-proposal.toml

# Bổ sung revision cho ID có sẵn
cargo xtask decision query O-03
cargo xtask decision propose O-03 'REV:SHA256' /tmp/decision-proposal.toml
cargo xtask decision diff O-03@r2 O-03@r3
```

Thay `REV:SHA256` bằng `head.revision:head.sha256` vừa tra. Nếu đã có thêm proposal,
so sánh với revision mới nhất tương ứng. Với yêu cầu cùng ID, giữ `kind` và `domain` của
revision đầu luồng hiện hành. ID chỉ dùng chữ ASCII viết hoa, chữ số và dấu gạch ngang,
tối đa 80 ký tự. Domain dùng tên kebab-case chữ thường, tối đa 48 ký tự.
`kind` nhận `product` hoặc `architecture`. `requires` là danh sách tùy chọn, mỗi mục ghim
`ID@rN` đang tồn tại. `related_ids` là danh sách tùy chọn các requirement ID hoặc D-ID để truy vết.
Revision nhập mang sẵn liên kết này. Khi viết revision mới cho một ID có sẵn, chép lại liên kết
còn đúng để `query ACC-03` vẫn thấy record. Có thể khai báo tùy chọn
`work_audiences = ["private", "public"]` cho quyết định sản phẩm miền `publication`. CLI đối chiếu
đúng trường này với `USER-02`. Các mô tả bằng văn xuôi và quan hệ nghiệp vụ giữa các miền cần đọc
và review.

Lệnh ghi tạo file mới trong `decisions/proposals/` với trạng thái `PENDING`. Nếu mất kết quả lệnh
sau khi ghi, tra lịch sử để nhận diện revision đã xuất hiện. Khi base cũ, đọc head mới và đối
chiếu thay đổi trước khi thử lại. File được ghi và đồng bộ trước khi công bố bằng hard link
cùng thư mục. Hệ thống file cần hỗ trợ hard link. Cùng base/cùng ID chỉ một lệnh ghi thành công.
Trên hai worktree, Git có thể báo xung đột thêm cùng tên file. So sánh payload rồi tạo lại proposal
thứ hai từ head đã hợp nhất. Nếu tiến trình bị dừng đột ngột, có thể còn file `.tmp-*`.
Xóa file tạm sau khi xác nhận tiến trình ghi đã dừng.

## Ghi signoff của chủ sản phẩm

Theo [`VDR-DECISION-SIGNOFF@r1`](signoffs/VDR-DECISION-SIGNOFF.r1.toml), một proposal chỉ có hiệu
lực khi chủ sản phẩm duyệt, từ chối hoặc ủy quyền quyết định cho đúng revision đó. Agent chạy
`signoff` khi lời đó có trong phiên làm việc hiện tại hoặc trong brief/issue do chủ sản phẩm viết.
Không suy signoff từ việc chủ sản phẩm giao việc, từ một bản review hay từ lần merge trước đó.

```toml
status = "PRODUCT_CONFIRMED"
decided_on = "2026-11-11"
source = """
Trích nguyên văn lời của chủ sản phẩm, nơi nói (phiên, issue, brief) và phạm vi được quyết.
"""
```

```bash
cargo xtask decision query O-12@r2          # lấy sha256 của đúng payload đã review
cargo xtask decision signoff O-12@r2 SHA256 /tmp/decision-signoff.toml
```

- Quyết định sản phẩm nhận `PRODUCT_CONFIRMED`, `DEFERRED` hoặc `OPEN`. Quyết định kiến trúc nhận
  `DESIGN_PROPOSED`, `PROVISIONAL` hoặc `ACCEPTED`. Cả hai loại đều nhận `REJECTED`. Revision bị
  từ chối giữ lại lời từ chối và không bao giờ thành hiện hành.
- `decided_on` là chuỗi `YYYY-MM-DD`. TOML datetime nằm ngoài hợp đồng v1. Signoff không có
  trường lạ, và mỗi revision chỉ có một signoff.
- Signoff gắn với `ID@rN` và SHA-256 của payload. Khi payload proposal bị đổi, `validate` báo
  signoff lệch và revision phải được review lại. Không sửa signoff đã ghi. Muốn đổi kết quả, viết
  revision mới rồi signoff revision đó.
- Revision nhập giữ trạng thái của nguồn. Không thể signoff revision nhập. Muốn đổi luật đã nhập,
  viết proposal mới.

## Định dạng và nguồn phê duyệt

- **Format v1.** TOML qua crate `toml` được khóa trong Cargo.lock, với kiểu Rust/Serde và kiểm tra
  ngữ nghĩa. Proposal gồm `version`, ID/revision, payload `content` và `base` khi có. Signoff gồm
  `version`, ID/revision, `sha256`, `status`, `decided_on` và `source`. Legacy có cấu trúc bundle
  riêng, kèm nhóm nhập, SHA, path và dòng nguồn. Field lạ, duplicate key/item, kiểu sai hoặc version
  khác v1 làm CLI dừng. Các field văn bản nhận Unicode và xuống dòng LF. Datetime, float,
  `inf`/`nan` và `null` nằm ngoài hợp đồng v1.
- **Tính ổn định.** Writer dùng thứ tự field của Rust và chuẩn hóa thứ tự các danh sách dạng tập.
  SHA-256 của proposal tính trên TOML render từ dữ liệu đã validate. Snapshot băm version, digest
  hai bundle nhập, các digest revision theo thứ tự ID và revision, rồi digest signoff. Comment trong
  input là ghi chú tạm. Khi thay đổi serializer hoặc format, phải thử lại roundtrip, đối chiếu hash
  cũ và thêm đường nâng phiên bản nếu output đổi.
- **Lịch sử và phê duyệt.** Mỗi bundle nhập có pin SHA-256 độc lập trong xtask, đối chiếu được
  bằng `migrate --dry-run [GROUP]`. SHA của proposal dùng cho so sánh payload, chống ghi đè cùng
  base và gắn signoff. Các field tự khai như `approval`, `authority` và `status` bị từ chối trong proposal.
- **Ranh giới tin cậy.** Signoff là chứng thực, không phải xác thực. CLI và `verify quick` trên CI
  kiểm payload khớp hash, signoff không trùng và outcome hợp lệ theo loại quyết định. Chúng không
  biết ai đã viết file. Trust anchor là lời trích của chủ sản phẩm cùng lần merge vào `develop` do
  chủ sản phẩm thực hiện hoặc yêu cầu. Người có quyền ghi repo vẫn có thể tự viết signoff. Lịch sử Git
  và review PR là nơi phát hiện. Branch protection, GitHub App và chữ ký chưa được bật. Quyền ghi
  local, Git merge và quyền phê duyệt sản phẩm là các quyền riêng. Kiểm thử hiện chứng minh hành vi
  khi có lỗi I/O và ghi cạnh tranh trên hệ thống file local. Độ bền qua mất điện chưa được kiểm chứng.

## Ghim tham chiếu

Đặt marker riêng một dòng trong Markdown thuộc `prd/`, `docs/` hoặc `.agents/skills/`
khi consumer cần ghim một revision cụ thể.

```text
<!-- vdr: active O-12@r2 -->
<!-- vdr: historical USER-08@r1 -->
```

`active` phải trỏ đúng revision hiện hành của ID. `historical` được giữ ở revision cũ có nguồn.
Proposal pending hoặc bị từ chối không thể làm nguồn cho marker. Lệnh `refs` chỉ báo phạm vi các
marker đã khai báo. Dùng `diff` rồi đối chiếu PRD, spec, code và test có liên quan khi cập nhật pin.
Các skill tổng quát dùng tra cứu theo miền ở đầu công việc để tránh ghim một phiên bản cố định ở
nhiều nơi.

## Dựng lại import

```bash
cargo xtask decision migrate --dry-run > /tmp/vink-decision-delta.toml
cmp /tmp/vink-decision-delta.toml decisions/decision-delta-20261110.toml
cargo xtask decision migrate --dry-run DECISION-LOG-20261111 > /tmp/vink-decision-log.toml
cmp /tmp/vink-decision-log.toml decisions/decision-log-20261111.toml
cargo xtask decision migrate
cargo xtask decision migrate DECISION-LOG-20261111
```

Dry-run đọc đúng Git object đã ghim và in kết quả ra stdout. DecisionDelta đọc `af3c34b`, sau khi
#165 thêm §7; §2–4 giống hệt bản `0c5bb27` của lần nhập đầu. Sổ D/ADR đọc `73ab92f`, sau khi #166
merge kho TOML. Lệnh migrate cài bundle nếu thiếu hoặc báo file hiện tại khớp. Lỗi khác biệt yêu cầu
đối chiếu nguồn và pin trước khi sửa.

Importer sổ D/ADR (`xtask/src/decision/decision_log.rs`) chỉ chép trạng thái nguồn đã ghi tại commit
ghim. Mỗi câu của record là một ô bảng hoặc một trích dẫn mà `render` phải tìm thấy nguyên văn trong
khối nguồn đã khai báo. Trạng thái được trích trong `context.source_status`. Importer không nhận
`ACCEPTED` và không ghép trạng thái sai loại quyết định. Tiêu đề của `CHOICE` theo issue chỉ là nhãn.
Importer không là đường ghi quyết định: luật mới hoặc thay đổi nghĩa sau lượt 11/11 đi qua proposal
và signoff. Muốn nhập thêm một nguồn Markdown, cần một pin riêng, trích nguyên văn và review riêng.
