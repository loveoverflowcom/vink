// #120 DISC-04: the discussion outbox lifecycle. One event owes two deliveries (two consumers);
// two workers claim, crash, stall past their lease and come back. Each action is one serialized
// commit: a claim, a fenced completion (effect + outcome in one transaction), a crash (the
// worker forgets its claims and writes nothing), a clock tick, or an operator requeue.
// ASSUMPTIONS, not proofs: SQL row locks serialize these commits; FOR UPDATE SKIP LOCKED, the
// savepoint, timeouts, the process supervisor and PostgreSQL itself are checked in the postgres
// lane. The deadline is modelled as never reached (its rule is a Kani/unit obligation).
// Lease 2 ticks, retry backoff 1 tick doubling to a cap of 2, at most 3 attempts.
module outbox {
  type Delivery = { state: str, attempts: int, generation: int, available: int, outcome: str }
  type Hold = { worker: int, delivery: int, generation: int }
  // claim/complete/requeue name a delivery; complete names the generation the worker holds and
  // the handler's result (applied | retry | fail). crash names a worker; tick names nothing.
  type Command = { op: str, worker: int, delivery: int, generation: int, result: str }
  pure val workers = 1.to(2)
  pure val deliveries = 1.to(2)
  pure val lease = 2
  pure val maxAttempts = 3
  pure val maxNow = 8
  pure def backoff(attempts: int): int = if (attempts <= 1) 1 else 2
  pure val fresh = { state: "ready", attempts: 0, generation: 0, available: 0, outcome: "none" }
  pure val noHold = { worker: 0, delivery: 0, generation: 0 }
  pure def cmd(op: str, worker: int, delivery: int, generation: int, result: str): Command =
    { op: op, worker: worker, delivery: delivery, generation: generation, result: result }
  pure def claimCmd(worker: int, delivery: int): Command = cmd("claim", worker, delivery, 0, "none")
  pure def completeCmd(worker: int, delivery: int, generation: int, result: str): Command =
    cmd("complete", worker, delivery, generation, result)
  pure def crashCmd(worker: int): Command = cmd("crash", worker, 0, 0, "none")
  pure val tickCmd = cmd("tick", 0, 0, 0, "none")
  pure def requeueCmd(delivery: int): Command = cmd("requeue", 0, delivery, 0, "none")

  var store: int -> Delivery
  var holds: Set[Hold]       // what each worker believes it holds (it may be stale)
  var effects: int -> int    // ghost: effects committed per delivery
  var now: int
  var command: Command
  var outcome: str
  var previous: int -> Delivery
  var previousEffects: int -> int
  var cursor: int

  action init = all {
    store' = deliveries.mapBy(_ => fresh), holds' = Set(), effects' = deliveries.mapBy(_ => 0),
    now' = 0, command' = cmd("init", 0, 0, 0, "none"), outcome' = "init", previous' = Map(),
    previousEffects' = Map(), cursor' = 0,
  }

  action record(c: Command, result: str): bool = all {
    command' = c, outcome' = result, previous' = store, previousEffects' = effects,
    cursor' = if (cursor < 60) cursor + 1 else cursor,
  }

  // decide_claim: due ready rows and expired leases; the budget ends in a dead letter.
  action claim(c: Command): bool = {
    val d = store.get(c.delivery)
    val due = d.available <= now
    val result = if (not(due and (d.state == "ready" or d.state == "leased"))) "not_eligible"
      else if (d.attempts >= maxAttempts) "dead_at_claim"
      else if (d.state == "leased") "reclaimed" else "claimed"
    val taken = result == "claimed" or result == "reclaimed"
    val updated = if (result == "dead_at_claim") d.with("state", "dead").with("outcome", "retry_exhausted")
      else if (taken) { state: "leased", attempts: d.attempts + 1, generation: d.generation + 1,
        available: now + lease, outcome: "none" }
      else d
    all {
      store' = store.set(c.delivery, updated),
      holds' = if (taken)
        holds.union(Set({ worker: c.worker, delivery: c.delivery, generation: d.generation + 1 }))
        else holds,
      effects' = effects, now' = now, record(c, result),
    }
  }

  // decide_completion in one transaction with the effect. `fenced` is the mutation switch.
  action completeWith(c: Command, fenced: bool): bool = {
    val d = store.get(c.delivery)
    val held = { worker: c.worker, delivery: c.delivery, generation: c.generation }
    val owner = d.state == "leased" and d.generation == c.generation
    val result = if (fenced and not(owner)) "stale"
      else if (c.result == "applied") "succeeded"
      else if (c.result == "fail") "dead"
      else if (d.attempts >= maxAttempts) "dead"
      else "retry"
    val updated = if (result == "succeeded") d.with("state", "succeeded").with("outcome", "applied")
      else if (result == "dead" and c.result == "fail") d.with("state", "dead").with("outcome", "invalid_payload")
      else if (result == "dead") d.with("state", "dead").with("outcome", "retry_exhausted")
      else if (result == "retry") d.with("state", "ready").with("available", now + backoff(d.attempts))
      else d
    all {
      holds.contains(held),
      store' = store.set(c.delivery, updated),
      holds' = holds.exclude(Set(held)),
      effects' = if (result == "succeeded") effects.set(c.delivery, effects.get(c.delivery) + 1) else effects,
      now' = now, record(c, result),
    }
  }

