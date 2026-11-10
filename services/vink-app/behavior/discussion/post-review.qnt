// Explicit operator obligation. Finite shell: one approved target/ticket, two generations,
// deadline=2 ticks, clock<=3. Authority and SQL commit serialization are assumptions; PostgreSQL
// tests cover the real producer/locks. reset is a labelled fresh-ticket fixture, not admission.
module post_review {
  type Command = { op: str, value: str, held: int, matches: bool }
  pure def cmd(op: str, value: str, held: int, matches: bool): Command = { op: op, value: value, held: held, matches: matches }
  pure def act(op: str): Command = cmd(op,"none",1,true)
  var review: str
  var version: int
  var removed: bool
  var now: int
  var deadline: int
  var generation: int
  var finding: str
  var exhausted: bool
  var receipt: str
  var outcome: str
  var command: Command
  var cursor: int
  var previousVersion: int
  var previousRemoved: bool

  action init = all {
    review'="approved", version'=2, removed'=false, now'=0, deadline'=2,
    generation'=1, finding'="none", exhausted'=false, receipt'="none", outcome'="init",
    command'=act("init"), cursor'=0, previousVersion'=2, previousRemoved'=false,
  }
  action dispatch(c: Command): bool = {
    val owner = c.held == generation
    val closed = receipt != "none"
    val effect = if (not(owner)) "stale" else if (closed) "already"
      else if (not(c.matches)) "invalid" else if (removed) "removed"
      else if (version != 2 or review != "approved") "superseded"
      else if (exhausted or now >= deadline) "held"
      else if (finding == "none") "waiting" else if (finding == "clear") "kept" else "held"
    val applying = c.op == "apply"
    val terminal = Set("kept","held","superseded","removed").contains(effect)
    val fresh = c.op == "reset"
    val human = c.op == "human" and not(removed)
    all {
      review' = if (fresh) "approved" else if (human) c.value else if (applying and effect == "held") "pending" else review,
      version' = if (fresh) 2 else if (human or (applying and effect == "held") or (c.op == "remove" and not(removed))) version+1 else version,
      removed' = if (fresh) false else removed or c.op == "remove",
      now' = if (fresh) 0 else if (c.op == "tick") now+1 else now,
      // Requeue never changes the business deadline.
      deadline' = if (fresh) 2 else deadline,
      generation' = if (fresh) 1 else if (c.op == "reclaim") 2 else generation,
      finding' = if (fresh) "none" else if (c.op == "result") c.value else finding,
      exhausted' = if (fresh) false else exhausted or c.op == "exhaust",
      receipt' = if (fresh) "none" else if (applying and terminal) effect else receipt,
      outcome' = if (applying) effect else if (c.op == "human" and removed) "removed" else c.op,
      command'=c, cursor'=cursor+1, previousVersion'=version, previousRemoved'=removed,
    }
  }
  pure val scenario = List(
    act("apply"), cmd("result","clear",1,true), cmd("human","approved",1,true), act("apply"),
    act("reset"), cmd("result","flagged",1,true), act("reclaim"), act("apply"), cmd("apply","none",2,true), act("requeue"), cmd("apply","none",2,true),
    act("reset"), act("tick"), act("tick"), act("apply"),
    act("reset"), act("exhaust"), act("apply"),
    act("reset"), act("remove"), cmd("result","clear",1,true), act("apply"),
    act("reset"), cmd("apply","none",1,false),
    act("reset"), cmd("result","clear",1,true), act("apply"),
    act("reset"), cmd("result","flagged",1,true), cmd("human","rejected",1,true), cmd("human","approved",1,true), act("apply")
  )
  action scenarioStep = all { cursor < scenario.length(), dispatch(scenario.nth(cursor)) }
  action step = {
    nondet held = Set(1,2).oneOf()
    nondet matches = Set(true,false).oneOf()
    nondet found = Set("clear","flagged","unresolved").oneOf()
    nondet human = Set("approved","rejected").oneOf()
    any {
      dispatch(cmd("apply","none",held,matches)), dispatch(cmd("result",found,held,matches)),
      dispatch(cmd("human",human,held,matches)), dispatch(act("remove")),
      all { now < 3, dispatch(act("tick")) }, dispatch(act("reclaim")),
      dispatch(act("requeue")), dispatch(act("exhaust")), dispatch(act("reset"))
    }
  }
  val safe = all {
    deadline == 2,
    outcome != "kept" or (version == 2 and not(removed) and now < deadline and not(exhausted)),
    outcome != "held" or (version == previousVersion+1 and review == "pending" and not(previousRemoved)),
    not(Set("stale","invalid","superseded","removed","already","waiting").contains(outcome)) or version == previousVersion,
  }
}
