// PRODUCT-DECISIONS-20261110, M1 — canonical publication model. #150 replays the author
// actions and publication effects of already-admitted decisions through the production pure
// core. Job/authority/policy validation and lifecycle mutations remain target-only; see README.md.
//
// One work of one author. Audience (the author's choice: only me / public) is a separate axis
// from the moderation decision of each immutable revision and from the publication pointer.
// ASSUMPTIONS, not proofs: each action is one serialized commit; actors are who they say they
// are (auth is D-05); the evaluator is a service capability that can only submit results; the
// admin is the product owner acting through a future operator surface. No clock, no SLA.
// ADMIN-MODERATION-20261111 (PUB-06): making the work private withdraws its open evaluations and
// closes its review cases (late results and decisions write nothing); republishing a pending
// revision whose evaluation was withdrawn asks again; an approval of an older policy is reviewed
// again for the exact snapshot (by the evaluator if the AI approved it, by a person otherwise).
// Who the person is (admin or moderator under a grant) is governance.qnt; here "admin".
module publication {
  type Revision = { decision: str, by: str, policy: int }
  type Job = { rev: int, policy: int, state: str }
  // actor: author | other | evaluator | system | admin. `expected` is the audience generation
  // the author's tab saw; `value` is the evaluator outcome or the admin verdict.
  type Command = { op: str, actor: str, rev: int, job: int, expected: int, value: str }
  type Snapshot = {
    revs: int -> Revision, jobs: int -> Job, audience: str, desired: int, intended: Set[int],
    pointer: int, lifecycle: str, generation: int, policy: int,
  }
  pure val maxRevs = 4
  pure val maxJobs = 8
  pure val maxPolicy = 2
  pure def cmd(op: str, actor: str, rev: int, job: int, expected: int, value: str): Command =
    { op: op, actor: actor, rev: rev, job: job, expected: expected, value: value }
  pure val save = cmd("snapshot", "author", 0, 0, 0, "none")
  pure def goPublic(actor: str, rev: int, expected: int): Command =
    cmd("request_public", actor, rev, 0, expected, "none")
  pure def goPrivate(expected: int): Command = cmd("set_private", "author", 0, 0, expected, "none")
  pure def ai(job: int, rev: int, value: str): Command = cmd("ai_result", "evaluator", rev, job, 0, value)
  pure def requeue(rev: int): Command = cmd("reevaluate", "system", rev, 0, 0, "none")
  pure def decide(actor: str, rev: int, value: str): Command = cmd("admin_decide", actor, rev, 0, 0, value)
  pure val authorDelete = cmd("author_delete", "author", 0, 0, 0, "none")
  pure val adminRemove = cmd("admin_remove", "admin", 0, 0, 0, "none")
  pure val policyBump = cmd("policy_bump", "admin", 0, 0, 0, "none")

  var revs: int -> Revision   // immutable snapshots the author saved; the decision is per revision
  var jobs: int -> Job        // evaluations the App asked for, each bound to a revision and policy
  var audience: str           // "private" | "public": what the author chose, not a moderation state
  var desired: int            // the revision the author wants public (0 = none)
  var intended: Set[int]      // revisions given public intent since the last withdrawal
  var pointer: int            // the revision other people can read (0 = none)
  var lifecycle: str          // "active" | "author_deleted" | "removed"
  var generation: int         // accepted audience intents of the author (fences old tabs)
  var policy: int
  var requested: Set[int]     // ghost: revisions the author ever asked to publish
  var command: Command
  var outcome: str
  var previous: Snapshot
  var cursor: int

  val current: Snapshot = {
    revs: revs, jobs: jobs, audience: audience, desired: desired, intended: intended,
    pointer: pointer, lifecycle: lifecycle, generation: generation, policy: policy,
  }
  pure val start: Snapshot = {
    revs: Map(), jobs: Map(), audience: "private", desired: 0, intended: Set(), pointer: 0,
    lifecycle: "active", generation: 1, policy: 1,
  }

  action init = all {
    revs' = Map(), jobs' = Map(), audience' = "private", desired' = 0, intended' = Set(),
    pointer' = 0, lifecycle' = "active", generation' = 1, policy' = 1, requested' = Set(),
    command' = cmd("init", "none", 0, 0, 0, "none"), outcome' = "init", previous' = start,
    cursor' = 0,
  }

