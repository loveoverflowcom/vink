---
name: vink-specify
description: Chuyển quyết định sản phẩm Vink thành đặc tả domain/UX/dữ liệu, cập nhật đúng tài liệu sở hữu và đối soát khi code hoặc yêu cầu thay đổi. Dùng sau khi chốt quyết định, khi viết đặc tả mới, sau khi triển khai làm đổi hiện trạng hoặc khi phát hiện tài liệu mâu thuẫn.
---

# Vink specify

Chuyển các quyết định đã chốt thành đặc tả rõ ràng và giữ cho bộ tài liệu Vink luôn khớp với nhau
cũng như khớp với hiện trạng code. Đọc [AGENTS.md](../../../AGENTS.md),
[bản đồ tài liệu](../../../docs/README.md), [requirements](../../../prd/requirements.md),
[kho quyết định TOML](../../../decisions/README.md),
[registry triển khai và bằng chứng](../../../docs/development/product-decisions-registry.md) và
[mẫu đặc tả / kịch bản đối soát](references/spec-templates.md).

Nếu trong lúc viết hoặc đối soát đặc tả phát hiện câu hỏi sản phẩm chưa có lời giải (`open_product`),
dùng [vink-shape](../vink-shape/SKILL.md) để làm rõ thay vì tự chốt quy tắc sản phẩm.

## 1. Cập nhật đúng nơi sở hữu, không nhân bản nội dung

Mỗi loại thông tin chỉ có một nơi sở hữu chính; các tài liệu khác dẫn link tới đó:

