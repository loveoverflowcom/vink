// O-17@r2 / MOD-10 / PUB-07 browser checks against the real App and scratch PostgreSQL.
// Three loopback fixture servers share the database: `--author-base` acts as the work's author, `--base` as a
// second principal holding an admin seat (`vink-app admin add`). Every state change goes through
// the real HTTP API: the author publishes a new work while automation is paused, the admin
// approves it from the human queue, a scoped moderator rejects it, the author appeals and
// the admin reverses. No response is fabricated or intercepted.

const TITLE = 'O-17 [Mẫu] Bài thơ sẽ bị gỡ';
const LINE = 'Dòng thơ chỉ có trong bản sẽ bị gỡ.';
const COMMENT = 'O-17 [Mẫu] Nhận xét đã duyệt vẫn ở lại sau khi bản bị gỡ.';
const NOTICE = 'Bản này đã bị gỡ sau khi kiểm duyệt.';

async function api(actor, path, init = {}) {
  const response = await fetch(`${actor.base}${path}`, {
    ...init,
    headers: { 'Content-Type': 'application/json', Origin: actor.base, ...(init.headers ?? {}) },
  });
  const text = await response.text();
  let body = null;
  try { body = text ? JSON.parse(text) : null; } catch { body = text; }
  return { status: response.status, cache: response.headers.get('cache-control'), body };
}

