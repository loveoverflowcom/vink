# Review RAM READ-02 trên document canonical

Ngày 18/11/2026, checkout Mac arm64. Review issue [#237](https://github.com/loveoverflowcom/vink/issues/237) và PR [#283](https://github.com/loveoverflowcom/vink/pull/283) sau #285, #286, #287 và #299. Hai timeline bình luận trả về rỗng. Benchmark cũ dùng blocks/lines. Các số dưới đây lấy từ lượt mới với `{format, source}`.

## Findings đã sửa

- **P2 · Payload sau khi giao cho Resource.** Khi fetch thành công, coordinator lấy waiter ra khỏi danh sách rồi giao DTO. Nếu owner bị dispose trước khi view gọi `follow_seed`, cleanup cũ chỉ tìm waiter trong danh sách đã rỗng. Body vẫn nằm trong `Entry.data`, ngoài bộ đếm cache, cho đến lần collection hoặc removal sau. Mỗi Resource nay giữ một ticket riêng. Cleanup cuối trả body về admission khi hết cả Resource và view. Removal/clear làm ticket cũ mất hiệu lực. Hai waiter nhận đúng cùng payload, hủy waiter đầu giữ quyền đọc của waiter thứ hai. Đối chứng bỏ đúng bước park làm regression đỏ, khôi phục làm nó xanh.
- **P2 · Thu hồi một tác phẩm đổi thứ tự eviction của Reader khác.** Trước sửa, withdrawal duyệt toàn bộ `BTreeMap`, clone và đưa từng snapshot trở lại cache kể cả khi dữ liệu chưa đổi. Regression trên base nhả Z trước A, rút một tác phẩm khác rồi thêm B. Cache đã đẩy A ra trước Z. Bản sửa chỉ sửa các danh sách bị ảnh hưởng và giữ thứ tự của Reader. Thứ tự ưu tiên xuất phát từ lần đưa snapshot vào cache.
- **P3 · Clone trên đường fetch.** Adapter cũ vẫn clone DTO vào envelope khi gate lưu thiết bị tắt. Quyền ghi từng cần cả lease và flight hiện hành, nhưng lease chỉ được xét sau khi dựng envelope. Adapter nay xét lease trước, đọc loại control ngay từ response và chuyển reply cuối vào channel. Với một waiter, phần fan-out không clone thêm một bản.

## Corpus và phép đo

`ae8c6d5cb73b779f6e222a3a545302be48c0f833` là develop đã fetch. `a06168a43684710c3e25ff7ae5ee5b6c443edbae` là implementation dùng cho Chrome A/B/C. Hai diff đối chứng chỉ đổi constructor hoặc budget của owner RAM. `67e1c44` bổ sung regression và hai hàng native, rồi chạy lại các gate với B. Target native aarch64-apple-darwin, browser wasm32-unknown-unknown, Chrome headless 157.0.8103.79, Rust 1.99.1, PostgreSQL 17.11. Web dùng profile dev, native chạy cả test debug và release. Các lượt tắt incremental và bỏ debug symbols cho dev/test. Dữ liệu tổng hợp chạy qua App thật trên loopback. Không có truy vấn production.

Chrome dùng một tab mới cho mỗi cấu hình. Nguồn gồm văn mẫu tiếng Việt trong contract, NFD 30k, năm bản thơ-tab 200k của #229 và một source đúng 2.097.152 byte với hai đoạn văn cách bằng dòng trống. HTTP save và capture dùng domain thật, sau đó gắn publication fixture trong DB scratch riêng. Mỗi lần mở đều so SHA-256 của toàn bộ text hiển thị, số đoạn và `performance.timeOrigin`. Mỗi lần đóng kiểm Reader đã rời DOM. Source gốc và projection hiển thị có hash riêng. Mỗi dòng là một lần chạy 50 vòng cùng lịch mở. Thứ tự thực hiện là B trên develop, B sau sửa, A, rồi C. Cache của hệ điều hành có thể khác giữa các lượt.

| Cấu hình | Reader GET quan sát trên 50 lần mở | GET cho 10 lần mở source 2 MiB | JS heap dùng sau GC, vòng 50 | WASM linear memory, vòng 50 | RSS cộng các process Chrome, bắt đầu → vòng 50 |
|---|---:|---:|---:|---:|---:|
| B trên develop | 46 | 10 | 5,04 MiB | 24,38 MiB | 988,3 → 1.231,6 MiB |
| A, bỏ trần byte | 8 | 1 | 5,03 MiB | 24,38 MiB | 1.002,7 → 1.179,5 MiB |
| B sau sửa, 2 MiB tổng / 1 MiB mỗi entry | 46 | 10 | 5,02 MiB | 24,38 MiB | 998,3 → 1.241,4 MiB |
| C, NoOp | 50 | 10 | 4,97 MiB | 22,38 MiB | 997,1 → 1.238,4 MiB |

GET ở bảng là sự kiện request của trang tới `/reader`. Chrome có thể tái xác thực qua HTTP cache. Native dưới đây đếm quyết định fetch. RSS là tổng các PID do Chrome của lượt đo báo qua CDP, gồm browser, renderer, GPU và utility, có thể tính lặp trang bộ nhớ chia sẻ. Lấy mẫu sau GC tại các mốc trong raw. Cả khi giữ NoOp, RSS cũng tăng. Với B, WASM giữ 24,38 MiB từ vòng 10 tới 50, JS heap 4,94 → 5,02 MiB và RSS cộng 1.122,9 → 1.241,4 MiB trong cùng khoảng. Một lần chạy ở profile dev chưa tách được nguyên nhân tăng RSS.

| Mô phỏng native, 50 vòng mỗi corpus | Peak A | Peak B | Peak C | Eviction B | Tổng số fetch intent A / B / C |
|---|---:|---:|---:|---:|---:|
| 16 bài ngắn | 15.868 B | 15.868 B | 0 B | 0 | 51 / 51 / 100 |
| 16 bài mixed 200k | 8.187.138 B | 2.059.530 B | 0 B | 47 | 51 / 51 / 100 |
| 4 bài gồm 200k đoạn một ký tự | 2.408.133 B | 1.807.510 B | 0 B | 47 | 5 / 51 / 100 |

Giữ B cho optional retention. B giảm khoảng 6,1 MB trọng số đỉnh ở corpus mixed và giữ được các snapshot còn trong cache để đọc lại khi mất mạng. A giảm mạnh request ở bộ tám bài vừa đo. Source canonical 200k đoạn có weight khoảng 600.647 B và được nhận từng entry; áp lực ở đây là tổng số entry cùng kích cỡ. Ngưỡng 1 MiB thực sự từ chối case NFD 1,2 MB và source 2 MiB. Thay đổi ngưỡng cần corpus thiết bị/người đọc đại diện hơn.

## Bộ nhớ khi có người đọc

`gloo-net` đọc `Response.text()`, chuyển chuỗi JS vào Rust rồi serde decode ra DTO sở hữu `String`. Trong lúc decode escaped source, byte JSON và source đã giải mã có thể cùng tồn tại. Khi mở cache, `SnapshotCache::get` clone giá trị sang Store. Các kết quả gửi cho waiter, Resource, status/data memo của view, bản revision đang hiển thị và DOM có lifetime khác nhau. Reader giữ bản đang đọc để một refresh không phá selection. Byte budget chỉ tính snapshot trong `InMemoryCache`. Đo WASM cho biết dung lượng linear memory đã cấp, chưa tách allocator live bytes hay đỉnh tức thời của mọi clone.

Lượt native test đếm 26 hàng corpus/baseline và 3 mô phỏng. Với source 2 MiB, weight 2.097.793 B, payload JSON publication 4.194.697 B, một lần clone 147,6 µs và decode `ReaderResponse` 42,01 ms. Case NFD 200k vượt budget có weight 1.200.327 B, clone 93,5 µs, decode 16,76 ms. Đó là số đo native debug. Ở profile release trên cùng source và corpus, bài 2 MiB mất 111,3 µs để clone và 8,07 ms để decode. Với NFD, hai số tương ứng là 27,0 µs và 1,88 ms. Mỗi case và profile có một mẫu. Timing WASM và phân phối độ trễ chưa được đo.

Khi browser mất mạng, view đang mở vẫn hiển thị trọn source 2 MiB. Sau khi rời view, mở lại offline đưa ra trạng thái tạm thời chưa đọc được. Kết nối lại và điều hướng về bài đã trả đúng toàn bộ nội dung.

Trên corpus `tiny_blocks-200000`, Chrome dựng đủ 200.000 đoạn từ 599,998 byte canonical, đối chiếu hash đủ hai lần. Lần đầu phát một Reader GET, lần thứ hai dùng cache. Lần mở đầu sau GC có 400,501 DOM nodes, JS heap 18,98 MiB và linear memory 31,25 MiB. Sau khi đóng lần hai còn 692 nodes, JS heap 12,91 MiB và linear memory 38,12 MiB. Ở lần mở thứ hai, RSS cộng Chrome là 1.556,50 MiB, sau khi đóng còn 1.197,44 MiB. Đây là một viewport desktop. Kết quả tách chi phí active view với weight giữ lại cho lần mở sau.

## Kiểm chứng và tái lập

- **PASS** Web lib với feature SSR, 243 passed và 1 benchmark ignored. Benchmark opt-in chạy riêng, 1/1. Hai đối chứng đỏ có lý do đúng assertion, test tương ứng xanh sau sửa.
- **PASS** Chrome trên source cuối, các selector `main`, `review`, `auth`, `perf-baseline`, `return-position`. Main có storage adapter, offline revisit, 404/republish. Auth dùng session thật, giữ response cũ của A tới sau khi B đã nhận danh sách, kiểm DOM và xác nhận origin chưa tạo IndexedDB. Review kiểm SSR/hydrate sau rút bài. Return-position đạt 50/50. Perf baseline có trạng thái `MEASURED_PARTIAL`, không áp timing gate.
- **PASS** Quick 13/13, PostgreSQL 3/3 `conditional_reads::`, 6/6 `publication_state::` và corpus domain 5/5. Full PostgreSQL, Kani, Quint, native/CMP và production **NOT_RUN**.
- Lượt dò ban đầu gọi tất cả Web lib khi thiếu feature SSR. Bốn test SSR và một test restore/NoOp thất bại. Đã khôi phục semantics của refresh sau restore, chạy lại với cấu hình SSR của repo và giữ log chẩn đoán.

Các kiểm tra PostgreSQL/HTTP xác nhận thứ tự visibility trước validator và projection metadata-only trong phạm vi test đã nêu. Source, title, hash, publication, immutable history và các guard Markdown thuộc backend vẫn dùng cùng implementation. READ-02 ở mức Web pilot. Quyết định tuổi offline và rollout persistence còn PENDING. `ReadOwner/PageChain` có model riêng, còn coordinator/freshness ở đây được kiểm bằng production seams và browser. Chưa đo RSS production, thiết bị yếu, Safari/Firefox, IME vật lý, byte của từng allocation hoặc đỉnh đồng thời của mọi buffer.

[Manifest và SHA-256](manifest.json) · [Raw, lệnh và probe của lượt đo](raw.zip)