| Loại thông tin | Tài liệu sở hữu |
| --- | --- |
| Định hướng sản phẩm và phạm vi lớn | [`prd/vision.md`](../../../prd/vision.md) |
| Yêu cầu và điều kiện nghiệm thu có ID (`WR`, `PUB`, `AUD`, `MOD`, `COM`, `LVE`, `TAG`, `COL`, `HTG`, `RANK`, `TOP`, `ACC`) | [`prd/requirements.md`](../../../prd/requirements.md) |
| Quyết định đã chuyển từ DecisionDelta §2–4 và §7 (USER, CHOICE theo phạm vi, O, ADM), D-02..D-19, CHOICE theo issue (`LVE-PR-01-CHOICE-*`, `TAG-PR-01-CHOICE-*`, `DISC-*-CHOICE-*`) và revision `VDR-*` | [Kho TOML](../../../decisions/README.md), qua `cargo xtask decision` |
| D-01, INV, S-ID và quyết định chưa chuyển trong ADR hoặc registry phát triển | [Sổ D-ID](../../../docs/architecture/decisions.md) và owner theo chủ đề ([bảng kiểm kê](../../../docs/development/decision-registry-rollout.md#lát-cutover-sổ-dadr-11112026)) |
| Đặc tả chi tiết theo miền | Tài liệu domain trong `docs/architecture/` (`publication-moderation.md`, `comment-editing.md`, `collections.md`, `discovery-ranking.md`, `hashtags.md`, …) |
| Đặc tả giao diện và bố cục | `docs/design/` (theo quy ước [vink-layout-spec](../vink-layout-spec/SKILL.md)) |
| Trạng thái triển khai, ánh xạ model/code và bằng chứng | Các registry trong `docs/development/` (`product-decisions-registry.md`, `*-core.md`, `*-runtime.md`, `feature-map.md`) |
| Nguyên tắc làm việc và điều hướng của agent | [`AGENTS.md`](../../../AGENTS.md) và `.agents/skills/*` (giữ gọn dưới 24 KB cho `AGENTS.md`; không chép lại nhật ký runtime từng PR vào skill) |

### Ghi quyết định đã chuyển

1. Dùng `cargo xtask decision query ID` để lấy trạng thái, revision đang tra cứu, head và SHA.
2. Sau câu trả lời của chủ sản phẩm hoặc khi cần đưa một phương án ra review, soạn payload TOML
   theo [hợp đồng](../../../decisions/README.md). Giữ căn cứ và phạm vi được xác nhận trong
   bàn giao. Dùng `decision propose ID REV:SHA256 INPUT.toml`, rồi `decision diff` để kiểm nội dung.
3. Revision mới ở `PENDING` trong kho. Chỉ chạy `decision signoff ID@rN SHA256 INPUT.toml` khi chủ
   sản phẩm đã duyệt, từ chối hoặc ủy quyền quyết định cho đúng revision đó, trong phiên hiện tại
   hoặc trong brief/issue do chủ sản phẩm viết. Trích nguyên văn lời đó trong `source`. Không suy
   signoff từ việc được giao việc hay từ một review. Thiếu lời đó thì giữ `PENDING` và ghi phần còn
   chờ. Sau signoff, đối chiếu tác động tới PRD/spec/code/evidence và cập nhật marker `active`.
   Khi chỉ thay đổi hiện trạng hoặc bằng chứng, cập nhật registry phát triển. Với quyết định chưa
   chuyển, ghi vào owner Markdown tương ứng.

## 2. Bảy phần cần có của một đặc tả (viết theo độ sâu phù hợp)

Một thay đổi nhỏ chỉ cần vài dòng hoặc một bảng ngắn; một tính năng mới hoặc luồng nhạy cảm (xuất
bản, kiểm duyệt, xóa bài, tài khoản, AI) cần đủ 7 mặt sau (xem chi tiết tại
[spec-templates.md](references/spec-templates.md)):

1. **Mục tiêu và điều kiện nghiệm thu:** gắn ID yêu cầu và hành vi quan sát được (cả thành công lẫn từ chối).
2. **Thuật ngữ domain:** bảng ánh xạ *tên trong sản phẩm ↔ tên trong code/API ↔ khái niệm mới đề xuất*.
3. **Quyền (`Actor × hành động × đối tượng × điều kiện × nguồn quyền`):** tách rõ quyền giả định
   trong model với kiểm tra quyền bền vững tại thời điểm commit trong PostgreSQL.
4. **Trạng thái và chuyển trạng thái:** điều kiện chuyển, version/generation fence, idempotency receipt
   và các trạng thái không được gộp chung.
5. **Trải nghiệm người dùng (UX contract):** hiển thị theo từng trạng thái, copy tiếng Việt, xử lý
   chờ/lỗi/xung đột. *Khi thiết kế đi trước runtime:* bắt buộc ghi rõ (a) đâu là target UX vs hành
   vi đang chạy, (b) đâu là dữ liệu mẫu vs API thật, và (c) các phụ thuộc runtime cần hoàn tất.
6. **Dữ liệu và quyền riêng tư:** dữ liệu lưu trữ, retention/xóa, và đặc biệt với **AI provider**:
   dữ liệu nào được phép gửi, chỉ gửi khi tác giả chủ động gửi duyệt (không gửi khi lưu nháp riêng),
   gửi đúng snapshot đang xét, log những gì (không log nội dung riêng), lưu bao lâu và xử lý khi xóa
   bài. Không suy quyền sử dụng dữ liệu chỉ vì struct/bảng đã có sẵn trường đó.
7. **Triển khai và bằng chứng:** nơi sở hữu trong code và lane kiểm chứng tương ứng.

## 3. Tách bạch bốn chiều trạng thái và bảo toàn lịch sử

Không dùng một nhãn chung gây hiểu nhầm; luôn tách rõ bốn chiều:

- **Quyết định sản phẩm:** `PRODUCT_CONFIRMED`, `DEFERRED`, hoặc `OPEN`.
- **Quyết định kiến trúc:** `DESIGN_PROPOSED`, `Provisional`, hoặc `Accepted`.
- **Mức triển khai:** `NOT_IMPLEMENTED`, `core`, `persistence-api`, `worker-provider`, `web`, `cmp`.
- **Bằng chứng:** lệnh đã chạy (`model`, `unit/kani`, `postgres`, `web`, `cmp`), SHA và giới hạn.

Khi cập nhật tài liệu, phân biệt rõ: (1) **Quy tắc đang áp dụng**, (2) **Hiện trạng triển khai**,
(3) **Lịch sử quyết định** (ghi rõ quyết định mới ngày nào thay thế phần nào của quyết định cũ, giữ
lại phần còn hiệu lực), và (4) **Bằng chứng kiểm thử lịch sử** (giữ nguyên SHA và môi trường của lần
chạy cũ; thêm kết quả mới ở mục riêng).

**Khi code và tài liệu lệch nhau** (ví dụ trước 11/11/2026 PRD ghi mật khẩu `≥ 10 ký tự` nhưng
`Password::parse` kiểm `10–128 byte`): tuyệt đối không tự ý sửa PRD để hợp thức hóa code khi chưa có
xác nhận của chủ sản phẩm; hãy ghi rõ độ lệch tại mục hiện trạng/registry và đưa vào câu hỏi cần chốt
khi làm phần liên quan. Ca này được chốt bằng `O-12@r2` (đếm ký tự sau NFKC) rồi mới sửa code.

## 4. Duy trì đặc tả ở năm thời điểm và khi làm nhiều phiên/PR

1. **Khi bắt đầu việc:** kiểm tra tài liệu liên quan có khớp với nhánh `develop`/base hiện tại không.
2. **Sau khi chủ sản phẩm trả lời:** ghi ID, câu hỏi, lựa chọn, lý do, căn cứ và quyết định bị thay.
   Dùng luồng proposal cho ID đã chuyển. Ghi tình trạng xác nhận, tài liệu sở hữu bị ảnh hưởng và
   các điểm còn mở trong bàn giao.
3. **Sau khi code thay đổi:** cập nhật hiện trạng triển khai, feature map và phần việc còn lại.
4. **Trước khi viết prompt hoặc mở PR:** kiểm tra không có thông tin tự mâu thuẫn ngay trong cùng
   một tài liệu hoặc giữa PRD và registry.
5. **Khi tiếp tục công việc trên nhánh mới:** đối chiếu các quyết định và PR đã merge từ phiên trước.

Khi công việc chia thành nhiều PR hoặc nhiều phiên song song: chỉ định **một** nhánh sở hữu cập nhật
PRD/registry dùng chung hoặc chốt đặc tả chung trước khi tách nhánh để tránh xung đột tài liệu.
