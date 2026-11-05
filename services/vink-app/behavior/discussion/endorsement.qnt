// LVE (#73), policy lve-core-v1: literary endorsement of a public work, a root comment and a
// flat reply, as desired state behind an accepted-intent version. No I/O.
// One work by principal 1 with revisions 1 and 2; published 0 means no public publication.
// Target 0 is the work, target 1 a root comment by 2, target 2 a flat reply to it by 3.
// Clients and the network are a labelled synthetic shell: a stream (tab/device; principal 3 has
// streams 3 and 4) issues a request at the version it last learned, the network delivers
// requests in any order, may deliver one again (retry/duplicate: `keep`) and may lose the answer
// (`heard`). Each delivery is one serialized commit; the row locks that serialize it with
// publication, moderation and the mode change are an assumption (persistence is #73 PR3).
// Comment lifecycle and mode authority belong to lifecycle.qnt: a synthetic moderator approves
// or removes, the author's mode change is current. No publish/withdraw command exists yet
// (PUB-01..03): `publish`/`withdraw` only move the publication pointer.
module endorsement {
  type Row = { sel: int, version: int }
  type Request = { stamp: int, stream: int, actor: int, target: int, expected: int,
                   desire: str, observed: int }
  // op: issue | deliver | read | publish | withdraw | approve | remove | set_mode
  type Command = { op: str, stamp: int, stream: int, actor: int, target: int, expected: int,
                   desire: str, observed: int, keep: bool, heard: bool, value: str }

  pure val POLICY = "lve-core-v1"
  pure val WORK_AUTHOR = 1
  pure val principals = 1.to(3)
  pure val targets = 0.to(2)
  pure val commentIds = 1.to(2)
  pure val revisions = 1.to(2)
  pure val streams = 1.to(4)
  pure def actorOf(s: int): int = if (s == 4) 3 else s
  pure def writer(t: int): int = if (t == 1) 2 else if (t == 2) 3 else WORK_AUTHOR
  pure val pairs = tuples(principals, targets)
  pure val streamTargets = tuples(streams, targets)
  // (target, anchor): a work is counted per revision, a comment at anchor 1 (itself).
  pure val anchors = Set((0, 1), (0, 2), (1, 1), (2, 1))
  // An actor who never acted: no selection, version 1 (`Endorsement::ABSENT`).
  pure val ABSENT = { sel: 0, version: 1 }
  pure val NO_REQUEST = { stamp: 0, stream: 0, actor: 0, target: 0, expected: 0, desire: "",
                          observed: 0 }

  var published: int
  var status: int -> str
  var mode: str
  // (actor, target) -> stored endorsement; sel is the revision for the work, 1 for a comment.
  var rows: (int, int) -> Row
  var counts: (int, int) -> int
  var inflight: Set[Request]
  // Client knowledge: (stream, target) -> highest version the stream learned.
  var known: (int, int) -> int
  // History for the ordering property: (stream, target) -> latest stamp the server confirmed.
  var confirmed: (int, int) -> int
  var nextStamp: int
  // Exported projections for the Rust replay: -1 is an unavailable summary.
  var summary: int -> int
  var views: (int, int) -> str
  var previousPublished: int
  var previousStatus: int -> str
  var previousMode: str
  var previousRows: (int, int) -> Row
  var previousCounts: (int, int) -> int
  var previousConfirmed: (int, int) -> int
  var command: Command
  var request: Request
  var outcome: str
  var answer: int
  // Only scripted scenarios use cursor; saturating it keeps the state space finite.
  var cursor: int

  pure def visibleOf(p: int, st: int -> str, t: int): bool =
    p != 0 and (t == 0 or st.get(t) == "public")
  pure def anchorOf(p: int, t: int): int = if (t == 0) p else 1
  pure def summaryOf(p: int, st: int -> str, cs: (int, int) -> int): int -> int =
    targets.mapBy(t => if (visibleOf(p, st, t)) cs.get((t, anchorOf(p, t))) else -1)
  pure def viewOf(p: int, st: int -> str, row: Row, t: int): str =
    if (not(visibleOf(p, st, t))) "unavailable"
    else if (row.sel == 0) "none"
    else if (row.sel == anchorOf(p, t)) "endorsed"
    else "earlier_revision"
  pure def viewsOf(p: int, st: int -> str, rs: (int, int) -> Row): (int, int) -> str =
    pairs.mapBy(pr => viewOf(p, st, rs.get(pr), pr._2))

