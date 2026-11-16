# Review độc lập PR #248 — #230 owner-state metadata projection

- PR: [loveoverflowcom/vink#248](https://github.com/loveoverflowcom/vink/pull/248), issue
  [#230](https://github.com/loveoverflowcom/vink/issues/230).
- Pin: HEAD `f55490f06056e4a4ed20af6613ff155a8b75c3f4`, base/merge-base
  `develop@ca9369653a516715fea95ca71fbd818d54c15b0a` (không có commit develop mới hơn lúc review).
- Phạm vi đọc: toàn bộ diff `services/`, các caller của `load_paired_state`
  (`publication.rs` ×2, `moderation.rs` ×2, `evaluation.rs`, `appeals.rs` ×2), mọi consumer của
  `AuthorPublicationStateFacts`, `PublicationStore::publication`, các fixture SQL chèn
  `revisions.content_hash` (tests, `xtask/src/provision.rs`, `apps/vink-web/e2e/fixtures`), SSOT và
  evidence #230. CI GitHub: không đọc/chờ theo yêu cầu.

## Kết luận

Không có finding P0–P2 trong code. Projection đạt CHOICE-01 A / CHOICE-02 A như issue mô tả:

- `PublicationState`/`ModerationState` chỉ có `restore`/`facts`; không có đường chuyển sang
  `Publication`/`WorkModeration` (doctest `compile_fail` có lawful twin). Validator được tách
  nguyên văn sang `PublicationFacts<C>::validate` và `validate_moderation_facts`; aggregate đầy đủ
  vẫn gọi cùng hàm, nên không có bộ luật thứ hai. `binding_facts()` là `pub(crate)`.
- Mọi command/worker/appeal vẫn qua `load_paired_state` → `CONTENT_REVISIONS_SQL` →
  `revision_content` (parse, NFC, `RevisionContent::new`, so hash) trước `Publication::restore`.
  `revision_binding` chạy thêm cho cả hai đường, không nới kiểm tra nào: title rỗng vẫn bị từ
  chối như `RevisionContent::new`, hash phải đúng dạng canonical.
- Đường đọc owner dùng `REPEATABLE READ, READ ONLY` là lệnh đầu tiên của transaction, nên năm
  SELECT đọc cùng snapshot; test race khóa `revisions` chứng minh không trộn pointer cũ với
  moderation mới. Missing/foreign vẫn `None` → 404.
- HTTP: bỏ fallback `now_utc()`/title từ body; tuple metadata được dựng trong cùng vòng lặp với
  revision nên `Internal` chỉ xảy ra khi invariant hỏng. DTO/field/fixture v1 không đổi.
- Public body (`persistence/discovery.rs`) tính hash v1 của đúng revision được trỏ trước khi trả
  DTO/ETag; test corrupt cho `If-None-Match: *` xác nhận không trả 304. Đây là kiểm tra mới, nghiêm
  hơn trước: fixture `endorsement.rs`/`taxonomy.rs` từng dùng hash ngẫu nhiên cho revision được
  publish nên đã được sửa sang hash canonical; fixture Web e2e đã tính hash v1 trong SQL. Fixture
  `repeat('a',64)` còn lại (`moderation_queue.rs`, `author_works.rs`) và legacy provisioning
  (`repeat('ab',32)`) không đi qua public body của App, nên không bị ảnh hưởng.

## Finding

| Mức | Vị trí | Nội dung | Xử lý |
| --- | --- | --- | --- |
| P3 | `docs/development/publication-state-read.md` (khối lệnh đo) | Lệnh tái hiện ghi cứng role cục bộ của máy đo (`manhblue`) dù văn bản bảo “thay role”; người khác chép nguyên sẽ lỗi role không tồn tại. | Đổi sang `$USER`. |
| P3 | `docs/development/feature-map.md` | Thừa một dòng trống trước bảng PUB-RUNTIME-01. | Bỏ dòng trống. |
| Có từ trước | `migrations/20261116000000_work_publication_persistence.sql:129` | `work_revision_moderation_pending_check` bắt `decision_version = 1`, trong khi domain re-review sau đổi policy tạo `pending` version 3 ⇒ approve → private → policy mới → request-public lại bản cũ trả `Internal` (rollback, không tạo approval sai). Không do PR này gây ra: constraint nằm trong migration đã áp dụng, có trên develop. | Ngoài phạm vi #230, cần issue/migration riêng (đã ghi ở SSOT). |

Ghi chú không phải defect: `WorkModeration::restore` giờ dựng một bản sao `PublicationFacts<ContentHash>`
cho mỗi command (O(N) hash nhỏ), không đáng kể so với việc tải đủ body.

## Check chạy lại trong review

Máy Linux x86_64 (22 CPU), Rust theo `rust-toolchain`, PostgreSQL 17.11 trong container scratch
dùng một lần (`max_connections=300`), không dùng database dev/production. Cả hai lane chạy trên
HEAD `f55490f` (trước commit review chỉ sửa tài liệu này).

- **PASS** `cargo xtask verify quick`: clippy native/wasm, type-boundary doctests (gồm
  `compile_fail` + lawful twin của `PublicationState`), unit/contract, repository checks.
- **PASS** `VINK_TEST_DATABASE_URL=… cargo xtask verify postgres`: Store 6/6, App 200 passed /
  5 ignored (phép đo opt-in), Identity 48/48; provisioning, worker, Identity/auth, backup/restore,
  dọn scratch. Sáu test `publication_state::` đều `ok`; `publication_state_budget` ignored như
  thiết kế.
- Commit review chỉ sửa Markdown; không chạy lại lane sau đó vì `services/`, `apps/`, `xtask/`
  không đổi.

Không chạy: browser/Web e2e (DTO/UI không đổi), Quint/Kani riêng (transition/authority không
đổi), benchmark opt-in (giữ số đo của tác giả, không tái đo), GitHub CI (theo yêu cầu).
