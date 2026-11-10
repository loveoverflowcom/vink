// #120 DISC-01: comment admission under a versioned policy, its shadow evaluation and the
// post-review of a publish-first admission. `trusted-cohort.v1` is a PROPOSED candidate; the
// model lets an operator make it effective only to check that candidate before any D-08 delta,
// while production `PolicySelection` keeps premoderation effective.
// Each action is one serialized commit. Standing/trust reads at commit, SQL locks, the outbox,
// worker leases and retries are ASSUMPTIONS here (DISC-03/04), not proofs. Human moderation and
// writer removal reuse the lifecycle at the current version; their authority is lifecycle.qnt's.
// Writers 1..3, comments 1..3 in simulation, one public open thread, an abstract clock of ticks.
module admission {
  type Comment = { writer: int, review: str, removed: bool, version: int, policy: str, reason: str }
  // submit: value = rules verdict, age = rules age (-1 future, 0 fresh, 1 stale), matches = rules
  // assessed this payload, flag = classifier verdict. moderate: value = approve|reject|remove.
  // post_review: value = clear|flagged|unresolved. Facts: standing|attest|trust|thread|policy|tick.
  type Command = { op: str, actor: int, id: int, value: str, age: int, matches: bool, flag: str }
  type Decided = { review: str, reason: str }
  pure val writers = 1.to(3)
  pure val ids = 1.to(3)
  pure val trustMaxAge = 2 // ticks; the Rust replay maps one tick to TRUST_MAX_AGE_MICROS / 2
  pure val premoderation = "premoderation.v1"
  pure val trustedCohort = "trusted-cohort.v1"
  pure val blank =
    { writer: 0, review: "none", removed: false, version: 0, policy: "none", reason: "none" }
  pure val undecided = { review: "none", reason: "none" }
  pure def cmd(op: str, actor: int, id: int, value: str, age: int, matches: bool, flag: str): Command =
    { op: op, actor: actor, id: id, value: value, age: age, matches: matches, flag: flag }
  pure def fact(op: str, actor: int, value: str): Command = cmd(op, actor, 0, value, 0, false, "none")
  pure def send(actor: int, id: int, rules: str, age: int, matches: bool, flag: str): Command =
    cmd("submit", actor, id, rules, age, matches, flag)
  pure def clean(actor: int, id: int): Command = send(actor, id, "clear", 0, true, "unassessed")
  pure def act(op: str, actor: int, id: int, value: str): Command = cmd(op, actor, id, value, 0, false, "none")

  var comments: int -> Comment
  var standing: int -> str  // active | suspended | unknown
  var trust: int -> str     // unknown | not_trusted | revoked | trusted
  var attested: int -> int  // tick of the last attestation
  var thread: str           // standard | reviewed
  var effective: str
  var shadowing: bool       // trusted-cohort.v1 evaluated in shadow while premoderation is effective
  var now: int
  var tickets: int -> int   // owed post-review: comment -> version its admission produced
  var command: Command
  var outcome: str
  var reason: str           // stored reason or refusal of this submit, else "none"
  var shadow: str           // shadow reason of this submit, else "none"
  var previous: int -> Comment
  var previousTickets: int -> int
  var cursor: int

  def commentAt(m: int -> Comment, id: int): Comment = if (m.keys().contains(id)) m.get(id) else blank
  def ticketAt(m: int -> int, id: int): int = if (m.keys().contains(id)) m.get(id) else 0

  action init = all {
    comments' = Map(), standing' = writers.mapBy(_ => "active"), trust' = writers.mapBy(_ => "unknown"),
    attested' = writers.mapBy(_ => 0), thread' = "standard", effective' = premoderation,
    shadowing' = false, now' = 0, tickets' = Map(), command' = fact("init", 0, "none"),
    outcome' = "init", reason' = "none", shadow' = "none", previous' = Map(),
    previousTickets' = Map(), cursor' = 0,
  }

