---
name: vink-git-workflow
description: Name Vink branches, write commit messages with verified source trailers and open or update pull requests with an honest Vietnamese body. Use before creating a branch, committing, pushing or opening/editing a PR, and when reading Git history for the issue, decision or spec behind a change.
---

# Vink Git workflow

Một bộ quy ước chung cho nhánh → commit → PR: `git log --oneline` gọn, `git show` đủ lý do, PR
đủ ngữ cảnh cho người review, và mọi liên kết nguồn đều có thật. Đọc
[AGENTS.md](../../../AGENTS.md#work-and-verification) trước. Yêu cầu hiện tại của người dùng (tên
nhánh, base, commit thẳng vào `develop`, phạm vi commit) thắng quy ước này. Skill quy định *cách*
làm; việc *có được* commit, push hay mở PR vẫn theo quyền đã giao trong hội thoại, không thêm
bước hỏi lại.

Chi tiết, bảng và ví dụ: [nhánh, commit, trailer và liên kết](references/conventions.md),
[PR](references/pull-request.md). Kiểm máy: `scripts/git-lint.sh` (xem cuối trang).

## Repo thực tế

- `develop` là nhánh tích hợp và default branch. Không có nhánh production riêng: image OCI chỉ
  được publish thủ công từ `develop` (`publish-images.yml`).
- Theo [mốc đối chiếu](../../../docs/references/skill-adaptation.md), repo dùng
  `COMMIT_OR_PR_TITLE` + `COMMIT_MESSAGES`: PR nhiều commit lấy **tiêu đề PR**, PR một
  commit lấy **subject của commit** làm subject squash trước hậu tố ` (#PR)`. Người merge
  có thể sửa subject và body mặc định. Vì vậy kiểm cả subject commit lẫn tiêu đề PR.
- Chưa có `.github/pull_request_template.md` hay `CODEOWNERS`; label hiện có gồm `bug`,
  `enhancement`, `documentation`, `accessibility`. Khi nghi cấu hình đã đổi, kiểm lại bằng
  `gh api repos/{owner}/{repo} --jq '{squash_merge_commit_title,squash_merge_commit_message}'`
  và `gh label list`. Trường trả về `null` không xác nhận cấu hình hiện tại.

## 1. Inspect

```bash
git status --short
git branch --show-current
git fetch origin develop
git log --oneline origin/develop..HEAD
git diff --stat origin/develop...HEAD
git diff --cached --stat
```

Đọc staged diff thật trước khi viết, không viết theo trí nhớ. Giữ nguyên thay đổi không thuộc
task; stage bằng đường dẫn cụ thể, không `git add -A` trong checkout có việc khác. Nếu staged
diff trộn các mục đích độc lập (refactor lẫn feat, sửa tài liệu ngoài phạm vi), tách thành các
commit nguyên tử trước khi viết message.

## 2. Normalize branch

