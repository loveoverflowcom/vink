// ADMIN-MODERATION-20261111, M4 — canonical moderation governance model with pure-core replay.
// Admins and moderators, grants and groups, claimed review cases, appeals, automation pause and
// private investigation (MOD-09..15, PUB-06 closure of a work's reviews). No runtime claim: no
// table, worker, API or UI exists; see README.md.
//
// Principals: 1 = author of work 1 and also an admin (recusal), 2 and 3 = moderators,
// 4 = admin, 5 = comment writer. Targets: 1 and 4 = revisions of work 1 (author 1),
// 2 = a comment of 5 on work 1, 3 = a revision of work 2 written by moderator 2.
// ASSUMPTIONS, not proofs: one serialized commit per action, and that commit re-reads the
// directory (grants, groups, admins), the case, the decision and the appeals; actors are who they
// say they are (auth is D-05); time is a bounded tick; reasons are always supplied where the core
// requires one (the core's own tests cover missing reasons).
module governance {
  type Grant = { holder: int, group: bool, cap: str, scope: int, expires: int }
  type Target = { cap: str, work: int, owner: int, workOwner: int }
  type Decision = { version: int, verdict: str, by: str }
  type Job = { target: int, state: str }
  type Case = { target: int, state: str, claimant: int, basis: int, version: int, fence: int, notes: int }
  type Appeal = { target: int, decision: int, by: str, state: str, notice: str }
  type Investigation = { admin: int, rev: int, expires: int, state: str }
  type Command = { op: str, actor: int, a: int, b: int, value: str }
  type Snapshot = {
    granted: Set[int], members: Set[int], now: int, automation: str, workPublic: bool,
    decisions: int -> Decision, jobs: int -> Job, cases: int -> Case, appeals: int -> Appeal,
    investigations: int -> Investigation, reads: Set[(int, int)],
  }

  pure val admins = Set(1, 4)
  pure val group = 7
  pure val maxTick = 6
  // g1: 2 reviews works everywhere; g2: 3 reviews work 1 until tick 3; g3: group 7 reviews
  // comments everywhere; g4: 3 reviews works everywhere; g5: 2's inherited comment grant.
  pure val grantTable: int -> Grant = Map(
    1 -> { holder: 2, group: false, cap: "works", scope: 0, expires: 0 },
    2 -> { holder: 3, group: false, cap: "works", scope: 1, expires: 3 },
    3 -> { holder: group, group: true, cap: "comments", scope: 0, expires: 0 },
    4 -> { holder: 3, group: false, cap: "works", scope: 0, expires: 0 },
    5 -> { holder: 2, group: false, cap: "comments", scope: 0, expires: 0 },
  )
  pure val targets: int -> Target = Map(
    1 -> { cap: "works", work: 1, owner: 1, workOwner: 1 },
    2 -> { cap: "comments", work: 1, owner: 5, workOwner: 1 },
    3 -> { cap: "works", work: 2, owner: 2, workOwner: 2 },
    4 -> { cap: "works", work: 1, owner: 1, workOwner: 1 },
  )
  // inv 1: admin 4 on revision 1 until tick 3; 2: the author-admin; 3: a never-public revision;
  // 4: a moderator; 5: admin 4 on revision 1 without expiry.
  pure val invTable: int -> Investigation = Map(
    1 -> { admin: 4, rev: 1, expires: 3, state: "none" },
    2 -> { admin: 1, rev: 1, expires: 0, state: "none" },
    3 -> { admin: 4, rev: 2, expires: 0, state: "none" },
    4 -> { admin: 2, rev: 1, expires: 0, state: "none" },
    5 -> { admin: 4, rev: 1, expires: 0, state: "none" },
  )
  pure val everPublic = Set(1)
  pure val workRevisions = Set(1, 4)

  pure def cmd(op: str, actor: int, a: int, b: int, value: str): Command =
    { op: op, actor: actor, a: a, b: b, value: value }

  var granted: Set[int]
  var members: Set[int]
  var now: int
  var automation: str
  var workPublic: bool
  var decisions: int -> Decision
  var jobs: int -> Job
  var cases: int -> Case
  var appeals: int -> Appeal
  var investigations: int -> Investigation
  var reads: Set[(int, int)]
  var command: Command
  var outcome: str
  var previous: Snapshot
  var cursor: int

  val current: Snapshot = {
    granted: granted, members: members, now: now, automation: automation, workPublic: workPublic,
    decisions: decisions, jobs: jobs, cases: cases, appeals: appeals,
    investigations: investigations, reads: reads,
  }
  pure val start: Snapshot = {
    granted: Set(), members: Set(), now: 0, automation: "enabled", workPublic: true,
    decisions: targets.keys().mapBy(t => { version: 1, verdict: "none", by: "none" }),
    jobs: Map(), cases: Map(), appeals: Map(), investigations: Map(), reads: Set(),
  }

  action init = all {
    granted' = start.granted, members' = start.members, now' = start.now,
    automation' = start.automation, workPublic' = start.workPublic, decisions' = start.decisions,
    jobs' = start.jobs, cases' = start.cases, appeals' = start.appeals,
    investigations' = start.investigations, reads' = start.reads,
    command' = cmd("init", 0, 0, 0, "none"), outcome' = "init", previous' = start, cursor' = 0,
  }

