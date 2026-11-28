// Điểm Vink V1 (#600, #644): one entitlement of one actor with two anchors (revisions 1 and 2),
// its recipient's current count and the Q41 credit machines of two independent calendars.
// Week and month clocks advance separately; period 0 is the partial period before activation
// readiness and `start` = 1 is each calendar's first full period (Q33). Source facts are
// synthetic, serialized inputs (no SQL, no auth); the worker re-reads them after each change.
module reputation {
  type Command = { op: str, anchor: int }
  type Credit = { state: str, pending: int }
  type Step = { credit: Credit, delta: int }

  pure val anchors = Set(1, 2)
  pure val start = 1
  pure val horizon = 5
  pure val unstarted = { state: "unstarted", pending: 0 }
  pure def cmd(op: str, anchor: int): Command = { op: op, anchor: anchor }

  var cursor: int
  var command: Command
  var outcome: str
  // Authoritative source facts after the command.
  var selected: Set[int]
  var hidden: Set[int]
  var public: bool
  var mature: bool
  var deleted: bool
  var week: int
  var month: int
  // What production must reproduce.
  var effective: bool
  var terminal: bool
  var weekCredit: Credit
  var monthCredit: Credit
  var currentDelta: int
  var weekDelta: int
  var monthDelta: int
  // Specification ledgers: net credit per period, the recipient's count from this entitlement
  // and the period of the first credit ever granted (0 = none).
  var weekNet: int -> int
  var monthNet: int -> int
  var current: int
  var firstWeekCredit: int
  var firstMonthCredit: int

  pure def eff(sel: Set[int], hid: Set[int], pub: bool, mat: bool, del: bool): bool =
    not(del) and pub and mat and sel.exclude(hid).size() > 0

  // Q41 table; `anyPeriod` is the mutant that lets a withdrawal be re-credited in a later period.
  pure def creditStep(c: Credit, input: str, period: int, before: bool, anyPeriod: bool): Step =
    if (period < start) { credit: c, delta: 0 }
    else {
      val s = if (c.state == "unstarted") { if (before) { state: "burned", pending: 0 } else { state: "unseen", pending: 0 } }
        else if (c.state == "withdrawn" and period > c.pending and not(anyPeriod)) { state: "burned", pending: 0 }
        else c
      if (s.state == "unseen" and input == "on") { credit: { state: "active", pending: 0 }, delta: 1 }
      else if (s.state == "active" and input == "off") { credit: { state: "withdrawn", pending: period }, delta: -1 }
      else if (s.state == "withdrawn" and input == "on" and (s.pending == period or anyPeriod))
        { credit: { state: "active", pending: 0 }, delta: 1 }
      else if (s.state == "active" and (input == "off_parked" or input == "retire")) { credit: { state: "burned", pending: 0 }, delta: 0 }
      else if ((s.state == "unseen" or s.state == "withdrawn") and input == "retire") { credit: { state: "burned", pending: 0 }, delta: 0 }
      else if (s.state == "unseen" and input == "burn") { credit: { state: "burned", pending: 0 }, delta: 0 }
      else { credit: s, delta: 0 }
    }

  pure def inputOf(cause: str, before: bool, after: bool, sel: Set[int]): str =
    if (cause == "terminal") "retire"
    else if (cause == "community" and not(before) and after) "on"
    else if (cause == "community" and before and not(after)) "off"
    else if (cause == "community" and not(before) and not(after) and sel.size() == 0) "off_parked"
    else if (cause == "maturity" and not(before) and after) "burn"
    else "neutral"

  pure def addNet(net: int -> int, period: int, delta: int): int -> int =
    if (period < start or delta == 0) net
    else if (net.keys().contains(period)) net.set(period, net.get(period) + delta)
    else net.put(period, delta)

  action init = all {
    cursor' = 0, command' = cmd("init", 0), outcome' = "init",
    selected' = Set(), hidden' = Set(), public' = true, mature' = false, deleted' = false,
    week' = 0, month' = 0,
    effective' = false, terminal' = false, weekCredit' = unstarted, monthCredit' = unstarted,
    currentDelta' = 0, weekDelta' = 0, monthDelta' = 0,
    weekNet' = Map(), monthNet' = Map(), current' = 0, firstWeekCredit' = 0, firstMonthCredit' = 0,
  }