  // Every action ends here: `after` is the whole durable state after the commit.
  action commit(c: Command, result: str, after: Snapshot, asked: Set[int]): bool = all {
    revs' = after.revs, jobs' = after.jobs, audience' = after.audience, desired' = after.desired,
    intended' = after.intended, pointer' = after.pointer, lifecycle' = after.lifecycle,
    generation' = after.generation, policy' = after.policy, requested' = asked,
    command' = c, outcome' = result, previous' = current,
    cursor' = if (cursor < 80) cursor + 1 else cursor,
  }
  action refuse(c: Command, result: str): bool = commit(c, result, current, requested)

  // The publish guard, re-read at every commit that could move the pointer (PUB-01, INV-01).
  // `intentGuard` is the mutation switch: without it an approval alone publishes.
  pure def lawful(s: Snapshot, r: int, intentGuard: bool): bool =
    s.lifecycle == "active" and s.revs.get(r).decision == "approved" and
    (not(intentGuard) or (s.audience == "public" and s.desired == r))
  pure def publish(s: Snapshot, r: int, intentGuard: bool): Snapshot =
    if (lawful(s, r, intentGuard)) s.with("pointer", r) else s
  pure def setRev(s: Snapshot, r: int, decision: str, by: str): Snapshot =
    s.with("revs", s.revs.set(r, { decision: decision, by: by, policy: s.policy }))
  pure val humanBy = Set("admin", "rereview")
  pure def openFor(s: Snapshot, r: int): bool = s.jobs.keys().exists(j =>
    s.jobs.get(j).rev == r and s.jobs.get(j).state == "open" and s.jobs.get(j).policy == s.policy)

  // A private save: a new immutable snapshot, no evaluation, no provider call (MOD-05, INV-08).
  action snapshot(c: Command): bool = {
    val id = revs.keys().size() + 1
    if (c.actor != "author") refuse(c, "not_found")
    else if (lifecycle != "active") refuse(c, "work_unavailable")
    else if (id > maxRevs) refuse(c, "limit")
    else commit(c, "saved", current.with("revs", revs.put(id, { decision: "none", by: "none", policy: 0 })), requested)
  }

  // "Công khai": the author's intent for exactly one snapshot. It never skips moderation, and
  // an approval is reused only under the policy it was made for (PUB-06).
  action requestPublic(c: Command): bool = {
    val job = jobs.keys().size() + 1
    val intent = current.with("audience", "public").with("desired", c.rev)
      .with("intended", intended.union(Set(c.rev))).with("generation", generation + 1)
    val ask = intent.with("jobs", jobs.put(job, { rev: c.rev, policy: policy, state: "open" }))
    val rev = if (revs.keys().contains(c.rev)) revs.get(c.rev) else { decision: "none", by: "none", policy: 0 }
    if (c.actor != "author") refuse(c, "not_found")
    else if (lifecycle != "active") refuse(c, "work_unavailable")
    else if (not(revs.keys().contains(c.rev))) refuse(c, "unknown_revision")
    else if (c.expected != generation) refuse(c, "stale")
    else if (rev.decision == "rejected") refuse(c, "revision_rejected")
    else if (rev.decision == "approved" and (rev.policy == policy or pointer == c.rev))
      commit(c, "published", intent.with("pointer", c.rev), requested)
    // An approval of an older policy: a person's goes back to people, the AI's to the AI.
    else if (rev.decision == "approved" and humanBy.contains(rev.by))
      commit(c, "rereview", setRev(intent, c.rev, "needs_admin", "rereview"), requested)
    else if (rev.decision == "approved")
      if (job > maxJobs) refuse(c, "limit")
      else commit(c, "rereview", setRev(ask, c.rev, "pending", "none"), requested)
    // Pending, but its evaluation was withdrawn when the work went private: ask again.
    else if (rev.decision == "pending" and not(openFor(current, c.rev)))
      if (job > maxJobs) refuse(c, "limit")
      else commit(c, "resubmitted", ask, requested)
    else if (rev.decision != "none") commit(c, "waiting", intent, requested)
    else if (job > maxJobs) refuse(c, "limit")
    else commit(c, "submitted", setRev(ask, c.rev, "pending", "none"), requested.union(Set(c.rev)))
  }

