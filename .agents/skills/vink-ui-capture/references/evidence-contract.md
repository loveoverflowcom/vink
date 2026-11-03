# Contract bundle UI evidence · format 1

Contract này do `vink-ui-capture` sở hữu; uploader và reviewer cùng dùng. JSON UTF-8,
giữ Unicode tiếng Việt. Field chưa biết dùng `null` + lý do, không dùng dữ liệu ví dụ
như sự thật. Schema version của bundle tách khỏi token/catalog version.

## Identity và cây output

`run_id`: `YYYY-MM-DDTHHmmss+0700_<branch-slug>_<sha8>`, múi giờ
`Asia/Ho_Chi_Minh`; thêm suffix nếu trùng. Lưu branch đầy đủ và SHA đầy đủ trong
manifest. Dirty checkout ghi fingerprint của diff liên quan và file untracked
ảnh hưởng build; không tự upload patch có thể chứa draft/secret.

Giữ `screen_id` theo nghĩa màn qua các platform và các run; không gắn version
design vào ID. `catalog_id`/`scenario_id` lấy nguyên văn từ catalog;
`component_id` theo catalog/API owner; `token_id` dùng đường dẫn khóa nguồn.
Khác nghĩa sản phẩm dùng ID khác; registry ghi alias/retired nếu đổi tên, không
tái sử dụng một ID cho màn khác. Run sau đọc registry/index trước khi cấp ID.

```text
<run_id>/
  manifest.json
  web/{desktop,tablet,mobile-responsive}/
  previews/{leptos,cmp-desktop}/
  cmp/{android,ios}/                 # chỉ khi có host/runtime
  sources/                          # tokens, catalog, contract UI an toàn
  reports/REPORT.md
  reports/navigation.mmd
  reports/navigation-adjacency.json
  comparisons/                      # chỉ khi được giao compare
```

Chỉ tạo thư mục có output. Ảnh ASCII, không khoảng trắng:
`vink__<surface-kind>__<renderer-host>__<subject-slug>__<scenario>__<auth>__<theme>__<state>__<WxH-unit>__<scale>__<nav-variant>__<viewport|full>__<capture-id>.png`.
`capture_id` duy nhất trong run để chống va chạm density/overlay/biến thể khác.
Giữ tên nguồn runner ở metadata nếu copy/đổi tên; không ghi đè raw evidence.

## manifest.json

Top-level bắt buộc: `format_version=1`, `run`, `scope`, `registry`, `captures`,
`navigation`, `components`, `tokens`, `references`, `artifacts`, `coverage`,
`publication`. Các collection có thể rỗng nhưng phải giải thích trong `scope`.

| Trường | Nội dung |
| --- | --- |
| `run` | run_id, timezone, started_at/finished_at ISO-8601, repo, branch, git_sha, dirty, dirty_fingerprint; runtime_build {verified, git_sha, origin, build_variant, evidence}; command/tool versions, data_mode, sanitized environment label |
| `scope` | surfaces/variants được giao, selection rule, exclusion/reason, expected capture IDs trước khi chạy; kết quả không tự làm co expected set |
| `registry` | screen IDs, semantic label, aliases/retired, route/source owner; catalog/scenario IDs tham chiếu version/hash |
| `captures[]` | capture_id, screen_id (nullable cho specimen), component_ids, catalog_id/scenario_id, surface_kind, renderer, host, capture_mode; route/pattern hoặc deep link, click_path; auth, data_mode, requested/effective theme, state, nav_state; viewport {width,height,unit}, image_pixels, DPR/density, text_scale, orientation, locale, OS/device/browser; captured_at, readiness/motion; capture_status, reason, inspected; artifact_id, reference_ids, compare_status, notes |
| `navigation` | nodes bằng screen/overlay IDs; edges {from,to,action,pre_state,post_state,guard,verification,capture_ids,source}; adjacency list cùng dữ liệu, không suy guard PASS từ ảnh |
| `components[]` | component_id, API/source owner, renderer support, catalog/scenario IDs, app usages {screen_id,source,verification}, state/variant và capture_ids; sample/service boundary |
| `tokens` | source artifact_id/path/version/hash, catalog version/hash, generated file paths/hash và drift check {status,command}; observations[] {token_id,role,source_value,resolved_value,unit,renderer,scope,theme,override,method,capture_id,status} |
| `references[]` | reference_id, screen_id/component_id, exact asset URL/ID, folder, version/amendment/scope, authority (confirmed/candidate/unknown), authority_evidence URL/text, checked_at, compatible theme/state/form factor và mapping reason |
| `artifacts[]` | artifact_id, relative path, kind, MIME, byte_size, local_sha256; screenshot còn có pixel width/height; upload {status,file_id,url,parent_id,verified_at,integrity,remote_checksum,error} |
| `coverage` | expected/captured/inspected/blocked/not_implemented/unsupported/out_of_scope, uploaded/metadata_verified/content_verified; breakdown theo surface/host/theme/state, không gộp specimen và product |
| `publication` | run_folder ID/URL, manifest ID/URL/readback status, report ID/URL, canonical index ID/URL/update status, complete/partial và lý do |

