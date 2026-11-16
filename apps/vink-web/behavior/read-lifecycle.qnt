// READ-LIFECYCLE: #202/#212, consumed by #201/#211 and #205/#223.
// Architecture proposal VDR-WEB-READ-LIFECYCLE, not product authority.
// One logical owner; scope is an opaque query/session identity, NOT an auth check.
// No HTTP, DTO, expiry, storage, DOM or SQL. See README.md for shell assumptions.
module read_lifecycle {
  type Owner = { scope: int, generation: int, flight: int, disposed: bool }
  type Ticket = { scope: int, generation: int, intent: str, input: int }
  type Chain = { ids: Set[int], cursors: Set[int], next: int, pages: int, generation: int }
  type State = { owner: Owner, chain: Chain, tickets: int -> Ticket, serial: int }
  type Command = { op: str, intent: str, ticket: int, scope: int, input: int, rows: List[int], next: int }

  pure def owner(s: int): Owner = { scope: s, generation: 0, flight: 0, disposed: false }
  pure val emptyChain: Chain = { ids: Set(), cursors: Set(), next: 0, pages: 0, generation: 0 }
  pure def cmd(op: str): Command =
    { op: op, intent: "initial", ticket: 0, scope: 1, input: 0, rows: List(), next: 0 }
  pure def begin(intent: str, input: int): Command = cmd("begin").with("intent", intent).with("input", input)
  pure def page(ticket: int, rows: List[int], after: int): Command =
    cmd("page").with("ticket", ticket).with("rows", rows).with("next", after)
  pure def finish(ticket: int): Command = cmd("finish").with("ticket", ticket)
  pure def scope(s: int): Command = cmd("scope").with("scope", s)

  var current: State
  var previous: State
  var command: Command
  var outcome: str
  var cursor: int

  pure def accepts(s: State, id: int): bool =
    if (not(s.tickets.keys().contains(id))) false
    else {
      val t = s.tickets.get(id)
      not(s.owner.disposed) and s.owner.flight == id and
        s.owner.scope == t.scope and s.owner.generation == t.generation
    }

  pure def validPage(s: State, c: Command): bool =
    if (not(s.tickets.keys().contains(c.ticket))) false
    else {
      val t = s.tickets.get(c.ticket)
      val isAppend = t.intent == "append"
      val ids = c.rows.indices().map(i => c.rows.nth(i))
      (not(isAppend) or (t.input != 0 and t.input == s.chain.next)) and
        ids.size() == c.rows.length() and
        (not(isAppend) or (ids.intersect(s.chain.ids).size() == 0)) and
        (c.next == 0 or (ids.size() > 0 and c.next != t.input and
          (not(isAppend) or not(s.chain.cursors.contains(c.next)))))
    }

  action init = all {
    current' = { owner: owner(1), chain: emptyChain, tickets: Map(), serial: 0 },
    previous' = { owner: owner(1), chain: emptyChain, tickets: Map(), serial: 0 },
    command' = cmd("init"), outcome' = "init", cursor' = 0,
  }
  action commit(c: Command, result: str, s: State): bool = all {
    current' = s, previous' = current, command' = c, outcome' = result, cursor' = cursor + 1,
  }
  // Mutants remove just one guard, leaving syntax/types and reachable scenarios intact.
  action dispatch(c: Command, unfenced: bool, uncheckedPage: bool): bool = {
    val s = current
    val o = s.owner
    val invalidated = o.with("generation", o.generation + 1).with("flight", 0)
    if (c.op == "begin") {
      val isAppend = c.intent == "append"
      val occupied = o.flight != 0
      val maySupersede = if (occupied) s.tickets.get(o.flight).intent == "append" else false
      if (o.disposed or (isAppend != (c.input != 0)) or (occupied and (isAppend or not(maySupersede))))
        commit(c, "blocked", s)
      else {
        val fresh = s.serial + 1
        val nextOwner = (if (occupied) invalidated else o).with("flight", fresh)
        val ticket = { scope: o.scope, generation: nextOwner.generation, intent: c.intent, input: c.input }
        commit(c, "started", s.with("owner", nextOwner).with("serial", fresh)
          .with("tickets", s.tickets.put(fresh, ticket)))
      }
    } else if (c.op == "page") {
      val known = s.tickets.keys().contains(c.ticket)
      if (known and (unfenced or accepts(s, c.ticket)) and (uncheckedPage or validPage(s, c))) {
        val isAppend = s.tickets.get(c.ticket).intent == "append"
        val base = if (isAppend) s.chain else emptyChain.with("generation", s.chain.generation + 1)
        val chain = { ids: base.ids.union(c.rows.indices().map(i => c.rows.nth(i))),
          cursors: if (c.next == 0) base.cursors else base.cursors.union(Set(c.next)),
          next: c.next, pages: base.pages + 1, generation: base.generation }
        commit(c, "accepted", s.with("chain", chain))
      } else commit(c, "rejected", s)
    } else if (c.op == "finish") {
      if (accepts(s, c.ticket)) commit(c, "finished", s.with("owner", o.with("flight", 0)))
      else commit(c, "ignored", s)
    } else if (c.op == "scope")
      commit(c, "changed", s.with("owner", invalidated.with("scope", c.scope)))
    else if (c.op == "invalidate") commit(c, "changed", s.with("owner", invalidated))
    else if (c.op == "dispose") commit(c, "disposed", s.with("owner", invalidated.with("disposed", true)))
    else if (c.op == "remount")
      commit(c, "remounted", s.with("owner", owner(c.scope)).with("chain", emptyChain))
    else commit(c, "unknown", s)
  }