  // "Chỉ mình tôi": closes public reading at once and withdraws open evaluations (their cases
  // and appeals close). Decisions already made are kept.
  action setPrivate(c: Command): bool = {
    val after = current.with("audience", "private").with("desired", 0).with("intended", Set())
      .with("pointer", 0).with("generation", generation + 1)
      .with("jobs", jobs.keys().mapBy(j =>
        if (jobs.get(j).state == "open") jobs.get(j).with("state", "withdrawn") else jobs.get(j)))
    if (c.actor != "author") refuse(c, "not_found")
    else if (lifecycle != "active") refuse(c, "work_unavailable")
    else if (c.expected != generation) refuse(c, "stale")
    else commit(c, if (audience == "public") "made_private" else "reaffirmed", after, requested)
  }

  // A result from the evaluator. It carries no authority of its own: it is validated against
  // the job, the revision, the policy version, the lifecycle and any admin decision first.
  // `overrideGuard` and `intentGuard` are the mutation switches.
  action aiResult(c: Command, overrideGuard: bool, intentGuard: bool): bool = {
    val known = jobs.keys().contains(c.job)
    val job = if (known) jobs.get(c.job) else { rev: 0, policy: 0, state: "none" }
    val closed = current.with("jobs", jobs.setBy(c.job, j => j.with("state", "applied")))
    if (c.actor != "evaluator") refuse(c, "not_evaluator")
    else if (not(known)) refuse(c, "unknown_job")
    else if (job.state == "withdrawn") refuse(c, "withdrawn")
    else if (job.state != "open") refuse(c, "duplicate")
    else if (job.rev != c.rev) refuse(c, "wrong_revision")
    else if (lifecycle != "active") refuse(c, "target_gone")
    else if (overrideGuard and humanBy.contains(revs.get(c.rev).by)) refuse(c, "overridden")
    else if (job.policy != policy) refuse(c, "stale_policy")
    else if (c.value == "approve") {
      val after = publish(setRev(closed, c.rev, "approved", "ai"), c.rev, intentGuard)
      commit(c, if (after.pointer == c.rev) "published" else "approved_unpublished", after, requested)
    }
    else if (c.value == "reject") commit(c, "rejected", setRev(closed, c.rev, "rejected", "ai"), requested)
    else if (c.value == "uncertain" or c.value == "invalid_output")
      commit(c, "needs_admin", setRev(closed, c.rev, "needs_admin", "ai"), requested)
    // timeout | provider_error: never an approval; the revision stays pending and retryable.
    else commit(c, "retryable",
      current.with("jobs", jobs.setBy(c.job, j => j.with("state", "failed"))), requested)
  }

  // The App asks again (after a failure or a policy change). Only for a pending revision.
  action reevaluate(c: Command): bool = {
    val job = jobs.keys().size() + 1
    val open = jobs.keys().exists(j =>
      jobs.get(j).rev == c.rev and jobs.get(j).state == "open" and jobs.get(j).policy == policy)
    if (lifecycle != "active" or audience != "public" or not(revs.keys().contains(c.rev)) or
        revs.get(c.rev).decision != "pending" or open) refuse(c, "not_needed")
    else if (job > maxJobs) refuse(c, "limit")
    else commit(c, "requeued",
      current.with("jobs", jobs.put(job, { rev: c.rev, policy: policy, state: "open" })), requested)
  }

  // The admin outranks the evaluator on the revision under review, and nothing else: no
  // decision on a snapshot the author never submitted, no publishing without author intent.
  action adminDecide(c: Command): bool = {
    if (c.actor != "admin") refuse(c, "not_admin")
    else if (not(revs.keys().contains(c.rev))) refuse(c, "unknown_revision")
    else if (lifecycle != "active") refuse(c, "target_gone")
    else if (revs.get(c.rev).decision == "none") refuse(c, "not_submitted")
    // PUB-06: the author made the work private; its review cases are closed.
    else if (audience == "private") refuse(c, "case_closed")
    else if (c.value == "approve") {
      val after = publish(setRev(current, c.rev, "approved", "admin"), c.rev, true)
      commit(c, if (after.pointer == c.rev) "published" else "approved_unpublished", after, requested)
    }
    else {
      val after = setRev(current, c.rev, "rejected", "admin")
      if (pointer == c.rev) commit(c, "unpublished", after.with("pointer", 0), requested)
      else commit(c, "rejected", after, requested)
    }
  }