  // The production decision `decide_initial_review`, guard by guard in its order.
  def admit(policy: str, c: Command, checkAge: bool): Decided = {
    val s = standing.get(c.actor)
    val t = trust.get(c.actor)
    if (s == "suspended") { review: "refused", reason: "suspended" }
    else if (s != "active") { review: "refused", reason: "standing_unknown" }
    else if (policy == premoderation) { review: "pending", reason: "policy_premoderation" }
    else if (thread == "reviewed") { review: "pending", reason: "thread_reviewed" }
    else if (c.value == "flagged" or c.flag == "flagged") { review: "pending", reason: "content_flagged" }
    else if (t == "unknown") { review: "pending", reason: "trust_unknown" }
    else if (t == "not_trusted") { review: "pending", reason: "not_trusted" }
    else if (t == "revoked") { review: "pending", reason: "trust_revoked" }
    else if (checkAge and now - attested.get(c.actor) > trustMaxAge) { review: "pending", reason: "trust_stale" }
    else if (c.value == "unassessed") { review: "pending", reason: "content_unassessed" }
    else if (not(c.matches)) { review: "pending", reason: "content_mismatch" }
    else if (c.age != 0) { review: "pending", reason: "content_stale" }
    else { review: "approved", reason: "trusted_and_clear" }
  }

  action keepFacts = all {
    standing' = standing, trust' = trust, attested' = attested, thread' = thread,
    effective' = effective, shadowing' = shadowing, now' = now,
  }
  action record(c: Command, result: str): bool = all {
    command' = c, outcome' = result, previous' = comments, previousTickets' = tickets,
    cursor' = if (cursor < 60) cursor + 1 else cursor,
  }

