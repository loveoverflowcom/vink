# Vink agent skills

Start with [AGENTS.md](../../AGENTS.md) and the [documentation map](../../docs/README.md).
Pick one primary skill for the task; load a second only when the slice crosses its boundary.
Do not run a full discovery ceremony for a small, clear change.

## Runtime discovery

The skill source of truth lives in `.agents/skills/<name>/SKILL.md` (with `agents/openai.yaml`
metadata for Codex and `.claude/skills/<name>` symlinks for Claude Code). All three agent runtimes
supported by this repository read the same canonical instructions:

| Runtime | Project rules | Skill discovery | Explicit invocation |
| --- | --- | --- | --- |
| **Codex** | `AGENTS.md` | `.agents/skills/<name>/SKILL.md` + `agents/openai.yaml` | `$vink-<name>` (e.g. `$vink-shape`, `$vink-work-item`) |
| **Claude Code** | `CLAUDE.md` (`@AGENTS.md`) | `.claude/skills/<name>` symlink → `../../.agents/skills/<name>` | `/vink-<name>` (e.g. `/vink-shape`, `/vink-work-item`) |
| **Antigravity** | `AGENTS.md` (must stay `< 24,000` bytes so `<user_rules>` never truncates) | `.agents/skills/<name>/SKILL.md` (single-line `description:` frontmatter) | Mention the skill name or read `.agents/skills/<name>/SKILL.md` |

## Choose the right skill

| Task | Skill |
| --- | --- |
| Clarify feature scope, detect open product rules in `O-01..O-19`, classify product vs technical choices and ask the owner | [vink-shape](vink-shape/SKILL.md) |
| Turn confirmed decisions into domain/UX/data specs, update owning docs (`prd/`, `docs/architecture/`, `docs/development/`) and reconcile doc/code drift | [vink-specify](vink-specify/SKILL.md) |
| Implement a feature, issue, bug fix or continuation | [vink-work-item](vink-work-item/SKILL.md) |
| Write or execute a self-contained Vink prompt for another coding chat | [vink-prompt](vink-prompt/SKILL.md) |
| Explain how an existing Vink flow works in code | [vink-how](vink-how/SKILL.md) |
| Investigate why a design or behavior was chosen | [vink-why](vink-why/SKILL.md) |
| Recover context from earlier chats or handoffs | [vink-recall](vink-recall/SKILL.md) |
| Compare architecture options and slice a large change | [vink-architect](vink-architect/SKILL.md) |
| Run a bounded spike or compare competing implementations | [vink-prototype](vink-prototype/SKILL.md) |
| Domain, Axum API, PostgreSQL, discussion module, worker and verification | [vink-engineering](vink-engineering/SKILL.md) |
| Revisions, audiences, moderation, publish intent, pointers and tombstone deletion | [vink-publication](vink-publication/SKILL.md) |
| Leptos SSR/hydration, routes, browser state and JS editor | [vink-leptos-web](vink-leptos-web/SKILL.md) |
| Kotlin Compose Multiplatform `:ui`/`:patterns`, gestures, insets, focus, CMP dependency evaluation, Android/iOS diagnostic hosts and Desktop previews | [vink-cmp-mobile](vink-cmp-mobile/SKILL.md) |
| Expressive M3 UX, interaction design (gesture/motion/focus/IME/reduced-motion), tokens, Vietnamese typography and accessibility | [vink-ui-design](vink-ui-design/SKILL.md) |
| Describe and compare UI layout and interaction structure before generating or editing UI | [vink-layout-spec](vink-layout-spec/SKILL.md) |
| Capture a reproducible UI/UX implementation snapshot and coverage report from a checkout | [vink-ui-capture](vink-ui-capture/SKILL.md) |
| Upload a captured UI evidence bundle to Google Drive and update the review index | [vink-ui-evidence-drive](vink-ui-evidence-drive/SKILL.md) |
| Component-by-component UI/UX and interaction audit from screenshots, source or a permitted live page | [vink-ui-inspector](vink-ui-inspector/SKILL.md) |
| Repair a scoped UI or interaction defect, or prepare an implementation handoff from findings/designs | [vink-ui-fix](vink-ui-fix/SKILL.md) |
| State-transition/race impact and focused Quint models | [vink-behavior](vink-behavior/SKILL.md) |
| Refresh the feature map or verification workflow | [vink-maintain-verification](vink-maintain-verification/SKILL.md) |
| Review a PR, commit range, patch or local diff | [vink-code-review](vink-code-review/SKILL.md) |

## Conditional routing for `vink-shape` and `vink-specify`