  action authorDeleted(c: Command): bool =
    if (c.actor != "author") refuse(c, "not_found")
    else if (lifecycle != "active") refuse(c, "work_unavailable")
    else commit(c, "deleted", current.with("lifecycle", "author_deleted").with("pointer", 0)
      .with("desired", 0).with("intended", Set()).with("audience", "private")
      .with("generation", generation + 1), requested)

  action adminRemoved(c: Command): bool =
    if (c.actor != "admin") refuse(c, "not_admin")
    else if (lifecycle != "active") refuse(c, "target_gone")
    else commit(c, "removed", current.with("lifecycle", "removed").with("pointer", 0)
      .with("desired", 0).with("intended", Set()), requested)

  action policyBumped(c: Command): bool =
    if (policy >= maxPolicy) refuse(c, "limit")
    else commit(c, "policy_changed", current.with("policy", policy + 1), requested)

  action dispatchWith(c: Command, overrideGuard: bool, intentGuard: bool): bool =
    if (c.op == "snapshot") snapshot(c)
    else if (c.op == "request_public") requestPublic(c)
    else if (c.op == "set_private") setPrivate(c)
    else if (c.op == "ai_result") aiResult(c, overrideGuard, intentGuard)
    else if (c.op == "reevaluate") reevaluate(c)
    else if (c.op == "admin_decide") adminDecide(c)
    else if (c.op == "author_delete") authorDeleted(c)
    else if (c.op == "admin_remove") adminRemoved(c)
    else policyBumped(c)
  action dispatch(c: Command): bool = dispatchWith(c, true, true)

  action step = {
    nondet r = 1.to(maxRevs).oneOf()
    nondet j = 1.to(maxJobs).oneOf()
    nondet e = Set(generation, generation - 1).oneOf()
    nondet who = Set("author", "other").oneOf()
    nondet v = Set("approve", "reject", "uncertain", "invalid_output", "timeout", "provider_error").oneOf()
    nondet verdict = Set("approve", "reject").oneOf()
    nondet judge = Set("admin", "evaluator", "author").oneOf()
    any {
      dispatch(save), dispatch(goPublic(who, r, e)), dispatch(goPrivate(e)),
      dispatch(ai(j, r, v)), dispatch(requeue(r)), dispatch(decide(judge, r, verdict)),
      dispatch(authorDelete), dispatch(adminRemove), dispatch(policyBump),
    }
  }