  // decide_work_endorsement / decide_comment_endorsement, then Endorsement::transition.
  // fence and observe are false ONLY in the negative-control scenarios below.
  pure def decide(r: Request, row: Row, p: int, st: int -> str, m: str,
                  fence: bool, observe: bool): (str, Row) = {
    val wanted = if (r.desire == "clear") 0 else anchorOf(p, r.target)
    if (not(visibleOf(p, st, r.target))) ("target_unavailable", row)
    else if (r.actor == writer(r.target)) ("own_content", row)
    else if (observe and r.target == 0 and r.desire == "select" and r.observed != p)
      ("revision_changed", row)
    else if (r.target != 0 and wanted != 0 and wanted != row.sel and m != "open")
      ("comments_closed", row)
    else if (r.expected == row.version and (fence or wanted != row.sel))
      (if (wanted != row.sel) "changed" else "reaffirmed", { sel: wanted, version: row.version + 1 })
    else if (wanted == row.sel) ("already_current", row)
    else ("stale", row)
  }

  // EndorsementCount::apply on the removed and the added anchor.
  pure def recount(cs: (int, int) -> int, t: int, before: Row, after: Row): (int, int) -> int = {
    val removed = if (before.sel != 0 and before.sel != after.sel)
      cs.set((t, before.sel), cs.get((t, before.sel)) - 1) else cs
    if (after.sel != 0 and after.sel != before.sel)
      removed.set((t, after.sel), removed.get((t, after.sel)) + 1)
    else removed
  }

  pure def asCommand(op: str, r: Request, keep: bool, heard: bool, value: str): Command =
    { op: op, stamp: r.stamp, stream: r.stream, actor: r.actor, target: r.target,
      expected: r.expected, desire: r.desire, observed: r.observed, keep: keep, heard: heard,
      value: value }
  pure def adminOf(op: str, target: int, observed: int, value: str): Command =
    asCommand(op, { ...NO_REQUEST, target: target, observed: observed }, false, false, value)
  pure def issueOf(s: int, t: int, desire: str, observed: int): Command =
    asCommand("issue", { ...NO_REQUEST, stream: s, target: t, desire: desire,
      observed: observed }, false, false, "")
  pure def deliverOf(stamp: int, keep: bool, heard: bool): Command =
    asCommand("deliver", { ...NO_REQUEST, stamp: stamp }, keep, heard, "")
  pure def readOf(s: int, t: int): Command =
    asCommand("read", { ...NO_REQUEST, stream: s, target: t }, false, false, "")

  action init = all {
    published' = 0,
    status' = commentIds.mapBy(_ => "pending"),
    mode' = "open",
    rows' = pairs.mapBy(_ => ABSENT),
    counts' = anchors.mapBy(_ => 0),
    inflight' = Set(),
    known' = streamTargets.mapBy(_ => 1),
    confirmed' = streamTargets.mapBy(_ => 0),
    nextStamp' = 1,
    summary' = targets.mapBy(_ => -1),
    views' = pairs.mapBy(_ => "unavailable"),
    previousPublished' = 0,
    previousStatus' = commentIds.mapBy(_ => "pending"),
    previousMode' = "open",
    previousRows' = pairs.mapBy(_ => ABSENT),
    previousCounts' = anchors.mapBy(_ => 0),
    previousConfirmed' = streamTargets.mapBy(_ => 0),
    command' = adminOf("init", 0, 0, ""),
    request' = NO_REQUEST,
    outcome' = "init",
    answer' = 0,
    cursor' = 0,
  }

