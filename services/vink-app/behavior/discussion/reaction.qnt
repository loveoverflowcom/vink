// D2T-INV-08: reactions on comments as desired state with a per-(actor, comment) accepted-intent
// generation (a request at the current generation is accepted and advances it even when the
// reaction already holds; lve-core-v1 CHOICE-03), interaction limits and a count projection.
// No I/O.
// One public work whose author is principal 1; principals 1..3; comment 1 is written by 2 and
// comment 2 by 3. Two of the four production kinds suffice to reach replacement. Each action is
// one serialized commit; the row locks that serialize a reaction with moderation and the mode
// change are an assumption here (persistence is not implemented yet, #43).
// Comment lifecycle and mode authority belong to lifecycle.qnt: here a synthetic moderator only
// approves or removes, and the work author's mode change always uses the current generation.
// The ghost `confirmed` records the generation of each (actor, comment)'s confirmed requests.
module reaction {
  type Reaction = { kind: str, generation: int }
  // op: react | approve | remove | set_mode. value: kind or "none" for react, mode for set_mode.
  type Command = { op: str, actor: int, id: int, expected: int, value: str }

  pure val WORK_AUTHOR = 1
  pure val principals = 1.to(3)
  pure val ids = 1.to(2)
  pure val kinds = Set("heart", "moved")
  pure val desires = Set("none", "heart", "moved")
  pure def writer(id: int): int = if (id == 1) 2 else 3
  pure val pairs = tuples(principals, ids)

  // pending | public | removed (the lifecycle axes collapsed to what reactions depend on).
  var comments: int -> str
  var mode: str
  var reactions: (int, int) -> Reaction
  var tally: int -> (str -> int)
  // Counts a reader may see, exported so the Rust replay compares it with `public_tally`.
  var visibleTally: int -> (str -> int)
  var previousComments: int -> str
  var previousMode: str
  var previousReactions: (int, int) -> Reaction
  var previousTally: int -> (str -> int)
  // Ghost history: (actor, comment) -> highest generation of a confirmed request (accepted, or
  // its reaction already held). A request at a generation the server never issued confirms
  // nothing: clients only learn generations from the server. 0 before any confirmation.
  var confirmed: (int, int) -> int
  var previousConfirmed: (int, int) -> int
  var command: Command
  var outcome: str
  // Only deterministic scenarios use cursor; saturating it keeps the state space finite.
  var cursor: int

  pure def countOf(rs: (int, int) -> Reaction, id: int, kind: str): int =
    principals.filter(a => rs.get((a, id)).kind == kind).size()

  pure def visibleOf(cs: int -> str, t: int -> (str -> int)): int -> (str -> int) =
    ids.filter(id => cs.get(id) == "public").mapBy(id => t.get(id))

  action init = {
    val noTally = ids.mapBy(_ => kinds.mapBy(_ => 0))
    all {
      comments' = ids.mapBy(_ => "pending"),
      mode' = "open",
      // An actor who never reacted is `ActorReaction::NONE`: no kind, generation 1.
      reactions' = pairs.mapBy(_ => { kind: "none", generation: 1 }),
      tally' = noTally,
      visibleTally' = Map(),
      previousComments' = ids.mapBy(_ => "pending"),
      previousMode' = "open",
      previousReactions' = pairs.mapBy(_ => { kind: "none", generation: 1 }),
      previousTally' = noTally,
      confirmed' = pairs.mapBy(_ => 0),
      previousConfirmed' = pairs.mapBy(_ => 0),
      command' = { op: "init", actor: 0, id: 0, expected: 0, value: "" },
      outcome' = "init",
      cursor' = 0,
    }
  }