  // Private save, failure and retry, a wrong revision, r1 public while r2 is rejected, an old
  // tab, a policy change, an admin override that a late result cannot undo, private again (late
  // results withdrawn, decisions refused), republishing an approval of the old policy, a
  // withdrawn evaluation asked again, approval without intent, author deletion after it.
  pure val lifecycleScenario = List(
    save,                          // r1: private, no evaluation
    decide("admin", 1, "approve"), // not_submitted: the admin has no say over a private save
    goPublic("other", 1, 1),       // not_found
    goPublic("author", 1, 1),      // submitted: job 1, generation 2
    goPublic("author", 1, 2),      // waiting: already pending, generation 3
    ai(1, 1, "timeout"),           // retryable: still pending, nothing public
    ai(1, 1, "approve"),           // duplicate: job 1 is closed
    requeue(1),                    // requeued: job 2
    requeue(1),                    // not_needed
    ai(2, 2, "approve"),           // wrong_revision
    ai(6, 1, "approve"),           // unknown_job
    ai(2, 1, "approve"),           // published: pointer r1, approved under policy 1
    save,                          // r2
    goPublic("author", 2, 3),      // submitted: job 3, generation 4; r1 stays public
    ai(3, 2, "reject"),            // rejected: r1 stays public (PUB-02)
    goPublic("author", 2, 4),      // revision_rejected
    save,                          // r3
    goPublic("author", 3, 3),      // stale: an old tab
    goPublic("author", 3, 4),      // submitted: job 4, generation 5
    policyBump,                    // policy 2
    ai(4, 3, "approve"),           // stale_policy: writes nothing
    requeue(3),                    // requeued: job 5 under policy 2
    ai(5, 3, "uncertain"),         // needs_admin: not public
    decide("evaluator", 3, "approve"), // not_admin: the evaluator is not an admin
    decide("admin", 3, "approve"), // published: pointer r3, decided by the admin
    ai(4, 3, "reject"),            // overridden: the late result loses to the admin
    goPrivate(5),                  // made_private: pointer 0, generation 6; job 4 withdrawn
    goPublic("author", 3, 5),      // stale: the old tab does not republish
    goPrivate(6),                  // reaffirmed: generation 7
    goPublic("author", 1, 7),      // rereview: r1's AI approval is of policy 1; job 6
    ai(4, 3, "approve"),           // withdrawn: the late result of a private period
    ai(6, 1, "approve"),           // published: r1 approved again under policy 2
    decide("admin", 1, "reject"),  // unpublished: the admin withdraws r1
    save,                          // r4
    goPublic("author", 4, 8),      // submitted: job 7, generation 9
    goPrivate(9),                  // made_private: job 7 withdrawn, generation 10
    ai(7, 4, "approve"),           // withdrawn: no decision while the work is private
    decide("admin", 4, "approve"), // case_closed: no human decision either
    goPublic("author", 4, 10),     // resubmitted: job 8, generation 11
    decide("admin", 1, "approve"), // approved_unpublished: r1 is not the current intent
    ai(8, 4, "approve"),           // published: r4
    authorDelete,                  // deleted
    goPublic("author", 4, 12),     // work_unavailable
    decide("admin", 4, "approve"), // target_gone
  )
  action lifecycleStep = all {
    cursor < lifecycleScenario.length(), dispatch(lifecycleScenario.nth(cursor)),
  }

  // A person's approval of an older policy goes back to people, not to the evaluator.
  pure val rereviewScenario = List(
    save, goPublic("author", 1, 1),
    ai(1, 1, "uncertain"),         // needs_admin
    decide("admin", 1, "approve"), // published under policy 1
    goPrivate(2),                  // made_private
    policyBump,                    // policy 2
    goPublic("author", 1, 3),      // rereview: needs a person again, no evaluation
    requeue(1),                    // not_needed: nothing for the evaluator
    decide("admin", 1, "approve"), // published under policy 2
  )
  action rereviewStep = all {
    cursor < rereviewScenario.length(), dispatch(rereviewScenario.nth(cursor)),
  }

  // Moderation removal is not author deletion: terminal here, nothing comes back.
  pure val removalScenario = List(
    save, goPublic("author", 1, 1),
    ai(1, 1, "provider_error"),    // retryable
    requeue(1),                    // requeued: job 2
    ai(2, 1, "invalid_output"),    // needs_admin: an unusable output approves nothing
    decide("admin", 1, "approve"), // published by the admin
    cmd("admin_remove", "author", 0, 0, 0, "none"), // not_admin
    adminRemove,                   // removed
    goPublic("author", 1, 2),      // work_unavailable
    authorDelete,                  // work_unavailable
    adminRemove,                   // target_gone
  )
  action removalStep = all {
    cursor < removalScenario.length(), dispatch(removalScenario.nth(cursor)),
  }

  // Negative control 1: without the admin-override guard a late evaluator result reverses the
  // admin's decision on the same revision.
  pure val lateResult = List(save, goPublic("author", 1, 1), decide("admin", 1, "approve"),
    ai(1, 1, "reject"))
  action mutatedOverrideStep = all {
    cursor < lateResult.length(), dispatchWith(lateResult.nth(cursor), false, true),
  }
  // Negative control 2: without the intent guard an approval of r1 publishes it although the
  // author's current public intent names r2.
  pure val withdrawnIntent = List(save, save, goPublic("author", 1, 1), goPublic("author", 2, 2),
    ai(1, 1, "approve"))
  action mutatedIntentStep = all {
    cursor < withdrawnIntent.length(), dispatchWith(withdrawnIntent.nth(cursor), true, false),
  }

