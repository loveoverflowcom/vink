// PRODUCT-DECISIONS-20261110, M4 — TARGET SPECIFICATION. Not implemented: comments have no
// edit use case, no body revision and no "edited" flag today (lifecycle.rs keeps edits blocked
// under the old D-08), and no evaluator decides comments. Nothing here is production
// conformance; see README.md for the path into behavior/discussion with a Rust replay.
//
// One comment on a public work (USER-15, CHOICE-01 A, DESIGN_PROPOSED): the text people read
// (`shown`) and the edit waiting for review (`candidate`) are different revisions. An edit never
// overwrites the public text; it becomes public only through a decision bound to that revision.
// ASSUMPTIONS: one serialized commit per action; principal 1 wrote the work, 2 wrote the
// comment, 3 is a reader, 7 is the evaluator capability, 9 the admin. No clock, no SLA.
module comment_edit {
  type Rev = { body: int, decision: str, by: str }
  type Job = { rev: int, state: str }
  type Receipt = { expected: int, body: int }
  type Command = { op: str, actor: int, rev: int, job: int, expected: int, key: int, body: int, value: str }
  type Public = { body: int, edited: bool, helpful: int }
  type Snapshot = {
    revs: int -> Rev, jobs: int -> Job, shown: int, candidate: int, firstShown: int, mark: bool,
    version: int, removal: str, receipts: int -> Receipt, votes: Set[(int, int)], changes: int,
  }
  pure val workAuthor = 1
  pure val writer = 2
  pure val evaluator = 7
  pure val admin = 9
  pure val maxRevs = 4
  pure val maxJobs = 6
  pure def cmd(op: str, actor: int, rev: int, job: int, expected: int, key: int, body: int, value: str): Command =
    { op: op, actor: actor, rev: rev, job: job, expected: expected, key: key, body: body, value: value }
  pure def edit(actor: int, expected: int, key: int, body: int): Command =
    cmd("edit", actor, 0, 0, expected, key, body, "none")
  pure def ai(job: int, rev: int, value: str): Command = cmd("ai_result", evaluator, rev, job, 0, 0, 0, value)
  pure val requeue = cmd("reevaluate", 0, 0, 0, 0, 0, 0, "none")
  pure def decide(actor: int, rev: int, value: str): Command = cmd("admin_decide", actor, rev, 0, 0, 0, 0, value)
  pure def adminRemove(actor: int): Command = cmd("admin_remove", actor, 0, 0, 0, 0, 0, "none")
  pure def removeOwn(actor: int): Command = cmd("remove_own", actor, 0, 0, 0, 0, 0, "none")
  pure def endorse(actor: int): Command = cmd("endorse", actor, 0, 0, 0, 0, 0, "none")

  var revs: int -> Rev           // every text the writer submitted; a decision is per revision
  var jobs: int -> Job
  var shown: int                 // the revision other people read (0 = none)
  var candidate: int             // the newest revision still waiting for a decision (0 = none)
  var firstShown: int            // the first revision that was ever public
  var mark: bool                 // "Đã sửa": the public text is not the first public text
  var version: int               // accepted edit intents (fences an older tab)
  var removal: str               // "none" | "writer" | "admin"
  var receipts: int -> Receipt   // the writer's edit keys
  var votes: Set[(int, int)]     // (actor, revision) "Hữu ích"
  var changes: int               // ghost: how many times the public text changed
  var seen: Public               // the public projection after this commit
  var command: Command
  var outcome: str
  var previous: Snapshot
  var cursor: int

