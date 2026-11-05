// D2T-02: comment lifecycle, per-work comments mode and moderation authority, no I/O.
// One public work whose author is principal 1; principals 1..3; comment IDs 1..3; viewer 0 is an
// anonymous reader. Each action is one serialized commit: the PostgreSQL locks that make grant,
// mode and comment rows serialize are an assumption here, tested in the postgres lane.
// Structure (root/context) belongs to structure.qnt; target here is only "reply to which comment".
// Submission receipts (D2T-INV-06): a receipt binds (actor, key) to the payload (work, target,
// body) and the comment it created; it is written only together with that comment.
// Comments mode (COM-07) uses an accepted-intent generation (owner decision 05/11/2026): a request
// at the current generation is accepted and advances it even when the mode already holds.
module lifecycle {
  type Comment = { writer: int, target: int, review: str, removal: str, version: int }
  // op: submit | set_mode | remove_own | moderate | grant | revoke.
  // value: mode for set_mode, approve/reject/remove for moderate, otherwise "".
  // key/body: the client's idempotency key and body text (1..2) of a submit, otherwise 0.
  type Command = { op: str, actor: int, id: int, target: int, expected: int, value: str,
                   key: int, body: int }
  type Receipt = { target: int, body: int, comment: int }

  pure val WORK_AUTHOR = 1
  pure val principals = 1.to(3)
  pure val ids = 1.to(3)
  pure val viewers = 0.to(3)

  var comments: int -> Comment
  var mode: str
  var generation: int
  var grants: Set[int]
  // (actor, key) -> receipt of the committed submission.
  var receipts: (int, int) -> Receipt
  // Comment ID the command answered with: the created or replayed comment, otherwise 0.
  var answer: int
  // Viewer projection of every comment, exported so the Rust replay compares it with `view`.
  var views: int -> (int -> str)
  // Snapshot of the step before, for transition invariants.
  var previous: int -> Comment
  var previousMode: str
  var previousGeneration: int
  var previousGrants: Set[int]
  var previousReceipts: (int, int) -> Receipt
  // Ghost history for COM-07: the highest generation of a confirmed mode request (accepted, or
  // its mode already held). Clients only learn generations from the server, so a request at a
  // generation the server never issued confirms nothing. 0 before any confirmation.
  var modeFence: int
  var previousModeFence: int
  var command: Command
  var outcome: str
  // Only deterministic scenarios use cursor; saturating it keeps the state space finite.
  var cursor: int

  pure def isPresent(c: Comment): bool = c.removal == "none"
  pure def isPublic(c: Comment): bool = isPresent(c) and c.review == "approved"

  pure def view(c: Comment, viewer: int): str =
    if (isPublic(c)) "public"
    else if (not(isPresent(c)) or viewer != c.writer) "unavailable"
    else if (c.review == "pending") "own_pending"
    else "own_rejected"

  pure def viewsOf(cs: int -> Comment): int -> (int -> str) =
    cs.keys().mapBy(id => viewers.mapBy(viewer => view(cs.get(id), viewer)))

  action init = all {
    comments' = Map(),
    // A new public work starts open (owner decision 03/11/2026).
    mode' = "open",
    generation' = 1,
    grants' = Set(),
    receipts' = Map(),
    answer' = 0,
    views' = Map(),
    previous' = Map(),
    previousMode' = "open",
    previousGeneration' = 1,
    previousGrants' = Set(),
    previousReceipts' = Map(),
    modeFence' = 0,
    previousModeFence' = 0,
    command' = { op: "init", actor: 0, id: 0, target: 0, expected: 0, value: "", key: 0, body: 0 },
    outcome' = "init",
    cursor' = 0,
  }