Các timestamp và URL runtime phải loại credential/query có secret. `auth` phân
biệt `guest`, `local-fixture`, `authenticated`, `not-applicable`, `unknown`;
preview không được gắn `authenticated` vì trông giống màn cá nhân.
`data_mode` ghi `synthetic`, `simulated`, hoặc nguồn thực đã được phép và làm sạch.

`capture_status`: `CAPTURED`, `BLOCKED`, `NOT_IMPLEMENTED`, `UNSUPPORTED`,
`OUT_OF_SCOPE`. Mọi expected capture đều có record; các trạng thái khác CAPTURED
cần lý do và prerequisite. Ảnh lỗi tái hiện thành công vẫn là CAPTURED/state=error.
`inspected` là boolean độc lập. `compare_status`: `NOT_REVIEWED`, `NO_REFERENCE`,
`NOT_COMPARABLE`, `REVIEWED`; không có mặc định MATCH/PASS.

Upload status độc lập: `NOT_REQUESTED`, `PENDING`, `UPLOADED`, `VERIFIED`,
`FAILED`, `UNCERTAIN`. `VERIFIED` nghĩa đã đọc lại metadata đúng parent/name/type/
size và URL; `integrity` riêng là `content-verified` (hash/download đối chiếu),
`metadata-only` hoặc `unavailable`. Không gọi metadata-only là toàn vẹn byte.
`remote_checksum` lưu cả thuật toán, không so MD5 của Drive với SHA-256 local.

Không đưa `manifest.json` vào danh sách tự hash chính nó. Nó là control file,
upload sau các artifact và ghi ID/readback/checksum cuối vào receipt local hoặc
output tool; tránh vòng manifest chứa chính checksum của manifest. `publication`
có thể giữ null cho ID của chính manifest đến khi được xác nhận ngoài file.

## Capture focused optional

Capture focused là bổ sung optional cho format 1: dùng `capture_mode=specimen`,
ghi `raw_artifact_id`, `clip_css`, `source_bounds_css`, padding và `tile/tiles` khi
chia specimen cao. Các artifact raw và focus có ID/hash/kích thước riêng; không
ghi đè raw hoặc suy `inspected=true` từ thành công của runner. Không đổi nghĩa
record format 1 cũ. Ảnh raw của Leptos preview có giới hạn 6000 CSS px nên khi nhập
bundle phải ghi `capture_mode=document-capped`, không gọi là full-page đầy đủ.

`focus-manifest.json` do preview runner xuất là subset `vink-preview-focus-v1`,
không phải bundle format 1 hoàn chỉnh: có source fingerprint, 36 expected records,
raw ↔ focus, bounds/PNG/hash và coverage captured/inspected/blocked. Import vào
bundle theo bảng trên, giữ nguyên tên raw và provenance. Source SHA khi dirty chỉ
định danh base; source fingerprint và build context mới xác định đầu vào đã chụp.

## Báo cáo và graph

`reports/REPORT.md` là bàn giao của một run, không phải index trung tâm thứ hai:

- SHA/build/scope/data mode, command đã chạy và giới hạn; coverage + blockers.
- Inventory màn/navigation/component/preview/token; bảng liên kết capture,
  artifact, source và reference đúng variant; thao tác tái hiện được.
- Graph navigation Mermaid và adjacency fallback; cạnh observed và source-only
  khác nhãn, edge tới design là dotted, không trở thành route app.
- Chưa review ghi rõ. Nếu được giao compare, mỗi finding có ID, screen/component,
  observed/expected, ảnh đã mở, reference/rule, P0–P3, impact, confidence, hướng sửa.
  P0 là rủi ro nghiêm trọng có bằng chứng; P1 cản trở tác vụ chính; P2 lỗi có
  workaround; P3 chi tiết ít ảnh hưởng. Không gán severity cho gap chưa quan sát.

Mermaid node ID dùng alias an toàn như `s_explore_feed`, label giữ `screen_id`;
ID có dấu chấm của catalog/screen không được chèn thành cú pháp graph tùy ý.
Giữ quan hệ `screen → component → catalog/scenario → capture` và `component → token`
trong manifest/table khi đã trace được; không đoán component tiêu thụ token từ màu
ảnh. Render SVG/PNG graph chỉ khi có tool, giữ Mermaid + adjacency nếu không có.

Sau upload, link Drive lấy từ readback; chưa upload dùng đường dẫn tương đối local
và ghi pending. Báo cáo upload có thể thay link local bằng link xác minh trước khi
được upload. Kiểm every capture có artifact hoặc lý do; every artifact có nơi tham
chiếu; every edge/component/token reference trỏ đến ID tồn tại; mọi tổng đếm tính
từ record. Review sau pin `run_id` + manifest, không chỉ dựa vào nhãn LATEST.