  val current: Snapshot = {
    revs: revs, jobs: jobs, shown: shown, candidate: candidate, firstShown: firstShown, mark: mark,
    version: version, removal: removal, receipts: receipts, votes: votes, changes: changes,
  }
  // The comment was submitted as revision 1 and waits for its first decision.
  pure val start: Snapshot = {
    revs: Map(1 -> { body: 1, decision: "pending", by: "none" }), jobs: Map(1 -> { rev: 1, state: "open" }),
    shown: 0, candidate: 1, firstShown: 0, mark: false, version: 1, removal: "none",
    receipts: Map(), votes: Set(), changes: 0,
  }
  // What other people get: the text, the "Đã sửa" mark and the "Hữu ích" count of the public
  // revision. The candidate is never part of it.
  pure def project(s: Snapshot): Public = {
    body: if (s.shown == 0) 0 else s.revs.get(s.shown).body,
    edited: s.shown != 0 and s.mark,
    helpful: s.votes.filter(v => v._2 == s.shown and s.shown != 0).size(),
  }

  action init = all {
    revs' = start.revs, jobs' = start.jobs, shown' = 0, candidate' = 1, firstShown' = 0,
    mark' = false, version' = 1, removal' = "none", receipts' = Map(), votes' = Set(), changes' = 0,
    seen' = project(start),
    command' = cmd("init", 0, 0, 0, 0, 0, 0, "none"), outcome' = "init", previous' = start, cursor' = 0,
  }
  action commit(c: Command, result: str, after: Snapshot): bool = all {
    revs' = after.revs, jobs' = after.jobs, shown' = after.shown, candidate' = after.candidate,
    firstShown' = after.firstShown, mark' = after.mark, version' = after.version,
    removal' = after.removal, receipts' = after.receipts, votes' = after.votes,
    changes' = after.changes, seen' = project(after), command' = c, outcome' = result,
    previous' = current,
    cursor' = if (cursor < 80) cursor + 1 else cursor,
  }
  action refuse(c: Command, result: str): bool = commit(c, result, current)

  pure def setRev(s: Snapshot, r: int, decision: str, by: str): Snapshot =
    s.with("revs", s.revs.setBy(r, x => x.with("decision", decision).with("by", by)))
  // The only way a revision becomes the public text: an approval of exactly that revision.
  pure def show(s: Snapshot, r: int): Snapshot =
    s.with("shown", r).with("candidate", if (s.candidate == r) 0 else s.candidate)
      .with("firstShown", if (s.firstShown == 0) r else s.firstShown)
      .with("mark", s.firstShown != 0 and s.firstShown != r).with("changes", s.changes + 1)

  // The writer edits. `overwrite` is the mutation switch: true puts the unreviewed text
  // straight into the public projection.
  action edited(c: Command, overwrite: bool): bool = {
    val id = revs.keys().size() + 1
    val job = jobs.keys().size() + 1
    val latest = if (candidate != 0) candidate else shown
    val receipt = { expected: c.expected, body: c.body }
    val fenced = current.with("version", version + 1).with("receipts", receipts.put(c.key, receipt))
    // COM-08: owning the work gives no right over someone else's comment.
    if (c.actor != writer) refuse(c, "not_comment_writer")
    else if (receipts.keys().contains(c.key))
      refuse(c, if (receipts.get(c.key) == receipt) "replayed" else "key_reused")
    else if (removal != "none") refuse(c, "target_unavailable")
    else if (c.expected != version) refuse(c, "stale")
    // Saving the same text is an accepted intent (it fences older tabs) and nothing else: no
    // revision, no evaluation, no "Đã sửa", no change of the public text or its time.
    else if (latest != 0 and revs.get(latest).body == c.body) commit(c, "reaffirmed", fenced)
    else if (id > maxRevs or job > maxJobs) refuse(c, "limit")
    else {
      val superseded = if (candidate == 0) fenced else setRev(fenced, candidate, "superseded", "writer")
      val added = superseded
        .with("revs", superseded.revs.put(id, { body: c.body, decision: "pending", by: "none" }))
        .with("jobs", jobs.put(job, { rev: id, state: "open" })).with("candidate", id)
      commit(c, "accepted", if (overwrite) added.with("shown", id) else added)
    }
  }

