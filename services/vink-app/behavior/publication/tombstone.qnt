// PRODUCT-DECISIONS-20261110 + ADMIN-MODERATION-20261111, M3 — canonical access model with
// pure-core replay (#157 and the admin-moderation core). No runtime claim; see README.md.
//
// What other people can read of one work and its comments, by the reason the work stopped being
// public (PUB-04/05/07, COM-11):
//   A  the author deleted a work that was public or hidden   -> a tombstone page keeps its comments
//   B  the author made a public work private                 -> a hidden page keeps its comments
//   C  an admin removed the work                             -> nothing
//   D  the work was never public (or deleted while never public) -> nothing
// On the tombstone and the hidden page the thread is readable, submitted comments are still
// reviewed (from their own snapshot) and writers may remove their own; nothing new is written.
// After republication the work's author is notified only of events of the new public period.
// ASSUMPTIONS: one serialized commit per action; who decides publication, removal and comment
// review is publication.qnt / governance.qnt, here they are shell commands. Viewers are other
// people (0 = guest, 2 and 3 = signed-in writers); the author reads through the owner path.
module tombstone {
  type Comment = { writer: int, review: str, removed: bool }
  type Command = { op: str, actor: int, id: int, value: str }
  // What one viewer gets: the page kind for the old URL, whether the work is listed anywhere
  // (Khám phá, search, hashtag, Đề xuất, Top 100, related), whether a system-made preview of the
  // body (snippet, quote, share card, notification text) is served, and the readable comments.
  type View = { page: str, listed: bool, snippet: bool, comments: Set[int] }
  pure val viewers = Set(0, 2, 3)
  pure val ids = 1.to(4)
  pure def cmd(op: str, actor: int, id: int, value: str): Command =
    { op: op, actor: actor, id: id, value: value }
  pure val publish = cmd("publish", 1, 0, "none")
  pure val makePrivate = cmd("make_private", 1, 0, "none")
  pure val authorDelete = cmd("author_delete", 1, 0, "none")
  pure val moderationRemove = cmd("moderation_remove", 9, 0, "none")
  pure def submit(actor: int, id: int): Command = cmd("submit", actor, id, "none")
  pure def review(id: int, value: str): Command = cmd("review", 9, id, value)
  pure def removeOwn(actor: int, id: int): Command = cmd("remove_own", actor, id, "none")

  // "unpublished" | "public" | "hidden" | "author_deleted" | "deleted_hidden" | "moderation_removed"
  var status: str
  var everPublic: bool
  var comments: int -> Comment
  var views: int -> View
  // COM-11: comments whose approval notified the work's author.
  var notified: Set[int]
  var previousNotified: Set[int]
  var command: Command
  var outcome: str
  var previousStatus: str
  var previousComments: int -> Comment
  var cursor: int

  // The read rule. `pages` is the mutation switch: the statuses whose old URL keeps a page and
  // its comment thread, with the page kind. The lawful rule has exactly these two.
  pure val lawfulPages = Map("author_deleted" -> "tombstone", "hidden" -> "hidden")
  pure def project(st: str, cs: int -> Comment, pages: str -> str): int -> View =
    viewers.mapBy(v => {
      val thread = st == "public" or pages.keys().contains(st)
      {
        page: if (st == "public") "body" else if (pages.keys().contains(st)) pages.get(st) else "not_found",
        listed: st == "public",
        snippet: st == "public",
        comments: if (thread)
          cs.keys().filter(id => not(cs.get(id).removed) and
            (cs.get(id).review == "approved" or cs.get(id).writer == v))
          else Set(),
      }
    })

  action init = all {
    status' = "unpublished", everPublic' = false, comments' = Map(),
    views' = project("unpublished", Map(), lawfulPages), notified' = Set(), previousNotified' = Set(),
    command' = cmd("init", 0, 0, "none"), outcome' = "init", previousStatus' = "unpublished",
    previousComments' = Map(), cursor' = 0,
  }

  action commitWith(c: Command, result: str, st: str, cs: int -> Comment, pages: str -> str,
      told: Set[int]): bool = all {
    status' = st, everPublic' = (everPublic or st == "public"), comments' = cs,
    views' = project(st, cs, pages), notified' = told, previousNotified' = notified,
    command' = c, outcome' = result, previousStatus' = status, previousComments' = comments,
    cursor' = if (cursor < 60) cursor + 1 else cursor,
  }
  action commit(c: Command, result: str, st: str, cs: int -> Comment, pages: str -> str): bool =
    commitWith(c, result, st, cs, pages, notified)

  pure val terminal = Set("author_deleted", "deleted_hidden", "moderation_removed")
  pure val readableThread = Set("public", "hidden", "author_deleted")

