// PRODUCT-DECISIONS-20261110, M2 — TARGET SPECIFICATION. Not implemented: no collection concept
// exists in services/vink-app or the Web app. Nothing here is production conformance.
//
// Personal collections (USER-09): a folder of the author's own short posts, each collection
// "only me" or "public". What a viewer sees is the AND of three independent facts:
//   membership(post, collection) AND can_read_collection(viewer, collection)
//                                AND can_read_post(viewer, post)
// ASSUMPTIONS: one serialized commit per action; `post` is a shell for the outcome of
// publication.qnt (audience, moderation, deletion); a post may sit in several collections and
// each collection keeps the author's order (both DESIGN_PROPOSED, not product decisions).
// Principal 1 owns both collections and posts 1 and 2; principal 2 owns post 3.
module collections {
  type Collection = { live: bool, audience: str, items: List[int] }
  type Command = { op: str, actor: int, post: int, collection: int, value: str }
  pure val owner = 1
  pure val viewers = 0.to(2)      // 0 guest, 1 the owner, 2 another signed-in person
  pure val posts = 1.to(3)
  pure val shelves = 1.to(2)
  pure def authorOf(p: int): int = if (p == 3) 2 else 1
  pure val pairsVC = tuples(viewers, shelves)
  pure val pairsVP = tuples(viewers, posts)
  pure val none = { live: false, audience: "private", items: List() }
  pure def cmd(op: str, actor: int, post: int, collection: int, value: str): Command =
    { op: op, actor: actor, post: post, collection: collection, value: value }
  pure def create(actor: int, c: int): Command = cmd("create", actor, 0, c, "none")
  pure def audience(actor: int, c: int, value: str): Command = cmd("set_audience", actor, 0, c, value)
  pure def add(actor: int, p: int, c: int): Command = cmd("add", actor, p, c, "none")
  pure def remove(actor: int, p: int, c: int): Command = cmd("remove", actor, p, c, "none")
  pure def drop(actor: int, c: int): Command = cmd("delete_collection", actor, 0, c, "none")
  pure def post(p: int, value: str): Command = cmd("post", authorOf(p), p, 0, value)
  // read one item after `post` (0 = from the start) as `actor`
  pure def read(actor: int, c: int, after: int): Command = cmd("read", actor, after, c, "none")

  var postState: int -> str                 // "private" | "pending" | "public" | "deleted"
  var shelf: int -> Collection
  var views: (int, int) -> List[int]        // (viewer, collection) -> items shown, in order
  var counts: (int, int) -> int             // (viewer, collection) -> the count shown
  var badges: (int, int) -> Set[int]        // (viewer, post) -> collections named on the post page
  var links: (int, int) -> Set[int]         // (viewer, post) -> posts linked from the post page
  var served: List[int]                     // the page returned by the last `read`
  var command: Command
  var outcome: str
  var previousPosts: int -> str
  var previousShelf: int -> Collection
  var cursor: int

  // The three facts, each from its own owner.
  pure def member(sh: int -> Collection, p: int, c: int): bool =
    sh.get(c).items.select(x => x == p).length() > 0
  pure def canReadCollection(sh: int -> Collection, v: int, c: int): bool =
    sh.get(c).live and (v == owner or sh.get(c).audience == "public")
  pure def canReadPost(ps: int -> str, v: int, p: int): bool =
    ps.get(p) != "deleted" and (v == authorOf(p) or ps.get(p) == "public")
  // `conjunction` is the mutation switch: false joins the two permissions with OR.
  pure def appears(ps: int -> str, sh: int -> Collection, v: int, p: int, c: int, conjunction: bool): bool =
    member(sh, p, c) and
    (if (conjunction) canReadCollection(sh, v, c) and canReadPost(ps, v, p)
     else canReadCollection(sh, v, c) or canReadPost(ps, v, p))
  pure def shown(ps: int -> str, sh: int -> Collection, v: int, c: int, conjunction: bool): List[int] =
    sh.get(c).items.select(p => appears(ps, sh, v, p, c, conjunction))
  pure def named(ps: int -> str, sh: int -> Collection, v: int, p: int, conjunction: bool): Set[int] =
    shelves.filter(c => appears(ps, sh, v, p, c, conjunction))
  pure def linked(ps: int -> str, sh: int -> Collection, v: int, p: int, conjunction: bool): Set[int] =
    posts.filter(q => q != p and
      named(ps, sh, v, p, conjunction).exists(c => appears(ps, sh, v, q, c, conjunction)))

  action init = all {
    postState' = posts.mapBy(_ => "private"), shelf' = shelves.mapBy(_ => none),
    views' = pairsVC.mapBy(_ => List()), counts' = pairsVC.mapBy(_ => 0),
    badges' = pairsVP.mapBy(_ => Set()), links' = pairsVP.mapBy(_ => Set()), served' = List(),
    command' = cmd("init", 0, 0, 0, "none"), outcome' = "init",
    previousPosts' = posts.mapBy(_ => "private"), previousShelf' = shelves.mapBy(_ => none),
    cursor' = 0,
  }

