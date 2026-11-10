# Các quyết định và lộ trình kiến trúc Vink

**Phiên bản:** 0.4 · **Ngày:** 30/10/2026, cập nhật 03/11/2026 và 10/11/2026 · **Trạng thái:** Decision log đề xuất. Không mục nào tự chuyển thành Accepted chỉ vì được viết trong tài liệu này; mục được chủ sản phẩm chốt ghi rõ ngày và phạm vi.

**Cập nhật 10/11/2026:** chủ sản phẩm chốt 15 quyết định nghiệp vụ (`PRODUCT-DECISIONS-20261110`).
[DecisionDelta](product-decisions-20261110.md) ghi luật cũ → luật mới, lựa chọn `DESIGN_PROPOSED`
và các điểm `OPEN`. Trong bảng dưới, “**Hướng sản phẩm đã chốt**” nghĩa là chủ sản phẩm đã chọn
ngữ nghĩa; nó **không** chuyển ADR thành Accepted và không nói runtime đã có. Các đoạn văn cũ được
giữ làm lịch sử; chỗ bị thay có ghi chú ngày.

## 1. Ba quyết định ảnh hưởng lớn nhất

### Quyết định 1 — Ai sở hữu tác phẩm?

**Đề xuất:** Vink sở hữu Work/Chapter/Revision/Publication trong PostgreSQL.

**Đã chốt 03/11/2026 (chủ sản phẩm, phần thảo luận):** Vink cũng sở hữu bình luận bằng Rust
core và PostgreSQL của App. Discourse chỉ là nguồn tham khảo thiết kế, API, kiểm duyệt và trải
nghiệm cộng đồng; không là dependency runtime hay nơi lưu bình luận
([ADR ownership](discussion-ownership.md)). Các đoạn dưới đây giữ lý do lịch sử của phần nội dung.

Lợi ích là làm chủ bản nháp song song với bản công khai, quyền xuất bản, tuyển chọn và export. Chi phí là Vink phải xây một màn vận hành tác phẩm riêng và chịu trách nhiệm về revision schema, backup, migration. Đây là lý do chọn B; không dựa trên khẳng định rằng Discourse tuyệt đối không thể mở rộng.

Chỉ chọn A nếu ưu tiên kiểm chứng cộng đồng nhanh hơn các khả năng này và chấp nhận Topic/Post là đơn vị nội dung ban đầu. Nếu B chỉ tận dụng một tập rất nhỏ chức năng Discourse, cần so lại với C trước khi đầu tư bridge sâu.

### Quyết định 2 — Frontend riêng tới mức nào trong beta?

**Đề xuất:** Vink tự làm đọc, soạn, publication và bình luận hai tầng. Theo quyết định 03/11/2026, không có Discourse Admin: Vink phải tự cung cấp màn kiểm duyệt bình luận tối thiểu, báo cáo và khiếu nại trước khi mở bình luận công khai cho pilot.

Không đặt mục tiêu tái tạo toàn bộ profile, notifications, search, badge và history kiểu diễn đàn trong Leptos ngay. Nếu yêu cầu trải nghiệm hoàn toàn thống nhất từ đầu, phải đưa toàn bộ phần UI/API đó vào kế hoạch và chấp nhận khối lượng lớn hơn.

### Quyết định 3 — Ai là nguồn xác thực?

**Đề xuất:** Vink có principal và session độc lập; khảo sát một auth authority được duy trì, với native auth riêng là phương án thay thế cần so bằng thời gian vận hành và UX thực tế. Từ 03/11/2026 không còn bridge DiscourseConnect hay dùng Discourse làm nguồn auth.

Không dùng chung bảng session/password với Cantos. Liên thông audio chưa đòi hỏi SSO; một handoff có sự đồng ý và tài khoản được xác minh có thể đi trước hệ thống đăng nhập chung. Cantos Identity hiện không phải OIDC provider sẵn có cho Vink. [C6]

## 2. Sổ quyết định

