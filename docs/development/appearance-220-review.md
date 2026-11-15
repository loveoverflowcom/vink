# Review và hòa giải Giao diện #220 sau #219

Phạm vi [#209](https://github.com/loveoverflowcom/vink/issues/209), UX-01 và
WR-01/WR-02. Đây là self-review của delta sau hòa giải; concept vẫn
`NO_APPROVED_REFERENCE`, không có quyết định sản phẩm hoặc native/CMP mới.

## Nguồn và kết luận review

- `develop`: `c3b6e7da2a1ebfdb13b8c37817be4a771e0bbc1d`, đã gồm #219
  (`97b29c30a87b5403759b2219ab0cdc710fb64cbd`) và #222 cho #214.
- #220 gốc: `3f5125529b3fbdb1c3909e0619045e5673c718bd`, mở, chưa merge;
  issue #209 và thảo luận hai PR không có comment tại lượt đọc này.
- Commit runtime hòa giải: `17d6344e367e9816025352b262938dc6fe92c1ef`.
  Merge `develop` vào nhánh PR giữ lịch sử gốc, không force-push.

Recipe chung, tonal selected, dấu check, bỏ divider và nhãn đầy đủ đã được #219
triển khai. Không đưa cả hai recipe vào `develop`: giữ owner CSS, cấu trúc một
cột, gutter tối đa 32 px ở 200%, cùng sửa focus/tiêu đề trên màn thấp của #219.
Gutter `7vw` của phương án gốc cho 22,4 px ở 320/100, mâu thuẫn assertion 24 px
của `accountLayoutGeometry` hiện hành; ở 200% nó còn có thể lên 48 px. Không
mang các assertion recipe cũ vào để áp hai đặc tả khác nhau cho cùng surface.

Delta hữu ích giữ lại trong owner hiện có:

1. Dấu check 16 px thay 12 px; cột dành chỗ cũng tăng tương ứng ở mọi lựa chọn.
2. Vòng focus của mục đã chọn dùng `on-primary-container` trên nền tonal để
   tăng tương phản. Tính từ token hiện hành: sáng 4,63 → 10,84:1; tối
   5,93 → 8,23:1. Mức cũ đã đạt ngưỡng hiện hành; đây là polish, không gọi
   sai thành lỗi tương phản. Forced-colors có rule phía sau giữ `HighlightText`.

Không đổi Rust/DOM/state, native radio, modal/inert/scrim, bootstrap/storage,
rail anchor, navigation guard hoặc dữ liệu #214. Tài liệu phương án thay thế
không được nhân thành owner mới; spec hiện hành là
[me-preferences-r1-209](../design/me-preferences-r1-209.md). Ảnh/tài liệu gốc
vẫn có trong commit #220 gốc; bản sao local ở `/tmp/vink-220-original-evidence`.

## Kiểm chứng local

Cwd `/tmp/vink-220-review`; macOS arm64, Rust 1.99.1, Node 22.15.0,
PostgreSQL 17.11 scratch riêng tại 127.0.0.1:55520, Chrome 157.0.8103.78 headless/CDP.
Các lệnh Cargo dùng `CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0`; target dùng lại qua symlink tới target của checkout chính.
Symlink hạ tầng và tài liệu review đang soạn chưa track khi chạy;
source runtime sạch ở SHA trên.
Chữ 200% là root font-size, không phải browser zoom. Không xem/chờ GitHub CI.

| Lệnh / boundary | Kết quả |
| --- | --- |
| `cargo xtask verify quick` | **PASS**, exit 0: fmt, server/WASM Clippy, doctest, unit/contract và repository checks |
| `cargo xtask verify web --scenario main`, thêm `npm_config_cache=/tmp/vink-220-npm`, `VINK_TEST_DATABASE_URL=<scratch>`, `VINK_CHROME=<Chrome>` | **FAIL**, exit 1 ở caret 390×420; tất cả kiểm Giao diện trước đó PASS, không nhận full Web PASS |
| `python3 /tmp/vink-220-focused/run.py` | **PASS**, exit 0: App + PG thật, native CDP selection; nguyên helper `shellWithDirtyEditor` được gọi riêng sau setup bản nháp đã lưu rồi thêm khổ chưa lưu |

Lượt Web đầu **FAIL** ở npm cache dùng chung (`EACCES`, chưa tới browser); chuyển
cache sang `/tmp` và giữ log cũ, không sửa dependency/source/assertion. Probe đầu
cũng dừng trước browser do đường dẫn import khi copy; bản cuối chỉ sửa import
sang file nguồn và dùng database/role scratch riêng.

`main` cuối: run `1794772990566793000-67600`. Caret top `-118.71875`, bottom
`-93.71875`, viewport height 420, toolbar `static`, không console error. Trùng
[baseline/head #222](desk-author-feedback.md#kiểm-mới-của-reviewer) đã ghi tại
các SHA/run riêng. [Ảnh lỗi đã mở](evidence/appearance-220/editor-short-mobile-caret-failed.png).
Đây là gap có sẵn, không sửa owner editor trong scope #220.

`shellWithDirtyEditor` nằm sau điểm dừng này trong `main`, nên là **NOT_RUN**
trong lượt aggregate. Probe bổ sung gọi nguyên helper này trong bản sao `/tmp`,
không đổi hoặc bỏ assertion caret. Nó giữ cùng EditorView, chữ Việt/khổ thơ,
selection và trạng thái unsaved qua màu/cỡ chữ/rail, Back/Escape, guard tới các
route, resize mobile; số POST/PUT phát sinh là 0 và version server không đổi.
[Script setup](evidence/appearance-220/focused-probe.py),
[dispatcher bổ sung](evidence/appearance-220/focused-dispatch.txt),
[hash helper/script](evidence/appearance-220/focused-provenance.json) và
[log](evidence/appearance-220/focused-output.log) tách khỏi runner canonical.
Đây là diagnostic local, không phải selector xtask mới. Khi tái lập phải chỉnh
`root` tới checkout đúng SHA và khởi động PG scratch tại port của script.

[Summary nguyên trạng](evidence/appearance-220/summary.txt) lưu trạng thái runner.
Log đầy đủ còn tại `/tmp/vink-220-quick.log`, `/tmp/vink-220-web.log`,
`/tmp/vink-220-web-artifacts-fail.log`, `/tmp/vink-220-focused/output.log`.
Artifact/captures nguyên run nằm dưới `target/verify-web/<run_id>` của checkout chính.

### Self-review năm chiều

| Chiều | Kết quả / giới hạn |
| --- | --- |
| Functional | **PASS** scoped Giao diện: native Arrow/Tab/checked, theme/system/text/rail, reload/persistence, first paint và storage denied trong phần `adaptiveShell` đã chạy. Aggregate `main` **FAIL** như trên |
| Geometry | **PASS** 54 ca panel + 36 ca Tôi, một gutter/scroll owner, containment ±1 px; control nhỏ nhất 99,25×48 px. Thêm ca mở 834×320/280, 200%, title/Close visible và scrollTop=0 |
| Visual | **PASS** scoped, self-review: đã mở 7 ảnh appearance và ảnh overlay trên editor dirty; nhãn nguyên, check rõ, tonal selected và hierarchy #219 giữ lại. `NO_APPROVED_REFERENCE`, không tự duyệt mốc |
| Accessibility / Responsive | **PASS** scoped Chrome: 100/200%, 320–1440 px; vòng focus đo 10,84:1 sáng, 8,23:1 tối, 19,04:1 forced-colors; native DOM/keyboard semantics và hit targets. Screen reader/OS scale **NOT_RUN** |
| Interaction | **PASS** scoped Giao diện + probe editor; full `main` vẫn **FAIL** ở caret resize có sẵn. Input/composition là chương trình, OS IME **NOT_RUN** |

[Số đo và hash ảnh](evidence/appearance-220/measurements.json) pin đúng runtime SHA;
ảnh chỉ có giá trị tại viewport/theme/font đã ghi. Một vòng hòa giải/render,
không đổi tiêu chí hoặc assertion để đạt. Không có finding mới còn mở trong delta
CSS sau self-review; lỗi caret có sẵn giữ nguyên trạng thái FAIL.

**NOT_RUN:** phần `main` sau điểm dừng ngoài helper được gọi riêng, các selector
Web khác, full PostgreSQL lane, OS IME, browser zoom, screen reader/thiết bị thật,
Safari/Firefox, CMP/native. UI primitive/token không đổi. GitHub CI **NOT_RUN**
theo yêu cầu; không deploy, không tick/đóng #209 và không khởi động hàng đợi UI cũ.
