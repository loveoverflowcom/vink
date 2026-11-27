# #593 — ma trận trước sửa / LayoutSpec R8.3

Base: `c189e4d97aabb383984b8440b2213d19be06ac2e`, develop, tracked sạch, checkout chính duy nhất. Canvas Drive `1bf7Qx9a5utuGJ5KLK1ydYfsi5wGZCgnK` là concept đã chọn; 34 PNG nguồn không có SHA riêng xác minh được. Đã mở render before/after đủ 11 nhóm; không coi chúng là golden runtime. Baseline mới: `1795767719002207000-76513`, Chrome headless + App/Identity/PostgreSQL disposable; sáu phase desk/editor-chrome/editor/editor-format/review/publication đều PASS.

| Nhóm canvas | Phân loại trước sửa | Bằng chứng / owner / quyết định |
|---|---|---|
| 01 Bàn viết list/header/search/hierarchy | FIX F01 (P3 presentation) | Runtime `desk-list-390-100-light.png` vẫn card nền + action kiểm duyệt tonal lặp, create chen trong heading, count/end lặp hierarchy. Owner `desk.rs`, `desk_review.rs`, CSS app. Giữ nguyên GET search/IME/focus, format native, private label, DTO/metric order và explicit review action. Làm phẳng row tại callsite Product; đưa create về hàng controls cạnh sort; count/end ở danh sách ngắn không lọc vẫn có cho AX nhưng không cạnh tranh title; count khi tìm kiếm/phân trang vẫn hiển thị. |
| 02 Bàn viết empty | ALREADY_CORRECT; DELEGATE đề xuất bỏ format | Shared FirstDraft artwork + một CTA, placement/reflow/focus đã chạy desk. Format mặc định sẵn; không bắt lựa chọn mới. Bỏ lựa chọn Markdown lúc tạo sẽ đổi hợp đồng DocumentSource; giữ current, owner #428/#475 nếu muốn đổi. Privacy copy giữ theo #259. |
| 03 Bàn viết error | FIX F02 (P3 duplicate state) | Source `DraftListView`: initial load error đồng thời có `#draft-scope` và `role=alert` EmptyState. Chỉ để alert là owner ở lỗi đầu tiên; stale results khi refresh lỗi vẫn phải hiện. Một Retry, giữ query/sort/focus. |
| 04 Bàn viết loading | ALREADY_CORRECT | UIS-03 DeskLoading/skeleton đã có, held-GET runtime. Không thay bằng spinner concept hay bỏ tools đang giữ focus. |
| 05 Editor canvas/title/toolbar | ALREADY_CORRECT; DELEGATE title multiline | Runtime paper clean/dirty desktop/mobile và native-title suite qua. R4/#482/#515 giữ một EditorView, native one-line title + full-title disclosure, một status owner, writing sheet. Concept multiline/contenteditable không thay canonical title, undo/IME/selection. Owner #475/#476 nếu thay contract. |
| 06 Editor options | ALREADY_CORRECT | Source/render sidebar rộng, details hẹp; focus recovery, tags 0/3, native Select, format conversion riêng. Không đổi thành modal hoặc audience Select vì current switch có semantics publication command. |
| 07 Publication/pending | ALREADY_CORRECT | Giữ intent vs moderation vs current public pointer; #158/#489/#514. Review phase đã kiểm old public pointer/current intent và appeal. Không áp concept “chưa công khai” cho tác phẩm đang có public pointer cũ. |
| 08 Leave dialog | FIX F03 (P3 copy); ALREADY_CORRECT geometry/state | 320×800/200% và 390×420/200% runtime đã chứng minh actions cuộn/Tab vào view, Save disabled và late ACK không rời discard confirmation. Below-fold không là inaccessible. Chỉ rút copy normal unsaved và title; failure/conflict/uncertain/newer ACK giữ nguyên. Shared Dialog/anatomy không sửa. |
| 09 Markdown/outline | ALREADY_CORRECT | Source + editor-format AST/heading/state/reload/version-fence đã chạy; ảnh `markdown-outline-320-200-dark.png` mở. Panel/sheet current đúng; ảnh canvas empty không chứng minh thiếu AST. Giữ Markdown-only toolbar, syntax, poetry, composition/selection/undo. |
| 10 Staff guard | STALE | #555/#568 đã retire Product staff routes; current general404, source + publication guest direct route checks. Không dựng lại moderation shell. |
| 11 No-submission review result | STALE (reload defect); ALREADY_CORRECT current | #489/#568 đã bỏ reload thành công/empty, giữ retry lỗi read; shared Dialog close/focus. Source `desk_review.rs`, review runtime PASS. Không suy luận public/approved từ empty. |

## LayoutSpec — selected canvas direction, residual patch only

```text
Screen #desk — tác giả chọn/mở/tạo tác phẩm
  Region #heading — Bàn viết/private context, search toggle, bell; search dock giữ một node
  Region #draft-controls — create (format + CTA) và native sort; wrap theo width/text scale
  Region #draft-scope — AX live count; visible for search/pagination/dynamic/stale/no-results; initial error dùng alert riêng
  Region #draft-results — flat AuthorWorkRow; title → status/time → metrics → text review action
  Region #draft-continuation — pagination/error controls; end announcement visually hidden
  Region #privacy — current author-only/public-pointer explanation giữ nguyên
Overlay #leave-dialog — current shared Dialog; title “Lưu trước khi rời đi?”
  Region #body — normal: “Bạn có thay đổi chưa lưu.”; tags-only/text+tags được phân biệt
  Region #actions — current Stay / Save / discard-confirm contract, không đổi
```

Sizing/themes: existing --vk-* tokens, Tailwind callsite utility for flat row in Product only; không sửa shared pattern/token/Preview. Action text variant retains hit target/focus. Flat resting row uses current bg-inset token when its title link is hovered; existing focus ring stays. Remove obsolete 32rem header branch for the former create controls; retain large-text reflow rules. Controls wrap at 320–1536 CSS px, light/dark,100/150%; 200% stress riêng. No fixed height or clipping. Existing private glyph/context retained despite decorative omission in concept.

Interaction/invariants: Search dock preserves node and explicit submit; focus survives empty/refresh; create-format remains native/preselected. Same AuthorWorkRow DTO and review callback; no new menu or endpoint. Single initial error alert; stale qualifier remains. Leave save/dirty/conflict/recovery/late-ACK unchanged; action below fold must scroll and Tab into view in dialog without moving background. No CSS hiding blocker or data.

Verification: baseline six selected Product scenarios; after patch desk + editor-chrome + review/publication only if affected assertions warrant; scoped static/unit/CSR checks from committed planner. Review rendered exact final code and computed flat background. No new tests mirroring CSS literals. Existing leave copy mapping assertion updated with actual copy. Preview source audited only because no shared inputs change; do not claim fresh Preview runtime.

Limits/owners: browser/OS Vietnamese IME, real mobile keyboard/touch and screen-reader speech UNKNOWN (synthetic composition + AX != device evidence). Broad semantic/footprint adoption #560, exact 1px centerline #522, work-view effective override #514, OCI #556 remain DELEGATE; no Epic #554 rerun or exploit. Historical evidence remains labelled original SHA.