export async function reviewWithdrawnPage(page, { args, check }) {
  console.log('O-17@r2 review-withdrawn page (author + admin fixtures, real API):');
  const author = { base: args['author-base'] };
  const admin = { base: args.base };
  const reviewer = { base: args['reviewer-base'] };
  if (!reviewer.base) throw new Error('missing --reviewer-base');
  if (!author.base) throw new Error('missing --author-base');
  const post = (actor, path, body) => api(actor, path, { method: 'POST', body: JSON.stringify(body ?? {}) });

  // 1. The author writes and submits a new work while automation is paused (no evaluator).
  const automation = await api(admin, '/api/v1/moderation/automation');
  check(automation.status === 200, 'the admin fixture reads the automation mode');
  if (automation.body.mode !== 'paused') {
    const paused = await post(admin, '/api/v1/moderation/automation/mode', { expected_version: automation.body.version, mode: 'paused', reason: 'verify web O-17' });
    check(paused.status === 200 && paused.body.mode === 'paused', 'the admin pauses automation with a reason');
  }
  const draft = await post(author, '/api/v1/drafts');
  check(draft.status === 201, 'the author creates a draft');
  const work = draft.body.work_id;
  const saved = await api(author, `/api/v1/drafts/${work}`, { method: 'PUT', body: JSON.stringify({ expected_version: draft.body.version, title: TITLE, document: { schema_version: draft.body.document.schema_version, blocks: [{ lines: [LINE] }] } }) });
  check(saved.status === 200, 'the author saves the draft');
  const tags = await api(author, `/api/v1/drafts/${work}/tags`);
  const snapshot = await post(author, `/api/v1/works/${work}/snapshots`, { command_key: `o17-snapshot-${work}`, expected_draft_version: saved.body.version, expected_metadata_version: tags.body.version });
  check(snapshot.status === 201, 'the author captures the exact snapshot');
  const revision = snapshot.body.revision_id;
  const state = await api(author, `/api/v1/works/${work}/publication/state`);
  const requested = await post(author, `/api/v1/works/${work}/publication/request-public`, { command_key: `o17-public-${work}`, revision_id: revision, expected_generation: state.body.generation });
  check(requested.status === 200, 'the author asks to make it public');

  // 2. The admin approves it from the human queue.
  const queue = await api(admin, '/api/v1/moderation/works/queue');
  const queued = queue.body.find((c) => c.work_id === work && c.revision_id === revision);
  check(queued?.origin === 'automation_paused', 'a paused submission waits in the human queue');
  const detail = await api(admin, `/api/v1/moderation/works/cases/${queued.id}`);
  check(detail.status === 200 && detail.body.document.blocks[0].lines[0] === LINE, 'the reviewer reads the exact submitted snapshot');
  await page.goto(`${admin.base}/kiem-duyet/ca/${queued.id}`);
  await page.waitFor(`document.documentElement.dataset.hydrated === 'true' && document.querySelector('#case-claim')`, 'private case snapshot');
  check(await page.eval(`document.querySelector('.vk-moderation__snapshot')?.textContent.includes(${JSON.stringify(LINE)})`), 'the case UI renders the submitted snapshot');
  const staffResponse = await fetch(`${admin.base}/kiem-duyet/ca/${queued.id}`);
  check(staffResponse.headers.get('cache-control') === 'no-store', 'private case HTML is no-store');
  const staffHtml = await staffResponse.text();
  check(!staffHtml.includes(LINE), 'staff snapshot is absent from SSR and hydration payload');
  check(await page.eval(`!document.querySelector('#case-approve')`), 'an unclaimed case has no conclusion action');
  await page.click('#case-claim');
  await page.waitFor(`document.querySelector('#moderation-case')?.textContent.includes('Phiên bản ca 2')`, 'claim acknowledged');
  await page.typeAtEnd('#case-reason', 'Phù hợp chính sách');
  await page.capture('moderation-case-desktop');
  await page.click('#case-approve');
  await page.waitFor(`document.querySelector('#moderation-case')?.textContent.includes('Đã kết luận')`, 'approval acknowledged');
  check((await api(admin, `/api/v1/works/${work}/publication`)).status === 200, 'case UI approves through the real API');

  // A comment from the author, approved by the admin (a comment moderator grant from the
  // endorsement phase), stays after the rejection.
  const comment = await post(author, `/api/v1/works/${work}/discussion/comments`, { target_id: null, body: COMMENT, command_key: crypto.randomUUID() });
  check(comment.status === 201, 'the author leaves a comment for review');
  const comments = await api(admin, '/api/v1/moderation/comments');
  const item = comments.body.items.find((entry) => entry.comment_id === comment.body.comment_id);
  const decided = await post(admin, `/api/v1/moderation/comments/${comment.body.comment_id}/decisions`, { expected_version: item.version, action: 'approve' });
  check(decided.status === 200, 'the comment is approved');

  // 3. The reader page and Explore show the public work; the tab caches both.
  const readerPath = `/tac-pham/${work}`;
  await page.goto(`${admin.base}/`);
  await page.waitFor(`document.documentElement.dataset.hydrated === 'true' && document.querySelector('a[href^="/tac-pham/${work}"]')`, 'Explore lists the published work');
  await page.goto(`${admin.base}${readerPath}`);
  await page.waitFor(`document.documentElement.dataset.hydrated === 'true' && document.querySelector('#reader-paper')?.textContent.includes(${JSON.stringify(LINE)})`, 'the reader shows the body');

  // The admin grants a third fixture principal Works authority. That moderator rejects;
  // an admin verdict would be final and could not be appealed (MOD-10).
  const grant = await post(admin, '/api/v1/moderation/grants', { grantee_kind: 'principal', grantee_id: '77777777-7777-4777-8777-777777777777', capability: 'works', scope_kind: 'work', scope_work_id: work, expires_at: null });
  check(grant.status === 201, 'the admin grants scoped work moderation');
  const opened = await post(reviewer, `/api/v1/moderation/works/${work}/revisions/${revision}/cases`);
  check(opened.status === 201, 'the moderator opens a case on the public revision');
  const claimed = await post(reviewer, `/api/v1/moderation/works/cases/${opened.body.case_id}/claim`, { expected_version: 1 });
  const rejected = await post(reviewer, `/api/v1/moderation/works/cases/${opened.body.case_id}/conclude`, { expected_version: claimed.body.version, verdict: 'rejected', reason: 'Cần xem lại chính sách' });
  check(rejected.status === 200, 'the moderator rejects the public revision');

  // 5. The public API: the old snapshot is gone, the reader read model is the notice.
  const publication = await api(admin, `/api/v1/works/${work}/publication`);
  check(publication.status === 404, 'GET …/publication answers 404 at once');
  const reader = await api(admin, `/api/v1/works/${work}/reader`);
  check(reader.status === 200 && reader.body.status === 'review_withdrawn' && reader.cache === 'no-store' && !JSON.stringify(reader.body).includes(TITLE), 'GET …/reader answers review_withdrawn, no-store, without the title');

  // 6. SSR of the old URL.
  const response = await fetch(`${admin.base}${readerPath}`);
  const html = await response.text();
  check(response.status === 200, 'the old URL answers 200');
  check((response.headers.get('cache-control') ?? '').includes('no-store'), 'the old URL is no-store');
  check(/<meta[^>]+name="robots"[^>]+content="noindex"/.test(html), 'the old URL is noindex');
  check(html.includes(NOTICE) && html.includes(COMMENT), 'SSR shows the notice and the approved comment');
  check(!html.includes(LINE) && !html.includes(TITLE), 'SSR carries neither the body nor the title of the removed revision');

  // 7. Opening the reader again in the same tab confirms with the server first, purges the tab
  // cache and removes the card from the cached Explore list.
  await page.goto(`${admin.base}/`);
  await page.waitFor(`document.documentElement.dataset.hydrated === 'true'`, 'Explore again');
  await page.goto(`${admin.base}${readerPath}`);
  await page.waitFor(`document.documentElement.dataset.hydrated === 'true' && document.querySelector('[data-testid=reader-review-withdrawn]')`, 'the reader shows the notice');
  check(await page.eval(`!document.body.textContent.includes(${JSON.stringify(LINE)}) && document.querySelector('#reader-header h1')?.textContent.trim() === ${JSON.stringify(NOTICE)}`), 'the hydrated page shows the notice, never the body');
  check(await page.eval(`document.body.textContent.includes(${JSON.stringify(COMMENT)})`), 'the discussion stays readable');
  await page.click('#reader-return');
  await page.waitFor(`location.pathname === '/' && document.documentElement.dataset.hydrated === 'true'`, 'back to Explore');
  check(await page.eval(`!document.querySelector('a[href^="/tac-pham/${work}"]')`), 'Explore no longer lists the removed work');

  // 8. A new comment is refused by the server.
  const refused = await post(author, `/api/v1/works/${work}/discussion/comments`, { target_id: null, body: 'Bình luận mới.', command_key: crypto.randomUUID() });
  check(refused.status === 404, 'a new comment on the removed revision is refused at the server');

  // The author files an appeal from their own status page; admin resolves the stored case.
  await page.goto(`${author.base}/ban-viet/${work}/kiem-duyet`);
  await page.waitFor(`document.querySelector('[id^="file-appeal"]')`, 'author appeal action');
  await page.click('[id^="file-appeal"]');
  await page.waitFor(`document.querySelector('#author-reviews')?.textContent.includes('Đã ghi nhận, chờ quản trị viên')`, 'appeal recorded');
  check(await page.eval(`document.body.textContent.includes('Chưa có thông báo nào được gửi')`), 'author is told that no notification was sent');
  const appealQueue = await api(admin, '/api/v1/moderation/appeals');
  const appeal = appealQueue.body.find(a => a.work_id === work);
  check(appeal?.state === 'open', 'admin queue contains the actual recorded appeal');
  await page.goto(`${admin.base}/kiem-duyet/phan-doi/${appeal.id}`);
  await page.waitFor(`document.querySelector('#appeal-reason')`, 'admin appeal snapshot');
  await page.viewport(390, 844);
  await page.typeAtEnd('#appeal-reason', 'Đã xem xét lại, bản này phù hợp');
  check(await page.eval(`document.documentElement.scrollWidth <= innerWidth`), 'appeal page reflows at 390px');
  await page.capture('moderation-appeal-mobile');
  await page.click('#appeal-reverse');
  await page.waitFor(`document.querySelector('#moderation-appeal')?.textContent.includes('Đã ghi nhận.')`, 'appeal conclusion acknowledged');
  check((await api(admin, `/api/v1/works/${work}/publication`)).status === 200, 'appeal reversal republishes the exact current intent');

  // PUB-07: hiding a public work serves only the notice and eligible discussion.
  const beforeHide = await api(author, `/api/v1/works/${work}/publication/state`);
  const hidden = await post(author, `/api/v1/works/${work}/publication/set-private`, { command_key: crypto.randomUUID(), expected_generation: beforeHide.body.generation });
  check(hidden.status === 200, 'author makes the public work private');
  const hiddenReader = await api(admin, `/api/v1/works/${work}/reader`);
  check(JSON.stringify(Object.keys(hiddenReader.body).sort()) === JSON.stringify(['status','work_id']) && hiddenReader.body.status === 'hidden' && hiddenReader.cache === 'no-store', 'hidden reader exposes no author or manuscript metadata');
  const hiddenResponse = await fetch(`${admin.base}${readerPath}`);
  const hiddenHtml = await hiddenResponse.text();
  check(hiddenResponse.status === 200 && hiddenResponse.headers.get('cache-control') === 'no-store' && /name="robots"[^>]+content="noindex"/.test(hiddenHtml), 'hidden SSR is 200, no-store and noindex');
  check(hiddenHtml.includes('Nội dung đã được ẩn') && hiddenHtml.includes(COMMENT) && !hiddenHtml.includes(TITLE) && !hiddenHtml.includes(LINE), 'hidden SSR keeps the comment without title or body');
  await page.goto(`${admin.base}${readerPath}`);
  await page.waitFor(`document.querySelector('[data-testid=reader-hidden]') && document.documentElement.dataset.hydrated === 'true'`, 'hidden hydrated reader');
  await page.capture('reader-hidden-mobile');
  const blocked = await post(author, `/api/v1/works/${work}/discussion/comments`, { body: 'Không ghi mới', command_key: crypto.randomUUID() });
  check(blocked.status === 409 && blocked.body.error.code === 'hidden_read_only', 'hidden page rejects new comments');
  const reopen = await api(author, `/api/v1/works/${work}/publication/state`);
  check((await post(author, `/api/v1/works/${work}/publication/request-public`, { command_key: crypto.randomUUID(), revision_id: revision, expected_generation: reopen.body.generation })).status === 200, 'author republishes the approved snapshot');
  check((await api(admin, `/api/v1/works/${work}/reader`)).body.status === 'published', 'republication replaces the hidden notice');

  // Leave automation enabled for later phases.
  const now = await api(admin, '/api/v1/moderation/automation');
  const enabled = await post(admin, '/api/v1/moderation/automation/mode', { expected_version: now.body.version, mode: 'enabled', reason: 'verify web O-17 done' });
  check(enabled.status === 200, 'the admin re-enables automation');
}
