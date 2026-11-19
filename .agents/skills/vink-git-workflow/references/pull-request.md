# Pull request

Tham chiếu cho [vink-git-workflow](../SKILL.md). Commit phục vụ lịch sử; PR phục vụ người review:
toàn cảnh nhiều commit, rủi ro và cách kiểm chứng. Body là Markdown nên dùng link có nhãn ở đây.

## Trước khi viết

```bash
git fetch origin develop
git log --format='%h %s' origin/develop..HEAD
git diff --stat origin/develop...HEAD
git log --format='%(trailers:only,unfold)' origin/develop..HEAD | sort -u
ls .github/pull_request_template.md .github/PULL_REQUEST_TEMPLATE 2>/dev/null
```

- Viết từ **toàn bộ** khoảng commit và diff, không chỉ commit cuối.
- Repo có template thì điền đúng template đó thay cho mẫu dưới đây.
- Trailer `Refs`/`Decision`/`Spec`/`Design`/`Evidence` của các commit gom thành mục Liên kết.
- **PR lớn:** quá khoảng 500 dòng logic (không tính lockfile, snapshot, ảnh bằng chứng, code
  sinh) hoặc gộp nhiều mục tiêu độc lập thì nêu trong báo cáo và đề xuất tách. Không tự chặn.

## Tiêu đề

Cùng chuẩn subject commit ([conventions](conventions.md#commit-subject)), ≤ 64 ký tự để subject
squash `… (#NNN)` vẫn ≤ 72. Không mã nhánh, không `(#N)`, không `Update`/`Misc`.

## Mẫu body

```markdown
## Tóm tắt
Hai–bốn câu: vấn đề, hướng đã chọn, và phương án bị loại kèm lý do khi có đánh đổi.

## Thay đổi chính
- **Reader:** …
- **App/API:** …
- **Kiểm thử:** …

## Liên kết
- Refs #328 · kế thừa #330
- Quyết định: `O-17@r2` (`PRODUCT_CONFIRMED`) — [decisions/…](https://github.com/<owner>/<repo>/blob/<sha>/decisions/…)
- Bằng chứng: [Ảnh và số đo Auth R2](https://github.com/<owner>/<repo>/blob/<sha>/docs/design/evidence/…)

## Kiểm chứng
- PASS `cargo xtask verify quick` — source `<sha>`, unit/contract + repository checks
- NOT_RUN `cargo xtask verify postgres` — chưa có `VINK_TEST_DATABASE_URL` trên máy này
- Không chạy GitHub CI. Chưa kiểm: thiết bị thật, IME hệ điều hành.

## Rủi ro và triển khai
- Migration/biến môi trường/đổi contract, cách rollback; bốn chiều trạng thái khi chạm quyết định.

## Cần review thêm
- Lựa chọn tác giả PR chưa chắc chắn hoặc cần chủ sản phẩm chốt, kèm lý do và phương án thay thế.
```

Các `<owner>`, `<repo>`, `<sha>`, `…` trong mẫu là chỗ thay bằng giá trị thật; PR không bao giờ
chứa chúng.

- **Tóm tắt** và **Thay đổi chính** gom theo luồng nghiệp vụ hoặc owner, không liệt kê từng file.
- **Liên kết:** `Closes #N` chỉ khi PR hoàn tất mọi AC của issue; ngược lại `Refs #N`. Mỗi link
  phải đến từ nguồn đã tra ([thứ tự tra](../SKILL.md#3-resolve-sources)). Không có nguồn thì
  xoá mục.
- **Kiểm chứng:** mỗi dòng là một lệnh hoặc kịch bản đã chạy thật trong phiên, với kết quả
  `PASS`/`FAIL`/`NOT_RUN`/`UNSUPPORTED`, SHA source đã chạy và boundary (pure, PostgreSQL thật,
  browser, cmp). Không dùng checkbox `[x]`. Kết quả tái dùng từ SHA cũ ghi rõ SHA đó và vì sao
  còn áp dụng ([verification guide](../../vink-engineering/references/verification.md#evidence-handoff-and-provenance)).
  Workflow CI có trong repo không phải bằng chứng CI đã chạy.
- **Rủi ro và triển khai** chỉ khi có migration, biến môi trường, đổi contract, breaking change,
  ảnh before/after hoặc thay đổi trạng thái quyết định. Không có thì xoá mục.
- **Cần review thêm** ghi điều tác giả PR chưa chắc chắn: suy luận chưa kiểm được, lựa chọn
  có thể sai hướng, quy ước mới cần chủ sản phẩm chốt. Mỗi dòng nêu điểm nghi, vì sao và cách
  kiểm hoặc phương án thay thế. Không có thì xoá mục; không biến nó thành danh sách khiêm tốn
  chung chung.
- Không thêm footer kiểu "Generated with …" của công cụ AI.

## Draft hay ready

Mặc định mở `--draft`. Nếu được giao mở ready, vẫn giữ draft và nêu lý do khi:

- một khẳng định ở tiêu đề hoặc Tóm tắt chưa có bằng chứng (check cần cho khẳng định đó là
  `NOT_RUN` hoặc `FAIL`);
- còn câu hỏi sản phẩm hoặc thiết kế mở cần chủ sản phẩm chốt;
- thay đổi UI cần người xem ảnh/video trước khi review.

Boundary nằm ngoài phạm vi đã nêu rõ (thiết bị thật, native host) và được ghi `NOT_RUN` trong
Kiểm chứng không tự cản yêu cầu chuyển ready.

## Lệnh tạo và kiểm

```bash
gh label list --limit 100
gh pr create --draft --base develop --head "$branch" --title "$title" --body-file - <<'EOF'
<body>
EOF
gh pr view --json number,url,headRefOid,isDraft
git rev-parse HEAD
```

- Label: `feat` → `enhancement`, `fix`/`hotfix` → `bug`, `docs` → `documentation`; chỉ gắn label
  có trong `gh label list`.
- Không tự gán assignee (kể cả `@me`) hay reviewer khi chưa được giao. Repo chưa có `CODEOWNERS`.
- `headRefOid` phải bằng `git rev-parse HEAD`; khác thì fetch và kiểm tra lại, không ghi đè commit người khác.

## Cập nhật PR

Sau vòng review đổi phạm vi, thiết kế hoặc thêm commit đáng kể:

1. Đọc lại cả khoảng `origin/develop..HEAD`.
2. Viết lại Tóm tắt, Thay đổi chính và Kiểm chứng theo HEAD mới; giữ kết quả cũ kèm SHA đã chạy.
3. `gh pr edit <n> --title '<tiêu đề>' --body-file - <<'EOF'` … `EOF`.
4. Chỉ đổi draft/ready (`gh pr ready <n>`) khi điều kiện draft ở trên đã hết.

Bình luận trả lời review, gắn reviewer hay merge là hành động riêng; làm khi được giao.

## Khi squash

Người merge giữ tiêu đề PR làm subject. Squash mặc định nối mọi commit message thành danh sách;
có thể gom các dòng `Refs:`/`Decision:` về đoạn cuối để git đọc được trailer trên `develop`.
Agent không merge khi chưa được giao.
