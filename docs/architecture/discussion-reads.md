# Đọc bình luận: public projection, overlay riêng tư và fence tại commit — DISC-02/03 phần 2 (#120)

**Ngày:** 10/11/2026. **Nguồn:** epic [#120](https://github.com/loveoverflowcom/vink/issues/120),
tiếp nối PR #126 (admission có version) và #127 (đường gửi + fence đình chỉ).
**Baseline thực thi:** `develop@8a58089` (sau PR #143: fence standing cho mọi lệnh ghi được bảo vệ).
Phạm vi: phần **đọc** của DISC-02/03, giới hạn cursor/HTTP, bằng chứng fence còn thiếu và số đo
PostgreSQL. Không bật `trusted-cohort.v1`, không đổi `premoderation.v1`, không làm outbox (DISC-04),
cache/counter (DISC-05), reaction, ranking hay UI. Production auth vẫn chưa có (D-05 Provisional).

## 1. GapMap tại HEAD thực thi

| Mục | Trạng thái tại `8a58089` | Bằng chứng source |
| --- | --- | --- |
| Own pending/rejected trong root/reply page | **implemented** (giữ) | `VISIBLE_TO_VIEWER`, `visibility::view` |
| Preview batch `unnest + LATERAL`, `PREVIEW_REPLIES + 1` | **implemented** (giữ) | `PgStore::root_page` |
| Snapshot đọc một trang | **implemented** (giữ) | `read_snapshot`: `REPEATABLE READ, READ ONLY` |
| Số query cho một root page | 4 câu dữ liệu (work, thread, roots, previews) + BEGIN/SET/COMMIT | `root_page` |
| Own pending ngoài trang công khai | **incomplete**: root của chính mình chỉ đến được bằng cách duyệt hết các trang; reply của mình dưới root đã ẩn mà không còn reply công khai **không đọc lại được ở đâu** (root không được liệt kê, reply page trả `404`) | `root_listing(view, 0) == None`; test `own_comments_are_recoverable_…` tái hiện |
| Guest có nhận private field? | Không nhận dữ liệu riêng, nhưng representation composed luôn mang trường viewer (`visibility`, `can_remove`, `viewer_is_work_author`, `rejection_reason`) | `DiscussionPage`, `CommentView1` |
| Đọc công khai phụ thuộc Identity | **có**: `Actor` trả `503` khi Identity không trả lời, kể cả với request chỉ cần dữ liệu công khai | `http/mod.rs` `Actor` |
| Quyền tác giả/kiểm duyệt | kiểm trong transaction: `authorize_*`, grant `FOR SHARE`, recusal | `persistence/discussion.rs` |
| Đường đọc vượt bound | Không: `PageLimit` 1–50, preview 3; nhưng cursor chấp nhận nhiều cách viết cho một vị trí (`+1`, `01`, `-0`, hex hoa) và không có giới hạn độ dài tổng trước khi parse | `PageCursor::decode` |
| Fence đình chỉ cho remove own / mode / moderate | **implemented** bởi PR #143 (`require_active` sau khóa hàng, trước ghi); thiếu bằng chứng race chờ cho remove và mode | `remove_own_comment`, `set_comments_mode`, `moderate_comment` |
| Overlay riêng có query/cursor riêng | **missing** | — |

Không có phần nào bị trùng lặp; không có phần nào bị chặn bởi Auth: authority cục bộ có nhãn
(`HeaderIdentity` trong test, LocalFixture) đủ cho mọi kiểm tra backend ở đây.

## 2. DecisionMap

### CHOICE-02 — Hợp đồng đọc công khai/riêng tư → chọn **B+**

| Tiêu chí | A: public endpoint + overlay, bỏ composed | **B+: giữ composed v1, thêm public projection độc lập và overlay riêng** | C: chỉ thêm overlay |
| --- | --- | --- | --- |
| Privacy | Tốt | Tốt: public projection là kiểu riêng chỉ dựng được qua kiểm tra; overlay chỉ đọc hàng của viewer, kiểm lại bằng domain | Guest vẫn nhận representation có trường viewer |
| SSR/no-JS | Phải đổi SSR/hydration Web ngay | SSR anonymous giữ nguyên (`discussion(None)`), không đổi consumer | Như B+ |
| Độ phức tạp query | Preview công khai thuần nhưng phải merge own vào vị trí thời gian ở client | Không thêm query cho trang công khai (cùng SQL với `viewer = NULL`); overlay một query có index | Như B+ |
| Tương thích API | Phá v1 (`DiscussionPage` có `own_pending` trong preview) | **Additive**: 3 route mới, route cũ không đổi byte | Additive |
| Cache safety | Public cache được về sau | Public projection có thể nhận validator ở DISC-05; hôm nay `no-store` | Không có đường public sạch để cache |
| Chi phí migration | Cao (Web, fixtures, endorsement batch) | Thấp: một index partial, fixtures mới | Thấp |

Lý do: B+ tách dữ liệu công khai khỏi trạng thái người xem ở cả kiểu Rust lẫn wire mà không phá
consumer hiện có. Composed v1 vẫn là đường Web dùng (hydration của người đã đăng nhập); việc Web
chuyển sang `public + overlay` là quyết định UI thuộc #129, không làm trong PR backend này.

### Quyết định phụ

- **Overlay = hai dạng, không trộn.** `?cursor=&limit=` liệt kê own **present + unpublished**
  (pending/rejected) theo `created_at, id`; `?ids=` (≤ 20) đối soát trạng thái hiện hành theo ID
  (pending/approved/rejected/removed). Thiếu dạng thứ hai, client không phân biệt được “đã duyệt
  nhưng nằm ở trang chưa tải” với “đã bị gỡ”. Trạng thái đọc từ hàng `comments` tại request,
  **không** từ receipt (receipt chỉ là lịch sử của lệnh gửi).
- **Không lộ root.** Reply trong overlay mang `root_id` (người viết đã trả lời root đó nên biết ID)
  và `root_listed`; không mang text/writer của root. Context dùng đúng luật `reply_context` cũ.
- **Cursor có scope.** `c1.` cho mọi danh sách thread (giữ nguyên format), `o1.` cho overlay; mỗi
  bên từ chối cursor của bên kia. Cursor chỉ là vị trí: mọi trang áp lại bộ lọc visibility và actor,
  nên cursor giả mạo không vượt quyền. Không bind cursor vào thread/actor vì không thêm authority
  nào; moderation queue vẫn dùng `c1.` để không đổi output hiện hành.
- **Cursor chính tắc và có giới hạn.** ≤ 56 byte trước khi parse; chỉ nhận đúng chuỗi server phát
  hành (`encode_in(scope) == raw`), từ chối `+1`, `01`, `-0`, hex hoa, khoảng trắng, chữ số toàn
  độ rộng. Không clamp, không chuẩn hóa.
- **Index partial cho overlay** (`comments_own_unpublished_idx`), chọn theo số đo ở §5. Không thêm
  index public-replies, counter table, cache hay materialization: không có lợi ích đủ lớn.

## 3. Thiết kế

```text
GET …/discussion/public           (không Actor)  → Vink::public_discussion(work, cursor, limit)
GET …/discussion/public/roots/{r}/replies          → Vink::public_replies(…)
     = store.root_page/reply_page(viewer = None) → PublicRootPage/PublicReplyPage::try_from
       (từ chối nếu có item không Public, có rejection_reason, hoặc viewer_is_work_author)
     → PublicDiscussionPage / PublicReplyList (không có trường viewer)

GET …/discussion/me?cursor=&limit= | ?ids=  (Actor bắt buộc, kiểm trước mọi input)
     → Vink::own_comments(viewer, work, OwnCommentsQuery) → OwnRead::parse (o1., ≤20 ids)
     → PgStore::own_comments: snapshot REPEATABLE READ; work công khai? thread?
         WHERE thread_id = $1 AND author_id = $2 AND (removal IS NULL AND review <> 'approved')
         | AND id = ANY($3)
       mỗi hàng → visibility::own_overlay(writer, state, viewer) (sai writer/sai bộ lọc → lỗi đóng)
     → OwnCommentList (no-store)
```

Domain thuần mới: `visibility::own_overlay` (text chỉ khi present; reason chỉ khi present +
rejected; `unpublished` = present ∧ ¬public), `PageCursor::{encode_in, decode_in}` +
`CursorScope` + `MAX_CURSOR_BYTES`. Không đổi lifecycle, admission, policy hay schema hai tầng.

## 4. Authority, khóa và ranh giới nhất quán

**Thứ tự khóa (không đổi, đã rà lại):** publication → thread → comment → principal (fence) →
receipt/insert. Gửi: share trên publication/thread/target rồi `principals FOR UPDATE`. Gỡ của
mình: `comments FOR UPDATE` rồi principal. Bật/tắt: `discussion_threads FOR UPDATE` rồi principal.
Kiểm duyệt: grant `FOR SHARE` → `comments FOR UPDATE` → principal của moderator → audit. Không đường
nào giữ principal khi chờ khóa hàng discussion, nên hai đường không chờ nhau ngược chiều; endorsement
dùng cùng tiền tố. Đình chỉ (`INSERT principal_suspensions`, FK lấy KEY SHARE trên principal) xung
đột với `FOR UPDATE` của fence: đình chỉ commit trước → lệnh ghi thấy và từ chối; lệnh ghi đã giữ
fence → đình chỉ chờ nó commit. Test chạy cả hai thứ tự cho gỡ, bật/tắt và kiểm duyệt, và một vòng
12 lượt ghi đồng thời hỗn hợp (gửi, trả lời, duyệt, gỡ, đóng/mở, đình chỉ) không có deadlock.

**Điểm tuyến tính hóa theo loại thu hồi:**

| Thu hồi | Lưu ở | Điểm tuyến tính hóa | Cửa sổ |
| --- | --- | --- | --- |
| Đình chỉ của App | App DB, `principal_suspensions` | Hàng principal `FOR UPDATE` trong transaction ghi | Không: nguyên tử tại commit |
| Thu hồi grant kiểm duyệt | App DB, `discussion_moderator_grants` | Grant `FOR SHARE` trong transaction quyết định | Không |
| Phiên/tài khoản bị vô hiệu ở Identity | Identity DB (khác database) | Lúc `Actor` resolve request | **Một request**: phiên bị thu hồi sau khi resolve, trước commit, vẫn ghi được một lần. Không có atomic revoke xuyên hai database; muốn chặn tức thời phải đình chỉ ở App |

**Đọc:** đọc không đi qua fence standing (đình chỉ hạn chế ghi). Resolver production biến principal
bị đình chỉ thành `None`, nên người bị đình chỉ nhận `401` ở overlay và view guest ở composed.

## 5. Số đo PostgreSQL (trước khi tối ưu)

Môi trường: macOS/Apple Silicon, PostgreSQL 17.11 (Homebrew), cluster scratch `fsync=off`,
`shared_buffers` mặc định, cache ấm, dữ liệu tổng hợp tái lập bằng `setseed`
([dataset và script](../development/evidence/disc-02-03-reads/README.md)): 113 520 comment; hot thread
2 000 root (80% approved, 10% pending, 5% rejected, 5% gỡ) và ~21 500 reply (5% root có 200 reply,
~20% có 1–5), 300 thread nền; viewer P có 120 lời (108 pending, 12 rejected) rải khắp timeline.
Câu lệnh là đúng SQL production, `PREPARE` có tham số như SQLx. Số buffer ổn định hơn thời gian
(< 1 ms dao động ± 0,3 ms giữa các lần).

| Workload (median 7 lần) | Baseline: ms / buffer | + `comments_own_unpublished_idx` |
| --- | --- | --- |
| Root page guest 20 | 0,248 / 255 | 0,123 / 255 |
| Root page guest 50 | 0,810 / 642 | 0,258 / 642 |
| Root page guest 20, trang sâu | 0,370 / 259 | 0,319 / 259 |
| Preview 20 root đầu (guest) | 0,293 / 189 | 0,279 / 189 |
| Preview 50 root nóng (writer) | 3,788 / 2 231 | 3,025 / 2 231 |
| Reply page root nóng 20 | 0,159 / 157 | 0,110 / 157 |
| **Overlay list P (20)** | **11,518 / 5 797** (Parallel Seq Scan toàn bảng) | **0,196 / 198** |
| **Overlay list A (4 lời)** | **7,639 / 5 447** | **0,284 / 103** |
| Overlay exact 20 ID | 0,327 / 266 | 0,271 / 266 |

pgbench (`-M prepared`, 4 client, 10 s): trang guest (roots + previews) p50 0,44–0,48 ms,
p95 0,73–0,75 ms; overlay P baseline p50 11,8–16,1 ms, p95 16,1–31,9 ms → với index p50 0,22 ms,
p95 0,36 ms, p99 0,52–0,59 ms.

**Ứng viên bị loại:** index đầy đủ `(author_id, thread_id, created_at, id)` cũng nhanh nhưng
8,6 MB so với 928 kB; index partial public-replies `(root_id, created_at, id) WHERE approved ∧
present` chỉ giảm 5–10% buffer của root/preview (thời gian dưới mức nhiễu), preview root nóng
không đổi (2 231) và tăng ghi cho mọi duyệt/gỡ — không đủ bằng chứng. Preview root nóng dùng
bitmap + sort mỗi root; ghi nhận cho DISC-05, không tối ưu ở đây.

**Write amplification:** WAL của 2 000 lần gửi + duyệt (sau `CHECKPOINT`, 3 lần):
16 986/17 376/17 966 KiB → 18 537/18 888/19 223 KiB, median +8,7%. Cập nhật duyệt vốn đã non-HOT
vì `review`/`removal` nằm trong predicate của `comments_pending_idx`. Rollback = drop index
(`migrations-rollback/20261113000000_own_comment_overlay.down.sql`), câu trả lời không đổi.

## 6. Ma trận bảo mật/riêng tư

| Kịch bản | Kết quả | Test (PostgreSQL thật trừ khi ghi khác) |
| --- | --- | --- |
| Guest / A / B / tác giả / moderator đọc public | Cùng một body, không trường viewer | `http_keeps_public_and_private_reads_apart` |
| B đọc overlay, hoặc đoán ID của A bằng `?ids=` | Rỗng; không phân biệt “của người khác” với “không tồn tại” | `own_comments_are_recoverable_…`, `sql_overlay_filters_match_the_domain_projection` |
| Pending root ngoài trang, reply dưới root đã ẩn | Overlay trả về, `root_listed: false` | `own_comments_are_recoverable_…` |
| Own rejected + lý do | Chỉ người viết, chỉ khi rejected | như trên, fixture `discussion.me.get.json` |
| Pending → approved | Rời overlay list, xuất hiện đúng một lần ở public | `a_comment_moves_from_the_overlay_…` |
| Approved → gỡ bởi kiểm duyệt | `removed`, `body: null` dù text còn lưu | như trên |
| Duyệt/gỡ giữa hai trang | Không lặp ID; lời duyệt phía trước cursor thấy khi refresh (keyset, không snapshot) | `approval_and_removal_during_pagination_…` |
| Cursor sai/dài/không chính tắc/khác scope, tham số lặp, `ids` + cursor | `422` với `field`, `no-store` | HTTP test + unit `page.rs`, `discussion_tests.rs` |
| Identity outage | Public `200`; overlay và composed `503`, không giả guest | `an_identity_outage_never_hides_the_public_projection` |
| Database outage | `503 dependency_unavailable`, `no-store`, không body rỗng | unit `outage_router` (`quick`) |
| SSR guest có header actor | Không render pending/rejected của ai | `http_keeps_public_and_private_reads_apart` |
| Store trả nhầm item riêng cho đọc ẩn danh | Kiểu public từ chối dựng → `500`, không serialize | unit application + contract (negative control) |
| Đình chỉ vs gỡ / bật-tắt / kiểm duyệt | Chờ rồi `403 principal_suspended`, không ghi | `own_removal_waits_…`, `a_mode_change_waits_…`, `a_moderation_decision_waits_…` (có từ #143) |
| Grant bị thu hồi khi đang quyết định | Thấy thu hồi hoặc commit trước | `moderation_authority_is_a_revocable_grant_…` (có từ trước) |
| Ghi đồng thời hỗn hợp | Không deadlock, không `Unavailable/Internal` | `concurrent_writes_on_one_thread_never_deadlock` |

Negative control (gỡ guard rồi chạy lại, đều làm test fail): bỏ `author_id = $2` khỏi SQL overlay;
bỏ fence của gỡ own; ép `root_listed = true`; nới bộ lọc overlay nhận approved; trả body cho lời
đã gỡ; tắt kiểm tra kiểu public.

## 7. Giới hạn bằng chứng và việc còn lại

- Kani: thêm `the_private_overlay_reveals_only_the_writers_own_comments` (projection thuần). Parser
  chuỗi cursor **không** có harness Kani: chuyển đổi lịch đã không kết thúc trong CBMC (ghi ở
  harness cursor cũ); phần chuỗi được kiểm bằng unit + HTTP raw-input. Không có fuzz.
- Không model Quint mới: không đổi lifecycle/admission; replay cũ vẫn là regression.
- Late response/actor switch phía client: server không cache và mỗi request tự resolve actor; việc
  bỏ overlay khi đăng xuất và bỏ qua câu trả lời muộn thuộc consumer (#129), **chưa có consumer**.
- Web/CMP chưa dùng public projection hay overlay; browser/native NOT_RUN cho hai route mới.
- Phiên Identity bị thu hồi: cửa sổ một request (§4); cần D-05/S-03 nếu muốn chặt hơn.
- Số đo trên một máy dev, dữ liệu tổng hợp; không phải SLA production.
- DISC-04 (outbox/worker) và DISC-05 (validator/cache/counter, preview root nóng) vẫn mở.
