# Kiểm chứng ba finding AC-02 còn lại của PR #367

[Draft PR #367](https://github.com/loveoverflowcom/vink/pull/367) · [Issue #353](https://github.com/loveoverflowcom/vink/issues/353) · [Kiểm chứng vòng trước](review-fixes.md) · [Ảnh trước và sau](README.md)

| Mốc | SHA |
| --- | --- |
| Base `develop` và merge-base | `ab5ecaff9685938dc689b52d98957375fbbfdf12` |
| Head Chat 3 review | `f04060b3c9350d9ba79b433c9e5bff199933f66f` |
| Source và fixture kiểm chứng vòng này | `4004e1c357504e3913e6df497a21109e931eb5ca` |

Cwd `/private/tmp/vink-353-follow-profile`. Fixture #331 cấp hai tài khoản thật trong App/Identity, dùng Chrome headless 158.0.8127.36 và PostgreSQL scratch 17.11. Mỗi lượt tạo handle mới và có cụm PostgreSQL loopback riêng.

| Finding | Thay đổi và phép kiểm |
| --- | --- |
| Riêng tư theo URL, chủ, phiên và cursor | Web xác nhận Graph cho đúng handle. Private Page và Restart phải khớp chủ canonical cùng hướng. Lượt append còn phải sở hữu đúng scope phiên và cursor đang chờ. Fixture giữ phản hồi Graph của A, đổi cookie trình duyệt scratch sang phiên thật B, rồi đọc Page của B. Lượt append dùng cursor của A với cookie B để nhận Restart của B. Cả hai phải quay về trạng thái công khai của URL A. Unit kiểm thêm sai hướng và sai URL canonical. |
| Vòng đời summary công khai | Projection của trang hồ sơ sở hữu lượt đọc/thử lại. Ticket bỏ phản hồi của lượt cũ khi có dữ kiện mới hoặc trang đã bị tháo. Chrome làm mất ACK của PUT thật và chỉ trả 503 cho summary. Đăng xuất cùng browser context giữ trang đang mở và hiện Thử lại cho khách. Giữ GET của lần thử lại, đăng nhập lại rồi trả GET. Link Người theo dõi và tổng 0 Đang theo dõi trở về độc lập. |
| Back từ hàng đã tải thêm | Back chỉ lưu cursor với định vị và đọc lại trang trong scope hiện tại. Ở trang khôi phục, Enter trên “Về trang đầu” xóa định vị khôi phục trong history, tải 12 hàng đầu và đưa focus về H1. Sau đó vẫn tải thêm được, và Back khi hàng mất vẫn về H1. |

## Kết quả trên `4004e1c`

- **PASS** `cargo xtask verify web --scenario password`, lượt `1795186927425599000-5948`. App và Identity chạy với PostgreSQL scratch. Kịch bản bao phủ ba dòng trên, cùng các phép kiểm Follow, alias và focus có sẵn trong fixture #331.
- **PASS** `python3.13 ./tools/local-budget run -- cargo xtask verify unit web --filter follow`, 14/14 test được chọn. Test mới kiểm Page và Restart với chủ khác, hướng khác và URL canonical sai.
- **PASS** `cargo fmt --all -- --check`, `git diff --check origin/develop...HEAD`, `node --check` trên `profile/navigation.js`, `author-follow-smoke.mjs` và `auth-password-smoke.mjs`.

**NOT_RUN** ma trận toàn Epic #351, submit no-JS trên hai trang danh sách, trình đọc màn hình và zoom trình duyệt. Chrome đã kiểm Back nhưng lượt này không có BFCache hit thật. Các ảnh trước và sau thuộc lượt gốc với provenance riêng. [Log kiểm chứng](review-ac02-checks.txt)

## Lượt trước khi chốt fixture

- **FAIL** ở lượt chạy trên bản chưa commit vì assertion “Về trang đầu” chạy sau khi cursor bị thu hồi. Lượt `1795186656907287000-3566` tại `2c30f76d6a190e5ee4a6c69d792fcecccf2b16dc` đã xác nhận Enter đúng tại trang khôi phục, rồi dừng vì assertion trùng ở màn khác.
- **FAIL** toàn lane tại `2753ce7a131fc62d30e47ec0157a174a9cb2ad52`, lượt `1795186774973892000-4755`. Ba regression AC-02 đã qua. Postgres từ chối lệnh dọn `DELETE` do giữ lịch sử cạnh. Fixture cuối chuyển các cạnh đó sang không hoạt động và tăng phiên bản/thứ tự.

Sau khi các lượt chạy kết thúc, `lsof` không thấy tiến trình giữ `target`. Dọn 4,44 GiB trong
worktree #353 bằng `cargo clean --offline --profile dev --target-dir target` và xóa hai thư mục
build WASM/bundle đã xác minh. Giữ `target/verify-web`, log scratch, ảnh và các Git ref.