  action commit(c: Command, result: str, ps: int -> str, sh: int -> Collection, page: List[int],
      conjunction: bool): bool = all {
    postState' = ps, shelf' = sh,
    views' = pairsVC.mapBy(k => shown(ps, sh, k._1, k._2, conjunction)),
    counts' = pairsVC.mapBy(k => shown(ps, sh, k._1, k._2, conjunction).length()),
    badges' = pairsVP.mapBy(k => named(ps, sh, k._1, k._2, conjunction)),
    links' = pairsVP.mapBy(k => linked(ps, sh, k._1, k._2, conjunction)),
    served' = page, command' = c, outcome' = result,
    previousPosts' = postState, previousShelf' = shelf,
    cursor' = if (cursor < 60) cursor + 1 else cursor,
  }

  action keep(c: Command, conjunction: bool, result: str): bool =
    commit(c, result, postState, shelf, List(), conjunction)

  action dispatchWith(c: Command, conjunction: bool): bool = {
    val target = if (shelves.contains(c.collection)) shelf.get(c.collection) else none
    // Another person cannot tell a private or missing collection from one they may not change.
    val mine = c.actor == owner and target.live
    if (c.op == "post")
      if (postState.get(c.post) == "deleted") keep(c, conjunction, "post_unavailable")
      else commit(c, "post_changed", postState.set(c.post, c.value), shelf, List(), conjunction)
    else if (c.op == "create")
      if (c.actor != owner) keep(c, conjunction, "not_found")
      else if (target.live) keep(c, conjunction, "already_exists")
      // A new collection starts "only me".
      else commit(c, "created", postState,
        shelf.set(c.collection, { live: true, audience: "private", items: List() }), List(), conjunction)
    else if (c.op == "set_audience")
      if (not(mine)) keep(c, conjunction, "not_found")
      else commit(c, if (target.audience == c.value) "unchanged" else "audience_changed", postState,
        shelf.set(c.collection, target.with("audience", c.value)), List(), conjunction)
    else if (c.op == "add")
      if (not(mine)) keep(c, conjunction, "not_found")
      else if (authorOf(c.post) != c.actor) keep(c, conjunction, "not_own_post")
      else if (postState.get(c.post) == "deleted") keep(c, conjunction, "post_unavailable")
      else if (member(shelf, c.post, c.collection)) keep(c, conjunction, "already_member")
      else commit(c, "added", postState,
        shelf.set(c.collection, target.with("items", target.items.append(c.post))), List(), conjunction)
    else if (c.op == "remove")
      if (not(mine)) keep(c, conjunction, "not_found")
      else if (not(member(shelf, c.post, c.collection))) keep(c, conjunction, "not_member")
      else commit(c, "removed", postState,
        shelf.set(c.collection, target.with("items", target.items.select(x => x != c.post))), List(), conjunction)
    else if (c.op == "delete_collection")
      if (not(mine)) keep(c, conjunction, "not_found")
      else commit(c, "collection_deleted", postState, shelf.set(c.collection, none), List(), conjunction)
    // A read re-decides every item against the state of this request; the cursor is only a
    // position (the last post seen), never a stored list of what was visible earlier.
    else {
      val visible = shown(postState, shelf, c.actor, c.collection, conjunction)
      val order = target.items
      val position = order.indices().filter(i => order.nth(i) == c.post)
      val rest = if (c.post == 0) visible
        else visible.select(p => order.indices().exists(i =>
          order.nth(i) == p and position.forall(at => i > at)))
      if (not(canReadCollection(shelf, c.actor, c.collection))) keep(c, conjunction, "not_found")
      else if (c.post != 0 and position.size() == 0) keep(c, conjunction, "cursor_invalid")
      else if (rest.length() == 0) keep(c, conjunction, "end")
      else commit(c, "page", postState, shelf, List(rest.head()), conjunction)
    }
  }
  action dispatch(c: Command): bool = dispatchWith(c, true)

  action step = {
    nondet actor = Set(1, 2).oneOf()
    nondet viewer = viewers.oneOf()
    nondet p = posts.oneOf()
    nondet after = 0.to(3).oneOf()
    nondet c = shelves.oneOf()
    nondet a = Set("private", "public").oneOf()
    nondet s = Set("private", "pending", "public", "deleted").oneOf()
    any {
      dispatch(create(actor, c)), dispatch(audience(actor, c, a)), dispatch(add(actor, p, c)),
      dispatch(remove(actor, p, c)), dispatch(drop(actor, c)), dispatch(post(p, s)),
      dispatch(read(viewer, c, after)),
    }
  }