  action commit(c: Command, result: str, after: Snapshot): bool = all {
    granted' = after.granted, members' = after.members, now' = after.now,
    automation' = after.automation, workPublic' = after.workPublic, decisions' = after.decisions,
    jobs' = after.jobs, cases' = after.cases, appeals' = after.appeals,
    investigations' = after.investigations, reads' = after.reads,
    command' = c, outcome' = result, previous' = current,
    cursor' = if (cursor < 80) cursor + 1 else cursor,
  }
  action refuse(c: Command, result: str): bool = commit(c, result, current)

  // The authority rule, re-read at each command (MOD-11): an admin seat or a live grant of the
  // right capability and scope held directly or through current group membership.
  pure def bases(s: Snapshot, p: int, t: int): Set[int] = {
    val tg = targets.get(t)
    val viaGrants = s.granted.filter(g => {
      val spec = grantTable.get(g)
      spec.cap == tg.cap and (spec.scope == 0 or spec.scope == tg.work) and
      (spec.expires == 0 or s.now < spec.expires) and
      (if (spec.group) s.members.contains(p) else spec.holder == p)
    })
    if (admins.contains(p)) viaGrants.union(Set(0)) else viaGrants
  }
  pure def recused(p: int, t: int): bool =
    targets.get(t).owner == p or targets.get(t).workOwner == p
  // "ok" | "not_authorized" | "recused"
  pure def authority(s: Snapshot, p: int, t: int): str =
    if (bases(s, p, t) == Set()) "not_authorized" else if (recused(p, t)) "recused" else "ok"
  pure def pick(s: Snapshot, p: int, t: int, prefer: int): int = {
    val b = bases(s, p, t)
    if (b.contains(prefer)) prefer
    else if (b.contains(0)) 0
    else b.fold(99, (m, g) => if (g < m) g else m)
  }
  pure def withdrawnTarget(s: Snapshot, t: int): bool = workRevisions.contains(t) and not(s.workPublic)

  pure def openCase(s: Snapshot, t: int, claimant: int, basis: int): Snapshot = {
    val id = s.cases.keys().size() + 1
    s.with("cases", s.cases.put(id, {
      target: t, state: if (claimant == 0) "queued" else "claimed", claimant: claimant,
      basis: basis, version: if (claimant == 0) 1 else 2, fence: s.decisions.get(t).version, notes: 0,
    }))
  }
  pure def hasOpenCase(s: Snapshot, t: int): bool = s.cases.keys().exists(k =>
    s.cases.get(k).target == t and Set("queued", "claimed", "escalated").contains(s.cases.get(k).state))
  pure def decide(s: Snapshot, t: int, verdict: str, by: str): Snapshot = {
    val d = s.decisions.get(t)
    s.with("decisions", s.decisions.set(t, { version: d.version + 1, verdict: verdict, by: by }))
  }

  // ---- directory (admin only) ----
  action grantCmd(c: Command): bool =
    if (not(admins.contains(c.actor))) refuse(c, "not_admin")
    else if (granted.contains(c.a)) refuse(c, "unchanged")
    else commit(c, "granted", current.with("granted", granted.union(Set(c.a))))
  action revokeCmd(c: Command): bool =
    if (not(admins.contains(c.actor))) refuse(c, "not_admin")
    else if (not(granted.contains(c.a))) refuse(c, "unchanged")
    else commit(c, "revoked", current.with("granted", granted.exclude(Set(c.a))))
  action membershipCmd(c: Command, join: bool): bool =
    if (not(admins.contains(c.actor))) refuse(c, "not_admin")
    else if (members.contains(c.a) == join) refuse(c, "unchanged")
    else commit(c, if (join) "joined" else "left",
      current.with("members", if (join) members.union(Set(c.a)) else members.exclude(Set(c.a))))
  action tickCmd(c: Command): bool =
    if (now >= maxTick) refuse(c, "limit") else commit(c, "ticked", current.with("now", now + 1))

  // ---- submission, evaluator and automation (MOD-08, MOD-14, PUB-06) ----
  action submitCmd(c: Command): bool = {
    val job = jobs.keys().size() + 1
    if (decisions.get(c.a).verdict != "none") refuse(c, "not_needed")
    else if (withdrawnTarget(current, c.a)) refuse(c, "work_private")
    else if (automation == "paused")
      commit(c, "queued", openCase(current
        .with("decisions", decisions.set(c.a, { version: 1, verdict: "pending", by: "none" })), c.a, 0, 0))
    else commit(c, "submitted_ai", current
      .with("decisions", decisions.set(c.a, { version: 1, verdict: "pending", by: "none" }))
      .with("jobs", jobs.put(job, { target: c.a, state: "open" })))
  }
  // `pauseGuard` is the mutation switch: without it a result applies while paused.
  action aiCmd(c: Command, pauseGuard: bool): bool = {
    val known = jobs.keys().contains(c.a)
    val job = if (known) jobs.get(c.a) else { target: 1, state: "none" }
    val d = decisions.get(job.target)
    val applied = current.with("jobs", jobs.set(c.a, job.with("state", "applied")))
    if (not(known)) refuse(c, "unknown_job")
    else if (job.state == "withdrawn") refuse(c, "withdrawn")
    else if (pauseGuard and (job.state == "rerouted" or automation == "paused")) refuse(c, "automation_paused")
    else if (job.state != "open" and job.state != "rerouted") refuse(c, "duplicate")
    else if (Set("moderator", "admin").contains(d.by)) refuse(c, "overridden")
    else if (d.verdict != "pending") refuse(c, "superseded")
    else if (c.value == "uncertain") {
      val flagged = decide(applied, job.target, "needs_human", "ai")
      commit(c, "needs_human", if (hasOpenCase(flagged, job.target)) flagged else openCase(flagged, job.target, 0, 0))
    }
    else commit(c, "applied",
      decide(applied, job.target, if (c.value == "approve") "approved" else "rejected", "ai"))
  }
  action pauseCmd(c: Command): bool =
    if (not(admins.contains(c.actor))) refuse(c, "not_admin")
    else if (automation == "paused") refuse(c, "unchanged")
    else {
      val open = jobs.keys().filter(j => jobs.get(j).state == "open")
      val rerouted = current.with("automation", "paused").with("jobs", jobs.keys().mapBy(j =>
        if (open.contains(j)) jobs.get(j).with("state", "rerouted") else jobs.get(j)))
      // One queued case per rerouted target (targets are few; open them in target order).
      // Only targets still waiting for a decision need a person; a decided one keeps its decision.
      val targetsOpen = open.map(j => jobs.get(j).target).filter(t => decisions.get(t).verdict == "pending")
      val withCases = 1.to(4).fold(rerouted, (s, t) =>
        if (targetsOpen.contains(t) and not(hasOpenCase(s, t))) openCase(s, t, 0, 0) else s)
      commit(c, "paused", withCases)
    }
  action resumeCmd(c: Command): bool =
    if (not(admins.contains(c.actor))) refuse(c, "not_admin")
    else if (automation == "enabled") refuse(c, "unchanged")
    else commit(c, "resumed", current.with("automation", "enabled"))

