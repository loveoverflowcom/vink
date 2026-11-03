# Thêm và sửa UI Vink

Giá trị do [tokens.json](shared/tokens.json) sở hữu; API/semantics do renderer sở hữu;
effect/authority do consumer sở hữu. Contract [components](shared/components.md),
[state](shared/states.md), [navigation](shared/navigation.md) và
[patterns](../vink-patterns/README.md) vẫn là nguồn anatomy/variants/slots/accessibility.
Mapping mọi public component tới entry nằm ở [api-coverage.json](shared/api-coverage.json).
Đó là admission map, không phải kết quả kiểm chứng.

## Sửa một semantic role

Ví dụ đổi màu chữ đọc: sửa alias `color.on-surface.light/dark` trong `shared/tokens.json`,
trỏ một primitive đã khai báo; không đặt hex tại consumer. Tăng version patch nếu chỉ sửa
giá trị giữ nguyên nghĩa; minor nếu thêm role; major nếu xóa/đổi nghĩa/đổi contract không tương thích.
Không đổi version vì formatter hay chuyển nguồn preview. Chạy từ repository root:

```sh
cargo xtask ui-tokens
cargo xtask ui-tokens
cargo xtask ui-tokens --check
cargo xtask verify quick
cargo xtask ui-preview leptos verify
cargo xtask ui-preview cmp verify
```

Hai lần generate phải cho cùng diff. `--check` bắt missing/drift; unit test generator so hai
output độc lập. Không sửa CSS/Rust/Kotlin sinh ra. Mở ảnh và đo lại contrast ở consumer cả hai
theme; token arithmetic không đo nền ảnh/composited focus. Ngưỡng chữ thường 4.5:1, chữ lớn
3:1 (≥24px hoặc ≥18.66px bold), focus/control 3:1 với nền liền kề. Không dùng ngưỡng chữ lớn
để bỏ qua chữ metadata nhỏ.

## Tạo scope và recovery có thể replay

`composition.nested/default` là ví dụ chạy được ở cả hai gallery; `foundation.color-roles/nested-theme`
cho thấy scope System bên trong Dark. Leptos dùng `ThemeScope`, CMP dùng `VinkScope`; override
chỉ role → role, không literal. Thư viện từ chối vòng/trùng/tự trỏ. Reduced motion cộng dồn,
scope con không bật lại motion khi OS/cha đã giảm.

Trong Web mở `#composition.nested/default`; CMP Desktop nhận
`--entry=composition.nested --scenario=default --theme=dark --motion=reduce --scale=200`
qua Gradle `:preview:run --args='...'`. Native nhận cùng entry/scenario và các tùy chọn
qua [lệnh diagnostic](../../docs/development/README.md#diagnostic-ui-native-uis-06).
Clock/seed/text đều từ `shared/fixtures/foundation.json`; ghi viewport/density/OS scale thực
vào evidence. `scale=200` là multiplier của host, không thay OS Dynamic Type hay browser zoom.

`state.error/partial`, `state.playground/loading-error-retry` và
`composition.feedback/uncertain` minh họa recovery: component nhận Problem/phase và phát intent;
adapter quyết định request. Không tự fetch, blind retry hay coi uncertain là saved. Catalog
và API typed nhận immutable facts + callback; không nhận DTO, token hay session.

## Thêm component hoặc state

1. Nêu anatomy, owner, variants/slots, token roles, disabled/pending/error/focus và known limits
   trong contract thích hợp. Field controlled phải giữ selection/composition, label và lỗi.
2. Thêm API ở Leptos và CMP commonMain, cùng state/identity vectors nếu có luật thuần.
   Không import HTTP/DB/router/auth; platform API đi qua adapter/expect-actual.
3. Đăng ký stable entry/scenario ở `preview-catalog.json`, cập nhật version và `api-coverage.json`.
   Thêm vào REGISTERED độc lập của hai preview và render bằng exported component thật.
   Không lấy danh sách REGISTERED từ catalog để tự chứng minh completeness.
4. Một renderer thiếu counterpart phải ghi `unsupported.<renderer>` với lý do rõ. Native target
   chưa chạy vẫn phải ghi NOT_RUN riêng; không đổi thành unsupported để bỏ qua implementation.
5. Thêm test phản ví dụ: click đôi/pending, phản hồi cũ, thiếu tên, Unicode, lỗi ảnh, giữ input,
   focus/keyboard, hidden/background/disposed, giảm motion. Mở ảnh kịch bản khó hẹp/rộng,
   hai theme/chữ lớn. Với motion, giữ timed frames và assertion; một PNG không chứng minh chuyển động.
6. Chạy các command trên, `verify web` nếu consumer Web đổi. Native common/build/UI runtime
   có lệnh/kết quả riêng. Ghi readiness theo target vào evidence, không suy parity từ compile.

Thêm avatar tone cần cặp nền/chữ contrast và golden vectors Web/Kotlin; font role cần provenance,
license, fallback và pipeline từng target; loading variant cần delay/min-visible/stop gate cùng
callback/lifecycle tests. Preview/test module không được trở thành dependency production.

## Version và migration

Ghi thay đổi ở [compatibility](shared/compatibility.md), giữ legacy name là alias sinh từ cùng
owner, kèm điều kiện gỡ. Không tạo palette/style source thứ hai. Mỗi API thêm cần khai mapping;
`verify quick` phát hiện API thiếu scenario, mapping cũ, dependency preview và style literal chưa
có owner/reason. Scan là guard có phạm vi, không thay review dynamic selectors/build assets.

UIS-06 giữ token 0.4.0/API production, catalog 0.5.1 thêm lý do Web adapter không đối ứng CMP.
Internal diagnostic 0.1.0 chuyển specimens Desktop vào common source; không publish package hay app.