  // The four rows of the truth table, navigation, paging while permissions change, and writes
  // by someone else.
  pure val truthScenario = List(
    create(2, 1),                 // not_found: not their namespace
    create(1, 1),                 // created, "only me"
    create(1, 1),                 // already_exists
    add(1, 1, 1), add(1, 2, 1),   // collection 1 = [1, 2], both posts private
    add(1, 1, 1),                 // already_member
    add(1, 3, 1),                 // not_own_post
    add(2, 3, 1),                 // not_found: another person's collection
    read(0, 1, 0),                // not_found: private collection + private posts
    post(1, "public"),            // private collection + public post: membership stays hidden
    read(0, 1, 0),                // not_found
    audience(1, 1, "public"),     // public collection: post 1 shows, private post 2 does not
    read(0, 1, 0),                // page [1]
    read(0, 1, 1),                // end: post 2 is private
    read(1, 1, 1),                // page [2]: the owner reads their own private post
    post(2, "public"),            // both readable: each links to the other
    read(2, 1, 1),                // page [2]
    post(2, "pending"),           // waiting for moderation is not readable by others
    read(2, 1, 1),                // end: the cursor is a position, permissions are current
    post(2, "public"),
    create(1, 2), add(1, 2, 2),   // post 2 also sits in private collection 2
    audience(1, 1, "private"),    // closing the collection changes no post
    read(0, 1, 0),                // not_found; posts 1 and 2 are still public on their own
    audience(1, 1, "public"),
    audience(1, 1, "public"),     // unchanged
    remove(1, 1, 1),              // removed: the post itself is untouched
    read(0, 1, 1),                // cursor_invalid: the cursor post left the collection
    remove(1, 1, 1),              // not_member
    post(2, "deleted"),           // a deleted post leaves every list and link, for the owner too
    read(1, 1, 0),                // end
    post(2, "public"),            // post_unavailable: deletion is terminal
    add(1, 2, 1),                 // post_unavailable: a deleted post is not added anywhere
    drop(1, 1),                   // collection_deleted: no post is deleted with it
    drop(2, 2),                   // not_found
    add(1, 1, 1),                 // not_found: the collection no longer exists
  )
  action truthStep = all { cursor < truthScenario.length(), dispatch(truthScenario.nth(cursor)) }

  // Negative control: OR instead of AND shows a public post inside a private collection, which
  // reveals the collection and its membership to a guest.
  pure val orLeak = List(create(1, 1), add(1, 1, 1), post(1, "public"))
  action mutatedOrStep = all { cursor < orLeak.length(), dispatchWith(orLeak.nth(cursor), false) }

  // Ground truth, written from the stored facts and not from the projection functions.
  def readable(v: int, p: int): bool =
    postState.get(p) == "public" or (v == authorOf(p) and postState.get(p) != "deleted")
  def openTo(v: int, c: int): bool =
    shelf.get(c).live and (v == owner or shelf.get(c).audience == "public")
  def inShelf(p: int, c: int): bool = shelf.get(c).items.select(x => x == p).length() > 0
  def listOf(v: int, c: int): Set[int] = posts.filter(p => views.get((v, c)).select(x => x == p).length() > 0)

  // A collection the viewer may not read shows nothing: no item, no count, no name on a post.
  val closedCollectionLeaksNothing = pairsVC.forall(k => openTo(k._1, k._2) or
    (views.get(k) == List() and counts.get(k) == 0 and
      posts.forall(p => not(badges.get((k._1, p)).contains(k._2)))))
  // Every listed item is a member the viewer may read on its own.
  val listedItemsAreReadableMembers = pairsVC.forall(k =>
    listOf(k._1, k._2).forall(p => inShelf(p, k._2) and readable(k._1, p)))
  // And nothing readable is withheld: the list is exactly the AND, in the author's order.
  val listIsExactlyTheConjunction = pairsVC.forall(k =>
    views.get(k) == (if (openTo(k._1, k._2)) shelf.get(k._2).items.select(p => readable(k._1, p)) else List()))
  val countsMatchTheList = pairsVC.forall(k => counts.get(k) == views.get(k).length())
  val badgesNeedAllThree = pairsVP.forall(k => badges.get(k).forall(c =>
    inShelf(k._2, c) and openTo(k._1, c) and readable(k._1, k._2)))
  // Related/next/previous links only lead to posts the viewer may read, through a collection
  // the viewer may read.
  val linksAreReadable = pairsVP.forall(k => links.get(k).forall(q => q != k._2 and readable(k._1, q) and
    shelves.exists(c => badges.get(k).contains(c) and inShelf(q, c))))
  // A page is decided against the state of this request.
  val servedIsReadableNow = served.select(p =>
    not(inShelf(p, command.collection) and openTo(command.actor, command.collection) and
      readable(command.actor, p))).length() == 0
  // Collections never change a post; posts never change membership; only the owner writes.
  val collectionChangesKeepPosts = command.op == "post" or postState == previousPosts
  val postChangesKeepMembership = command.op != "post" or shelf == previousShelf
  val onlyOwnerWrites = shelf == previousShelf or command.actor == owner
  val ownPostsOnly = shelves.forall(c => posts.forall(p => not(inShelf(p, c)) or authorOf(p) == owner))
  val noDuplicateMembership = shelves.forall(c => posts.forall(p =>
    shelf.get(c).items.select(x => x == p).length() <= 1))
  val collectionsSafe = closedCollectionLeaksNothing and listedItemsAreReadableMembers and
    listIsExactlyTheConjunction and countsMatchTheList and badgesNeedAllThree and linksAreReadable and
    servedIsReadableNow and collectionChangesKeepPosts and postChangesKeepMembership and
    onlyOwnerWrites and ownPostsOnly and noDuplicateMembership
}