  // `backfill` is the notification mutation switch: republishing tells the author about every
  // approved comment it has not told them about, including those approved while hidden.
  action dispatchWith(c: Command, pages: str -> str, backfill: bool): bool = {
    val refuse = if (terminal.contains(status)) "work_unavailable" else "unchanged"
    val approvedNow = comments.keys().filter(id => comments.get(id).review == "approved")
    if (c.op == "publish")
      if (status == "unpublished" or status == "hidden")
        commitWith(c, "published", "public", comments, pages,
          if (backfill) notified.union(approvedNow) else notified)
      else commit(c, refuse, status, comments, pages)
    // Only a public work becomes the hidden page; a never-public work stays not found.
    else if (c.op == "make_private")
      if (status == "public") commit(c, "made_private", "hidden", comments, pages)
      else commit(c, refuse, status, comments, pages)
    // Deleting a public or hidden work leaves the tombstone; deleting a never-public one leaves
    // nothing, so deletion never widens what a work shows.
    else if (c.op == "author_delete")
      if (status == "public" or status == "hidden")
        commit(c, "deleted_with_tombstone", "author_deleted", comments, pages)
      else if (status == "unpublished") commit(c, "deleted", "deleted_hidden", comments, pages)
      else commit(c, refuse, status, comments, pages)
    else if (c.op == "moderation_remove")
      if (status == "public" or status == "hidden")
        commit(c, "removed_by_moderation", "moderation_removed", comments, pages)
      else commit(c, refuse, status, comments, pages)
    // New text needs a public work. The tombstone and the hidden page take no new comment.
    else if (c.op == "submit")
      if (comments.keys().contains(c.id)) commit(c, "duplicate", status, comments, pages)
      else if (status == "author_deleted") commit(c, "tombstone_read_only", status, comments, pages)
      else if (status == "hidden") commit(c, "hidden_read_only", status, comments, pages)
      else if (status != "public") commit(c, "work_unavailable", status, comments, pages)
      else commit(c, "created", status,
        comments.put(c.id, { writer: c.actor, review: "pending", removed: false }), pages)
    // A comment keeps its own review whatever happened to the work. Only an approval while the
    // work is public tells the work's author.
    else if (c.op == "review")
      if (not(comments.keys().contains(c.id)) or comments.get(c.id).removed)
        commit(c, "comment_unavailable", status, comments, pages)
      else {
        val after = comments.setBy(c.id, x => x.with("review", c.value))
        val approved = c.value == "approved"
        commitWith(c, "reviewed", status, after, pages,
          if (approved and status == "public") notified.union(Set(c.id)) else notified)
      }
    // A writer may still take their own words down where the thread is readable.
    else
      if (not(comments.keys().contains(c.id)) or comments.get(c.id).removed or
          comments.get(c.id).writer != c.actor or not(readableThread.contains(status)))
        commit(c, "comment_unavailable", status, comments, pages)
      else commit(c, "comment_removed", status,
        comments.setBy(c.id, x => x.with("removed", true)), pages)
  }
  action dispatch(c: Command): bool = dispatchWith(c, lawfulPages, false)

  action step = {
    nondet actor = Set(2, 3).oneOf()
    nondet id = ids.oneOf()
    nondet verdict = Set("approved", "rejected").oneOf()
    any {
      dispatch(publish), dispatch(makePrivate), dispatch(authorDelete), dispatch(moderationRemove),
      dispatch(submit(actor, id)), dispatch(review(id, verdict)), dispatch(removeOwn(actor, id)),
    }
  }

  // A: public, three comments (approved, rejected, pending), then the author deletes.
  pure val deletedScenario = List(
    submit(2, 1),       // work_unavailable: nothing is public yet
    publish,
    submit(2, 1), submit(3, 2), submit(2, 3),
    review(1, "approved"), review(2, "rejected"),
    authorDelete,       // deleted_with_tombstone: guests read comment 1 on the old URL
    submit(3, 4),       // tombstone_read_only
    review(3, "approved"), // a comment written while public may still be approved
    removeOwn(3, 1),    // comment_unavailable: not theirs
    removeOwn(2, 1),    // comment_removed
    publish,            // work_unavailable: deletion is terminal
    makePrivate,        // work_unavailable
  )
  action deletedStep = all { cursor < deletedScenario.length(), dispatch(deletedScenario.nth(cursor)) }

  // B then A: a public work made private shows the hidden page with its comments; a pending
  // comment is still reviewed there (no notification), its writer may remove their own; public
  // again restores the work; a new approval notifies; hidden then deleted keeps the comments.
  pure val hiddenScenario = List(
    publish, submit(2, 1), review(1, "approved"), submit(3, 2), submit(3, 3),
    makePrivate,        // made_private: the old URL is the hidden page, comment 1 for guests
    submit(2, 4),       // hidden_read_only
    review(2, "approved"), // reviewed: shown on the hidden page, the work's author not told
    removeOwn(3, 3),    // comment_removed: the writer removes their own on the hidden page
    makePrivate,        // unchanged
    publish,            // published: the body is back
    submit(2, 4), review(4, "approved"), // a new approval in the new public period notifies
    makePrivate,        // made_private
    authorDelete,       // deleted_with_tombstone: hidden then deleted keeps the comments
    submit(3, 4),       // duplicate (comment 4 exists)
  )
  action hiddenStep = all { cursor < hiddenScenario.length(), dispatch(hiddenScenario.nth(cursor)) }

