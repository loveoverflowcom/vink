// PRODUCT-DECISIONS-20261110, M5 — TARGET SPECIFICATION. Not implemented: GET /api/v1/works
// orders by publication time only; no ranking, recommendation or top-N exists and endorsement
// counts are not used for ordering. Nothing here is production conformance; see README.md.
//
// Two separate public lists over the same eligible set (USER-12, USER-13):
//   "Đề xuất"  — valid "Đánh giá cao" votes combined with recency (formula `recommend.v1`,
//                CHOICE-02 A, DESIGN_PROPOSED: not a product-owner formula and not tuned);
//   "Top 100"  — votes only, recency only as a tie-breaker, at most `cap` posts, paged from a
//                finite candidate snapshot (CHOICE-03 A, DESIGN_PROPOSED).
// ASSUMPTIONS: one serialized commit per action; `vote`, `eligibility` and `edit` are shells for
// outcomes of LVE and publication.qnt; `votes` is already the count of valid votes on the
// current public revision; `cap` = 3 stands for 100, `pageSize` = 2. Time is an explicit input.
module discovery {
  type Post = { eligible: bool, votes: int, publishedAt: int, tagged: bool }
  type Command = { op: str, post: int, value: int, tagOnly: bool }
  pure val posts = 1.to(4)
  pure val cap = 3
  pure val pageSize = 2
  pure val maxVotes = 3
  pure val maxNow = 8
  // recommend.v1: every doubling of (votes + 1) is worth `halfLife` ticks of freshness.
  pure val halfLife = 2
  pure def ilog2(n: int): int = if (n >= 4) 2 else if (n >= 2) 1 else 0
  pure def score(votes: int, publishedAt: int, now: int): int =
    halfLife * ilog2(votes + 1) - (now - publishedAt)
  pure def cmd(op: str, post: int, value: int, tagOnly: bool): Command =
    { op: op, post: post, value: value, tagOnly: tagOnly }
  pure def vote(post: int, delta: int): Command = cmd("vote", post, delta, false)
  pure def eligibility(post: int, on: int): Command = cmd("eligibility", post, on, false)
  pure def edit(post: int): Command = cmd("edit", post, 0, false)
  pure def open(tagOnly: bool): Command = cmd("open", 0, 0, tagOnly)
  pure val nextPage = cmd("next", 0, 0, false)
  pure val expire = cmd("expire", 0, 0, false)
  pure val tick = cmd("tick", 0, 0, false)

  var store: int -> Post
  var now: int
  var recommended: List[int]   // the "Đề xuất" order of this moment
  var snapshot: List[int]      // the Top candidates captured when the reader opened the list
  var snapshotFilter: bool     // the filter that snapshot was taken under
  var snapshotLive: bool       // false once the cursor expired
  var offset: int
  var delivered: Set[int]      // ghost: everything served from this snapshot so far
  var served: List[int]        // the page returned by the last open/next
  var command: Command
  var outcome: str
  var previousStore: int -> Post
  var previousDelivered: Set[int]
  var cursor: int

  // Total orders: the key, then newer first, then the larger id. No author, length or AI score.
  pure def newerFirst(st: int -> Post, a: int, b: int): bool =
    st.get(a).publishedAt > st.get(b).publishedAt or
    (st.get(a).publishedAt == st.get(b).publishedAt and a > b)
  pure def topBefore(st: int -> Post, a: int, b: int): bool =
    st.get(a).votes > st.get(b).votes or
    (st.get(a).votes == st.get(b).votes and newerFirst(st, a, b))
  pure def hotBefore(st: int -> Post, at: int, a: int, b: int): bool = {
    val sa = score(st.get(a).votes, st.get(a).publishedAt, at)
    val sb = score(st.get(b).votes, st.get(b).publishedAt, at)
    sa > sb or (sa == sb and newerFirst(st, a, b))
  }
  pure def ordered(st: int -> Post, at: int, s: Set[int], top: bool): List[int] =
    range(0, s.size()).foldl(List(), (acc, i) => acc.concat(
      s.filter(p => s.filter(q =>
        if (top) topBefore(st, q, p) else hotBefore(st, at, q, p)).size() == i)
        .fold(List(), (l, p) => l.append(p))))
  pure def matches(st: int -> Post, p: int, tagOnly: bool): bool =
    st.get(p).eligible and (not(tagOnly) or st.get(p).tagged)
  pure def firstN(l: List[int], n: int): List[int] = l.slice(0, if (l.length() < n) l.length() else n)
  pure def feed(st: int -> Post, at: int): List[int] =
    ordered(st, at, posts.filter(p => st.get(p).eligible), false)

