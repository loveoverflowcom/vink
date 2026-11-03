// D2T-02: comment lifecycle, per-work comments mode and moderation authority, no I/O.
// One public work whose author is principal 1; principals 1..3; comment IDs 1..3; viewer 0 is an
// anonymous reader. Each action is one serialized commit: the PostgreSQL locks that make grant,
// mode and comment rows serialize are an assumption here, tested in the postgres lane.
// Structure (root/context) belongs to structure.qnt; target here is only "reply to which comment".
module lifecycle {
  type Comment = { writer: int, target: int, review: str, removal: str, version: int }
  // op: submit | set_mode | remove_own | moderate | grant | revoke.
  // value: mode for set_mode, approve/reject/remove for moderate, otherwise "".
  type Command = { op: str, actor: int, id: int, target: int, expected: int, value: str }

  pure val WORK_AUTHOR = 1
  pure val principals = 1.to(3)
  pure val ids = 1.to(3)
  pure val viewers = 0.to(3)

  var comments: int -> Comment
  var mode: str
  var generation: int
  var grants: Set[int]
  // Viewer projection of every comment, exported so the Rust replay compares it with `view`.
  var views: int -> (int -> str)
  // Snapshot of the step before, for transition invariants.
  var previous: int -> Comment
  var previousMode: str
  var previousGeneration: int
  var previousGrants: Set[int]
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
    views' = Map(),
    previous' = Map(),
    previousMode' = "open",
    previousGeneration' = 1,
    previousGrants' = Set(),
    command' = { op: "init", actor: 0, id: 0, target: 0, expected: 0, value: "" },
    outcome' = "init",
    cursor' = 0,
  }

  action commit(cmd: Command, result: str, nextComments: int -> Comment, nextMode: str,
                nextGeneration: int, nextGrants: Set[int]): bool = all {
    comments' = nextComments,
    mode' = nextMode,
    generation' = nextGeneration,
    grants' = nextGrants,
    views' = viewsOf(nextComments),
    previous' = comments,
    previousMode' = mode,
    previousGeneration' = generation,
    previousGrants' = grants,
    command' = cmd,
    outcome' = result,
    cursor' = if (cursor < 30) cursor + 1 else cursor,
  }

  // decide_submission with no receipt on a public work. The server generates a fresh ID.
  action submit(cmd: Command): bool = {
    val result = if (mode != "open") "closed"
      else if (cmd.target != 0 and not(comments.keys().contains(cmd.target) and
        isPublic(comments.get(cmd.target)))) "target_unavailable"
      else "created"
    all {
      not(comments.keys().contains(cmd.id)),
      commit(cmd, result,
        if (result == "created") comments.put(cmd.id, {
          writer: cmd.actor, target: cmd.target, review: "pending", removal: "none", version: 1,
        }) else comments,
        mode, generation, grants),
    }
  }

  // authorize_mode_change, then DiscussionSettings::change_mode (desired-state, generation CAS).
  action setMode(cmd: Command): bool = {
    val result = if (cmd.actor != WORK_AUTHOR) "not_work_author"
      else if (cmd.value == mode) "unchanged"
      else if (cmd.expected != generation) "stale"
      else "changed"
    commit(cmd, result, comments,
      if (result == "changed") cmd.value else mode,
      if (result == "changed") generation + 1 else generation,
      grants)
  }

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
    // Expected versions near the current one make stale and current requests both likely.
    val expectedGeneration = if (generation + delta < 1) 1 else generation + delta
    val version = if (comments.keys().contains(id)) comments.get(id).version else 1
    val expectedVersion = if (version + delta < 1) 1 else version + delta
    any {
      submit({ op: "submit", actor: actor, id: id, target: target, expected: 0, value: "" }),
      setMode({ op: "set_mode", actor: actor, id: 0, target: 0, expected: expectedGeneration, value: modeValue }),
      removeOwn({ op: "remove_own", actor: actor, id: id, target: 0, expected: 0, value: "" }),
      moderate({ op: "moderate", actor: actor, id: id, target: 0, expected: expectedVersion, value: decision }),
      grant({ op: "grant", actor: actor, id: 0, target: 0, expected: 0, value: "" }),
      revoke({ op: "revoke", actor: actor, id: 0, target: 0, expected: 0, value: "" }),
    }
  }

  pure def cmd(op: str, actor: int, id: int, target: int, expected: int, value: str): Command =
    { op: op, actor: actor, id: id, target: target, expected: expected, value: value }

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
    cmd("set_mode", 1, 0, 0, 1, "open"), // stale tab
    cmd("set_mode", 1, 0, 0, 2, "open"),
    cmd("submit", 3, 2, 1, 0, ""),
    cmd("submit", 1, 3, 2, 0, ""), // pending target
    cmd("grant", 2, 0, 0, 0, ""),
    cmd("moderate", 2, 2, 0, 1, "reject"),
    cmd("moderate", 2, 2, 0, 2, "approve"), // a later review reverses the rejection
    cmd("submit", 1, 3, 2, 0, ""),
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
  action mutatedRecusalStep = all {
    cursor < authorRecusal.length(), mutatedDispatch(authorRecusal.nth(cursor)),
  }

  // Nothing but an accepted command writes; every refusal and no-op leaves all state alone.
  val rejectionAtomic =
    Set("created", "changed", "granted", "revoked").contains(outcome) or (
      comments == previous and mode == previousMode and generation == previousGeneration and
      grants == previousGrants)

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
    commentsScoped and modeScoped and grantsScoped
  }

  // COM-02: every new comment is pending, version 1, written by the actor, only while open and
  // only in reply to a comment that was public at that commit.
  val premoderatedSubmission = command.op != "submit" or (
    if (previousMode == "closed") outcome == "closed"
    else if (outcome == "created")
      comments.get(command.id) == {
        writer: command.actor, target: command.target, review: "pending", removal: "none", version: 1,
      } and (command.target == 0 or isPublic(previous.get(command.target)))
    else outcome == "target_unavailable" and command.target != 0)

  // COM-07: only the work author changes the mode, with the generation they saw, by one step.
  val modeAuthority = (mode == previousMode and generation == previousGeneration) or (
    command.op == "set_mode" and command.actor == WORK_AUTHOR and
    command.expected == previousGeneration and generation == previousGeneration + 1 and
    mode == command.value and mode != previousMode)

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

  val finiteDomain = comments.keys().subseteq(ids) and grants.subseteq(principals) and
    Set("open", "closed").contains(mode) and generation >= 1 and
    comments.keys().forall(id => {
      val c = comments.get(id)
      principals.contains(c.writer) and (c.target == 0 or ids.contains(c.target)) and
      Set("pending", "approved", "rejected").contains(c.review) and
      Set("none", "author", "moderation").contains(c.removal) and c.version >= 1
    })

  val lifecycleSafe = rejectionAtomic and scopedWrites and premoderatedSubmission and
    modeAuthority and versionedLifecycle and writeAuthority and viewerProjection and finiteDomain
}
