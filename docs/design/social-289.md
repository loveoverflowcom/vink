# Social Graph Web · LayoutSpec r2

**18/11/2026 · [#289](https://github.com/loveoverflowcom/vink/issues/289) · `DESIGN_PROPOSED`.**
Người đọc tìm đúng tác giả từ bút danh hoặc Khám phá, mở hồ sơ, Theo dõi, đọc bài và trở
lại đúng kết quả. Chính chủ vào Tôi để sửa và biết ai thấy từng giá trị. Phạm vi là Web
SSR/hydration và responsive. Hồ sơ, Handle, Follow và Following Feed ở mức
`NOT_IMPLEMENTED` tại base được đối chiếu.

## Nguồn và phạm vi đã chọn

| Nguồn | Phần áp dụng |
| --- | --- |
| [#289](https://github.com/loveoverflowcom/vink/issues/289), [LayoutSpec r2 trên Drive](https://drive.google.com/file/d/19jEf8nDu-VkGEwH9VTHoI2kuU7nomqlF/view?usp=drivesdk), [HTML v4](https://drive.google.com/file/d/11WUsdM1rMNJH_lKs8WnX3z2TEGHHgl_d/view), [ảnh mobile](https://drive.google.com/file/d/1etxRmNl5MNuKDanjegyheLgpfF4G1Raa/view), [ảnh rail](https://drive.google.com/file/d/1Kx-g703s9u1ERxljL6DAtH0RckrfEDDV/view) | Chủ sản phẩm chọn hướng v4 cho điều hướng, Giao diện và logo ngày 18/11. Chín luồng Social kế thừa concept bản 1 trong issue. SHA-256 HTML do bản Drive ghi là `3405d782790f234f0c5dcb5712c42591295441aeb415db608f8e6a8cc8046b01`. Preview dùng phiên và dữ liệu mô phỏng. Fidelity Web Social là `NO_APPROVED_REFERENCE`. |
| [PRD §15](../../prd/requirements.md#15-hồ-sơ-tác-giả-và-theo-dõi-social-graph), [Epic #284 · R10](https://github.com/loveoverflowcom/vink/issues/284), [hợp đồng liên miền r1](../architecture/social-graph.md), [HTTP Social r1](../../services/vink-app/contracts/http/v1/social-graph.md) | `R1–R9` đã xác nhận ở phạm vi PRD. Contract #288 được tích hợp qua [PR #303](https://github.com/loveoverflowcom/vink/pull/303) tại `12790d821d565260903727284233b3282943a259`. Bảng quyền, lỗi và biểu diễn r1 là đầu vào thiết kế. Route API, wire name và fixture chạy thật do provider công bố. |
| [Bàn giao #288](../development/social-graph.md), [registry Social](../../decisions/README.md#social-graph), [thảo luận public/own](../../decisions/decision-dev-20261112.toml), [vòng đời đọc Web](../../decisions/proposals/VDR-WEB-READ-LIFECYCLE.r1.toml) | Đối chiếu tại `develop@8c7afc946ae2b8769209808425237ed9996356d7`. `VDR-SOCIAL-GRAPH@r1`, `VDR-SOCIAL-CONTRACT@r1` và `VDR-WEB-READ-LIFECYCLE@r1` là `PENDING`. `DISC-WEB-CHOICE-01@r1` là tham chiếu `DESIGN_PROPOSED` cho tách dữ liệu public/own. |
| [Lavender và typography](README.md), [token](../../packages/ui-system/shared/tokens.json), [shell UIS-04](../../packages/ui-system/shared/navigation.md), [trạng thái UIS-03](state-and-feedback.md), [Quay lại Khám phá #236](discovery-return-position-236.md) | Dùng nền thiết kế và chỗ phục hồi focus/anchor hiện có. [Shell Web](../../apps/vink-web/src/shell.rs) đang mở panel Giao diện từ chân rail. [Tôi](../../apps/vink-web/src/pages/me.rs) đã có AccountPanel và Giao diện. [Khám phá](../../apps/vink-web/src/pages/discovery.rs) đang dùng logo dạng link. Bảng dưới ghi đích thiết kế cho các vùng này. |

## Cây vùng và thứ tự đọc

Các ID dưới đây là mốc bàn giao. Thứ tự bên trong mỗi vùng đồng thời là thứ tự đọc DOM.
Tab đi qua điều khiển theo thứ tự đó. Heading và status chỉ nhận focus theo thao tác
điều hướng hoặc phục hồi đã nêu.

```text
#social-shell
  #skip → #main
  #rail (760 px trở lên)
    #rail-toggle → #rail-explore → #rail-desk → #rail-me
  #mobile-top (dưới 760 px)
    #mobile-brand
  #main (một trang tại một thời điểm)
    #explore
      #explore-brand (desktop) → #explore-title → #explore-mode
      #following-feed → #feed-state → #feed-works → #feed-more
      #author-search → #author-query → #author-state → #author-rows → #author-more
    #profile
      #profile-identity (Avatar, bút danh, @handle)
      #follow hoặc #edit-entry → #profile-bio → #profile-relationships
      #profile-works (tiêu đề, state, thẻ bài, tải thêm)
    #me
      #me-title → #me-profile-or-auth → #appearance → #me-account
    #edit-profile
      #edit-back → #edit-title → #edit-fields
        #privacy (quyền cạnh giá trị tương ứng)
      #handle (điểm vào tác vụ địa chỉ riêng) → #save-profile (phản hồi, Lưu)
    #relationship-list
      #relationship-back → #relationship-title → #relationship-state → #people → #people-more
    #reader
    #profile-unavailable
      #unavailable-title → #unavailable-state → #unavailable-action
  #bottom-nav (dưới 760 px)
    #nav-explore → #nav-desk → #nav-me (mỗi đích gồm icon rồi nhãn)
  #dialog (xác nhận quyền, đổi địa chỉ hoặc đăng xuất)
```

**Phân cấp.** Hồ sơ dùng hướng A một cột theo nhịp đọc. So với hướng B đặt danh tính
ở cột bên, A giữ hành động và trạng thái “Chưa có tác phẩm công khai” trong cùng trục
khi bút danh dài ở 200%. Bio dài đẩy bài xuống theo luồng đọc. Quyền xem đặt cạnh từng
giá trị để chủ thấy tác động của lần sửa. Cách gom xác nhận khi lưu thuộc `SOC-OPEN-01`.
Form hồ sơ có CTA **Lưu**. Vùng địa chỉ mở tác vụ Handle với xác nhận và lệnh riêng.
Mỗi trạng thái chỉ nhấn một hành động chính.

### Shell, logo và Giao diện

| Bề rộng CSS | Bố cục và hành vi đích |
| --- | --- |
| Dưới 760 px · 320/390 | Thanh dưới kiểu Material 3, nền tonal liền mép; ba ô bằng nhau theo thứ tự **Khám phá → Bàn viết → Tôi**. Pill tối đa 64 × 32 CSS px chỉ bao icon đang chọn. Nhãn luôn ở dưới pill và được xuống dòng. Đích dùng link thật, trạng thái hiện hành dùng `aria-current="page"`; **Tôi** vẫn hiện và active trong Chỉnh hồ sơ. Logo Vink tĩnh ở đầu màn. |
| 760–1099 px · 834/1024 | Mặc định rail 96 px. Icon trên nhãn. Toggle **Mở rộng thanh điều hướng** ở đầu, Tôi ở chân. |
| Từ 1100 px · 1440 | Mặc định rail 220 px. Icon cạnh nhãn. Toggle **Thu gọn thanh điều hướng** cùng vị trí, Tôi ở chân. |
| Cả hai trạng thái rail | Toggle là nút có tên, tooltip, `aria-expanded` và `aria-controls`. Preference rail của thiết bị thắng mặc định và có lại hiệu lực khi từ mobile trở lên. Giữ quy ước riêng của Bàn viết editor trong UIS-04. Logo tĩnh cùng lockup V/ngòi bút nằm trong phần đầu Khám phá ở cả hai trạng thái; rail bắt đầu bằng toggle. Dùng [asset theo theme](../../apps/vink-web/assets/brand/README.md) và tên tiếp cận “Vink”. |

**Tôi và Giao diện.** Link Tôi từ cả hai khung mở `/toi`. Khách có “Đăng nhập” và
có thể đổi giao diện. Chủ có “Chỉnh hồ sơ”, lối xem hồ sơ công khai và hành động
“Đăng xuất”. Trong vùng Giao diện, nhóm lựa chọn có nhãn **Theo hệ thống → Sáng → Tối**,
có ngữ nghĩa chọn và áp dụng tại chỗ. [Owner Appearance](../../apps/vink-web/src/appearance.rs)
giữ `vink.ui.theme` trên thiết bị, bỏ override khi chọn Theo hệ thống và khôi phục trước
hydration. Đổi theme giữ focus trên lựa chọn. Toggle/resize giữ route, cuộn, focus còn
hợp lệ và cùng instance vùng nội dung. Nếu nút đang focus khuất do đổi khung, chuyển
focus tới đích tương ứng hoặc main. Đổi đích trong cùng phiên giữ nháp hồ sơ ở owner
của form; đăng xuất xác nhận và nhắc rõ khi có thay đổi chưa lưu. Hủy trả focus về nút
Đăng xuất. ACK thành công đưa Tôi về trạng thái khách, dọn dữ liệu cũ và giữ preference
thiết bị. Nếu mất ACK, đối soát phiên qua owner tài khoản trước khi báo đã đăng xuất.

**Sizing, scroll và theme dùng chung.** 320/390 px dùng một cột linh hoạt. Ở 834/1024/1440 px,
Profile, form, danh sách người và trang thay thế dùng cột đọc `--vk-measure` trong phần
còn lại sau rail. Kết quả dùng cột Khám phá hiện có. Dùng luồng cuộn chính của trang,
dự trữ chiều cao thanh dưới theo nội dung và tính safe area một lần. Rail tự cuộn khi
cửa sổ thấp. Lưu, Tải thêm và Thử lại nằm trong luồng, có thể cuộn tới khi bàn phím mở;
dialog có vùng cuộn nội dung và nút luôn tới được. Dùng Literata cho tiêu đề đọc, Be
Vietnam Pro cho điều khiển, vai trò `surface`/`surface-container` và
`primary-container`/`on-primary-container` cho pill. Lấy text, focus, space và motion
từ token trong cả Sáng và Tối. Tên Việt dài và Handle được ngắt dòng; Bio giữ xuống
dòng, khoảng cách và dấu tiếng Việt.

**Interaction và accessibility dùng chung.** Skip link tới main. Link dùng Enter/chạm,
nút dùng Enter/Space/chạm, nhóm chọn dùng phím mũi tên theo semantics native. Dialog có
tiêu đề, Tab ở trong lớp trên; Escape, Back hoặc Hủy đóng trước khi rời trang và trả
focus về trigger nếu còn, nếu mất trigger chuyển về main hoặc heading vùng. Giữ input,
selection và IME khi báo trạng thái; Enter trong composition chưa gửi form hay truy vấn.
Trạng thái đặt trong vùng nội dung tương ứng, có tên và thông báo vừa đủ qua live region.
Sau ACK hợp lệ thông báo kết quả một lần, giữ focus trên điều khiển còn tồn tại. Với
reduced motion, phản hồi và chuyển khung áp dụng tức thời, phục hồi cuộn không animate.

## Quyền, trạng thái và DTO dùng chung

| Scope theo HTTP r1 | Ai đọc và thao tác | Web dùng facts nào |
| --- | --- | --- |
| Public | Khách, `needs_principal`, chủ và người khác đọc cùng projection khi profile còn hiệu lực. Server kiểm quyền đọc hiện tại. | Bút danh, current Handle, href nội bộ do server cấp, trường có quyền và bài còn public. SSR/no-JS, head, hydration công khai, gợi ý trung tính và cache công khai dùng cùng biểu diễn. Hidden và absent có cùng footprint cả ở placeholder, ảnh, count, cursor và metadata. |
| Own | Đúng chủ có phiên active. | Giá trị đã lưu/chưa điền, quyền theo trường, version, setup/quota và list/count của mình. App định danh actor từ phiên và kiểm owner, standing, lifecycle, version khi commit. |
| Viewer | Thành viên active xem target hợp lệ nhận quan hệ của mình, capability, version và receipt được phép. | Theo dõi/Bỏ theo dõi theo trạng thái xác nhận; Mutual chỉ được cấp riêng cho hai đầu cạnh. Người thứ ba, kể cả admin chỉ có quyền moderation, đọc bản public. Self-follow không có action. |
| Feed | Thành viên active đọc trong scope phiên của mình. | Membership từ Graph và bài/mốc từ Publication qua Discovery. Scope này tách khỏi cache Khám phá public. |
| Phiên hoặc quyền thay đổi | Hết phiên, full suspension hoặc Identity disable đóng own/viewer/Feed và write/replay; quyền public đánh giá độc lập. | Bỏ dữ liệu vừa mất quyền và phản hồi muộn; đưa tới đăng nhập, trạng thái hạn chế hoặc lỗi authority tương ứng. Hạn chế chỉ đăng/bình luận giữ capability Social khi `SOC-DEP-02` có nguồn. Profile đang xóa/đã xóa áp fence cho mọi scope. |

Theo [HTTP r1](../../services/vink-app/contracts/http/v1/social-graph.md), quyền và lifecycle
được kiểm trước ETag/304. Hai cách tích hợp của `CHOICE-05` còn mở: SSR public rồi
tải overlay/Feed riêng sau xác thực, hoặc SSR cá nhân cô lập theo request với
`private, no-store`. Own/viewer/Feed và lỗi dùng `no-store`. Web giữ nháp/pending
theo generation của phiên; A→B→A vẫn là generation mới. Khi đổi phiên bỏ request,
page chain, receipt UI, nháp và dữ liệu Back của phiên cũ. Khi biết quyền vừa bị thu hồi,
các tab bỏ dữ liệu liên quan và phản hồi cũ đang bay. Mỗi lượt đọc mới, kể cả focus/Back,
phải xác nhận quyền trước khi vẽ lại từ cache bất kỳ trường nào chủ có thể ẩn.
Ví dụ, Bình đã đọc Bio công khai của An rồi mở Reader. An chuyển Bio sang riêng tư
trên thiết bị khác. Khi Bình quay lại, Web chờ đọc lại hồ sơ. Với ETag của bản có Bio,
server trả body mới đã lọc. Nếu cả hồ sơ mất quyền, Web hiển thị kết quả từ chối hoặc
không còn hồ sơ. Khi lỗi mạng, vùng đang chờ hiện Thử lại. Own/viewer/Feed đứng ngoài shared cache, IndexedDB và
`PublicKey::Works`. Dùng [ReadOwner/PageChain](../../apps/vink-web/src/read_lifecycle.rs)
làm tham chiếu về quyền sở hữu response và trang, với scope Social từ provider.

Trạng thái phổ biến theo scope của bảng trên:

| Trạng thái | Phản hồi, hành động và phục hồi |
| --- | --- |
| Đang tải, rỗng | Đặt chỉ báo ở vùng đang đọc. Chỉ hiện rỗng sau lần đọc thành công có quyền. Trường private/absent được bỏ theo cùng quy tắc công khai. |
| Lỗi đọc | Lỗi lần đầu có Thử lại. Lỗi append giữ các dòng còn hợp lệ và focus ở điều khiển tải; retry cùng scope còn hiệu lực. Cursor hết hạn/sai scope hoặc biết quyền đã đổi thì bỏ chuỗi. Đọc trang đầu nếu scope còn quyền; nếu mất điểm neo, chuyển focus về heading kết quả. |
| Lỗi ghi, conflict | Input sai hiện cạnh trường. Conflict đọc version/receipt có quyền, giữ nháp đúng phiên, cho chủ so giá trị hiện hành rồi tự xác nhận ý định mới. |
| Pending, mất ACK | Pending tại control, chặn lặp, giữ focus. Mất ACK báo “Đang kiểm tra kết quả”, đọc receipt theo command ID và trạng thái/version hiện hành. Chỉ retry cùng ID/cùng nội dung khi còn quyền. Receipt trả kết quả lệnh; trạng thái hiện hành quyết định UI. Bỏ ACK cũ/sai scope, không báo lưu thành công từ timeout. |
| Không có quyền hoặc authority lỗi | Bỏ điều khiển/dữ liệu ngoài quyền. Đưa đến lối đăng nhập hay giải thích hạn chế theo kết quả server. Lỗi DB/mạng/authority hiện Thử lại theo scope; khác rỗng, count 0 và xóa. |

## Chín bề mặt

Mỗi vùng dùng sizing, theme, thứ tự focus, overlay và các trạng thái chung ở trên.
“Hàng mới” và “form mới” chỉ định consumer hoặc biến thể cần ở child; thư viện đã có
[primitives](../../packages/ui-system/leptos/src/lib.rs) và
[patterns Leptos](../../packages/vink-patterns/leptos/src/lib.rs).

<a name="profile"></a>

### 1. Mở hồ sơ người viết · `#profile`

- **Entry và Back.** Mở `/@handle` từ URL, bút danh trên WorkCard/Reader/root/reply/reply
  context hoặc kết quả tác giả. Mỗi byline nhận public author ref của đúng người.
  Card có link bài và link tác giả độc lập, không lồng nhau. Dữ liệu cũ chỉ có tên hợp
  lệ hiển thị chữ; link đến từ ref có thật. Hai bút danh trùng phân biệt bằng Handle.
  Back về query/mode và đúng link đã mở nếu còn hợp lệ; thiếu anchor về heading của vùng nguồn.
- **Nội dung và hành động.** Avatar theo quyền → H1 bút danh → @handle → CTA theo vai →
  Bio/trường được xem → quan hệ → tác phẩm đang công khai. Khách đọc qua SSR, CTA
  đăng nhập rồi về đích nội bộ đã kiểm; `needs_principal` dùng luồng hoàn tất bút danh.
  Chủ vào Chỉnh hồ sơ; thành viên khác dùng `#follow`. Tác giả không bài vẫn mở được
  hồ sơ và nhận Follow nếu hợp lệ. Đặt loading/rỗng/lỗi của bài ngay dưới tiêu đề
  danh sách, giữ danh tính công khai còn quyền. Reader Back về thẻ bài hoặc heading
  danh sách nếu bài mất quyền. Focus tới H1 khi đến bằng điều hướng trong app.
- **Owner UI.** Dùng Heading, Avatar, Link, StatePanel, WorkCard. Cần trang hồ sơ và
  adapter author ref cho WorkByline, Reader và Discussion.

<a name="edit-profile"></a>

### 2. Sửa hồ sơ trong Tôi · `#edit-profile`

- **Entry và Back.** Từ `/toi` hoặc CTA chính chủ trên hồ sơ. Chủ thấy form có
  version; khách và người khác chỉ có lối được cấp ở bảng quyền. Focus H1 khi mở.
  Back về trigger Chỉnh hồ sơ của màn nguồn hoặc H1 Tôi khi vào trực tiếp. Giữ Tôi
  active và nháp trong cùng phiên.
- **Nội dung và hành động.** Bút danh → Avatar và quyền → Bio và quyền → ngày sinh
  khi đủ gate → quyền list/count → lối vào tác vụ Handle → Lưu và trạng thái.
  Dữ liệu đang tải có status trong form; lỗi đọc có Thử lại. Trường có nhãn và lỗi gắn
  với input. Lỗi validate đưa focus về trường lỗi đầu, giữ Unicode và những lựa chọn khác.
  Lỗi mạng giữ chữ. Khi hai tab xung đột, hiện bản hiện hành cạnh bản nháp đúng phiên để so,
  người viết chủ động chọn rồi lưu lại. Mất ACK dùng state chung. Đổi tài khoản xóa
  form, không mở lại nháp cũ khi Back.
- **Owner UI.** Dùng TextField, Select, Button, InlineNotice theo owner hiện có.
  Cần form có version và state nháp theo phiên, dùng AccountPanel cho lối tài khoản.

<a name="handle"></a>

### 3. Xác nhận và đổi Handle · `#handle`

> [Delta #480 ngày 24/11](profile-inline-handle-480.md) chuyển tác vụ vào Chỉnh hồ sơ.
> Phần dưới lưu quyết định setup/quota và bố cục gốc của Social R2.

- **Entry và Back.** Sau khi có bút danh, chính chủ vào thiết lập hoặc đổi từ vùng địa
  chỉ trong Tôi. Trước setup, hiện Handle đã sinh và cho sửa. Sau setup hiện địa chỉ
  hiện hành và số lượt còn lại. Công khai chỉ đọc Handle hiện tại qua ref. Quay lại
  giữ form hồ sơ của phiên hiện tại và đưa focus về lối vào vùng địa chỉ.
- **Interaction.** `SOC-OPEN-02` đã chọn A ở [#290 r2](https://github.com/loveoverflowcom/vink/issues/290).
  **Dùng địa chỉ này** → pending → commit thành công kết thúc setup dù giữ gợi ý
  hay sửa, còn đủ 2 lượt. Sau setup, đổi thành công trừ một lượt; ghi lại cùng tên
  là no-op. Trước commit giải thích URL cũ dẫn tới người này khi tài khoản còn hiệu
  lực. Hủy xác nhận giữ input và trả focus. Trùng/reserved/không hợp lệ giữ input,
  lỗi tại trường; hết lượt hiển thị giá trị đã lưu và số lượt thật. Mất ACK tra
  receipt và version/quota hiện hành trước khi cho gửi lại. `SOC-OPEN-03` chỉ giữ
  luồng dùng lại chính alias. Người khác vĩnh viễn không được cấp tên đã dùng.
- **Owner UI.** Dùng TextField, InlineNotice và Dialog. Cần vùng setup/quota và
  điều khiển đổi theo version/receipt từ App Profile.

<a name="privacy"></a>

### 4. Quyền theo trường · `#privacy`

| Trường | Mặc định theo r1 | Điều khiển của chính chủ |
| --- | --- | --- |
| Bút danh | `everyone` | Luôn công khai, hiển thị thông tin quyền ở cạnh trường. |
| Avatar, Bio | `everyone` cho từng trường | Mỗi giá trị có lựa chọn riêng. Cách nhập Avatar theo `SOC-OPEN-04`. |
| Ngày sinh đầy đủ | `self` khi được phép thu thập | `USER-01@r1 DEFERRED` là revision có hiệu lực. Chỉ hiện trường thu thập sau khi ngoại lệ `USER-01@r2` có hiệu lực. Khi đó công khai cả ngày/tháng/năm cần xác nhận chủ động gắn đúng giá trị và version. |
| Danh sách và số Người theo dõi | `everyone` cho từng mục | Hai điều khiển độc lập. |
| Danh sách và số Đang theo dõi | `self` cho từng mục | Hai điều khiển độc lập. Ẩn Following của mình vẫn có thể xuất hiện trong Followers công khai của người khác. |

- **Interaction.** Chính chủ chọn “Chỉ mình tôi”/“Công khai” cạnh trường tương ứng, sau đó
  commit theo luồng Lưu của Profile. Confirm self→everyone nêu đúng giá trị/dữ liệu
  sắp lộ. `SOC-OPEN-01` quyết định xác nhận theo từng trường hay theo lần lưu.
  Cancel giữ nháp, trả focus về control mở xác nhận; khi tiếp tục chỉnh, bản nháp
  vẫn khác dữ liệu đã lưu. ACK đổi quyền cập nhật own state và revalidate biểu diễn
  công khai liên quan. Lỗi/xung đột giữ lựa chọn của đúng phiên. Khách và người khác
  nhận cùng biểu diễn cho giá trị private và chưa điền.
- **Owner UI.** Dùng Select, Dialog và InlineNotice. Cần điều khiển riêng cho từng
  quyền và copy tiết lộ theo field. Nếu có ảnh tải lên, bytes và biến thể cũng theo
  quyền hiện thời, bao gồm URL/validator cũ; #298 sở hữu đường phát ảnh.

<a name="follow"></a>

### 5. Theo dõi · `#follow`

- **Entry và hành động.** Vùng cạnh danh tính. Với khách, đăng nhập quay về đúng hồ
  sơ nội bộ hoặc trạng thái thay thế nếu đích đã mất. Thành viên khác thấy một nút
  **Theo dõi** hoặc **Bỏ theo dõi** sau khi đọc quan hệ. Trong lúc tải/lỗi overlay,
  status nằm cạnh danh tính; lỗi có Thử lại, không suy ra chưa Follow. Chủ dùng
  Chỉnh hồ sơ. “Theo dõi lẫn nhau” chỉ ở scope viewer của hai đầu cạnh.
- **Interaction.** Nút gửi desired state với version và ID. Pending giữ vị trí/focus.
  Ví dụ, Follow từ v0 đã commit ở v1 nhưng mất ACK. Sau khi tra receipt và đọc
  quan hệ ở v1, người dùng gửi Unfollow bằng ID mới và nhận v2. Khi ACK v1 về muộn,
  nút vẫn là **Theo dõi** theo v2. ACK v1 cũng không cập nhật số đếm hay phát lại
  thông báo. Sau khi xác nhận, thông báo một lần và đọc lại số đếm/list được phép. Khi capability/target mất, bỏ action,
  focus về danh tính nếu nút biến mất.
- **Owner UI.** Dùng Button hoặc ToggleAction cho trạng thái xác nhận và InlineNotice.
  Cần adapter quan hệ, capability và receipt; Graph sở hữu cạnh, count và version.

<a name="relationship-list"></a>

### 6. Danh sách người · `#relationship-list`

- **Entry và Back.** Link “Người theo dõi” hoặc “Đang theo dõi” trong hồ sơ chỉ khi
  list có quyền. Count có quyền có thể hiện riêng dưới dạng chữ khi list bị ẩn.
  List có quyền và count ẩn dùng link theo tên, bỏ cả tổng trang và `total_count`.
  Chủ phân trang danh sách hợp lệ của mình. Người ngoài đọc list/count theo public
  projection, độc lập với phiên. Có count 0 thật khi được trả từ server, rỗng list
  chỉ sau đọc thành công.
- **Interaction.** Heading theo hướng cạnh, rows gồm bút danh và Handle/link khi
  còn được cấp. Tab tới từng link rồi Tải thêm. Append lỗi giữ dòng còn quyền và focus;
  nếu quyền/cursor đổi, bỏ chuỗi và đọc lại theo scope mới. Back về link đã mở hoặc
  heading quan hệ/hồ sơ khi link đã mất. Đích tác giả từ row dùng return anchor của list.
- **Owner UI.** Dùng Heading, Link, StatePanel, Button. Cần hàng người, vùng kết quả
  có tên và cursor riêng cho public/own và từng hướng.

<a name="following-feed"></a>

### 7. Khám phá → Đang theo dõi → Reader → Back · `#following-feed`

- **Entry và nội dung.** Mode trong Khám phá; khách đi qua CTA đăng nhập. Thành viên
  thấy thẻ bài có tiêu đề, tác giả, thẻ hiện hành và mốc lần đầu công khai. Pilot
  chọn `exact` theo #291/#296. Bài có trước Follow vẫn hợp lệ; mỗi WorkId một dòng
  theo snapshot còn public, mới trước theo mốc đầu và khóa phụ ổn định do provider.
- **States.** Đang tải/lỗi đầu trong vùng kết quả. Phân biệt “Chưa theo dõi tác giả
  nào” (lối tìm tác giả khi khả dụng) và “Chưa có tác phẩm công khai đủ điều kiện”
  khi đã theo dõi. Append lỗi giữ dòng hợp lệ. Khi biết vừa Unfollow hoặc bài rút,
  bỏ item không còn quyền và kết quả cũ đang bay. `CHOICE-06` còn mở giữa live
  membership mỗi trang với restart sau mutation, và cursor gắn graph revision với
  reject/restart khi graph đổi. Cả hai đọc lại quyền theo lần đọc hiện tại.
- **Interaction và owner UI.** WorkCard mở Reader. Back dùng mode, scope phiên và
  anchor còn hợp lệ, khôi phục cuộn/focus sau revalidate. Mất anchor focus heading
  kết quả. Dùng WorkCard, StatePanel và quy ước ExploreContext/PageChain; cần mode,
  state đọc riêng theo phiên và cursor riêng. Không tái dùng chain của tìm tác giả hay tìm bài.

#### Following Empty CTA R1 · #355

Chỉ hiện khi lượt đọc theo phiên trả về trạng thái sẵn sàng, danh sách rỗng và
`has_following=false`. Vùng `community-stream` vẫn có tên “Đang theo dõi”. Cây hiển thị gồm hai phần.

- **Khám phá** → nhóm chọn mode → **Đang theo dõi**.
- **Vùng kết quả** → biểu tượng người trang trí → H2 “Chưa theo dõi tác giả nào”
  → “Khám phá những cây bút bạn yêu thích.”
  → link tonal có biểu tượng tìm kiếm “Khám phá tác giả” (`/?mode=authors`).

Khối căn giữa trực tiếp trên nền trang. Tiêu đề của vùng chỉ dùng để đặt tên cho
`community-stream` trong trạng thái này. CTA không gạch chân, cao tối thiểu 48 CSS px,
bo góc `radius.control-48` và xuống dòng khi cần. Kiểm sáng/tối ở 320/390/834 CSS px
ở cỡ mặc định 100%, cỡ Lớn 150% của ứng dụng và stress riêng với font gốc 200%.
Thanh tab cố định tuân theo LayoutSpec #530. Enter mở mode Tác giả với Search Dock
đóng. Khi Back từ Reader
và lượt đọc lại xác nhận đã mất anchor vì không còn theo dõi ai, focus về H2 của
trạng thái rỗng. Nếu vẫn có tác giả theo dõi, dùng heading Feed hiện hành.

**CHOICE-02 (triển khai Web)** dùng composition cục bộ với Heading, Icon và
ButtonVariant::Tonal. Tiêu đề của trạng thái là đích khôi phục focus dành cho
Feed. Cách này tránh mở rộng API của `EmptyState` và vẫn dùng kiểu biểu tượng,
lời nhắn và nút từ hệ hiện hành. Catalog hiện có dùng biểu tượng người và tìm kiếm.

[Issue #355](https://github.com/loveoverflowcom/vink/issues/355) ·
[SVG](https://drive.google.com/file/d/1GFpKKO5ho2z7pDbNLtJXtMOXo6gS1nRT/view?usp=drivesdk) ·
[HTML](https://drive.google.com/file/d/1fgf7RjIS2nIairatOkrbzTTWz43T70h9/view?usp=drivesdk) ·
[PNG](https://drive.google.com/file/d/11EzRusgJxWsfMzwxq257Kbo4ff8czRmz/view?usp=drivesdk).
Các khung R1 là concept `DESIGN_PROPOSED / NO_APPROVED_REFERENCE`.

<a name="author-search"></a>

### 8. Tìm và xem gợi ý tác giả · `#author-search`

- **Entry và layout.** Từ Khám phá, dùng SearchField; kết quả/gợi ý bên dưới có
  bút danh + @handle trong cùng link tới hồ sơ, người chưa có bài vẫn có hàng.
  Từ khóa trống có thể hiện gợi ý. Tách trạng thái rỗng và lỗi; loading tại vùng
  danh sách. Guest và thành viên đều đọc projection công khai. Nút theo dõi nếu
  được chọn nằm ngoài link và chỉ nhận quan hệ ở scope viewer.
- **Interaction.** Gõ IME chưa commit giữ query đang sửa. Commit query mới thay
  chain cũ; bỏ response muộn. Tải thêm giữ focus, lỗi append giữ dòng còn hợp lệ.
  Mở profile rồi Back phục hồi mode/query, cuộn và link đã mở nếu còn quyền; nếu
  profile vừa xóa trả trạng thái không còn hồ sơ và khi Back chọn heading nếu
  mất hàng. Preview dùng ví dụ public; candidate/score/thứ tự gốc chỉ lấy facts
  công khai. Chủ sản phẩm chọn mode **Tác giả** riêng, chỉ tra bút danh và Handle
  hiện hành. Gợi ý giữ cùng danh sách và thứ tự. Nút Theo dõi nạp theo phiên.
  [SOC-OPEN-05/06 ở #295](https://github.com/loveoverflowcom/vink/issues/295)
- **Owner UI.** Dùng SearchField, Avatar, Link, StatePanel và mẫu phục hồi #236.
  Cần hàng tác giả và kết quả/cursor độc lập với tác phẩm, overlay nếu có CTA riêng.

<a name="profile-unavailable"></a>

### 9. Alias và hồ sơ không còn · `#profile-unavailable`

- **Entry và kết quả.** Mở URL đã dùng qua link chia sẻ hoặc kết quả cũ. Alias
  còn hiệu lực **301 thẳng** đến current Handle nội bộ tại lúc đọc, chỉ giữ query
  được phép, fragment do browser xử lý. Kiểm GET/HEAD và `no-store` trong
  giai đoạn chưa chứng minh được revalidation qua hai lần đổi rồi xóa. Router
  đưa focus đến H1 đích khi điều hướng trong app; lần tải tài liệu mới có title/H1
  và thứ tự đọc của trang đích.
- **Hồ sơ đích đã mất.** Tombstone dừng ở trạng thái “Hồ sơ không còn truy cập
  được”, có lối quay lại hoặc Khám phá. Sau H0 → H1 → H2, GET/HEAD tại H0 và H1
  đều trả 301 thẳng đến H2. Sau khi xóa, cả ba URL dừng ở trang thay thế, bỏ
  `Location` và mọi dấu định danh cũ, kể cả khi trình duyệt từng mở H0.
  Mạng/5xx dùng trạng thái lỗi và Thử lại.
  URL tác phẩm theo trang thay thế của Publication; bình luận còn hợp lệ chỉ hiện
  “Tài khoản đã xóa” ở root, reply và reply context, bỏ link, ảnh, tên, Handle và
  id cũ cả trong HTML/head/hydration. Trên Back bỏ item đã biết mất quyền.
- **Owner UI.** Dùng Heading, StatePanel và mẫu trang Reader thay thế. Cần resolver
  HTTP và kiểu attribution đã xóa từ App; xóa liên miền thuộc Account và các owner.

## Lựa chọn sản phẩm và phần bị ảnh hưởng

Bảng này theo [hợp đồng liên miền r1](../architecture/social-graph.md) và áp tại
vùng đã nêu. Các gate chỉ giữ phần phụ thuộc tương ứng.

| Quyết định | Các phương án và hành vi | Phạm vi còn chờ |
| --- | --- | --- |
| `SOC-OPEN-01` · mở | A dùng chung dialog cho từng trường, ghi giá trị sắp lộ. B xác nhận cả lần lưu, liệt kê chính xác từng trường đã chọn công khai. Cả hai yêu cầu xác nhận chủ động với ngày sinh khi đủ gate. | `#privacy`: luồng self→everyone. |
| `SOC-OPEN-02` · đã chốt A tại #290 r2 | A hoàn tất khi xác nhận thành công cả giữ gợi ý và nhập tên mới. B từng đề xuất đợi tên tự nhập được lưu mới hoàn tất. B là lịch sử; số lượt sau setup vẫn là 2. | `#handle`: triển khai/test cả giữ và đổi. |
| `SOC-OPEN-03` · mở | A alias đã nghỉ vĩnh viễn không trở lại current. B chỉ chính Principal có thể dùng lại, tốn một lượt đổi thường sau setup. | `#handle`: giữ reclaim chưa mở; resolve alias và cấm tái cấp khác chủ áp trong cả hai. |
| `SOC-OPEN-04` · mở | A Avatar chữ, điều chỉnh nghiệm thu media theo phạm vi v1 nếu được chọn. B upload kèm thay/xóa, ảnh gốc, biến thể và phục vụ bytes có kiểm quyền. | `#profile`/`#privacy`: chờ quyết định phạm vi Avatar; upload chỉ mở sau khi #298 kiểm được quyền. |
| `SOC-OPEN-05` · đã chọn A ở #295 | A exact Handle, tìm bút danh/Handle hiện hành; alias chỉ resolve URL. B tìm thêm alias về cùng Principal, ghi rõ quan hệ lịch sử. Cân nhắc mode tác giả riêng (query/Back riêng) hoặc nhóm tác giả cùng truy vấn (anchor độc lập cho nhóm). Cùng thử người trùng tên và chưa có bài để so hai cách. | `#author-search`: khớp alias và cấu trúc kết quả. |
| `SOC-OPEN-06` · đã chọn A ở #295 | A giữ tập/thứ tự gợi ý công khai, CTA theo viewer. B sau rank lọc chính mình/đã Follow bằng dữ liệu được viewer phép đọc. Candidate/score và DTO công khai bất biến khi đổi cạnh riêng. | `#author-search`: biến thể B/CTA theo viewer cần #294. |

## Bàn giao và kiểm chứng

Khi nhận một vùng, dùng đường dẫn này, **r2**, SHA thực tế của commit đã tích hợp
chứa LayoutSpec và ID vùng. Đối chiếu DTO/capability của provider ở SHA đang review
trước khi triển khai. Scope, dependency và thứ tự gộp ở [bàn giao #288](../development/social-graph.md).

| Vùng | Issue nhận bàn giao | Bằng chứng cần lấy |
| --- | --- | --- |
| `#profile`, `#edit-profile`, `#privacy` | [#292](https://github.com/loveoverflowcom/vink/issues/292); author ref tại [#293](https://github.com/loveoverflowcom/vink/issues/293); ảnh có điều kiện tại [#298](https://github.com/loveoverflowcom/vink/issues/298) | HTTP/SSR/no-JS và hydration cùng footprint hidden/absent; public/own hai phiên, revoke trước validator; fixture tên Việt trùng, chưa có bài, card/Reader/root/reply/context đúng ref. Browser 320 px/200%, focus, keyboard inset, conflict, A→B→A; thử bytes cũ nếu có upload. |
| [`#handle`](#handle), [`#profile-unavailable`](#profile-unavailable) | [#290 · preview Handle r2 đã duyệt](https://github.com/loveoverflowcom/vink/issues/290) | Dùng preview r2 của #290 cho giao diện Handle. Kiểm setup giữ hoặc sửa, quota, mất ACK và hai tab. Kiểm SSR/no-JS cho hồ sơ tối thiểu, GET/HEAD và Back qua hai lần đổi, query/fragment và tombstone ở resolver. |
| [`#profile-unavailable`](#profile-unavailable) | [#297](https://github.com/loveoverflowcom/vink/issues/297) | Với `SOC-DEP-01`, kiểm xóa liên miền sau khi browser đã mở alias, các URL cũ dừng ở trang thay thế và attribution của root/reply/context dùng nhãn đã xóa. |
| `#follow`, `#relationship-list` | [#294](https://github.com/loveoverflowcom/vink/issues/294), policy field từ [#292](https://github.com/loveoverflowcom/vink/issues/292) | Browser pending và mất ACK → Unfollow → ACK cũ; Mutual hai đầu/khách/người thứ ba, list có quyền/count ẩn và ngược lại, pagination đổi quyền. Race/receipt/count cần PG/HTTP theo #294. |
| `#following-feed` | [#296](https://github.com/loveoverflowcom/vink/issues/296), fact đầu vào từ [#291](https://github.com/loveoverflowcom/vink/issues/291) | Bài có trước Follow, lọc snapshot/provenance, lỗi append; Browser Reader→Back, A→B→A, rút bài hoặc Unfollow giữa hai trang, mất anchor. |
| `#author-search` | [#295](https://github.com/loveoverflowcom/vink/issues/295) | Tên trùng, Handle chính xác, alias theo lựa chọn, người 0 bài, IME và query/Back; HTTP đổi cạnh riêng rồi so biểu diễn/rank công khai. |
| `#rail`, `#bottom-nav`, `#mobile-top`, `#explore-brand`, `#appearance`, `#dialog` | Owners Web shell/Tôi/Khám phá; nghiệm thu ở child chạm vùng tương ứng ([#292](https://github.com/loveoverflowcom/vink/issues/292), [#295](https://github.com/loveoverflowcom/vink/issues/295), [#296](https://github.com/loveoverflowcom/vink/issues/296)) | Mở Tôi từ hai khung, guest chọn theme và giữ qua logout, focus/cancel; so v4 cho logo/pill/rail ở các biên và kiểm 320 px/200% khi đang chỉnh hồ sơ. Ghi phạm vi shell trong PR tiêu thụ. |

Theo [rubric năm chiều](../../.agents/skills/vink-ui-design/references/ux-quality.md),
mỗi child có UI ghi SHA, build/lệnh, fixture, viewport, theme, text scale và trạng thái.
Chạy tại 320/390/834/1024/1440 CSS px, Sáng/Tối, chữ 100/200%; thêm ca Theo hệ thống và
các biên 759/760/1099/1100. Fixture gồm hai tác giả trùng tên, người chưa có bài, bút
danh/Bio tiếng Việt dài, field self/absent và mạng chậm. Rà theo cùng kịch bản tối đa
hai vòng sửa rồi ghi phần còn mở.

| Chiều | Điều kiện quan sát | Web Social tại PR đặc tả |
| --- | --- | --- |
| Functional | Chín luồng theo actor và quyền; empty khác lỗi; hai lối vào Tôi đưa tới cùng Giao diện; thử SSR/no-JS và sau hydration. | `NOT_RUN` |
| Geometry | Tràn ngang trang 0 CSS px. Ba ô mobile rộng bằng nhau và trục icon/dòng nhãn đầu lệch tối đa 1 CSS px do làm tròn. Rail 96/220 CSS px. Vùng chạm ≥44 CSS px. Lưu/CTA cuối tiếp cận được khi mở bàn phím. | `NOT_RUN` |
| Visual | Ở 100% so phân cấp, pill chỉ ôm icon, thanh liền mép và vị trí logo với concept v4. Ở 200% đọc đủ và còn thao tác được. Mở ảnh đúng trạng thái khi đánh giá. | `NOT_RUN` |
| Accessibility/Responsive | Đếm link lồng = 0; kiểm tên/heading, thứ tự Tab, focus visible, text thường ≥4,5:1 và focus ≥3:1. Màn hẹp/chữ lớn còn đầy đủ nhãn và CTA, đo inset trên thiết bị thật. | `NOT_RUN` |
| Interaction | Thử phím/chạm, IME hệ điều hành, reduced motion, ACK muộn với trình đọc màn hình có tên, dialog trả focus, Reader→Back và rail/theme đổi trong khi giữ route, cuộn và nháp. | `NOT_RUN` |

Bản r2 trên Drive ghi Chrome chạy **HTML mô phỏng v4** tại 24 tổ hợp thanh dưới
(320/390 px × Sáng/Tối × chữ 100/200% × 3 màn), các biên rail và ca giữ preference/nháp.
Bằng chứng này thuộc preview nguồn. PR đặc tả đối chiếu tài liệu, actor, contract và
component tại base. Web SSR/hydration, mobile thật, IME OS và screen reader cần bằng
chứng của child tương ứng. Native product ngoài phạm vi.

**Truy vết nghiệm thu #289.** UX-01 dùng cây vùng và chín bề mặt (SOC-AC-22/24/27);
UX-02 dùng ma trận quyền/trạng thái và sáu lựa chọn (SOC-AC-06/07/09/10/13/19/27/29);
UX-03 dùng quy tắc responsive/Interaction và bảng bàn giao (SOC-AC-22/23/27/28);
UX-04 đối chiếu delta điều hướng r2/v4. Các bất biến SOC-INV-05/06/07/09/12 và gate
PRE-04 được truy về [registry #288](../development/social-graph.md).
