// D2T-01: append-only structure only. One implicit thread, comment IDs 1..4.
// Sentinel 0 means no reply context. rank is model-only insertion-order evidence.
module structure {
  type Placement = { root: int, reply_to: int, rank: int }
  type Command = { op: str, id: int, target: int }

  pure val ids = 1.to(4)
  var placements: int -> Placement
  var previous: int -> Placement
  var command: Command
  var outcome: str
  // Only deterministic scenarios use cursor; saturating it keeps the state space finite.
  var cursor: int

  action init = all {
    placements' = Map(),
    previous' = Map(),
    command' = { op: "init", id: 0, target: 0 },
    outcome' = "init",
    cursor' = 0,
  }

  action apply(cmd: Command, normalize: bool): bool = {
    val duplicate = placements.keys().contains(cmd.id)
    val missing = cmd.op == "reply" and not(placements.keys().contains(cmd.target))
    val result = if (duplicate) "duplicate" else if (missing) "missing" else "created"
    val updated = if (result != "created") placements
      else if (cmd.op == "root") placements.put(cmd.id, {
        root: cmd.id, reply_to: 0, rank: placements.keys().size(),
      })
      else {
        val target = placements.get(cmd.target)
        placements.put(cmd.id, {
          // false is ONLY for the explicit negative-control action below.
          root: if (normalize) target.root else cmd.target,
          reply_to: if (target.root == cmd.target) 0 else cmd.target,
          rank: placements.keys().size(),
        })
      }
    all {
      placements' = updated,
      previous' = placements,
      command' = cmd,
      outcome' = result,
      cursor' = if (cursor < 10) cursor + 1 else cursor,
    }
  }

  action step = {
    nondet op = Set("root", "reply").oneOf()
    nondet id = ids.oneOf()
    nondet target = ids.oneOf()
    apply({ op: op, id: id, target: if (op == "root") 0 else target }, true)
  }

  // Inputs only: expected placements are always computed by apply, never fixture code.
  pure val chain = List(
    { op: "root", id: 1, target: 0 },
    { op: "reply", id: 2, target: 1 },
    { op: "reply", id: 3, target: 3 }, // missing/self-reference
    { op: "reply", id: 3, target: 4 }, // missing
    { op: "reply", id: 1, target: 4 }, // duplicate takes precedence over missing
    { op: "reply", id: 3, target: 2 },
    { op: "reply", id: 4, target: 3 },
    { op: "root", id: 2, target: 0 }, // duplicate
    { op: "reply", id: 1, target: 4 }, // cannot reparent root/create a cycle
    { op: "reply", id: 2, target: 2 }, // duplicate/self-reference
  )
  pure val separateRoots = List(
    { op: "root", id: 1, target: 0 },
    { op: "root", id: 2, target: 0 },
    { op: "reply", id: 3, target: 2 },
    { op: "reply", id: 4, target: 3 },
    { op: "reply", id: 3, target: 1 }, // cannot move between roots
  )

  action chainStep = all { cursor < chain.length(), apply(chain.nth(cursor), true) }
  action rootsStep = all { cursor < separateRoots.length(), apply(separateRoots.nth(cursor), true) }
  action mutatedChainStep = all { cursor < chain.length(), apply(chain.nth(cursor), false) }

  // Every parent is a present root; replies cannot introduce a third structural tier.
  val rootIsRoot = placements.keys().forall(id => {
    val root = placements.get(id).root
    placements.keys().contains(root) and placements.get(root).root == root
  })

  // Context only refers to a reply under the same root, never to a root itself.
  val canonicalContext = placements.keys().forall(id => {
    val p = placements.get(id)
    if (p.root == id) p.reply_to == 0
    else p.reply_to == 0 or (
      placements.keys().contains(p.reply_to) and
      placements.get(p.reply_to).root != p.reply_to and
      placements.get(p.reply_to).root == p.root
    )
  })

  // Root and optional context edges strictly decrease insertion rank (except root self).
  // Thus no directed cycle is possible, regardless of numeric ID order.
  val noCycles = placements.keys().forall(id => {
    val p = placements.get(id)
    (p.root == id or placements.get(p.root).rank < p.rank) and
    (p.reply_to == 0 or placements.get(p.reply_to).rank < p.rank)
  })

  val immutable = previous.keys().forall(id =>
    placements.keys().contains(id) and placements.get(id) == previous.get(id))
  val rejectionAtomic = outcome == "created" or placements == previous
  val finiteDomain = placements.keys().subseteq(ids) and placements.keys().forall(id => {
    val p = placements.get(id)
    ids.contains(p.root) and (p.reply_to == 0 or ids.contains(p.reply_to)) and
    p.rank >= 0 and p.rank < 4
  })

  // Check error precedence and the exact normalized result for the last input.
  val resultMatchesInput = if (outcome == "init") placements == Map()
    else if (previous.keys().contains(command.id)) outcome == "duplicate"
    else if (command.op == "reply" and not(previous.keys().contains(command.target))) outcome == "missing"
    else {
      val p = placements.get(command.id)
      outcome == "created" and placements.keys().size() == previous.keys().size() + 1 and
      (if (command.op == "root") p.root == command.id and p.reply_to == 0
       else p.root == previous.get(command.target).root and
         p.reply_to == (if (previous.get(command.target).root == command.target) 0 else command.target))
    }

  val structureSafe = rootIsRoot and canonicalContext and noCycles and immutable and
    rejectionAtomic and finiteDomain and resultMatchesInput
}
