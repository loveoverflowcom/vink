# Chọn cách deploy Vink production

Áp dụng cho website trước cutover SPA, với Pages phục vụ asset và VPS chạy App, Identity, Worker cùng Notifications nếu đã kích hoạt. Mỗi lần phát hành chọn đường đi theo **SHA đang phục vụ** và **SHA trong receipt GHCR**. Quyết định ưu tiên phát hành sớm đã được ghi ở [VDR-EARLY-PRODUCTION-RELEASE@r1](../../decisions/signoffs/VDR-EARLY-PRODUCTION-RELEASE.r1.toml). Tên chế độ dưới đây dùng trong sổ vận hành. Controller hiện dùng các cờ ở cột cuối.

| Chế độ | Khi nào chọn | Backup và cách chạy hôm nay |
| --- | --- | --- |
| **FAST** | Cùng DB, role, volume, service và migration. | Giữ timer hằng ngày. Khi file Compose nền, production và overlay Resend giống nhau, controller bỏ bước đòi dump. Dùng HTTP smoke trên controller với `--http-only`. |
| **SCHEMA** | Lịch sử cũ là tiền tố nguyên vẹn. SQL mới chỉ mở rộng, giữ nguyên dữ liệu hiện có và hành vi đọc/ghi của predecessor. | Chỉ đọc bộ backup hằng ngày gần nhất, ghi timestamp UTC, SHA-256 của `SHA256SUMS` và cảnh báo vào `daily_backup`. Dùng `--schema-review` với `action=expand`, `changes_existing_data=false` và bằng chứng tương thích thực đo. Có thể thêm `--http-only` khi đã có Pages predecessor. |
| **BOOTSTRAP** | Thêm DB hoặc service, ví dụ Notifications lần đầu. | Kiểm nguồn gốc DB, role và volume rồi provision đúng phần mới. Chọn tiếp SCHEMA hoặc MAINTENANCE cho các DB cũ theo diff. DB vừa tạo chưa có dữ liệu cần khôi phục của predecessor. |
| **MAINTENANCE** | Migration contract, đổi kiểu, xóa cột/bảng, backfill hoặc cập nhật dữ liệu hiện có, hay binary cũ mất readiness. | Kiểm bộ backup đủ DB và khả năng đọc dump, drain, dump và clone sau drain, migrate, probe riêng rồi mở ghi. Dùng `--maintenance --schema-review` theo [thủ tục cửa sổ bảo trì](cloudflare-edge.md). |

**Mất hoặc thiếu backup khác với mất DB.** Với SCHEMA đã review, backup thiếu, ngoài 26 giờ hoặc thiếu dump của một owner chỉ tạo `WARNING`. Controller tiếp tục và giữ kết quả quan sát lúc bắt đầu trong journal khi resume. Đây là kiểm kê file và hash của manifest. Việc đọc được danh mục backup chưa chứng minh restore thành công. Nếu timer ngừng lâu, khoảng dữ liệu phát sinh sau backup gần nhất có thể vượt một ngày. Với MAINTENANCE phải có bộ backup khôi phục được, gồm cả DB giữ lại sau rollback nếu còn dữ liệu. Giữ timer hoạt động trong mọi chế độ.

## 1. Xác định predecessor và xử lý 503

Trên release host, lấy revision từ `readlink -f /opt/vink-prod/current`, đối chiếu digest/label của container đang chạy, `active.json`, `pending.json` (nếu có) trong `/var/lib/vink-production-release` và deployment canonical ở Pages. Cách đọc metadata nằm trong [bàn giao production](production-handoff.md). Kiểm lịch và lần chạy gần nhất bằng `systemctl list-timers --all vink-prod-backup.timer` và `journalctl -u vink-prod-backup.service -n 30`. Đọc log App/Identity và body readiness trong cửa sổ xảy ra 503. Đối chiếu ledger theo từng DB thực tế bằng truy vấn chỉ đọc sau.

```sql
SELECT version, success, encode(checksum, 'hex') AS sha384
FROM public._sqlx_migrations ORDER BY version;
```

So số phiên bản và SHA-384 của từng SQL đã áp dụng với migration nhúng trong SHA đang phục vụ. Binary hiện tại trả `diverged` khi gặp phiên bản lạ hoặc checksum lệch. Ghi nguyên nhân 503 sau khi đối chiếu log, ledger, role và socket. Nếu đã có journal dở, xử lý journal đó trước khi mở lượt mới.