  action step = {
    nondet id = 0.to(12).oneOf()
    nondet input = 0.to(3).oneOf()
    nondet after = 0.to(3).oneOf()
    nondet rows = Set(List(), List(1), List(2), List(3), List(1, 2), List(1, 1)).oneOf()
    nondet intent = Set("initial", "refresh", "append", "restart").oneOf()
    nondet newScope = Set(1, 2).oneOf()
    nondet c = Set(page(id, rows, after), finish(id), scope(newScope), cmd("invalidate"),
      cmd("dispose"), cmd("remount").with("scope", newScope)).oneOf()
    any {
      all { current.serial < 12, dispatch(begin(intent, input), false, false) },
      dispatch(c, false, false),
    }
  }

  // Same scope after A→B→A, disposal/remount, and equal generations must not revive old requests.
  pure val fences = List(
    begin("initial", 0), scope(2), scope(1), begin("initial", 0),
    page(1, List(1), 1), finish(1), page(2, List(2), 2), finish(2),
    begin("refresh", 0), cmd("dispose"), page(3, List(3), 0), begin("initial", 0),
    cmd("remount"), begin("initial", 0), page(1, List(1), 0),
    page(4, List(3), 0), finish(4),
    begin("refresh", 0), cmd("invalidate"), page(5, List(1), 0), finish(5)
  )
  action fencesStep = all { cursor < fences.length(), dispatch(fences.nth(cursor), false, false) }

  pure val paging = List(
    begin("append", 0), begin("initial", 1), begin("initial", 0), begin("refresh", 0),
    page(1, List(1), 1), finish(1), begin("append", 1),
    page(2, List(1), 2), page(2, List(2, 2), 2), page(2, List(), 2),
    page(2, List(2), 1), page(2, List(2), 2), finish(2),
    begin("append", 2), page(3, List(3), 1), finish(3),
    begin("append", 3), page(4, List(3), 0), finish(4),
    begin("append", 2), begin("refresh", 0), page(5, List(3), 0), finish(5),
    page(6, List(3), 3), finish(6), begin("append", 3), page(7, List(1), 0), finish(7),
    begin("restart", 0), page(8, List(), 0), finish(8)
  )
  action pagingStep = all { cursor < paging.length(), dispatch(paging.nth(cursor), false, false) }

  pure val late = List(begin("initial", 0), cmd("remount"), begin("initial", 0), page(1, List(1), 0))
  action unfencedStep = all { cursor < late.length(), dispatch(late.nth(cursor), true, false) }
  pure val overlap = List(begin("initial", 0), page(1, List(1), 1), finish(1),
    begin("append", 1), page(2, List(1), 2))
  action uncheckedPageStep = all { cursor < overlap.length(), dispatch(overlap.nth(cursor), false, true) }

  val ticketFence = outcome != "accepted" or accepts(previous, command.ticket)
  val pageIntegrity = outcome != "accepted" or validPage(previous, command)
  val rejectedKeepsChain = outcome != "rejected" or current.chain == previous.chain
  val headReplacesTail = if (outcome == "accepted") {
    val t = previous.tickets.get(command.ticket)
    t.intent == "append" or (current.chain.ids == command.rows.indices().map(i => command.rows.nth(i)) and current.chain.pages == 1
      and current.chain.generation == previous.chain.generation + 1)
  } else true
  val readSafe = ticketFence and pageIntegrity and rejectedKeepsChain and headReplacesTail
}