  pure val initial: int -> Post = Map(
    1 -> { eligible: true, votes: 0, publishedAt: 0, tagged: true },
    2 -> { eligible: true, votes: 0, publishedAt: 1, tagged: false },
    3 -> { eligible: true, votes: 0, publishedAt: 2, tagged: true },
    4 -> { eligible: true, votes: 0, publishedAt: 2, tagged: true })

  action init = all {
    store' = initial, now' = 4, recommended' = feed(initial, 4), snapshot' = List(),
    snapshotFilter' = false, snapshotLive' = false, offset' = 0, delivered' = Set(),
    served' = List(), command' = cmd("init", 0, 0, false), outcome' = "init",
    previousStore' = initial, previousDelivered' = Set(), cursor' = 0,
  }

  action record(c: Command, result: str, st: int -> Post, at: int): bool = all {
    store' = st, now' = at, recommended' = feed(st, at), command' = c, outcome' = result,
    previousStore' = store, previousDelivered' = delivered,
    cursor' = if (cursor < 60) cursor + 1 else cursor,
  }
  action keepSnapshot = all {
    snapshot' = snapshot, snapshotFilter' = snapshotFilter, snapshotLive' = snapshotLive,
    offset' = offset, delivered' = delivered, served' = List(),
  }

  action changed(c: Command): bool = {
    val p = store.get(c.post)
    val votes = p.votes + c.value
    val updated =
      if (c.op == "vote") { if (votes < 0 or votes > maxVotes) p else p.with("votes", votes) }
      else if (c.op == "eligibility") p.with("eligible", c.value == 1)
      // A new public revision: the count starts again (LVE-FR-04), the first-publication time
      // does not move, so an edit can never lift a post.
      else p.with("votes", 0)
    val result = if (c.op == "edit") "edited" else if (updated == p) "unchanged"
      else if (c.op == "vote") "votes_changed" else "eligibility_changed"
    all { record(c, result, store.set(c.post, updated), now), keepSnapshot }
  }

  // Opening Top: filter and eligibility first, then the order, then the cap. `filterFirst` is a
  // mutation switch: false takes the global top `cap` and filters afterwards.
  action opened(c: Command, filterFirst: bool): bool = {
    val wanted = posts.filter(p => matches(store, p, c.tagOnly))
    val list = if (filterFirst) firstN(ordered(store, now, wanted, true), cap)
      else firstN(ordered(store, now, posts.filter(p => store.get(p).eligible), true), cap)
        .select(p => matches(store, p, c.tagOnly))
    val page = firstN(list, pageSize)
    all {
      snapshot' = list, snapshotFilter' = c.tagOnly, snapshotLive' = true, offset' = pageSize,
      delivered' = posts.filter(p => page.select(x => x == p).length() > 0), served' = page,
      record(c, "opened", store, now),
    }
  }

