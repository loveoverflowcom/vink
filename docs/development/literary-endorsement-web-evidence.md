# Bằng chứng LVE Web adapter (#73, WP-04)

Cập nhật 10/11/2026: CHOICE-07 A đã tích hợp; xem [Pen Flight R2](pen-flight-r2-evidence.md).
Các kết quả/gap dưới đây giữ nguyên theo snapshot của lần kiểm trước.


**Ngày:** 06/11/2026 (UTC+07) · **Issue:** [#73](https://github.com/loveoverflowcom/vink/issues/73)
(WP-04, adapter app) · **Nhánh:** `codex/lve-web-endorsement`, nền PR
[#85](https://github.com/loveoverflowcom/vink/pull/85) (`8facefe`, trên `develop@23fb5af`).
Kết quả chạy trên máy dev với PostgreSQL scratch và LocalFixture loopback; **không** phải auth
production (D-05), screen reader thật, thiết bị thật hay native. Owner và bảng ánh xạ:
[runtime](literary-endorsement-runtime.md#web-adapter-wp-04); quyết định CHOICE-10 A:
[decision record](../architecture/literary-endorsement.md#lát-web-adapter-wp-04-06112026).

## Môi trường

| Mục | Giá trị |
| --- | --- |
| Máy | Ubuntu 26.04.1 LTS, Linux 7.0.0-42-generic, x86_64 |
| Rust / Node / Chrome | 1.99.1 / v24.22.2 / Chrome 156.0.8078.79 headless, DPR 1 |
| PostgreSQL | 17.11 (container `postgres:17.11-alpine` tạm, xóa sau khi chạy); `psql` 17.11 qua `VINK_PG_BIN` |
| Dữ liệu | `dev-seed` (tác phẩm mẫu, hai principal mẫu); bình luận và ghi nhận do chính smoke tạo trong database scratch `vink_verify_web` |

## HOW / WHY

- **HOW:** summary/own-state/write của API v1 → `contract.rs` → reducer thuần `endorsement_state.rs`
  (generation/session, đích, accepted order; command key, attempt, count stamp) → `EndorsementFacts`
  → `EndorsementAction` (tác phẩm sau văn bản; mỗi gốc/phản hồi qua slot `CommentEndorsements` của
  `DiscussionThread`). SSR đọc tổng số công khai in-process và serialize vào payload hydration.
- **WHY:** mọi luật thứ tự/đối soát nằm ở một reducer thuần, kiểm được bằng unit test rẻ; view chỉ
  trình bày `toggle-contract-v1`. Lane web chạy hai server trên cùng database (tác giả viết bình luận,
  principal thứ hai có grant kiểm duyệt duyệt rồi ghi nhận) vì luật recuse và “trả lời chỉ đích công
  khai” không cho một principal tự dựng đủ dữ liệu; không thêm principal hay seed mới.

## Kết quả

| Kiểm | Lệnh | Kết quả | Ghi chú |
| --- | --- | --- | --- |
| Lane quick | `cargo xtask verify quick` | PASS | fmt, clippy server/hydrate wasm32/preview wasm32, doctest type-boundary, unit + contract (`vink-web` 85 gồm 13 `endorsement_state::tests::lve_web_*`, 3 `pages::endorsement::tests`, 2 contract LVE, 1 đường dẫn; `vink-patterns` 5 gồm `thread_endorsements_render_one_action_per_target_in_provider_order`; `vink-app` 136), repository checks |
| Lane web | `cargo xtask verify web` | PASS 8/8 | Lần chạy cuối trên code chốt. Lần 1 FAIL: phản hồi chỉ được trả lời đích công khai → chuyển sang hai server; lần 2 FAIL ở duyệt Tab của reader (action tác phẩm là điểm dừng đầu tiên) → smoke duyệt `#work-endorsement` trước vùng Nhận xét; lần 3 PASS; sau đó dời “Hữu ích” về cuối hàng thao tác (ảnh khách cho thấy ghi chú lý do đẩy “Trả lời” lệch) và chạy lại PASS |
| Pha `main` (tác giả) | `endorsementAsAuthor` | PASS 6 | SSR: action sau dòng thơ cuối, trước `#feedback-region`; số công khai `0` cho revision hiện hành, không `aria-pressed`; `Unknown` trước own read (fixture không phải khách). Sau hydration: “Bạn không thể đánh giá tác phẩm của chính mình.”, `aria-disabled`, bấm không gửi lệnh; API trả `403 self_endorsement` + `no-store`. Duyệt Tab ở 320/200%: “Đánh giá cao” là điểm dừng đầu tiên sau văn bản, focus ring hiện |
| Pha `endorsement` (người đọc + kiểm duyệt) | `endorsementAsReader`, hai server cùng database | PASS 44 | Tác giả gửi gốc → kiểm duyệt duyệt → tác giả trả lời → duyệt. SSR “Hữu ích” có số trên gốc và phản hồi. Chạm đúp: một PUT, không optimistic, ACK giữ lại rồi thả → ✓ + “Bỏ đánh giá cao” + số 1. Mất ACK sau commit (`Fetch.failRequest` ở Response) → “Chưa xác nhận được.”, On giữ nguyên, bấm không gửi; “Kiểm tra lại” → PUT giống hệt byte → `replayed` → Off. Tab khác (Node) chọn rồi bỏ → bấm trên trang gửi order cũ → `409` → hiện Off + “Không thể đánh giá lúc này.”, không gửi lại; bấm mới dùng order quan sát. Enter/Space trên phản hồi: lệnh riêng, focus giữ trên cùng nút, gốc/tác phẩm không đổi số, thứ tự bình luận không đổi. PostgreSQL: tác phẩm 1, bình luận 0; SSR mới hiện 1. Ma trận 320/390/834/1024/1440 sáng/tối + 200% ở 320/390 không tràn ngang. Own read trả `401` (interception) → mọi action “Cần đăng nhập …”, số công khai giữ, không link đăng nhập. Không lỗi console/hydration |
| Ảnh đã mở | `target/verify-web/screenshots/discussion-lve-local-postgres-{390-light,320-200-dark,1440-light}.png`, `lve-signed-out-reason.png` | Đã xem | Pill tonal + ✓ chỉ sau xác nhận; nhãn xuống dòng trong nút ở 320/200%; khách: chữ lý do dưới nút, “Trả lời · Hữu ích 0” liền nhau sau khi dời |
| Lane postgres / discussion / kani | — | NOT_RUN | Không đổi lõi, SQL, migration hay HTTP provider; cổng SSR mới gọi use case có sẵn và được lane web chạy thật |
| CMP / native | — | NOT_RUN | Không đổi Kotlin; slot bình luận là của Leptos |

## Không chứng minh

- Auth production và E2E production: không có auth authority (D-05); production trả `401` cho ghi và
  own state. LocalFixture + principal mẫu chỉ là harness cục bộ.
- Glyph CHOICE-07 (slot trống), xếp hạng CHOICE-06, ghi danh CHOICE-12 B, OPEN-R4.
- Screen reader thật (NVDA/VoiceOver/TalkBack), IME/bàn phím vật lý, nhiều trình duyệt, mobile thật;
  phím được gửi bằng CDP `Input.dispatchKeyEvent`.
- SSR khách trên server không có fixture (`NoAuthority`) chỉ được kiểm bằng render Rust
  (`pages::endorsement::tests`); trình duyệt kiểm lý do đăng nhập bằng own read bị trả `401`.
- Hiệu năng/độ trễ ghi thật (lý do có thể xem lại CHOICE-10) chưa đo.
- Consumer native; UI gỡ hàng giữ lại trên đích ẩn (R7, chỉ API).
- Quint/Kani không phủ reducer phía client; unit test và smoke là bằng chứng của lớp này.