  action crash(c: Command): bool = all {
    holds.exists(h => h.worker == c.worker),
    store' = store, holds' = holds.filter(h => h.worker != c.worker), effects' = effects,
    now' = now, record(c, "crashed"),
  }

  action tick(c: Command): bool = all {
    now < maxNow,
    store' = store, holds' = holds, effects' = effects, now' = now + 1, record(c, "ticked"),
  }

  // decide_requeue: dead letters only, fresh budget, generation kept.
  action requeue(c: Command): bool = {
    val d = store.get(c.delivery)
    all {
      store' = if (d.state == "dead")
        store.set(c.delivery, d.with("state", "ready").with("attempts", 0).with("available", now)
          .with("outcome", "none"))
        else store,
      holds' = holds, effects' = effects, now' = now,
      record(c, if (d.state == "dead") "requeued" else "requeue_refused"),
    }
  }

  action dispatchWith(c: Command, fenced: bool): bool =
    if (c.op == "claim") claim(c)
    else if (c.op == "complete") completeWith(c, fenced)
    else if (c.op == "crash") crash(c)
    else if (c.op == "requeue") requeue(c)
    else tick(c)
  action dispatch(c: Command): bool = dispatchWith(c, true)

  action step = {
    nondet w = workers.oneOf()
    nondet d = deliveries.oneOf()
    nondet h = holds.union(Set(noHold)).oneOf()
    nondet r = Set("applied", "retry", "fail").oneOf()
    any {
      dispatch(claimCmd(w, d)),
      all { h != noHold, dispatch(completeCmd(h.worker, h.delivery, h.generation, r)) },
      dispatch(crashCmd(w)),
      dispatch(tickCmd),
      dispatch(requeueCmd(d)),
    }
  }

  // Reclaim after a stall, a late stale result, one effect retried while the other is done,
  // a crash spending the budget, dead letters, refused and accepted requeues.
  pure val leaseScenario = List(
    claimCmd(1, 1),                  // A holds delivery 1 at generation 1
    claimCmd(2, 2),                  // B holds delivery 2 at generation 1
    completeCmd(2, 2, 1, "retry"),   // effect 2 fails; due again at tick 1
    claimCmd(1, 2),                  // not yet due
    tickCmd,
    tickCmd,                         // A's lease on delivery 1 expired
    claimCmd(2, 1),                  // B reclaims: generation 2
    completeCmd(1, 1, 1, "applied"), // A comes back: stale, writes nothing
    completeCmd(2, 1, 2, "applied"), // B's effect and outcome commit together
    claimCmd(1, 2),                  // only the failed effect runs again
    completeCmd(1, 2, 2, "retry"),   // backoff 2 ticks
    tickCmd,
    tickCmd,
    claimCmd(2, 2),                  // third attempt
    crashCmd(2),                     // B dies holding it
    tickCmd,
    tickCmd,
    claimCmd(1, 2),                  // the budget is spent: dead letter at claim
    requeueCmd(1),                   // a finished effect is never requeued
    requeueCmd(2),
    claimCmd(1, 2),
    completeCmd(1, 2, 4, "fail"),    // permanent failure: dead letter
    requeueCmd(2),
    claimCmd(2, 2),
    completeCmd(2, 2, 5, "applied"),
    claimCmd(1, 1),                  // terminal: not eligible
  )
  action scenarioStep = all { cursor < leaseScenario.length(), dispatch(leaseScenario.nth(cursor)) }

  // Negative control: completion without the generation fence, on the shortest trace.
  pure val staleReturn = List(claimCmd(1, 1), tickCmd, tickCmd, claimCmd(2, 1),
    completeCmd(2, 1, 2, "applied"), completeCmd(1, 1, 1, "applied"))
  action mutatedUnfencedStep = all {
    cursor < staleReturn.length(), dispatchWith(staleReturn.nth(cursor), false),
  }

  val effectAtMostOnce = deliveries.forall(d => effects.get(d) <= 1)
  val effectIffSucceeded = deliveries.forall(d =>
    (store.get(d).state == "succeeded") == (effects.get(d) == 1))
  val attemptsBounded = deliveries.forall(d => store.get(d).attempts <= maxAttempts)
  val leaseShape = deliveries.forall(d => store.get(d).state != "leased" or
    (store.get(d).generation >= 1 and store.get(d).attempts >= 1))
  val holdsNeverAhead = holds.forall(h => h.generation >= 1 and h.generation <= store.get(h.delivery).generation)
  val fenceMonotone = previous.keys().forall(d => store.get(d).generation >= previous.get(d).generation)
  val successIsFinal = previous.keys().forall(d =>
    previous.get(d).state != "succeeded" or store.get(d) == previous.get(d))
  val staleWritesNothing = outcome != "stale" or (store == previous and effects == previousEffects)
  // Only the named delivery changes; one consumer's outcome never stands for another's.
  val scoped = previous.keys().forall(d =>
    store.get(d) == previous.get(d) or (d == command.delivery and Set("claim", "complete", "requeue").contains(command.op)))
  val finiteDomain = now <= maxNow and deliveries.forall(d => store.get(d).generation <= 60)
  val outboxSafe = effectAtMostOnce and effectIffSucceeded and attemptsBounded and leaseShape and
    holdsNeverAhead and fenceMonotone and successIsFinal and staleWritesNothing and scoped and
    finiteDomain
}
