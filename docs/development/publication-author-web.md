# Vùng công khai của Bàn viết — PUB-AUTHOR-WEB-01

**Phạm vi:** consumer Web của API publication tác giả ([publication-runtime](publication-runtime.md),
#158) và của trang trạng thái kiểm duyệt ([moderation-runtime](moderation-runtime.md), #159/#170):
chọn “Chỉ mình tôi”/“Công khai”, lưu → chụp snapshot → gửi duyệt đúng bản, theo dõi kết quả, chuyển
riêng tư, đối soát khi mất phản hồi, hai tab và đổi phiên. Không có store, authority, job queue,
route hay migration mới. Layout: [LayoutSpec](../design/publication-author.md). Liên quan (không
đóng): [#174](https://github.com/loveoverflowcom/vink/issues/174).

## Bốn chiều trạng thái

1. **Product decision:** không mở lại luật nào. Đọc từ registry tại checkout thực thi
   (`cargo xtask decision query --domain publication|moderation`, snapshot
   `70a6a63ebbf19622599853050ca0910a3dbd99fdca9b4ceab4f87bfede908107`): `USER-02@r1` (hai audience),
   `ADM-04@r1` (phản đối theo phiên bản quyết định), `ADM-06@r1` (gửi duyệt theo phiên bản; riêng tư
   đóng ca/phản đối), `ADM-07@r1` (trang ẩn), `O-17@r2` — tất cả `PRODUCT_CONFIRMED`. Không có điểm
   chưa chốt nào đổi quyền, privacy hay ngữ nghĩa publication nên không có blocker sản phẩm.
2. **Architecture decision:** không ADR mới, không nâng quyết định nào lên `Accepted`. Các lựa chọn
   dưới đây là lựa chọn triển khai, đảo được.
3. **Implementation level:** `web` (consumer của `persistence-api`). Chưa có CMP, xóa tác phẩm/
   tombstone, AI provider thật, thông báo.
4. **Verification evidence:** mục [Bằng chứng](#bằng-chứng).

## Tác giả làm được gì (và chưa)

Đăng nhập (mã email) → Bàn viết → viết, lưu, chọn thẻ → vùng “Công khai” ở cuối trang: chọn
**Công khai** → “Lưu và gửi duyệt” (hoặc “Gửi duyệt bản đã lưu” khi đã lưu) → vùng nói bản nào đang
được đọc, bản nào đang chờ và quyết định → người duyệt có grant kết luận bằng trang hiện hữu →
tác phẩm hiện ở Khám phá/reader đúng revision và thẻ → tác giả chọn **Chỉ mình tôi** → “Chuyển
riêng tư” (không cần lưu, không mất chữ đang viết) → trang công khai ngừng phục vụ nội dung.

Chưa có: công khai lại một revision đã duyệt mà không chụp bản mới (hiện luôn chụp bản mới); polling;
thông báo; xóa tác phẩm; CMP. Chuyển riêng tư không hứa xóa bản sao ngoài Vink hay ở tab chưa tải
lại, và vùng nói rõ điều đó.

## Cấu trúc

| Lớp | Owner | Ghi chú |
| --- | --- | --- |
| DTO và phân loại lỗi | [contract/publication.rs](../../apps/vink-web/src/contract/publication.rs), `ApiError` trong [contract.rs](../../apps/vink-web/src/contract.rs) | Một DTO `AuthorPublicationState` duy nhất (trang `…/kiem-duyet` dùng lại; DTO cũ chỉ đọc một phần đã bỏ). `audience`/`lifecycle`/`moderation` có `Unknown`: giá trị lạ khóa mọi lệnh và hiện câu an toàn, không suy ra “riêng tư/đã duyệt/rỗng” |
| Transport | [api.rs](../../apps/vink-web/src/api.rs) | `publication_state_path`, `snapshots_path`, `request_public_path`, `set_private_path` (work id là một segment mã hóa); `bounded()` nay phủ cả ba lệnh |
| State owner (thuần) | [publication_state.rs](../../apps/vink-web/src/publication_state.rs) | Reducer `Controller`: pha client, ticket, key, fence; trả `Step`, không I/O |
| Adapter + view | [pages/publication.rs](../../apps/vink-web/src/pages/publication.rs) | `PublicationPanel`, `Runner` thực thi `Step`; câu chữ (`notice_text`, `summary_lines`, `decision_label`) |
| Editor | [pages/desk.rs](../../apps/vink-web/src/pages/desk.rs) | Chỉ gắn vùng vào khung `.vk-editor`, truyền `save` và `is_composing`. `DraftSession`/`TagSession`/`NavigationGuard` giữ nguyên chủ sở hữu việc lưu, conflict, chữ chưa lưu và rời trang |
| Cache công khai | [public_reads.rs](../../apps/vink-web/src/public_reads.rs) | `invalidate_withdrawn` (đã có) sau rút bài; `invalidate_publication` (mới, không xóa thẻ) khi con trỏ công khai đổi |

## Pha client (không phải trạng thái kiểm duyệt)

`reading`, `unavailable`, `unreadable`, `ready`, `saving`, `capturing`, `requesting_public`,
`making_private`, `uncertain(capturing|requesting|making)`, `conflict(draft|metadata|generation)`,
`failed(…)`, `reconciled(requested|made_private)`. Mỗi lệnh gắn epoch (phiên + work), id thao tác và
lần thử vật lý; câu trả lời của bất kỳ thứ gì khác không đổi gì.

Giữ độc lập (UI chỉ tổng hợp để trình bày, không tạo một status mơ hồ rồi dùng nó thay guard): nháp
đang viết; revision bất biến; audience + ý định công khai; quyết định kiểm duyệt; con trỏ công
khai; lifecycle. “Đang công khai” **chỉ** đến từ con trỏ công khai của máy chủ; radio đang chọn là
ý định của tác giả; receipt là lịch sử của một lệnh và luôn được đọc lại.

## Lưu → chụp → gửi

1. Bấm ghim `generation` đang hiển thị và dấu “đã bấm” của editor (đếm sửa + lựa chọn thẻ).
2. Còn chữ/thẻ chưa lưu: gọi đúng hàm lưu của editor rồi **chỉ quan sát** `DraftSession`.
3. Chụp snapshot ở đúng `draft_version` + `metadata_version` mà máy chủ vừa xác nhận. Nếu có chữ/thẻ
   mới sau bước 1, hoặc lưu lỗi/xung đột: **dừng**, không chụp, nói rõ chưa gửi gì.
4. `request-public` với đúng `revision_id` vừa chụp và generation đã ghim; rồi đọc lại state.

Chụp thành công mà gửi thất bại/chưa rõ: giữ revision và thao tác; “Kiểm tra lại” không chụp mới.
Chuỗi HTTP không phải một transaction; không hứa rollback save/snapshot khi bước sau lỗi. Reload hoặc
đổi route giữa chụp và gửi để lại một revision `unsubmitted`; vùng đếm “Có N bản đã chụp nhưng chưa
gửi duyệt” thay vì đoán, và không tự tiếp tục.

## Mất phản hồi, xung đột, phiên

| Tình huống | Hành vi |
| --- | --- |
| Mất ACK của snapshot | `uncertain(capturing)`; “Kiểm tra lại” phát lại **cùng key + cùng version**: máy chủ trả revision đầu (`replayed`), không bản thứ hai |
| Mất ACK của request-public/set-private | `uncertain`; kiểm tra **đọc state trước**: generation đã ghim còn đứng → phát lại đúng key + payload; đã tăng và ý định đúng của mình → xong từ state, không phát lại; đã tăng vì tab khác → `conflict(generation)`, không phát lại, không ghi đè |
| `409` của snapshot | `draft` (`expected_draft_version`) khác `metadata` (`expected_metadata_version`): nêu đúng loại, chưa ghi gì, nội dung giữ nguyên |
| `409` generation | Đọc lại state; “Đã hiểu”, hoặc “Gửi lại bản đã chụp” (lệnh mới, key mới, generation đang hiển thị, **không** chụp mới) — chỉ sau khi đọc state mới |
| `revision_rejected`, `unknown_revision`, `work_unavailable`, `idempotency_key_reused` | Dừng, nói đúng điều đã xảy ra, đọc lại state |
| `401`/`403`/`404` | Phân biệt phiên hết / bị từ chối / không thuộc về bạn; dữ kiện riêng của phiên cũ rời trang |
| Mạng/timeout/`5xx`/phản hồi không hiểu | Luôn là *chưa rõ*, không thành thành công hay từ chối |
| `replayed` thiếu `ticket_id` | Một câu trả lời bình thường; đọc state, không tạo ticket thứ hai |
| Đổi phiên/tài khoản/work | Epoch mới: không dữ kiện, thao tác hay câu trả lời nào của phạm vi cũ còn tác dụng |

Chuyển riêng tư thắng một lần gửi chưa có câu trả lời: bước kế tiếp của lần gửi đó không bắt đầu và
câu trả lời muộn của nó không đổi gì. Một request đã ra khỏi trình duyệt vẫn có thể commit; fence
generation của máy chủ xử lý, không phải việc hủy request.

## Lựa chọn kỹ thuật

| Lựa chọn | Chọn | Bỏ | Lý do |
| --- | --- | --- | --- |
| CHOICE-01 | **B** — “Lưu và gửi duyệt” gọi hàm lưu của editor và quan sát `DraftSession` | A — bắt bấm “Lưu” riêng | Không sao chép logic lưu, không sửa editor; đảm bảo đúng bản nhờ dấu “đã bấm” + ack đúng phiên bản. A vẫn là hành vi khi thẻ chưa tải hoặc lưu đang lỗi/xung đột (nút khóa kèm lý do) |
| CHOICE-02 | **A** — vùng gọn tại chỗ | B — dialog | Trạng thái chưa rõ/xung đột phải còn thấy sau khi gửi; ở chữ 200% và 320 CSS px chỉ xếp dọc trong luồng trang, không thêm vùng cuộn hay bẫy focus |
| CHOICE-03 | **A** — đọc khi mở, khi quay lại cửa sổ (tối đa 2 s một lần, không giữa lệnh), sau mỗi lệnh, nút “Tải lại” | B — thêm polling | Kiểm duyệt không có SLA; polling thêm hẹn giờ, request chồng, rủi ro cướp focus/cuộn. Đảo được nếu có số liệu |

## Quyền riêng tư

Vùng nằm trong `Editor`, chỉ được dựng sau khi bản nháp (`LocalResource`) tải xong ở trình duyệt, nên
HTML do server render không có trạng thái publication hay chữ riêng (phase `publication-author` kiểm
điều này). Dữ liệu chỉ đọc ở trình duyệt, gắn phiên + route; mọi response `no-store`; không URL query, không browser storage mới,
không telemetry. Không lưu chữ, token hay mật khẩu để phục vụ khôi phục — chữ chưa lưu ở lại trong
editor (thuộc `DraftSession`), không ở đâu khác. Key lệnh sinh từ 16 byte `crypto.getRandomValues` của
trình duyệt; không có nguồn ngẫu nhiên thì nút khóa kèm lý do.

## Bằng chứng

Mọi lệnh chạy cục bộ trên macOS với PostgreSQL 17.11 cục bộ và Chrome headless; không dùng GitHub CI.

**Nguồn:** base `origin/develop@23fffd8ba758940569d09bcb250da88ba9a243ba`; nhánh
`codex/publication-author-ux`; không có dirty input liên quan (worktree sạch từ base). SHA cuối của
nhánh nằm trong mô tả PR (không thể tự chứa trong commit của chính nó). Kết quả dưới đây là của cây
làm việc ngay trước commit.

| Lệnh (lane) | Kết quả |
| --- | --- |
| `cargo xtask verify quick` | PASS: fmt, clippy server/browser(wasm32 hydrate)/UI preview, doctest, unit + contract (`vink-web` 196, `vink-app` 323), kiểm tra repo (liên kết, skills, quyết định) |
| `cargo xtask verify postgres` (PostgreSQL 17.11 cục bộ) | PASS: `vink-app` 167 test (1 `ignored` có sẵn), gồm `author_web_recovery_contract_on_real_rows` mới; identity 48; các bước nâng cấp/backout/backup |
| `cargo xtask verify web` | PASS: bảy phase trình duyệt (main, discovery-corpus, after-restart, endorsement, review-withdrawn, auth và **publication-author**); phase mới 76 kiểm tra, 72 ảnh trong `target/verify-web/screenshots` |
| `verify discussion`, `verify kani`, `verify expressions` | **NOT_RUN**: lát này không sửa domain, guard, persistence hay invariant (chỉ consumer Web, fixture/test và tài liệu), nên không có model/Kani nào bị tác động. Không coi NOT_RUN là PASS |

**AC → bằng chứng:**

| AC | Test / scenario | Kết quả | Ghi chú |
| --- | --- | --- | --- |
| AC-01 happy path | phase `publication-author`: đăng nhập mã email ×3, viết + chọn thẻ + gửi bằng UI, người duyệt có grant nhận ca và duyệt bằng trang, Khám phá/reader/public read đúng revision + thẻ | PASS | Admin seat cấp bằng CLI (setup); không SQL publication, không response giả |
| AC-02 đúng bản | cùng phase: chữ gõ khi lưu → không chụp, không gửi; `409` nháp ≠ `409` thẻ; snapshot bất biến khi nháp đổi tiếp (người duyệt đọc đúng bản lúc bấm); IME mô phỏng; `publication_state::tests`; PG `author_web_recovery_contract_on_real_rows` | PASS | IME là mô phỏng CDP; OS IME **NOT_RUN** |
| AC-03 hai revision | cùng phase: r1 công khai, r2 chờ rồi bị từ chối, vùng nói r1 vẫn được đọc, đọc công khai vẫn trả r1; `Summary` tests (“đã duyệt” ≠ “đang công khai”) | PASS | r2 *được duyệt* chỉ khi ý định hiện hành còn hợp lệ: bằng chứng ở persistence (`review_withdrawn_page_survives_a_pending_resubmission`, `appeal_on_old_revision_never_overwrites_new_public_intent`), không có kịch bản Web riêng |
| AC-04 retry | cùng phase: chạm đúp (một snapshot, một request), mất ACK thật (response bị hủy *sau khi* máy chủ commit) ở snapshot/request-public/set-private rồi “Kiểm tra lại”; request chưa tới máy chủ → đọc rồi phát lại đúng key; `replayed` thiếu `ticket_id` | PASS | Oracle độc lập: số revision, generation, hàng chờ ca của người duyệt |
| AC-05 hai tab | cùng phase: request chậm của tab A không thắng rút bài của tab B (409 generation, “Gửi lại bản đã chụp” là lựa chọn tường minh, không chụp mới); `409` nháp/thẻ ở bước chụp; reducer: fence epoch/attempt | PASS | Đọc lệch thứ tự được kiểm ở reducer (đọc đơn luồng), không ở trình duyệt |
| AC-06 riêng tư | cùng phase: rút bài khi còn chữ chưa lưu (chữ và trạng thái `unsaved` giữ nguyên, server không có chữ đó), mất ACK rồi đối soát, public read `404`, `If-None-Match` cũ `404` không `304`, Khám phá bỏ thẻ, reader `hidden` chỉ `status`+`work_id`, SSR không tiêu đề/thân | PASS | Bình luận hợp lệ ở trang ẩn: bằng chứng ở phase `review-withdrawn` và PG `hidden_reader_retains_only_discussion_and_republication_restores_body`, không lặp ở đây |
| AC-07 phiên/privacy | cùng phase: đăng xuất thật ở tab khác (`204`), câu trả lời muộn của phiên cũ bị bỏ, chữ chưa lưu ở lại, đổi sang tài khoản khác không còn dữ kiện/chữ của tác giả, `404` + `no-store` cho state của người khác, SSR editor không có state; `contract::tests`, `publication_state::tests` | PASS | `503` và `403` khi đọc state là câu trả lời **dựng có nhãn** (request không tới App); `401`/`404` thật |
| AC-08 phản đối | cùng phase: lý do từ chối + liên kết “Xem lý do và gửi phản đối”, trang `/ban-viet/{id}/kiem-duyet` cùng lời gọi tên và nút phản đối theo `can_appeal` + `decision_version`, gửi hồ sơ qua luồng hiện hữu | PASS | Không có trạng thái “cần sửa”, SLA hay thông báo; `needs_admin` = “Đã chuyển cho người duyệt” |
| AC-09 UX | `matrix`: 5 trạng thái (pending, public-r1+pending-r2, uncertain, conflict, private) × 320/390/1024 × sáng/tối × 100%/200% — không cuộn ngang, không nằm ngoài vùng, không cắt/ellipsis; `refreshInvariant`; focus trả về hành động sau “Đã hiểu” | PASS (phạm vi bên) | Xem bảng chiều và ảnh dưới đây |
| AC-10 handoff | fixture hai phía, tài liệu này, [LayoutSpec](../design/publication-author.md), feature-map | PASS | — |

**Báo riêng từng chiều (phase `publication-author`, Chrome headless, một máy):**

| Chiều | Kết quả |
| --- | --- |
| Functional | PASS — các kiểm tra ở bảng AC; mọi kết quả đọc lại từ server (state, draft, public read, hàng chờ ca), không từ chữ trên trang |
| Geometry | PASS — invariants của LayoutSpec ở 12 ô × 5 trạng thái; mục tiêu chạm của nút/radio ≥ 44 px (liên kết kết quả dùng biến thể Standalone sau vòng 1) |
| Visual | Đã mở và đọc ảnh (320 sáng 200% xung đột, 390 tối public-r1+pending-r2, 320 tối 200% uncertain trước/sau sửa, 1024 sáng private, 390 sáng xung đột). `NO_APPROVED_REFERENCE`; ảnh không là golden |
| Accessibility/Responsive | Responsive PASS như trên. Tên truy cập, vùng `role=status` và thứ tự DOM có ở source/assertion; **chưa kiểm** screen reader, bàn phím-only toàn luồng, reduced motion, thiết bị cảm ứng thật |
| Interaction | PASS cho chạm/nhấp, chống gửi trùng, trả focus sau “Đã hiểu”, đọc lại không cướp focus/cuộn/remount editor. IME chỉ mô phỏng |

**Vòng review ảnh (tối đa hai vòng, đã dùng hết):**

| Finding | Trạng thái |
| --- | --- |
| F-1 Nút “Kiểm tra lại” tràn vùng 19 px ở 320 px/200% (hai lớp padding + nút Filled) | **Fixed** vòng 1 — nút dạng Text như phục hồi của editor, padding thông báo gọn hơn; assertion hình học PASS |
| F-2 Liên kết kết quả cao 26 px < 44 px | **Fixed** vòng 1 — `LinkVariant::Standalone` |
| F-3 Dòng “bản đã chụp chưa gửi” nói “trang có thể đã đóng giữa chừng” sai trong ca xung đột | **Fixed** vòng 1 |
| F-4 Nút “Gửi bản mới” xám không lý do khi có lệnh chưa rõ | **Fixed** vòng 2 — “Kiểm tra lại lệnh trước rồi mới gửi tiếp.” |
| F-5 Câu “…không nằm trong bản đã gửi” sai khi chưa gửi gì | **Fixed** vòng 2 — “…sẽ không nằm trong bản được gửi.” |
| F-6 Ảnh 200% bị cắt dưới vùng | **Fixed** (chiều cao capture) |
| F-7 Ở 320 px/200% nhãn “Kiểm tra lại” xuống ba dòng; cột chữ rất hẹp (~160 CSS px, do khung trang chung với “Thông tin tác phẩm”) | **Still open** — dùng được, không cắt; không có vòng thứ ba |

**Hai lần chạy `verify web` đầu tiên của phase mới** thất bại vì môi trường, không phải hành vi: (1)
`target/` bị xóa giữa chừng bởi một tiến trình ngoài; (2) phase `auth` có sẵn cố ý làm cạn bộ giới
hạn đăng nhập của App (10 mã / 15 phút / mạng, lưu trong database), nên phase thứ hai trên cùng
database nhận `429`. Phase mới nay chạy trên database App riêng (`vink_verify_web_publication`).

## Giới hạn

- IME chỉ mô phỏng bằng `Input.imeSetComposition` (CDP), không phải IME hệ điều hành hay bàn phím
  tiếng Việt thật; chưa kiểm screen reader, thiết bị di động thật hay Safari/Firefox.
- `503`/`403` khi đọc state trong phase là câu trả lời **có nhãn fabricated** (request không tới
  App); mọi câu trả lời khác là thật. Mất ACK là thật: response bị hủy *sau khi* máy chủ commit.
- Reduced motion: vùng không thêm chuyển động; chưa có kiểm tra riêng cho chế độ này.
- Ảnh là bằng chứng của lượt chạy, không phải baseline; `NO_APPROVED_REFERENCE`.
- Chưa đo `Worker` thật (consumer tắt mặc định; evaluator kịch bản chỉ ở local/test) và chưa có
  AI provider, SLA, thông báo.
