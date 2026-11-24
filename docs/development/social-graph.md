# Social Graph · truy vết và bàn giao #288

[PRD §15](../../prd/requirements.md#15-hồ-sơ-tác-giả-và-theo-dõi-social-graph) sở hữu các yêu cầu
từ R1–R9 theo R10 §2. [Hợp đồng liên miền r1](../architecture/social-graph.md) sở hữu chuyển
trạng thái và quyền. [Hợp đồng HTTP r1](../../services/vink-app/contracts/http/v1/social-graph.md)
là đầu vào chung cho API, Design và Web. Các mã SOC-INV và SOC-AC giữ nguyên nghĩa trong
[§7 và §10 của Epic](https://github.com/loveoverflowcom/vink/issues/284).

### Revision registry dùng khi bàn giao

Query ở base `e0551edf` cho snapshot `3f7687488cdb5906a2850dea4b363a67ba8bf4e61b0a781908b7486f99c8c11c`.
Sau khi thêm ba proposal, snapshot của r1 là `43865e626ee7ce88e8f453140666e10d3b5f3ecf5fe6c52d7a44b6887300fb22`.
Hash dưới đây là SHA-256 của payload do CLI xuất, dùng để review đúng revision.

| Revision | Trạng thái | SHA-256 |
| --- | --- | --- |
| USER-01@r1 | DEFERRED | `2c982184d1470d7d7163556a5e8e27936842689647f77cbfe90b6a51d51fcec5` |
| USER-01@r2 | PENDING | `6b02829e85d36b66d36f55b1a72c55f07142397cfed82ff5fa78fbc4d1280163` |
| VDR-SOCIAL-CONTRACT@r1 | PENDING | `a514d2c8d78873eb5fee4a935623e5ec4e6c8e6d5d7ca5141124defb60f0c005` |
| VDR-SOCIAL-GRAPH@r1 | PENDING | `bdafefdcb99524bd09de5b872292efaa63cf8136a271b629714c9bb190380595` |

## Từ yêu cầu đến bất biến

| Bất biến | Yêu cầu PRD | Owner chịu trách nhiệm và điều phải giữ |
| --- | --- | --- |
| SOC-INV-01 | SOC-ID-01..06 | Principal giữ binding và ownership qua đổi credential, bút danh và Handle. |
| SOC-INV-02 | SOC-HDL-01, SOC-LIFE-02 | Profile giữ duy nhất khóa tên giữa current/alias/tombstone, resolve đúng chủ. |
| SOC-INV-03 | SOC-HDL-01 | Profile giữ quota qua duplicate, no-op, conflict, rollback và mất ACK. |
| SOC-INV-04 | SOC-FOL-01 | Graph cấm self-follow, tối đa một cạnh có hướng/cặp, count bằng cạnh hợp lệ. |
| SOC-INV-05 | SOC-ID-04, SOC-PRIV-01, SOC-FOL-01, SOC-LIFE-01 | App kiểm actor, capability, standing và validity của đích ở server, kiểm lại tại commit. |
| SOC-INV-06 | SOC-PRIV-01, SOC-FOL-01, SOC-FIND-01 | App chỉ serialize facts có quyền. Private facts tách theo phiên, public projection độc lập viewer. |
| SOC-INV-07 | SOC-PRIV-01, SOC-FEED-01, SOC-LIFE-02 | Owner đường đọc kiểm quyền hiện tại trước validator. Web bỏ dữ liệu vừa bị thu hồi và ACK/response cũ. |
| SOC-INV-08 | SOC-FEED-01 | Publication cấp snapshot hiện hành và mốc đầu bền. Discovery dùng duy nhất một dòng/WorkId đúng thứ tự. |
| SOC-INV-09 | SOC-HDL-01, SOC-FOL-01 | Owner của lệnh giữ thứ tự đã nhận và kết quả để reconcile. Retry cũ không đảo ý định mới. |
| SOC-INV-10 | SOC-LIFE-02, SOC-ID-03 | Account điều phối xóa. App kết thúc cạnh hai chiều, giữ tombstone, dừng public identity link và đối soát số đếm. |
| SOC-INV-11 | SOC-LIFE-01 | App kiểm capability hạn chế nội dung riêng với full suspension/Identity disable. |
| SOC-INV-12 | COM-06, UX-04, ACC-01..03, PRD §15 | Từng fact có một owner, theo bảng ranh giới liên miền. PRD/registry/spec/Web/evidence truy ngược cùng revision. |

## Tiêu chí và phép kiểm theo ranh giới

Mỗi dòng nối với SOC-INV ở bảng trên qua số cuối. Đây là **kế hoạch kiểm**. Tại #288
chỉ kiểm tài liệu, TOML, liên kết và dữ liệu minh họa. Runtime Social và model tương ứng
là `NOT_RUN`. Khi nhận lát, ghi command, fixture, SHA thực chạy, kết quả và giới hạn vào
feature-map/PR của owner. Với state machine cần model, thêm tại owner và replay intent qua
production seam. Race bền vững cần PostgreSQL thật.

| AC của Epic | INV | Owner / lát | Phép kiểm phân biệt kết quả |
| --- | --- | --- | --- |
| SOC-AC-01 | 01, 02, 05 | Principal #290 | PG hai provision cùng subject + backfill hai lần, đếm đúng một Profile/Handle, không tạo cho principal kỹ thuật. |
| SOC-AC-02 | 01, 02 | Profile #290 | Pure generator với collision/exhaustion, không dùng credential. PG từ chối trùng sau chuẩn hóa xuyên cả ba trạng thái. |
| SOC-AC-03 | 03, 09 | Profile #290 | Hai tab cùng version, cả hai cách setup, no-op, lỗi, mất ACK và retry. Đọc lại đúng quota và receipt. Reclaim theo OPEN-03. |
| SOC-AC-04 | 02, 07, 10 | Resolver #290/#297 | HTTP + browser qua hai rename rồi delete với 301 đã dùng. Kiểm Location cùng origin, query, fragment, không vòng, không tái cấp. |
| SOC-AC-05 | 01, 10 | Profile → Web #293 | Đổi bút danh, đọc card/Reader/root/reply/context, kể cả hai người cùng tên. Đúng current ref, hash revision và quan hệ giữ nguyên. |
| SOC-AC-06 | 05, 06 | Profile #292 | Bảng test tám mặc định, đổi một trường rồi so bảy trường khác. Bút danh luôn public. |
| SOC-AC-07 | 06, 07 | Profile #292 / media #298 | So hai hồ sơ có/không có cùng trường self qua API, SSR/no-JS, hydration, head, tìm/gợi ý, cursor. Kiểm bytes URL cũ/thumbnail nếu có upload. |
| SOC-AC-08 | 06, 07 | App/Web #292 | Hai request hai phía của commit everyone→self, validator cũ, tab khác và response muộn sau invalidation. Không phục hồi giá trị đã thu hồi. |
| SOC-AC-09 | 06, 07 | Profile/Graph #292/#294 | Dùng hai phiên, đổi A→B→A rồi thử list public/count self và ngược lại. Không lộ total/phiên trước, không giả 0 khi bị ẩn hoặc lỗi. |
| SOC-AC-10 | 05, 06 | Profile #292 | Kiểm gate USER-01 và OPEN-01. Khi đủ gate thử bỏ trống, giữ self, hủy xác nhận, công khai đầy đủ khi chủ chấp thuận. |
| SOC-AC-11 | 04, 05, 09 | Graph #294 | PG/HTTP từ chối self/sai actor. Follow mất ACK → Unfollow → retry cũ, cạnh vẫn tắt, không đếm đôi khi đua. |
| SOC-AC-12 | 04, 10 | Graph #294/#297 | Đối chiếu count và list độc lập sau duplicate, rollback, unfollow, delete hai chiều. Rebuild nếu có counter. |
| SOC-AC-13 | 04, 06 | Graph #294 | Hai đầu cạnh nhận Mutual trong view riêng. Người thứ ba/SSR public không nhận Mutual dù cả hai list công khai. |
| SOC-AC-14 | 01, 04, 05 | Profile/Graph #290/#294 | Tạo tác giả không bài, tìm đúng người, đọc hồ sơ và Follow thành công. |
| SOC-AC-15 | 05, 11 | Account/App #294/#297 | Khi có capability DEP-02, so quyền hạn chế nội dung với full suspension/disable ở HTTP và commit bị chặn sau khi chờ khóa. |
| SOC-AC-16 | 04, 08 | Discovery #296 | PG query plan trên tác giả có nhiều bài, gồm bài trước Follow. Kiểm lọc trước LIMIT, unique WorkId, snapshot mới, số query hữu hạn. |
| SOC-AC-17 | 08 | Publication #291 | Hai writer đua lần đầu, rút/thay revision/duyệt lại/restore. Mốc exact giữ nguyên, legacy có provenance, unknown không thành exact khi tái đăng. |
| SOC-AC-18 | 05, 07, 08 | Feed #296 | Giữa hai trang rút/private/xóa/unfollow, thử cursor cũ/sai scope và response muộn. Đọc mới cùng Back chỉ chứa bài còn quyền. |
| SOC-AC-19 | 06, 12 | Discovery #295 | Cố định dữ liệu public rồi thay cạnh/field private và kiểm tra candidate, score, thứ tự và bytes public bất biến. Hồ sơ không bài có row trung tính. |
| SOC-AC-20 | 02, 04, 05, 09, 10 | Social lifecycle #297 | PG race delete ↔ Follow/rename. Crash và retry từng bước, đối chiếu cả hai hướng, quota, tombstone. Đóng AC ở Epic còn cần DEP-01. |
| SOC-AC-21 | 01, 07, 10 | Account #297 | E2E Identity/App hai DB, phiên cũ, URL tác phẩm/alias, comment/reply context, search và cache. Chỉ còn đúng projection sau xóa. |
| SOC-AC-22 | 06, 07, 12 | Web #289 và từng child UI | Browser 320/390/834/1024/1440, hai theme, chữ 100/200%, phím/focus/IME/a11y. Thử SSR không JS, pending→empty/error, Reader→Back. |
| SOC-AC-23 | 12 | Từng child, Epic #284 | Đối chiếu SHA, fixture, lệnh với boundary pure/model/PG/HTTP/browser. Nêu phần chưa chạy. |
| SOC-AC-24 | 12 | PRD/Design #288/#289 | So roadmap PRD §15 với UI dự kiến. Không có placeholder cho tính năng ngoài Epic hay claim native từ preview. |
| SOC-AC-25 | 12 | SSOT #288, Design #289 | Kiểm PRE-01..08 qua PRD, query registry, owner/contract, LayoutSpec và feature-map. PRE-04 thuộc #289, PRE-07 theo lần tích hợp. |
| SOC-AC-26 | 01, 02, 03, 12 | PRD/registry #288 | Tra USER-01 --history và proposal/hash riêng. R9 giữ Identity, ASCII, một setup + hai đổi. Phần còn lại của age policy vẫn hoãn. |
| SOC-AC-27 | 06, 07, 12 | Design #289 | So 9 bề mặt với projection, state, viewport, focus và Interaction của revision tích hợp. Ghi phạm vi mock/đã chọn. |
| SOC-AC-28 | 12 | SSOT #288 | Đối chiếu issue thật, owner, base r1, phụ thuộc và gate cục bộ trong bảng dưới. |
| SOC-AC-29 | 01, 06 | Tìm tác giả #295 | Exact Handle, tên tiếng Việt trùng, alias theo OPEN-05, tác giả chưa có bài/vừa xóa, page tiếp. Kiểm hành trình tìm và Back. |
| SOC-AC-30 | 02, 03, 07, 09, 10 | Từng writer/reader | Chạy bản cũ và mới cùng lúc, dừng rollout sau revoke/rename/unfollow. Alias, quota, cạnh, receipt, fact và privacy vẫn đúng sau restart. |

## Bàn giao theo issue

Mọi child bên dưới tiêu thụ **contract r1 của #288**. Khi gộp, ghi SHA tích hợp thực tế trên
develop cùng revision thiết kế của bề mặt. PRE-01/05/08 có discovery, issue và kế hoạch ở
Epic. #288 bàn giao PRE-02/03/06. PRE-07 được đáp ứng theo từng lát sau khi contract và
các phụ thuộc tương ứng đã tích hợp. PRE-04 và bằng chứng UI thuộc #289/child.

| Child và owner đầu ra | Đầu vào ngoài r1 | Gate cục bộ / bằng chứng cần bàn giao |
| --- | --- | --- |
| [#289 · Design](https://github.com/loveoverflowcom/vink/issues/289) | LayoutSpec r2, hướng điều hướng preview v4 đã chọn ở #289 | Chín bề mặt dùng ma trận quyền/lỗi ở HTTP r1. Kiểm nguồn/cỡ chữ. LayoutSpec chưa có trong base #288. |
| [#290 · Principal/Handle](https://github.com/loveoverflowcom/vink/issues/290) | #289, preview Handle r2 đã duyệt | OPEN-02 đã chốt, OPEN-03 chỉ chặn reclaim. CHOICE-01 cần PG + HTTP/browser 301. |
| [#291 · Publication](https://github.com/loveoverflowcom/vink/issues/291) | Preview r2 (#291), độc lập với UI | CHOICE-04 đã có [fact PostgreSQL theo WorkId và view chỉ nhận exact còn công khai](publication-runtime.md#mốc-công-khai-đầu-tiên-291). Fixture kiểm writer, seed, backfill và rollback. #296 nhận mốc cùng revision/metadata hiện hành. |
| [#292 · Profile](https://github.com/loveoverflowcom/vink/issues/292) | #290, LayoutSpec vùng Tôi/Profile | Public ref, DTO và quyền tám trường. OPEN-01/04 và USER-01 chỉ giữ phần tương ứng. CHOICE-05 kiểm HTTP trước 304 + browser. |
| [#293 · Attribution](https://github.com/loveoverflowcom/vink/issues/293) | #292, ref công khai | Card, Reader, comment và reply context cùng đúng ref. Kiểm fixture provider/consumer, SSR và hai link độc lập. |
| [#294 · Graph](https://github.com/loveoverflowcom/vink/issues/294) | #292, policy list/count | CHOICE-02/03 cần race/receipt thật, đối soát và query plan. DEP-02 chỉ giữ claim hạn chế nội dung. |
| [#295 · Tìm/gợi ý](https://github.com/loveoverflowcom/vink/issues/295) | #292, thêm #294 cho nút/lọc theo viewer | OPEN-05/06 chọn A. So bytes public, pagination và query plan trên hai viewer. |
| [#296 · Following Feed](https://github.com/loveoverflowcom/vink/issues/296) | #291 + #293 + #294 | Chỉ nhận exact khi mở thử. CHOICE-05/06 kiểm query, cursor, đổi phiên, graph và Back. |
| [#297 · Lifecycle](https://github.com/loveoverflowcom/vink/issues/297) | #293 + #294, DEP-01 từ Account/Identity | Kiểm Social fence và resume cục bộ. E2E xóa cần cả hai DB, old session, Publication và Discussion. |
| [#298 · Media](https://github.com/loveoverflowcom/vink/issues/298) | #292, chỉ khi OPEN-04 chọn upload | Chứng minh quyền đến URL cũ/bytes/variants/validator sau revoke, thay ảnh và xóa. |

Ở Epic, ghép signup → hồ sơ → Follow → Feed → thu hồi với fixture và SHA cuối. Phạm vi chứng
cứ của preview #289/#290/#291 là bản thiết kế đã ghi trong từng issue.

## Địa chỉ tác giả #290

Tôi → **Chỉnh hồ sơ** → **Địa chỉ tác giả** mở hộp thoại theo [#480](../design/profile-inline-handle-480.md).
`/toi/dia-chi` chỉ chuyển hướng tới hàng đó. Tài khoản đã có binding nhận hồ sơ ngay khi
lưu bút danh. Xác nhận tên gợi ý hoặc tên tự nhập đều kết thúc thiết lập và giữ hai lượt
đổi. Khi mất phản hồi, Web giữ nguyên tên đã gửi, tra receipt rồi đọc lại địa chỉ và số lượt
hiện hành. Phiên khác không thể dùng principal trong payload để chọn chủ của lệnh.

| Chiều | Trạng thái và nguồn |
| --- | --- |
| Sản phẩm | SOC-ID-02/04/05/06, SOC-HDL-01 và SOC-OPEN-02 theo #288, #290. SOC-OPEN-03 chỉ còn quyền chính chủ tái dùng alias. |
| Kiến trúc | CHOICE-01 trong phạm vi #290 dùng một registry tên bất biến và một con trỏ hiện hành cho mỗi PrincipalId. VDR-SOCIAL-CONTRACT@r1 vẫn PENDING. |
| Triển khai | `core`, `persistence-api`, `web`. [Luật tên](../../services/vink-app/src/domain/handle.rs), [giao dịch và resolver](../../services/vink-app/src/persistence/handle.rs), [hợp đồng HTTP](../../services/vink-app/contracts/http/v1/social-graph.md#địa-chỉ-tác-giả-290), [controller Handle](../../apps/vink-web/src/pages/profile/browser/handle.rs), [public resolver Web](../../apps/vink-web/src/pages/handle.rs). |
| Kiểm chứng | [PG và HTTP](../../services/vink-app/tests/postgres/handle.rs), [đua provision](../../services/vink-app/tests/postgres/principal.rs), [Chrome với phiên thật](../../apps/vink-web/e2e/author-handle-smoke.mjs). Lệnh tương ứng là `verify postgres` và `verify web --scenario password`. [Báo cáo #290](evidence/author-handle-290/README.md) ghi SHA, kịch bản, ảnh Chrome và số đo accessibility. |

**Cấp tên và nâng schema.** Chạy migration bổ sung, chuyển sang writer mới, rồi chạy
`vink-app principal backfill-handles` bằng role runtime App. Mỗi batch khóa tối đa
100 principal theo UUID. Các hồ sơ đã commit là checkpoint cho lần chạy lại. Báo cáo
gồm `created`, `existing`, `attempts`, `unbound` (kể cả seed/fixture kỹ thuật) và
`invalid` (binding có bút danh cần xử lý). Bút danh cũ được trim/NFC qua cùng parser.
Mỗi lần cấp thử tối đa 16 tên. Địa chỉ chỉ mở đọc sau đối soát binding đủ điều kiện.
Writer cũ còn hoạt động mà tạo binding thiếu hồ sơ sẽ đóng lại cổng đọc đến lần backfill
kế tiếp. Khi quay lui phiên bản ứng dụng, giữ nguyên migration, reservations, quota và receipts.

**Bàn giao lifecycle.** Dùng [Social adapter #297](#social-adapter-thu-hồi-tài-khoản-297)
để ghi fence bằng operation ID đã xác minh. Adapter áp dụng cả với binding legacy chưa có
hồ sơ tác giả.

## Hồ sơ và bút danh #292/#293

- **Sản phẩm.** Các mặc định và ownership SOC theo #288. `USER-01@r1` còn `DEFERRED`,
  `USER-01@r2` còn `PENDING`. SOC-OPEN-01 giữ phần công khai lại một trường, SOC-OPEN-04
  giữ phạm vi giá trị Avatar và ảnh. Giao diện công khai dùng dấu chữ hiện có.
- **Kiến trúc.** CHOICE-05 dùng SSR công khai và reads riêng sau hydrate vì cả Reader và
  Tôi đã có hai ranh giới này. App chiếu một snapshot cho API và SSR. Version hồ sơ riêng
  bao bọc Bio, tám policy và bút danh. Giao dịch lấy khóa Principal trước, cùng fence dùng
  bởi rename Handle, suspension và thu hồi Profile. VDR-SOCIAL-CONTRACT vẫn `PENDING`.
- **Triển khai.** `core`, `persistence-api`, `web` cho phần đã chốt. Kho App sở hữu
  [ghi/đọc Profile](../../services/vink-app/src/persistence/profile.rs) và
  [projection attribution](../../services/vink-app/migrations/20261125010000_public_author_attribution.sql).
  Mọi tên hiển thị lấy từ `principals.display_name`. #294 có thể dùng
  [ProfilePolicies](../../services/vink-app/src/domain/profile.rs) để đánh giá list/count
  độc lập. [Hợp đồng và fixtures](../../services/vink-app/contracts/http/v1/social-graph.md#hồ-sơ-và-bút-danh-292293)
  định nghĩa cả projection công khai lẫn phiên bản được chủ sửa.
- **Web.** [Tôi → Chỉnh hồ sơ](../../apps/vink-web/src/pages/profile.rs) giữ nháp trong RAM
  của generation đang xác nhận. Scope riêng dừng ngay khi nghe dấu hiệu đổi cookie. Sau lost
  ACK, đọc trạng thái đúng scope rồi so version và payload. [Hồ sơ công khai](../../apps/vink-web/src/pages/handle.rs)
  đưa Bio và danh sách bài vào cùng read. Focus, Back và thông báo đổi hồ sơ làm mới quyền
  đọc. Generation của read loại response đã cũ. READ-02 tính thêm byte của attribution,
  ẩn bút danh ở snapshot chưa xác minh, kiểm lại ngay khi snapshot có liên kết hồ sơ,
  làm hết hiệu lực response đang bay và gỡ attribution khỏi snapshot RAM khi nhận đổi hồ sơ.
  HTML Khám phá và Reader dùng
  `no-store` để native Back đọc lại projection. History của tab giữ scope và đích focus,
  chờ byline đáp ứng quyền hiện tại. Cổng IndexedDB vẫn đóng.
- **UI.** [LayoutSpec Social r2](../design/social-289.md) là nguồn của quyền, trạng thái và
  hành trình. `fidelity: NO_APPROVED_REFERENCE`. [Ghi chú Khám phá r1](https://drive.google.com/file/d/1HH4FOCWNHjLbsJJ4Mhpv_f14qhhOXvb0/view)
  bổ sung phép kiểm bàn phím và Back trong query/tag trên WorkCard.
  [Focused browser test](../../apps/vink-web/e2e/author-profile-smoke.mjs) chạy trong
  `verify web --scenario password` và seed hai tác giả bằng phiên thật cùng corpus bài/bình luận
  kiểm soát trong database tạm. [Bằng chứng #292/#293](evidence/author-profile-292-293/README.md)
  ghi kết quả theo từng AC, ảnh và phiếu thử tay.

### Mở và quay lui

Triển khai hai migration bổ sung trên schema App đã qua #290. Dùng App có ledger hiện hành
để phục vụ các route và projection. Khi tạm dừng tính năng, giữ columns, dữ liệu, version,
reservation và receipts. Có thể ẩn form/link hoặc ngừng phục vụ projection mở rộng trong bản
App vẫn kiểm quyền mới. Giữ authz và redaction tại provider cùng header revalidate. Tiếp tục
xóa các snapshot đã được biết là cũ. Build trước migration không qua readiness trên ledger
mở rộng, và provider byline cũ có thể phục vụ tên trước thu hồi nếu cố chạy lại. Backout an
toàn dùng bản có cả migration lẫn redaction, ưu tiên tắt entry form/links.

## Theo dõi tác giả và danh sách quan hệ #294

- **Sản phẩm.** `SOC-FOL-*` / `SOC-REL-*` theo #288 và #294: chủ động Follow/Unfollow theo desired state (`expected_order` + `command_id`), tách riêng bốn quyền đọc `followers_list`, `followers_count`, `following_list`, `following_count`, Mutual chỉ lộ cho đúng hai chủ thể trong cặp, không self-follow/blind toggle. `SOC-DEP-02` (tách hạn chế đăng/bình luận khỏi full suspension) vẫn là phụ thuộc ngoài thuộc Account/Identity.
- **Kiến trúc.** `CHOICE-02 = A`: lưu bảng cặp bền `author_follows` (trigger giữ hàng, parity `active == ((version % 2) = 1)`, tăng đơn điệu `accepted_order`) kèm ledger `author_follow_receipts` append-only. `CHOICE-03 = A`: tính live aggregate qua partial index `WHERE active` và join `author_profiles.current_handle IS NOT NULL` trong snapshot `REPEATABLE READ`. Thứ tự khóa chuẩn hóa theo cặp `PrincipalId` tăng dần giúp `set_follow` hai chiều và `persistence::handle::retire` (`retire_edges`) không deadlock. `VDR-SOCIAL-GRAPH@r1` và `VDR-SOCIAL-CONTRACT@r1` vẫn ở trạng thái `DESIGN_PROPOSED` / `PENDING`.
- **Triển khai.** `core` ([domain/follow.rs](../../services/vink-app/src/domain/follow.rs)), `persistence-api` ([20261126000000_author_follows.sql](../../services/vink-app/migrations/20261126000000_author_follows.sql), [persistence/follow.rs](../../services/vink-app/src/persistence/follow.rs), [http/follow.rs](../../services/vink-app/src/http/follow.rs)), và `web` ([follow_state.rs](../../apps/vink-web/src/follow_state.rs), [pages/follow.rs](../../apps/vink-web/src/pages/follow.rs), route `/@{handle}/nguoi-theo-doi` và `/@{handle}/dang-theo-doi`, `/toi?next=…`). Bàn giao seam batch membership (`followed_in_tx`, `followed_principals_in`, `GET /api/v1/me/follows/membership`) cho #295/#296 và hook `retire_edges`/`retire_edges_batch` trong `handle::retire` cho #297.
- **Kiểm chứng.** [Bằng chứng #294](evidence/author-follow-294/README.md) ghi lệnh và SHA, PostgreSQL thật, Web password với Follow/Profile/Handle/Auth, ma trận responsive và bốn ảnh đã xem. Cursor danh sách riêng bị ràng buộc theo phiên; response cũ không vượt rào A→B→A. Quyền hiện hành được kiểm trước cursor, HTTP validator và hiển thị danh sách/số đếm.

## Đang theo dõi trong Khám phá #296

- **Lựa chọn triển khai.** CHOICE-05 dùng SSR độc lập với actor và tải Feed riêng
  sau hydrate, cùng hướng với #292. CHOICE-06 dùng membership hiện hành ở từng snapshot đọc.
  Cách này dùng index/cạnh có sẵn, tránh cập nhật graph revision cho mọi writer. Cạnh mới
  có thể đưa bài vào phía trên cursor cũ. Refresh và ACK Follow được xác nhận mở lại trang
  đầu. Publication giữ mốc `exact` theo #291. [Hợp đồng r1](../architecture/social-graph.md)
  và LayoutSpec Social r2 tiếp tục theo trạng thái ghi trong registry.
- **PostgreSQL và HTTP.** [Provider](../../services/vink-app/src/persistence/feed.rs)
  lấy WorkId từ view Publication, lọc tác giả/cạnh/readable pointer trước `ORDER BY/LIMIT`
  và đọc nhiều nhất 13 ID/trang. Chỉ dựng trích đoạn sau khi chọn ID; thẻ dùng cùng projection
  và các query batched của Discovery. Các lượt đọc dùng `REPEATABLE READ, READ ONLY`.
  [Hợp đồng Feed](../../services/vink-app/contracts/http/v1/social-graph.md#đang-theo-dõi-296)
  ghi giới hạn batch, thứ tự, lý do restart và quyền tại lần đọc.
- **Web và #295.** Khám phá tiếp tục ở `/`. `ExploreMode` và trình chọn mode ở
  [route owner](../../apps/vink-web/src/pages/discovery.rs) định tuyến `Works` và
  `Following` (`/?mode=following`). [Module Feed](../../apps/vink-web/src/pages/discovery/feed.rs)
  sở hữu status, owner phiên và append. Thêm mode Tác giả ở route chung bằng module và
  query/cursor riêng. Thay liên kết “Khám phá tác phẩm” của trạng thái chưa theo dõi ai
  bằng đích Tác giả khi #295 có route thật. Feed v1 giữ query cố định và mẫu WorkCard
  Lavender R3. Tìm kiếm/lọc thẻ tiếp tục ở mode Tất cả.
- **Vòng đời.** `ReadOwner` và `PageChain` xét ticket theo generation phiên. Khi append,
  revalidate các thẻ đã hiển thị và trả lại projection còn hợp lệ. Khi nhận ACK Follow,
  xóa chain và đọc lại. Wake/Back đọc lại từ server. Mỗi history entry giữ anchor
  công khai và dấu điều hướng mờ của phiên, không giữ thẻ hay cursor. Sau lần đọc
  được chấp nhận, Back trả về link nhan đề hoặc bút danh còn hợp lệ, kể cả qua
  document mới. Khi anchor mất, focus về tiêu đề “Đang theo dõi” (H2). Lỗi mạng
  giữ ý định quay lại để thử đọc mới. Thay đổi Follow trong lúc chờ focus cần một
  lần đọc mới. Thay phiên loại dữ liệu và kết quả đang bay; dấu cũ mất hiệu lực.
  Khi Back nhanh đổi entry trước hydrate, Web đối chiếu mã băm tuyến SSR rồi tải
  đúng địa chỉ đang mở.
  [Bằng chứng Web #355](evidence/following-feed-355/README.md) đi qua Feed, Reader và hồ sơ.
- **Quay lui.** Đặt `VINK_FOLLOWING_FEED=disabled` rồi khởi động lại App. Link mode ẩn,
  đường dẫn đã lưu có trạng thái tạm tắt và lối về Khám phá. Route API trả 404.
  Giữ migration index, fact Publication và cả cạnh/receipt. Chuyển lại `enabled`
  rồi khởi động lại để mở Feed từ dữ liệu hiện hành.

## Tìm và gợi ý tác giả #295

- **Sản phẩm.** SOC-OPEN-05/06 chọn A theo [#295](https://github.com/loveoverflowcom/vink/issues/295).
  Mode **Tác giả** riêng trong Khám phá. Kết quả lấy bút danh/Handle hiện hành. Handle cũ
  dùng resolver khi mở URL. Gợi ý và thứ tự dùng chung, nút Theo dõi cập nhật theo phiên.
- **Kiến trúc.** Dùng Handle công khai làm khóa sắp trang. Dùng hai nhánh prefix rời nhau
  theo Handle và bút danh gập dấu. Mỗi nhánh lấy tối đa một trang cộng một và kết quả
  chung cũng giữ giới hạn này. Hai index dùng projection hiện hành của Profile.
  `VDR-SOCIAL-CONTRACT@r1` giữ trạng thái `PENDING`.
- **Triển khai.** [Provider](../../services/vink-app/src/persistence/author_discovery.rs),
  [hợp đồng HTTP](../../services/vink-app/contracts/http/v1/social-graph.md#tìm-và-gợi-ý-tác-giả-295),
  [Web](../../apps/vink-web/src/pages/author_discovery.rs). Kết quả và nút Theo dõi có
  các request riêng. Lỗi tải thêm giữ hàng đang hiển thị. History lưu vị trí công khai đã
  mở và Web đọc lại trang trước khi phục hồi focus. Có thể ẩn mode và ngừng phục vụ
  đường đọc mới cùng lúc, giữ nguyên bảng tên, tombstone và các giới hạn Profile.
- **Kiểm chứng.** [PG](../../services/vink-app/tests/postgres/author_discovery.rs) so bytes
  công khai sau thay đổi Bio riêng và quan hệ hai chiều, kiểm NFC/NFD, Handle, alias và
  truy vấn có dữ liệu lệch. [Chrome](../../apps/vink-web/e2e/author-search-smoke.mjs) có
  scenario `cargo xtask verify web --scenario author-search` dùng App và Identity scratch.
  Giá trị ngày sinh chưa có trong runtime. Scope Chrome IME dùng sự kiện mô phỏng.

## Trình bày tìm tác giả #354

- **Web.** Tiêu đề tìm kiếm hiển thị truy vấn đã gửi. Gợi ý có chú thích về danh sách công
  khai dùng chung. Truy vấn bút danh tô phần khớp trong tên và truy vấn exact `@handle`
  tô Handle. Kết quả rỗng có lối tìm cùng từ khóa trong tác phẩm và xóa tìm kiếm.
  Chờ lần đầu, lỗi lần đầu và lỗi tải thêm có phản hồi riêng trong vùng kết quả.
- **Phiên và Back.** Lớp hydrate dùng Handle từ một lượt đọc hồ sơ của phiên hiện hành
  cho mỗi vùng tìm tác giả đang mở để hiện “Bạn” ở hàng chính mình. Khi revalidate
  trên đường Back và hàng đã mở đã mất, Web thông báo cập nhật danh sách rồi đặt
  focus ở tiêu đề vùng kết quả.
- **Kiểm chứng.** [Ma trận Chrome trước/sau, số đo và AX tree](evidence/author-search-354/README.md).
  Các khung 295-01..04 có trạng thái `DESIGN_PROPOSED / NO_APPROVED_REFERENCE`.

## Social adapter thu hồi tài khoản #297

- **Sản phẩm.** SOC-LIFE-02 yêu cầu thu hồi mọi địa chỉ và quan hệ của tài khoản đã
  xác minh xóa. SOC-DEP-01 thuộc Account/Identity và vẫn là phụ thuộc ngoài. SOC-DEP-02
  giữ nguyên phạm vi. Phân loại full suspension tiếp tục dùng guard hiện hành.
- **Kiến trúc.** [Adapter App](../../services/vink-app/src/persistence/social_retirement.rs)
  nhận PrincipalId đã binding và operation ID ổn định qua retry. Transaction thứ nhất
  khóa Principal trước Profile, ghi fence và bỏ con trỏ Handle. Từ commit này, các alias
  và Handle hiện hành trả 410. Count/list loại cạnh theo Profile, kể cả khi batch dọn
  cạnh chưa chạy. Với principal legacy chưa có Profile, fence che attribution và chặn
  backfill cấp tên. Transaction batch khóa checkpoint và cạnh theo cặp có thứ tự,
  tối đa 100 cạnh mỗi lượt và ghi hoàn tất trong cùng commit với batch cuối.
- **Tích hợp và quay lui.** Account tiếp tục cùng operation ID sau lỗi hoặc mất ACK.
  Existing binding, lịch sử order/receipt và reservation Handle tồn tại qua các lượt
  dọn. Có thể dừng gọi adapter giữa hai batch và tiếp tục từ checkpoint. Duy trì schema,
  fence và projection có redaction khi quay lui. `Complete` biểu thị hoàn tất Social
  Profile/Graph trong App. Luồng xác minh, revoke phiên, Publication, retention và
  điều phối hai DB thuộc Account/owner tương ứng.
- **Kiểm chứng.** [Fixture adapter trên PostgreSQL](../../services/vink-app/tests/postgres/social_retirement.rs)
  kiểm fence trước cleanup, fault trong batch, restart, replay, alias và ghi đua.
  [Feed và tìm tác giả](../../services/vink-app/tests/postgres/feed.rs) đọc lại sau fence
  trong khi cạnh còn active trong bảng. [Reader và Discussion](../../services/vink-app/tests/postgres/discussion.rs)
  kiểm validator cũ, attribution và ngữ cảnh trả lời.
  Account E2E và SOC-AC-20/21 cấp Epic ở trạng thái `NOT_RUN` cho đến khi có SOC-DEP-01.