  // One reaction of the pure decision to re-read facts. `perAnchor` is the mutant that counts
  // each newly selected anchor instead of the entitlement.
  action react(c: Command, cause: str, sel: Set[int], hid: Set[int], pub: bool, mat: bool, del: bool,
               anyPeriod: bool, perAnchor: bool): bool = {
    val before = effective
    val after = if (cause == "terminal") false else eff(sel, hid, pub, mat, del)
    val input = inputOf(cause, before, after, sel)
    val ws = if (terminal) { credit: weekCredit, delta: 0 } else creditStep(weekCredit, input, week, before, anyPeriod)
    val ms = if (terminal) { credit: monthCredit, delta: 0 } else creditStep(monthCredit, input, month, before, anyPeriod)
    val cd = if (terminal) 0
      else if (perAnchor and c.op == "select") 1
      else if (not(before) and after) 1 else if (before and not(after)) -1 else 0
    all {
      cursor' = cursor + 1, command' = c, outcome' = if (terminal) "ignored" else input,
      selected' = sel, hidden' = hid, public' = pub, mature' = mat, deleted' = del,
      week' = week, month' = month,
      effective' = if (terminal) effective else after,
      terminal' = (terminal or cause == "terminal"),
      weekCredit' = ws.credit, monthCredit' = ms.credit,
      currentDelta' = cd, weekDelta' = ws.delta, monthDelta' = ms.delta,
      weekNet' = addNet(weekNet, week, ws.delta), monthNet' = addNet(monthNet, month, ms.delta),
      current' = current + cd,
      firstWeekCredit' = if (firstWeekCredit == 0 and ws.delta == 1) week else firstWeekCredit,
      firstMonthCredit' = if (firstMonthCredit == 0 and ms.delta == 1) month else firstMonthCredit,
    }
  }

  action tick(c: Command): bool = all {
    if (c.op == "next_week") week < horizon else month < horizon,
    cursor' = cursor + 1, command' = c, outcome' = "time",
    selected' = selected, hidden' = hidden, public' = public, mature' = mature, deleted' = deleted,
    week' = if (c.op == "next_week") week + 1 else week,
    month' = if (c.op == "next_month") month + 1 else month,
    effective' = effective, terminal' = terminal, weekCredit' = weekCredit, monthCredit' = monthCredit,
    currentDelta' = 0, weekDelta' = 0, monthDelta' = 0,
    weekNet' = weekNet, monthNet' = monthNet, current' = current,
    firstWeekCredit' = firstWeekCredit, firstMonthCredit' = firstMonthCredit,
  }

  // Source preconditions: a selection needs a public, unhidden anchor and a live account; a
  // removal of one's own selection is always allowed (LVE own removal).
  action dispatchWith(c: Command, anyPeriod: bool, perAnchor: bool): bool = {
    val a = c.anchor
    if (c.op == "select") all {
      not(deleted), public, not(hidden.contains(a)), not(selected.contains(a)),
      react(c, "community", selected.union(Set(a)), hidden, public, mature, deleted, anyPeriod, perAnchor),
    }
    else if (c.op == "unselect") all {
      not(deleted), selected.contains(a),
      react(c, "community", selected.exclude(Set(a)), hidden, public, mature, deleted, anyPeriod, perAnchor),
    }
    else if (c.op == "hide") all { not(hidden.contains(a)),
      react(c, "neutral", selected, hidden.union(Set(a)), public, mature, deleted, anyPeriod, perAnchor) }
    else if (c.op == "unhide") all { hidden.contains(a),
      react(c, "neutral", selected, hidden.exclude(Set(a)), public, mature, deleted, anyPeriod, perAnchor) }
    else if (c.op == "privatize") all { public,
      react(c, "neutral", selected, hidden, false, mature, deleted, anyPeriod, perAnchor) }
    else if (c.op == "publish") all { not(public), not(deleted),
      react(c, "neutral", selected, hidden, true, mature, deleted, anyPeriod, perAnchor) }
    else if (c.op == "mature") all { not(mature),
      react(c, "maturity", selected, hidden, public, true, deleted, anyPeriod, perAnchor) }
    else if (c.op == "delete") all { not(deleted),
      react(c, "terminal", Set(), hidden, public, mature, true, anyPeriod, perAnchor) }
    else tick(c)
  }
  action dispatch(c: Command): bool = dispatchWith(c, false, false)

  action step = {
    nondet a = anchors.oneOf()
    any {
      dispatch(cmd("select", a)), dispatch(cmd("unselect", a)), dispatch(cmd("hide", a)),
      dispatch(cmd("unhide", a)), dispatch(cmd("privatize", 0)), dispatch(cmd("publish", 0)),
      dispatch(cmd("mature", 0)), dispatch(cmd("delete", 0)),
      dispatch(cmd("next_week", 0)), dispatch(cmd("next_month", 0)),
    }
  }