  // The author makes work 1 private: its revisions' jobs, cases and appeals close (PUB-06).
  // Comment cases go on: the hidden page still processes submitted comments (COM-11).
  action makePrivateCmd(c: Command): bool =
    if (not(workPublic)) refuse(c, "unchanged")
    else commit(c, "made_private", current.with("workPublic", false)
      .with("jobs", jobs.keys().mapBy(j => if (workRevisions.contains(jobs.get(j).target) and
        Set("open", "rerouted").contains(jobs.get(j).state)) jobs.get(j).with("state", "withdrawn") else jobs.get(j)))
      .with("cases", cases.keys().mapBy(k => if (workRevisions.contains(cases.get(k).target) and
        Set("queued", "claimed", "escalated").contains(cases.get(k).state))
        cases.get(k).with("state", "closed").with("claimant", 0).with("basis", 0).with("version", cases.get(k).version + 1)
        else cases.get(k)))
      .with("appeals", appeals.keys().mapBy(k => if (workRevisions.contains(appeals.get(k).target) and
        appeals.get(k).state == "open") appeals.get(k).with("state", "closed") else appeals.get(k))))
  action makePublicCmd(c: Command): bool =
    if (workPublic) refuse(c, "unchanged") else commit(c, "made_public", current.with("workPublic", true))