  // applyShadow/checkAge are mutation switches for negative controls; production is (false, true).
  action submitWith(c: Command, applyShadow: bool, checkAge: bool): bool = {
    val decided = admit(effective, c, checkAge)
    val candidate = if (shadowing and effective == premoderation) admit(trustedCohort, c, checkAge) else undecided
    val stored = if (applyShadow and candidate != undecided) candidate else decided
    val created = stored.review != "refused"
    all {
      not(comments.keys().contains(c.id)), // the server generates a fresh identity
      comments' = if (created) comments.put(c.id, { writer: c.actor, review: stored.review,
        removed: false, version: 1, policy: effective, reason: stored.reason }) else comments,
      tickets' = if (created and stored.review == "approved") tickets.put(c.id, 1) else tickets,
      reason' = stored.reason, shadow' = candidate.reason,
      keepFacts, record(c, if (created) "created" else "refused"),
    }
  }

  // A human decision at the current version (ModerationAction::apply).
  action moderate(c: Command): bool = {
    val current = commentAt(comments, c.id)
    val review = if (c.value == "approve") "approved" else "rejected"
    val result = if (current.removed) "removed" else "changed"
    val updated = if (c.value == "remove") current.with("removed", true)
      else current.with("review", review)
    all {
      comments.keys().contains(c.id),
      comments' = if (result == "changed") comments.set(c.id, updated.with("version", current.version + 1)) else comments,
      tickets' = tickets, reason' = "none", shadow' = "none", keepFacts, record(c, result),
    }
  }

  // The writer withdraws their own comment (remove_by_author).
  action removeOwn(c: Command): bool = {
    val current = commentAt(comments, c.id)
    all {
      comments.keys().contains(c.id), current.writer == c.actor,
      comments' = if (current.removed) comments
        else comments.set(c.id, current.with("removed", true).with("version", current.version + 1)),
      tickets' = tickets, reason' = "none", shadow' = "none", keepFacts,
      record(c, if (current.removed) "unchanged" else "changed"),
    }
  }

  // decide_post_review; the job is acknowledged whatever the outcome. `fenced` is a mutation switch.
  action postReviewWith(c: Command, fenced: bool): bool = {
    val current = commentAt(comments, c.id)
    val result = if (current.removed) "removed"
      else if (fenced and current.version != ticketAt(tickets, c.id)) "stale"
      else if (c.value == "clear") "kept"
      else if (current.review == "approved") "held"
      else "already_hidden"
    all {
      tickets.keys().contains(c.id),
      comments' = if (result == "held")
        comments.set(c.id, current.with("review", "pending").with("version", current.version + 1)) else comments,
      tickets' = tickets.keys().exclude(Set(c.id)).mapBy(id => tickets.get(id)),
      reason' = "none", shadow' = "none", keepFacts, record(c, result),
    }
  }

  // Synthetic environment inputs only: no trust authority, suspension or operator command exists.
  action environment(c: Command): bool = all {
    comments' = comments, tickets' = tickets, reason' = "none", shadow' = "none",
    standing' = if (c.op == "standing") standing.set(c.actor, c.value) else standing,
    trust' = if (c.op == "attest") trust.set(c.actor, "trusted")
      else if (c.op == "trust") trust.set(c.actor, c.value) else trust,
    attested' = if (c.op == "attest") attested.set(c.actor, now) else attested,
    thread' = if (c.op == "thread") c.value else thread,
    effective' = if (c.op == "policy") { if (c.value == "trusted_cohort") trustedCohort else premoderation }
      else effective,
    shadowing' = if (c.op == "policy") c.value == "shadow" else shadowing,
    now' = if (c.op == "tick") now + 1 else now,
    record(c, "facts_set"),
  }

  action dispatchWith(c: Command, applyShadow: bool, checkAge: bool, fenced: bool): bool =
    if (c.op == "submit") submitWith(c, applyShadow, checkAge)
    else if (c.op == "moderate") moderate(c)
    else if (c.op == "remove") removeOwn(c)
    else if (c.op == "post_review") postReviewWith(c, fenced)
    else environment(c)
  action dispatch(c: Command): bool = dispatchWith(c, false, true, true)

  action step = {
    nondet actor = writers.oneOf()
    nondet id = ids.oneOf()
    nondet rules = Set("unassessed", "clear", "flagged").oneOf()
    nondet flag = Set("unassessed", "clear", "flagged").oneOf()
    nondet age = Set(-1, 0, 1).oneOf()
    nondet matches = Set(true, false).oneOf()
    nondet decision = Set("approve", "reject", "remove").oneOf()
    nondet finding = Set("clear", "flagged", "unresolved").oneOf()
    nondet standingValue = Set("active", "suspended", "unknown").oneOf()
    nondet trustValue = Set("unknown", "not_trusted", "revoked").oneOf()
    nondet threadValue = Set("standard", "reviewed").oneOf()
    nondet policyValue = Set("premoderation", "shadow", "trusted_cohort").oneOf()
    any {
      dispatch(send(actor, id, rules, age, matches, flag)),
      dispatch(act("moderate", 0, id, decision)),
      dispatch(act("remove", commentAt(comments, id).writer, id, "none")),
      dispatch(act("post_review", 0, id, finding)),
      all { now < 6, dispatch(fact("tick", 0, "none")) },
      dispatch(fact("standing", actor, standingValue)),
      dispatch(fact("attest", actor, "none")),
      dispatch(fact("trust", actor, trustValue)),
      dispatch(fact("thread", 0, threadValue)),
      dispatch(fact("policy", 0, policyValue)),
    }
  }

  // Shadow measured, then publish-first, every hold reason, post-review outcomes, kill switch.
  pure val rollout = List(
    clean(1, 1),                                 // premoderation, no shadow
    fact("policy", 0, "shadow"),
    clean(2, 2),                                 // stored pending; shadow trust_unknown
    fact("attest", 2, "none"),
    clean(2, 3),                                 // stored pending; shadow would approve
    fact("policy", 0, "trusted_cohort"),
    clean(2, 4),                                 // approved, owes post-review
    send(2, 5, "clear", 0, false, "unassessed"), // rules assessed another payload
    send(2, 6, "clear", 1, true, "unassessed"),  // rules too old
    send(2, 7, "clear", -1, true, "unassessed"), // rules dated in the future
    send(2, 8, "unassessed", 0, true, "clear"),  // a clear classifier cannot clear
    send(2, 9, "clear", 0, true, "flagged"),     // a classifier flag holds
    clean(3, 10),                                // no attestation
    fact("trust", 3, "not_trusted"),
    clean(3, 11),
    fact("thread", 0, "reviewed"),
    clean(2, 12),
    fact("thread", 0, "standard"),
    act("post_review", 0, 4, "flagged"),          // held: back to pending at version 2
    clean(2, 13),
    act("post_review", 0, 13, "clear"),           // kept
    clean(2, 14),
    act("moderate", 0, 14, "reject"),             // a human acts first
    act("post_review", 0, 14, "flagged"),         // stale: the human decision wins
    clean(2, 15),
    act("remove", 2, 15, "none"),
    act("post_review", 0, 15, "unresolved"),      // removal is terminal
    act("moderate", 0, 4, "approve"),             // a human approves the held comment
    act("moderate", 0, 4, "approve"),             // accepted reaffirmation advances version
    act("moderate", 0, 15, "reject"),             // removed
    act("remove", 2, 15, "none"),                 // unchanged
    fact("tick", 0, "none"),
    fact("tick", 0, "none"),
    fact("tick", 0, "none"),
    clean(2, 16),                                // attestation from tick 0 is stale at tick 3
    fact("attest", 2, "none"),
    fact("trust", 2, "revoked"),
    clean(2, 17),
    fact("standing", 1, "suspended"),
    clean(1, 18),                                // refused, nothing written
    fact("standing", 1, "unknown"),
    clean(1, 18),
    fact("attest", 2, "none"),
    fact("policy", 0, "premoderation"),          // kill switch
    clean(2, 19),                                // pending again; 13 stays approved
  )
  action rolloutStep = all { cursor < rollout.length(), dispatch(rollout.nth(cursor)) }

  // Negative controls: one type-correct mutation each, on the shortest trace that reaches it.
  pure val shadowLeak = List(fact("policy", 0, "shadow"), fact("attest", 2, "none"), clean(2, 1))
  action mutatedShadowStep = all {
    cursor < shadowLeak.length(), dispatchWith(shadowLeak.nth(cursor), true, true, true),
  }
  pure val staleTrust = List(fact("policy", 0, "trusted_cohort"), fact("attest", 2, "none"),
    fact("tick", 0, "none"), fact("tick", 0, "none"), fact("tick", 0, "none"), clean(2, 1))
  action mutatedStaleTrustStep = all {
    cursor < staleTrust.length(), dispatchWith(staleTrust.nth(cursor), false, false, true),
  }
  pure val humanFirst = List(fact("policy", 0, "trusted_cohort"), fact("attest", 2, "none"),
    clean(2, 1), act("moderate", 0, 1, "reject"), act("moderate", 0, 1, "approve"),
    act("post_review", 0, 1, "flagged"))
  action mutatedUnfencedStep = all {
    cursor < humanFirst.length(), dispatchWith(humanFirst.nth(cursor), false, true, false),
  }

  val created = outcome == "created"
  val mine = commentAt(comments, command.id)
  val admissionRecordsProvenance = not(created) or all {
    not(previous.keys().contains(command.id)),
    comments.keys() == previous.keys().union(Set(command.id)),
    mine.writer == command.actor, mine.version == 1, not(mine.removed), mine.policy == effective,
    Set("pending", "approved").contains(mine.review),
    (mine.review == "approved") == (mine.reason == "trusted_and_clear"),
  }
  val refusalWritesNothing = command.op != "submit" or all {
    (outcome == "refused") == (standing.get(command.actor) != "active"),
    outcome != "refused" or (comments == previous and tickets == previousTickets),
  }
  val premoderationAdmitsPending = not(created) or effective != premoderation or
    (mine.review == "pending" and mine.reason == "policy_premoderation")
  val approvalNeedsEveryGuard = not(created and mine.review == "approved") or all {
    effective == trustedCohort, standing.get(command.actor) == "active", thread == "standard",
    trust.get(command.actor) == "trusted", now - attested.get(command.actor) <= trustMaxAge,
    command.value == "clear", command.matches, command.age == 0, command.flag != "flagged",
  }
  val shadowOnlyMeasures = shadow == "none" or
    (command.op == "submit" and shadowing and effective == premoderation)
  val ticketsOnlyForPublishFirst = tickets.keys().forall(id =>
    comments.keys().contains(id) and tickets.get(id) == 1 and comments.get(id).reason == "trusted_and_clear")
  // Post-review never approves, acts only at the observed version and touches one comment.
  val postReviewFenced = command.op != "post_review" or all {
    not(tickets.keys().contains(command.id)),
    if (outcome == "held") all {
      commentAt(previous, command.id).review == "approved",
      commentAt(previous, command.id).version == ticketAt(previousTickets, command.id),
      comments == previous.set(command.id, commentAt(previous, command.id).with("review", "pending")
        .with("version", commentAt(previous, command.id).version + 1)),
    } else comments == previous,
  }
  val removalTerminal = previous.keys().forall(id =>
    not(previous.get(id).removed) or comments.get(id) == previous.get(id))
  val scopedVersionedWrites = previous.keys().forall(id => {
    val before = previous.get(id)
    val after = comments.get(id)
    after == before or all {
      id == command.id, Set("moderate", "remove", "post_review").contains(command.op),
      after.version == before.version + 1, after.writer == before.writer,
      after.policy == before.policy, after.reason == before.reason,
    }
  })
  val finiteDomain = now <= 6 and comments.keys().forall(id => comments.get(id).version <= 40)
  val admissionSafe = admissionRecordsProvenance and refusalWritesNothing and
    premoderationAdmitsPending and approvalNeedsEveryGuard and shadowOnlyMeasures and
    ticketsOnlyForPublishFirst and postReviewFenced and removalTerminal and
    scopedVersionedWrites and finiteDomain
}
