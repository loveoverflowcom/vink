# Review độc lập PR #246 — Editor Paper First

Review PR #246 cho #218 theo `.agents/skills/vink-code-review/SKILL.md`.
BASE/merge-base: `a78c9ff5138a0df26e9febe4fdf6146f5915d28a`.
HEAD ban đầu: `04d5c2c08fb6aa5d4f4b7658dd02f6d2ab1d7ad4`.
Source sau sửa: `b5463a30f316aa1e6553ed60ab3857772094f3b1`.
Reviewer khác agent triển khai PR ban đầu; bản sửa trong lượt này được self-review.
Không kế thừa PASS của tác giả, không đọc/chờ GitHub CI, không deploy.

## Finding

**P2 / FIXED — Lối tải chữ bị giấu khi chỉ thẻ lỗi/xung đột (AC-03).**
`desk.rs` chuyển `#export` vào native tools, nhưng các nút trực tiếp
`#export-failed` và `#export-conflict` chỉ đi theo trạng thái chữ. Khi chữ đã lưu
và thẻ nhận 409, `WorkInfo` có cảnh báo và recovery thẻ nhưng không có export;
đường tải duy nhất bị giấu trong Thêm. Tương tự cho lỗi tải/lưu hoặc từ chối thẻ.
Trước sửa, regression Chrome/PG thực FAIL đúng assertion
`tag-only conflict keeps a direct text download outside closed tools (AC-03)`.
Server version và selection còn đúng; đây là regression presentation, không có
bằng chứng ghi đè hoặc mất dữ liệu server.

Sửa tại `WorkInfo`: thêm `Tải chữ (.txt)` trong bốn vùng phục hồi, nối cùng
callback live-buffer export của Editor. Không đổi state machine, CAS, partial-save,
reconciliation, projection, overwrite, LeaveGuard hoặc publication. Tệp chỉ chứa
chữ và tiêu đề, không sao lưu thẻ. Regression kiểm Tab/target/focus/tương phản,
tệp tải đúng buffer, không PUT và không tăng version chữ/thẻ.

Không thấy finding thực chất khác trong diff và các luồng đã kiểm. Điều này không
bao phủ các target/boundary được liệt kê NOT_RUN dưới đây.

## Kiểm cuối

Xem `review-246-checks.json` cho lệnh, cwd, SHA, exit code và hash log đầy đủ.

| Kiểm | Kết quả |
| --- | --- |
| `cargo xtask verify quick` | PASS — fmt, clippy SSR/hydrate/preview, doctest, unit/contract, repository checks |
| `cargo test --locked -p vink-patterns --test editor_tools` | PASS 2/2 — SSR opt-in/default anatomy |
| `cargo xtask verify web --scenario main` | PASS — Chrome 157/CDP + PostgreSQL scratch, SSR/privacy, save/reload/conflict, recovery/leave/preview/tags |
| `cargo xtask ui-preview leptos verify` | PASS — 862 captures, no console errors |

Ảnh fresh bên dưới có `evidence.build=b5463a3…`, `dirty=""`. Commit report chỉ thêm
evidence; không thay source đã kiểm. [Checks/log hashes](review-246-checks.json),
[negative evidence](review-246-red-excerpt.txt), [Web excerpt](review-246-web-excerpt.txt).

## Kết quả UI

| Chiều | Kết quả / phạm vi |
| --- | --- |
| Functional | PASS — Web main, export live-buffer không PUT/đổi version; partial save và guard giữ nguyên |
| Geometry | PASS — core 32 tổ hợp clean/dirty × 320/390/834/1440 × light/dark × app text100/200; 8 tổ hợp export thẻ narrow100/200; target mới ≥48px và hit-test được |
| Visual | PASS trong ảnh đã mở: desktop clean, mobile dirty/tools, tag recovery 320/200 sáng/tối. 200% xuống nhiều dòng, cuộn dọc; không clip glyph/action. Không claim pixel fidelity |
| Accessibility/Responsive | PASS trong Chrome: AX native tools/preview, Tab/Shift+Tab/focus, text200; export thẻ contrast 8.37:1 sáng / 4.83:1 tối |
| Interaction | PASS — menu Enter/Space/touch-equivalent, preview/live export, save ACK focus, late/disposed ACK, pending/new edits, safe leave/cancel/two-step discard. OS targets chưa kiểm |

Đã mở ảnh fresh: [desktop clean](review-246-paper-clean-1440-100-light.png),
[dirty/tools 320/200](review-246-paper-dirty-320-200-light-tools.png),
[export thẻ sáng](review-246-tag-recovery-export-320x800-200-light.png),
[export thẻ tối](review-246-tag-recovery-export-320x800-200-dark.png).
[Metadata gốc chọn lọc](review-246-captures.json), [contrast](review-246-contrast.json).
Các ảnh before/after tác giả cung cấp được đọc như evidence lịch sử, không thay run mới.

## Giới hạn và tài nguyên

`NO_APPROVED_REFERENCE`: review theo LayoutSpec/rubric, không tự duyệt golden.
OS IME Telex/VNI, screen reader, Safari/Firefox, browser zoom và thiết bị mobile
thật: NOT_RUN. CDP composition/touch và viewport/text200 không thay các target đó.
Full Web ngoài main, full PostgreSQL lane, CMP/native, worker: NOT_RUN trong lượt
review presentation này. Save/reload/conflict dùng PostgreSQL scratch thật;
HTTP failures/lost ACK được fault-inject qua CDP. Refusal/tải thẻ lỗi được đối chiếu
source; regression mới chạy runtime trực tiếp cho conflict và uncertain save.

Hai lần kiểm ban đầu bị môi trường chặn: Quick không được bind socket local;
Web không tự tìm Chrome. Đã chạy lại với quyền socket và `VINK_CHROME` thật.
Một lần Quick trùng artifact lock đã dừng an toàn; kiểm cuối chạy tuần tự.
FAIL trước sửa của regression là negative evidence, không đổi thành PASS lịch sử.

Worktree `/Users/manhblue/projects/loveoverflow/vink-204`: status và SHA-256 của
cả 30 file sửa/untracked giống trước/sau; không reset/stash/checkout hoặc xóa evidence.
Đã kiểm tiến trình trước build; giữ nguyên Chrome/PostgreSQL của #204. Chỉ dọn
`target/debug/{deps,.fingerprint}` và `target/wasm32-unknown-unknown` không dùng trong
checkout task (#242) và task-2 (#218), sau khi tạo clone cache riêng. Giữ source,
site bundles, captures, log và DB cũ. App/Chrome kiểm hữu hạn tự dừng; PostgreSQL
riêng `/tmp/vink-review-246-pg` đã dừng, data/log giữ lại. Đĩa còn khoảng 5 GiB.

## Phối hợp PR #247

Đã fetch/read #247 `codex/web-visual-evidence-oracles@0fd9ed85ab9f8e224c7e87556f1fe1e559a49091`:
OPEN, chưa merge, cùng base `a78c9ff`. Mô phỏng `git merge-tree --write-tree`
giữa hai head (không checkout/merge nhánh) cho thấy `feature-map.md` tự merge,
`browser-smoke.mjs` có hai conflict ở khai báo provenance và ghi capture record.
Khi tích hợp #247 sau này cần giữ `SOURCE_IDENTITY`/`EVIDENCE_STATE` và record
manifest v2 của #247, đồng thời bảo toàn thông tin dirty, geometry Editor,
`editorPaperFirst` và regression/export thẻ của #246; không chọn nguyên một phía.
Không sửa/push/merge PR #247 trong lượt này. Runtime của tổ hợp hai PR: NOT_RUN.