  pure val refusals = Set("not_found", "work_unavailable", "limit", "unknown_revision", "stale",
    "revision_rejected", "not_evaluator", "unknown_job", "duplicate", "wrong_revision",
    "target_gone", "overridden", "stale_policy", "not_needed", "not_admin", "not_submitted",
    "withdrawn", "case_closed")

  // PUB-01 / INV-01: a public revision needs the author's intent, an approval and a live work.
  val publicNeedsEverything = pointer == 0 or (lifecycle == "active" and audience == "public" and
    revs.get(pointer).decision == "approved" and intended.contains(pointer))
  // MOD-05 / INV-08: only revisions the author asked to publish are ever sent for evaluation.
  val privateSaveNeverEvaluated = jobs.keys().forall(j => requested.contains(jobs.get(j).rev))
  // USER-14: once a person decided a revision, only a person changes that decision, or the author
  // republishing it under a newer policy sends it back to people (PUB-06).
  val adminOverrideWins = previous.revs.keys().forall(r => not(humanBy.contains(previous.revs.get(r).by)) or
    revs.get(r) == previous.revs.get(r) or command.op == "admin_decide" or
    (command.op == "request_public" and command.rev == r and previous.revs.get(r).policy < policy and
      revs.get(r) == { decision: "needs_admin", by: "rereview", policy: policy }))
  val refusalWritesNothing = not(refusals.contains(outcome)) or current == previous
  // MOD-03: a failure, an unusable output or a rejection never changes what is public.
  val failureNeverPublishes = not(command.op == "ai_result" and command.value != "approve") or
    pointer == previous.pointer
  // INV-07: deletion and removal are terminal; no result or command brings the work back.
  val noResurrection = previous.lifecycle == "active" or (lifecycle == previous.lifecycle and
    pointer == 0 and desired == 0 and revs == previous.revs and audience == previous.audience)
  val pointerMovesLawfully = pointer == previous.pointer or pointer == 0 or pointer == desired
  // MOD-02: a decision belongs to one revision; a new snapshot starts undecided.
  val decisionsAreVersionBound = revs.keys().forall(r =>
    if (previous.revs.keys().contains(r)) revs.get(r) == previous.revs.get(r) or r == command.rev
    else revs.get(r) == { decision: "none", by: "none", policy: 0 })
  val evaluatorOnlyDecidesPending = not(command.op == "ai_result" and revs != previous.revs) or
    (previous.revs.get(command.rev).decision == "pending" and revs.get(command.rev).by == "ai")
  // PUB-03 / COM-07 style fence: an older tab never undoes a newer accepted audience intent.
  val audienceFence = generation == previous.generation or
    (generation == previous.generation + 1 and
      (command.expected == previous.generation or command.op == "author_delete"))
  // PUB-06: while the work is private no evaluation is open and no decision changes.
  val privateWithdrawsReview = lifecycle != "active" or audience == "public" or
    jobs.keys().forall(j => jobs.get(j).state != "open")
  val privateFreezesDecisions = not(previous.audience == "private" and previous.lifecycle == "active" and
    command.op != "request_public") or previous.revs.keys().forall(r => revs.get(r) == previous.revs.get(r))
  // PUB-06: republishing never reuses an approval of an older policy.
  val staleApprovalNeverRepublishes = not(command.op == "request_public" and pointer != 0 and
    pointer != previous.pointer) or revs.get(pointer).policy == policy
  val finiteDomain = revs.keys().size() <= maxRevs and jobs.keys().size() <= maxJobs and
    policy <= maxPolicy
  val publicationSafe = publicNeedsEverything and privateSaveNeverEvaluated and adminOverrideWins and
    refusalWritesNothing and failureNeverPublishes and noResurrection and pointerMovesLawfully and
    decisionsAreVersionBound and evaluatorOnlyDecidesPending and audienceFence and
    privateWithdrawsReview and privateFreezesDecisions and staleApprovalNeverRepublishes and finiteDomain
}
