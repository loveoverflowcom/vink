# Nghiệm thu hồ sơ và bút danh #292/#293

Base lúc bắt đầu là `develop@a71ccd14b0612111cb8db53aecf80f7f67021847`.
Phạm vi theo [#292](https://github.com/loveoverflowcom/vink/issues/292), [#293](https://github.com/loveoverflowcom/vink/issues/293),
[hợp đồng Social](../../../../services/vink-app/contracts/http/v1/social-graph.md#hồ-sơ-và-bút-danh-292293) và
[LayoutSpec Social r2](../../../design/social-289.md).

## Ranh giới và dữ liệu kiểm

- macOS 15.6 arm64, Rust 1.99.1, wasm-bindgen 0.2.129, Node 22.15.0 và Chrome 157 headless.
- PostgreSQL 17.11 trên cluster test riêng tại `127.0.0.1:56429`, data directory `/tmp/vink-profile-test-db/data`.
  App và Identity của Chrome dùng database scratch và role runtime riêng, fixture tạo hai phiên password,
  hai tác giả trùng bút danh dài, hai bài đã công khai và một thread có phản hồi cho lời đáp khác.
- Test PostgreSQL dùng database cô lập cho từng test. HTTP qua Axum router thật với resolver test cho
  owner, guest và other. Chrome chạy App và Identity native, đăng nhập thật, bấm trên route sản phẩm.
  Fixture dựng dữ liệu công khai và thu hồi Handle của một tác giả để kiểm projection ở ranh giới Web/App.
- #297 giữ điều phối xóa xuyên Account và App. #298 giữ giá trị ảnh và đường phục vụ bytes.
  Ma trận Chrome dùng cỡ chữ của phần tử gốc, thao tác chạm CDP và `CompositionEvent`.

## Vết chạy

| Lệnh | SHA thực chạy | Kết quả và artefact |
| --- | --- | --- |
| `./tools/local-budget run -- cargo xtask verify unit app --filter contract` | `3168b25c7b7c8d2d3bf658b5e9e2464e045380b7` | `PASS`, 27 App. [Trích log unit](unit.txt). |
| `./tools/local-budget run -- cargo xtask verify unit app --filter domain::profile` và `./tools/local-budget run -- cargo xtask verify unit web --filter profile_scope_is_fenced` | `3168b25c7b7c8d2d3bf658b5e9e2464e045380b7` | `PASS`, 2 App và 1 Web. [Trích log unit](unit.txt). |
| `./tools/local-budget run -- cargo xtask verify unit web --filter contract` | `0051b8cb0b05268bd416a03b60a9c036c34db377` | `PASS`, 27 Web, gồm ẩn nhãn và href ở snapshot chưa xác minh. [Trích log unit](unit.txt). |
| `./tools/local-budget run -- cargo xtask verify postgres` | `ea761cc4195d4c022d94793d30130f648dca8e9a` | `PASS` các bộ test 6 Store, 236 App, 48 Identity, 7 probe App opt-in bị bỏ qua. [Trích log PostgreSQL](postgres.txt). |
| `CARGO_BUILD_JOBS=2 cargo xtask verify web --scenario password --scenario discovery --scenario return-position` | `deafab0b221fd33e561222723cc213800604e416` | `PASS` cả ba. Profile/byline và Handle dùng password thật. Run `1795074251564505000-29175`, [trích log Chrome](browser.txt). |
| `CARGO_BUILD_JOBS=2 cargo xtask verify web --scenario discovery --scenario return-position` | `0051b8cb0b05268bd416a03b60a9c036c34db377` | `PASS` hai lane bị ảnh hưởng bởi thay đổi cuối, 50/50 lượt giữ vị trí, `bounded-read-owner` và `query-A-B-A-late` đạt. Run `1795074548276010000-37405`, [trích log](browser.txt) và [báo cáo thô](return-position.json). |

Unit App giữ SHA của lượt chạy. Contract App, policy và session scope đã kiểm vẫn giữ
nguyên. Các thay đổi sau PostgreSQL chỉ ảnh hưởng click và snapshot RAM sau hydration hoặc
fixture Web. SSR dùng snapshot đã xác minh, nên phép kiểm footprint ở App vẫn áp dụng.
Delta sau lượt password chỉ ẩn nhãn legacy/unavailable chưa đối chiếu trong cache và thêm
assertion. Luồng được kiểm với hai author ref có Handle, profile, phiên, form và API giữ nguyên.
Ảnh form, hồ sơ, WorkCard và reply context chụp ở `ea761cc` (run `1795073336476913000-14115`).
Ảnh Thử lại chụp ở `deafab0`. CSS và các trạng thái được chụp giữ nguyên ở HEAD.
Lane Web dùng build dev với `CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0`, ở ngoài
local-budget theo phạm vi launcher và chạy tuần tự với PostgreSQL. Báo cáo Web có
`dirty` chỉ ghi tài liệu và artefact đang được soạn trong worktree.

Ở ca Back qua hai mục lịch sử, Chrome báo `MainResourceHasCacheControlNoStore`,
`JsNetworkRequestReceivedCacheControlNoStoreResource` và `BrowsingInstanceNotSwapped`.
Kết quả dùng HTML mới và projection đã thu hồi. Đường khôi phục tài liệu BFCache có dấu
`pageshow.persisted` còn `NOT_RUN`. Phản hồi muộn được kiểm ở CDP sau khi nhận sự kiện mạng:
Chrome hủy lượt public cũ sau invalidation và giao lượt own của A cũ sau khi có phiên A mới.

Các lượt chẩn đoán đã bắt lỗi thời điểm khi fixture bấm Reader trước hydration,
callback WorkCard hết hiệu lực khi dòng làm mới và lịch đọc dư cho byline không có đích.
Lane dùng bản sửa đã qua hồi quy Khám phá và 50 vòng return-position. Ca lỗi hồ sơ đóng
cổng đọc trên database scratch rồi mở lại để kiểm Thử lại. Các tab Profile ghi nhận
zero console error và zero hydration exception.

## Từ tiêu chí tới bằng chứng

| AC | Caller và bằng chứng đạt trong phạm vi chạy | Phần còn lại |
| --- | --- | --- |
| PROFILE-01 | [Profile ở App](../../../../services/vink-app/src/persistence/profile.rs) + PG/HTTP kiểm tám mặc định, thay quyền từng trường, owner/other, hai lệnh dùng cùng version và từ chối widening. [Chrome](../../../../apps/vink-web/e2e/author-profile-smoke.mjs) kiểm lưu, tải lại, mất ACK và đối chiếu dữ liệu. | `USER-01@r1 DEFERRED`, `USER-01@r2 PENDING`. `SOC-OPEN-01` giữ luồng mở lại quyền. `SOC-OPEN-04` giữ phạm vi giá trị Avatar. |
| PROFILE-02 | [HTTP conditional](../../../../services/vink-app/tests/postgres/conditional_reads.rs) so từng byte JSON, ETag, HTML, head và hydration của Bio private với absent. Phép so HTML chuẩn hóa nonce đổi từng request. ETag từng trả Bio nhận 200 mới sau thu hồi, GET/HEAD dùng quyền hiện tại. Chrome kiểm hai tab, guest focus/Back, phản hồi public cũ bị hủy và phản hồi own A cũ thực sự tới sau A→B→A. | Hai lượt native Back dùng tài liệu mới. Đường browser BFCache hit là `NOT_RUN`. |
| PROFILE-03 | [Hồ sơ công khai](../../../../apps/vink-web/src/pages/handle.rs) + PostgreSQL seed 13 bài đúng chủ, bài mới hơn của người trùng tên, bài đã rút và work chưa công khai. Hai trang lần lượt 12/1 bài, cursor sai scope yêu cầu mở trang đầu. Chrome kiểm truy cập lúc chưa có bài, lỗi nhập giữ chữ, lỗi nguồn và Thử lại, form conflict giữ nháp, Về Tôi và focus. | IME hệ điều hành và bàn phím ảo trên máy thật `NOT_RUN`. |
| PROFILE-04 | [Contract và cache](../../../../apps/vink-web/src/public_reads.rs) kiểm fixture, decode legacy, ref bytes, invalidation. [Nâng schema](../../../../services/vink-app/tests/postgres/handle.rs) giữ setup/alias/quota/receipt, ledger cũ báo `Unknown`. Router mới trên cùng DB tiếp tục bác ETag trước thu hồi. Profile là owner của policy list/count để #294 dùng. | `NOT_RUN` với lần chuyển/lùi bản phát hành có cả binary/browser cũ chạy song song. Backout dùng bản có đủ projection bảo vệ quyền. |
| BYLINE-01 | [PG/HTTP](../../../../services/vink-app/tests/postgres/discussion.rs) đổi bút danh của hai Principal, đọc lại card/Reader/root/reply/context với đúng Handle và digest revision giữ nguyên. Chrome mở từng đích trên App nối database scratch. | |
| BYLINE-02 | [Projection PG](../../../../services/vink-app/migrations/20261125010000_public_author_attribution.sql) kiểm terminal fixture, public JSON/SSR/hydration/metadata chỉ còn danh xưng an toàn. Chrome Back qua hai mục lịch sử về Reader sau thu hồi fixture trả projection mới. Unit kiểm nhãn legacy/unavailable cũ được che khi cache chưa xác minh. | Test fixture đánh dấu lifecycle App. Điều phối Account/Identity thật thuộc #297. BFCache hit `NOT_RUN`. |
| BYLINE-03 | [WorkCard và ngữ cảnh trả lời](../../../../packages/vink-patterns/leptos/src/lib.rs) có các link độc lập. Chrome kiểm Tab/Shift+Tab/Enter, chạm CDP, Back giữ query/tag/focus, ngữ cảnh có thể đến hồ sơ và nhảy đúng bình luận. Unit decode legacy/unavailable giữ chữ và dừng cấp href khi thiếu ref. | Chạm vật lý `NOT_RUN`. |

## UI và accessibility

`fidelity: NO_APPROVED_REFERENCE` cho Social. Đối chiếu LayoutSpec r2 với tám ảnh đã mở
trên Mac. Ghi chú Khám phá r1 bổ sung tình huống title → Back → byline → Back cùng scope.

| Chiều | Kết quả trên Chrome |
| --- | --- |
| Functional | Phiên password thật vào form từ Tôi. Xung đột hiển thị bản đã lưu, giữ bản nháp. Hồ sơ khách chỉ hiện tác phẩm đúng chủ. |
| Geometry | 60 tổ hợp cho form, hồ sơ công khai và WorkCard, gồm 320/390/834/1024/1440 px, sáng/tối, chữ gốc 100/200%. Tất cả đích đo được nằm trong chiều rộng tài liệu. |
| Visual | Bút danh, Handle, Bio và bài theo một cột. Quyền Bio cạnh Bio. Hai đích trên WorkCard gạch chân riêng. Bản lưu trong xung đột có nền riêng và Lưu đổi trạng thái. |
| Accessibility/Responsive | Tab tới Lưu ở 320×580, chữ 200%, nút nằm trên thanh dưới và nhận thao tác. Tương phản tại nút Lưu, ô Bio và trợ giúp bút danh đạt tối thiểu [6,98:1 sáng](profile-contrast-light.json) và [8,16:1 tối](profile-contrast-dark.json). |
| Interaction | Về Tôi trả focus về Chỉnh hồ sơ. Conflict chọn bản đã lưu trả focus về Bút danh. Byline khôi phục đích hợp lệ sau Back. `CompositionEvent` chặn gửi giữa lúc ghép chữ. |

[Form desktop](author-edit-1440-light-100.png) · [Lưu có focus 320×580](author-edit-short-viewport.png)
· [Hồ sơ desktop](author-public-desktop.png) · [Bút danh dài 320 px/200%](author-public-320-dark-200.png)
· [Xung đột](author-profile-conflict.png) · [Lỗi và Thử lại](author-public-retry.png)
· [Hai đích trên WorkCard](author-byline-320-dark-200.png)
· [Ngữ cảnh trả lời](author-reply-context.png) · [SHA-256 của ảnh](screenshots.sha256).

## Gợi ý thử tay khoảng hai phút

`Human smoke: NOT_RUN`. Từ nhánh `codex/author-profile-and-byline`, chạy bản dev ở HEAD của PR.
Chuẩn bị App/Identity PostgreSQL local, một tài khoản A và một tài khoản B cùng bút danh,
mỗi người có một bài public. Đặt `VINK_AUTH_SIGN_IN=enabled` ở môi trường local đã cấu hình
và chọn `cargo xtask dev web --auth identity --migrate` nếu database local cần migration.
Có thể dùng hai tài khoản do người thử tạo. Fixture của lane Chrome dùng database tạm đã được dọn.
Xem [scoped workflows](../../scoped-workflows.md) cho role, biến cấu hình và cổng mặc định `3000`.

1. Đăng nhập A tại `http://127.0.0.1:3000/toi`, mở [Chỉnh hồ sơ](http://127.0.0.1:3000/toi/chinh-ho-so),
   nhập Bio thử, để **Công khai** rồi lưu và tải lại. Bio đã lưu còn nguyên, Tôi vẫn active.
2. Qua cửa sổ ẩn danh, mở địa chỉ `/@handle` của A từ Tôi. Bio hiện, danh sách chỉ có bài public của A.
3. Chuyển Bio sang **Chỉ mình tôi** và lưu. Quay lại cửa sổ khách hoặc tải lại. Bio biến mất ở khách,
   bản của chủ vẫn giữ đoạn văn. Đây là thao tác thu hồi, dùng Bio thử có thể giữ riêng trong suốt lần thử.
4. Ở [Khám phá](http://127.0.0.1:3000/), tìm hai bài bằng từ khóa/thẻ nếu fixture có thẻ.
   Tab qua tiêu đề và bút danh ở cỡ chữ 200%, mở hồ sơ từ từng thẻ và Back. Hai Handle khác nhau,
   kết quả/tags và focus trở lại đúng đích còn tồn tại. Nếu có bộ gõ Việt, thử Enter trong lúc đang
   ghép chữ và xác nhận Lưu tới được khi màn thấp, focus còn hợp lệ sau lưu hoặc Về Tôi.