  action commit(cmd: Command, result: str, nextComments: int -> str, nextMode: str,
                nextReactions: (int, int) -> Reaction, nextTally: int -> (str -> int)): bool = all {
    comments' = nextComments,
    mode' = nextMode,
    reactions' = nextReactions,
    tally' = nextTally,
    visibleTally' = visibleOf(nextComments, nextTally),
    previousComments' = comments,
    previousMode' = mode,
    previousReactions' = reactions,
    previousTally' = tally,
    confirmed' = if (cmd.op == "react" and
        Set("changed", "reaffirmed", "already_current").contains(result) and
        cmd.expected <= reactions.get((cmd.actor, cmd.id)).generation and
        cmd.expected > confirmed.get((cmd.actor, cmd.id)))
      confirmed.set((cmd.actor, cmd.id), cmd.expected) else confirmed,
    previousConfirmed' = confirmed,
    command' = cmd,
    outcome' = result,
    cursor' = if (cursor < 30) cursor + 1 else cursor,
  }

  // decide_reaction on a public work, then ReactionTally::apply with the decision's delta. The
  // generation is compared before the state, so a request at the current generation is accepted
  // (a reaffirmation when the reaction already holds) and advances it. fenced, desiredState and
  // fenceReaffirm are false ONLY for the explicit negative-control actions below; fenceReaffirm
  // false restores the former rule, where asking for the held reaction was a no-op.
  action reactWith(cmd: Command, fenced: bool, desiredState: bool, fenceReaffirm: bool): bool = {
    val current = reactions.get((cmd.actor, cmd.id))
    val wanted = if (not(desiredState) and cmd.value == current.kind) "none" else cmd.value
    val newInteraction = wanted != "none" and wanted != current.kind
    val accepted = (not(fenced) or cmd.expected == current.generation) and
      (fenceReaffirm or wanted != current.kind)
    val result = if (comments.get(cmd.id) != "public") "target_unavailable"
      else if (cmd.actor == writer(cmd.id)) "own_comment"
      else if (newInteraction and mode != "open") "closed"
      else if (accepted) (if (wanted == current.kind) "reaffirmed" else "changed")
      else if (wanted == current.kind) "already_current"
      else "stale"
    val written = result == "changed" or result == "reaffirmed"
    val counts = tally.get(cmd.id)
    val decremented = if (current.kind == "none") counts
      else counts.set(current.kind, counts.get(current.kind) - 1)
    val updated = if (wanted == "none") decremented
      else decremented.set(wanted, decremented.get(wanted) + 1)
    commit(cmd, result, comments, mode,
      if (written)
        reactions.set((cmd.actor, cmd.id), { kind: wanted, generation: current.generation + 1 })
      else reactions,
      if (result == "changed") tally.set(cmd.id, updated) else tally)
  }

  action react(cmd: Command): bool = reactWith(cmd, true, true, true)

  // Synthetic moderation shell: ModerationAction::apply at the comment's current version.
  action approve(cmd: Command): bool = {
    val result = if (comments.get(cmd.id) == "pending") "moderated" else "noop"
    commit(cmd, result,
      if (result == "moderated") comments.set(cmd.id, "public") else comments,
      mode, reactions, tally)
  }
  action remove(cmd: Command): bool = {
    val result = if (comments.get(cmd.id) != "removed") "moderated" else "noop"
    // Removal keeps reactions and counts for audit; only their visibility changes.
    commit(cmd, result,
      if (result == "moderated") comments.set(cmd.id, "removed") else comments,
      mode, reactions, tally)
  }
  // The work author's desired-state mode change at the current generation: always accepted, a
  // reaffirmation when the mode already holds (its generation belongs to lifecycle.qnt).
  action setMode(cmd: Command): bool =
    commit(cmd, if (cmd.value == mode) "mode_reaffirmed" else "mode_set", comments, cmd.value,
      reactions, tally)

  action dispatch(cmd: Command): bool =
    if (cmd.op == "react") react(cmd)
    else if (cmd.op == "approve") approve(cmd)
    else if (cmd.op == "remove") remove(cmd)
    else setMode(cmd)

  action step = {
    nondet actor = principals.oneOf()
    nondet id = ids.oneOf()
    nondet delta = Set(-1, 0, 1).oneOf()
    nondet wanted = desires.oneOf()
    nondet modeValue = Set("open", "closed").oneOf()
    // Expected generations near the current one make stale and current requests both likely.
    val generation = reactions.get((actor, id)).generation
    val expected = if (generation + delta < 1) 1 else generation + delta
    any {
      react({ op: "react", actor: actor, id: id, expected: expected, value: wanted }),
      approve({ op: "approve", actor: 0, id: id, expected: 0, value: "" }),
      remove({ op: "remove", actor: 0, id: id, expected: 0, value: "" }),
      setMode({ op: "set_mode", actor: WORK_AUTHOR, id: 0, expected: 0, value: modeValue }),
    }
  }

  pure def cmd(op: str, actor: int, id: int, expected: int, value: str): Command =
    { op: op, actor: actor, id: id, expected: expected, value: value }

  // Inputs only: expected outcomes and states are always computed by the actions above.
  pure val desiredState = List(
    cmd("react", 3, 1, 1, "heart"), // pending comment: unavailable
    cmd("approve", 0, 1, 0, ""),
    cmd("react", 2, 1, 1, "heart"), // the writer's own comment
    cmd("react", 2, 1, 1, "none"), // every request of the writer is refused: no row is stored
    cmd("react", 3, 1, 1, "heart"),
    cmd("react", 3, 1, 1, "heart"), // retry after a lost response
    cmd("react", 1, 1, 1, "moved"), // the work author reacts like any reader
    cmd("react", 3, 1, 2, "moved"), // replace, one reaction per actor
    cmd("react", 3, 1, 2, "none"), // stale tab
    cmd("react", 3, 1, 3, "none"), // clear
    cmd("react", 3, 1, 2, "heart"), // delayed duplicate cannot revert the clear
    cmd("react", 3, 1, 4, "none"), // clearing twice is not a toggle; it is a reaffirmation
    cmd("set_mode", 1, 0, 0, "closed"),
    cmd("react", 3, 1, 5, "heart"), // closed: no new reaction
    cmd("react", 1, 1, 1, "moved"), // closed: a retry of the current state still succeeds
    cmd("react", 1, 1, 2, "moved"), // closed: reaffirming the held reaction is no new interaction
    cmd("react", 1, 1, 3, "none"), // closed: clearing is always allowed
    cmd("set_mode", 1, 0, 0, "closed"), // the author reaffirms the mode
    cmd("set_mode", 1, 0, 0, "open"),
    cmd("react", 2, 2, 1, "heart"), // comment 2 is still pending
    cmd("approve", 0, 2, 0, ""),
    cmd("react", 2, 2, 1, "none"), // tab B: "no reaction", the newer intent, arrives first
    cmd("react", 2, 2, 1, "heart"), // tab A's older Heart@1, delayed behind B: stale
    cmd("react", 2, 2, 2, "heart"),
    cmd("react", 1, 2, 1, "heart"),
    cmd("remove", 0, 2, 0, ""), // reactions stay stored, counts become invisible
    cmd("react", 2, 2, 3, "none"), // hidden: even clearing is unavailable
    cmd("react", 1, 1, 4, "heart"),
  )
  // Negative controls: the same inputs as a client would send, through mutated decisions.
  pure val toggleRetry = List(
    cmd("approve", 0, 1, 0, ""),
    cmd("react", 3, 1, 1, "heart"),
    cmd("react", 3, 1, 2, "heart"), // a toggle would clear it
  )
  pure val delayedDuplicate = List(
    cmd("approve", 0, 1, 0, ""),
    cmd("react", 3, 1, 1, "heart"),
    cmd("react", 3, 1, 2, "none"),
    cmd("react", 3, 1, 1, "heart"), // without the fence the old request wins
  )
  // No reaction at generation 1; tab A asks Heart@1 and is delayed; tab B asks none@1 later and
  // arrives first. If asking for the held reaction is a no-op, B fences nothing and A wins.
  pure val delayedBehindNoop = List(
    cmd("approve", 0, 1, 0, ""),
    cmd("react", 3, 1, 1, "none"), // tab B: the newer intent
    cmd("react", 3, 1, 1, "heart"), // tab A: the older request
  )

  action desiredStateStep = all { cursor < desiredState.length(), dispatch(desiredState.nth(cursor)) }
  action mutatedToggleStep = all {
    cursor < toggleRetry.length(),
    val c = toggleRetry.nth(cursor)
    if (c.op == "react") reactWith(c, true, false, true) else dispatch(c),
  }
  action mutatedUnfencedStep = all {
    cursor < delayedDuplicate.length(),
    val c = delayedDuplicate.nth(cursor)
    if (c.op == "react") reactWith(c, false, true, true) else dispatch(c),
  }
  action mutatedNoopStep = all {
    cursor < delayedBehindNoop.length(),
    val c = delayedBehindNoop.nth(cursor)
    if (c.op == "react") reactWith(c, true, true, false) else dispatch(c),
  }

  // Only an accepted reaction request writes a reaction, and only a change moves counts;
  // everything else leaves them alone.
  val reactionWritesAreApplied =
    (command.op == "react" and outcome == "changed") or
    (command.op == "react" and outcome == "reaffirmed" and tally == previousTally) or
    (reactions == previousReactions and tally == previousTally)

  // A reaction command touches only the actor's reaction on that comment and its tally; other
  // commands never touch reactions.
  val scopedWrites = pairs.forall(p =>
    reactions.get(p) == previousReactions.get(p) or (command.op == "react" and
      p == (command.actor, command.id))) and
    ids.forall(id => tally.get(id) == previousTally.get(id) or
      (command.op == "react" and id == command.id)) and
    (command.op == "react" implies (comments == previousComments and mode == previousMode))

  // Desired state, never a toggle: asking for the current reaction never changes any kind or
  // count (a reaffirmation only advances its generation), and an accepted request leaves exactly
  // the requested reaction.
  val noBlindToggle = command.op != "react" or (
    if (previousReactions.get((command.actor, command.id)).kind == command.value)
      pairs.forall(p => reactions.get(p).kind == previousReactions.get(p).kind) and
      tally == previousTally
    else outcome != "changed" or reactions.get((command.actor, command.id)).kind == command.value)

  // Every write needs the generation the actor saw and advances it by exactly one.
  val generationFence = pairs.forall(p => {
    val before = previousReactions.get(p)
    val after = reactions.get(p)
    after == before or (command.op == "react" and p == (command.actor, command.id) and
      command.expected == before.generation and after.generation == before.generation + 1)
  })

  // Once a request is confirmed (accepted, or its reaction already held), no request from that
  // view or an older one is accepted afterwards: a delayed request cannot undo a newer intent,
  // even one that asked for the state already held (lve-core-v1 CHOICE-03).
  val confirmedIntentFence = command.op != "react" or
    not(Set("changed", "reaffirmed").contains(outcome)) or
    command.expected > previousConfirmed.get((command.actor, command.id))

  // Only a public comment accepts a write and never from its writer; a new or replacing reaction
  // also needs open comments. Clearing and reaffirming stay possible while closed.
  val interactionLimits = pairs.forall(p => {
    val before = previousReactions.get(p)
    val after = reactions.get(p)
    after == before or (previousComments.get(p._2) == "public" and p._1 != writer(p._2) and
      (after.kind == "none" or after.kind == before.kind or previousMode == "open"))
  })

  // Counts are exactly the number of actors holding each kind: never negative, never drifting.
  val tallyMatchesReactions = ids.forall(id => kinds.forall(kind =>
    tally.get(id).get(kind) == countOf(reactions, id, kind) and tally.get(id).get(kind) >= 0))

  // No writer ever holds a reaction, or even a stored row, on their own comment.
  val noSelfReaction = ids.forall(id => reactions.get((writer(id), id)) ==
    { kind: "none", generation: 1 })

  // A hidden comment exposes no counts; a public one exposes exactly its tally.
  val hiddenCountsStayHidden = visibleTally.keys() == ids.filter(id => comments.get(id) == "public")
    and visibleTally.keys().forall(id => visibleTally.get(id) == tally.get(id))

  val finiteDomain = Set("open", "closed").contains(mode) and
    ids.forall(id => Set("pending", "public", "removed").contains(comments.get(id))) and
    pairs.forall(p => desires.contains(reactions.get(p).kind) and reactions.get(p).generation >= 1)

  val reactionSafe = reactionWritesAreApplied and scopedWrites and noBlindToggle and
    generationFence and confirmedIntentFence and interactionLimits and tallyMatchesReactions and
    noSelfReaction and hiddenCountsStayHidden and finiteDomain
}