  // ---- cases (MOD-12) ----
  pure def caseOf(s: Snapshot, k: int): Case =
    if (s.cases.keys().contains(k)) s.cases.get(k)
    else { target: 1, state: "none", claimant: 0, basis: 0, version: 0, fence: 0, notes: 0 }
  pure def bump(s: Snapshot, k: int, x: Case): Snapshot =
    s.with("cases", s.cases.set(k, x.with("version", x.version + 1)))
  // Common fence of every case command; "" = passes.
  pure def fence(s: Snapshot, k: int, expected: int, casGuard: bool): str = {
    val x = caseOf(s, k)
    if (x.state == "none") "unknown_case"
    else if (Set("concluded", "closed").contains(x.state)) "case_closed"
    else if (casGuard and expected != x.version) "stale_case"
    else ""
  }
  // `casGuard` is the mutation switch: without it a second claim takes a held case.
  action claimCmd(c: Command, casGuard: bool): bool = {
    val x = caseOf(current, c.a)
    val f = fence(current, c.a, c.b, casGuard)
    val auth = authority(current, c.actor, x.target)
    if (f != "") refuse(c, f)
    else if (x.state == "escalated") refuse(c, "escalated_to_admin")
    else if (casGuard and x.state == "claimed") refuse(c, "already_claimed")
    else if (auth != "ok") refuse(c, auth)
    else commit(c, "claimed", bump(current, c.a, x.with("state", "claimed").with("claimant", c.actor)
      .with("basis", pick(current, c.actor, x.target, -1))))
  }
  action releaseCmd(c: Command): bool = {
    val x = caseOf(current, c.a)
    val f = fence(current, c.a, c.b, true)
    if (f != "") refuse(c, f)
    else if (x.state != "claimed" or x.claimant != c.actor) refuse(c, "not_claimant")
    else commit(c, "released", bump(current, c.a, x.with("state", "queued").with("claimant", 0).with("basis", 0)))
  }
  action noteCmd(c: Command): bool = {
    val x = caseOf(current, c.a)
    val f = fence(current, c.a, c.b, true)
    val auth = authority(current, c.actor, x.target)
    if (f != "") refuse(c, f)
    else if (auth != "ok") refuse(c, auth)
    else if (not(x.state == "claimed" and x.claimant == c.actor) and not(admins.contains(c.actor)))
      refuse(c, "not_claimant")
    else commit(c, "noted", bump(current, c.a, x.with("notes", x.notes + 1)))
  }
  action escalateCmd(c: Command): bool = {
    val x = caseOf(current, c.a)
    val f = fence(current, c.a, c.b, true)
    if (f != "") refuse(c, f)
    else if (x.state != "claimed" or x.claimant != c.actor) refuse(c, "not_claimant")
    else commit(c, "escalated", bump(current, c.a, x.with("state", "escalated").with("claimant", 0).with("basis", 0)))
  }
  action takeoverCmd(c: Command): bool = {
    val x = caseOf(current, c.a)
    val f = fence(current, c.a, c.b, true)
    val auth = authority(current, c.actor, x.target)
    if (f != "") refuse(c, f)
    else if (not(admins.contains(c.actor))) refuse(c, "not_admin")
    else if (auth != "ok") refuse(c, auth)
    else if (x.state == "claimed" and x.claimant == c.actor) refuse(c, "already_claimed")
    else commit(c, "taken_over", bump(current, c.a, x.with("state", "claimed").with("claimant", c.actor).with("basis", 0)))
  }
  // A reviewer opens a case on a target and holds it (intervention on AI or on a public revision).
  action interveneCmd(c: Command): bool = {
    val auth = authority(current, c.actor, c.a)
    if (auth != "ok") refuse(c, auth)
    else if (decisions.get(c.a).verdict == "none") refuse(c, "not_submitted")
    else if (withdrawnTarget(current, c.a)) refuse(c, "case_closed")
    else if (hasOpenCase(current, c.a)) refuse(c, "case_open")
    else commit(c, "intervened", openCase(current, c.a, c.actor, pick(current, c.actor, c.a, -1)))
  }
  action revalidateCmd(c: Command): bool = {
    val x = caseOf(current, c.a)
    if (x.state != "claimed") refuse(c, "not_claimant")
    else if (authority(current, x.claimant, x.target) != "ok")
      commit(c, "authority_lost", bump(current, c.a, x.with("state", "queued").with("claimant", 0).with("basis", 0)))
    else if (bases(current, x.claimant, x.target).contains(x.basis)) refuse(c, "authority_kept")
    else commit(c, "basis_changed", bump(current, c.a, x.with("basis", pick(current, x.claimant, x.target, x.basis))))
  }
  pure def appealOpenOn(s: Snapshot, t: int, version: int): bool = s.appeals.keys().exists(k =>
    s.appeals.get(k).target == t and s.appeals.get(k).decision == version and s.appeals.get(k).state == "open")
  // `authorityGuard` is the mutation switch: without it a claimant who lost every grant concludes.
  action concludeCmd(c: Command, authorityGuard: bool): bool = {
    val x = caseOf(current, c.a)
    val f = fence(current, c.a, c.b, true)
    val auth = authority(current, c.actor, x.target)
    val d = decisions.get(x.target)
    val basis = pick(current, c.actor, x.target, x.basis)
    if (f != "") refuse(c, f)
    else if (x.state != "claimed" or x.claimant != c.actor) refuse(c, "not_claimant")
    else if (authorityGuard and auth != "ok") refuse(c, auth)
    else if (d.version != x.fence) refuse(c, "stale_decision")
    else if (appealOpenOn(current, x.target, d.version) and not(admins.contains(c.actor))) refuse(c, "appeal_open")
    else commit(c, "concluded", decide(bump(current, c.a, x.with("state", "concluded").with("basis", basis)), x.target,
      if (c.value == "approve") "approved" else "rejected", if (basis == 0) "admin" else "moderator"))
  }

  // ---- appeals (MOD-10) ----
  // `dedupGuard` is the mutation switch: without it a retry opens a second appeal.
  action appealCmd(c: Command, dedupGuard: bool): bool = {
    val t = c.a
    val d = decisions.get(t)
    val id = appeals.keys().size() + 1
    val existing = appeals.keys().exists(k => appeals.get(k).target == t and appeals.get(k).decision == c.b)
    if (targets.get(t).owner != c.actor) refuse(c, "not_found")
    else if (withdrawnTarget(current, t)) refuse(c, "withdrawn")
    else if (dedupGuard and existing) refuse(c, "appeal_replayed")
    else if (c.b != d.version) refuse(c, "stale_decision")
    else if (d.by == "admin") refuse(c, "final_decision")
    else if (d.verdict != "rejected") refuse(c, "nothing_to_appeal")
    else commit(c, "appeal_recorded", current.with("appeals", appeals.put(id,
      { target: t, decision: d.version, by: d.by, state: "open", notice: "not_sent" })))
  }
  action appealConcludeCmd(c: Command): bool = {
    val known = appeals.keys().contains(c.a)
    val ap = if (known) appeals.get(c.a) else { target: 1, decision: 0, by: "none", state: "none", notice: "not_sent" }
    val d = decisions.get(ap.target)
    val verdict = if (c.value == "uphold") d.verdict else if (d.verdict == "rejected") "approved" else "rejected"
    if (not(admins.contains(c.actor))) refuse(c, "not_admin")
    else if (not(known)) refuse(c, "unknown_appeal")
    else if (ap.state != "open") refuse(c, "already_resolved")
    else if (withdrawnTarget(current, ap.target)) refuse(c, "withdrawn")
    else if (recused(c.actor, ap.target)) refuse(c, "recused")
    else if (ap.decision != d.version) refuse(c, "stale_decision")
    else commit(c, if (c.value == "uphold") "upheld" else "reversed",
      decide(current, ap.target, verdict, "admin")
        .with("appeals", appeals.set(c.a, ap.with("state", if (c.value == "uphold") "upheld" else "reversed"))))
  }

