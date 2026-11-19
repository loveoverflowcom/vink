# Nhánh, commit, trailer và liên kết

Tham chiếu cho [vink-git-workflow](../SKILL.md). Commit message là plain text cho `git log` và
`git show`; PR body là Markdown cho người review ([pull-request.md](pull-request.md)).

## Nhánh

Format `<type>/<issue>-<slug>`; bỏ `<issue>-` khi không có issue.

| Type | Dùng cho | Ví dụ |
| --- | --- | --- |
| `feat/` | Hành vi hoặc tính năng mới | `feat/294-follow-author` |
| `fix/` | Sửa lỗi trên luồng phát triển thường | `fix/318-tag-save-after-422` |
| `hotfix/` | Vá khẩn bản đang chạy production; base theo người dùng chỉ định, mặc định `origin/develop` | `hotfix/podman-healthcheck` |
| `refactor/` | Đổi cấu trúc nội bộ, không đổi hành vi bên ngoài | `refactor/split-discussion-reads` |
| `perf/` | Hiệu năng, bộ nhớ, truy vấn, ngân sách tải | `perf/238-comment-containment` |
| `chore/` | Build, CI, dependency, tooling, skill của agent | `chore/git-workflow-skill` |
| `docs/` | PRD, ADR, đặc tả, kho quyết định, README | `docs/289-social-layout` |
| `test/` | Test, fixture, lane kiểm chứng | `test/web-ci-pointer-sync` |

- Regex: `^(feat|fix|hotfix|refactor|perf|chore|docs|test)/([0-9]+-)?[a-z0-9]+(-[a-z0-9]+)*$`,
  tối đa 50 ký tự.
- Slug 2–5 từ tiếng Anh hoặc tiếng Việt bỏ dấu (`đ` → `d`): `fix/328-giu-nhap-nhan-xet`.
- Không tiền tố công cụ (`codex/`, `claude/`, `ai/`, `agent/`…), không ngày, không tên người.
  Tên nhánh mô tả loại việc và kết quả, không mô tả công cụ đã viết code.
- Tạo từ base mới fetch, không track base, để `git push -u` đặt upstream đúng tên:

  ```bash
  git fetch origin develop
  git switch --no-track -c fix/318-tag-save-after-422 origin/develop
  git worktree add --no-track -b fix/318-tag-save-after-422 ../vink-fix-318 origin/develop
  ```

## Commit subject

`type(scope): mô tả` hoặc `type: mô tả` khi thay đổi cắt ngang nhiều owner. Thêm `!` sau type
hoặc scope cho thay đổi phá tương thích (`feat(api)!: …`) kèm trailer `BREAKING CHANGE:`.

- **Type:** `feat`, `fix`, `refactor`, `perf`, `chore`, `docs`, `test`, `revert`. Nhánh
  `hotfix/` dùng `fix`. CI và build dùng `chore(ci)`, `chore(build)`.
- **Scope:** owner nhỏ nhất mà người đọc log nhận ra, viết thường:

  | Nhóm | Scope |
  | --- | --- |
  | Package/service | `app`, `identity`, `web`, `editor`, `worker`, `ui-system`, `patterns`, `expressions`, `store-postgres`, `postgres`, `xtask`, `infra` |
  | Domain/journey | `reader`, `desk`, `auth`, `discussion`, `publication`, `moderation`, `endorsement`, `taxonomy`, `discovery`, `social`, `profile` |
  | Tài liệu/quy trình | `prd`, `decisions`, `design`, `skills`, `ci` |

  Bảng là gợi ý, không đóng; chọn tên đã có trong repo thay vì tạo từ mới.
- **Mô tả:** tiếng Việt, bắt đầu bằng động từ viết thường (`giữ`, `sửa`, `thêm`, `tách`,
  `bỏ`, `chặn`), nêu kết quả hành vi; giữ nguyên định danh kỹ thuật. Không dấu chấm cuối.
- **Độ dài:** ≤ 72 ký tự Unicode (không tính byte). Không `(#N)` vì issue nằm ở trailer và
  GitHub tự nối số PR.
- Tránh: `Update`, `Misc fixes`, `WIP`, `Sửa theo review`, mã nhánh thô, tên công cụ AI.

| Không nên | Nên |
| --- | --- |
| `Update reader` | `fix(reader): giữ nháp nhận xét khi mở đăng nhập` |
| `Sửa theo review` | `fix(profile): chặn byline cũ khi quay lại trang` |
| `Thêm VDR social` | `docs(decisions): thêm VDR-SOCIAL-CONTRACT@r1 ở trạng thái PENDING` |
| `Khám phá r1 (#308)` | `feat(discovery): tìm toàn màn hình và trích đoạn bản công khai` |

## Commit body

