// PRODUCT-DECISIONS-20261110, M6 — TARGET SPECIFICATION. Not implemented: no appeal, case,
// escalation or admin surface exists; lifecycle.rs leaves "appeal reopening" to a later reviewed
// transition. Nothing here is production conformance; see README.md.
//
// Objecting to a decision of the AI evaluator (USER-06) and escalation to the admin (USER-05):
// a case is bound to one decision version, a repeated request never multiplies cases, and five
// things stay distinct: a dialog was opened; a case was recorded; a notification was sent; the
// admin saw it; the admin resolved it. In this phase NO delivery channel is connected, so no
// case is ever "sent", and a recorded case proves nothing about the admin having received it.
// ASSUMPTIONS: one serialized commit per action; one piece of content (a revision of a post or
// of a comment) owned by principal 2; 3 is someone else; 7 the evaluator; 9 the admin.
module appeal {
  type Decision = { version: int, verdict: str, by: str, reason: str }
  type Case = { appellant: int, decision: int, state: str, notice: str, seen: bool }
  type Receipt = { decision: int, answer: int }
  type Command = { op: str, actor: int, key: int, decision: int, target: int, value: str, reason: str }
  type Snapshot = { decision: Decision, cases: int -> Case, receipts: int -> Receipt }
  pure val contentOwner = 2
  pure val evaluator = 7
  pure val admin = 9
  pure val maxCases = 4
  pure val maxVersion = 6
  pure val channel = "not_connected"
  pure def cmd(op: str, actor: int, key: int, decision: int, target: int, value: str, reason: str): Command =
    { op: op, actor: actor, key: key, decision: decision, target: target, value: value, reason: reason }
  pure def dialog(actor: int): Command = cmd("dialog", actor, 0, 0, 0, "none", "none")
  pure def appealOf(actor: int, key: int, decision: int): Command =
    cmd("appeal", actor, key, decision, 0, "none", "none")
  pure def escalate(decision: int): Command = cmd("escalate", 0, 0, decision, 0, "none", "none")
  pure def ai(observed: int, value: str): Command = cmd("ai_result", evaluator, 0, observed, 0, value, "policy")
  pure def resolve(actor: int, target: int, value: str, reason: str): Command =
    cmd("admin_resolve", actor, 0, 0, target, value, reason)

  var decision: Decision
  var cases: int -> Case
  var receipts: int -> Receipt   // the owner's request keys
  var answer: int                // the case a request was answered with (0 = none)
  var command: Command
  var outcome: str
  var previous: Snapshot
  var cursor: int

  val current: Snapshot = { decision: decision, cases: cases, receipts: receipts }
  // The evaluator rejected the content: the decision an owner may object to.
  pure val start: Snapshot = {
    decision: { version: 1, verdict: "rejected", by: "ai", reason: "policy" }, cases: Map(), receipts: Map(),
  }

  action init = all {
    decision' = start.decision, cases' = Map(), receipts' = Map(), answer' = 0,
    command' = cmd("init", 0, 0, 0, 0, "none", "none"), outcome' = "init", previous' = start, cursor' = 0,
  }
  action commit(c: Command, result: str, after: Snapshot, answered: int): bool = all {
    decision' = after.decision, cases' = after.cases, receipts' = after.receipts, answer' = answered,
    command' = c, outcome' = result, previous' = current,
    cursor' = if (cursor < 60) cursor + 1 else cursor,
  }
  action refuse(c: Command, result: str): bool = commit(c, result, current, 0)

  pure def openCase(cs: int -> Case, who: int, version: int): Set[int] =
    cs.keys().filter(id => cs.get(id).appellant == who and cs.get(id).decision == version and
      cs.get(id).state == "open")

  // The owner objects. `dedupe` and `delivers` are the mutation switches.
  action appealed(c: Command, dedupe: bool, delivers: bool): bool = {
    val id = cases.keys().size() + 1
    val existing = openCase(cases, c.actor, c.decision)
    // Someone else cannot tell whether there is anything to appeal.
    if (c.actor != contentOwner) refuse(c, "not_found")
    else if (dedupe and receipts.keys().contains(c.key))
      if (receipts.get(c.key).decision == c.decision) commit(c, "replayed", current, receipts.get(c.key).answer)
      else refuse(c, "key_reused")
    // A case names the decision it objects to; an objection to an older decision is refused.
    else if (c.decision != decision.version) refuse(c, "stale_decision")
    else if (decision.verdict == "approved") refuse(c, "nothing_to_appeal")
    else if (decision.verdict == "needs_admin") refuse(c, "awaiting_admin")
    else if (decision.by == "admin") refuse(c, "final_decision")
    else if (dedupe and existing.size() > 0) {
      val same = existing.fold(0, (found, x) => x)
      commit(c, "already_open",
        current.with("receipts", receipts.put(c.key, { decision: c.decision, answer: same })), same)
    }
    else if (id > maxCases) refuse(c, "limit")
    // Recorded durably, and honestly: nothing was sent to anyone.
    else commit(c, "recorded_not_delivered", current
      .with("cases", cases.put(id, { appellant: c.actor, decision: c.decision, state: "open",
        notice: if (delivers) "sent" else "not_sent", seen: false }))
      .with("receipts", receipts.put(c.key, { decision: c.decision, answer: id })), id)
  }

