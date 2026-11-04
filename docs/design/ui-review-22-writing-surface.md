# UI-REVIEW-22 — Writing Surface: Editor Layout & Typography

**Kết quả lượt runtime: REVIEW-ONLY.** Chưa xác nhận defect UI trên snapshot `4ce0ff1` trong phạm vi baseline UI-REPAIR-05 đã kiểm. PR này lưu báo cáo và tóm tắt evidence; không có patch sản phẩm và không hoàn tất S-01. Đối chiếu yêu cầu mới bên dưới xác nhận title FloatingTonal và các thay đổi R1/R2 ở #66/#67 vẫn **IMPLEMENTATION_PENDING**; kết luận runtime cũ không nghiệm thu các yêu cầu đó.

## Nền và ủy quyền

- Đã đọc toàn bộ 775 dòng file Library `Pasted text(20261104-080027).txt`, ID `libfile_77361edc2d7c8191ac4a3e8c5e6db6aa`.
- `git fetch origin develop`, kiểm WIP sạch rồi `git pull --ff-only origin develop` tại checkout chính; đã chứa [PR #69](https://github.com/loveoverflowcom/vink/pull/69).
- Snapshot/runtime đã review: `4ce0ff18c9602ec2ce0bfe09c59dc5f84eef1193`. Nhánh `codex/review-editor-surface`, worktree riêng trong workspace task này. Không reset về review snapshot `c6aa486`.
- Trước khi tạo nhánh: không có branch local, remote-tracking branch hay PR cùng head `codex/review-editor-surface`; API pull requests trả danh sách rỗng. Không ghi đè feature branch.
- Đã đọc [#47](https://github.com/loveoverflowcom/vink/pull/47), [#56](https://github.com/loveoverflowcom/vink/pull/56), [#62](https://github.com/loveoverflowcom/vink/pull/62). Cả ba đã merge. Diff từ `c6aa486` tới snapshot `4ce0ff1` không đổi editor bridge, CodeMirror runtime hoặc editor composition; CSS app đổi các vùng đọc công khai.
- Khi tạo PR ban đầu, phạm vi là tài liệu review và evidence dạng văn bản, với **MERGE/DEPLOY FORBIDDEN, OWNER_REVIEW REQUIRED**. Chỉ dẫn tiếp theo của người dùng đã cho phép review độc lập, sửa, cập nhật develop, resolve conflicts và tự merge khi validation đạt; quyền mới thay ghi chú cấm merge cũ. Không tự approve, bypass branch protection hoặc deploy; không kiểm/chờ CI GitHub.
- Base tích hợp tài liệu: `325746232ccf5a05b53c1d531f69bb09c77ea6b1`, sau [PR #70](https://github.com/loveoverflowcom/vink/pull/70). Đã fast-forward không conflict. #70 đổi appearance/bootstrap, brand rail CSS và browser smoke; `desk.rs`, Rust editor bridge và CodeMirror runtime không đổi. Không gắn kết quả runtime `4ce0ff1` cho base mới hoặc PR tài liệu này.

## Authority và nguồn đã đọc

Accepted base là [UI-REPAIR-05 r1](https://github.com/loveoverflowcom/vink/blob/4ce0ff18c9602ec2ce0bfe09c59dc5f84eef1193/docs/design/ui-repair-05.md), cùng [UI-REVIEW-18](https://github.com/loveoverflowcom/vink/blob/4ce0ff18c9602ec2ce0bfe09c59dc5f84eef1193/docs/design/ui-review-18.md). Đã đọc AGENTS, development guide/feature map, cả chín skill bắt buộc và các reference về layout/evidence/verification.

Đã mở [archive desktop](https://drive.google.com/file/d/1i_YiJ1pb8ippWdy3kaAWobeV_KrGdYNk/view), [archive mobile](https://drive.google.com/file/d/1bucuiIqUpjMj0S6gKnL77hiVJXa5O8BA/view) và [implementation lịch sử](https://drive.google.com/file/d/1DmMCd9BZUK3aH8-DVaoWNJt7Bii11-VB/view). Archive chỉ là reference lịch sử; không dùng làm HEAD capture, pixel baseline hoặc contract lưu. Copy “Đã lưu trên thiết bị này” không phải contract hiện hành.

## Đối chiếu yêu cầu cập nhật 04/11/2026

Review độc lập PR #71 bắt đầu ở head `03e2644eb4e11f287fe9146d664ac5be412d4fa3`, merge-base và latest `origin/develop` đều là `325746232ccf5a05b53c1d531f69bb09c77ea6b1`. `git merge origin/develop --no-edit` báo already up to date; không có conflict. Diff sản phẩm của PR rỗng.

Đã đọc đầy đủ file Space `Pasted text(20261104-090708).txt` (`libfile_c83bc8f6eec08191b061569b2a70e17a`, 82 dòng), description và toàn bộ comments [#66](https://github.com/loveoverflowcom/vink/issues/66)/[#67](https://github.com/loveoverflowcom/vink/issues/67), [LayoutSpec R2](https://drive.google.com/file/d/1ChFchoWsrc9XFtkW5BvP5XUfr6GAAbKy/view), [audit/handoff hiện hành](https://drive.google.com/file/d/1Ao43o1ymizuBILy0BPHa0RG_Y9uRFDSM/view) và source [Desk v2.6](https://drive.google.com/file/d/15PQVO1_dT9TXTmML_QR5_xvCTSg50xxb/view). Chỉ dẫn hiện tại → R2 cho appearance/account → R1 cho field/action/search/editor → baseline chưa bị thay thế. Prototype/source design không là bằng chứng runtime app.

| Yêu cầu / owner | Source tại develop `3257462` | Trạng thái và phần tiếp theo |
| --- | --- | --- |
| #66 AC-02..05, PR-01 `codex/r2-shared-controls` | Shared `TextField` chưa có typed FloatingTonal; `.vk-ui-input` còn idle border 1px và invalid border + inset shadow; CMP vẫn dùng `OutlinedTextField` | **IMPLEMENTATION_PENDING.** PR-01 sở hữu `packages/ui-system/**`, API/anatomy/focus và qualification riêng Web/CMP |
| #66 AC-06..08 và #67 AC-08, PR-02 `codex/r2-explore-desk-consumers` | `desk.rs` vẫn dùng `TextField id="draft-title" name="title" input_ref=title_input`; Explore giữ native GET `name=q` với clear link bên ngoài; `SearchField` chưa có prop `name` | **IMPLEMENTATION_PENDING; DEPENDENCY: PR-01.** Migrate title qua API thật sau PR-01; giữ DOM input/caret/IME/undo/dirty guard, GET/no-JS và CodeMirror/save. Không thêm CSS giả FloatingTonal hoặc đoán prop chưa tồn tại |
| #67 AC-02..07, PR-03 `codex/r2-appearance-account` | ThemePicker dùng native RadioGroup; app còn khung preference/choice và backdrop transparent. #70 đã bổ sung bootstrap checked-radio và rail wrap | **IMPLEMENTATION_PENDING.** R2 appearance/account là owner PR-03; giữ nguyên fixes #69/#70. Không suy merge #70 là nghiệm thu R2 |

Đây là **gap yêu cầu đã có trước PR #71**, không phải regression do PR tài liệu. R22-04 chỉ giải thích title một dòng so với archive cũ; không phủ nhận yêu cầu FloatingTonal mới. Không sửa shared controls, app consumers hoặc appearance trong PR tài liệu này vì chúng là ba lát cắt có owner/dependency riêng đã được chỉ định; PR-02 cần contract chưa triển khai của PR-01. Không tick acceptance hoặc đóng #66/#67 từ bằng chứng review cũ. Phần body layout/typography/selection của UI-REVIEW-22 vẫn là evidence hồi quy hữu ích cho PR-02.

Kiểm độc lập cho delta tài liệu: đối chiếu 11 SHA-256 source với đúng commit runtime, kiểm 120 tổ hợp geometry đầy đủ/không trùng và các invariant metadata về raw source/view/overflow; kiểm 13 locator ảnh và tổng 145 capture; diff/whitespace/relative links. Đây là kiểm source/metadata, không phải chạy lại browser hoặc mở lại ảnh. Library đã resolve đúng ZIP `UI-REVIEW-22-review-only.zip`, ID `libfile_b53ab409196c8191bd3b8700dd0e525a`, version `1`, metadata 6.946.844 byte; chưa tải lại ZIP để kiểm độc lập checksum/pixel. Không chuyển kết quả quick/web lịch sử thành PASS runtime mới.

## HOW / WHY và owner

| Mối quan tâm | Owner đã trace | Cơ chế hiện có |
| --- | --- | --- |
| Tải/mở một nháp | `apps/vink-web/src/pages/desk.rs:335` — DraftEditor → Editor | LocalResource; SSR chỉ khung, không đưa chữ riêng vào HTML |
| Title | `desk.rs:387`, TextField ở dòng 700; shared `field.rs` | Signal trong Editor; native input có maxlength 200; callback nhận raw value |
| Body snapshot/lifecycle | `apps/vink-web/src/editor.rs:27`, :88 | BodySource giữ chữ loaded và handle; snapshot đọc live document; cleanup destroy view khi owner kết thúc |
| EditorView/extensions | `apps/vink-web/editor/src/editor.ts:34` | Một view; history, default/history keymap, Enter không tự chép indentation, lineWrapping, aria names, scroll margins |
| Focus/selection | `editor.ts:39` | focusSelection/focusEpoch, map qua thay đổi document; native focus restore; mouse intent hủy restore cũ |
| Toolbar | `packages/vink-patterns/leptos/src/lib.rs:134`, `patterns.css:76` | Chỉ nhận status và children; caller giữ save authority |
| Typography/paper/sticky | `apps/vink-web/style/vink.css:310`, :352, :369, :393 | App presentation; token source là `packages/ui-system/shared/tokens.json` |
| Label/help | `desk.rs:707`, :729; `editor.rs:13` | Visible label/hint IDs truyền vào aria-labelledby/aria-describedby; native details |
| Save/preview/export | `desk.rs:403`, :564; `apps/vink-web/src/document.rs` | Manual save chụp snapshot; preview/export không lưu; LeaveGuard/NavigationGuard vẫn giữ owner hiện có |

**Lý do đã được ghi nhận:** development guide và editor README chọn CodeMirror 6 để giữ history/selection/IME tốt hơn textarea; plain source giữ câu thơ/khổ bằng newline; manual save là giới hạn bootstrap để kiểm CAS, chưa hoàn tất autosave hay chốt rich text. Một EditorView per draft tránh remount khi theme/status thay đổi. UI-REPAIR-05 ghi sticky chỉ khi đủ rộng/cao và giải phóng toolbar khi chữ lớn. **Suy luận từ source:** các điều chỉnh thị giác có thể thuộc CSS/composition hiện có, không cần thay architecture hoặc persistence; lượt này không có defect để áp dụng điều chỉnh đó.

## LayoutSpec r0 — observed, kế thừa r1

Đã trình bày cây này trước mọi quyết định patch. Không có r1 patch mới vì không có finding CONFIRMED.

```text
Screen #editor — /ban-viet/{id}
  Region #editor-heading
    BackLink → Bàn viết
    H1 → Bản nháp
  Region #editor-toolbar
    SaveStatus
    Secondary → Xem trước / Tải .txt
    Primary → Lưu nháp
  Region #recovery — failed/conflict hiện có
  Region #writing-surface — .vk-editor-paper
    Region #title — TextField Tiêu đề, native single-line input
    Region #body
      Label Nội dung + Hint Ctrl/⌘ + S
      CodeMirror EditorView — plain manuscript
      Region #writing-help — native details
  Overlay #draft-preview
  Overlay #leave-editor
```

DOM/focus order giữ công cụ phụ → Save → title → body → details. Title/body có cùng outer width; text inset theo owner control riêng. Paper measure tối đa `42rem + 2×space-6` = 720 CSS px ở root 16px. Không có fixed mobile toolbar. Sticky chỉ bật khi container ≥44rem và viewport ≥40rem; 834/390/320, root 200% và viewport thấp trong ma trận đều static. Light/dark giữ cùng cấu trúc và nghĩa trạng thái.

## Ma trận, fixture và tính toàn vẹn chữ

Run `2026-11-04T154819+0700_review-editor-surface_4ce0ff18` có [tóm tắt evidence được track](evidence/ui-review-22/review-summary.json): 120 ca geometry/raw-source, fingerprint source/fixture, 13 locator ảnh đã mở, SHA-256 từng PNG và các giới hạn. Không có relative link tới `target/` cần tồn tại trên máy reviewer.

Gói đầy đủ `UI-REVIEW-22-review-only.zip` được giữ trong Library của owner, 6.946.844 byte, SHA-256 `aa4f1fc4d8de428f80213198455ac5f91ae60f16c6f52ca2a42360c3e5a133fe`. ZIP gồm manifest, raw fixture plan, metrics, runtime hashes, PNG và lịch sử kiểm thử; chưa có URL public. Các artifact trong ZIP không được commit vào repo.

- 6 viewport: 1440×900, 834×900, 390×844, 320×900; stress 1440×360 và 390×600.
- Light/dark × root font 100/200% × 5 nội dung = **120 ca DOM/raw-text**. Empty/ready, poetry/selection, prose/body focus, title 200 ký tự/title focus và dirty Unicode/word stress. Không tuyên bố mọi tổ hợp state.
- 120 ảnh body/title + 24 ảnh poetry top + 1 ảnh caret bổ sung = **145 PNG**, **13 ảnh đã mở**, 132 ảnh chưa mở. Web suite gốc tạo riêng 897 PNG; không cộng vào số ảnh đã inspect.
- Cùng local scratch work `01a1a5b7-6173-73be-8e8c-5ef537958ffa`, database riêng `review22_local` trên cluster 17.11 cổng 55442. Không dùng hay đổi shared DB/production seed.
- Literal thơ/văn xuôi trích trực tiếp từ file Library vào JSON UTF-8. Indentation thơ theo từng hard line là `0,4,0,3,3` spaces; văn xuôi `0,2,0,2,2`. Không trim/normalize khi so live source.
- Mỗi ca chờ hydration, CodeMirror editable, fonts ready, hai animation frame và theme transition hữu hạn. Context ghi route, viewport/theme/font/scroll, save state, focus, selection; metrics ghi bbox, computed font/line-height/spacing/padding và visual line rects.
- 120/120 giữ byte-for-byte live document và cùng EditorView, không page overflow; scroller/content không có overflow thực vượt client size >1px. Thơ vẫn 5 hard lines dù mỗi dòng wrap thành nhiều visual rects. Dirty fixture giữ NFD/NFC, Đ/đ, emoji/supplementary Unicode, dấu ngoặc, indentation, nhiều dòng trống và trailing newline.
- Matrix phát **0 POST/PUT**. Supplement kiểm preview/help/rail/theme/resize/focus: **0 POST/PUT**, draft version 2 → 2; edit con trỏ/long-word diễn ra sau phép kiểm zero-mutation và không lưu. Matrix canonical có một lần Cmd+S có chủ ý ở interaction, rồi composition vẫn unsaved; không suy các thao tác layout là save.
- Contract document v1 hiện hành **có** collapse separator blank lines và server NFC khi lưu; đó là normalization được ghi ở `document.rs:1` và bootstrap guide, ngoài scope patch. PASS live source không được diễn giải thành lossless raw-byte save/reload cho mọi separator.

### Số đo đại diện sau font load

| Viewport / scale | Outer title/body | Title | Body Literata | Toolbar |
| --- | --- | --- | --- | --- |
| 1440×900 / 100% | 720px | 22px / 28.6px | 19.6px / 36.26px; padding 24px | sticky, 64px cao |
| 834×900 / 100% | 659px | 22px / 28.6px | 18.085px / 33.4572px; padding 24px | static |
| 390×844 / 100% | 343px | 22px / 28.6px | 17px / 31.45px; padding 16px 12px | static, 93px cao |
| 320×900 / 200% | 241px | 44px / 57.2px | 34px / 62.9px; padding 32px 24px | static; wrap trong trang |

Nguồn token: `font.reading`, `text.reading`, `leading.reading=1.85`, `type.title`, `space.3/4/6`. Letter spacing computed là `normal`. Title dài giữ đủ 200 ký tự và native horizontal input scrolling; accepted r1 đã ghi hành vi này. Không coi title input là multiline wrapping.

### Findings và quyết định

Không có finding CONFIRMED P1–P3. Dấu “—” ở severity nghĩa không gán mức defect cho khác biệt thiết kế hoặc gap chưa kiểm. AFTER là N/A vì không patch.

| ID / component | Scenario, expected / actual | Classification / confidence | Severity | Source / root cause / action / verification |
| --- | --- | --- | --- | --- |
| R22-01 toolbar/help | Cần thứ bậc Save, wrap và help không che body. HEAD có Text/Text/Filled, details dưới body; full web kiểm target ≥44px, short viewport và sticky clearance | RESOLVED bởi #47; cao | — | `patterns.css:76`, `vink.css:352/369`, `desk.rs:633/729`. Giữ merged fix; PASS web + current captures |
| R22-02 editor selection/history | Select nhiều dòng; rail/theme/desktop→mobile→desktop; cần giữ view/chữ/selection/undo. Actual giữ nguyên; native pointer edit theo caret đã chọn | RESOLVED bởi #56; cao | — | `editor.ts:39/57/94`. Không reimplement focus restore; PASS web + canonical + supplement |
| R22-03 leave focus | Quay lại từ confirm discard cần focus trigger hoặc fallback sau late ACK. Regression hiện có PASS | RESOLVED bởi #62; cao | — | `desk.rs:525`, `editorChrome`; owner dialog ngoài patch scope. Giữ nguyên; PASS web |
| R22-04 title/paper | Archive có title trông như văn bản nhiều dòng; HEAD là input một dòng có scroll giữ 200 ký tự. Outer width khớp body, font serif và separator có r1 | DESIGN_DELTA; cao | — | `vink.css:369/370`, `TextField`. Không đổi control/API vì prototype; PASS source + 200-char fixture + ảnh đã mở |
| R22-05 CodeMirror scroll/caret | Source dùng overflow auto nên nghi nested trap/right clipping. 120 geometry cases không có active nested overflow; native End ở long word 320/200 cho caret x155.17, y845.34–895.34 trong viewport900, scroller x33–272, width239=scrollWidth239 | HYPOTHESIS từ source, chưa tái hiện defect; cao về số đo | — | `vink.css:415/421`, `editor.ts:89`. FAIL ban đầu do harness serialize DOMRect thành {}; sửa phép đọc scalar, giữ assertion, PASS; không patch UI |
| R22-06 typography/IME acceptance | Chrome mô phỏng giữ Unicode và composition, nhưng chưa có OS keyboard/screen reader/multi-browser evidence | NOT_VERIFIED ngoài Chrome; cao về gap | — | OS IME, mobile thật, Safari/Firefox, screen reader NOT_RUN. Không suy từ headless Chrome |
| R22-07 save copy | Archive “Đã lưu trên thiết bị này” khác server-confirmed manual-save contract. HEAD không nhận prototype copy làm authority | DESIGN_DELTA; cao | — | UI-REPAIR-05 + EditorSession/toolbar caller; giữ current save semantics |
| R22-08 SSR log | Khi thu stdout cuối của server scratch, có hai thông báo Leptos LocalResource panic không gắn request/timestamp. Probe process mới trên cùng build: GET dashboard và hai GET editor đều HTTP200, hoàn tất ~1–3ms, không lặp panic | NOT_VERIFIED về trigger/ảnh hưởng; cao về quan sát log | — | `DraftEditor`/Suspense/LocalResource SSR boundary; không có UI hang được tái hiện, không patch. Giữ log và `ssr-probe.json`; cần pin request nếu hiện tượng lặp lại |

Ảnh snapshot `4ce0ff1` dùng cho các quan sát thị giác nằm trong bảng ảnh đã mở bên dưới; reference chung là UI-REPAIR-05 r1. Không có before/after fix pair hoặc root cause UI cần sửa.

## Verification: visual và interaction là hai lớp

| Check trên runtime snapshot `4ce0ff1` | Kết quả | Evidence / giới hạn |
| --- | --- | --- |
| `cargo xtask verify quick` | PASS | tokens, fmt, clippy SSR/WASM/preview, type-boundary doctests, unit/contract, repo checks; app source HEAD nguyên vẹn |
| `cargo xtask verify web` | PASS r2 | Real local PostgreSQL + server + Chrome, editorChrome, save/reload/conflict/retry/lost ack, leave/late ACK, selection/history, preview/export, graceful SIGTERM/restart; không console error |
| Focused canonical matrix | PASS | 120 exact-source/same-view/reflow cases + 144 PNG; chỉ 12 ảnh của ma trận được mở |
| Keyboard/selection/undo/redo | PASS | Tab title/body/toolbar ở suite; Home/End/Arrow/Shift+Arrow, Enter/Shift+Enter, Cmd+Z/Cmd+Shift+Z/Cmd+S; multiline selection qua rail/theme/resize; raw text snapshots |
| CDP composition | PASS simulation | d→đ→commit đúng một chữ; Cmd+S trong composition không PUT; selection valid; undo/redo đúng. Không OS IME |
| Preview/help/native pointer/caret | PASS supplement r4 | Enter mở, Space đóng details; preview Escape; no implicit writes/version change; pointer insertion/undo; caret không clip |
| SSR probe bổ sung | PASS ba HTTP GET; trigger panic NOT_VERIFIED | Dashboard + hai lần mở editor trên process mới hoàn tất HTTP200; không thấy panic mới. Hai thông báo ở process trước vẫn được giữ, không suy ra đã sửa |
| Contrast | PASS ở existing web targets | Chữ editor/toolbar/label/help từ computed colors hai theme, ≥4.5; disabled exempt. Không chứng nhận WCAG hay screen-reader acceptance |
| Shared Leptos preview | NOT_RUN | Không thay patterns/primitive/shared contract |
| CI GitHub | NOT_RUN | Theo yêu cầu không kiểm/chờ CI |
| OS IME/mobile keyboard/real device/Safari/Firefox/screen reader/browser zoom | NOT_RUN | Root CSS text 200% và CDP không chứng minh các ranh giới này |

Môi trường: macOS, Rust 1.99.1, Node 22.15.0, Chrome 156.0.8078.80 headless, PostgreSQL 17.11 Homebrew. Build dev SSR + hydration WASM, debug=0/incremental=0 để hạn chế disk; SHA-256 binary/bundle đã đối chiếu lại không đổi.

**Lịch sử FAIL được giữ:** web r1 dừng ở npm cache EACCES, browser NOT_RUN; r2 dùng npm cache riêng PASS. Scratch capture đầu dùng accessor CodeMirror cũ rồi serialize live view lỗi; các run đó không phải product failure. Supplement r1/r3 FAIL caret vì DOMRect thành `{}`; r2 diagnostic thiếu output dir; r4 đọc scalar và giữ nguyên visibility assertion PASS. Capture canonical r1 FAIL bước nhập empty trên draft đã có chữ; r2 dùng Select All + Backspace thật trước khi insert và PASS nguyên assertion exact-source. Các ma trận khám phá trước canonical dùng indentation tự dựng, không nhận làm canonical acceptance. Không giảm assertion, sửa baseline, source UI hay dữ liệu shared để lấy PASS.

## Final review và bàn giao

PR chỉ thêm tài liệu và tóm tắt evidence, production diff với develop = rỗng. Không thay CSS, editor lifecycle, Unicode/newline policy, state/API/CAS hoặc merged fixes. Quick/web và các capture trong bảng trên được chạy trên snapshot `4ce0ff1`; integration base mới sau #70 chưa được chạy lại runtime bởi lượt tài liệu này. Kiểm tra tài liệu trên base `325746232ccf5a05b53c1d531f69bb09c77ea6b1` gồm diff/whitespace, JSON, link chỉ tới file được track và `cargo xtask verify quick`; kết quả cuối ghi ở PR.

Quan sát ngoài kết luận layout: stdout thu khi server scratch đầu tiên dừng có hai thông báo `Reading from a LocalResource outside Suspense in ssr mode...`. Chưa pin được request hoặc chứng minh ảnh hưởng tới tác vụ; web suite và frontend không ghi exception. Đã bật backtrace trên process scratch mới, GET dashboard rồi editor hai lần đều trả 200 nhanh và log không lặp panic. R22-08 là gap điều tra, không được gọi là defect đã sửa hoặc bị bỏ qua vì browser PASS. Sau probe đã dừng lại server/cluster riêng.

Giới hạn thị giác: 13/145 ảnh được mở, các capture còn lại chỉ có DOM/raw-text checks; không claim mọi pixel/state đã review. Source fidelity khi layout thay đổi được kiểm riêng với persistence normalization hiện có. PR tài liệu không đại diện cho một confirmed UI fix; owner có thể review evidence và yêu cầu kiểm thiết bị/IME thực để tiếp tục các acceptance còn thiếu.

## Ảnh snapshot đã mở

Bảng dưới là locator trong ZIP; PNG không nằm trong checkout GitHub. SHA-256, tên file đầy đủ, viewport/theme/scale và trạng thái của từng ảnh nằm trong [review-summary.json](evidence/ui-review-22/review-summary.json). Ảnh canonical có cùng scratch fixture; ảnh caret là kiểm bổ sung cùng work ID.

| Capture ID | Scenario / locator trong ZIP |
| --- | --- |
| c0009 | `r22-empty-390x844-light-t100` |
| c0025 | `r22-poetry-1440x900-light-t100` |
| c0026 | `r22-poetry-1440x900-light-t100-top` |
| c0055 | `r22-poetry-320x900-dark-t200` |
| c0064 | `r22-poetry-1440x360-dark-t200-top` |
| c0069 | `r22-poetry-390x600-dark-t100` |
| c0079 | `r22-prose-834x900-dark-t100` |
| c0081 | `r22-prose-390x844-light-t100` |
| c0088 | `r22-prose-320x900-dark-t200` |
| c0112 | `r22-long-title-320x900-dark-t200` |
| c0121 | `r22-dirty-1440x900-light-t100` |
| c0134 | `r22-dirty-320x900-light-t200` |
| c0145 | `r22-focused-longword-320x900-dark-t200` |
