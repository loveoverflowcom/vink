# Kiểm chứng Web V-pen R1 #117

Baseline: `f23df33e6776dd08bd089bf8a0fd767dcd3cbe0e`.
Implementation được chạy: `a78960fd896fe0f85a192f08f0e74f202592491e`,
tree `d33b59c4c70b931aa5442d1fa5a408ecbfd3e199`. Commit evidence sau đó chỉ thêm tài liệu/ảnh/log.

Môi trường: macOS, Rust 1.99.1, Node 22.15.0, Chrome headless (UA trong contexts),
PostgreSQL 17 tạm riêng tại loopback 55437. Dữ liệu `dev-seed` tổng hợp có nhãn mẫu.
Không dùng database phát triển, không deploy hoặc kiểm GitHub CI.

## Lệnh và kết quả

Cwd là checkout Vink (`$REPO` trong log đã chuẩn hóa đường dẫn).
Các lượt Cargo đặt `CARGO_INCREMENTAL=0`, `CARGO_PROFILE_DEV_DEBUG=0`,
`CARGO_PROFILE_TEST_DEBUG=0`, `CARGO_PROFILE_DEV_CODEGEN_UNITS=16`,
`CARGO_PROFILE_TEST_CODEGEN_UNITS=16`, `CARGO_BUILD_JOBS=2` để giới hạn bộ nhớ/đĩa.

- **PASS** `cargo xtask verify quick` tại implementation HEAD: [quick.log](quick.log).
- **PASS** `cargo xtask verify web` tại cùng HEAD: [web.log](web.log). Đặt
  `VINK_CHROME` tới Chrome macOS, `VINK_TEST_DATABASE_URL` tới cluster tạm và cache npm riêng.
  Bao gồm SSR/hydration, real asset HTTP/MIME/decode, theme/OS/storage denied, keyboard,
  editor lưu/reload/conflict/leave guard, SIGTERM/restart/export và endorsement.
- **PASS** guard âm `brand_rejects_rehashed_artwork_and_unrecorded_legacy_assets`
  trong quick: hash runtime đã sửa vẫn bị source manifest chặn; file circle ngoài record bị chặn.
- **PASS** [compiled-brand-audit.json](compiled-brand-audit.json): đúng 21 file được phục vụ,
  đủ hash/bytes; 18 artwork nguyên byte từ ZIP; CSS chỉ R1; source ZIP/board/provenance không phục vụ.
- **PASS** source SVG mark/lockup Light/Dark có cùng mọi path geometry; HTTP browser canvas
  xác nhận alpha PNG giống nhau ở 24/32/48/96/192/512. URL circle đại diện trả 404.
- **PASS** 16 baseline + 16 R1 captures: 320/390/1024/1440 × Light/Dark × root text 100/200%,
  DPR1, cao 900, fixture/route `/` giống nhau. [Before contexts](before/contexts.json),
  [after contexts](after/contexts.json), [geometry delta](geometry-comparison.json).

**Giữ kết quả lỗi:** lượt full Web đầu thất bại ở `selection after mobile resize`
([log](web-first-attempt.log)), trong khi EditorView/chữ/dirty guard vẫn giữ và không có browser
exception. Không đổi code editor hay nới assertion. Probe riêng gọi lại đúng helper
`shellWithDirtyEditor` trên fixture mới **PASS** ([log](focused-probe.log)); sau đó full Web lặp lại
**PASS**. Đây là dấu hiệu timing/context nhạy của smoke, không được diễn giải thành đã chứng minh
root cause hoặc đã sửa một lỗi editor. Lỗi thiếu đĩa, socket sandbox, cache npm EACCES và đường dẫn
Chrome ở các lần chuẩn bị được giữ trong thư mục evidence ngoài checkout; không đổi chúng thành PASS.

## Ảnh thực đã mở và tự review

Đã mở source reference/concept và [small-size contact sheet](brand-contact-sheet.png): V/ngòi
vẫn nhận ra ở SVG 16px, SVG/PNG 24/32/48px; dark không dùng filter đảo màu.
Đã mở các cặp dưới đây cùng kích thước, đối chiếu hình và header:

| Kịch bản | Trước | R1 |
| --- | --- | --- |
| 1440 Light, chữ 100% | [ảnh](before/explore-1440-light-100.png) | [ảnh](after/explore-1440-light-100.png) |
| 320 Dark, chữ 200% | [ảnh](before/explore-320-dark-200.png) | [ảnh](after/explore-320-dark-200.png) |

Ngoài ra đã mở R1 390 Light 100%, 1024 Dark 200%, ảnh dark trước hydration và mono forced colors
trong full run ngoài checkout. Các biến thể còn lại được capture và kiểm geometry/overflow,
không tự gọi tất cả là pixel-inspected. Không phát hiện lỗi mới cần sửa trong review scope:
logo duy nhất ở góc phải, signature không trùng chữ, đúng silhouette/giọt i, nền trong suốt,
không đẩy search/heading hoặc thêm bề mặt gradient. Forced colors có wordmark mono rõ.
Toàn bộ Web captures/contexts và 32 ảnh so sánh được giữ ngoài `target` trước cleanup.

## AC và phần chưa nghiệm thu

| AC | Kết luận local | Bằng chứng / giới hạn |
| --- | --- | --- |
| 01 | PASS | inventories trước/sau source + named asset paths; compiled family; HTTP old URLs 404 |
| 02 | PASS | ZIP bytes, SVG geometry, canvas alpha, contact sheet 16/24/32/48 và signature |
| 03 | PASS | 16 variants, placement assertions, không logo rail/route khác, keyboard target |
| 04 | PARTIAL | Web first paint/explicit/System/OS/reload/storage-denied, MIME/decode/manifest PASS; cài PWA thực, OS launcher/masking và phiên browser cũ trước migration NOT_RUN |
| 05 | PASS | pinned ZIP/manifest, source hashes, guard âm, build copy, docs và lịch sử #34 |
| 06 | PASS trong Web Chrome | full Web lặp lại + probe; giữ lịch sử FAIL timing. Không có performance trace CLS tổng thể hoặc IME/screen-reader/device thật |
| 07 | PASS | lệnh/log thật, source revision, captures mở và coverage/gaps ghi rõ |

Delta checklist đề xuất: tick 01/02/03/05/06/07; giữ 04 unchecked và issue mở.
Chưa ghi delta này lên GitHub vì remote publication đang bị chặn quyền/approval.