  action commit(c: Command, r: Request, result: str, ans: int, p: int, st: int -> str, m: str,
                rs: (int, int) -> Row, cs: (int, int) -> int, fl: Set[Request],
                kn: (int, int) -> int, cf: (int, int) -> int, ns: int): bool = all {
    published' = p,
    status' = st,
    mode' = m,
    rows' = rs,
    counts' = cs,
    inflight' = fl,
    known' = kn,
    confirmed' = cf,
    nextStamp' = ns,
    summary' = summaryOf(p, st, cs),
    views' = viewsOf(p, st, rs),
    previousPublished' = published,
    previousStatus' = status,
    previousMode' = mode,
    previousRows' = rows,
    previousCounts' = counts,
    previousConfirmed' = confirmed,
    command' = c,
    request' = r,
    outcome' = result,
    answer' = ans,
    cursor' = if (cursor < 60) cursor + 1 else cursor,
  }

  action admin(c: Command, result: str, p: int, st: int -> str, m: str): bool =
    commit(c, NO_REQUEST, result, 0, p, st, m, rows, counts, inflight, known, confirmed,
      nextStamp)

  // A stream sends its desire at the version it last learned. Inputs only; nothing is decided.
  action issue(s: int, t: int, desire: str, observed: int): bool = {
    val r = { stamp: nextStamp, stream: s, actor: actorOf(s), target: t,
              expected: known.get((s, t)), desire: desire,
              observed: if (t == 0) observed else 0 }
    commit(asCommand("issue", r, false, false, ""), r, "issued", 0, published, status, mode,
      rows, counts, inflight.union(Set(r)), known, confirmed, nextStamp + 1)
  }

  // The server commits one request. retain is false ONLY for the version-reset control, which
  // deletes a cleared row instead of keeping its version.
  action deliverWith(r: Request, keep: bool, heard: bool,
                     fence: bool, retain: bool, observe: bool): bool = {
    val pair = (r.actor, r.target)
    val row = rows.get(pair)
    val decided = decide(r, row, published, status, mode, fence, observe)
    val result = decided._1
    val accepted = result == "changed" or result == "reaffirmed"
    val stored = if (not(retain) and accepted and decided._2.sel == 0) ABSENT else decided._2
    val ans = if (accepted) stored.version
      else if (result == "already_current" or result == "stale") row.version
      else 0
    val sk = (r.stream, r.target)
    commit(asCommand("deliver", r, keep, heard, ""), r, result, ans, published, status, mode,
      if (accepted) rows.set(pair, stored) else rows,
      if (accepted) recount(counts, r.target, row, stored) else counts,
      if (keep) inflight else inflight.exclude(Set(r)),
      if (heard and ans > known.get(sk)) known.set(sk, ans) else known,
      if ((accepted or result == "already_current") and r.stamp > confirmed.get(sk))
        confirmed.set(sk, r.stamp)
      else confirmed,
      nextStamp)
  }

  pure def pick(fl: Set[Request], stamp: int): Request =
    fl.fold(NO_REQUEST, (found, r) => if (r.stamp == stamp) r else found)

  action deliverStamp(stamp: int, keep: bool, heard: bool,
                      fence: bool, retain: bool, observe: bool): bool = {
    val r = pick(inflight, stamp)
    all { r != NO_REQUEST, deliverWith(r, keep, heard, fence, retain, observe) }
  }

  // Readback after a lost answer: the stream learns its current version; a hidden target
  // answers nothing.
  action read(s: int, t: int): bool = {
    val row = rows.get((actorOf(s), t))
    val sk = (s, t)
    val visible = visibleOf(published, status, t)
    commit(asCommand("read", { ...NO_REQUEST, stream: s, actor: actorOf(s), target: t }, false,
        visible, ""),
      NO_REQUEST, if (visible) "read" else "read_hidden", if (visible) row.version else 0,
      published, status, mode, rows, counts, inflight,
      if (visible and row.version > known.get(sk)) known.set(sk, row.version) else known,
      confirmed, nextStamp)
  }