| ID | Nội dung | Trạng thái | Mặc định đề xuất / điều cần chứng minh |
| --- | --- | --- | --- |
| D-01 | PostgreSQL, database nghiệp vụ độc lập Cantos | Ràng buộc đầu vào | Giữ nguyên trong mọi phương án |
| D-02 | Vink sở hữu content; thảo luận do Vink sở hữu (Rust core + PostgreSQL App) | Content: đề xuất ưu tiên. Thảo luận: **đã chốt 03/11/2026** | Discourse chỉ là tham khảo; Vink tự xây tiền kiểm, report/appeal, chống lạm dụng, thông báo trước khi mở công khai |
| D-03 | Một Rust app + worker, chia module trước | Đề xuất | Không tách microservices theo tên nghiệp vụ |
| D-04 | Leptos SSR + JavaScript editor | Cần spike | Giữ nếu editor/SSR đạt; có thể đổi frontend mà giữ API/domain |
| D-05 | Auth authority | **Đề xuất A cho pilot (Provisional, 07/11/2026)**; chưa Accepted. **10/11/2026: hướng sản phẩm đã chốt là username + mật khẩu làm đăng nhập chính** (ACC-03); phương thức mã email không còn là hướng chính, có thể bổ sung sau. Authority native trong `vink-identity`, phiên opaque, binding/đình chỉ ở App không đổi. **Hiện trạng code vẫn chỉ có mã email**; username/mật khẩu chưa có ([ADR §12](auth-authority.md#12-đổi-hướng-10112026-username--mật-khẩu-là-chính)) | Spike S-03: authority native trong `vink-identity` (mã email một lần + phiên opaque phía server), App giữ principal/binding/đình chỉ; Keycloak và Supabase/Clerk không chọn cho pilot. 10/11/2026: backend có đường production (outbox bền + SMTP/Resend, rate limit theo mạng nguồn + trần Identity, fence đình chỉ tại commit cho mọi lệnh ghi được bảo vệ, retention, rollback; đăng nhập production khóa bằng `VINK_AUTH_SIGN_IN`). Blocker còn: chủ sản phẩm chấp nhận UX, gửi thật qua Resend + deliverability, edge/WAF, qualification trình duyệt trên HTTPS thật + native (UI Web đã có và chạy trong môi trường thử, [Auth Web](../development/auth-web.md)), review bảo mật độc lập, người chịu trách nhiệm auth ([ADR](auth-authority.md) §9, §11). Không còn DiscourseConnect |
| D-06 | Nháp riêng + public publication | Đề xuất pilot. **10/11/2026: hướng sản phẩm đã chốt** — đúng hai audience “Chỉ mình tôi”/“Công khai”, tách khỏi trạng thái kiểm duyệt (AUD-01/02) | Nhóm kín/unlisted/shared editing để sau. 10/11/2026: không thêm unlisted, bạn bè, nhóm kín hay ACL; runtime chưa có ([đặc tả đích](publication-moderation.md)) |
| D-07 | AI hỗ trợ từ MVP | Ưu tiên đầu vào. **10/11/2026: hướng sản phẩm đã chốt** — trợ lý AI là luồng duyệt mặc định cho bài, có thể tự quyết định theo policy; hiện chỉ đặc tả contract + mock (MOD-07/08) | Pilot có người duyệt; auto-approval theo nhóm được đánh giá. 10/11/2026: không mặc định có người trực, không tự đặt SLA; lỗi/timeout không bao giờ là duyệt; **bật auto-approval production vẫn cần MOD-06**; không provider thật trong lượt này |
| D-08 | Bình luận mới tiền kiểm; sửa công khai theo gate đã chứng minh | Cần spike. **10/11/2026: hướng sản phẩm đã chốt** — bình luận gốc/phản hồi do trợ lý AI kiểm duyệt (COM-09), không còn điều kiện “chỉ người đáng tin”; cho sửa bình luận của mình, có revision, kiểm duyệt và dấu “Đã sửa” (COM-10). **Code hiện tại vẫn `premoderation.v1` và chặn sửa** ([admission §8](discussion-admission.md#8-hướng-đích-sau-quyết-định-10112026), [sửa bình luận](comment-editing.md)) | Nếu edit gate chưa đạt, chặn edit cả UI/API trong pilot. #120 DISC-01: tiền kiểm là policy có version `premoderation.v1`, hiệu lực duy nhất; ứng viên đăng-trước `trusted-cohort.v1` chỉ chạy shadow, delta và điều kiện bật là **PROPOSED** ([admission](discussion-admission.md)) |
| D-09 | Tạp Chí chỉ hiện snapshot còn là publication hiện hành | Đề xuất pilot. **`DEFERRED` 10/11/2026:** Tạp Chí, tuyển chọn, biên tập và curation không thuộc phạm vi hiện tại; không còn là gate của mạng xã hội bài ngắn | Bản mới làm tuyển chọn tạm ngừng, chờ biên tập xác nhận lại (giữ làm lịch sử) |
| D-10 | Một phần trước, nhiều chương cơ bản ở beta | Cần lựa chọn sản phẩm. **10/11/2026: đã chọn** — Vink là mạng xã hội bài ngắn/tản văn; **không** triển khai nhiều chương (WR-04 `DEFERRED`) | Nếu nhiều chương là điểm khác biệt ngày đầu, nâng WR-04 thành Must — Pilot (lịch sử). Tổ chức bài bằng bộ sưu tập cá nhân: D-16 |
| D-11 | Nhóm tuổi beta và kênh giao tiếp | Cần lựa chọn sản phẩm. **`DEFERRED` 10/11/2026:** chưa quyết định nhóm tuổi; không code/thiết kế age gate, ngày sinh, parental consent. Hoãn **không** phải quyết định mở cho mọi độ tuổi và không chặn lượt tài liệu/model | Nhóm người trưởng thành được mời là phương án giảm phạm vi ban đầu (giả định “pilot 18+ theo lời mời” **đã gỡ** khỏi phạm vi đã chốt); chưa mở DM |
| D-12 | Rút bài và thu hồi discussion/excerpt | Cần chốt policy + spike. **10/11/2026: hướng sản phẩm đã chốt cho xóa bài** — tách bốn trường hợp (PUB-05): tác giả xóa bài từng công khai để lại trang thay thế + bình luận hợp lệ (PUB-04); chuyển riêng tư, kiểm duyệt gỡ và chưa từng công khai thì không | Vink chặn đọc tác phẩm và thảo luận ngay vì cùng owner (nay chỉ đúng cho riêng tư/gỡ do kiểm duyệt); cache/excerpt có trạng thái riêng; không báo hoàn tất trước khi xác minh. Còn `OPEN`: retention/purge, quyền ghi trên tombstone ([đặc tả đích](publication-moderation.md#6-xóa-riêng-tư-gỡ-và-tombstone)) |
| D-13 | Tác giả bật/tắt bình luận theo tác phẩm; không sửa/xóa bình luận người khác | **Đã chốt 03/11/2026** | Quyền bật/tắt tách khỏi quyền kiểm duyệt hệ thống; “tắt” chỉ ngừng nhận bình luận mới, không ẩn bình luận đã duyệt; tác phẩm mới mặc định bật; tác giả có grant kiểm duyệt vẫn tự né tác phẩm của mình (xác nhận 03/11/2026); bật/tắt dùng generation của ý định được chấp nhận, yêu cầu ở đúng generation hiện tại luôn tăng generation kể cả khi trùng chế độ (xác nhận 05/11/2026), xem [ADR](discussion-ownership.md#4-quyền-của-tác-giả-tác-phẩm) |
| D-14 | Ghi nhận văn học (LVE, #73): work revision, root, flat reply | Lựa chọn pure slice LVE-PR-01, hợp nhất PR #80 sau #78 ngày 05/11/2026 | CHOICE-01 A, 02 B, core 03 B (accepted order + exact receipt), 08 A, 09 A, 11 A; không đổi COM-07/08 hoặc luật #79. Lát API + PostgreSQL (#81): PUT có command key, hai bảng state + receipt, đếm từ hàng chuẩn, tổng số tách trạng thái riêng, voter list không có. Ranking/icon/UI còn OPEN. 10/11/2026: “Đánh giá cao” hợp lệ của revision công khai hiện tại là đầu vào của Đề xuất và Top 100 (D-18); “Hữu ích” của bình luận không bao giờ là vote của bài; quan hệ với sửa bình luận là O-06. [Decision owner](literary-endorsement.md) |
| D-15 | Phân loại tác phẩm (TAG, #94/#95): catalog, thẻ nháp, metadata công khai | Lựa chọn pure slice TAG-PR-01 ngày 08/11/2026, chưa Accepted | CHOICE-01 B có giới hạn (một catalog, facet đóng dạng bài ≤ 1/chủ đề ≤ 3), CHOICE-02 B (snapshot metadata bất biến riêng, hash v1/`RevisionId`/anchor LVE không đổi), CHOICE-03 A (curated, không đề xuất). Store PostgreSQL ở #96 ([storage](../development/taxonomy-postgres.md): catalog/slot chuẩn hóa, version metadata riêng, binding cạnh revision); API catalog/thẻ nháp và lệnh operator ở #97 ([API](../development/taxonomy-api.md): resource thẻ nháp có version riêng, quản trị bằng `vink-app tag …` có audit, đối soát bằng desired state + `409` kèm thẻ đang lưu; lựa chọn slice, chưa Accepted); discovery ở #98 ([discovery](../development/discovery-api.md): mở rộng `GET /api/v1/works`, khóa gập dấu lưu bằng cột generated + index, nhiều thẻ là AND; lựa chọn slice, chưa Accepted); UI ở #99–#100; bắt buộc dạng bài khi gửi duyệt và duyệt lại snapshot metadata còn OPEN; curator theo người (grant/HTTP) chờ authority production. 10/11/2026: catalog curated **không** bị thay bằng hashtag; hashtag là hệ riêng (D-17). [Decision owner](work-taxonomy.md) |
| D-16 | Bộ sưu tập cá nhân | **Hướng sản phẩm đã chốt 10/11/2026**; runtime chưa có | Thư mục bài của chính tác giả, hai audience; hiển thị = membership ∧ quyền bộ sưu tập ∧ quyền bài; không lộ membership riêng tư; không cộng tác. Nhiều bộ sưu tập/bài và thứ tự do tác giả sắp là `DESIGN_PROPOSED` (O-01/02). [Đặc tả đích](collections.md) |
| D-17 | Hashtag để lọc | **Hướng sản phẩm đã chốt 10/11/2026**; runtime chưa có | Tách khỏi catalog curated; danh tính giữ dấu, bỏ dấu chỉ để tìm; chỉ index bài đủ điều kiện công khai. CHOICE-04 A (trích từ nội dung theo grammar) là `DESIGN_PROPOSED`. [Đặc tả đích](hashtags.md) |
| D-18 | Đề xuất (vote + độ mới) và Top 100 (vote) | **Hướng sản phẩm đã chốt 10/11/2026**; công thức **chưa chốt**; runtime chưa có | Hai policy riêng trên cùng tập đủ điều kiện; eligibility trước ranking; quyền hiện hành thắng snapshot. CHOICE-02 A `recommend.v1` và CHOICE-03 A (snapshot ≤ 100 trong cursor) là `DESIGN_PROPOSED`. [Đặc tả đích](discovery-ranking.md) |
| D-19 | Quản trị viên trên trợ lý AI; phản đối quyết định AI | **Hướng sản phẩm đã chốt 10/11/2026**; runtime chưa có | AI là actor dịch vụ có capability giới hạn; admin override thắng callback AI cùng phiên bản, có provenance và lý do, không vượt privacy/audit; phản đối tạo case gắn decision version, không nhân case; **chưa có** kênh gửi, dashboard hay email (MOD-09/10). [Đặc tả đích](publication-moderation.md#5-quản-trị-viên-và-phản-đối) |

Thảo luận hai tầng D2T-01/D2T-02: [ADR ownership](discussion-ownership.md) ghi quyết định
03/11/2026 chọn Vink làm owner duy nhất ghi/kiểm duyệt bình luận; [core/registry](../development/discussion-core.md)
ghi phần đã hiện thực và gap. D-05, D-08 và D-12 vẫn mở: D-05 có đề xuất provisional từ spike
S-03 nhưng chưa có auth production, nên ghi bình luận production vẫn bị chặn; tiền kiểm cần
authority kiểm duyệt và màn vận hành.

Củng cố thảo luận [#120](https://github.com/loveoverflowcom/vink/issues/120), DISC-01:
[admission](discussion-admission.md) chọn CHOICE-01 A (admission thuần có version + shadow, tiền kiểm
vẫn hiệu lực) và CHOICE-06 A (chỉ văn bản). D-05/D-08 giữ trạng thái; delta D-08 chưa được chấp nhận.
DISC-02/03 phần 2: [đọc bình luận](discussion-reads.md) chọn CHOICE-02 B+ (giữ composed v1, thêm public
projection độc lập Identity và overlay riêng tư có cursor riêng) và một index partial đã đo; ghi điểm
tuyến tính hóa từng loại thu hồi (App đình chỉ/grant nguyên tử tại commit; phiên Identity: cửa sổ một
request). Không đổi D/INV; không bật đăng-trước.

Phân loại tác phẩm [#95](https://github.com/loveoverflowcom/vink/issues/95), TAG-PR-01:
[decision owner](work-taxonomy.md) ghi CHOICE-01/02/03 của pure core; [registry](../development/taxonomy-core.md)
tách bằng chứng/gap. Thẻ không cấp quyền, không xuất bản hay tuyển chọn; không đổi D/INV hiện hành.

Literary endorsement [#73](https://github.com/loveoverflowcom/vink/issues/73), LVE-PR-01:
[decision owner](literary-endorsement.md) ghi CHOICE-01/02/03/08/09/11 cho pure slice;
[registry](../development/endorsement-core.md) tách evidence/gaps. Không đổi D/INV hiện hành,
không đóng epic hoặc nhận production readiness; các CHOICE còn lại vẫn OPEN.

## 3. Không dùng một state enum cho toàn bộ vòng đời

Tách các trục sau giúp diễn đạt đúng trạng thái người dùng thấy:

| Trục | Ý nghĩa | Ví dụ |
| --- | --- | --- |
| Working draft | Bản tác giả đang soạn | Version 18 chưa gửi duyệt |
| Revision moderation | Kết luận chính sách cho snapshot bất biến | r2 đang PendingAI, r1 đã Approved |
| Publication | Snapshot đang được phép đọc công khai | Pointer hiện tại là r1, generation 7 |
| Curation | Quyết định giới thiệu một snapshot | r1 từng được chọn; r2 chưa được chọn |
| Audience/access | Ai có quyền đọc tại thời điểm hiện tại | Nháp riêng, public publication; rút bài đóng public read |
| Job/integration | Tiến độ xử lý kỹ thuật | Retry AI, đang tạo thread, remote cleanup pending |

`Approved` là đủ điều kiện theo một quyết định; `Published` là kết quả của hành động công khai có quyền và ý định tác giả. Job thành công không tự quyết định publication.

### Máy trạng thái kiểm duyệt ở mức khái niệm

```mermaid
stateDiagram-v2
    direction TB
    [*] --> Draft
    Draft --> PendingAI: Gửi snapshot
    PendingAI --> Approved: Đạt policy được phép tự duyệt
    PendingAI --> NeedsRevision: Policy cho phép yêu cầu sửa tự động
    PendingAI --> HumanReview: Mơ hồ, lỗi, hết hạn hoặc cần người duyệt
    NeedsRevision --> HumanReview: Khiếu nại
    HumanReview --> Approved: Người duyệt chấp thuận
    HumanReview --> Rejected: Người duyệt từ chối
```

Sơ đồ mô tả luồng, không phải file Quint đã thực thi. Trong pilot AI chỉ tư vấn, cả kết quả rõ và mơ hồ đều đi HumanReview kèm gợi ý; AI không tự áp quyết định cuối. **10/11/2026:** câu “AI chỉ tư vấn” bị thay bởi D-07/D-19: kết quả hợp lệ của trợ lý AI có thể là quyết định; `HumanReview` nay là “cần quản trị viên xem xét”, không hàm ý có người đang trực. Đặc tả thực thi của luồng đích là [`publication.qnt`](../../services/vink-app/behavior/target/publication.qnt) (chưa có implementation). Chỉnh nội dung sau submit tạo snapshot mới; không biến một revision bất biến về Draft rồi sửa tại chỗ. Mọi phương án hủy, gửi lại và mở lại quyết định cần audit và version guard khi triển khai.

### Guard khi xuất bản

Một publish command chỉ có hiệu lực khi đúng snapshot/hash đã được duyệt, người thực hiện còn quyền, có ý định xuất bản phù hợp, access/withdrawal gate cho phép và expected publication generation vẫn khớp. Giao dịch ghi pointer, receipt/audit và outbox cùng lúc.

Để giữ UX gọn, nút “Gửi để đăng khi được duyệt” có thể lưu trước ý định xuất bản cho đúng snapshot. Khi được duyệt, application vẫn phải kiểm lại các guard; tác giả đã rút yêu cầu hoặc publication đã tiến sang phiên bản khác thì không tự đăng tiếp.

### Ví dụ cần giữ đúng

Cột “Tạp Chí thấy” được giữ làm lịch sử (D-09 `DEFERRED` 10/11/2026). Dòng “Tác giả rút tác phẩm”
nay tách thành xóa/riêng tư/gỡ theo PUB-04/05.

| Sự kiện | Người đọc Xưởng thấy | Tạp Chí thấy |
| --- | --- | --- |
| r1 đã công khai và được tuyển | r1 | r1 |
| Tác giả sửa r2, r2 đang chờ | r1 | r1 |
| r2 bị yêu cầu sửa/từ chối | r1 | r1 |
| r2 được duyệt nhưng chưa có publish intent hợp lệ | r1 | r1 |
| r2 được xuất bản thay r1 | r2 | Mục tuyển chọn tạm ngừng; chờ biên tập chọn r2 |
| Tác giả rút tác phẩm | Không trả văn bản công khai tại Vink | Không hiển thị tuyển chọn; xử lý các bản sao theo policy thu hồi |

Pilot chưa mở kho phiên bản công khai. Lịch sử revision và quyết định tuyển chọn vẫn được giữ theo quyền/retention đã chốt. Cách tách editing head và publication có điểm tương đồng với ADR 0054 của Cantos, nhưng thiết kế và mức hoàn thiện của Vink được quyết định riêng. [C7]

## 4. Các bất biến và giới hạn kiểm chứng

| ID | Thuộc tính cần giữ | Bằng chứng cần có |
| --- | --- | --- |
| INV-01 | Public read chỉ trả snapshot có quyết định phù hợp, ý định xuất bản và access gate hiện tại | Model/pure tests + API/SSR/search tests |
| INV-02 | Sửa/chờ/từ chối r2 không thay publication r1 | Model + PostgreSQL transaction/concurrency test |
| INV-03 | Callback cũ/sai hash không thắng decision hoặc generation mới | Model + worker lease/version tests |
| INV-04 | Curation không tự chuyển sang revision mới; mất quyền hiển thị thì không được đọc qua tuyển tập | Model + query/UI tests |
| INV-05 | Client không chọn thay identity hoặc quyền người khác | Auth/API integration tests; không chỉ model giả định actor hợp lệ |
| INV-06 | Một subject có một thread discussion hữu hiệu; retry không tạo thread ngoài kiểm soát | Unique constraint + PostgreSQL concurrent/retry test |
| INV-07 | Publish event cũ không khôi phục nội dung đã rút | Subject generation kiểm tại commit; projection/cache có fencing hoặc kiểm quyền khi đọc |
| INV-08 | Nháp riêng không lọt vào public DTO/cache/provider ngoài scope | Review data flow + integration/browser tests |

INV-01..08 không đổi sau quyết định 10/11/2026. Các model đích ở
[`behavior/target`](../../services/vink-app/behavior/target/README.md) đặt tên invariant riêng và
dẫn về các INV này trong [registry](../development/product-decisions-registry.md); chúng là bằng
chứng **model**, chưa phải bằng chứng code hay PostgreSQL.

**Tiến triển cần điều kiện vận hành:** PendingAI phải được worker/watchdog đẩy tiếp khi retry/deadline hết; HumanReview phải có người phụ trách và khung xử lý được thông báo. Một queue có dữ liệu không chứng minh rằng người dùng sẽ được giải quyết. Thời gian chờ là chỉ số cần đo trong pilot.

Quint phân biệt safety và liveness; liveness thường cần temporal properties và giả định fairness. Model chỉ có giá trị trong phạm vi đã mô tả và phương pháp chạy được ghi lại. Không dùng câu “đã chứng minh không bao giờ lỗi” cho code hay PostgreSQL chỉ vì simulation PASS. [T8], [C8]

## 5. Quyết định về thu hồi và đồng bộ cần viết rõ

**10/11/2026:** mặc định dưới đây nay áp cho **chuyển riêng tư** và **gỡ do kiểm duyệt**. Khi
tác giả **xóa** bài đang công khai, nội dung và mọi preview do hệ thống sinh bị đóng nhưng URL cũ
giữ trang thay thế và các bình luận hợp lệ (PUB-04/05, [đặc tả đích](publication-moderation.md#6-xóa-riêng-tư-gỡ-và-tombstone)).

**Mặc định đề xuất cho pilot:** rút bài đóng ngay public read ở Vink, kể cả thảo luận của tác
phẩm: vì thảo luận nay cùng owner và cùng database (quyết định 03/11/2026), đường đọc/ghi bình
luận kiểm trạng thái publication hiện hành tại thời điểm đọc và tại commit, không chờ event.
Bình luận được giữ nguyên để đối soát và khôi phục theo policy; không xóa hàng loạt.

Excerpt, trích dẫn trong replies, preview, thông báo đã gửi và cache là các bề mặt khác nhau,
cần policy riêng. Projection/cache phải kiểm quyền hiện hành khi đọc hoặc bị vô hiệu theo
generation; xóa một bản cache không tự chứng minh mọi bản sao đã được thu hồi.

Mỗi subject cần generation tăng dần và desired state bền vững. Worker xử lý projection/thông
báo đọc lại desired state trước side effect và đối soát sau; side effect ra ngoài hệ thống
(email, push) không được coi là exactly-once. Thời hạn thu hồi các bản sao ngoài Vink và cách xử
lý khi quá hạn cần chủ sản phẩm chốt trước public beta.

## 6. Năm thử nghiệm cần làm trước khi chốt stack

| Spike | Câu hỏi cần trả lời | Đầu ra và tiêu chí go/no-go |
| --- | --- | --- |
| S-01 Editor + SSR | Leptos và editor có cho trải nghiệm viết tiếng Việt tốt không? | Demo có thơ/tản văn, IME, paste, undo, autosave, hai tab, refresh và mobile; schema round-trip; public HTML/metadata đúng. Nếu không đạt, thử editor/frontend thay thế trước khi xây nhiều màn |
| S-02 Tiền kiểm bình luận | ~~Discourse moderation~~ — thay ngày 03/11/2026 bằng tiền kiểm trong module thảo luận Vink | Mọi đường ghi (Web/API/staff) tạo bình luận chờ duyệt; quyết định gắn version; list/count/preview/thông báo không lộ pending; quyết định trễ không thắng; có người trực hàng đợi |
| S-03 Identity | Map người và quyền mà không mở lỗ hổng đăng thay được không? | Thử signup/verified email, account đã có, đổi email/username, provisioning lỗi, logout/ban, request giả user; quyết định auth authority và authority cho suspension |
| S-04 Đồng bộ + dữ liệu | Có sống sót qua crash/replay/withdraw/restore không? | PostgreSQL transaction và CAS; lệnh ghi bình luận mất acknowledgment; publish/withdraw đến sai thứ tự; remote cleanup thất bại; restore và reconcile giữ đúng dữ liệu/quyền |
| S-05 AI + vận hành | Tự động hóa mức nào giảm việc mà không làm hại vòng phản hồi? | Bộ mẫu tiếng Việt, schema output, thử injection/timeout, cost baseline, false positives, tuổi human queue; chốt nhóm auto-approve và cách tắt/đảo quyết định |

Ngày 02/11/2026, theo yêu cầu của người dùng, PostgreSQL được tách theo owner và Identity thành process riêng chưa có authority ([ranh giới PostgreSQL/Identity](identity-postgres-boundaries.md)). Đó là ranh giới kiến trúc, không phải lựa chọn auth authority.

Ngày 07/11/2026 spike S-03 ([ADR D-05](auth-authority.md), [bằng chứng](../development/auth-authority-s03-evidence.md)) so ba hướng (native Rust, Keycloak, Supabase/Clerk) và hiện thực hướng native trên ranh giới đó: tài khoản/mã/phiên ở Identity, binding và đình chỉ ở App, kiểm bằng PostgreSQL thật và hai binary thật. Phần “thử signup/verified email, account đã có, đổi email/username, provisioning lỗi, logout/ban, request giả user” đã có bằng chứng cục bộ; authority cho suspension là App (đình chỉ principal) cộng Identity (disable tài khoản). S-03 **chưa hoàn tất ở mức production**: không có mail transport, rate limit biên, trình duyệt thật hay native.

Skeleton ngày 30/10/2026 ([docs/development](../development/README.md)) dùng Hybrid B, một app Rust + worker và Leptos SSR làm giả định bootstrap. Nó cung cấp chỗ thử cho S-01, S-03 và S-04 (textarea, seam identity, CAS bản nháp trên PostgreSQL) nhưng không hoàn tất spike nào và không đổi trạng thái D-02–D-12.

Mỗi spike cần ghi phiên bản, môi trường, kịch bản, kết quả quan sát, giới hạn và quyết định tiếp theo. Không cần xây một sản phẩm hoàn chỉnh để thử, nhưng demo “happy path” duy nhất chưa đủ để chốt B.

## 7. Lộ trình theo điều kiện hoàn thành

| Mốc | Sản phẩm bàn giao | Điều kiện chuyển mốc |
| --- | --- | --- |
| M0 — Nền tảng quyết định | Bộ vision/stack/requirements này và chủ sở hữu review | Xác định nhóm beta, content ownership, UI boundary và auth direction |
| M1 — Luồng xuyên suốt | Một tác phẩm thật đi từ editor tới publication và discussion | S-01–S-04 có bằng chứng đủ; không lộ nháp, không đăng nhầm bản, có admin tối thiểu |
| M2 — Pilot có mời (10/11/2026: không còn giả định 18+ theo lời mời hay người trực hàng đợi; D-11 `DEFERRED`, MOD-08) | Nhóm nhỏ viết/đọc/góp ý; AI hỗ trợ; người trực hàng đợi | Quan sát chất lượng phản hồi, thời gian chờ, lỗi editor, chi phí; có export/restore thử |
| M3 — Public beta | Phạm vi đã chốt, quy tắc công khai, vận hành có người chịu trách nhiệm | MOD-06 nếu mở auto-approval; budgets, privacy/age, withdrawal policy và compatibility matrix được chấp nhận |
| M4 — Mở rộng | Nhiều chương nâng cao, margin notes, coach, Cantos handoff theo nhu cầu | Mỗi phần có usage case, owner, quyền dữ liệu và thử nghiệm riêng |

Không ấn định số tuần hoặc ngày ra mắt trước khi biết số người vận hành, phạm vi UI headless và kết quả spike. Với đội nhỏ, giảm phạm vi chức năng trước khi giảm các bảo đảm về bản thảo, quyền và xuất bản đúng phiên bản.

## 8. Những tài liệu nên viết sau bộ này

Sau review/spikes: ADR content ownership và identity; schema tài liệu editor và migration strategy; API contracts/fixtures; publication/moderation transition rules; một hoặc hai model Quint tập trung; runbook moderation/backup/restore. Mỗi tài liệu gắn với quyết định đã chốt, tránh sinh đồng thời một bộ đặc tả lớn chưa có cơ sở.

[C6]: https://github.com/loveoverflowcom/cantos/blob/a8e2b6f40b58e0e72a2bdab2d4f44240cb6a0330/services/cantos-server/contracts/auth/identity-auth/v1/README.md
[C7]: https://github.com/loveoverflowcom/cantos/blob/a8e2b6f40b58e0e72a2bdab2d4f44240cb6a0330/docs/decisions/0054-publication-generation-and-access.md
[C8]: https://github.com/loveoverflowcom/cantos/blob/a8e2b6f40b58e0e72a2bdab2d4f44240cb6a0330/docs/decisions/0027-quint-executable-behavior.md
[T8]: https://quint.sh/docs/checking-properties