  action commitWith(cmd: Command, result: str, nextComments: int -> Comment, nextMode: str,
                    nextGeneration: int, nextGrants: Set[int], nextReceipts: (int, int) -> Receipt,
                    nextAnswer: int): bool = all {
    comments' = nextComments,
    mode' = nextMode,
    generation' = nextGeneration,
    grants' = nextGrants,
    receipts' = nextReceipts,
    answer' = nextAnswer,
    views' = viewsOf(nextComments),
    previous' = comments,
    previousMode' = mode,
    previousGeneration' = generation,
    previousGrants' = grants,
    previousReceipts' = receipts,
    modeFence' = if (cmd.op == "set_mode" and
        Set("changed", "reaffirmed", "already_current").contains(result) and
        cmd.expected <= generation and cmd.expected > modeFence) cmd.expected else modeFence,
    previousModeFence' = modeFence,
    command' = cmd,
    outcome' = result,
    cursor' = if (cursor < 30) cursor + 1 else cursor,
  }

  // Every command except submit leaves receipts alone and answers no comment.
  action commit(cmd: Command, result: str, nextComments: int -> Comment, nextMode: str,
                nextGeneration: int, nextGrants: Set[int]): bool =
    commitWith(cmd, result, nextComments, nextMode, nextGeneration, nextGrants, receipts, 0)

  // decide_submission on a public work. The receipt check comes before every other gate, so a
  // retry replays even after comments close. A refused submission rolls back and keeps no
  // receipt. The server generates a fresh ID even for a retry; a replay answers the first one.
  // honorReceipts is false ONLY for the explicit negative-control action below.
  action submitWith(cmd: Command, honorReceipts: bool): bool = {
    val receiptKey = (cmd.actor, cmd.key)
    val hasReceipt = honorReceipts and receipts.keys().contains(receiptKey)
    val result = if (hasReceipt)
        (if (receipts.get(receiptKey).target == cmd.target and receipts.get(receiptKey).body == cmd.body)
          "replayed" else "key_reused")
      else if (mode != "open") "closed"
      else if (cmd.target != 0 and not(comments.keys().contains(cmd.target) and
        isPublic(comments.get(cmd.target)))) "target_unavailable"
      else "created"
    all {
      not(comments.keys().contains(cmd.id)),
      commitWith(cmd, result,
        if (result == "created") comments.put(cmd.id, {
          writer: cmd.actor, target: cmd.target, review: "pending", removal: "none", version: 1,
        }) else comments,
        mode, generation, grants,
        if (result == "created")
          receipts.put(receiptKey, { target: cmd.target, body: cmd.body, comment: cmd.id })
        else receipts,
        if (result == "created") cmd.id
        else if (result == "replayed") receipts.get(receiptKey).comment
        else 0),
    }
  }

  action submit(cmd: Command): bool = submitWith(cmd, true)

  // authorize_mode_change, then DiscussionSettings::change_mode: the generation is compared first,
  // so a request at the current generation is accepted (a reaffirmation when the mode already
  // holds) and advances it. fenceReaffirm is false ONLY for the explicit negative control below:
  // it restores the former rule, where asking for the current mode was a no-op at any generation.
  action setModeWith(cmd: Command, fenceReaffirm: bool): bool = {
    val accepted = cmd.expected == generation and (fenceReaffirm or cmd.value != mode)
    val result = if (cmd.actor != WORK_AUTHOR) "not_work_author"
      else if (accepted) (if (cmd.value == mode) "reaffirmed" else "changed")
      else if (cmd.value == mode) "already_current"
      else "stale"
    val written = result == "changed" or result == "reaffirmed"
    commit(cmd, result, comments,
      if (written) cmd.value else mode,
      if (written) generation + 1 else generation,
      grants)
  }

  action setMode(cmd: Command): bool = setModeWith(cmd, true)

  // authorize_writer_removal on the actor's own view, then remove_by_author (no version).
  action removeOwn(cmd: Command): bool = all {
    comments.keys().contains(cmd.id), removeExisting(cmd),
  }
  action removeExisting(cmd: Command): bool = {
    val c = comments.get(cmd.id)
    val result = if (cmd.actor != c.writer)
        (if (view(c, cmd.actor) == "public") "not_comment_writer" else "hidden")
      else if (not(isPresent(c))) "unchanged"
      else "changed"
    commit(cmd, result,
      if (result == "changed") comments.put(cmd.id, { ...c, removal: "author", version: c.version + 1 })
      else comments,
      mode, generation, grants)
  }

  // Grant read at commit, recusal (authorize_moderation), then ModerationAction::apply.
  // recuseWorkAuthor is false ONLY for the explicit negative-control action below.
  action moderateWith(cmd: Command, recuseWorkAuthor: bool): bool = all {
    comments.keys().contains(cmd.id), moderateExisting(cmd, recuseWorkAuthor),
  }
  action moderateExisting(cmd: Command, recuseWorkAuthor: bool): bool = {
    val c = comments.get(cmd.id)
    val result = if (not(grants.contains(cmd.actor))) "not_moderator"
      else if (cmd.actor == c.writer or (recuseWorkAuthor and cmd.actor == WORK_AUTHOR)) "recused"
      else if (cmd.value == "remove")
        (if (not(isPresent(c))) "unchanged" else if (cmd.expected != c.version) "stale" else "changed")
      else if (not(isPresent(c))) "removed"
      else if (cmd.expected != c.version) "stale"
      else if ((cmd.value == "approve" and c.review == "approved") or
               (cmd.value == "reject" and c.review == "rejected")) "unchanged"
      else "changed"
    val decided = if (cmd.value == "remove") { ...c, removal: "moderation", version: c.version + 1 }
      else { ...c, review: if (cmd.value == "approve") "approved" else "rejected", version: c.version + 1 }
    commit(cmd, result, if (result == "changed") comments.put(cmd.id, decided) else comments,
      mode, generation, grants)
  }

  action moderate(cmd: Command): bool = moderateWith(cmd, true)

  // Operator commands `vink-app moderator grant|revoke`: idempotent, never touch comments.
  action grant(cmd: Command): bool = commit(cmd,
    if (grants.contains(cmd.actor)) "already_active" else "granted",
    comments, mode, generation, grants.union(Set(cmd.actor)))
  action revoke(cmd: Command): bool = commit(cmd,
    if (grants.contains(cmd.actor)) "revoked" else "not_active",
    comments, mode, generation, grants.exclude(Set(cmd.actor)))

  action dispatch(cmd: Command): bool =
    if (cmd.op == "submit") submit(cmd)
    else if (cmd.op == "set_mode") setMode(cmd)
    else if (cmd.op == "remove_own") removeOwn(cmd)
    else if (cmd.op == "moderate") moderate(cmd)
    else if (cmd.op == "grant") grant(cmd)
    else revoke(cmd)

  action step = {
    nondet actor = principals.oneOf()
    nondet id = ids.oneOf()
    nondet target = 0.to(3).oneOf()
    nondet delta = Set(-1, 0, 1).oneOf()
    nondet modeValue = Set("open", "closed").oneOf()
    nondet decision = Set("approve", "reject", "remove").oneOf()
    // Two keys and two bodies per actor make retries, replays and key reuse likely.
    nondet key = 1.to(2).oneOf()
    nondet body = 1.to(2).oneOf()
    // Expected versions near the current one make stale and current requests both likely.
    val expectedGeneration = if (generation + delta < 1) 1 else generation + delta
    val version = if (comments.keys().contains(id)) comments.get(id).version else 1
    val expectedVersion = if (version + delta < 1) 1 else version + delta
    any {
      submit({ op: "submit", actor: actor, id: id, target: target, expected: 0, value: "", key: key, body: body }),
      setMode(cmd("set_mode", actor, 0, 0, expectedGeneration, modeValue)),
      removeOwn(cmd("remove_own", actor, id, 0, 0, "")),
      moderate(cmd("moderate", actor, id, 0, expectedVersion, decision)),
      grant(cmd("grant", actor, 0, 0, 0, "")),
      revoke(cmd("revoke", actor, 0, 0, 0, "")),
    }
  }

  // A submit through this helper uses its fresh comment ID as its key and body 1, so these
  // scenarios never retry; submission_retry below sets keys and bodies explicitly.
  pure def cmd(op: str, actor: int, id: int, target: int, expected: int, value: str): Command =
    { op: op, actor: actor, id: id, target: target, expected: expected, value: value,
      key: if (op == "submit") id else 0, body: if (op == "submit") 1 else 0 }
  pure def keyed(actor: int, id: int, target: int, key: int, body: int): Command =
    { op: "submit", actor: actor, id: id, target: target, expected: 0, value: "", key: key, body: body }

  // Inputs only: expected outcomes and states are always computed by the actions above.
  // Principal 1 = work author (later also a moderator), 2 = reader, 3 = moderator.
  pure val moderation = List(
    cmd("grant", 3, 0, 0, 0, ""),
    cmd("grant", 1, 0, 0, 0, ""), // the work author is also a moderator
    cmd("grant", 1, 0, 0, 0, ""), // already active
    cmd("submit", 2, 1, 0, 0, ""),
    cmd("moderate", 2, 1, 0, 1, "approve"), // no grant
    cmd("moderate", 1, 1, 0, 1, "approve"), // author-moderator recuses on own work
    cmd("remove_own", 1, 1, 0, 0, ""), // pending: the work author cannot even see it
    cmd("moderate", 3, 1, 0, 1, "approve"),
    cmd("remove_own", 1, 1, 0, 0, ""), // public but someone else's
    cmd("moderate", 3, 1, 0, 1, "reject"), // late decision on version 1
    cmd("moderate", 3, 1, 0, 2, "approve"), // already approved: safe retry
    cmd("submit", 3, 2, 1, 0, ""), // moderator replies like a reader
    cmd("moderate", 3, 2, 0, 1, "approve"), // own comment
    cmd("moderate", 1, 2, 0, 1, "remove"), // author-moderator again
    cmd("revoke", 3, 0, 0, 0, ""),
    cmd("revoke", 3, 0, 0, 0, ""), // nothing active
    cmd("moderate", 3, 1, 0, 2, "reject"), // revoked before this commit
    cmd("remove_own", 3, 2, 0, 0, ""), // the writer withdraws a pending reply
    cmd("remove_own", 3, 2, 0, 0, ""), // retry
    cmd("grant", 2, 0, 0, 0, ""),
    cmd("moderate", 2, 2, 0, 2, "approve"), // removal is terminal
    cmd("grant", 3, 0, 0, 0, ""),
    cmd("moderate", 3, 1, 0, 1, "remove"), // stale removal
    cmd("moderate", 3, 1, 0, 2, "remove"),
    cmd("moderate", 3, 1, 0, 3, "remove"), // already removed
    cmd("submit", 3, 3, 1, 0, ""), // target no longer public
  )
  pure val commentsMode = List(
    cmd("submit", 2, 1, 0, 0, ""),
    cmd("grant", 3, 0, 0, 0, ""),
    cmd("moderate", 3, 1, 0, 1, "approve"),
    cmd("set_mode", 2, 0, 0, 1, "closed"), // only the work author
    cmd("set_mode", 1, 0, 0, 1, "closed"),
    cmd("submit", 3, 2, 0, 0, ""), // closed: no new root
    cmd("submit", 1, 2, 1, 0, ""), // closed: no reply, even from the author
    cmd("set_mode", 1, 0, 0, 1, "closed"), // retry after a lost response
    cmd("set_mode", 1, 0, 0, 2, "closed"), // tab B keeps it closed: accepted, fences generation 2
    cmd("set_mode", 1, 0, 0, 2, "open"), // tab A's older reopen, delayed behind B: stale
    cmd("set_mode", 1, 0, 0, 1, "open"), // stale tab
    cmd("set_mode", 1, 0, 0, 3, "open"),
    cmd("submit", 3, 2, 1, 0, ""),
    cmd("submit", 1, 3, 2, 0, ""), // pending target
    cmd("grant", 2, 0, 0, 0, ""),
    cmd("moderate", 2, 2, 0, 1, "reject"),
    cmd("moderate", 2, 2, 0, 2, "approve"), // a later review reverses the rejection
    cmd("submit", 1, 3, 2, 0, ""),
  )
  // D2T-INV-06: `id` is the server's fresh ID for that attempt, unused by a replay.
  pure val submissionRetry = List(
    keyed(2, 1, 0, 5, 1),
    keyed(2, 2, 0, 5, 1), // retry after a lost response: replays comment 1
    keyed(2, 2, 0, 5, 2), // same key, other body
    keyed(2, 2, 1, 5, 1), // same key, other target
    cmd("set_mode", 1, 0, 0, 1, "closed"),
    keyed(2, 2, 0, 5, 1), // the receipt precedes the closed gate
    keyed(2, 2, 0, 6, 1), // a new key while closed
    cmd("set_mode", 1, 0, 0, 2, "open"),
    cmd("remove_own", 2, 1, 0, 0, ""),
    keyed(2, 2, 0, 5, 1), // replaying never recreates the removed comment
    keyed(3, 2, 0, 5, 1), // keys are scoped by actor
    keyed(2, 3, 0, 6, 1), // the refused attempt kept no receipt
  )
  pure val receiptRetry = List(
    keyed(2, 1, 0, 5, 1),
    keyed(2, 2, 0, 5, 1), // without the receipt check the retry creates a second comment
  )
  // COM-07 counterexample: closed at generation 2; tab A asks open@2 and is delayed; tab B asks
  // closed@2 later and arrives first. With a no-op for the current mode B fences nothing.
  pure val delayedReopen = List(
    cmd("set_mode", 1, 0, 0, 1, "closed"),
    cmd("set_mode", 1, 0, 0, 2, "closed"), // tab B: the author's newer choice
    cmd("set_mode", 1, 0, 0, 2, "open"), // tab A: without the fence it reopens comments
  )
  pure val authorRecusal = List(
    cmd("grant", 1, 0, 0, 0, ""),
    cmd("submit", 2, 1, 0, 0, ""),
    cmd("moderate", 1, 1, 0, 1, "remove"),
  )

  action moderationStep = all { cursor < moderation.length(), dispatch(moderation.nth(cursor)) }
  action modeStep = all { cursor < commentsMode.length(), dispatch(commentsMode.nth(cursor)) }
  action mutatedDispatch(c: Command): bool =
    if (c.op == "moderate") moderateWith(c, false) else dispatch(c)
  action retryStep = all { cursor < submissionRetry.length(), dispatch(submissionRetry.nth(cursor)) }
  action mutatedReceiptStep = all {
    cursor < receiptRetry.length(),
    val c = receiptRetry.nth(cursor)
    if (c.op == "submit") submitWith(c, false) else dispatch(c),
  }
  action mutatedRecusalStep = all {
    cursor < authorRecusal.length(), mutatedDispatch(authorRecusal.nth(cursor)),
  }
  action mutatedNoopModeStep = all {
    cursor < delayedReopen.length(), setModeWith(delayedReopen.nth(cursor), false),
  }

  // Nothing but an accepted command writes; every refusal and no-op leaves all state alone.
  val rejectionAtomic =
    Set("created", "changed", "reaffirmed", "granted", "revoked").contains(outcome) or (
      comments == previous and mode == previousMode and generation == previousGeneration and
      grants == previousGrants and receipts == previousReceipts)

  // A command touches only its own target: comment commands one comment, set_mode only the mode
  // (closing never hides, edits or removes a comment), operator commands only grants.
  val scopedWrites = {
    val settingsOrOperator = Set("set_mode", "grant", "revoke").contains(command.op)
    val commentsScoped = if (settingsOrOperator) comments == previous
      else previous.keys().subseteq(comments.keys()) and
        comments.keys().forall(id => id == command.id or comments.get(id) == previous.get(id))
    val modeScoped = command.op == "set_mode" or
      (mode == previousMode and generation == previousGeneration)
    val grantsScoped = Set("grant", "revoke").contains(command.op) or grants == previousGrants
    val receiptsScoped = command.op == "submit" or receipts == previousReceipts
    commentsScoped and modeScoped and grantsScoped and receiptsScoped
  }

  // COM-02: every new comment is pending, version 1, written by the actor, only while open and
  // only in reply to a comment that was public at that commit.
  val premoderatedSubmission = command.op != "submit" or (
    if (previousReceipts.keys().contains((command.actor, command.key)))
      Set("replayed", "key_reused").contains(outcome)
    else if (previousMode == "closed") outcome == "closed"
    else if (outcome == "created")
      comments.get(command.id) == {
        writer: command.actor, target: command.target, review: "pending", removal: "none", version: 1,
      } and (command.target == 0 or isPublic(previous.get(command.target)))
    else outcome == "target_unavailable" and command.target != 0)

  // COM-07: only the work author writes the settings, with the generation they saw, by one step;
  // a reaffirmation advances the generation and keeps the mode.
  val modeAuthority = (mode == previousMode and generation == previousGeneration) or (
    command.op == "set_mode" and command.actor == WORK_AUTHOR and
    command.expected == previousGeneration and generation == previousGeneration + 1 and
    mode == command.value and (outcome == "reaffirmed" iff mode == previousMode))

  // COM-07 "tab cũ không đè lựa chọn mới": once a mode request is confirmed (accepted, or its
  // mode already held), no request from that view or an older one is accepted afterwards. This
  // is what makes a request for the current mode at the current generation consume it.
  val confirmedModeFence = command.op != "set_mode" or
    not(Set("changed", "reaffirmed").contains(outcome)) or command.expected > previousModeFence

  // MOD-02/INV-07: versions never decrease; each change advances exactly one version; removal is
  // terminal; a decision bound to an older version changes nothing.
  val versionedLifecycle = previous.keys().forall(id => {
    val before = previous.get(id)
    val after = comments.get(id)
    (after == before or after.version == before.version + 1) and
    (isPresent(before) or after == before) and
    (after.writer == before.writer and after.target == before.target) and
    (command.op != "moderate" or command.id != id or not(isPresent(before)) or
     command.expected == before.version or after == before)
  })

  // Write authority (COM-08, MOD-02, owner decision 03/11/2026): a comment changes only through
  // its writer's removal or a moderator who held a grant at that commit and is neither its writer
  // nor the work's author. A grant never gives the work author power over others' comments.
  val writeAuthority = previous.keys().forall(id => {
    val before = previous.get(id)
    comments.get(id) == before or (
      command.id == id and (
        (command.op == "remove_own" and command.actor == before.writer and
         comments.get(id).removal == "author") or
        (command.op == "moderate" and previousGrants.contains(command.actor) and
         command.actor != before.writer and command.actor != WORK_AUTHOR)))
  })

  // D2T-INV-04: only present + approved is public; others see nothing of pending, rejected or
  // removed comments; the writer sees the status of their own present comment; the work author
  // gets nothing extra.
  val viewerProjection = views.keys() == comments.keys() and comments.keys().forall(id => {
    val c = comments.get(id)
    viewers.forall(viewer => {
      val seen = views.get(id).get(viewer)
      (seen == "public" iff isPublic(c)) and
      (viewer == c.writer or seen == "public" or seen == "unavailable") and
      (viewer != c.writer or isPublic(c) or not(isPresent(c)) or
       seen == (if (c.review == "pending") "own_pending" else "own_rejected"))
    })
  })

  // D2T-INV-06: one durable effect per (actor, key). A comment is created only under a fresh key
  // and its receipt records exactly that payload; the same key and payload answers the first
  // comment; the same key with another payload is refused. Receipts are never rewritten or lost.
  val idempotentSubmission = {
    val k = (command.actor, command.key)
    val payloadMatches = previousReceipts.keys().contains(k) and
      previousReceipts.get(k).target == command.target and previousReceipts.get(k).body == command.body
    previousReceipts.keys().forall(r => receipts.keys().contains(r) and
      receipts.get(r) == previousReceipts.get(r)) and
    receipts.keys().forall(r => comments.keys().contains(receipts.get(r).comment) and
      comments.get(receipts.get(r).comment).writer == r._1 and
      comments.get(receipts.get(r).comment).target == receipts.get(r).target) and
    (outcome != "created" or (not(previousReceipts.keys().contains(k)) and answer == command.id and
      receipts.get(k) == { target: command.target, body: command.body, comment: command.id })) and
    (outcome != "replayed" or (payloadMatches and answer == previousReceipts.get(k).comment)) and
    (outcome != "key_reused" or (previousReceipts.keys().contains(k) and not(payloadMatches))) and
    (Set("created", "replayed").contains(outcome) or answer == 0)
  }

  val finiteDomain = comments.keys().subseteq(ids) and grants.subseteq(principals) and
    Set("open", "closed").contains(mode) and generation >= 1 and
    comments.keys().forall(id => {
      val c = comments.get(id)
      principals.contains(c.writer) and (c.target == 0 or ids.contains(c.target)) and
      Set("pending", "approved", "rejected").contains(c.review) and
      Set("none", "author", "moderation").contains(c.removal) and c.version >= 1
    })

  val lifecycleSafe = rejectionAtomic and scopedWrites and premoderatedSubmission and
    modeAuthority and confirmedModeFence and versionedLifecycle and writeAuthority and
    viewerProjection and idempotentSubmission and finiteDomain
}