  // The next page comes from the snapshot positions, and every item is decided again against
  // the state of this request. `recheck` is a mutation switch.
  action paged(c: Command, recheck: bool): bool = {
    val window = snapshot.slice(if (offset < snapshot.length()) offset else snapshot.length(),
      if (offset + pageSize < snapshot.length()) offset + pageSize else snapshot.length())
    val page = if (recheck) window.select(p => matches(store, p, snapshotFilter)) else window
    if (not(snapshotLive)) all { record(c, "refresh_required", store, now), keepSnapshot }
    else if (offset >= snapshot.length()) all { record(c, "end", store, now), keepSnapshot }
    else all {
      snapshot' = snapshot, snapshotFilter' = snapshotFilter, snapshotLive' = true,
      offset' = offset + pageSize,
      delivered' = delivered.union(posts.filter(p => page.select(x => x == p).length() > 0)),
      served' = page, record(c, "page", store, now),
    }
  }

  action expired(c: Command): bool = all {
    snapshot' = List(), snapshotFilter' = false, snapshotLive' = false, offset' = 0,
    delivered' = Set(), served' = List(), record(c, "expired", store, now),
  }
  action ticked(c: Command): bool = all {
    now < maxNow, record(c, "ticked", store, now + 1), keepSnapshot,
  }

  action dispatchWith(c: Command, filterFirst: bool, recheck: bool): bool =
    if (c.op == "open") opened(c, filterFirst)
    else if (c.op == "next") paged(c, recheck)
    else if (c.op == "expire") expired(c)
    else if (c.op == "tick") ticked(c)
    else changed(c)
  action dispatch(c: Command): bool = dispatchWith(c, true, true)

  action step = {
    nondet p = posts.oneOf()
    nondet d = Set(1, -1).oneOf()
    nondet on = Set(0, 1).oneOf()
    nondet f = Set(true, false).oneOf()
    any {
      dispatch(vote(p, d)), dispatch(eligibility(p, on)), dispatch(edit(p)), dispatch(open(f)),
      dispatch(nextPage), dispatch(expire), dispatch(tick),
    }
  }

  // An old post with votes against fresh posts without, the two lists disagreeing, a filter
  // applied before the cap, revocation and vote changes between pages, an unlike, an edit, an
  // expired cursor.
  pure val rankingScenario = List(
    nextPage,           // refresh_required: no snapshot yet
    vote(3, -1),        // unchanged: a count never goes below zero
    vote(1, 1),         // Top now starts with post 1; Đề xuất still starts with the fresh post 4
    open(false),        // snapshot [1, 4, 3] while Đề xuất reads [4, 3, 1, 2]
    vote(1, 1),
    vote(1, 1),         // three votes lift the oldest post to the head of Đề xuất as well
    vote(2, 1),
    open(true),         // filter first: {1, 3, 4} -> snapshot [1, 4, 3], page [1, 4]
    eligibility(3, 0),  // post 3 stops being readable between two pages
    nextPage,           // page []: post 3 is dropped, nothing else is pulled in
    nextPage,           // end
    open(false),        // snapshot [1, 2, 4], page [1, 2]
    vote(4, 1), vote(4, 1), vote(4, 1), // post 4 now ties post 1 and would rank first
    nextPage,           // page [4]: no repeat and no gap inside the snapshot
    vote(1, -1),        // an unlike
    edit(4),            // a new revision: the count starts again, the time does not move
    tick,
    eligibility(3, 1),
    expire,
    nextPage,           // refresh_required
    open(false),        // a fresh snapshot sees the new order
    nextPage,
    nextPage,           // end
  )
  action rankingStep = all { cursor < rankingScenario.length(), dispatch(rankingScenario.nth(cursor)) }

  // Negative control 1: permission checked only when the snapshot was taken.
  pure val staleAccess = List(open(false), eligibility(2, 0), nextPage)
  action mutatedStaleAccessStep = all {
    cursor < staleAccess.length(), dispatchWith(staleAccess.nth(cursor), true, false),
  }
  // Negative control 2: the global top `cap` filtered afterwards loses an eligible post.
  pure val filterLast = List(vote(2, 1), open(true))
  action mutatedFilterLastStep = all {
    cursor < filterLast.length(), dispatchWith(filterLast.nth(cursor), false, true),
  }