Trong checkout có cả hai Git object, kiểm hai SHA và đọc nội dung SQL thay đổi. Dùng `git diff "$OLD_SHA" "$NEW_SHA" -- services/vink-{app,identity,notifications}/migrations` trong Bash. Đối chiếu cả `infra/production`, `services/postgres`, unit/drop-in đang cài, tên volume gắn vào PostgreSQL và catalog DB/role thực tế. Đổi tên migration đã áp dụng, sửa checksum hay hàng `success=false` cần điều tra trước khi ghi tiếp. Một thay đổi Compose chưa phân loại dùng đường kiểm backup hiện có cho đến khi xác minh tác động.

## 2. Build, publish và chuyển release

1. Dispatch [Publish OCI images](../../.github/workflows/publish-images.yml) trên `develop` với platform đã kiểm trên VPS, `publish=true`, `api_only=false`, `website=false`. Giữ JSON digest từ chính job publish. Không cần đợi toàn bộ GitHub CI. Ghi `WARNING` cho timeout hạ tầng hoặc lỗi đã biết của lane ngoài đường phát hành, kèm run/lý do. Build, smoke image với PostgreSQL scratch và push digest là các bước workflow đang thực thi trước khi cấp receipt. Nếu thiếu digest, sửa hoặc chạy lại bước publish trên đúng SHA. [Lệnh dispatch và cách lấy receipt](production-handoff.md#chạy-qualification-hoặc-publish-oci)
2. Giữ receipt của predecessor. Trên release host, trong tree sạch của candidate, nạp biến từ [runbook edge](cloudflare-edge.md#từ-build-đến-receipt). Với FAST và SCHEMA dùng Bash như dưới đây. `$READER` là UUID của tác phẩm công khai đã kiểm. Nếu catalog thật rỗng, thay cặp `--reader "$READER"` bằng `--empty-catalog`. Với SCHEMA, đặt `$SCHEMA_REVIEW` trỏ tới báo cáo đúng hai SHA trước khi chạy.
3. Kiểm kết quả `plan` rồi mới chạy hai lệnh thay đổi. Khi tiếp tục sau mất kết nối, đọc `status` và dùng đúng input đã ghi. Lượt `website=true` qua SSH receiver vẫn dùng browser mặc định và yêu cầu UUID thật.

```bash
r=infra/cloudflare/production/release.py
args=(/secure/release.json --account "$CLOUDFLARE_ACCOUNT_ID"
      --previous-origin "$OLD_SHA" --previous-edge "$OLD_EDGE"
      --reader "$READER" --http-only 'Early production, kiểm qua HTTP')
# Chỉ thêm cho SCHEMA
# args+=(--schema-review "$SCHEMA_REVIEW")
python3 "$r" plan "${args[@]}"
```

```bash
python3 "$r" apply --preview-only "${args[@]}" &&
  python3 "$r" resume "${args[@]}"
```

Sau promote, kiểm origin readiness, digest từng container, asset của cả hai SHA, HTTP ở apex và Reader đã chọn. Browser tùy chọn ở đường rolling trực tiếp. Khi cần kiểm hydration, bỏ `--http-only` và dùng Chrome cục bộ với UUID Reader. Browser ngoài qua SSH hỗ trợ rolling cùng schema và bảo trì. Ghi `NOT_RUN` và lý do cho mọi phép kiểm đã bỏ qua. [Cấu hình browser](cloudflare-edge.md#browser-trên-máy-kiểm-riêng)

## 3. Đổi schema và đưa Notifications vào lần đầu

**SCHEMA.** Người review đọc toàn bộ SQL, gồm function/trigger và tác động lên writer cũ. Chỉ khai `changes_existing_data: false` trong JSON khi xác nhận SQL thuần bổ sung. Backfill, `UPDATE` hay thao tác có thể làm mất dữ liệu đi qua MAINTENANCE. Từng owner trong schema review vẫn cần kết quả chạy thật. Nếu binary cũ không đọc được schema mới, dùng đường bảo trì/clone. Review cũ thiếu trường phân loại tiếp tục dùng cổng backup cũ. [Hợp đồng controller](../cloudflare/production/release.py)

**BOOTSTRAP.** Đối chiếu manifest đang chạy, receipt đã mở origin, DB/OID và role thực tế, volume đã gắn, các bộ backup và lần provision trước. `plan` cũng tạo archive candidate. Image trong archive là lý do tra thêm nhật ký, tự nó chưa chứng minh DB đã tồn tại. Phải giải thích được mọi dấu vết trước khi kết luận đây là lần tạo mới. DB, role hoặc volume đã có nay biến mất thì giữ nguyên bằng chứng và khôi phục theo hồ sơ của nó. Không tạo đối tượng rỗng cùng tên để tiếp tục.

Nếu đúng là lần đầu, dùng credential riêng cho Notifications, giữ nguyên credential hiện hữu, hoàn tất file còn thiếu từ cùng bộ đã cấp và kiểm lại binding runtime, thư mục socket, chủ sở hữu và drop-in systemd. Chỉ chạy provision đa owner sau khi đã xác minh App/Identity đầy đủ. Làm theo [thủ tục chuyển từ ba sang bốn image](README.md#bổ-sung-notifications-vào-release-ba-image-đang-chạy). Với BOOTSTRAP + SCHEMA đã review, khai thêm `bootstrap_service: "notifications"` sau khi xác nhận diff Compose production chỉ phục vụ lần thêm Notifications. Controller so Compose PostgreSQL và overlay Resend byte-for-byte, rồi dùng bộ timer của predecessor. Nếu tree cũ thiếu inventory của owner mới hoặc maintenance đòi owner chưa tồn tại, controller hiện chưa hỗ trợ chuyển kết hợp đó. Tách việc thêm owner khỏi migration contract thành các release kiểm chứng riêng. Giữ các DB bền vững đã tạo khi quay về image cũ.

**MAINTENANCE.** Kiểm SHA256SUMS, danh mục bằng `pg_restore --list` và bằng chứng restore trên DB cô lập trước thao tác phá dữ liệu. So topology với các DB bền vững thực sự đang có, gồm cả DB tạm ngừng phục vụ. Ghi `changes_existing_data: true` trong schema review cho migration sửa dữ liệu. Khi đó cả hai binary có thể vẫn ready. Ghi kết quả thực tế của `old_on_result` và `old_on_restored`. Controller giữ cổng backup trong 24 giờ và đối chiếu hash với báo cáo diễn tập. Nếu backup có DB bền vững ngoài tập owner controller nhận diện, đối soát topology trước khi mở cửa sổ. Giữ nguyên bộ dump đầy đủ.

## 4. Khi có lỗi và khi bàn giao

| Quan sát | Hành động |
| --- | --- |
| Thiếu image, thư mục hoặc file runtime mới | Pull lại đúng digest. Dựng lại phần thiếu từ release và credential đã xác minh, kiểm quyền và chạy lại preflight. Khi chưa rõ kết quả lần chạy trước, đọc lại provider, container và journal trước khi retry. |
| Migration lỗi, dirty hoặc checksum lệch | Dừng ghi migration. Đối chiếu độc lập từng owner. Rolling đổi env và symlink trước khi migrate, nên một số DB có thể đã tiến dù container còn ở bản cũ. |
| FAST qua HTTP hỏng sau khi đổi origin | Đối chiếu journal và điều kiện [rollback về predecessor](cloudflare-edge.md#quay-lại-predecessor-của-lượt-http-only). Giữ cả hai bộ asset, kiểm lại HTTP/health của bản phục hồi. Ghi lượt candidate thất bại kể cả khi dịch vụ đã hồi phục. |
| MAINTENANCE trước khi mở ghi | Dùng `rollback-origin` khi journal cho phép, kiểm OID/ledger và readiness của predecessor. Từ `origin_opening`, có thể đã nhận writes mới. Đối soát và phục hồi theo chiều tiến. |
| Health chưa hồi phục, mất tài nguyên từng tồn tại, hoặc migration phá dữ liệu thiếu backup khôi phục được | Ghi `BLOCKED`, triệu chứng, bước khắc phục đã thử và điều kiện cụ thể để tiếp tục. Giữ journal, DB, volume và backup. |

Bàn giao mỗi lượt gồm chế độ và BOOTSTRAP nếu có, SHA trước/sau, digest image, Pages deployment ID, trạng thái cuối của origin/HTTP, timestamp UTC và hash manifest backup đã dùng hoặc lý do thiếu, `WARNING`, `NOT_RUN` và kết quả rollback. Chỉ ghi `PASS` cho phép kiểm đã chạy.

**Thiết kế chuyển tiếp (Provisional).** Để bỏ diễn tập thủ công cho SCHEMA, cần phát hành trước một bộ binary hiểu lịch sử migration dạng tiền tố và epoch tương thích trên từng DB. Chỉ báo `ahead_compatible` khi checksum phần tiền tố khớp, mọi migration thành công và epoch của DB được binary hỗ trợ. Contract phải nâng epoch. Cần kiểm PostgreSQL thật và readiness của App/Identity/Notifications cùng worker trước khi dựa vào image rollback. Controller hiện cũng chưa tự phân loại toàn bộ diff, tự provision hoặc tự rollback mọi pha. Merge tài liệu không kích hoạt các khả năng này.