Format `<type>/<issue>-<slug>`, ví dụ `fix/318-tag-save-after-422`; bỏ `<issue>-` khi không có
issue. Chữ thường `a-z`, số, `-`; tối đa 50 ký tự; không tên công cụ, ngày hay tên người.
Bảng `type` ở [conventions](references/conventions.md#nhánh).

| Tình huống | Làm |
| --- | --- |
| Detached HEAD đã có commit của task | Neo nhánh mới tại HEAD hiện tại (`git switch -c <tên-mới>`), hoặc tiếp tục đúng head PR đã giao; không chuyển sang base làm mất dấu commit |
| Bắt đầu task mới từ nhánh tích hợp hoặc detached HEAD không có thay đổi cần giữ | Tạo nhánh mới từ `origin/develop` vừa fetch với `--no-track` (worktree riêng nếu checkout đang có việc khác), trừ khi người dùng yêu cầu commit thẳng |
| Tiền tố công cụ (`codex/`, `claude/`, `ai/`, `agent/`, `copilot/`, `cursor/`, `gemini/`, `antigravity/`…) hoặc sai format, **chưa có trên remote và chưa có PR** | `git branch -m <tên-mới>`, kể cả khi host đặt sẵn tên đó; báo tên mới |
| Đã có trên remote hoặc đã có PR (gồm các nhánh `codex/*` cũ) | Giữ nguyên, ghi một dòng cảnh báo trong báo cáo. Chỉ đổi khi người dùng yêu cầu |
| Người dùng chỉ định tên nhánh | Dùng đúng tên đó |

Kiểm đúng ref bằng `git ls-remote --exit-code --heads origin "refs/heads/$branch"` (0: có, 2: không có) và
`gh pr list --head <nhánh> --state all --json number`. Nếu không tra được remote/PR, giữ nguyên tên.
Không dựa vào `@{u}`: nhánh tạo từ `origin/develop` thường track sẵn `origin/develop` dù chưa từng push.

## 3. Resolve sources

Tra theo thứ tự, chỉ dùng nguồn có thật:

1. Hội thoại hiện tại: URL, số issue/PR, ID quyết định, đường dẫn tài liệu người dùng đưa.
2. Bàn giao của task: issue đọc qua `gh issue view`, prompt `VINK_PROMPT_INPUT_V1` của
   [vink-prompt](../vink-prompt/SKILL.md), ngữ cảnh khôi phục bằng
   [vink-recall](../vink-recall/SKILL.md).
3. Số issue trong tên nhánh (`fix/158-…` → `Refs: #158`), xác minh bằng
   `gh issue view 158 --json number,title,state`.
4. Trailer đã có trên nhánh: `git log --format='%(trailers:only,unfold)' origin/develop..HEAD`.
5. Nguồn nằm trong chính diff: ID quyết định trong `decisions/`, PRD hay ADR mà thay đổi chạm
   tới (tra revision bằng `cargo xtask decision query <ID>`), LayoutSpec, bundle bằng chứng của
   [vink-ui-capture](../vink-ui-capture/SKILL.md) hoặc link Drive từ
   [vink-ui-evidence-drive](../vink-ui-evidence-drive/SKILL.md).

Cấm placeholder (`#TBD`, `https://drive.google.com/...`, `<link>`), số issue đoán, URL dựng từ
tiêu đề, revision quyết định chưa tra. Không tìm thấy nguồn: mặc định **fast mode** — bỏ trailer
và ghi "không có nguồn liên kết" trong báo cáo cuối. **Strict mode** chỉ khi người dùng yêu
cầu — hỏi một câu ngắn trước khi commit.

## 4. Draft & commit

```text
type(scope): mô tả ngắn bằng tiếng Việt

Một–hai câu vì sao: lỗi, yêu cầu hay đánh đổi mà diff không tự nói.

- Thay đổi gom theo hành vi hoặc owner, 3–6 gạch
- Ảnh hưởng/kiểm chứng chỉ khi có: migration, biến môi trường, lệnh đã chạy

Refs: #328
Decision: O-17@r2
```

- Subject ≤ 72 ký tự Unicode, bắt đầu bằng động từ viết thường, không dấu chấm cuối, không
  `(#N)`. Commit nhỏ (khoảng ≤ 30 dòng, typo, đổi config) chỉ cần subject và trailer.
- Body kể *vì sao* và *hành vi đổi*, không kể lại diff theo file/dòng.
- Trailer: `Refs`, `Decision`, `Spec`, `Design`, `Evidence`, `BREAKING CHANGE`; URL trần,
  không Markdown. Keyword đóng issue (`Closes`/`Fixes`) chỉ đặt trong PR body —
  [lý do](references/conventions.md#trailer).
- Viết message qua stdin: `git commit -F - <<'EOF'` … `EOF`; rồi kiểm
  `git log -1 --format=%B` và `git status --short`.
- `Co-Authored-By:` chỉ khi host hoặc người dùng yêu cầu ghi đồng tác giả; PR không có footer
  quảng bá công cụ.

## PR flow

Chỉ push/mở PR khi được giao. Chi tiết và mẫu body: [pull-request.md](references/pull-request.md).

1. Push đúng nhánh: `git push -u origin <nhánh>`; không bao giờ push vào `develop` khi chưa
   được yêu cầu.
2. Đọc **toàn bộ** khoảng `origin/develop..HEAD` (log, diff stat, trailer), không chỉ commit cuối.
3. Tiêu đề: cùng chuẩn subject, ≤ 64 ký tự để còn chỗ cho ` (#NNN)`.
4. Body tiếng Việt: Tóm tắt, Thay đổi chính, Liên kết, Kiểm chứng; Rủi ro và Cần review thêm
   (điểm tác giả chưa chắc chắn) chỉ khi có. Mục không liên quan thì xoá, không ghi "N/A". Kiểm chứng dùng `PASS`/`FAIL`/`NOT_RUN`/`UNSUPPORTED`
   kèm lệnh, SHA và boundary theo [verification guide](../vink-engineering/references/verification.md);
   không ghi kết quả cho lệnh chưa chạy.
5. Mặc định mở `--draft`. Khi được giao mở ready, kiểm bằng chứng cho tiêu đề/Tóm tắt,
   câu hỏi sản phẩm còn mở và nhu cầu xem UI trước review.
6. Chỉ dùng label có sẵn; không tự gán assignee hoặc reviewer khi chưa được giao.
7. Sau khi mở: `gh pr view --json number,url,headRefOid,isDraft`, đối chiếu `headRefOid` với
   `git rev-parse HEAD`.
8. Cập nhật sau review đổi phạm vi hoặc thiết kế: đọc lại cả khoảng, `gh pr edit <n> --title …
   --body-file -`, giữ bằng chứng cũ kèm SHA đã chạy.

## Đọc lịch sử

- Liệt kê issue/quyết định sau một đường dẫn:
  `git log --format='%h %s%n%(trailers:key=Refs,key=Decision,key=Spec,valueonly)' -- <path>`.
- Nếu body squash giữ trailer của từng commit bên trong danh sách `*`, Git có thể không
  nhận chúng là trailer. Tìm bằng chuỗi cố định:
  `git log origin/develop -F --grep='Decision: O-17'`, `git log origin/develop -F --grep='#328'`.
- Số PR ở cuối subject (` (#332)`) mở được bằng `gh pr view 332`. Commit trước skill này dùng
  `Refs #328` (không dấu `:`) và tiêu đề tiếng Việt không có `type(scope)`; tìm theo `#328`.
- Trailer chỉ là con trỏ. Lý do và hành vi vẫn phải đọc từ nguồn được trỏ tới.

## Phối hợp với các skill khác

| Skill | Phần dùng chung |
| --- | --- |
| [vink-work-item](../vink-work-item/SKILL.md), [vink-ui-fix](../vink-ui-fix/SKILL.md) | Gọi skill này khi được giao commit/PR; handoff kiểm chứng đi vào mục Kiểm chứng |
| [vink-specify](../vink-specify/SKILL.md) | Commit đổi quyết định/đặc tả mang `Decision: <ID>@rN` và `Spec:`; PR ghi đúng trạng thái registry (`PENDING` vẫn là `PENDING`; merge không phải signoff hay `Accepted`) |
| [vink-why](../vink-why/SKILL.md), [vink-how](../vink-how/SKILL.md), [vink-recall](../vink-recall/SKILL.md) | Dùng trailer và `--grep` ở mục trên để lần từ code tới issue, PR, quyết định |
| [vink-code-review](../vink-code-review/SKILL.md) | Kiểm PR body khớp diff cả nhánh, mục Kiểm chứng khớp bằng chứng thật, liên kết mở được |
| [vink-prompt](../vink-prompt/SKILL.md) | Prompt giao việc ghi nhánh theo format này cùng quyền commit/push/PR |
| [vink-ui-capture](../vink-ui-capture/SKILL.md), [vink-ui-evidence-drive](../vink-ui-evidence-drive/SKILL.md) | `Evidence:` trỏ tới bundle đã commit hoặc thư mục Drive đã upload |

## Kiểm máy (tuỳ chọn)

```bash
.agents/skills/vink-git-workflow/scripts/git-lint.sh branch            # nhánh hiện tại
.agents/skills/vink-git-workflow/scripts/git-lint.sh message MSG_FILE  # commit message
.agents/skills/vink-git-workflow/scripts/git-lint.sh title "fix(reader): …"
```

Người dùng có thể tự bật hook (commit-msg kiểm message; pre-push chặn tạo nhánh remote **mới**
sai tên, không chặn nhánh cũ đã có) bằng
`git config core.hooksPath .agents/skills/vink-git-workflow/scripts/hooks`. Cấu hình này dùng chung giữa các
worktree, thay nơi Git tìm hook (không xoá hook cũ).
Đường dẫn tương đối được tìm trong checkout đang thao tác; revision chưa chứa script thì
Git không chạy các hook này. Agent không tự bật. Kiểm hồi quy cục bộ, không cần bật hook ở repo thật:
`python3 .agents/skills/vink-git-workflow/scripts/test_git_lint.py`.

## Báo cáo cuối

Nhánh (kèm tên cũ nếu đã đổi), SHA commit, link PR và trạng thái draft, liên kết đã dùng hoặc
"không có nguồn liên kết", và cảnh báo còn lại: nhánh cũ giữ nguyên tên, PR lớn, check `NOT_RUN`.