  // ---- investigation (MOD-15) ----
  action openInvCmd(c: Command): bool = {
    val inv = invTable.get(c.a)
    if (investigations.keys().contains(c.a)) refuse(c, "unchanged")
    else if (not(admins.contains(inv.admin))) refuse(c, "not_admin")
    else if (inv.admin == targets.get(1).owner) refuse(c, "conflict")
    else if (workPublic) refuse(c, "condition_lapsed")
    else if (not(everPublic.contains(inv.rev))) refuse(c, "never_public")
    else if (inv.expires != 0 and now >= inv.expires) refuse(c, "expired")
    else commit(c, "investigation_opened", current.with("investigations",
      investigations.put(c.a, inv.with("state", "open"))))
  }
  // `revisionGuard` is the mutation switch: without it a read returns another revision.
  action readInvCmd(c: Command, revisionGuard: bool): bool = {
    val known = investigations.keys().contains(c.a)
    val inv = if (known) investigations.get(c.a) else { admin: 0, rev: 0, expires: 0, state: "none" }
    if (not(known)) refuse(c, "unknown_investigation")
    else if (inv.state != "open") refuse(c, "closed")
    else if (inv.expires != 0 and now >= inv.expires) refuse(c, "expired")
    else if (c.actor != inv.admin) refuse(c, "not_investigator")
    else if (revisionGuard and c.b != inv.rev) refuse(c, "wrong_revision")
    else if (workPublic) refuse(c, "condition_lapsed")
    else commit(c, "read_audited", current.with("reads", reads.union(Set((c.a, c.b)))))
  }
  action closeInvCmd(c: Command): bool = {
    val known = investigations.keys().contains(c.a)
    if (not(known)) refuse(c, "unknown_investigation")
    else if (investigations.get(c.a).state != "open") refuse(c, "closed")
    else if (c.actor != investigations.get(c.a).admin) refuse(c, "not_investigator")
    else commit(c, "investigation_closed", current.with("investigations",
      investigations.set(c.a, investigations.get(c.a).with("state", "closed"))))
  }

  action dispatchWith(c: Command, guards: str -> bool): bool =
    if (c.op == "grant") grantCmd(c)
    else if (c.op == "revoke") revokeCmd(c)
    else if (c.op == "join") membershipCmd(c, true)
    else if (c.op == "leave") membershipCmd(c, false)
    else if (c.op == "tick") tickCmd(c)
    else if (c.op == "submit") submitCmd(c)
    else if (c.op == "ai_result") aiCmd(c, guards.get("pause"))
    else if (c.op == "pause") pauseCmd(c)
    else if (c.op == "resume") resumeCmd(c)
    else if (c.op == "make_private") makePrivateCmd(c)
    else if (c.op == "make_public") makePublicCmd(c)
    else if (c.op == "claim") claimCmd(c, guards.get("cas"))
    else if (c.op == "release") releaseCmd(c)
    else if (c.op == "note") noteCmd(c)
    else if (c.op == "escalate") escalateCmd(c)
    else if (c.op == "takeover") takeoverCmd(c)
    else if (c.op == "intervene") interveneCmd(c)
    else if (c.op == "revalidate") revalidateCmd(c)
    else if (c.op == "conclude") concludeCmd(c, guards.get("authority"))
    else if (c.op == "appeal") appealCmd(c, guards.get("dedup"))
    else if (c.op == "appeal_conclude") appealConcludeCmd(c)
    else if (c.op == "open_investigation") openInvCmd(c)
    else if (c.op == "read_investigation") readInvCmd(c, guards.get("revision"))
    else closeInvCmd(c)
  pure val lawful = Map("pause" -> true, "cas" -> true, "authority" -> true, "dedup" -> true, "revision" -> true)
  action dispatch(c: Command): bool = dispatchWith(c, lawful)

  pure def grant(actor: int, g: int): Command = cmd("grant", actor, g, 0, "none")
  pure def revoke(actor: int, g: int): Command = cmd("revoke", actor, g, 0, "none")
  pure def join(actor: int, p: int): Command = cmd("join", actor, p, 0, "none")
  pure def leave(actor: int, p: int): Command = cmd("leave", actor, p, 0, "none")
  pure val tick = cmd("tick", 0, 0, 0, "none")
  pure def submit(t: int): Command = cmd("submit", 0, t, 0, "none")
  pure def ai(job: int, value: str): Command = cmd("ai_result", 0, job, 0, value)
  pure def pause(actor: int): Command = cmd("pause", actor, 0, 0, "none")
  pure def resume(actor: int): Command = cmd("resume", actor, 0, 0, "none")
  pure val makePrivate = cmd("make_private", 1, 0, 0, "none")
  pure val makePublic = cmd("make_public", 1, 0, 0, "none")
  pure def claim(actor: int, k: int, v: int): Command = cmd("claim", actor, k, v, "none")
  pure def release(actor: int, k: int, v: int): Command = cmd("release", actor, k, v, "none")
  pure def note(actor: int, k: int, v: int): Command = cmd("note", actor, k, v, "none")
  pure def escalate(actor: int, k: int, v: int): Command = cmd("escalate", actor, k, v, "none")
  pure def takeover(actor: int, k: int, v: int): Command = cmd("takeover", actor, k, v, "none")
  pure def intervene(actor: int, t: int): Command = cmd("intervene", actor, t, 0, "none")
  pure def revalidate(k: int): Command = cmd("revalidate", 0, k, 0, "none")
  pure def conclude(actor: int, k: int, v: int, value: str): Command = cmd("conclude", actor, k, v, value)
  pure def appeal(actor: int, t: int, v: int): Command = cmd("appeal", actor, t, v, "none")
  pure def appealConclude(actor: int, k: int, value: str): Command = cmd("appeal_conclude", actor, k, 0, value)
  pure def openInv(k: int): Command = cmd("open_investigation", invTable.get(k).admin, k, 0, "none")
  pure def readInv(actor: int, k: int, rev: int): Command = cmd("read_investigation", actor, k, rev, "none")
  pure def closeInv(actor: int, k: int): Command = cmd("close_investigation", actor, k, 0, "none")

