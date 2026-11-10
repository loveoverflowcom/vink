# Reader Web: đọc công khai và nhận xét riêng

Lát `DISC-WEB-PUBLIC-OWN-READS`, tiếp nối #144 và #146, nền `develop@4891c77`
(10/11/2026; fetch không có commit mới hơn). COM-02/03/04/07/08, ACC-02, UX-01/03.
Không đổi lifecycle, schema, admission hoặc policy; D-05 vẫn Provisional.

## Quyết định

- **01 A — vùng riêng gọn** “Nhận xét của bạn” ngay sau composer gốc. Giữ timeline
  công khai có nghĩa công khai; own list tới được pending/rejected ngoài trang hoặc dưới root
  không được liệt kê. Loại B (ghép theo chronology) vì cần thêm vị trí/cursor/context cho
  hàng riêng dưới root chưa tải và dễ làm sai `public_replies`.
- **02 B — composed capability seam có giới hạn**: sau hydration gọi composed v1 với
  `limit=1`, chỉ decode `work_id`, `viewer_is_work_author`; bỏ toàn bộ roots ngay tại decoder.
  Public SSR/browser dùng DTO/endpoint public riêng, không chờ hoặc phụ thuộc seam này.
  Loại A (API capability mới) vì provider hiện có đã cấp đủ fact; không cần thêm API/SQL.
  Lookup own của các ID công khai cấp `can_remove`, không suy từ tên hoặc login.
- **03 — generation ở owner hiện có**: `ViewState` giữ epoch public/mutation;
  `OwnState` giữ scope + read generation. Đổi scope dọn private facts và release busy ngay,
  không xếp hàng sau request cũ. Mỗi own/capability read có một child Owner hủy fetch và
  deadline khi read kế tiếp thay thế nó. Write vẫn ở feature Owner: hủy transport không
  chứng minh server rollback. Không thêm store toàn cục hoặc framework state.

## Scope và quyền sở hữu

