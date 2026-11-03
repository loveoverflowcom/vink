# Vink — Việt Ink

## Bộ định hướng sản phẩm và công nghệ

**Phiên bản:** 0.2 · **Ngày:** 30/10/2026 (UTC+07) · **Trạng thái:** Đề xuất để review, chưa phải ADR đã được phê duyệt hay hệ thống đã triển khai.

**Vink giúp người viết Việt duy trì việc viết, tìm được người đọc phù hợp và tiến bộ qua phản hồi tử tế.** Trải nghiệm ưu tiên văn bản, tiếng Việt và cộng đồng có chủ đích; thiết kế trẻ trung theo Material 3 Expressive, đồng thời đủ yên tĩnh để đọc dài.

Hướng kiến trúc: Vink sở hữu tác phẩm, vòng đời xuất bản **và thảo luận** trong PostgreSQL. Ngày 03/11/2026 chủ sản phẩm chốt bình luận hai tầng do Vink tự xây bằng Rust core ([ADR ownership](architecture/discussion-ownership.md)); Discourse chỉ là nguồn tham khảo. Leptos SSR và Axum phục vụ giao diện riêng; một worker xử lý kiểm duyệt nền. Các phần khác cần vượt qua thử nghiệm trong tài liệu quyết định trước khi chốt.

## Đọc theo thứ tự nào

| File | Nội dung | Câu hỏi được giải quyết |
| --- | --- | --- |
| [vision.md](../prd/vision.md) | Định vị, người dùng, Bàn viết, Xưởng Viết, Tạp Chí, AI và phát triển cộng đồng | Vink tồn tại để làm gì và trải nghiệm nào đáng đầu tư? |
| [tech-stack.md](architecture/tech-stack.md) | So sánh A/B/C, bài học Cantos, stack đề xuất, ownership, editor, identity, vận hành | Chọn công nghệ nào, vì sao và khi nào đổi hướng? |
| [requirements.md](../prd/requirements.md) | Phạm vi MVP, yêu cầu có ID, nghiệm thu và yêu cầu chất lượng | Một bản đầu tiên đủ dùng phải làm được gì? |
| [decisions.md](architecture/decisions.md) | Quyết định mở, vòng đời revision/publication, thử nghiệm kỹ thuật và lộ trình theo bằng chứng | Cần chốt gì trước khi triển khai lớn? |
| [sources.md](references/sources.md) | Nguồn Cantos theo commit, tài liệu chính thức và các nhận định đã hiệu chỉnh | Đâu là bằng chứng, đâu là đề xuất? |

## Cách hiểu trạng thái trong bộ tài liệu

- **Ràng buộc đầu vào:** Vink độc lập với Cantos; PostgreSQL cho nghiệp vụ Vink, không dùng Turso hay database nghiệp vụ chung; sản phẩm văn học tiếng Việt, giao diện riêng theo định hướng M3 Expressive.
- **Ưu tiên từ yêu cầu mới:** AI hỗ trợ kiểm duyệt ngay từ MVP. Mức tự động hóa tăng sau khi có kết quả đánh giá và khả năng vận hành phù hợp.
- **Đã chốt 03/11/2026:** Vink sở hữu bình luận (module trong app, PostgreSQL); tác giả bật/tắt bình luận theo tác phẩm, không sửa/xóa bình luận người khác.
- **Khuyến nghị:** Vink sở hữu nội dung văn học, Rust modular monolith, một nguồn định danh Vink ổn định.
- **Chưa chốt:** Nhà cung cấp xác thực, authority và màn tiền kiểm bình luận, nhóm tuổi phục vụ, thời điểm mở nhiều chương và ngân sách hiệu suất/AI.
- **Đã đối chiếu nguồn:** Cantos `develop` tại commit `a8e2b6f40b58e0e72a2bdab2d4f44240cb6a0330`; tài liệu chính thức Discourse và các công nghệ tham chiếu ngày 30/10/2026. Đây là bằng chứng về nguồn được đọc, không xác nhận trạng thái production.

## Những thay đổi chính so với vision ban đầu

1. Bổ sung **Bàn viết riêng**; phân biệt thể loại nhật ký với quyền riêng tư.
2. Tách **kiểm duyệt chính sách**, **xuất bản theo ý định tác giả** và **tuyển chọn biên tập**.
3. Đưa AI hỗ trợ vào MVP, đồng thời giữ hàng đợi thủ công, giải thích và khiếu nại.
4. Khuyến nghị Vink sở hữu Work/Chapter/Revision/Publication; thảo luận giữ định danh ổn định qua nhiều lần sửa.
5. Ghi rõ chi phí của Hybrid: hai hệ sinh thái, tích hợp xác thực, đồng bộ và màn quản trị tác phẩm riêng (03/11/2026: thay bằng thảo luận do Vink sở hữu).
6. Hiệu chỉnh các giả định: Horizon là theme; Cantos Web hiện dùng CSR; Quint có giới hạn phạm vi bằng chứng; webhook không tự bảo đảm tiền kiểm.

## Bước triển khai đầu tiên được đề xuất