  action step = {
    nondet p = 1.to(5).oneOf()
    nondet g = 1.to(5).oneOf()
    nondet t = 1.to(4).oneOf()
    nondet k = 1.to(6).oneOf()
    nondet v = 1.to(8).oneOf()
    nondet j = 1.to(6).oneOf()
    nondet i = 1.to(5).oneOf()
    nondet r = 1.to(2).oneOf()
    nondet ai_value = Set("approve", "reject", "uncertain").oneOf()
    nondet verdict = Set("approve", "reject").oneOf()
    nondet answer = Set("uphold", "reverse").oneOf()
    all {
      cases.keys().size() < 6, jobs.keys().size() < 6, appeals.keys().size() < 4,
      any {
        dispatch(grant(p, g)), dispatch(revoke(p, g)), dispatch(join(p, p)), dispatch(leave(p, p)),
        dispatch(tick), dispatch(submit(t)), dispatch(ai(j, ai_value)), dispatch(pause(p)),
        dispatch(resume(p)), dispatch(makePrivate), dispatch(makePublic), dispatch(claim(p, k, v)),
        dispatch(release(p, k, v)), dispatch(note(p, k, v)), dispatch(escalate(p, k, v)),
        dispatch(takeover(p, k, v)), dispatch(intervene(p, t)), dispatch(revalidate(k)),
        dispatch(conclude(p, k, v, verdict)), dispatch(appeal(p, t, v)),
        dispatch(appealConclude(p, i, answer)), dispatch(openInv(i)), dispatch(readInv(p, i, r)),
        dispatch(closeInv(p, i)),
      },
    }
  }

  // Grants, recusal, scope, expiry and groups; two moderators race; a revoked or expired grant
  // while processing (kept through another grant, then lost); release, note, escalate, takeover.
  pure val claimsScenario = List(
    grant(2, 1),           // not_admin
    grant(4, 1),           // granted: 2 reviews works
    grant(4, 2),           // granted: 3 reviews work 1 until tick 3
    submit(1),             // submitted_ai: job 1
    ai(1, "uncertain"),    // needs_human: case 1 on decision version 2
    claim(5, 1, 1),        // not_authorized
    claim(2, 1, 1),        // claimed by 2 (grant 1); case version 2
    claim(3, 1, 1),        // stale_case: the second moderator saw version 1
    claim(3, 1, 2),        // already_claimed
    note(2, 1, 2),         // noted: internal, version 3
    release(3, 1, 3),      // not_claimant
    release(2, 1, 3),      // released: back to the queue, version 4
    claim(3, 1, 4),        // claimed by 3 (grant 2), version 5
    grant(4, 4),           // granted: 3 also reviews works everywhere
    tick, tick, tick,      // grant 2 expires
    revalidate(1),         // basis_changed: 3 keeps the case through grant 4, version 6
    revoke(4, 4),          // revoked
    conclude(3, 1, 6, "approve"), // not_authorized: no live basis at the conclusion
    revalidate(1),         // authority_lost: back to the queue, version 7
    claim(2, 1, 7),        // claimed by 2, version 8
    escalate(2, 1, 8),     // escalated, version 9
    claim(2, 1, 9),        // escalated_to_admin
    takeover(2, 1, 9),     // not_admin
    takeover(1, 1, 9),     // recused: admin 1 wrote this work
    takeover(4, 1, 9),     // taken_over by admin 4, version 10
    conclude(4, 1, 10, "approve"), // concluded: decision version 3, by the admin
    submit(3),             // submitted_ai: job 2 (work 2 of moderator 2)
    ai(2, "uncertain"),    // needs_human: case 2
    claim(2, 2, 1),        // recused: moderator 2 wrote work 2
    grant(4, 3),           // granted: group 7 reviews comments
    submit(2),             // submitted_ai: job 3 (comment of 5)
    intervene(2, 2),       // not_authorized: a works grant does not review comments
    join(4, 3),            // joined: 3 in group 7
    intervene(3, 2),       // intervened: case 3 held by 3 through the group
    leave(4, 3),           // left
    conclude(3, 3, 2, "reject"), // not_authorized: membership is re-read
    revalidate(3),         // authority_lost
  )
  action claimsStep = all { cursor < claimsScenario.length(), dispatch(claimsScenario.nth(cursor)) }