| State/request | Thuộc | Nhận response khi | Khi lỗi/đổi phiên |
| --- | --- | --- | --- |
| Public roots, replies, cursor, public count | work | work/scope và public epoch còn khớp, không mutation đang chạy | Giữ last success khi lỗi tạm; 404 work xóa public và own; đổi phiên giữ public nhưng fence read cũ và release loading |
| Own list/cursor/lookup | work + Session scope + request generation | ticket còn khớp; đúng work; lookup chỉ nhận ID của batch | Lỗi tạm giữ body đã đọc, bỏ capability; 401/403/404 bỏ private facts. Scope mới sạch, read mới bắt đầu ngay |
| Capability tác giả | work + Session scope + cancellable read owner | scope khớp và chưa có refusal authority | Không xác nhận được thì bỏ author capability; public vẫn hoạt động |
| Remove/mode busy | work + session + public epoch tại begin | scope hiện tại; mỗi scope chỉ một mutation | Scope đổi tăng epoch, release busy/message; ACK cũ không chạm busy mới |
| Draft/native field | người gõ + work (`text_survives` #146) | ACK đúng snapshot/edit generation | Cùng người giữ chữ; khác người bỏ chữ; scope mới không replay lệnh cũ |
| ACK typed ID/status + receipt | phiên của lần gửi | controller scope + physical sequence | Bỏ khi đổi phiên; ID đã biết được lookup, không dò bằng body |
| Public redaction IDs | work + private request scope, chỉ ID đã từng công khai | lookup hiện hành xác nhận held/removed | Scope đổi bỏ cả ID; public page đã scrub giữ nguyên và old reads bị epoch fence. Giữ count là public count |

Lần theo: SSR `PublicReadsAdapter::discussion` → typed public resource → hydrate → public
roots/replies → own list → lookup → submit/ACK → refresh → Session scope đổi → Reader cleanup.
Own/capability chỉ chạy trong `hydrate`; không storage, cache hoặc SSR payload riêng tư.
Lookup dedup rồi chia ≤20 ID; một task await từng batch (concurrency 1). Read mới hủy task cũ.
Không quét public timeline, không một request cho mỗi comment.

## Race và reconciliation

Trên baseline, ba đường lỗi được **tái hiện bằng lần theo source**, không tuyên bố đã chạy
browser baseline: (1) `read_roots` loading → đổi scope → refresh queued → response return ở
scope guard trước release loading; (2) `remove_own`/`change_mode` tương tự bỏ ACK trước release
mutation busy; (3) chỉ controller đổi scope, ViewState còn composed page/replies của viewer cũ.
Regression dùng production `ViewState::change_session` kiểm cả release và epoch; browser giữ
response/ACK thật để kiểm adapter. Không đổi core/model nên không chọn lane discussion/Kani.

List là snapshot sớm hơn lookup trong cùng lượt: list chỉ thêm ID chưa biết; exact lookup
thắng, và ticket cũ không cập nhật ticket mới. Không có ID trong lookup không chứng minh bị gỡ:
giữ last known status, bỏ `can_remove` chưa được xác nhận. Approved ngoài public page vẫn đọc
lại được; khi có public row thì own row biến mất theo ID. Removed không render body/reason;
held/removed của ID từng công khai làm mất body/context ngay, kể cả response public cũ đến sau.
`root_listed=false` chỉ có nhãn trung tính, không lấy tên/text root từ cache.

Mất ACK chưa có ID dùng lại đúng command key/payload; không suy từ text. ACK hợp lệ giữ
`AcknowledgedComment { comment_id, status }` và lookup trạng thái hiện tại. ACK cũ không xóa
chữ mới. Thu composer chỉ đổi presentation, write vẫn thuộc Reader.

## LayoutSpec delta và #129

Kế thừa Reader/R3.1 và Pen Flight, không đổi shared presentation.

```text
#feedback-region
  #comments-mode / #root-composer — giữ native field, focus và receipt hiện hữu
  #own-comments — heading “Nhận xét của bạn”, làm mới và lỗi riêng
    [data-own-comment] — status → nhãn ngữ cảnh trung tính → body → lý do nếu rejected
      Gỡ nhận xét của tôi — chỉ capability server còn được xác nhận
    #own-comments-more — cursor own độc lập
  #root-list / #root-pagination — chỉ projection công khai
```

Loading nằm trên action có status; empty chỉ sau own read thành công; error riêng có retry.
Pending “Chờ duyệt · Chưa công khai”; rejected có lý do riêng; removed không có body hoặc
hành động gỡ. Một cột, cùng measure Reader; wrap ở 320/390/834/1440, sáng/tối và chữ100/200;
body giữ dòng/khổ, không fixed height. Không đổi node input khi refresh hoặc thay theme/size.
Không có overlay/modal mới; thao tác làm mới giữ focus control, thứ tự đọc native.

Phần #129 ở đây: tách public/private, recovery ngoài trang, typed ACK và scope/race fencing.
Clipboard, picker chọn lại context, toàn bộ delta composer R3, OS IME, screen reader và CMP/native
vẫn ngoài lát này. Không đóng #129/#120 hoặc đánh dấu acceptance chưa kiểm.

## Bằng chứng

Kết quả lệnh, SHA và giới hạn của lượt chạy được ghi trong mô tả PR. Nguồn kiểm:

- `discussion_reads::tests`, `separated_read_regressions`, `separated_discussion_tests`,
  `separated_discussion_paths`, controller D2T-03: scope, ownership, privacy wire, batch/cursor,
  dedup, ưu tiên lookup và chống hồi sinh.
- `discussionReadingAndWrites`: LocalFixture + PostgreSQL thật cho lost ACK/keyed retry,
  author toggle, own removal, closed composer; public contract interception cho paging/context.
- `discussionSessionRaces` (Auth Web): App + Identity thật, mã qua local outbox; real pending,
  đổi phiên direct A→B, A→guest→A/B, giữ own read/submit ACK/remove ACK thật. Cookie switching
  bằng CDP dùng hai phiên thật còn hiệu lực; không dựng principal/session response.
- `discussionOwnFixtures`: **intercepted provider fixtures**, không phải database mutation:
  pending ngoài trang, root unlisted, rejected reason, approved ngoài trang, removed; ma trận
  bố cục/input identity/selection 320/390/834/1440 × hai theme × chữ100/200.
- Lỗi riêng `503` trong ca Auth là fabricated và ghi nhãn, public GET vẫn thật.

Không claim production authentication, mail thật, HTTPS/domain thật, OS IME hoặc screen reader.