Do not force every request through a multi-skill pipeline. Route conditionally based on what the
task actually touches:

1. **New idea, UX flow, or change to permissions/states/data with unclear rules:**
   `vink-shape` (classify & ask 1–3 concrete product questions if needed) → `vink-specify` (update
   owning docs) → `vink-architect` / `vink-ui-design` / `vink-work-item` / `vink-prompt`.
2. **Owner just answered a product question or changed a prior decision:**
   `vink-specify` (record the Decision Record, supersession, and updated status across `prd/`,
   domain specs and `docs/development/` registries) → continue implementation or handoff.
3. **Clear bug fix, small UI repair, or bounded slice with settled rules:**
   Go directly to `vink-work-item` or `vink-ui-fix` (skip `vink-shape`). If implementation changes
   runtime status or exposes doc/code drift, run `vink-specify` before finishing.

## Conditional routing for UI and Interaction UX

Triggers: gesture/swipe, motion/transition, focus, IME/keyboard, overlay, scroll restoration, reduced motion, interaction performance.

1. **Bug rõ, luật đã chốt (ví dụ: focus SearchField, thiếu nhãn trạng thái khi dừng loop, insets bị double-pad):**
   Đi thẳng tới `vink-ui-fix` (hoặc `vink-cmp-mobile` / `vink-leptos-web` theo renderer). Không mở rộng scope sang redesign hay shaping.
2. **Thiết kế tương tác mới hoặc cử chỉ phức tạp (gesture/swipe, motion/transition, overlay/sheet, scroll restoration):**
   `vink-ui-design` (đặc tả interaction theo `references/interaction-ux.md`) → `vink-layout-spec` (trình bày spec kèm trường `Interaction`) → triển khai ở renderer tương ứng.
3. **Cử chỉ phá hủy hoặc đổi điều hướng cốt lõi (vuốt để xóa bài, vuốt bỏ nháp, vuốt đổi tab toàn màn):**
   `vink-shape` (phân loại `open_product` và hỏi chủ sản phẩm) → `vink-specify` nếu có quyết định → `vink-ui-design`.
4. **Câu hỏi tương thích thư viện / dependency CMP:**
   `vink-cmp-mobile` (áp dụng quy trình ba tầng và checklist tại `references/dependency-evaluation.md`) → `vink-prototype` nếu cần thử nghiệm độc lập.
5. **Kiểm tra / Audit tương tác (focus stealing, jank masking, reduced motion compliance):**
   `vink-ui-inspector` (quan sát và ghi nhận finding khách quan, phân biệt rõ bằng chứng thực với giả thuyết "mượt").
6. **Thu thập bằng chứng (Capture):**
   `vink-ui-capture` (ghi snapshot, metadata interaction và navigation graph; không tự đưa ra kết luận audit pass/fail).
7. **Chỉ thiếu thiết bị thật (Android/iOS device):**
   Tiếp tục hoàn tất phần độc lập trên `commonMain` và Desktop preview; ghi nhận `NOT_RUN` riêng cho boundary native host.

## Resolve a decision

For a task that depends on a decision, start with
[the decision registry](../../decisions/README.md). Query the relevant domain or ID at the
working checkout and use the PRD/ADR for detailed requirements and IDs outside the migrated
DecisionDelta slice. Keep the revision and snapshot in a handoff. An unavailable lookup needs
a sourced gap or an explicit historical citation. Route proposals for migrated IDs through
[vink-specify](vink-specify/SKILL.md). Record a signoff only for the owner's own words about that
exact revision; a proposal without one stays `PENDING` and never becomes current. A clear fix with
settled rules goes straight to its owner.

## Shared conventions

- Keep product/architecture proposals, owner product decisions (`PRODUCT_CONFIRMED` / `DEFERRED` /
  `OPEN`), architecture status (`DESIGN_PROPOSED` / `Provisional` / `Accepted`), implementation
  level (`core` / `persistence-api` / `worker-provider` / `web` / `cmp`), and verification evidence
  separate.
- Use `cargo xtask verify quick|discussion|expressions|kani|postgres|web` from
  [docs/development](../../docs/development/README.md) when verification is in scope, and
  [verification.md](vink-engineering/references/verification.md) for reporting.
- Respect user authorization already granted in the active conversation; do not ask again merely
  because a skill has a conservative default, and do not infer permission to push, publish or post
  externally from a local edit.
- Provenance for how these skills were adapted from Cantos and the coding-agent workflow draft
  lives in [skill-adaptation.md](../../docs/references/skill-adaptation.md).