  action aiResult(c: Command): bool = {
    val known = jobs.keys().contains(c.job)
    val job = if (known) jobs.get(c.job) else { rev: 0, state: "none" }
    val closed = current.with("jobs", jobs.setBy(c.job, j => j.with("state", "applied")))
    if (c.actor != evaluator) refuse(c, "not_evaluator")
    else if (not(known)) refuse(c, "unknown_job")
    else if (job.state != "open") refuse(c, "duplicate")
    else if (job.rev != c.rev) refuse(c, "wrong_revision")
    else if (removal != "none") refuse(c, "target_gone")
    else if (revs.get(c.rev).by == "admin") refuse(c, "overridden")
    // A newer edit replaced this revision: its result decides nothing.
    else if (c.rev != candidate) refuse(c, "superseded")
    else if (c.value == "approve") commit(c, "published", show(setRev(closed, c.rev, "approved", "ai"), c.rev))
    // A rejected edit leaves the approved text in place.
    else if (c.value == "reject")
      commit(c, "rejected", setRev(closed, c.rev, "rejected", "ai").with("candidate", 0))
    else if (c.value == "uncertain" or c.value == "invalid_output")
      commit(c, "needs_admin", setRev(closed, c.rev, "needs_admin", "ai"))
    else commit(c, "retryable", current.with("jobs", jobs.setBy(c.job, j => j.with("state", "failed"))))
  }

  action reevaluate(c: Command): bool = {
    val job = jobs.keys().size() + 1
    val open = jobs.keys().exists(j => jobs.get(j).rev == candidate and jobs.get(j).state == "open")
    if (removal != "none" or candidate == 0 or revs.get(candidate).decision != "pending" or open)
      refuse(c, "not_needed")
    else if (job > maxJobs) refuse(c, "limit")
    else commit(c, "requeued", current.with("jobs", jobs.put(job, { rev: candidate, state: "open" })))
  }

  // The admin decides one named revision. An approval publishes it only while it is still the
  // newest text; a decision about an older revision never applies to newer content.
  action adminDecide(c: Command): bool = {
    val newest = revs.keys().size()
    if (c.actor != admin) refuse(c, "not_admin")
    else if (not(revs.keys().contains(c.rev))) refuse(c, "unknown_revision")
    else if (removal != "none") refuse(c, "target_gone")
    else if (c.value == "approve")
      if (c.rev == shown) commit(c, "kept", setRev(current, c.rev, "approved", "admin"))
      else if (c.rev == newest) commit(c, "published", show(setRev(current, c.rev, "approved", "admin"), c.rev))
      else refuse(c, "not_current")
    else
      if (c.rev == shown) commit(c, "unpublished",
        setRev(current, c.rev, "rejected", "admin").with("shown", 0).with("changes", changes + 1))
      else if (c.rev == candidate)
        commit(c, "rejected", setRev(current, c.rev, "rejected", "admin").with("candidate", 0))
      else if (revs.get(c.rev).decision == "rejected")
        commit(c, "upheld", setRev(current, c.rev, "rejected", "admin"))
      else refuse(c, "not_current")
  }

  pure def removed(s: Snapshot, by: str): Snapshot =
    s.with("removal", by).with("shown", 0).with("candidate", 0)
      .with("changes", if (s.shown == 0) s.changes else s.changes + 1)
  action adminRemoved(c: Command): bool =
    if (c.actor != admin) refuse(c, "not_admin")
    else if (removal != "none") refuse(c, "target_gone")
    else commit(c, "removed", removed(current, "admin"))
  action ownRemoved(c: Command): bool =
    if (c.actor != writer) refuse(c, "not_comment_writer")
    else if (removal != "none") refuse(c, "target_unavailable")
    else commit(c, "removed", removed(current, "writer"))

  // "Hữu ích" stays with the text it was given to (LVE-FR-04); it is never moved to a rewrite.
  action endorsed(c: Command): bool =
    if (removal != "none" or shown == 0) refuse(c, "target_unavailable")
    else if (c.actor == writer) refuse(c, "own_content")
    else commit(c, "endorsed", current.with("votes", votes.union(Set((c.actor, shown)))))

