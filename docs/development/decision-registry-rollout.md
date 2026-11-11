# Rà soát và chuyển Decision Registry sang TOML

Lát cắt của [#164](https://github.com/loveoverflowcom/vink/issues/164) khảo sát
`develop@0c5bb275c15342607e6701f14019c07e269c2a68`. Lát cắt nhập lại tại
`develop@af3c34bd624be18d404bd5789620759cae0b0ddb` sau khi #165 thêm §7 quản trị kiểm duyệt. Ở commit
đó, §2–4 giống hệt bản khảo sát. Phạm vi có thể đối chiếu theo từng mục là
[DecisionDelta §2–4](../architecture/product-decisions-20261110.md#2-decisiondelta) và §7.1–7.2.
Tra cứu, ghi đề xuất và signoff đều qua [cargo xtask decision](../../decisions/README.md).
Lát cutover sổ D/ADR theo CHOICE-07 đọc `develop@73ab92f0064674833ec31bc7533b210149b26b79`, sau khi
#166 merge kho TOML. Kết quả ở [mục riêng](#lát-cutover-sổ-dadr-11112026).

**Rà soát và merge ngày 11/11/2026.** Chủ sản phẩm giao rà soát và merge PR #166 và ủy quyền chốt
các điểm còn mở theo vision. Kết quả: CHOICE-01 thành `VDR-DECISION-SIGNOFF@r1`, `O-12` thành
`O-12@r2`, đều có signoff ghi lời ủy quyền. Lượt nhập §7 đóng `O-05` và bổ sung `O-03`/`O-15`.
Xem [các điểm đã chốt](#đã-chốt-khi-rà-soát-1111) và [các điểm còn mở](#còn-mở-hoặc-cần-rà-tiếp).

## Quyền sở hữu và kiểm kê

| Nguồn tại SHA trên | ID hoặc phạm vi và nơi dùng | Xử lý trong PR | Điểm còn rà |
| --- | --- | --- | --- |
| [DecisionDelta §2](../architecture/product-decisions-20261110.md#2-decisiondelta) | `USER-01..15`, mapping bên dưới. PRD §14, vision §13, D-log, domain specs, AGENTS, skills và model đích | `migrate_decision` sang TOML. Giữ bảng gốc làm `historical_alias` và lịch sử code của lượt 10/11 | `USER-09` xác nhận bài ngắn và bộ sưu tập, cùng dòng ghi nhiều chương được hoãn (`WR-04`) |
| [DecisionDelta §3–4](../architecture/product-decisions-20261110.md#3-lựa-chọn-triển-khai-design_proposed) | Bốn `CHOICE`, `O-01..16`. Specs theo miền và model M1–M6 | `migrate_decision` với trạng thái cũ. Scope đầy đủ cho CHOICE để tránh trùng với issue khác | Baseline model và câu hỏi có thể được review theo miền khi triển khai |
| [DecisionDelta §7.1–7.2](../architecture/product-decisions-20261110.md#7-bổ-sung-11112026--quản-trị-kiểm-duyệt) | `ADM-01..10`, revision mới của `O-03/05/15`, `O-17..19`. Publication spec §5–§8, moderation core, `governance.qnt` | `migrate_decision` tại `af3c34b`, nhóm `ADMIN-MODERATION-20261111`. Revision cũ của `O-03/05/15` được đánh dấu `historical` | Tên kiểu, mã từ chối và baseline kỹ thuật của §7 vẫn là `DESIGN_PROPOSED` ở owner Markdown/core |
| [DecisionDelta §1, §5–6, §7.3](../architecture/product-decisions-20261110.md) | Giải nghĩa nhãn, UX/dữ liệu đề xuất, snapshot triển khai, kế hoạch PR, bàn giao §7 | `keep_spec`, `keep_runtime_evidence` và lịch sử | Các mục chưa có ID riêng ở §5 cần review nếu muốn chuyển thành quyết định |
| [Vision](../../prd/vision.md), [requirements](../../prd/requirements.md), [LVE PRD](../../prd/literary-endorsement.md) | Phạm vi, ID/acceptance, hàng đề xuất UX và guard nghiệp vụ | `keep_spec`. Thêm đường tra record vào §14. ID requirement vẫn có owner ở PRD | Đối chiếu lại các điều kiện dễ hiểu khác nhau trong bảng review bên dưới |
| [Sổ D/INV/S](../architecture/decisions.md), [tech stack](../architecture/tech-stack.md) | `D-01..19`, `INV-01..08`, `S-01..05`. Có hàng lẫn quyết định, phương án, lịch sử và bằng chứng | Lát cutover 11/11: `migrate_decision` cho D-02..05, D-08, D-12..15 qua nhóm `DECISION-LOG-20261111`. `historical_alias` cho D-06/07/09/10/11/16..19. `pending_review` cho D-01. `keep_spec` cho INV, §3, §5 và lộ trình. `keep_runtime_evidence` cho S-ID | Xem [bảng D-ID](#d-id-sau-cutover). D-01 và Quyết định 2 chưa có trạng thái v1 hoặc ID |
| [Ownership thảo luận](../architecture/discussion-ownership.md), [admission](../architecture/discussion-admission.md), [reads](../architecture/discussion-reads.md), [outbox](../architecture/discussion-outbox.md), [ranh giới Identity](../architecture/identity-postgres-boundaries.md), [auth authority](../architecture/auth-authority.md) | D-02/D-05/D-08/D-13, quyền tác giả và ủy quyền, `DISC-*` và `CHOICE-*` cục bộ | `migrate_decision` cho D-02-DISCUSSION, D-05, D-08, D-13, `DISC-01-CHOICE-01/06`, `DISC-02-03-CHOICE-02`, `DISC-04-CHOICE-01..04`. ADR giữ thiết kế, bằng chứng và marker `active`. `pending_review` cho C1..C5 của ADR D-05 §11. `keep_spec` cho ranh giới Identity | D-05 vẫn Provisional về authority. Lịch sử `USER-08@r1` chỉ nói phương thức đăng nhập |
| [LVE](../architecture/literary-endorsement.md) và [taxonomy](../architecture/work-taxonomy.md) | D-14, D-15, `LVE-PR-01`, `TAG-PR-01`, CHOICE cục bộ theo từng issue | `migrate_decision` cho D-14, D-15 và 15 CHOICE đã chọn trong hai ADR, ID gồm lát cắt. `pending_review` cho CHOICE-06/07 của #73 và lựa chọn #96–#99 ghi trong registry phát triển | Xem [bảng CHOICE theo issue](#choice-theo-issue). CHOICE-07 (icon) lệch giữa ADR và design |
| [Publication](../architecture/publication-moderation.md), [sửa bình luận](../architecture/comment-editing.md), [bộ sưu tập](../architecture/collections.md), [xếp hạng](../architecture/discovery-ranking.md), [hashtag](../architecture/hashtags.md) | Đặc tả đích dẫn USER/CHOICE/O; review đặc biệt `PUB-04/05`, `O-03..06`, `O-09`, `O-15/16` | `keep_spec`. Ghim mẫu cho quyền ghi tombstone. TOML sở hữu trạng thái quyết định đã nhập | Mở rộng marker sau khi xác định consumer ảnh hưởng theo từng thay đổi |
| [Design](../design/README.md), [source map UI](../../.agents/skills/vink-ui-design/references/source-map.md) | LayoutSpec, token, ảnh review và thiết kế đã chọn cho từng bề mặt | `keep_spec` và `keep_runtime_evidence`. `pending_review` cho lựa chọn thiết kế có tuổi thọ ngoài một issue | Cần rà lựa chọn theo từng màn nếu đưa quyết định thiết kế vào kho |
| [Development](README.md), [feature map](feature-map.md), [registry 10/11](product-decisions-registry.md) | Cấu trúc code, runner, hành vi đang có, implementation level và bằng chứng theo SHA | `keep_runtime_evidence`. Thêm link VDR để tra trạng thái quyết định | Những câu tóm tắt hiện trạng cũ trong PRD/D-log cần dọn trong lượt đối soát runtime |
| [References](../references/sources.md), [skill adaptation](../references/skill-adaptation.md), [archive](../archive/initial/README.md) | Nguồn Cantos/Discourse, lịch sử bộ định hướng/skill và vision gốc | `historical_alias` hoặc `excluded_with_reason`. Giữ nguyên bằng chứng, snapshot và pin | Bản nguồn ngoài repo hoặc chat gốc của 10/11 chưa được xác minh độc lập |
| [Skill index](../../.agents/skills/README.md), [AGENTS](../../AGENTS.md), [CLAUDE](../../CLAUDE.md) | Skill và reference của shape, specify, work-item, architect, why, recall, prompt, code-review, maintain-verification, behavior, publication, UI design | Cùng dùng một hướng dẫn tra cứu và luồng ghi qua specify. Giữ metadata discovery và Claude symlink | Các plugin ngoài repo và prompt đã sao chép sang issue/chat cần đối chiếu khi được dùng lại |
| [Model đích](../../services/vink-app/behavior/target/README.md), [publication application](../../services/vink-app/src/application/publication.rs), [password core](../../services/vink-identity/src/password.rs) | `USER-05/06/09/12..15`, `CHOICE-01`, `O-03`, `O-12` và một số ID trong contract/migration | `keep_spec` cho model và `keep_runtime_evidence` cho code/test. `password.rs` theo `O-12@r2` từ lượt này | Marker v1 có phạm vi Markdown. ID trong Rust, SQL, Quint, JSON và prompt cũ cần review theo miền |
| [Dispatcher](../../xtask/src/main.rs), [checks](../../xtask/src/checks.rs), [workflow](../../.github/workflows/ci.yml), [Cargo.lock](../../Cargo.lock) | CLI, kiểm skill/link/ranh giới và lane local `quick` | `excluded_with_reason` khỏi record vì đây là tooling/config. Kiểm registry qua lane hiện có | Phần cưỡng chế ngoài checkout local cần lựa chọn nguồn quyền riêng |

Kiểm kê duyệt nhóm nguồn qua danh sách file theo Git, heading/bảng và các tham chiếu
`DecisionDelta`, `PRODUCT-DECISIONS-20261110`, `O-*`, `USER-*` trong Markdown, skill
references, model và code lân cận. Đối chiếu từng hàng đầy đủ cho §2–4. Các nhóm
`pending_review` có owner và phạm vi để tiếp tục rà nội dung từng mục. Marker được kiểm
cho các consumer đã khai báo. Bảng kiểm kê ghi những nguồn nằm ngoài phạm vi marker.

## ID đã nhập

Các ID dưới đây giữ nguyên và mặc định ở `@r1`. `USER-08@r1` giữ phương thức mã email từng
được đề xuất, với trạng thái Provisional và ngữ cảnh lịch sử. `USER-08@r2` là hướng đăng nhập
chính được chốt trong DecisionDelta. Bảng dưới ghi trạng thái **hiện hành** sau lượt nhập §7 và các
signoff của lượt rà soát. Mỗi dòng TOML giữ dòng nguồn và các ID liên quan.
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
| O-03@r2, O-05@r2 | PRODUCT_CONFIRMED (§7.2) | Không ghi mới trên trang ẩn/thay thế; xóa từ trang ẩn sau khi từng công khai giữ bình luận, PUB-04/05/07, COM-11 |
| O-04/08/09/10 | OPEN | Bình luận chờ, purge, dữ liệu trang thay thế và khôi phục, PUB-03/04/05 |
| O-06/07 | OPEN | “Hữu ích” theo revision bình luận và vote dùng cho ranking, LVE-FR-04/RANK-02 |
| O-11 | OPEN | Khôi phục tài khoản, ACC-03 |
| O-12@r2 | PRODUCT_CONFIRMED (signoff ủy quyền 11/11) | Mật khẩu 10–128 ký tự sau NFKC, mỗi code point một ký tự; username giữ ACC-03 |
| O-13/14 | OPEN | Giới hạn hashtag và tham số xếp hạng, HTG-06/RANK-01/04 |
| O-15@r2 | OPEN (đóng một phần ở §7.2) | Quyết định moderator phản đối được, kết luận admin là cấp cuối; phần còn mở là phản đối quyết định trực tiếp của admin, MOD-10 |
| O-16 | OPEN | Hủy bản sửa chờ duyệt, COM-10 |
| ADM-01..05, ADM-09/10 | PRODUCT_CONFIRMED (§7.1) | Admin tổng và grant, nhận ca, can thiệp, phản đối, dừng tự động, điều tra riêng, mặc định; MOD-09..15 |
| ADM-06..08 | PRODUCT_CONFIRMED (§7.1) | Gửi duyệt theo phiên bản, trang “Nội dung đã được ẩn”, bình luận qua URL cũ; PUB-04..07, COM-11 |
| O-17/18/19 | OPEN (§7.2) | URL cũ sau khi bác bản đang đăng, thời hạn grant/điều tra, danh mục thông báo; MOD-11/13/15, PUB-05, COM-11 |
| VDR-DECISION-SIGNOFF@r1 | PRODUCT_CONFIRMED (signoff ủy quyền 11/11) | Nguồn xác nhận và mức cưỡng chế cho revision mới (CHOICE-01 của #164) |

## Lựa chọn của lát cắt và đánh đổi

| ID của #164 | Cách triển khai | Khi cần xem lại |
| --- | --- | --- |
| CHOICE-01 | **Đã chốt** thành `VDR-DECISION-SIGNOFF@r1`. Signoff trích lời chủ sản phẩm, gắn `ID@rN` và SHA-256 của payload, vào `develop` qua merge do chủ sản phẩm làm hoặc yêu cầu. Cưỡng chế là local cộng CI, kiểm toàn vẹn chứ không xác thực danh tính | Khi có nhiều người ghi repo, hoặc khi cần chặn ghi ngoài workflow bằng branch protection, chữ ký hay GitHub App |
| CHOICE-02 | Revision bất biến ở đường dẫn theo ID/revision. Bundle legacy được nhập một lần. `diff` đọc hai payload và chỉ in trường thay đổi | So sánh với file current cố định nếu PO cần xem diff dòng trực tiếp trên GitHub thường xuyên |
| CHOICE-03 | Rust/Serde và validator làm contract v1, dùng phiên bản crate TOML đã có trong Cargo.lock | Khi có editor consumer, thử corpus chung và kiểm dialect trước khi sinh schema |
| CHOICE-04 | Tra cứu và xuất từ TOML mỗi lần. Không có index/export tracked | Đo quy mô thật và xem PO có cần view dựng sẵn trên GitHub hay không |
| CHOICE-05 | Mở rộng `vink-specify` để ghi proposal, chia sẻ lệnh qua skill index | Xem lại nếu lifecycle phê duyệt sau này cần owner riêng |
| CHOICE-06 | Marker Markdown có mode active/historical ở một số consumer. Skill tra theo miền khi bắt đầu, handoff giữ pin | Cần audit dependency chưa khai báo, marker trong code/model và cập nhật pin sau khi duyệt ngữ nghĩa |
| CHOICE-07 | Cutover theo nguồn đã đối soát. Lượt đầu chuyển §2–4 của DecisionDelta. Lượt 11/11 chuyển sổ D/ADR và CHOICE theo issue bằng importer ghim riêng | Còn D-01, Quyết định 2, lựa chọn trong registry phát triển, ID trong code/model và C1..C5 của D-05 |

## Lát cutover sổ D/ADR (11/11/2026)

Nguồn ghim: `develop@73ab92f0064674833ec31bc7533b210149b26b79`. Tra theo nhóm
`cargo xtask decision query DECISION-LOG-20261111` (32 revision) hoặc theo D-ID. Bundle là
[`decision-log-20261111.toml`](../../decisions/decision-log-20261111.toml).

### Cách nhập: importer ghim riêng

| Phương án | Kết quả | Lý do |
| --- | --- | --- |
| **(a) Mở rộng importer có pin riêng** (chọn) | `xtask/src/decision/decision_log.rs` đọc sáu file tại `73ab92f`. Mỗi câu của record là một ô bảng hoặc một trích dẫn, và `render` phải tìm thấy nó nguyên văn trong khối đã khai báo. Trạng thái nguồn nằm ở `context.source_status`. Pin SHA-256 độc lập, đối chiếu bằng `migrate --dry-run DECISION-LOG-20261111` | D-ID giữ nguyên ID và trạng thái nguồn (Provisional, đề xuất, đã chốt), giống quy tắc “Revision nhập giữ trạng thái của nguồn”. Record nhập là hiện hành nên consumer có thể đặt marker `active`. Trích nguyên văn có kiểm tra là bằng chứng không đổi nghĩa khi tách luật khỏi snapshot. Importer từ chối `ACCEPTED` và trạng thái sai loại quyết định |
| (b) Proposal `PENDING` | Không chọn | Proposal mới phải mang ID `VDR-*`, nên D-ID bị đổi tên. Trạng thái nguồn mất vì proposal luôn `PENDING`, kể cả D-13 đã được chủ sản phẩm chốt. Proposal pending không làm nguồn cho marker. Signoff cần lời chủ sản phẩm cho từng revision, và lượt này không có lời đó |

Phương án (a) không đổi `VDR-DECISION-SIGNOFF@r1`, vì luật đó áp cho revision mới. Import chỉ chép
quyết định đã ghi kèm trạng thái lúc ghi. Câu trong README cũ (“Importer không nhận thêm nguồn
Markdown mới”) được thay bằng ranh giới hẹp hơn: thêm nguồn cần pin riêng, trích nguyên văn và
review riêng. Luật mới hoặc thay đổi nghĩa vẫn đi qua proposal và signoff. Lượt này không ghi
proposal hay signoff nào.

### D-ID sau cutover

| D-ID | Disposition | Authority | Phần đã tách và nơi giữ |
| --- | --- | --- | --- |
| D-01 | `pending_review` | Sổ D-ID | Format v1 chưa có trạng thái tương ứng với “Ràng buộc đầu vào”. Ghi `ACCEPTED` là nâng trạng thái, còn ghi `DESIGN_PROPOSED` là hạ |
| D-02 | `migrate_decision`, tách hai record | `D-02-CONTENT@r1` (`DESIGN_PROPOSED`), `D-02-DISCUSSION@r1` (`PRODUCT_CONFIRMED`) | Hàng gốc mang hai trạng thái nên mỗi phần giữ một trạng thái riêng. Lý do chọn A/B/C ở §1 sổ là lịch sử. Trách nhiệm tự xây ở ADR ownership (`keep_spec`) |
| D-03 | `migrate_decision` | `D-03@r1` (`DESIGN_PROPOSED`) | — |
| D-04 | `migrate_decision` | `D-04@r1` (`DESIGN_PROPOSED`, nguồn ghi “Cần spike”) | Spike S-01 (`keep_runtime_evidence`, §6 sổ) |
| D-05 | `migrate_decision` | `D-05@r1` (`PROVISIONAL`), `requires = ["USER-08@r2"]` | Hướng đăng nhập 10/11 thuộc `USER-08@r2`. Mốc code 10/11, backend #152 và đường production là `keep_runtime_evidence` ở ADR §11/§12 và runbook. Danh sách blocker vào `context.review_when` |
| D-06 | `historical_alias` | `USER-02@r1` | “Đề xuất pilot” và nhóm kín/unlisted để sau đã bị USER-02 thay. “Runtime chưa có” là `keep_runtime_evidence` |
| D-07 | `historical_alias` | `USER-03@r1`, `USER-05@r1` | “Pilot có người duyệt” là lịch sử. MOD-06 là `keep_spec` ở PRD. “Không provider thật” là `keep_runtime_evidence` |
| D-08 | `migrate_decision` | `D-08@r1` (`DESIGN_PROPOSED`, “Cần spike”), `requires` USER-04/USER-15 | Hướng 10/11 thuộc USER-04/15. `premoderation.v1` và chặn sửa trong code là `keep_runtime_evidence`. Phần #120 DISC-01 là `DISC-01-CHOICE-01@r1` |
| D-09 | `historical_alias` | `USER-10@r1` | — |
| D-10 | `historical_alias` | `USER-09@r1` | — |
| D-11 | `historical_alias` | `USER-01@r1` | — |
| D-12 | `migrate_decision` | `D-12@r1` (`DESIGN_PROPOSED`, “Cần chốt policy + spike”), `requires` USER-07, ADM-07, ADM-08 | Câu “Còn `OPEN`: retention/purge, quyền ghi trên tombstone” không nhập: quyền ghi đã chốt ở `O-03@r2`, còn purge ở `O-08`. Mặc định thu hồi ở §5 sổ là `keep_spec` |
| D-13 | `migrate_decision` | `D-13@r1` (`PRODUCT_CONFIRMED`) | — |
| D-14 | `migrate_decision` | `D-14@r1` (`DESIGN_PROPOSED`) cùng các LVE CHOICE | Mô tả store/HTTP #81 nằm ở `LVE-PR-02-03-CHOICE-*`. Đầu vào xếp hạng 10/11 thuộc USER-12/13, O-06/07. Câu “Ranking/icon/UI còn OPEN” không nhập vì CHOICE-07 cần rà |
| D-15 | `migrate_decision` | `D-15@r1` (`DESIGN_PROPOSED`) cùng `TAG-PR-01-CHOICE-01..03` | Lựa chọn #96–#99 là `pending_review`. Câu hashtag tách khỏi catalog thuộc USER-11 |
| D-16 | `historical_alias` | `USER-09@r1`, `O-01`, `O-02` | Sổ ghi O-01/02 là `DESIGN_PROPOSED`, còn kho ghi `OPEN` từ lượt nhập 10/11. Giữ kho |
| D-17 | `historical_alias` | `USER-11@r1`, `PRODUCT-DECISIONS-20261110-CHOICE-04` | — |
| D-18 | `historical_alias` | `USER-12@r1`, `USER-13@r1`, CHOICE-02/03 của DecisionDelta, `O-14` | — |
| D-19 | `historical_alias` | `USER-06@r1`, `USER-14@r1`, `ADM-01..10` | Core thuần và model là `keep_runtime_evidence` |

Phần khác của sổ: Quyết định 2 ở §1 chưa có ID (`pending_review`). Các trục §3, máy trạng thái,
guard và INV-01..08 là `keep_spec`. §5 thu hồi là `keep_spec`. S-01..05 ở §6 là
`keep_runtime_evidence`. Lộ trình §7 là `keep_spec`. Danh sách tài liệu §8 là `excluded_with_reason`
vì đó là kế hoạch viết tài liệu.

### CHOICE theo issue

ID có phạm vi theo dạng `<họ>-<lát>-CHOICE-<số>`. Số giữ theo bảng của owner. Lát gộp ghi đủ hai số.
Các record nhập ở `DESIGN_PROPOSED`: lựa chọn của một lát không được nâng thành Accepted.

| Issue và lát (owner) | ID có phạm vi | Disposition |
| --- | --- | --- |
| #73 LVE-PR-01 ([ADR](../architecture/literary-endorsement.md)) | `LVE-PR-01-CHOICE-01`, `-02`, `-03`, `-08`, `-09`, `-11` | `migrate_decision` |
| #73 lát API + PostgreSQL #81 (LVE-PR-02/03) | `LVE-PR-02-03-CHOICE-03` (HTTP), `-04`, `-05`, `-12`, `-13` | `migrate_decision` |
| #73 lát Web adapter WP-04 | `LVE-WP-04-CHOICE-10` | `migrate_decision` |
| #73 | CHOICE-06 (ranking) | `pending_review`: chưa chọn. Hướng xếp hạng thuộc USER-12 và O-07 |
| #73 | CHOICE-07 (icon) | `pending_review`: ADR ghi OPEN, còn [Pen Flight R2](../design/pen-flight-r2.md) ghi “CHOICE-07 A · Mềm”. Cần chọn owner trước khi nhập |
| #95 TAG-PR-01 ([ADR](../architecture/work-taxonomy.md)) | `TAG-PR-01-CHOICE-01..03` | `migrate_decision` |
| #96 TAG-PR-02 ([storage](taxonomy-postgres.md)) | `TAG-PR-02-CHOICE-01`, `-02` | `pending_review`: ghi cạnh bằng chứng trong registry phát triển |
| #97 TAG-PR-03 ([API](taxonomy-api.md)) | `TAG-PR-03-CHOICE-01..03` | `pending_review` |
| #98 TAG-PR-04 ([discovery](discovery-api.md)) | `TAG-PR-04-CHOICE-01..03` | `pending_review` |
| #99 TAG-PR-05 ([editor](taxonomy-editor.md)) | `TAG-PR-05-CHOICE-01`, `-02` | `pending_review` (bố cục và ranh giới UI) |
| #120 DISC-01 ([admission](../architecture/discussion-admission.md)) | `DISC-01-CHOICE-01`, `-06` | `migrate_decision` |
| #120 DISC-01, hàng chuyển tiếp | CHOICE-02..05 của epic | `historical_alias`: CHOICE-02 thành `DISC-02-03-CHOICE-02`, CHOICE-05 thành `DISC-04-CHOICE-02`. CHOICE-03/04 là `pending_review` vì DISC-05 chưa làm |
| #120 DISC-02/03 phần 2 ([reads](../architecture/discussion-reads.md)) | `DISC-02-03-CHOICE-02` | `migrate_decision`. Quyết định phụ ở §2 là `keep_spec` |
| #120 DISC-04 ([outbox](../architecture/discussion-outbox.md)) | `DISC-04-CHOICE-01..04` | `migrate_decision`, đánh số theo prompt DISC-04 |
| D-05 đường production ([ADR §11](../architecture/auth-authority.md#11-đường-production-10112026)) | C1..C5 | `pending_review`: chuyển cùng lượt rà D-05 trước Accepted |
| [Ranh giới Identity](../architecture/identity-postgres-boundaries.md) | Quyết định phạm vi của PR | `keep_spec`. Đây không phải lựa chọn auth authority |

### Consumer và kiểm tra

Marker `active` có ở sổ D-ID (10 record), ADR auth, ownership, admission, reads, outbox, literary
endorsement, taxonomy và đặc tả publication. `cargo xtask decision refs` liệt kê 47 marker.
Test `decision_log_import_keeps_source_status_and_one_authority_per_d_id` kiểm trạng thái, phần tách
D-02/D-05 và các D-ID `historical_alias`. Test `excerpts_must_be_verbatim_in_their_blocks` kiểm
trích dẫn sai, nhãn ngoài tiêu đề, anchor trùng và trạng thái bị nâng. Test
`pinned_imports_render_byte_identical_when_their_sources_are_available` so cả hai bundle với Git object
ghim khi checkout có object đó. CI dùng checkout nông nên dựa vào pin SHA-256.

## Đã chốt khi rà soát 11/11

Nguồn: chủ sản phẩm ủy quyền trong phiên rà soát PR #166 (“những chỗ cần chốt thì tự suy luận theo
vision mà làm”). Agent suy luận nội dung theo vision và ghi bằng `propose` cùng `signoff`. Chủ sản
phẩm có thể thay bằng revision mới.

| Điểm | Kết quả | Căn cứ theo vision và bằng chứng |
| --- | --- | --- |
| Nguồn xác nhận và mức cưỡng chế (CHOICE-01) | `VDR-DECISION-SIGNOFF@r1`: proposal luôn `PENDING`. Signoff trích lời chủ sản phẩm, gắn hash payload, có thể là `REJECTED`. Cưỡng chế ở CLI và `verify quick` trên CI | Kết quả AI không mang thẩm quyền (MOD-01..15), nên agent không tự chốt. Chủ sản phẩm làm việc qua agent bằng cùng tài khoản GitHub, nên phương án writer tách biệt không vận hành được. Test `signoff_binds_the_exact_payload_and_only_then_changes_current` kiểm hash sai, signoff cũ, revision nhập, outcome sai loại, trường tự khai, trùng signoff, payload bị sửa và rejection |
| `O-12`: đơn vị độ dài mật khẩu | `O-12@r2`: chuẩn hóa NFKC, 10–128 ký tự, mỗi code point một ký tự. Dữ liệu nhập vượt 4096 byte bị từ chối trước khi chuẩn hóa. `Password::parse` đã đổi theo | Tiếng Việt và IME là ưu tiên: đếm byte làm chữ có dấu tính 2–3 lần, và bộ gõ dựng sẵn/tổ hợp cho hash khác nhau. NIST SP 800-63B tính mỗi code point là một ký tự. Chưa có tài khoản production. Test `password_length_counts_characters_after_nfkc` |
| `O-05`: xóa bài từng công khai rồi chuyển riêng tư | Đã đóng ở DecisionDelta §7.2 (#165), nhập thành `O-05@r2`. Xóa từ trang ẩn sau khi từng công khai để lại trang thay thế và giữ bình luận. Bài chưa từng công khai không để lại gì. PUB-04 có ghi chú tại chỗ | Brief của chủ sản phẩm ngày 11/11 (ADM-07/08, PUB-07, COM-11). Core `WorkAccessPlan::AuthorHidden` và `tombstone.qnt` theo #165 |

## Còn mở hoặc cần rà tiếp

| Mức | Tình huống và quyết định cần ghi | Hệ quả và bằng chứng cần thêm |
| --- | --- | --- |
| Cần chủ sản phẩm chốt khi làm auth | **O-11.** Luồng khôi phục và vai trò email | Đề xuất trong PRD là email đã xác minh làm kênh khôi phục tùy chọn. Cần chốt trước form Web username/mật khẩu |
| Cần chủ sản phẩm chốt khi mở tombstone | **O-04/08/09/10, O-17.** Số phận bình luận đang chờ, purge, dữ liệu hiển thị, khôi phục, URL cũ sau khi bác bản đang đăng | Model và core đã có baseline. Cần chốt trước API/UI xóa và kiểm cache/excerpt, tombstone và discussion |
| Cần chủ sản phẩm chốt khi mở tương ứng | **O-01/02/06/07/13/14/15/16/18/19.** Membership và thứ tự, neo “Hữu ích” qua bản sửa, giới hạn hashtag, tham số Đề xuất, phản đối quyết định trực tiếp của admin, rút bản sửa đang chờ, thời hạn grant/điều tra, danh mục thông báo | Các model và cột đề xuất trong PRD có phương án để thử. Xem lại cả COM-10/CHOICE-01, RANK-04/O-14 và LVE-FR-04 khi chọn rule có hiệu lực |
| Rà provenance | DecisionDelta ghi chủ sản phẩm đã xác nhận 15 mục ngày 10/11 và brief §7 ngày 11/11. Git object đã ghim là nguồn được nhập. Chat gốc và người ký chưa được truy vết độc lập | Dùng lại trạng thái đã ghi cho snapshot này. Khi nguồn gốc độc lập có thể truy cập, đối chiếu nếu phạm vi hoặc nội dung thay đổi |
| Rà migration | Lát cutover 11/11 đã chuyển sổ D/ADR và `CHOICE` theo issue ([kết quả](#lát-cutover-sổ-dadr-11112026)). Còn `pending_review`: D-01, Quyết định 2 của sổ, CHOICE-06/07 của #73, lựa chọn #96–#99 trong registry phát triển, C1..C5 của ADR D-05 và ID trong Rust/SQL/Quint/JSON | Ưu tiên CHOICE-07 (icon) vì ADR và design ghi khác nhau. Mọi thay đổi nghĩa cần proposal và signoff. Đổi trạng thái D-01 cần chọn cách ghi “ràng buộc đầu vào” |
| Rà giới hạn kỹ thuật | V1 chỉ so xung đột có cấu trúc cho tập audience bài viết. Graph kiểm tồn tại/cycle cho dependency revision khai báo. Chưa có quan hệ `supersedes` giữa các ID: ADM-04 sửa luật của `USER-06`/`O-15` nhưng chỉ nối qua `MOD-10` | Khi cần, thêm quan hệ thay thế liên ID và kiểm xung đột liên miền. Trước đó, tra theo requirement ID (`query MOD-10`) để thấy mọi record hiện hành liên quan |
| Rà consumer | Marker có trong PRD, auth/publication spec và bản lịch sử. Dùng `refs` để liệt kê cụ thể. Code, model, prompt ngoài repo và marker bị xóa cần đối chiếu riêng | Truy vết theo ID khi chuyển thêm miền, chạy semantic review và test thực tế sau mỗi lần đổi pin |

## Cutover và hoàn tác

Tra §2–4 và §7.1–7.2 tại [TOML](../../decisions/decision-delta-20261110.toml) và các revision sau
trong `decisions/proposals/` cùng `decisions/signoffs/`. Dẫn lịch sử về
[DecisionDelta gốc](https://github.com/loveoverflowcom/vink/blob/af3c34bd624be18d404bd5789620759cae0b0ddb/docs/architecture/product-decisions-20261110.md).
D-log, ADR ngoài phạm vi, PRD và registry bằng chứng tiếp tục dùng ở các vai trò ghi trong
bảng trên. Khi có hai nguồn diễn giải lệch, ghi cả nguồn, SHA và hành vi bị ảnh hưởng để review.

Để hoàn tác lát cắt, xuất/cất các proposal và signoff đã ghi sau merge, rồi revert cùng nhau phần
CLI, checks, bundle, marker và hướng dẫn chuyển nguồn. Để hoàn tác riêng lát cutover sổ D/ADR, revert
cùng nhau `decision_log.rs`, bundle `decision-log-20261111.toml`, marker trong ADR và bảng §2 của sổ.
Sổ tại `73ab92f` giữ nguyên văn bảng cũ. Đổi luật mật khẩu `O-12@r2` nằm trong
`password.rs` và revert riêng được. DecisionDelta giữ nguyên các bảng gốc,
vì vậy có thể mở lại đường tra Markdown theo SHA cũ. Khi tiếp tục chuyển miền, chốt lại
ownership của từng ID, giữ link lịch sử và đối chiếu requirement/ADR trước khi đổi nguồn.