  // Late AI results after a human decision, after a pause and after the work went private.
  pure val automationScenario = List(
    grant(4, 1),           // granted
    submit(1),             // submitted_ai: job 1
    intervene(2, 1),       // intervened: case 1 held by 2
    conclude(2, 1, 2, "approve"), // concluded: decided by the moderator
    ai(1, "reject"),       // overridden: the late result loses to the moderator
    submit(2),             // submitted_ai: job 2
    submit(3),             // submitted_ai: job 3
    pause(2),              // not_admin
    pause(4),              // paused: jobs 2 and 3 rerouted, cases 2 and 3 queued
    ai(2, "approve"),      // automation_paused
    resume(4),             // resumed
    ai(3, "approve"),      // automation_paused: a rerouted job stays with people
    submit(4),             // submitted_ai: a new submission goes to the evaluator again (job 4)
    makePrivate,           // made_private: job 4 withdrawn
    ai(4, "approve"),      // withdrawn
    submit(4),             // not_needed: the revision stays submitted; publication.qnt reopens it
  )
  action automationStep = all {
    cursor < automationScenario.length(), dispatch(automationScenario.nth(cursor)),
  }

  // Appeals of an AI and of a moderator decision; duplicates, stale and final decisions; an open
  // appeal reserves the decision for an admin; private closes the work's appeals.
  pure val appealsScenario = List(
    grant(4, 1),           // granted
    submit(1),             // submitted_ai: job 1
    ai(1, "reject"),       // applied: rejected by the AI, decision version 2
    appeal(5, 1, 2),       // not_found: not the owner
    appeal(1, 1, 1),       // stale_decision
    appeal(1, 1, 2),       // appeal_recorded: not delivered
    appeal(1, 1, 2),       // appeal_replayed: the same appeal
    intervene(2, 1),       // intervened: case 1 on version 2
    conclude(2, 1, 2, "approve"), // appeal_open: only an admin concludes now
    appealConclude(2, 1, "reverse"), // not_admin
    appealConclude(1, 1, "reverse"), // recused: the author-admin
    appealConclude(4, 1, "uphold"),  // upheld: version 3, by the admin
    appeal(1, 1, 3),       // final_decision
    conclude(2, 1, 2, "approve"), // stale_decision: the admin's version moved on
    appealConclude(4, 1, "reverse"), // already_resolved
    submit(3),             // submitted_ai: job 2
    ai(2, "approve"),      // applied
    appeal(2, 3, 2),       // nothing_to_appeal
    grant(4, 5),           // granted: 2's inherited comment grant
    submit(2),             // submitted_ai: job 3
    intervene(2, 2),       // intervened: case 2
    conclude(2, 2, 2, "reject"), // concluded: the comment rejected by the moderator
    ai(3, "approve"),      // overridden
    appeal(5, 2, 2),       // appeal_recorded: a moderator's decision is appealable
    appealConclude(4, 2, "reverse"), // reversed: approved by the admin
    submit(4),             // submitted_ai: job 4
    ai(4, "reject"),       // applied
    appeal(1, 4, 2),       // appeal_recorded
    makePrivate,           // made_private: the appeal closes
    appealConclude(4, 3, "reverse"), // already_resolved
    appeal(1, 4, 2),       // withdrawn
  )
  action appealsStep = all { cursor < appealsScenario.length(), dispatch(appealsScenario.nth(cursor)) }

  // Per-read investigation of a once-public revision of a private work.
  pure val investigationScenario = List(
    openInv(1),            // condition_lapsed: the work is public
    makePrivate,           // made_private
    openInv(2),            // conflict: the admin wrote the work
    openInv(3),            // never_public
    openInv(4),            // not_admin
    openInv(1),            // investigation_opened
    readInv(2, 1, 1),      // not_investigator
    readInv(4, 1, 2),      // wrong_revision
    readInv(4, 1, 1),      // read_audited
    tick, tick, tick,
    readInv(4, 1, 1),      // expired
    openInv(5),            // investigation_opened
    readInv(4, 5, 1),      // read_audited
    makePublic,            // made_public
    readInv(4, 5, 1),      // condition_lapsed
    closeInv(4, 5),        // investigation_closed
    readInv(4, 5, 1),      // closed
  )
  action investigationStep = all {
    cursor < investigationScenario.length(), dispatch(investigationScenario.nth(cursor)),
  }

  // Negative controls.
  pure val lostAuthority = List(grant(4, 1), submit(1), ai(1, "uncertain"), claim(2, 1, 1),
    revoke(4, 1), conclude(2, 1, 2, "approve"))
  action mutatedAuthorityStep = all {
    cursor < lostAuthority.length(),
    dispatchWith(lostAuthority.nth(cursor), lawful.set("authority", false)),
  }
  pure val stolenClaim = List(grant(4, 1), grant(4, 4), submit(1), ai(1, "uncertain"),
    claim(2, 1, 1), claim(3, 1, 1))
  action mutatedClaimStep = all {
    cursor < stolenClaim.length(), dispatchWith(stolenClaim.nth(cursor), lawful.set("cas", false)),
  }
  pure val duplicateAppeal = List(submit(1), ai(1, "reject"), appeal(1, 1, 2), appeal(1, 1, 2))
  action mutatedAppealStep = all {
    cursor < duplicateAppeal.length(), dispatchWith(duplicateAppeal.nth(cursor), lawful.set("dedup", false)),
  }
  pure val pausedResult = List(submit(1), pause(4), ai(1, "approve"))
  action mutatedPauseStep = all {
    cursor < pausedResult.length(), dispatchWith(pausedResult.nth(cursor), lawful.set("pause", false)),
  }
  pure val wrongRead = List(makePrivate, openInv(1), readInv(4, 1, 2))
  action mutatedReadStep = all {
    cursor < wrongRead.length(), dispatchWith(wrongRead.nth(cursor), lawful.set("revision", false)),
  }