  action dispatchWith(c: Command, overwrite: bool): bool =
    if (c.op == "edit") edited(c, overwrite)
    else if (c.op == "ai_result") aiResult(c)
    else if (c.op == "reevaluate") reevaluate(c)
    else if (c.op == "admin_decide") adminDecide(c)
    else if (c.op == "admin_remove") adminRemoved(c)
    else if (c.op == "remove_own") ownRemoved(c)
    else endorsed(c)
  action dispatch(c: Command): bool = dispatchWith(c, false)

  action step = {
    nondet who = Set(workAuthor, writer, 3).oneOf()
    nondet e = Set(version, version - 1).oneOf()
    nondet k = 1.to(5).oneOf()
    nondet b = 1.to(3).oneOf()
    nondet r = 1.to(maxRevs).oneOf()
    nondet j = 1.to(maxJobs).oneOf()
    nondet v = Set("approve", "reject", "uncertain", "invalid_output", "timeout", "provider_error").oneOf()
    nondet verdict = Set("approve", "reject").oneOf()
    nondet judge = Set(admin, evaluator, workAuthor).oneOf()
    any {
      dispatch(edit(who, e, k, b)), dispatch(ai(j, r, v)), dispatch(requeue),
      dispatch(decide(judge, r, verdict)), dispatch(adminRemove(judge)), dispatch(removeOwn(who)),
      dispatch(endorse(who)),
    }
  }

  // First approval (no mark), an edit that waits while the old text stays public, a save
  // without change, two tabs, a lost acknowledgement, a rejected edit, a superseded result, a
  // failure and retry, an admin override, votes that stay with their text, removal during review.
  pure val editScenario = List(
    endorse(3),                // target_unavailable: nothing is public yet
    ai(1, 1, "approve"),       // published: revision 1, no "Đã sửa"
    endorse(3),                // endorsed on revision 1
    endorse(writer),           // own_content
    edit(workAuthor, 1, 1, 2), // not_comment_writer: the work author cannot edit it
    edit(writer, 1, 1, 2),     // accepted: revision 2 pending, revision 1 still public
    edit(writer, 1, 1, 2),     // replayed: the lost acknowledgement, same key and payload
    edit(writer, 1, 1, 3),     // key_reused
    edit(writer, 1, 2, 3),     // stale: the other tab still thinks version 1
    edit(writer, 2, 3, 2),     // reaffirmed: same text, version 3, no revision, no mark
    edit(writer, 2, 4, 3),     // stale: an older tab after the reaffirmation
    ai(2, 2, "timeout"),       // retryable
    ai(2, 2, "approve"),       // duplicate
    requeue,                   // requeued: job 3
    requeue,                   // not_needed
    ai(3, 1, "approve"),       // wrong_revision
    ai(3, 2, "reject"),        // rejected: revision 1 stays public, still no mark
    edit(writer, 3, 5, 3),     // accepted: revision 3, job 4
    edit(writer, 4, 6, 1),     // accepted: revision 4 (the original words again), job 5
    ai(4, 3, "approve"),       // superseded: a newer edit replaced revision 3
    decide(evaluator, 4, "approve"), // not_admin
    decide(admin, 3, "approve"),     // not_current: an older revision is not published
    decide(admin, 4, "approve"),     // published: revision 4 with "Đã sửa"; the old vote stays behind
    ai(5, 4, "reject"),        // overridden: the late result loses to the admin
    endorse(3),                // endorsed on revision 4
    decide(admin, 2, "reject"),      // upheld: the rejected edit stays rejected, by the admin
    decide(admin, 4, "reject"),      // unpublished: nothing is public
    removeOwn(3),              // not_comment_writer
    removeOwn(writer),         // removed
    edit(writer, 5, 7, 2),     // target_unavailable
    decide(admin, 4, "approve"),     // target_gone
  )
  action editStep = all { cursor < editScenario.length(), dispatch(editScenario.nth(cursor)) }