  action publish(rev: int): bool =
    admin(adminOf("publish", 0, rev, ""), if (published == rev) "noop" else "published", rev,
      status, mode)
  action withdraw: bool =
    admin(adminOf("withdraw", 0, 0, ""), if (published == 0) "noop" else "withdrawn", 0,
      status, mode)
  action approve(c: int): bool = {
    val result = if (status.get(c) == "pending") "moderated" else "noop"
    admin(adminOf("approve", c, 0, ""), result, published,
      if (result == "moderated") status.set(c, "public") else status, mode)
  }
  action remove(c: int): bool = {
    val result = if (status.get(c) != "removed") "moderated" else "noop"
    admin(adminOf("remove", c, 0, ""), result, published,
      if (result == "moderated") status.set(c, "removed") else status, mode)
  }
  action setMode(m: str): bool =
    admin(adminOf("set_mode", 0, 0, m), if (m == mode) "noop" else "mode_set", published,
      status, m)

  action step = {
    nondet s = streams.oneOf()
    nondet t = targets.oneOf()
    nondet desire = Set("select", "clear").oneOf()
    nondet observed = revisions.oneOf()
    nondet c = commentIds.oneOf()
    nondet rev = revisions.oneOf()
    nondet keep = Bool.oneOf()
    nondet heard = Bool.oneOf()
    nondet m = Set("open", "closed").oneOf()
    nondet r = inflight.union(Set(NO_REQUEST)).oneOf()
    any {
      issue(s, t, desire, observed),
      all { r != NO_REQUEST, deliverWith(r, keep, heard, true, true, true) },
      all { r != NO_REQUEST, deliverWith(r, false, true, true, true, true) },
      read(s, t),
      publish(rev),
      withdraw,
      approve(c),
      remove(c),
      setMode(m),
    }
  }

  action dispatch(c: Command, fence: bool, retain: bool, observe: bool): bool =
    if (c.op == "issue") issue(c.stream, c.target, c.desire, c.observed)
    else if (c.op == "deliver") deliverStamp(c.stamp, c.keep, c.heard, fence, retain, observe)
    else if (c.op == "read") read(c.stream, c.target)
    else if (c.op == "publish") publish(c.observed)
    else if (c.op == "withdraw") withdraw
    else if (c.op == "approve") approve(c.target)
    else if (c.op == "remove") remove(c.target)
    else setMode(c.value)

  action scripted(script: List[Command], fence: bool, retain: bool, observe: bool): bool =
    all { cursor < script.length(), dispatch(script.nth(cursor), fence, retain, observe) }

