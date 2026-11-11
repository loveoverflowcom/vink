# Rà soát và chuyển Decision Registry sang TOML

Lát cắt của [#164](https://github.com/loveoverflowcom/vink/issues/164) lấy nguồn tại
`develop@0c5bb275c15342607e6701f14019c07e269c2a68`. Phạm vi có thể đối chiếu theo từng mục là
[DecisionDelta §2–4](../architecture/product-decisions-20261110.md#2-decisiondelta). Tra cứu và
ghi đề xuất qua [cargo xtask decision](../../decisions/README.md). Các đánh giá nghiệp vụ và
phê duyệt sau lần nhập này còn cần nguồn quyết định tương ứng.

## Quyền sở hữu và kiểm kê

| Nguồn tại SHA trên | ID hoặc phạm vi và nơi dùng | Xử lý trong PR | Điểm còn rà |
| --- | --- | --- | --- |
| [DecisionDelta §2](../architecture/product-decisions-20261110.md#2-decisiondelta) | `USER-01..15`, mapping bên dưới. PRD §14, vision §13, D-log, domain specs, AGENTS, skills và model đích | `migrate_decision` sang TOML. Giữ bảng gốc làm `historical_alias` và lịch sử code của lượt 10/11 | `USER-09` xác nhận bài ngắn và bộ sưu tập, cùng dòng ghi nhiều chương được hoãn (`WR-04`) |
| [DecisionDelta §3–4](../architecture/product-decisions-20261110.md#3-lựa-chọn-triển-khai-design_proposed) | Bốn `CHOICE`, `O-01..16`. Specs theo miền và model M1–M6 | `migrate_decision` với trạng thái cũ. Scope đầy đủ cho CHOICE để tránh trùng với issue khác | Baseline model và câu hỏi có thể được review theo miền khi triển khai |
| [DecisionDelta §1, §5–6](../architecture/product-decisions-20261110.md) | Giải nghĩa nhãn, UX/dữ liệu đề xuất, snapshot triển khai, kế hoạch PR | `keep_spec`, `keep_runtime_evidence` và lịch sử | Các mục chưa có ID riêng ở §5 cần review nếu muốn chuyển thành quyết định |
| [Vision](../../prd/vision.md), [requirements](../../prd/requirements.md), [LVE PRD](../../prd/literary-endorsement.md) | Phạm vi, ID/acceptance, hàng đề xuất UX và guard nghiệp vụ | `keep_spec`. Thêm đường tra record vào §14. ID requirement vẫn có owner ở PRD | Đối chiếu lại các điều kiện dễ hiểu khác nhau trong bảng review bên dưới |
| [Sổ D/INV/S](../architecture/decisions.md), [tech stack](../architecture/tech-stack.md) | `D-01..19`, `INV-01..08`, `S-01..05`. Có hàng lẫn quyết định, phương án, lịch sử và bằng chứng | `pending_review` cho phần quyết định riêng. `keep_spec` cho INV. Giữ S-ID và bằng chứng spike | Cần tách phần sản phẩm và kiến trúc trong D-02, D-05..08, D-12, D-14..19. Các D-ID hiện chỉ tra được dưới dạng liên quan nếu nằm trong mapping |
| [Ownership thảo luận](../architecture/discussion-ownership.md), [admission](../architecture/discussion-admission.md), [reads](../architecture/discussion-reads.md), [outbox](../architecture/discussion-outbox.md), [ranh giới Identity](../architecture/identity-postgres-boundaries.md), [auth authority](../architecture/auth-authority.md) | D-02/D-05/D-08/D-13, quyền tác giả và ủy quyền, `DISC-*` và `CHOICE-*` cục bộ | `pending_review` cho quyết định riêng của ADR. Giữ spec và hiện trạng tại owner | D-05 vẫn Provisional về authority. Lịch sử `USER-08@r1` chỉ nói phương thức đăng nhập |
| [LVE](../architecture/literary-endorsement.md) và [taxonomy](../architecture/work-taxonomy.md) | D-14, D-15, `LVE-PR-01`, `TAG-PR-01`, CHOICE cục bộ theo từng issue | `pending_review` cho quyết định kỹ thuật có ID. Giữ runtime/evidence trong registry phát triển | Phạm vi ID phải gồm cả issue/lát cắt để tránh nhập nhầm với CHOICE của DecisionDelta |
| [Publication](../architecture/publication-moderation.md), [sửa bình luận](../architecture/comment-editing.md), [bộ sưu tập](../architecture/collections.md), [xếp hạng](../architecture/discovery-ranking.md), [hashtag](../architecture/hashtags.md) | Đặc tả đích dẫn USER/CHOICE/O; review đặc biệt `PUB-04/05`, `O-03..06`, `O-09`, `O-15/16` | `keep_spec`. Ghim mẫu cho quyền ghi tombstone. TOML sở hữu trạng thái quyết định đã nhập | Mở rộng marker sau khi xác định consumer ảnh hưởng theo từng thay đổi |
| [Design](../design/README.md), [source map UI](../../.agents/skills/vink-ui-design/references/source-map.md) | LayoutSpec, token, ảnh review và thiết kế đã chọn cho từng bề mặt | `keep_spec` và `keep_runtime_evidence`. `pending_review` cho lựa chọn thiết kế có tuổi thọ ngoài một issue | Cần rà lựa chọn theo từng màn nếu đưa quyết định thiết kế vào kho |
| [Development](README.md), [feature map](feature-map.md), [registry 10/11](product-decisions-registry.md) | Cấu trúc code, runner, hành vi đang có, implementation level và bằng chứng theo SHA | `keep_runtime_evidence`. Thêm link VDR để tra trạng thái quyết định | Những câu tóm tắt hiện trạng cũ trong PRD/D-log cần dọn trong lượt đối soát runtime |
| [References](../references/sources.md), [skill adaptation](../references/skill-adaptation.md), [archive](../archive/initial/README.md) | Nguồn Cantos/Discourse, lịch sử bộ định hướng/skill và vision gốc | `historical_alias` hoặc `excluded_with_reason`. Giữ nguyên bằng chứng, snapshot và pin | Bản nguồn ngoài repo hoặc chat gốc của 10/11 chưa được xác minh độc lập |
| [Skill index](../../.agents/skills/README.md), [AGENTS](../../AGENTS.md), [CLAUDE](../../CLAUDE.md) | Skill và reference của shape, specify, work-item, architect, why, recall, prompt, code-review, maintain-verification, behavior, publication, UI design | Cùng dùng một hướng dẫn tra cứu và luồng ghi qua specify. Giữ metadata discovery và Claude symlink | Các plugin ngoài repo và prompt đã sao chép sang issue/chat cần đối chiếu khi được dùng lại |
| [Model đích](../../services/vink-app/behavior/target/README.md), [publication application](../../services/vink-app/src/application/publication.rs), [password core](../../services/vink-identity/src/password.rs) | `USER-05/06/09/12..15`, `CHOICE-01`, `O-03`, `O-12` và một số ID trong contract/migration | `keep_spec` cho model và `keep_runtime_evidence` cho code/test. Ghi trường hợp suy luận từ code bên dưới | Marker v1 có phạm vi Markdown. ID trong Rust, SQL, Quint, JSON và prompt cũ cần review theo miền |
| [Dispatcher](../../xtask/src/main.rs), [checks](../../xtask/src/checks.rs), [workflow](../../.github/workflows/ci.yml), [Cargo.lock](../../Cargo.lock) | CLI, kiểm skill/link/ranh giới và lane local `quick` | `excluded_with_reason` khỏi record vì đây là tooling/config. Kiểm registry qua lane hiện có | Phần cưỡng chế ngoài checkout local cần lựa chọn nguồn quyền riêng |

Kiểm kê duyệt nhóm nguồn qua danh sách file theo Git, heading/bảng và các tham chiếu
`DecisionDelta`, `PRODUCT-DECISIONS-20261110`, `O-*`, `USER-*` trong Markdown, skill
references, model và code lân cận. Đối chiếu từng hàng đầy đủ cho §2–4. Các nhóm
`pending_review` có owner và phạm vi để tiếp tục rà nội dung từng mục. Marker được kiểm
cho các consumer đã khai báo. Bảng kiểm kê ghi những nguồn nằm ngoài phạm vi marker.

## ID đã nhập

Các ID dưới đây giữ nguyên và mặc định ở `@r1`. `USER-08@r1` giữ phương thức mã email từng
được đề xuất, với trạng thái Provisional và ngữ cảnh lịch sử. `USER-08@r2` là hướng đăng nhập
chính được chốt trong DecisionDelta. Mỗi dòng TOML giữ dòng nguồn và các ID liên quan.
Requirement ID và D-ID có thể liên quan nhiều bản ghi. Chúng vẫn tra cứu chính tại owner.

| ID | Trạng thái | D-ID, requirement hoặc chủ đề để truy vết |
| --- | --- | --- |
| USER-01 | DEFERRED | D-11, vision §2/§12, nhóm tuổi |
| USER-02 | PRODUCT_CONFIRMED | D-06, AUD-01/02, PUB-01 |
| USER-03 | PRODUCT_CONFIRMED | D-07, MOD-02/05/07 |
| USER-04 | PRODUCT_CONFIRMED | D-08, COM-02/09 |
| USER-05 | PRODUCT_CONFIRMED | D-07, MOD-03/08 |
| USER-06 | PRODUCT_CONFIRMED | D-19, MOD-04/10 |
| USER-07 | PRODUCT_CONFIRMED | D-12, PUB-03/04/05 |
| USER-08 | PRODUCT_CONFIRMED | D-05, ACC-03 |
| USER-09 | PRODUCT_CONFIRMED | D-10/16, WR-04 (hoãn), COL-01..06 |
| USER-10 | DEFERRED | D-09, ED-02, vision §3 |
| USER-11 | PRODUCT_CONFIRMED | D-15/17, HTG-01..06 |
| USER-12 | PRODUCT_CONFIRMED | D-14/18, RANK-01..04 |
| USER-13 | PRODUCT_CONFIRMED | D-18, TOP-01..04 |
| USER-14 | PRODUCT_CONFIRMED | D-19, MOD-09, INV-03 |
| USER-15 | PRODUCT_CONFIRMED | D-08, COM-10, LVE-FR-04 |
| PRODUCT-DECISIONS-20261110-CHOICE-01..04 | DESIGN_PROPOSED | Theo thứ tự sửa bình luận, điểm Đề xuất, cursor Top 100 và grammar hashtag |
| O-01/02 | OPEN | Membership và thứ tự bộ sưu tập, COL-05/06 |
| O-03/04/05/08/09/10 | OPEN | Quyền ghi, bình luận chờ, xóa khi đã riêng tư, purge, dữ liệu trang thay thế và khôi phục, PUB-04/05 |
| O-06/07 | OPEN | “Hữu ích” theo revision bình luận và vote dùng cho ranking, LVE-FR-04/RANK-02 |
| O-11/12 | OPEN | Khôi phục và định dạng credential, ACC-03 |
| O-13/14 | OPEN | Giới hạn hashtag và tham số xếp hạng, HTG-06/RANK-01/04 |
| O-15/16 | OPEN | Xét lại quyết định admin và hủy bản sửa chờ duyệt, MOD-10/COM-10 |

## Lựa chọn của lát cắt và đánh đổi

| ID của #164 | Cách triển khai | Khi cần xem lại |
| --- | --- | --- |
| CHOICE-01 | `OPEN`. Dùng pin riêng của tool để kiểm toàn vẹn dữ liệu nhập. Writer chỉ lưu `PENDING` | Chủ sản phẩm chọn nguồn xác nhận cho revision mới và mức cưỡng chế mong muốn |
| CHOICE-02 | Revision bất biến ở đường dẫn theo ID/revision. Bundle legacy được nhập một lần. `diff` đọc hai payload và chỉ in trường thay đổi | So sánh với file current cố định nếu PO cần xem diff dòng trực tiếp trên GitHub thường xuyên |
| CHOICE-03 | Rust/Serde và validator làm contract v1, dùng phiên bản crate TOML đã có trong Cargo.lock | Khi có editor consumer, thử corpus chung và kiểm dialect trước khi sinh schema |
| CHOICE-04 | Tra cứu và xuất từ TOML mỗi lần. Không có index/export tracked | Đo quy mô thật và xem PO có cần view dựng sẵn trên GitHub hay không |
| CHOICE-05 | Mở rộng `vink-specify` để ghi proposal, chia sẻ lệnh qua skill index | Xem lại nếu lifecycle phê duyệt sau này cần owner riêng |
| CHOICE-06 | Marker Markdown có mode active/historical ở một số consumer. Skill tra theo miền khi bắt đầu, handoff giữ pin | Cần audit dependency chưa khai báo, marker trong code/model và cập nhật pin sau khi duyệt ngữ nghĩa |
| CHOICE-07 | Cutover theo nguồn đã đối soát. Lượt này chuyển §2–4 của DecisionDelta và dẫn consumer sang CLI | Sau khi chốt trust boundary, xử lý D/ADR và CHOICE issue-local theo từng miền, ưu tiên ID bị trộn trạng thái |

## Cần chủ sản phẩm chốt hoặc reviewer đối soát

| Mức | Tình huống và quyết định cần ghi | Hệ quả và bằng chứng cần thêm |
| --- | --- | --- |
| Cần chủ sản phẩm chốt | **Nguồn xác nhận revision mới.** Một agent ghi proposal cập nhật `O-03`. Chọn receipt từ phiên/công cụ có danh tính PO được xác thực độc lập hoặc writer riêng với PO review đúng commit và payload | Receipt cần gắn ID, phạm vi, revision và hash, kiểm đúng actor và vô hiệu khi đổi payload. PR review cần writer khác reviewer và review còn hiệu lực với head đang xét. Thử ca thiếu, giả, stale và sai scope/payload ở nguồn đã chọn. |
| Cần chủ sản phẩm chốt | **Mức cưỡng chế.** Workflow qua AI ở checkout local hay cần chặn ghi ngoài workflow ở repo từ xa | Cách local có kiểm tra ở công cụ và khi review. Cưỡng chế từ xa cần quyền, chính sách và bằng chứng vận hành độc lập. |
| Cần chủ sản phẩm chốt khi làm auth | **O-11/O-12.** Xác định luồng khôi phục, vai trò email, ngưỡng và đơn vị độ dài mật khẩu | `Password::parse` dùng 10–128 byte UTF-8. PRD §14.6 nêu ≥10 ký tự trong đề xuất UX/dữ liệu. D-05 §12 ghi backend và ý định sản phẩm khác lớp. Ca tiếng Việt cần thử ASCII, nhiều byte và cách đếm ký tự đã chốt. |
| Cần chủ sản phẩm chốt khi mở tombstone | **O-03/04/05/08/09/10.** Quyền ghi, số phận bình luận đang chờ, xóa bài từng công khai sau khi chuyển riêng tư, purge, dữ liệu hiển thị và khôi phục | PUB-04 dùng “đang công khai”, PUB-05 và DecisionDelta dùng “từng công khai”. Model có nhánh riêng và `O-05` đang mở. Code `application::publication` có baseline `O-03`. Cần chốt trước API/UI xóa và kiểm trường hợp thay đổi audience rồi xóa, cache/excerpt, tombstone và discussion. |
| Cần chủ sản phẩm chốt khi mở tương ứng | **O-01/02/06/07/13/14/15/16.** Membership và thứ tự, neo ghi nhận qua bản sửa, giới hạn hashtag, tham số Đề xuất, quyền phản đối admin, rút bản sửa đang chờ | Các model và cột đề xuất trong PRD có phương án để thử. Xem lại cả COM-10/CHOICE-01, RANK-04/O-14 và LVE-FR-04 khi chọn rule có hiệu lực. |
| Rà provenance | DecisionDelta ghi chủ sản phẩm đã xác nhận 15 mục trong lượt 10/11. Git object đã ghim là nguồn được nhập. Chat gốc và người ký chưa được truy vết độc lập | Dùng lại trạng thái đã ghi cho snapshot này. Khi nguồn gốc độc lập có thể truy cập, đối chiếu nếu phạm vi hoặc nội dung thay đổi. |
| Rà migration | D-log trộn snapshot code với luật. Ở base của PR, D-05 và ACC-03 đầu PRD còn nói chỉ có mã email. PR đã cập nhật hai chỉ dẫn này theo backend #152. COM-10 mô tả giữ bản đang công khai, CHOICE-01 còn có chi tiết đề xuất | Tách bằng chứng lịch sử khỏi câu về hiện trạng. Review từng D/ADR và phần chi tiết có overlap. Mọi thay đổi nghĩa cần căn cứ từ owner. |
| Rà giới hạn kỹ thuật | V1 chỉ so xung đột có cấu trúc cho tập audience bài viết. Graph kiểm tồn tại/cycle cho dependency revision khai báo. Head của proposal khác revision đang tra cứu | Khi hỗ trợ thêm approval, cần kiểm hai head có quyền hiệu lực trong cùng scope, thu hồi/supersede liên ID, quyền theo từng miền, và khôi phục/lỗi khi thao tác trên FS mục tiêu. |
| Rà consumer | Marker mới có trong PRD, auth/publication spec và bản lịch sử. Dùng `refs` để liệt kê cụ thể. Code, model, prompt ngoài repo và marker bị xóa cần đối chiếu riêng | Truy vết theo ID khi chuyển thêm miền, chạy semantic review và test thực tế sau mỗi lần đổi pin. |

## Cutover và hoàn tác

Tra §2–4 tại [TOML](../../decisions/decision-delta-20261110.toml). Dẫn lịch sử về
[DecisionDelta gốc](https://github.com/loveoverflowcom/vink/blob/0c5bb275c15342607e6701f14019c07e269c2a68/docs/architecture/product-decisions-20261110.md).
D-log, ADR ngoài phạm vi, PRD và registry bằng chứng tiếp tục dùng ở các vai trò ghi trong
bảng trên. Khi có hai nguồn diễn giải lệch, ghi cả nguồn, SHA và hành vi bị ảnh hưởng để review.

Để hoàn tác lát cắt, xuất/cất các proposal đã ghi sau merge, rồi revert cùng nhau phần CLI,
checks, bundle, marker và hướng dẫn chuyển nguồn. DecisionDelta giữ nguyên các bảng gốc,
vì vậy có thể mở lại đường tra Markdown theo SHA cũ. Khi tiếp tục chuyển miền, chốt lại
ownership của từng ID, giữ link lịch sử và đối chiếu requirement/ADR trước khi đổi nguồn.