  // Removal while an edit is under review: the late result finds nothing to change.
  pure val removalScenario = List(
    ai(1, 1, "uncertain"),     // needs_admin: nothing is public
    ai(9, 1, "approve"),       // unknown_job
    decide(admin, 1, "approve"), // published by the admin
    edit(writer, 1, 1, 2),     // accepted: revision 2 under review
    adminRemove(workAuthor),   // not_admin
    adminRemove(admin),        // removed
    ai(2, 2, "approve"),       // target_gone
    removeOwn(writer),         // target_unavailable
    endorse(3),                // target_unavailable
  )
  action removalStep = all { cursor < removalScenario.length(), dispatch(removalScenario.nth(cursor)) }

  // Negative control: an edit that overwrites the public text shows unreviewed words.
  pure val overwriteLeak = List(ai(1, 1, "approve"), edit(writer, 1, 1, 2))
  action mutatedOverwriteStep = all {
    cursor < overwriteLeak.length(), dispatchWith(overwriteLeak.nth(cursor), true),
  }

  pure val refusals = Set("not_comment_writer", "replayed", "key_reused", "target_unavailable", "stale",
    "limit", "not_evaluator", "unknown_job", "duplicate", "wrong_revision", "target_gone", "overridden",
    "superseded", "not_needed", "not_admin", "unknown_revision", "not_current", "own_content")

  // COM-02: only an approved revision is public, and never the one still under review.
  val pendingNeverPublic = (shown == 0 or (revs.get(shown).decision == "approved" and shown != candidate)) and
    (seen.body == 0 or revs.keys().exists(r => revs.get(r).decision == "approved" and revs.get(r).body == seen.body))
  // The projection never counts a vote given to another revision.
  val countBelongsToPublicText = seen.helpful == votes.filter(v => v._2 == shown).size() and
    (shown != 0 or seen == { body: 0, edited: false, helpful: 0 })
  // "Đã sửa" says exactly that the public text is not the first public text.
  val editedMarkTruthful = shown == 0 or mark == (shown != firstShown)
  // A save that changed nothing, a retry or a refused edit leaves no trace in public.
  val unchangedSaveLeavesNoTrace = command.op != "edit" or outcome == "accepted" or
    (revs == previous.revs and shown == previous.shown and mark == previous.mark and
      changes == previous.changes and jobs == previous.jobs)
  val acceptedEditKeepsPublicText = not(command.op == "edit" and outcome == "accepted") or
    (shown == previous.shown and mark == previous.mark and changes == previous.changes)
  val publicTextChangesOnlyByDecision = shown == previous.shown or
    Set("ai_result", "admin_decide", "admin_remove", "remove_own").contains(command.op)
  // COM-08: only the writer creates a revision.
  val onlyWriterCreatesRevisions = revs.keys().size() == previous.revs.keys().size() or
    (command.op == "edit" and command.actor == writer)
  val adminOverrideWins = previous.revs.keys().forall(r => previous.revs.get(r).by != "admin" or
    revs.get(r) == previous.revs.get(r) or command.op == "admin_decide")
  val removalTerminal = previous.removal == "none" or (current == previous and shown == 0)
  val refusalWritesNothing = not(refusals.contains(outcome)) or current == previous
  val versionFence = version == previous.version or (version == previous.version + 1 and
    command.op == "edit" and command.expected == previous.version)
  val receiptsAreStable = previous.receipts.keys().forall(k =>
    receipts.keys().contains(k) and receipts.get(k) == previous.receipts.get(k))
  // Votes are given to the public text of that moment and never follow a rewrite.
  val votesStayWithTheirText = previous.votes.subseteq(votes) and
    votes.exclude(previous.votes).forall(v => v._2 == shown and v._2 == previous.shown and v._1 != writer)
  val finiteDomain = revs.keys().size() <= maxRevs and jobs.keys().size() <= maxJobs
  val commentEditSafe = pendingNeverPublic and editedMarkTruthful and unchangedSaveLeavesNoTrace and
    acceptedEditKeepsPublicText and publicTextChangesOnlyByDecision and onlyWriterCreatesRevisions and
    adminOverrideWins and removalTerminal and refusalWritesNothing and versionFence and
    receiptsAreStable and votesStayWithTheirText and countBelongsToPublicText and finiteDomain
}
