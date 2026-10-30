# Vink — Việt Ink

## Bộ định hướng sản phẩm và công nghệ

**Phiên bản:** 0.2 · **Ngày:** 30/10/2026 (UTC+07) · **Trạng thái:** Đề xuất để review, chưa phải ADR đã được phê duyệt hay hệ thống đã triển khai.

**Vink giúp người viết Việt duy trì việc viết, tìm được người đọc phù hợp và tiến bộ qua phản hồi tử tế.** Trải nghiệm ưu tiên văn bản, tiếng Việt và cộng đồng có chủ đích; thiết kế trẻ trung theo Material 3 Expressive, đồng thời đủ yên tĩnh để đọc dài.

Hướng kiến trúc ưu tiên là **Hybrid có giới hạn phạm vi**: Vink sở hữu tác phẩm và vòng đời xuất bản trong PostgreSQL; Discourse sở hữu thảo luận. Leptos SSR và Axum phục vụ giao diện riêng; một worker xử lý kiểm duyệt và đồng bộ nền. Phương án này cần vượt qua các thử nghiệm tích hợp trong tài liệu quyết định trước khi chốt.

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
- **Khuyến nghị:** Hybrid B, Vink sở hữu nội dung văn học, Rust modular monolith, Discourse giới hạn ở cộng đồng, một nguồn định danh Vink ổn định.
- **Chưa chốt:** Nhà cung cấp xác thực, phạm vi UI Discourse được giữ trong beta, nhóm tuổi phục vụ, thời điểm mở nhiều chương, plugin tiền kiểm và ngân sách hiệu suất/AI.
- **Đã đối chiếu nguồn:** Cantos `develop` tại commit `a8e2b6f40b58e0e72a2bdab2d4f44240cb6a0330`; tài liệu chính thức Discourse và các công nghệ tham chiếu ngày 30/10/2026. Đây là bằng chứng về nguồn được đọc, không xác nhận trạng thái production.

## Những thay đổi chính so với vision ban đầu

1. Bổ sung **Bàn viết riêng**; phân biệt thể loại nhật ký với quyền riêng tư.
2. Tách **kiểm duyệt chính sách**, **xuất bản theo ý định tác giả** và **tuyển chọn biên tập**.
3. Đưa AI hỗ trợ vào MVP, đồng thời giữ hàng đợi thủ công, giải thích và khiếu nại.
4. Khuyến nghị Vink sở hữu Work/Chapter/Revision/Publication; thảo luận giữ định danh ổn định qua nhiều lần sửa.
5. Ghi rõ chi phí của Hybrid: hai hệ sinh thái, tích hợp xác thực, đồng bộ và màn quản trị tác phẩm riêng.
6. Hiệu chỉnh các giả định: Horizon là theme; Cantos Web hiện dùng CSR; Quint có giới hạn phạm vi bằng chứng; webhook không tự bảo đảm tiền kiểm.

## Bước triển khai đầu tiên được đề xuất

Chốt nhóm beta và phạm vi tác phẩm; thử một luồng xuyên suốt **soạn tiếng Việt → lưu nháp → gửi duyệt → xuất bản đúng phiên bản → bình luận qua Discourse → sửa bản mới trong khi bản cũ vẫn đọc được**. Chỉ sau khi luồng này vượt qua các tình huống lỗi quan trọng mới mở rộng tính năng.

Bộ đề xuất v0.2 đã được đưa vào repository Vink ngày 30/10/2026. Việc nhập tài liệu không phê duyệt các đề xuất và không xác nhận đã triển khai hay kiểm chứng runtime Vink.

## Bản đồ tài liệu trong repository

| Nhóm | Vị trí | Cách sử dụng |
| --- | --- | --- |
| Sản phẩm | [Vision](../prd/vision.md), [Requirements](../prd/requirements.md) | Định vị, phạm vi và acceptance có ID; các mục vẫn giữ trạng thái đề xuất |
| Kiến trúc | [Tech stack](architecture/tech-stack.md), [Decision log](architecture/decisions.md) | So sánh phương án, quyết định mở, bất biến và S-01–S-05; chưa phải ADR Accepted |
| Bằng chứng nguồn | [Sources](references/sources.md) | Nguồn nghiên cứu của foundation, có commit và giới hạn bằng chứng |
| Hướng dẫn agent | [AGENTS.md](../AGENTS.md), [Skills](../.agents/skills/README.md) | Cách làm việc với repo và chọn skill theo nhiệm vụ |
| Nguồn skills | [Skill adaptation](references/skill-adaptation.md) | Nguồn Cantos được pin riêng và các thay đổi cho Vink |
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

Khi bổ sung implementation, cập nhật trạng thái ở tài liệu sở hữu và dẫn bằng chứng tương ứng. Không suy ra một module, API, command hay gate đã tồn tại từ ví dụ trong tài liệu; kiểm tra mã nguồn và manifest hiện hành. Bản archive không được dùng để ghi đè hướng hiện tại.