Chốt nhóm beta và phạm vi tác phẩm; thử một luồng xuyên suốt **soạn tiếng Việt → lưu nháp → gửi duyệt → xuất bản đúng phiên bản → bình luận hai tầng có tiền kiểm → sửa bản mới trong khi bản cũ vẫn đọc được**. Chỉ sau khi luồng này vượt qua các tình huống lỗi quan trọng mới mở rộng tính năng.

Bộ đề xuất v0.2 đã được đưa vào repository Vink ngày 30/10/2026. Việc nhập tài liệu không phê duyệt các đề xuất và không xác nhận đã triển khai hay kiểm chứng runtime Vink.

## Bản đồ tài liệu trong repository

| Nhóm | Vị trí | Cách sử dụng |
| --- | --- | --- |
| Sản phẩm | [Vision](../prd/vision.md), [Requirements](../prd/requirements.md) | Định vị, phạm vi và acceptance có ID; các mục vẫn giữ trạng thái đề xuất |
| Kiến trúc | [Tech stack](architecture/tech-stack.md), [Decision log](architecture/decisions.md), [Ranh giới PostgreSQL/Identity](architecture/identity-postgres-boundaries.md) | So sánh phương án, quyết định mở, bất biến và S-01–S-05; ranh giới database/role theo owner do người dùng yêu cầu; chưa phải ADR Accepted |
| Bằng chứng nguồn | [Sources](references/sources.md) | Nguồn nghiên cứu của foundation, có commit và giới hạn bằng chứng |
| Phát triển | [Development](development/README.md), [API v1](../services/vink-app/contracts/http/v1/README.md) | Setup, lệnh `cargo xtask`, cấu trúc code, bằng chứng và giới hạn skeleton; contract do server sở hữu |
| Discussion hai tầng | [Core/registry](development/discussion-core.md), [ownership đã chốt](architecture/discussion-ownership.md), [LayoutSpec UI](design/discussion-ui.md) | Core/lifecycle + Quint replay, PostgreSQL/API và Web reader hai tầng; ghi cục bộ qua LocalFixture, production auth và Google assets còn thiếu |
| Thiết kế | [Baseline thiết kế](design/README.md) | Palette Lavender, font tự host, thứ tự điều hướng, trạng thái logo và tương phản đã đo; owner giá trị là `packages/ui-system/shared/tokens.json` ([UI system](../packages/ui-system/README.md)), component web ở `vink.css`; [trạng thái và phản hồi](design/state-and-feedback.md) (UIS-03); [shell và điều hướng](../packages/ui-system/shared/navigation.md) (UIS-04) |
| Bản đồ tính năng | [Feature map](development/feature-map.md) | Điểm vào, thao tác, trạng thái, source và assertion hiện có; tách coverage khỏi kết quả đã chạy |
| Hướng dẫn agent | [AGENTS.md](../AGENTS.md), [Skills](../.agents/skills/README.md) | Cách làm việc với repo và chọn skill theo nhiệm vụ |
| Nguồn skills | [Skill adaptation](references/skill-adaptation.md) | Nguồn Cantos được pin riêng, draft workflow coding agent và cách chọn lọc cho Vink |
| Bản ban đầu | [README](archive/initial/README.md), [Vision](archive/initial/prd/vision.md) | Giữ nguyên nội dung có sẵn trước lần nhập foundation; chỉ là tư liệu lịch sử |

## Ánh xạ bộ foundation

Các file gốc từ thư mục `vink-foundation` được giữ nguyên nội dung sản phẩm, ID yêu cầu, trạng thái và nguồn tham chiếu; liên kết nội bộ được sửa theo vị trí mới. Trang chỉ mục này bổ sung bản đồ repository và ghi nhận việc nhập tài liệu. Không chạy lại nghiên cứu web, spike hay acceptance runtime trong lần sắp xếp.

| Tên file đầu vào | Vị trí hiện tại |
| --- | --- |
| `Vink-00-README.md` | [docs/README.md](README.md) |
| `Vink-01-Vision.md` | [prd/vision.md](../prd/vision.md) |
| `Vink-02-Tech-Stack.md` | [docs/architecture/tech-stack.md](architecture/tech-stack.md) |
| `Vink-03-Requirements.md` | [prd/requirements.md](../prd/requirements.md) |
| `Vink-04-Architecture-Decisions.md` | [docs/architecture/decisions.md](architecture/decisions.md) |
| `Vink-05-Sources.md` | [docs/references/sources.md](references/sources.md) |

Skeleton ngày 30/10/2026 hiện thực một phần luồng trên: soạn và lưu nháp tiếng Việt có kiểm phiên bản, đọc snapshot công khai từ seed. Gửi duyệt, xuất bản và API bình luận chưa có; trạng thái từng phần nằm ở [docs/development](development/README.md).

Khi bổ sung implementation, cập nhật trạng thái ở tài liệu sở hữu và dẫn bằng chứng tương ứng. Không suy ra một module, API, command hay gate đã tồn tại từ ví dụ trong tài liệu; kiểm tra mã nguồn và manifest hiện hành. Bản archive không được dùng để ghi đè hướng hiện tại.
