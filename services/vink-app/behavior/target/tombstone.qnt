// PRODUCT-DECISIONS-20261110, M3 — TARGET SPECIFICATION. Not implemented: today a work without
// a publication row answers 404 on every surface, including its discussion, and no author
// deletion exists. Nothing here is production conformance; see README.md.
//
// What other people can read of one work and its comments, by the reason the work stopped being
// public. Four cases must stay apart (USER-07):
//   A  the author deleted a work that was public          -> a tombstone page keeps its comments
//   B  the author made the work private                    -> nothing, old links included
//   C  moderation (admin/system) removed the work          -> nothing
//   D  the work was never public (or deleted while hidden) -> nothing
// ASSUMPTIONS: one serialized commit per action; who decides publication, removal and comment
// review is publication.qnt / comment_edit.qnt, here they are shell commands. Viewers are other
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

  // "unpublished" | "public" | "private" | "author_deleted" | "deleted_hidden" | "moderation_removed"
  var status: str
  var everPublic: bool
  var comments: int -> Comment
  var views: int -> View
  var command: Command
  var outcome: str
  var previousStatus: str
  var previousComments: int -> Comment
  var cursor: int

  // The read rule. `tombstones` is the mutation switch: the set of statuses whose old URL keeps
  // a tombstone and its comment thread. The rule has exactly one member.
  pure val lawfulTombstones = Set("author_deleted")
  pure def project(st: str, cs: int -> Comment, tombstones: Set[str]): int -> View =
    viewers.mapBy(v => {
      val thread = st == "public" or tombstones.contains(st)
      {
        page: if (st == "public") "body" else if (tombstones.contains(st)) "tombstone" else "not_found",
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
    views' = project("unpublished", Map(), lawfulTombstones),
    command' = cmd("init", 0, 0, "none"), outcome' = "init", previousStatus' = "unpublished",
    previousComments' = Map(), cursor' = 0,
  }

  action commit(c: Command, result: str, st: str, cs: int -> Comment, tombstones: Set[str]): bool = all {
    status' = st, everPublic' = (everPublic or st == "public"), comments' = cs,
    views' = project(st, cs, tombstones), command' = c, outcome' = result,
    previousStatus' = status, previousComments' = comments,
    cursor' = if (cursor < 60) cursor + 1 else cursor,
  }

  pure val terminal = Set("author_deleted", "deleted_hidden", "moderation_removed")

  action dispatchWith(c: Command, tombstones: Set[str]): bool = {
    val refuse = if (terminal.contains(status)) "work_unavailable" else "unchanged"
    if (c.op == "publish")
      if (status == "unpublished" or status == "private") commit(c, "published", "public", comments, tombstones)
      else commit(c, refuse, status, comments, tombstones)
    else if (c.op == "make_private")
      if (status == "public") commit(c, "made_private", "private", comments, tombstones)
      else commit(c, refuse, status, comments, tombstones)
    // Deleting a public work leaves the tombstone; deleting a hidden one leaves nothing, so
    // deletion never widens what a private or unpublished work shows.
    else if (c.op == "author_delete")
      if (status == "public") commit(c, "deleted_with_tombstone", "author_deleted", comments, tombstones)
      else if (status == "private" or status == "unpublished")
        commit(c, "deleted", "deleted_hidden", comments, tombstones)
      else commit(c, refuse, status, comments, tombstones)
    else if (c.op == "moderation_remove")
      if (status == "public" or status == "private")
        commit(c, "removed_by_moderation", "moderation_removed", comments, tombstones)
      else commit(c, refuse, status, comments, tombstones)
    // New text needs a public work. A tombstone takes no new comment or reply (safe baseline).
    else if (c.op == "submit")
      if (comments.keys().contains(c.id)) commit(c, "duplicate", status, comments, tombstones)
      else if (status == "author_deleted") commit(c, "tombstone_read_only", status, comments, tombstones)
      else if (status != "public") commit(c, "work_unavailable", status, comments, tombstones)
      else commit(c, "created", status,
        comments.put(c.id, { writer: c.actor, review: "pending", removed: false }), tombstones)
    // A comment keeps its own review whatever happened to the work.
    else if (c.op == "review")
      if (not(comments.keys().contains(c.id)) or comments.get(c.id).removed)
        commit(c, "comment_unavailable", status, comments, tombstones)
      else commit(c, "reviewed", status,
        comments.setBy(c.id, x => x.with("review", c.value)), tombstones)
    // A writer may still take their own words down where the thread is readable.
    else
      if (not(comments.keys().contains(c.id)) or comments.get(c.id).removed or
          comments.get(c.id).writer != c.actor or not(status == "public" or status == "author_deleted"))
        commit(c, "comment_unavailable", status, comments, tombstones)
      else commit(c, "comment_removed", status,
        comments.setBy(c.id, x => x.with("removed", true)), tombstones)
  }
  action dispatch(c: Command): bool = dispatchWith(c, lawfulTombstones)

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

  // B then D: private hides everything, public again restores, deleting while private leaves
  // no tombstone.
  pure val privateScenario = List(
    publish, submit(2, 1), review(1, "approved"),
    makePrivate,        // old links answer not_found, comments included
    removeOwn(2, 1),    // comment_unavailable while the work is private
    makePrivate,        // unchanged
    publish,            // the approved comment is readable again
    makePrivate,
    authorDelete,       // deleted: no tombstone for a work that was private when deleted
    submit(3, 2),       // work_unavailable
  )
  action privateStep = all { cursor < privateScenario.length(), dispatch(privateScenario.nth(cursor)) }

  // C: moderation removal is not author deletion.
  pure val removedScenario = List(
    publish, submit(2, 1), review(1, "approved"),
    moderationRemove,   // not_found, no comments
    authorDelete,       // work_unavailable: the author cannot turn a removal into a tombstone
    review(1, "rejected"), // moderation of the comment still works
  )
  action removedStep = all { cursor < removedScenario.length(), dispatch(removedScenario.nth(cursor)) }

  // Negative control: treating "private" like "deleted by the author" lets an old link read the
  // comments of a work its author made private.
  pure val privateLeak = List(publish, submit(2, 1), review(1, "approved"), makePrivate)
  action mutatedPrivateStep = all {
    cursor < privateLeak.length(),
    dispatchWith(privateLeak.nth(cursor), Set("author_deleted", "private")),
  }

  pure val hidden = Set("unpublished", "private", "deleted_hidden", "moderation_removed")
  val approvedPresent = comments.keys().filter(id =>
    comments.get(id).review == "approved" and not(comments.get(id).removed))

  // The body, its previews and every listing exist only while the work is public.
  val bodyOnlyWhilePublic = viewers.forall(v =>
    (views.get(v).page == "body") == (status == "public") and
    views.get(v).listed == (status == "public") and views.get(v).snippet == (status == "public"))
  val tombstoneOnlyAfterAuthorDeletesPublicWork = viewers.forall(v =>
    (views.get(v).page == "tombstone") == (status == "author_deleted")) and
    (status != "author_deleted" or everPublic)
  // B, C and D get no exception: no page, no comments, whatever link is used.
  val noExceptionForHiddenWork = not(hidden.contains(status)) or viewers.forall(v =>
    views.get(v) == { page: "not_found", listed: false, snippet: false, comments: Set() })
  // The tombstone keeps exactly the comments that were lawful to read, not every comment.
  val tombstoneKeepsApprovedComments = status != "author_deleted" or viewers.forall(v =>
    approvedPresent.subseteq(views.get(v).comments))
  val othersSeeOnlyApproved = viewers.forall(v => views.get(v).comments.forall(id =>
    not(comments.get(id).removed) and
    (comments.get(id).review == "approved" or comments.get(id).writer == v)))
  // Deleting, hiding or removing the work never deletes or edits anybody's comment.
  val workChangesKeepComments =
    not(Set("publish", "make_private", "author_delete", "moderation_remove").contains(command.op)) or
    comments == previousComments
  val noNewTextOffPublic = comments.keys() == previousComments.keys() or previousStatus == "public"
  val deletionIsTerminal = not(terminal.contains(previousStatus)) or status == previousStatus
  val finiteDomain = comments.keys().subseteq(ids)
  val tombstoneSafe = bodyOnlyWhilePublic and tombstoneOnlyAfterAuthorDeletesPublicWork and
    noExceptionForHiddenWork and tombstoneKeepsApprovedComments and othersSeeOnlyApproved and
    workChangesKeepComments and noNewTextOffPublic and deletionIsTerminal and finiteDomain
}