- Một dòng trống sau subject; ngắt dòng khoảng 72 cột (URL không bắt buộc).
- **Vì sao (1–2 câu):** lỗi, yêu cầu hay đánh đổi kiến trúc. Đây là phần diff không tự nói.
- **Thay đổi (3–6 gạch):** gom theo hành vi hoặc owner. Chỉ nêu tên file khi chính file là
  điểm chính (migration, contract HTTP, manifest).
- **Ảnh hưởng/kiểm chứng (tuỳ chọn):** migration, biến môi trường mới, đổi contract, breaking;
  lệnh đã chạy trên đúng nội dung commit (`Kiểm: cargo xtask verify quick PASS`).
- Khi chạm quyết định hoặc trạng thái triển khai, gọi đúng chiều (sản phẩm, kiến trúc, mức
  triển khai, bằng chứng). Merge hay pass check không biến đề xuất thành `Accepted`.
- Commit nhỏ (khoảng ≤ 30 dòng, typo, đổi config, chỉnh link) chỉ cần subject và trailer.

## Trailer

Đoạn cuối, cách body đúng một dòng trống, mỗi dòng `Key: value`, URL trần, không Markdown.
Lặp key cho nhiều giá trị. Chỉ dùng các key sau:

| Key | Giá trị | Ví dụ |
| --- | --- | --- |
| `Refs` | Issue, PR hoặc commit liên quan | `Refs: #328`, `Refs: loveoverflowcom/vink#120`, `Refs: 4cac8373` |
| `Decision` | ID quyết định kèm revision đã tra bằng `cargo xtask decision query` | `Decision: O-17@r2`, `Decision: VDR-SOCIAL-CONTRACT@r1` |
| `Spec` | Requirement ID, tài liệu sở hữu trong repo, hoặc URL tài liệu người dùng đưa | `Spec: MOD-09`, `Spec: docs/architecture/social-graph.md` |
| `Design` | LayoutSpec, mốc thị giác hoặc URL thiết kế người dùng đưa | `Design: docs/design/tag-selector.md` |
| `Evidence` | Bundle bằng chứng đã commit hoặc thư mục Drive đã upload | `Evidence: docs/design/evidence/auth-r2-328/README.md` |
| `BREAKING CHANGE` | Điều phá tương thích và cách chuyển | `BREAKING CHANGE: bỏ trường bio khỏi PUT /me/profile` |
| `Co-Authored-By` | Chỉ khi host hoặc người dùng yêu cầu ghi đồng tác giả | |

**Không đặt keyword đóng issue (`Closes`, `Fixes`, `Resolves`) trong commit.** `develop` là
default branch, nên keyword trong commit message (kể cả khi nằm trong body squash) đóng issue
ngay khi tới `develop`. Nhiều issue Vink trải qua nhiều PR (epic như #120). Đóng issue thuộc về
PR body, và chỉ khi PR hoàn tất mọi AC của issue.

`Decision` chỉ trích ID; trạng thái (`PENDING`, `PRODUCT_CONFIRMED`…) đọc từ registry, và nếu
cần nêu thì nêu đúng như registry trả về.

## Ví dụ commit

```text
fix(reader): giữ nháp nhận xét khi mở đăng nhập

Mở Auth từ ô soạn làm Reader mất focus và nội dung đang gõ; ACK muộn
còn mở lại modal sau khi người đọc đã đóng.

- Dùng một host Auth cho sheet mobile và dialog desktop
- Trả focus, selection và vị trí viewport sau khi App xác nhận principal
- Bỏ qua ACK đến sau khi modal đã đóng

Refs: #328
Evidence: docs/design/evidence/auth-r2-328/README.md
```

Commit nhỏ:

```text
docs(skills): sửa link verification guide trong vink-prompt
```

## Liên kết theo nơi xuất hiện

| Loại | Commit (plain text) | PR body (Markdown) |
| --- | --- | --- |
| Issue/PR cùng repo | `Refs: #328` | `Refs #328`, hoặc `Closes #328` khi PR hoàn tất issue |
| Issue repo khác | `Refs: owner/repo#12` | `owner/repo#12` |
| File trong repo | Đường dẫn từ root: `docs/x.md` | Code span `` `docs/x.md` `` hoặc permalink `https://github.com/<owner>/<repo>/blob/<sha>/docs/x.md` |
| Quyết định | `Decision: O-17@r2` | `` `O-17@r2` `` (`PRODUCT_CONFIRMED`) kèm permalink tới file TOML |
| Tài liệu ngoài (Drive, Docs, Figma) | `Spec:`/`Design:`/`Evidence:` + URL trần | `[nhãn rõ nghĩa](URL)` |
| Commit | `Refs: <sha 8+ ký tự>` | SHA 8+ ký tự; GitHub tự link |

- Link tương đối trong PR body bị resolve theo URL trang PR (`…/pull/<n>/docs/x.md`) nên gãy.
  Dùng permalink với `owner/repo` lấy từ `gh repo view --json nameWithOwner` và SHA đã push,
  hoặc code span.
- Không dựng URL từ tiêu đề hay đoán ID tài liệu; không rút gọn URL thành `.../`.