  pure val refusals = Set("not_admin", "unchanged", "limit", "not_needed", "work_private", "unknown_job",
    "withdrawn", "automation_paused", "duplicate", "overridden", "superseded", "unknown_case",
    "case_closed", "stale_case", "escalated_to_admin", "already_claimed", "not_authorized", "recused",
    "not_claimant", "not_submitted", "case_open", "authority_kept", "stale_decision", "appeal_open",
    "not_found", "appeal_replayed", "final_decision", "nothing_to_appeal", "unknown_appeal",
    "already_resolved", "conflict", "condition_lapsed", "never_public", "expired",
    "unknown_investigation", "closed", "not_investigator", "wrong_revision")

  val refusalWritesNothing = not(refusals.contains(outcome)) or current == previous
  // MOD-11/12: claims and conclusions need live authority at that commit; reviewers recuse.
  val claimNeedsAuthority = not(Set("claimed", "taken_over", "intervened").contains(outcome)) or
    authority(previous, command.actor, if (command.op == "intervene") command.a
      else previous.cases.get(command.a).target) == "ok"
  val conclusionNeedsLiveAuthority = outcome != "concluded" or
    authority(previous, command.actor, previous.cases.get(command.a).target) == "ok"
  // MOD-12: at most one claimant; a held case changes hands only by release, loss or takeover.
  val claimNeverStolen = previous.cases.keys().forall(k => {
    val before = previous.cases.get(k)
    val after = cases.get(k)
    not(before.state == "claimed" and after.state == "claimed" and after.claimant != before.claimant) or
      command.op == "takeover"
  })
  // MOD-12: a conclusion matches the case version and the decision version it was opened on.
  val conclusionFenced = outcome != "concluded" or (previous.cases.get(command.a).version == command.b and
    previous.decisions.get(previous.cases.get(command.a).target).version == previous.cases.get(command.a).fence)
  val versionsOnlyGrow = previous.cases.keys().forall(k => cases.get(k).version >= previous.cases.get(k).version) and
    previous.decisions.keys().forall(t => decisions.get(t).version >= previous.decisions.get(t).version)
  // MOD-09: a person's decision is never changed by the evaluator.
  val humanDecisionWins = command.op != "ai_result" or previous.decisions.keys().forall(t =>
    not(Set("moderator", "admin").contains(previous.decisions.get(t).by)) or decisions.get(t) == previous.decisions.get(t))
  // MOD-14: while paused, the evaluator writes no decision.
  val pausedAiWritesNothing = not(command.op == "ai_result" and previous.automation == "paused") or
    decisions == previous.decisions
  val reroutedStaysHuman = previous.jobs.keys().forall(j =>
    previous.jobs.get(j).state != "rerouted" or Set("rerouted", "withdrawn").contains(jobs.get(j).state))
  // PUB-06: a private work has no open job, case or appeal on its revisions.
  val privateClosesWork = workPublic or (
    jobs.keys().forall(j => not(workRevisions.contains(jobs.get(j).target)) or
      not(Set("open", "rerouted").contains(jobs.get(j).state))) and
    cases.keys().forall(k => not(workRevisions.contains(cases.get(k).target)) or
      not(Set("queued", "claimed", "escalated").contains(cases.get(k).state))) and
    appeals.keys().forall(k => not(workRevisions.contains(appeals.get(k).target)) or appeals.get(k).state != "open"))
  // MOD-10: one appeal per decision version; never on an admin decision; nothing is delivered.
  val oneAppealPerDecision = appeals.keys().forall(a => appeals.keys().forall(b =>
    a == b or appeals.get(a).target != appeals.get(b).target or appeals.get(a).decision != appeals.get(b).decision))
  val adminIsFinal = appeals.keys().forall(k => appeals.get(k).by != "admin")
  val nothingIsDelivered = appeals.keys().forall(k => appeals.get(k).notice == "not_sent")
  val appealReservesAdmin = outcome != "concluded" or admins.contains(command.actor) or
    not(appealOpenOn(previous, previous.cases.get(command.a).target,
      previous.decisions.get(previous.cases.get(command.a).target).version))
  // MOD-15: every audited read is of the file's exact once-public revision, by its admin, while
  // the file is open, unexpired and the work private.
  val readsAreAuthorized = reads.forall(r => investigations.keys().contains(r._1) and
    investigations.get(r._1).rev == r._2 and everPublic.contains(r._2)) and
    (outcome != "read_audited" or (not(previous.workPublic) and
      previous.investigations.get(command.a).state == "open" and
      previous.investigations.get(command.a).admin == command.actor and
      (previous.investigations.get(command.a).expires == 0 or previous.now < previous.investigations.get(command.a).expires)))
  val investigatorHasNoConflict = investigations.keys().forall(k =>
    admins.contains(investigations.get(k).admin) and investigations.get(k).admin != targets.get(1).owner)
  val governanceSafe = refusalWritesNothing and claimNeedsAuthority and conclusionNeedsLiveAuthority and
    claimNeverStolen and conclusionFenced and versionsOnlyGrow and humanDecisionWins and
    pausedAiWritesNothing and reroutedStaysHuman and privateClosesWork and oneAppealPerDecision and
    adminIsFinal and nothingIsDelivered and appealReservesAdmin and readsAreAuthorized and
    investigatorHasNoConflict
}
