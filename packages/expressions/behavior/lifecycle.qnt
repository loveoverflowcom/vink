// EMO-01A owns playback only; D2T-INV-11 links here. No discussion authority is modeled.
// Three mount-local players, capacity two. Each run is bounded in length, with at most
// three activations per player (the Rust u32 overflow branch has its own code test).
module lifecycle {
  type Player = { phase: str, generation: int }
  type Command = { op: str, id: int, expected: int, flag: bool }
  pure val ids = 0.to(2)
  pure val capacity = 2
  var players: int -> Player
  var previous: int -> Player
  var reduced: bool
  var foreground: bool
  var runtime: bool
  var visible: Set[int]
  var command: Command
  var outcome: str
  var cursor: int

  action init = all {
    players' = ids.mapBy(_ => { phase: "static", generation: 0 }),
    previous' = ids.mapBy(_ => { phase: "static", generation: 0 }),
    reduced' = false, foreground' = true, runtime' = true, visible' = ids,
    command' = { op: "init", id: 0, expected: 0, flag: false },
    outcome' = "init", cursor' = 0,
  }
  pure def active(ps: int -> Player): Set[int] = ids.filter(i => Set("loading", "playing").contains(ps.get(i).phase))

  action apply(cmd: Command, guarded: bool): bool = {
    val rm = if (cmd.op == "reduced") cmd.flag else reduced
    val fg = if (cmd.op == "foreground") cmd.flag else foreground
    val rt = if (cmd.op == "runtime") cmd.flag else runtime
    val vs = if (cmd.op != "visible") visible else if (cmd.flag) visible.union(Set(cmd.id)) else visible.exclude(Set(cmd.id))
    val cur = players.get(cmd.id)
    val blocked = rm or not(fg) or not(rt) or not(vs.contains(cmd.id))
    val live = Set("loading", "playing").contains(cur.phase)
    val environment = Set("reduced", "foreground", "runtime", "visible").contains(cmd.op)
    val eligible = cur.phase != "disposed" and not(blocked)
    val result = if (environment) "facts"
      else if (cmd.op == "activate") {
        if (not(eligible)) "static" else if (live) "already_active"
        else if (active(players).size() >= capacity) "budget" else "load"
      } else if (cmd.op == "ready") {
        if (not(guarded) or (eligible and cur.phase == "loading" and cur.generation == cmd.expected)) "play" else "ignored"
      } else if (Set("complete", "error").contains(cmd.op)) {
        if (live and cur.generation == cmd.expected) cmd.op else "ignored"
      } else if (cmd.op == "dispose") "disposed"
      else if (cmd.op == "cancel") "cancelled" else "ignored"
    val updated = if (environment) ids.mapBy(i => {
      val p = players.get(i)
      if (p.phase != "disposed" and (rm or not(fg) or not(rt) or not(vs.contains(i)))) { phase: "static", generation: p.generation } else p
    }) else if (result == "load") players.put(cmd.id, { phase: "loading", generation: cur.generation + 1 })
      else if (result == "play") players.put(cmd.id, { phase: "playing", generation: cur.generation })
      else if (result == "complete" or (result == "cancelled" and live)) players.put(cmd.id, { phase: "static", generation: cur.generation })
      else if (result == "error") players.put(cmd.id, { phase: "error", generation: cur.generation })
      else if (result == "disposed") players.put(cmd.id, { phase: "disposed", generation: cur.generation })
      else players
    all {
      players' = updated, previous' = players, reduced' = rm, foreground' = fg,
      runtime' = rt, visible' = vs, command' = cmd, outcome' = result,
      cursor' = if (cursor < 40) cursor + 1 else cursor,
    }
  }

  action step = {
    nondet op = Set("activate", "ready", "complete", "error", "cancel", "dispose", "reduced", "foreground", "runtime", "visible").oneOf()
    nondet id = ids.oneOf()
    nondet expected = 0.to(3).oneOf()
    nondet flag = Set(true, false).oneOf()
    all { (op != "activate" or players.get(id).generation < 3), apply({ op: op, id: id, expected: expected, flag: flag }, true) }
  }
  pure def c(op: str, id: int, expected: int, flag: bool): Command = { op: op, id: id, expected: expected, flag: flag }
  pure val scenario = List(
    c("activate", 0, 0, false), c("activate", 0, 0, false),
    c("activate", 1, 0, false), c("activate", 2, 0, false),
    c("ready", 0, 1, false), c("complete", 0, 1, false), c("complete", 0, 1, false),
    c("activate", 2, 0, false), c("cancel", 2, 0, false), c("ready", 2, 1, false),
    c("activate", 2, 0, false), c("ready", 2, 1, false), c("ready", 2, 2, false),
    c("visible", 2, 0, false), c("ready", 2, 2, false), c("visible", 2, 0, true),
    c("reduced", 0, 0, true), c("activate", 0, 0, false), c("reduced", 0, 0, false),
    c("activate", 0, 0, false), c("error", 0, 2, false), c("activate", 0, 0, false),
    c("foreground", 0, 0, false), c("ready", 0, 3, false), c("foreground", 0, 0, true),
    c("runtime", 0, 0, false), c("activate", 1, 0, false), c("runtime", 0, 0, true),
    c("activate", 1, 0, false), c("dispose", 1, 0, false), c("ready", 1, 2, false),
    c("dispose", 1, 0, false), c("activate", 1, 0, false)
  )
  pure val late = List(c("activate", 0, 0, false), c("dispose", 0, 0, false), c("ready", 0, 1, false))
  action scenarioStep = all { cursor < scenario.length(), apply(scenario.nth(cursor), true) }
  action lateStep = all { cursor < late.length(), apply(late.nth(cursor), true) }
  // Meaningful negative control: a type-correct missing callback guard resurrects disposed.
  action unguardedLateStep = all { cursor < late.length(), apply(late.nth(cursor), false) }

  val budgetSafe = active(players).size() <= capacity
  val eligiblePlayback = active(players).forall(i => not(reduced) and foreground and runtime and visible.contains(i))
  val terminal = ids.forall(i => previous.get(i).phase != "disposed" or players.get(i).phase == "disposed")
  val callbackFence = outcome != "play" or (previous.get(command.id).phase == "loading" and previous.get(command.id).generation == command.expected)
  val activationRequired = ids.forall(i => players.get(i).generation == previous.get(i).generation or
    (command.op == "activate" and command.id == i and players.get(i).generation == previous.get(i).generation + 1))
  val lifecycleSafe = budgetSafe and eligiblePlayback and terminal and callbackFence and activationRequired
}