  def has(l: List[int], p: int): bool = l.select(x => x == p).length() > 0
  def fits(p: int): bool = store.get(p).eligible and (not(snapshotFilter) or store.get(p).tagged)

  // Revocation beats the snapshot: every served item is readable under this request's state.
  val servedIsReadableNow = posts.forall(p => not(has(served, p)) or fits(p))
  val servedComesFromSnapshot = posts.forall(p => not(has(served, p)) or has(snapshot, p))
  val noRepeatAcrossPages = command.op != "next" or
    posts.forall(p => not(has(served, p)) or not(previousDelivered.contains(p)))
  val snapshotCapped = snapshot.length() <= cap and
    posts.forall(p => snapshot.select(x => x == p).length() <= 1)
  // When opened, the snapshot is exactly the head of the filtered eligible set in Top order.
  val snapshotIsTopOfFilteredSet = outcome != "opened" or (
    posts.forall(p => has(snapshot, p) implies fits(p)) and
    posts.forall(p => (fits(p) and not(has(snapshot, p))) implies
      (snapshot.length() == cap and posts.forall(q => has(snapshot, q) implies topBefore(store, q, p)))) and
    snapshot.indices().forall(i => snapshot.indices().forall(j =>
      i < j implies topBefore(store, snapshot.nth(i), snapshot.nth(j)))))
  // Top never puts fewer votes above more votes, however new the post is.
  val topIgnoresRecency = outcome != "opened" or snapshot.indices().forall(i =>
    snapshot.indices().forall(j =>
      i < j implies store.get(snapshot.nth(i)).votes >= store.get(snapshot.nth(j)).votes))
  // Đề xuất lists every eligible post once and nothing else, by score then the tie-breaker.
  val recommendedIsEligibleOnce =
    posts.forall(p => recommended.select(x => x == p).length() == (if (store.get(p).eligible) 1 else 0))
  val recommendedIsOrdered = recommended.indices().forall(i => recommended.indices().forall(j =>
    i < j implies hotBefore(store, now, recommended.nth(i), recommended.nth(j))))
  // The laws of recommend.v1 over the whole small domain.
  val moreVotesNeverLower = 0.to(maxVotes - 1).forall(v => 0.to(3).forall(t => 4.to(maxNow).forall(n =>
    score(v, t, n) <= score(v + 1, t, n))))
  val newerNeverLower = 0.to(maxVotes).forall(v => 0.to(2).forall(t => 4.to(maxNow).forall(n =>
    score(v, t, n) <= score(v, t + 1, n))))
  // The clock moves every score by the same amount, so the order does not depend on it.
  val orderIgnoresClock = 0.to(maxVotes).forall(v => 0.to(3).forall(t => 4.to(maxNow - 1).forall(n =>
    score(v, t, n) - score(v, t, n + 1) == 1)))
  // An edit never raises a post: first-publication time fixed, count not increased.
  val editNeverBumps = command.op != "edit" or (
    store.get(command.post).publishedAt == previousStore.get(command.post).publishedAt and
    store.get(command.post).votes <= previousStore.get(command.post).votes)
  val scopedWrites = posts.forall(p => store.get(p) == previousStore.get(p) or
    (p == command.post and Set("vote", "eligibility", "edit").contains(command.op)))
  val finiteDomain = now <= maxNow and posts.forall(p => store.get(p).votes >= 0 and store.get(p).votes <= maxVotes)
  val discoverySafe = servedIsReadableNow and servedComesFromSnapshot and noRepeatAcrossPages and
    snapshotCapped and snapshotIsTopOfFilteredSet and topIgnoresRecency and
    recommendedIsEligibleOnce and recommendedIsOrdered and moreVotesNeverLower and newerNeverLower and
    orderIgnoresClock and editNeverBumps and scopedWrites and finiteDomain
}