  // D: never public, private and deleted: nothing, ever.
  pure val neverPublicScenario = List(
    submit(2, 1),       // work_unavailable
    makePrivate,        // unchanged
    authorDelete,       // deleted: no page for a work nobody could read
    submit(3, 2),       // work_unavailable
  )
  action neverPublicStep = all {
    cursor < neverPublicScenario.length(), dispatch(neverPublicScenario.nth(cursor)),
  }

  // C: admin removal is not author deletion, from public or from the hidden page.
  pure val removedScenario = List(
    publish, submit(2, 1), review(1, "approved"),
    makePrivate,        // made_private: the hidden page
    moderationRemove,   // removed_by_moderation: not_found, no comments
    authorDelete,       // work_unavailable: the author cannot turn a removal into a tombstone
    review(1, "rejected"), // moderation of the comment still works
  )
  action removedStep = all { cursor < removedScenario.length(), dispatch(removedScenario.nth(cursor)) }

  // Negative control 1: treating an admin removal like the hidden page lets an old link read the
  // comments of a removed work.
  pure val removedLeak = List(publish, submit(2, 1), review(1, "approved"), moderationRemove)
  action mutatedRemovedStep = all {
    cursor < removedLeak.length(),
    dispatchWith(removedLeak.nth(cursor),
      Map("author_deleted" -> "tombstone", "hidden" -> "hidden", "moderation_removed" -> "hidden"), false),
  }
  // Negative control 2: republishing notifies the author of an approval made while hidden.
  pure val hiddenNotice = List(publish, submit(2, 1), makePrivate, review(1, "approved"), publish)
  action mutatedNoticeStep = all {
    cursor < hiddenNotice.length(), dispatchWith(hiddenNotice.nth(cursor), lawfulPages, true),
  }

  pure val unavailable = Set("unpublished", "deleted_hidden", "moderation_removed")
  val approvedPresent = comments.keys().filter(id =>
    comments.get(id).review == "approved" and not(comments.get(id).removed))

  // The body, its previews and every listing exist only while the work is public.
  val bodyOnlyWhilePublic = viewers.forall(v =>
    (views.get(v).page == "body") == (status == "public") and
    views.get(v).listed == (status == "public") and views.get(v).snippet == (status == "public"))
  val tombstoneOnlyAfterAuthorDeletesPublicWork = viewers.forall(v =>
    (views.get(v).page == "tombstone") == (status == "author_deleted")) and
    (status != "author_deleted" or everPublic)
  val hiddenPageOnlyForAPublicWorkMadePrivate = viewers.forall(v =>
    (views.get(v).page == "hidden") == (status == "hidden")) and (status != "hidden" or everPublic)
  // C and D get no exception: no page, no comments, whatever link is used.
  val noExceptionForUnavailableWork = not(unavailable.contains(status)) or viewers.forall(v =>
    views.get(v) == { page: "not_found", listed: false, snippet: false, comments: Set() })
  // The replacement pages keep exactly the comments that were lawful to read, not every comment.
  val pagesKeepApprovedComments = not(Set("author_deleted", "hidden").contains(status)) or
    viewers.forall(v => approvedPresent.subseteq(views.get(v).comments))
  val othersSeeOnlyApproved = viewers.forall(v => views.get(v).comments.forall(id =>
    not(comments.get(id).removed) and
    (comments.get(id).review == "approved" or comments.get(id).writer == v)))
  // Deleting, hiding or removing the work never deletes or edits anybody's comment.
  val workChangesKeepComments =
    not(Set("publish", "make_private", "author_delete", "moderation_remove").contains(command.op)) or
    comments == previousComments
  val noNewTextOffPublic = comments.keys() == previousComments.keys() or previousStatus == "public"
  val deletionIsTerminal = not(terminal.contains(previousStatus)) or status == previousStatus
  // COM-11: the work's author is told only of an approval made while the work is public; nothing
  // that happened while it was hidden is delivered later, on republication or otherwise.
  val noNoticeFromHiddenPeriod = notified == previousNotified or
    (command.op == "review" and previousStatus == "public" and command.value == "approved" and
      notified.exclude(previousNotified) == Set(command.id))
  val finiteDomain = comments.keys().subseteq(ids)
  val tombstoneSafe = bodyOnlyWhilePublic and tombstoneOnlyAfterAuthorDeletesPublicWork and
    hiddenPageOnlyForAPublicWorkMadePrivate and noExceptionForUnavailableWork and
    pagesKeepApprovedComments and othersSeeOnlyApproved and workChangesKeepComments and
    noNewTextOffPublic and deletionIsTerminal and noNoticeFromHiddenPeriod and finiteDomain
}