  // Inputs only: outcomes and states are always computed by the actions above. Stamps are
  // assigned in issue order.
  pure val reorderAndRetry = [
    adminOf("publish", 0, 1, ""),
    issueOf(3, 0, "select", 1), // stamp 1: A = endorse@1, created first, delayed
    issueOf(3, 0, "clear", 1), // stamp 2: B = clear@1, the newer intent
    deliverOf(2, false, true), // B arrives first: reaffirmed, the version moves to 2
    deliverOf(1, false, true), // A arrives last: stale, it cannot revive the older intent
    issueOf(3, 0, "select", 1), // stamp 3 at version 2
    deliverOf(3, true, false), // changed; the answer is lost and the request stays in flight
    readOf(3, 0), // readback reconciles the client
    deliverOf(3, false, true), // the duplicate: already current, no second effect
    issueOf(3, 0, "clear", 1), // stamp 4
    deliverOf(4, true, true), // changed
    deliverOf(4, false, true), // retry after the removal: already current
    issueOf(4, 0, "select", 1), // stamp 5: principal 3's other device with an old view
    deliverOf(5, false, true), // stale: no claim about the latest intent across devices
    issueOf(2, 0, "select", 1), // stamp 6
    deliverOf(6, true, true), // changed, duplicate kept
    issueOf(2, 0, "clear", 1), // stamp 7
    deliverOf(7, false, true), // changed; the cleared row keeps version 3
    deliverOf(6, false, true), // the old duplicate after the removal: stale, no ABA
    issueOf(1, 0, "select", 1), // stamp 8: the work's author
    deliverOf(8, false, true), // own content
  ]
  pure val revisionAnchor = [
    adminOf("publish", 0, 1, ""),
    issueOf(2, 0, "select", 1), // stamp 1
    deliverOf(1, false, true), // changed: counted at revision 1
    issueOf(3, 0, "select", 1), // stamp 2: reader 3 read revision 1
    adminOf("publish", 0, 2, ""), // principal 2's endorsement is now of an earlier revision
    deliverOf(2, false, true), // revision changed: nothing moves to revision 2 silently
    issueOf(2, 0, "select", 1), // stamp 3: the old page again
    deliverOf(3, false, true), // revision changed
    issueOf(2, 0, "select", 2), // stamp 4
    deliverOf(4, false, true), // reconfirmed: one count moves from revision 1 to 2
    issueOf(3, 0, "select", 2), // stamp 5
    deliverOf(5, false, true), // changed
    adminOf("withdraw", 0, 0, ""), // no count, no own state
    issueOf(3, 0, "clear", 2), // stamp 6
    deliverOf(6, true, true), // target unavailable, even for clearing
    readOf(3, 0), // hidden: answers nothing
    adminOf("publish", 0, 1, ""), // republishing revision 1: both selections are earlier
    adminOf("publish", 0, 2, ""), // revision 2 again: effective with no write
    deliverOf(6, false, true), // the retried clear now applies
    issueOf(1, 0, "clear", 2), // stamp 7: the author
    deliverOf(7, false, true), // own content
  ]
  pure val commentTargets = [
    adminOf("publish", 0, 1, ""),
    issueOf(3, 1, "select", 0), // stamp 1
    deliverOf(1, false, true), // the root is pending: unavailable
    adminOf("approve", 1, 0, ""),
    adminOf("approve", 2, 0, ""),
    issueOf(2, 1, "select", 0), // stamp 2: the root's writer
    deliverOf(2, false, true), // own content
    issueOf(3, 1, "select", 0), // stamp 3: the reply's writer endorses the root
    deliverOf(3, false, true), // changed
    issueOf(1, 2, "select", 0), // stamp 4: the work's author endorses the reply
    deliverOf(4, false, true), // changed; the root's count does not move
    adminOf("set_mode", 0, 0, "closed"),
    issueOf(2, 2, "select", 0), // stamp 5
    deliverOf(5, false, true), // comments closed: no new endorsement
    issueOf(1, 2, "select", 0), // stamp 6 at the current version
    deliverOf(6, false, true), // reaffirmed: closing does not block a held selection
    issueOf(3, 1, "clear", 0), // stamp 7
    deliverOf(7, false, true), // clearing while closed
    issueOf(3, 0, "select", 1), // stamp 8: the work is not governed by comments mode
    deliverOf(8, false, true), // changed
    adminOf("set_mode", 0, 0, "open"),
    issueOf(2, 2, "select", 0), // stamp 9
    deliverOf(9, false, true), // changed
    adminOf("remove", 2, 0, ""), // the reply is hidden; its selections stay stored
    issueOf(1, 2, "clear", 0), // stamp 10
    deliverOf(10, false, true), // target unavailable
    adminOf("remove", 1, 0, ""),
    adminOf("withdraw", 0, 0, ""),
  ]
  // Negative controls: the same kind of inputs through deliberately weakened decisions.
  pure val unfenced = [
    adminOf("publish", 0, 1, ""),
    issueOf(3, 0, "select", 1),
    issueOf(3, 0, "clear", 1),
    deliverOf(2, false, true), // without the fence the newer intent is a mere no-op
    deliverOf(1, false, true), // and the older one is applied after it
  ]
  pure val versionReset = [
    adminOf("publish", 0, 1, ""),
    issueOf(2, 0, "select", 1),
    deliverOf(1, true, true),
    issueOf(2, 0, "clear", 1),
    deliverOf(2, false, true), // deleting the cleared row restarts its version
    deliverOf(1, false, true), // so the old duplicate applies again (ABA)
  ]
  pure val silentTransfer = [
    adminOf("publish", 0, 1, ""),
    issueOf(2, 0, "select", 1),
    adminOf("publish", 0, 2, ""),
    deliverOf(1, false, true), // without the observed revision it lands on revision 2
  ]

