// LVE PR1: serialized commits are an assumption, not a SQL/auth proof.
// 3 principals; 1=work/r1, 2=work/r2, 3=root, 4=flat reply.
// Mathematical order is unbounded; 40-step simulation reaches at most order41.
// Production i64/count exhaustion belongs to Rust/Kani, not this small model.
module endorsement {
  type Row = { selected: bool, version: int, order: int }
  type Command = { op: str, actor: int, id: int, expected: int, key: int, value: bool }
  type Receipt = { command: Command, committed: Row }
  pure val actors = 1.to(3)
  pure val ids = 1.to(4)
  pure val pairs = tuples(actors, ids)
  pure val initial = { selected: false, version: 1, order: 1 }
  pure def author(id: int): int = if (id <= 2) 1 else if (id == 3) 2 else 3
  pure def cmd(op: str, actor: int, id: int, expected: int, key: int, value: bool): Command =
    { op: op, actor: actor, id: id, expected: expected, key: key, value: value }
  var rows: (int, int) -> Row
  var present: Set[(int, int)]
  var receipts: (int, int) -> Receipt
  var counts: int -> int
  var publicCounts: int -> int
  var publication: int // 0=withdrawn, 1=r1, 2=r2; revision identity, not generation
  var shown: Set[int]
  var commentsOpen: bool
  var authority: bool
  var command: Command
  var outcome: str
  var historical: Row
  var previousRows: (int, int) -> Row
  var previousCounts: int -> int
  var previousReceipts: (int, int) -> Receipt
  var previousPresent: Set[(int, int)]
  // Independent specification clock counts accepted intents even in the mutant.
  var serial: (int, int) -> int
  var previousSerial: (int, int) -> int
  var cursor: int
  pure def visible(id: int, pub: int, vs: Set[int]): bool =
    pub != 0 and (if (id <= 2) id == pub else vs.contains(id))
  pure def publicOf(cs: int -> int, pub: int, vs: Set[int]): int -> int =
    ids.filter(id => visible(id, pub, vs)).mapBy(id => cs.get(id))
  action init = all {
    rows' = pairs.mapBy(_ => initial), present' = Set(), receipts' = Map(),
    counts' = ids.mapBy(_ => 0), publicCounts' = Map(1 -> 0, 3 -> 0, 4 -> 0),
    publication' = 1, shown' = Set(3, 4), commentsOpen' = true, authority' = true,
    command' = cmd("init", 0, 0, 0, 0, false), outcome' = "init", historical' = initial,
    previousRows' = pairs.mapBy(_ => initial), previousCounts' = ids.mapBy(_ => 0),
    previousPresent' = Set(), previousReceipts' = Map(),
    serial' = pairs.mapBy(_ => 1), previousSerial' = pairs.mapBy(_ => 1), cursor' = 0,
  }
  action setWith(c: Command, fenceNoop: bool): bool = {
    val pair = (c.actor, c.id)
    val current = rows.get(pair)
    val privateRemoval = not(c.value) and present.contains(pair)
    val key = (c.actor, c.key)
    val replay = receipts.keys().contains(key)
    val result = if (not(authority)) "authority_unavailable"
      else if (not(privateRemoval) and not(visible(c.id, publication, shown))) "target_unavailable"
      else if (replay) { if (receipts.get(key).command == c) "replay" else "key_reused" }
      else if (c.value and c.actor == author(c.id)) "own_content"
      else if (c.expected != current.order) "conflict"
      else if (c.value == current.selected) "accepted_unchanged"
      else "accepted_changed"
    val accepted = Set("accepted_unchanged", "accepted_changed").contains(result)
    val changed = c.value != current.selected
    val updated = { selected: c.value, version: current.version + (if (changed) 1 else 0),
      order: current.order + (if (fenceNoop or changed) 1 else 0) }
    val nextCounts = if (accepted and changed) counts.set(c.id, counts.get(c.id) + (if (c.value) 1 else -1)) else counts
    all {
      rows' = if (accepted) rows.set(pair, updated) else rows,
      present' = if (accepted) present.union(Set(pair)) else present,
      receipts' = if (accepted) receipts.put(key, { command: c, committed: updated }) else receipts,
      counts' = nextCounts, publicCounts' = publicOf(nextCounts, publication, shown),
      publication' = publication, shown' = shown, commentsOpen' = commentsOpen, authority' = authority,
      command' = c, outcome' = result,
      historical' = if (result == "replay") receipts.get(key).committed else initial,
      previousRows' = rows, previousCounts' = counts, previousPresent' = present,
      previousReceipts' = receipts, previousSerial' = serial,
      serial' = if (accepted) serial.set(pair, serial.get(pair) + 1) else serial,
      cursor' = if (cursor < 50) cursor + 1 else cursor,
    }
  }
  // Synthetic environment inputs only, not implementations of the missing lifecycles.
  action environment(c: Command): bool = {
    val pub = if (c.op == "publish") c.id else publication
    val vs = if (c.op == "show") { if (c.value) shown.union(Set(c.id)) else shown.exclude(Set(c.id)) } else shown
    all {
      rows' = rows, present' = present, receipts' = receipts, counts' = counts,
      publicCounts' = publicOf(counts, pub, vs), publication' = pub, shown' = vs,
      commentsOpen' = if (c.op == "mode") c.value else commentsOpen,
      authority' = if (c.op == "authority") c.value else authority,
      command' = c, outcome' = "facts_set", historical' = initial,
      previousRows' = rows, previousCounts' = counts, previousPresent' = present,
      previousReceipts' = receipts, previousSerial' = serial, serial' = serial,
      cursor' = if (cursor < 50) cursor + 1 else cursor,
    }
  }
  action dispatch(c: Command): bool = if (c.op == "set") setWith(c, true) else environment(c)
  action step = {
    nondet actor = actors.oneOf()
    nondet id = ids.oneOf()
    nondet key = 1.to(12).oneOf()
    nondet value = Set(true, false).oneOf()
    nondet offset = Set(-1, 0, 1).oneOf()
    nondet pub = 0.to(2).oneOf()
    nondet comment = Set(3, 4).oneOf()
    val expected = if (rows.get((actor, id)).order + offset < 1) 1 else rows.get((actor, id)).order + offset
    any {
      dispatch(cmd("set", actor, id, expected, key, value)),
      environment(cmd("publish", 0, pub, 0, 0, false)),
      environment(cmd("show", 0, comment, 0, 0, value)),
      environment(cmd("mode", 0, 0, 0, 0, value)),
      environment(cmd("authority", 0, 0, 0, 0, value)),
    }
  }
  pure val adversarial = List(
    cmd("set", 3, 1, 1, 1, false), // B=OFF first, same state but new acceptance
    cmd("set", 3, 1, 1, 2, true),  // delayed A=ON must conflict
    cmd("set", 3, 1, 1, 3, false), // stale same-state with another key
    cmd("set", 3, 1, 2, 2, true),  // NEW intent; prior rejected key was not retained
    cmd("set", 3, 1, 2, 2, true),  // exact replay
    cmd("set", 3, 1, 3, 4, false),
    cmd("set", 3, 1, 2, 2, true),  // historical ON, observed OFF
    cmd("set", 3, 1, 2, 5, true),  // stale different key
    cmd("set", 3, 1, 3, 2, false), // same key, different payload
    cmd("set", 3, 1, 4, 5, true),  // ABA retains order
    cmd("set", 3, 1, 5, 6, false),
    cmd("set", 1, 1, 1, 1, true),  // own work
    cmd("mode", 0, 0, 0, 0, false),
    cmd("set", 3, 3, 1, 7, true),  // closed text does not block endorsement
    cmd("set", 2, 4, 1, 7, true),  // reply independently; key actor-scoped
    cmd("show", 0, 3, 0, 0, false),
    cmd("set", 3, 3, 2, 8, false), // own removal after hide
    cmd("set", 3, 3, 3, 9, true),  // hidden select refused
    cmd("publish", 0, 2, 0, 0, false),
    cmd("set", 3, 1, 6, 9, true),  // old revision select refused
    cmd("set", 3, 2, 1, 1, true),  // actor-wide key reuse across targets
    cmd("set", 3, 2, 1, 9, true),  // independent new revision state/count
    cmd("set", 3, 1, 6, 10, false),
    cmd("publish", 0, 0, 0, 0, false),
    cmd("set", 3, 2, 2, 11, false), // own removal after withdrawal
    cmd("set", 2, 4, 2, 10, true),
    cmd("authority", 0, 0, 0, 0, false),
    cmd("set", 2, 4, 2, 11, false), // no current capability
    cmd("authority", 0, 0, 0, 0, true),
    cmd("set", 2, 4, 2, 11, false),
    cmd("publish", 0, 1, 0, 0, false),
    cmd("show", 0, 3, 0, 0, true),
    cmd("set", 1, 3, 1, 1, true),  // work author may endorse another writer's root
  )
  action adversarialStep = all { cursor < adversarial.length(), dispatch(adversarial.nth(cursor)) }
  action mutatedNoopStep = all { cursor < 2, setWith(adversarial.nth(cursor), false) }
  val isAccepted = Set("accepted_changed", "accepted_unchanged").contains(outcome)
  val acceptedAgainstLatest = not(isAccepted) or command.expected == previousSerial.get((command.actor, command.id))
  val acceptedFence = pairs.forall(p => {
    val before = previousRows.get(p)
    val after = rows.get(p)
    if (isAccepted and p == (command.actor, command.id))
      after.order == before.order + 1 and after.selected == command.value and
      after.version == before.version + (if (after.selected != before.selected) 1 else 0)
    else after == before
  })
  val exactCounts = ids.forall(id => counts.get(id) == actors.filter(a => rows.get((a, id)).selected).size())
  val retained = previousPresent.subseteq(present)
  val hiddenCountsStayHidden = publicCounts == publicOf(counts, publication, shown)
  val noSelfEndorsement = ids.forall(id => not(rows.get((author(id), id)).selected))
  val refusalAndReplayDoNotWrite = isAccepted or
    (rows == previousRows and counts == previousCounts and receipts == previousReceipts and present == previousPresent)
  val endorsementSafe = acceptedAgainstLatest and acceptedFence and exactCounts and retained and
    hiddenCountsStayHidden and noSelfEndorsement and refusalAndReplayDoNotWrite
}