  // Q41 oracle with the month crossing inside it, then same-period loops and two anchors.
  pure val oracle = List(
    cmd("next_week", 0), cmd("next_month", 0), cmd("mature", 0),
    cmd("select", 1),     // week 1 +1, month 1 +1
    cmd("next_week", 0),
    cmd("unselect", 1),   // week 2 -1, month 1 -1
    cmd("next_week", 0),
    cmd("select", 1),     // week 3 0 (burned), month 1 +1 (same month)
    cmd("next_week", 0),
    cmd("unselect", 1),   // week 4 0, month 1 -1
    cmd("select", 1),     // week 4 0, month 1 +1
    cmd("next_month", 0),
    cmd("select", 2),     // second anchor: no delta
    cmd("unselect", 1),   // one anchor left: no delta
    cmd("unselect", 2),   // month 2 -1
    cmd("select", 2),     // month 2 +1 (same period compensation)
    cmd("unselect", 2),   // month 2 -1
    cmd("next_month", 0),
    cmd("select", 1),     // month 3 0: withdrawal burned at the boundary
  )
  // Neutral lifecycle, parking, retirement and terminal deletion.
  pure val lifecycle = List(
    cmd("next_week", 0), cmd("next_month", 0), cmd("mature", 0),
    cmd("select", 1),     // +1 / +1
    cmd("hide", 1),       // current -1, credit parked
    cmd("unhide", 1),     // current +1, no new credit
    cmd("privatize", 0),  // current -1, parked
    cmd("unselect", 1),   // OFF while parked: retire, no debit
    cmd("publish", 0),
    cmd("select", 1),     // current +1, credit 0 (burned)
    cmd("next_week", 0),
    cmd("select", 2),
    cmd("hide", 2),       // still valid through anchor 1
    cmd("delete", 0),     // DEL-05: current -1, retired
    cmd("unhide", 2),     // ignored: no resurrection
    cmd("privatize", 0),  // ignored
  )
  // Activation partial period and maturity (Q33, Q42, Q37).
  pure val maturity = List(
    cmd("select", 1),     // period 0, immature: held
    cmd("next_week", 0), cmd("next_month", 0),
    cmd("mature", 0),     // current +1, both calendars burned
    cmd("unselect", 1),   // current -1, no debit after burn
    cmd("select", 1),     // 0
    cmd("next_week", 0),
    cmd("select", 2),     // nothing new
    cmd("unselect", 1),
    cmd("unselect", 2),
  )
  pure val heldOffThenOn = List(
    cmd("next_week", 0), cmd("next_month", 0),
    cmd("select", 1),     // held
    cmd("unselect", 1),   // OFF before maturity
    cmd("mature", 0),     // nothing selected: nothing recorded
    cmd("select", 1),     // first real ON after the threshold: +1 / +1
    cmd("next_week", 0),
    cmd("unselect", 1),   // week 2 -1, month 1 -1
  )
  // Baseline: effective at the first boundary after activation is burned (Q33).
  pure val baseline = List(
    cmd("mature", 0),
    cmd("select", 1),     // partial period 0: current +1, no credit
    cmd("next_week", 0),
    cmd("unselect", 1),   // week 1: baseline burned, no debit; month still partial
    cmd("next_month", 0),
    cmd("select", 2),     // week 1 0 (burned); month 1 starts unseen -> +1
  )
  action oracleStep = all { cursor < oracle.length(), dispatch(oracle.nth(cursor)) }
  action lifecycleStep = all { cursor < lifecycle.length(), dispatch(lifecycle.nth(cursor)) }
  action maturityStep = all { cursor < maturity.length(), dispatch(maturity.nth(cursor)) }
  action heldStep = all { cursor < heldOffThenOn.length(), dispatch(heldOffThenOn.nth(cursor)) }
  action baselineStep = all { cursor < baseline.length(), dispatch(baseline.nth(cursor)) }

  pure val relike = List(cmd("next_week", 0), cmd("next_month", 0), cmd("mature", 0), cmd("select", 1),
    cmd("next_week", 0), cmd("unselect", 1), cmd("next_week", 0), cmd("select", 1))
  action mutatedRelikeStep = all { cursor < relike.length(), dispatchWith(relike.nth(cursor), true, false) }
  pure val twoAnchors = List(cmd("next_week", 0), cmd("mature", 0), cmd("select", 1), cmd("select", 2))
  action mutatedAnchorStep = all { cursor < twoAnchors.length(), dispatchWith(twoAnchors.nth(cursor), false, true) }

  pure def total(net: int -> int): int = net.keys().fold(0, (acc, k) => acc + net.get(k))
  val periodBounds = weekNet.keys().forall(p => weekNet.get(p) >= -1 and weekNet.get(p) <= 1) and
    monthNet.keys().forall(p => monthNet.get(p) >= -1 and monthNet.get(p) <= 1)
  val lifetimeBounds = total(weekNet) >= 0 and total(weekNet) <= 1 and total(monthNet) >= 0 and total(monthNet) <= 1
  val onlyFirstCreditIsGrowth = weekNet.keys().forall(p => weekNet.get(p) == 1 implies p == firstWeekCredit) and
    monthNet.keys().forall(p => monthNet.get(p) == 1 implies p == firstMonthCredit)
  val currentMatchesEffective = current == (if (effective) 1 else 0)
  val effectiveFollowsFacts = effective == eff(selected, hidden, public, mature, deleted)
  val terminalIsFinal = deleted implies (terminal and not(effective))
  val noCreditBeforeStart = weekNet.keys().forall(p => p >= start) and monthNet.keys().forall(p => p >= start)
  val reputationSafe = periodBounds and lifetimeBounds and onlyFirstCreditIsGrowth and
    currentMatchesEffective and effectiveFollowsFacts and terminalIsFinal and noCreditBeforeStart
}