  action reorderStep = scripted(reorderAndRetry, true, true, true)
  action anchorStep = scripted(revisionAnchor, true, true, true)
  action commentStep = scripted(commentTargets, true, true, true)
  action mutatedUnfencedStep = scripted(unfenced, false, true, true)
  action mutatedResetStep = scripted(versionReset, true, false, true)
  action mutatedTransferStep = scripted(silentTransfer, true, true, false)

  val wrote = command.op == "deliver" and (outcome == "changed" or outcome == "reaffirmed")
  val key = (request.actor, request.target)

  // LVE-INV-04: only an accepted request writes, and only its own row and its target's counts;
  // a delivery never changes publication, moderation or mode.
  val scopedWrites =
    pairs.forall(p => rows.get(p) == previousRows.get(p) or (wrote and p == key)) and
    anchors.forall(a => counts.get(a) == previousCounts.get(a) or
      (wrote and a._1 == request.target)) and
    (command.op == "deliver" implies
      (published == previousPublished and status == previousStatus and mode == previousMode))

  // LVE-INV-05: an accepted request carried the stored version and advances it by exactly one;
  // a reaffirmation moves only the version, so it changes no count.
  val acceptedVersionFence =
    pairs.forall(p => rows.get(p) == previousRows.get(p) or (
      request.expected == previousRows.get(p).version and
      rows.get(p).version == previousRows.get(p).version + 1)) and
    (outcome == "reaffirmed" implies
      (rows.get(key).sel == previousRows.get(key).sel and counts == previousCounts))

  // LVE-FR-03, LVE-UC-05/06, LVE-INV-05: once the server confirmed a request of a stream (it
  // accepted it or found its desire already current), no request that stream issued earlier
  // takes effect afterwards, whatever the delivery order, duplicates or lost answers.
  val noRevival = not(wrote) or
    request.stamp > previousConfirmed.get((request.stream, request.target))

  // LVE-FR-04, LVE-UC-10: a work is only ever selected at the revision its reader read.
  val noSilentTransfer = not(wrote and request.target == 0 and rows.get(key).sel != 0) or
    rows.get(key).sel == request.observed

  // LVE-INV-02/06/12: a write needs a visible target at commit and an actor who is not its
  // author; a new comment selection also needs open comments (CHOICE-09 B).
  val eligibleWrites = not(wrote) or (
    visibleOf(previousPublished, previousStatus, request.target) and
    request.actor != writer(request.target) and
    (request.target == 0 or rows.get(key).sel == 0 or
      rows.get(key).sel == previousRows.get(key).sel or previousMode == "open"))

  // LVE-INV-03/07: each count is exactly the number of actors selecting that anchor.
  val countsMatchSelections = anchors.forall(a => counts.get(a) >= 0 and
    counts.get(a) == principals.filter(x => rows.get((x, a._1)).sel == a._2).size())

  // CHOICE-08 A: an author never holds a row for their own target, not even a cleared one.
  val noSelfEndorsement = targets.forall(t => rows.get((writer(t), t)) == ABSENT)

  // LVE-FR-02, LVE-INV-08: a hidden target shows no count and no own state (never zero); a
  // visible one shows the number of effective selections, earlier revisions excluded.
  val projectionsHideAndCountEffective = targets.forall(t =>
    if (visibleOf(published, status, t))
      summary.get(t) == principals.filter(a => views.get((a, t)) == "endorsed").size()
    else summary.get(t) == -1 and principals.forall(a => views.get((a, t)) == "unavailable"))

  val finiteDomain = Set(0, 1, 2).contains(published) and Set("open", "closed").contains(mode) and
    commentIds.forall(c => Set("pending", "public", "removed").contains(status.get(c))) and
    pairs.forall(p => rows.get(p).version >= 1 and
      (if (p._2 == 0) Set(0, 1, 2) else Set(0, 1)).contains(rows.get(p).sel))

  val endorsementSafe = scopedWrites and acceptedVersionFence and noRevival and
    noSilentTransfer and eligibleWrites and countsMatchSelections and noSelfEndorsement and
    projectionsHideAndCountEffective and finiteDomain
}
