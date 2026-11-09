# Phân loại tác phẩm — bộ chọn thẻ và Thông tin tác phẩm TAG-PR-05

**Ngày:** 09/11/2026. **Nguồn:** [#99](https://github.com/loveoverflowcom/vink/issues/99) (TAG-PR-05, một
phần của epic [#94](https://github.com/loveoverflowcom/vink/issues/94)), baseline `develop@f23df33` (sau
#95–#97; không phụ thuộc code discovery #98/PR #119). **Trạng thái:** consumer Web đầu tiên của thẻ nháp,
composition dùng chung Leptos + CMP, kiểm bằng unit, preview hai renderer và trình duyệt trên PostgreSQL
thật qua LocalFixture loopback. Production vẫn `401` cho route riêng tư (D-05 Provisional), nên tác giả
thật chưa dùng được. Bố cục và lựa chọn thiết kế: [LayoutSpec](../design/tag-selector.md); hợp đồng:
[HTTP v1 Work tags](../../services/vink-app/contracts/http/v1/README.md#work-tags-tag-pr-03); lựa chọn API:
[taxonomy-api](taxonomy-api.md).

| Owner | File |
| --- | --- |
| DTO và phân loại lỗi của consumer | [`contract.rs`](../../apps/vink-web/src/contract.rs) (`TagDto`, `TagList`, `TagSuggestions`, `DraftTags`, `SaveDraftTagsRequest`, `SavedDraftTags`, `StoredTags`, `ApiError::{TagConflict, TagRefused}`) |
| Đường dẫn và gọi HTTP (chỉ trình duyệt) | [`api.rs`](../../apps/vink-web/src/api.rs) (`draft_tags_path`, `tag_suggestions_path`, `load_tag_catalog`, `suggest_tags`, `load_draft_tags`, `save_draft_tags`) |
| Trạng thái thuần: lựa chọn, ticket, query | [`tag_state.rs`](../../apps/vink-web/src/tag_state.rs) (`Selection`, `TagSession`, `QueryState`) |
| Chữ + thẻ, guard rời trang | [`desk_state.rs`](../../apps/vink-web/src/desk_state.rs) (`DraftSession`, `DraftTicket`, `Guarded`, `LeaveGuard<T>`) |
| Adapter DTO → facts | [`ui/adapters.rs`](../../apps/vink-web/src/ui/adapters.rs) (`TagLabels`, `tag_selection`, `tag_catalog`, `tag_options`, `exact_option`, `tag_refusal_text`) |
| App controller trong editor | [`pages/work_info.rs`](../../apps/vink-web/src/pages/work_info.rs) (`WorkInfo`), gửi/settle trong `Editor` của [`pages/desk.rs`](../../apps/vink-web/src/pages/desk.rs) |
| Composition dùng chung | [`vink-patterns/leptos/src/tags.rs`](../../packages/vink-patterns/leptos/src/tags.rs), [`Tags.kt`](../../packages/vink-patterns/cmp/src/commonMain/kotlin/vink/patterns/Tags.kt); CSS ở [`patterns.css`](../../packages/vink-patterns/leptos/patterns.css) |
| Gallery | Leptos [`preview/src/tags.rs`](../../packages/ui-system/leptos/preview/src/tags.rs), CMP [`TagSpecimens.kt`](../../packages/ui-system/cmp/diagnostics/src/commonMain/kotlin/vink/ui/preview/TagSpecimens.kt); entry `composition.tag-selector` |
| Kiểm trình duyệt | `draftTags`, `retiredTagAfterRestart` trong [`browser-smoke.mjs`](../../apps/vink-web/e2e/browser-smoke.mjs); seed/retire catalog qua `vink-app tag` trong [`xtask/src/web.rs`](../../xtask/src/web.rs) |

## 1. Lựa chọn

Hai câu OPEN của issue được chọn bằng bằng chứng cục bộ; chi tiết và phương án bị loại ở
[LayoutSpec](../design/tag-selector.md). Đây là lựa chọn của lát, chưa có usability test.

| Lựa chọn | Chọn | Xem lại khi |
| --- | --- | --- |
| CHOICE-01 bố cục | **A**: mục “Thông tin tác phẩm” inline sau trang viết, không lớp phủ | Danh mục cần nhóm/mô tả dài/nhiều facet: panel/sheet bằng `OverlayLayer` hiện có |
| CHOICE-02 ranh giới | **A**: composition `TagSelector`/`TagChip` trong `vink-patterns`, ghép primitive có sẵn; không primitive/token mới | Combobox `aria-activedescendant` hoặc chọn nhiều nơi khác cần cùng ngữ nghĩa bàn phím: admit primitive vào `ui-system` với vectors |
| Lưu | Một nút **Lưu** (và Ctrl/Cmd+S) gửi phần đã đổi: thẻ nếu thẻ đổi, chữ nếu chữ đổi; không đổi gì thì lưu chữ như trước | Có autosave: autosave chỉ chữ, thẻ vẫn lưu theo ý định rõ |
| Đối soát mất ACK | Gửi lựa chọn hiện tại với `expected_version` cũ. Nhận version từ `409` nếu thẻ đang lưu bằng lựa chọn vừa gửi, hoặc bằng một lựa chọn chưa rõ kết quả và version chỉ tăng đúng một. Các trường hợp còn lại cần tác giả xử lý xung đột | Owner cần chứng từ lệnh chính xác (receipt) |
| Gợi ý | Chỉ hỏi khi giá trị đã commit (không trong composition) và không trống; mỗi request có generation, câu trả lời phải cùng generation **và** `query` echo đúng chuỗi đã gửi | Đo thấy cần debounce/cancel fetch |
| Nhãn của ID trong `409` | Từ dữ liệu tab đã đọc (thẻ nháp, danh mục, gợi ý); ID chưa đọc hiện “Thẻ chưa tải được tên”, không đoán | Thẻ tạo sau khi mở trang thường gặp: đọc lại `GET …/tags` khi xung đột |
| Giới hạn ở client | Chặn thêm chủ đề thứ 4 và thẻ trùng như projection; server vẫn là authority (`too_many_themes` vẫn được xử lý) | — |
| Thứ tự chủ đề | Thứ tự là một phần lựa chọn (server giữ thứ tự xuất hiện); bỏ rồi thêm lại là thay đổi | Product quyết định thứ tự không có nghĩa |

## 2. HOW — từ chip tới database

```text
TagSelector (vink-patterns)          props/callback, không fetch
  → WorkInfo (app controller)        on_form/on_add_theme/... → TagSession trong DraftSession
  → Editor "Lưu" / Ctrl+S            DraftSession::begin_save → DraftTicket{text?, tags?}
       tags: TagSession::begin_save → TagTicket{expected_version, request} + lựa chọn đã gửi
  → api::save_draft_tags             PUT /api/v1/drafts/{id}/tags {expected_version, form, themes}
  → App (TAG-PR-03)                  actor từ server → PgStore một transaction → decide_tag_change
  → 200 {version, form, themes}      TagSession::save_succeeded(ticket, …) chỉ khi ticket còn đang bay
     409 version_conflict + tags     lựa chọn đã khớp hoặc có thể vừa ghi → nhận version
                                     còn lại → Conflict (tác giả chọn)
     409 tag_retired / 422 …         Refused gắn vào chip; lựa chọn giữ nguyên
     mạng/503/khác                   Failed (chưa chắc chắn) + nhớ lựa chọn đã gửi để đối soát
  → LeaveGuard::save_settled         rời trang chỉ khi mọi phần intent chờ đã settle và không còn gì chưa xác nhận
```

Đọc: `WorkInfo` gọi `GET /api/v1/drafts/{id}/tags` và `GET /api/v1/tags` khi editor mở trong trình duyệt
(`LocalResource` của draft giữ SSR không có dữ liệu riêng tư; mục thẻ không render ở server). Gợi ý: `GET
/api/v1/tags/suggestions?q=…&facet=theme`. Ba route có deadline 30 s như discussion/endorsement, nên một
lần lưu thẻ treo thành “chưa xác nhận”, không treo mãi.

## 3. WHY

- **Server quyết, UI chiếu.** Danh mục, facet, giới hạn và quyền là của #95/#97; chip chỉ hiển thị và phát
  ý định. Không có nút tạo/đề xuất thẻ vì không có route; “không có gợi ý” nói thẳng điều đó.
- **“Đã lưu” chỉ khi đúng lựa chọn đã gửi được xác nhận.** Ticket giữ lựa chọn gửi đi; ACK cũ không thể
  đánh dấu lựa chọn mới hơn là đã lưu, như `EditorSession` với chữ.
- **Hai tài nguyên, một ý định lưu.** API giữ version riêng cho chữ và thẻ (TAG-PR-03 CHOICE-01 B). Một
  nút Lưu tránh tình huống tác giả bấm Lưu, thấy “Đã lưu” của chữ rồi rời trang mất thẻ. Trạng thái
  toolbar gộp nhưng gọi tên phần thẻ (“Thẻ chưa lưu được.”, “Chưa xác nhận lưu thẻ.”); mục Thông tin tác
  phẩm có trạng thái riêng của thẻ.
- **Đối soát bằng trạng thái đang lưu.** Lưu lại sau lỗi dùng cùng `expected_version`. Khi version
  chỉ tăng một và thẻ đang lưu bằng lựa chọn chưa rõ kết quả, lần lưu đó có thể đã thành công.
  Trả về lựa chọn cũ trong lúc đang lưu hoặc mất ACK vẫn giữ guard cho tới khi đối soát hoặc tác giả
  xác nhận rời trang.
- **Thẻ không động vào CodeMirror.** Mục thẻ nằm ngoài cây `BodyEditor`; đổi thẻ, lỗi thẻ hay xung đột thẻ
  không remount editor, không đổi caret/undo (browser smoke kiểm cùng `EditorView`).

## 4. TEACH — ba luồng

1. **Chọn thẻ rồi lưu.** Tác giả bấm chip “Tản văn” (component phát `on_form`), gõ `hồi`: field giữ
   chữ qua composition, controller hỏi gợi ý cho `hồi` khi composition xong; “Ký ức · qua tên “Hồi ức”” hiện
   ra; bấm thì controller thêm vào `TagSession` và xóa ô tìm. Trạng thái: “Chưa lưu thẻ”. Bấm Lưu: chỉ thẻ
   được gửi với version 1; server ghi version 2 (domain kiểm facet/≤3/retired dưới khóa); ACK trùng ticket →
   “Đã lưu thẻ”. Bản công khai không đổi.
2. **Đang lưu lại chọn thẻ khác.** Request version 1 đang bay; tác giả thêm “Gia đình”. ACK version 2 về
   cho lựa chọn cũ: `saved` = lựa chọn cũ, lựa chọn trên trang khác → vẫn “Chưa lưu thẻ”, lần lưu sau gửi
   version 2. Nếu ACK mất: “Chưa xác nhận lưu thẻ”, “Thử lại” gửi lựa chọn hiện tại với version 1; server
   trả `409` version 2 kèm lựa chọn cũ đã gửi → nhận version 2, lựa chọn mới vẫn chưa lưu. Nếu server
   đã lên version 3 và chỉ khớp lựa chọn cũ, editor hiện xung đột để tác giả chọn.
3. **Chọn thẻ nhưng chưa lưu rồi bấm rời editor.** `NavigationGuard` hỏi `LeaveGuard`; `DraftSession` có
   thẻ chưa xác nhận nên giữ điều hướng: “Thẻ đã chọn chưa được máy chủ xác nhận…”. “Lưu và rời trang”
   bắt đầu đúng phần thiếu (chỉ thẻ, hoặc chữ + thẻ, không gửi trùng phần đang bay) và chờ mọi ACK; một phần
   lỗi thì ở lại với thông báo của phần đó và mở “Rời mà không lưu…” có xác nhận.

Phần nào thuộc ai: component (hiển thị, bàn phím, focus), controller (`WorkInfo` + `Editor`: request
generation, version, gửi/settle), API (`/drafts/{id}/tags`, `/tags…`), domain (#95: luật thẻ).

## 5. AC → bằng chứng

“Unit” là `quick`; “preview” là `ui-preview leptos|cmp verify`; “browser” là `verify web` trên PostgreSQL
thật với LocalFixture loopback có nhãn. Kết quả lần chạy ở mục 7.

| AC | Oracle | Loại | Gap |
| --- | --- | --- | --- |
| AC-01 LayoutSpec trước code | [LayoutSpec](../design/tag-selector.md) (ba hướng, cùng trạng thái khó, ánh xạ Quiet Lavender) được trình bày trước khi sửa UI | Tài liệu | VinkDesign NOT_INSPECTED |
| AC-02 lưu thật, dirty, ACK muộn | `tag_state::tests` và `desk_state::tests` kiểm cả trường hợp trả về lựa chọn cũ khi đang lưu và xung đột sau mất ACK nếu có version mới hơn. Browser `draftTags` kiểm chọn/lưu/reload, version chữ không đổi, publication `404`, conflict + lưu đè, guard giữ khi trả về lựa chọn cũ và ACK mất, đối soát không ghi lần hai, chữ + thẻ rời sau hai ACK; `retiredTagAfterRestart` | Unit + browser | Production `401` |
| AC-03 bàn phím/IME/focus/semantics | Preview smoke: tên nút bỏ, legend/đếm, `role=status`, ↓/↑/Escape, Enter khi composition không chọn, focus sau thêm/bỏ; browser: Enter khi composition không chọn và không PUT, Escape từ hộp thoại rời trang về ô tìm | Preview + browser | Composition mô phỏng (CDP), không IME hệ điều hành; không screen reader; không sheet (CHOICE-01 A) |
| AC-04 theme/viewport/chữ 200 %/lỗi | Preview smoke chung: không cuộn ngang ở 320/390/834/1024/1440 px, chữ 100/200 %, cho mọi kịch bản của catalog (gồm 9 kịch bản `composition.tag-selector`); ảnh `default` sáng/tối ở năm bề rộng (Leptos) và hẹp/rộng (CMP); màu chỉ qua cặp role đã có contrast trong `tokens.json` (`secondary-container`, `warning-container`, `error-container` và chữ “on-” tương ứng); browser: trạng thái conflict/failed/refused có chữ | Preview + browser | Contrast đo tự động của preview không gồm entry này; ảnh chỉ được agent mở (390 px sáng, CMP rộng), chưa review thiết kế |
| AC-05 HOW/WHY/TEACH, map, contract | Tài liệu này; [feature map](feature-map.md); [patterns](../../packages/vink-patterns/README.md); [compatibility](../../packages/ui-system/shared/compatibility.md) | Tài liệu | — |

## 6. Còn thiếu

- **Authority production** (D-05): mọi route riêng tư `401`; chỉ LocalFixture loopback ghi thật.
- **Explore/reader/trang thẻ** (#100): dùng lại `TagChip` cho bộ lọc đang áp dụng và `TagSelector`/gợi ý
  để chọn bộ lọc; app giữ URL, request generation và `GET /api/v1/works` (#98).
- **Native**: CMP chỉ có composition + specimen Desktop; không app native, không runtime Android/iOS.
- **IME thật, screen reader, thiết bị mobile thật**: chưa chạy.
- **Đọc lại khi xung đột**: nhãn cho ID chưa từng đọc không được tải thêm (xem lựa chọn ở mục 1).

## 7. Bằng chứng phiên

Chạy ngày 09/11/2026 trên Linux x86_64, Rust 1.99.1, Chrome 157 headless, Node 24, JDK 17, PostgreSQL
`postgres:17.11-alpine` trong container tạm riêng của phiên (không chạm database dev). Đây là kết quả của
lần chạy đó, không phải trạng thái xanh vĩnh viễn. Chrome trong sandbox của công cụ cần wrapper
`--no-sandbox` qua `VINK_CHROME` (môi trường, không phải thay đổi sản phẩm).

| Lệnh | Mã nguồn | Kết quả |
| --- | --- | --- |
| `cargo xtask verify quick` | `660507f` (đã rebase lên `develop@8ee18b1`) | PASS — 9 bước: token drift, expressions, fmt, ba clippy (ssr, hydrate wasm32, preview csr), doctest, unit/contract, repository checks |
| `cargo xtask verify web` | `660507f` | PASS — 8 bước; phase `main` gồm `draftTags`, phase `after-restart` gồm `retiredTagAfterRestart` (catalog seed và retire qua `vink-app tag`), phase `endorsement` |
| `cargo xtask verify web` | `3af6b24` (trước rebase, base `f23df33`) | PASS — 8 bước |
| `cargo xtask ui-preview leptos verify` | `3af6b24`; `packages/` giống hệt `660507f` | PASS — 612 kiểm tra, gồm 15 của `TagSelector`; không cuộn ngang ở 320/390/834/1024/1440 px, chữ 100/200 %, mọi kịch bản catalog |
| `cargo xtask ui-preview cmp verify` | `3af6b24`; `packages/` giống hệt `660507f` | PASS — token drift, test common + desktop (gồm `TagsTest`), ảnh off-screen `composition.tag-selector` |

NOT_RUN: `verify postgres`, `kani`, `discussion`, `expressions` (lát này không đổi `services/vink-app/src`,
migration, model hay policy biểu cảm); IME hệ điều hành thật, screen reader, thiết bị mobile thật,
Android/iOS. GitHub Actions không được đọc hay chờ.