  // The evaluator could not decide: the App records that an admin is needed. The flag is a
  // state of the case, not evidence that anybody looked at it.
  action escalated(c: Command): bool = {
    val id = cases.keys().size() + 1
    val existing = openCase(cases, 0, c.decision)
    if (c.decision != decision.version or decision.verdict != "needs_admin") refuse(c, "not_needed")
    else if (existing.size() > 0) commit(c, "already_open", current, existing.fold(0, (found, x) => x))
    else if (id > maxCases) refuse(c, "limit")
    else commit(c, "escalated", current.with("cases", cases.put(id,
      { appellant: 0, decision: c.decision, state: "open", notice: "not_sent", seen: false })), id)
  }

  // A later evaluator result about the same content (a retry or re-evaluation).
  action aiResult(c: Command): bool =
    if (c.actor != evaluator) refuse(c, "not_evaluator")
    else if (decision.by == "admin") refuse(c, "overridden")
    else if (c.decision != decision.version) refuse(c, "stale")
    else if (decision.version >= maxVersion) refuse(c, "limit")
    else commit(c, "decided", current.with("decision",
      { version: decision.version + 1, verdict: c.value, by: "ai", reason: "policy" }), 0)

  pure def closeCase(cs: int -> Case, id: int, state: str): int -> Case =
    cs.setBy(id, x => x.with("state", state).with("seen", true))

  // The admin resolves one case. An override always carries who decided and why.
  action resolved(c: Command): bool = {
    val known = cases.keys().contains(c.target)
    val target = if (known) cases.get(c.target)
      else { appellant: 0, decision: 0, state: "none", notice: "none", seen: false }
    if (c.actor != admin) refuse(c, "not_admin")
    else if (not(known)) refuse(c, "unknown_case")
    else if (target.state != "open") refuse(c, "already_resolved")
    else if (c.reason == "none") refuse(c, "reason_required")
    // The decision moved on since the case was opened: close the case, change nothing else.
    else if (target.decision != decision.version)
      commit(c, "case_superseded", current.with("cases", closeCase(cases, c.target, "superseded")), c.target)
    else if (decision.version >= maxVersion) refuse(c, "limit")
    else commit(c, if (c.value == "overturn") "overturned" else "upheld", current
      .with("cases", closeCase(cases, c.target, if (c.value == "overturn") "overturned" else "upheld"))
      .with("decision", { version: decision.version + 1, by: "admin", reason: c.reason,
        verdict: if (c.value == "overturn") "approved" else "rejected" }), c.target)
  }

  action dispatchWith(c: Command, dedupe: bool, delivers: bool): bool =
    if (c.op == "dialog") refuse(c, "dialog_shown")
    else if (c.op == "appeal") appealed(c, dedupe, delivers)
    else if (c.op == "escalate") escalated(c)
    else if (c.op == "ai_result") aiResult(c)
    else resolved(c)
  action dispatch(c: Command): bool = dispatchWith(c, true, false)

  action step = {
    nondet who = Set(contentOwner, 3).oneOf()
    nondet judge = Set(admin, evaluator, contentOwner).oneOf()
    nondet k = 1.to(4).oneOf()
    nondet v = Set(decision.version, decision.version - 1).oneOf()
    nondet target = 1.to(maxCases).oneOf()
    nondet verdict = Set("approved", "rejected", "needs_admin").oneOf()
    nondet ruling = Set("overturn", "uphold").oneOf()
    nondet why = Set("none", "context").oneOf()
    any {
      dispatch(dialog(who)), dispatch(appealOf(who, k, v)), dispatch(escalate(v)),
      dispatch(ai(v, verdict)), dispatch(resolve(judge, target, ruling, why)),
    }
  }

