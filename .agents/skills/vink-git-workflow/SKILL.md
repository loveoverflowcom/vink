---
name: vink-git-workflow
description: Name Vink branches, write commit messages with verified source trailers and open or update pull requests with an honest Vietnamese body. Use before creating a branch, committing, pushing or opening/editing a PR, and when reading Git history for the issue, decision or spec behind a change.
---

# Vink Git workflow

Dùng base, worktree, tên nhánh và phạm vi commit/push/PR người dùng đã giao. Kiểm
`git status --short --branch` trước khi thao tác. Với PR đang có, đối chiếu head trên GitHub
với checkout. [Quy ước của repo](../../../AGENTS.md#work-and-verification)

| Việc được giao | Cách làm |
| --- | --- |
| Tạo hoặc sửa tên nhánh | Dùng `origin/develop` vừa fetch cho task mới nếu người dùng chưa chỉ định base. Kiểm remote, PR và thay đổi cục bộ trước khi đổi tên. [Quy ước nhánh và worktree](references/conventions.md#nhánh) |
| Viết commit | Stage đường dẫn của task, đọc `git diff --cached`, tách mục đích độc lập. Viết subject/body tiếng Việt và trailer từ nguồn đã xác minh. Sau commit, đọc lại `git log -1 --format=%B` và status. [Mẫu commit](references/conventions.md#ví-dụ-commit) · [Tra nguồn](references/conventions.md#tra-nguồn) |
| Push hoặc mở/cập nhật PR | Push đúng nhánh đã giao. Viết từ toàn bộ diff so với base, ghi SHA và kết quả kiểm chứng thực tế. Dùng trạng thái draft/ready người dùng yêu cầu. Khi mở mới mà chưa được chỉ định trạng thái, mặc định draft. Sau thao tác, kiểm lại base, head SHA và trạng thái trên GitHub. [Mẫu và lệnh PR](references/pull-request.md) |
| Tìm issue, quyết định hoặc lý do của một thay đổi | Theo trailer và số PR về nguồn gốc. Với commit squash, tìm cả nội dung message. [Đọc lịch sử](references/conventions.md#đọc-lịch-sử) |

Có thể dùng [git-lint](references/conventions.md#kiểm-máy-tuỳ-chọn) để kiểm tên nhánh,
commit hoặc tiêu đề PR. Khi bàn giao, ghi nhánh, commit, link và trạng thái PR nếu đã mở.