  // Dialog, record, retry, duplicate, a decision that moves on, escalation, an override with a
  // reason, and a late evaluator result that cannot undo it.
  pure val overturnScenario = List(
    dialog(contentOwner),              // dialog_shown: opening the dialog records nothing
    appealOf(3, 1, 1),                 // not_found
    appealOf(contentOwner, 1, 1),      // recorded_not_delivered: case 1
    appealOf(contentOwner, 1, 1),      // replayed: the lost acknowledgement
    appealOf(contentOwner, 1, 2),      // key_reused
    appealOf(contentOwner, 2, 1),      // already_open: a second request, still one case
    resolve(evaluator, 1, "overturn", "context"), // not_admin
    resolve(admin, 1, "overturn", "none"),        // reason_required
    resolve(admin, 3, "overturn", "context"),     // unknown_case
    escalate(1),                       // not_needed: the decision is a rejection, not "needs admin"
    ai(1, "needs_admin"),              // decided: version 2 (a re-evaluation could not decide)
    appealOf(contentOwner, 3, 1),      // stale_decision
    appealOf(contentOwner, 3, 2),      // awaiting_admin
    escalate(2),                       // escalated: case 2, not sent, not seen
    escalate(2),                       // already_open
    resolve(admin, 1, "overturn", "context"),     // case_superseded: case 1 named version 1
    resolve(admin, 2, "overturn", "context"),     // overturned: version 3, approved by the admin
    resolve(admin, 2, "uphold", "context"),       // already_resolved
    ai(2, "rejected"),                 // overridden: the late result loses
    ai(3, "rejected"),                 // overridden
    appealOf(contentOwner, 4, 3),      // nothing_to_appeal
  )
  action overturnStep = all { cursor < overturnScenario.length(), dispatch(overturnScenario.nth(cursor)) }

  // The admin upholds the rejection: that decision is final in this phase.
  pure val upholdScenario = List(
    appealOf(contentOwner, 1, 1),
    ai(0, "approved"),                 // stale
    resolve(admin, 1, "uphold", "context"),       // upheld: version 2, rejected by the admin
    appealOf(contentOwner, 2, 2),      // final_decision
    ai(2, "approved"),                 // overridden
  )
  action upholdStep = all { cursor < upholdScenario.length(), dispatch(upholdScenario.nth(cursor)) }

  // Negative control 1: without the receipt and the open-case check, a retry opens a second case.
  pure val duplicateAppeal = List(appealOf(contentOwner, 1, 1), appealOf(contentOwner, 1, 1))
  action mutatedDuplicateStep = all {
    cursor < duplicateAppeal.length(), dispatchWith(duplicateAppeal.nth(cursor), false, false),
  }
  // Negative control 2: a mock that reports the notification as sent.
  pure val fakeDelivery = List(dialog(contentOwner), appealOf(contentOwner, 1, 1))
  action mutatedDeliveryStep = all {
    cursor < fakeDelivery.length(), dispatchWith(fakeDelivery.nth(cursor), true, true),
  }

  pure val refusals = Set("dialog_shown", "not_found", "key_reused", "stale_decision", "nothing_to_appeal",
    "awaiting_admin", "final_decision", "limit", "not_needed", "not_evaluator", "overridden", "stale",
    "not_admin", "unknown_case", "already_resolved", "reason_required")

  // MOD-04: one open case per (appellant, decision), whatever the number of requests.
  val oneOpenCasePerDecision = cases.keys().forall(a => cases.keys().forall(b =>
    a == b or cases.get(a).state != "open" or cases.get(b).state != "open" or
    cases.get(a).appellant != cases.get(b).appellant or cases.get(a).decision != cases.get(b).decision))
  val caseNamesARealDecision = cases.keys().forall(id =>
    cases.get(id).decision >= 1 and cases.get(id).decision <= decision.version)
  // No channel is connected: nothing is ever reported as sent.
  val nothingIsDelivered = channel == "not_connected" and
    cases.keys().forall(id => cases.get(id).notice == "not_sent")
  // "Seen" comes only from the admin acting on the case, never from recording or flagging it.
  val seenOnlyByAdminAction = cases.keys().forall(id => not(cases.get(id).seen) or
    (previous.cases.keys().contains(id) and previous.cases.get(id).seen) or
    (command.op == "admin_resolve" and command.actor == admin and command.target == id))
  // USER-14: an admin decision is changed only by the admin.
  val adminDecisionIsFinalForAi = previous.decision.by != "admin" or decision == previous.decision or
    command.op == "admin_resolve"
  val overrideHasProvenance = decision.by != "admin" or decision.reason != "none"
  val refusalWritesNothing = not(refusals.contains(outcome)) or current == previous
  val resolutionIsTerminal = previous.cases.keys().forall(id =>
    previous.cases.get(id).state == "open" or cases.get(id) == previous.cases.get(id))
  val receiptsAreStable = previous.receipts.keys().forall(k =>
    receipts.keys().contains(k) and receipts.get(k) == previous.receipts.get(k))
  val versionMonotone = decision.version == previous.decision.version or
    decision.version == previous.decision.version + 1
  val finiteDomain = cases.keys().size() <= maxCases and decision.version <= maxVersion
  val appealSafe = oneOpenCasePerDecision and caseNamesARealDecision and nothingIsDelivered and
    seenOnlyByAdminAction and adminDecisionIsFinalForAi and overrideHasProvenance and
    refusalWritesNothing and resolutionIsTerminal and receiptsAreStable and versionMonotone and
    finiteDomain
}
