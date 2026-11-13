// PUB-AUTHOR-WEB-01 browser scenario: the author's whole path through the real Web, against the
// real App, the real Identity process and scratch PostgreSQL (docs/design/publication-author.md).
//
// Two real people are signed in at the same time, each in its own browser context (its own cookie
// jar): the author, and a second person who holds an admin seat. Sign-in goes through the
// email-code form (codes are read from Identity's local outbox: the only non-production part).
// The admin seat is granted through the operator CLI (`vink-app admin add`); pausing the
// automatic evaluator, writing, saving, tagging, submitting, approving/rejecting and withdrawing
// are done through the pages. No principal is a fixture, no publication row is inserted, no
// approval is faked.
//
// Fault injection only delays or drops real traffic: an answer that the server did commit is
// dropped on the way back (`Fetch.failRequest` at the response stage), a request is held before
// it reaches the server, an expired session is real (sign-out in another tab). Nothing synthetic
// stands in for a success. Every outcome is read back from the server (state/draft/public reads)
// and not from the page's own words.

import { execFileSync } from 'node:child_process';
import { EDITOR_READY, HYDRATED, STANDING, account, fabricate, fill, openAccount, q, signIn, signOut } from './auth-smoke.mjs';

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

const TITLE_1 = '[Mẫu] Mưa tháng Mười — bản một';
const LINE_1A = 'Dòng đầu của bản một, ỹ ỷ ữ ợ Đ đ.';
const LINE_1B = 'Khổ thứ hai của bản một.';
const BODY_1 = `${LINE_1A}\n\n${LINE_1B}`;
const TITLE_2 = '[Mẫu] Mưa tháng Mười — bản hai';
const LINE_2 = 'Dòng thêm của bản hai.';
const FORM_TAG = 'Tản văn';
const PUBLICATION = '#publication';

// ---- small helpers -----------------------------------------------------------------------

const call = (page, path, init) =>
  page.eval(`fetch(${JSON.stringify(path)}, ${JSON.stringify(init ?? {})}).then(async (r) => ({ status: r.status, etag: r.headers.get('etag'), cache: r.headers.get('cache-control'), body: await r.json().catch(() => null) }))`);
const post = (page, path, body) => call(page, path, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) });
const stateOf = async (page, work) => (await call(page, `/api/v1/works/${work}/publication/state`)).body;
const phase = (page) => page.eval(`document.querySelector('${PUBLICATION}')?.dataset.phase ?? null`);
const waitPhase = (page, wanted, what, timeout) =>
  page.waitFor(`document.querySelector('${PUBLICATION}')?.dataset.phase === ${JSON.stringify(wanted)}`, what ?? `publication ${wanted}`, timeout);
const panelText = (page) => page.eval(`document.querySelector('${PUBLICATION}')?.innerText ?? ''`);
const choose = (page, value) => page.eval(`document.querySelector('input[name=audience][value=${value}]').click()`);
const primary = (page) => page.eval(`document.querySelector('#publication-primary')?.innerText ?? null`);
const storedText = (draft) => draft.document.blocks.map((block) => block.lines.join('\n')).join('\n\n');
const publicRead = async (base, work) => {
  const response = await fetch(`${base}/api/v1/works/${work}/publication`);
  return { status: response.status, etag: response.headers.get('etag'), body: response.status === 200 ? await response.json() : null };
};

/** Counts and optionally drops/holds real traffic of one page. */
class Wire {
  constructor(page) {
    this.page = page;
    this.requests = [];
    this.drop = null; // { method, suffix }: fail the next matching response after the server answered
    this.hold = null; // { method, suffix }: hold the next matching request before it reaches the server
    this.holdResponse = null; // { method, suffix }: hold the next matching response after the server answered
    this.failBefore = null; // { method, suffix }: fail the next matching request before it reaches the server
    this.held = null;
    this.heldResponse = null;
    this.dropped = 0;
    this.listener = (message) => {
      if (message.sessionId !== page.sessionId) return;
      if (message.method === 'Network.requestWillBeSent') {
        const { request } = message.params;
        const path = new URL(request.url).pathname;
        if (path.startsWith('/api/v1/')) this.requests.push({ method: request.method, path });
      }
      if (message.method === 'Fetch.requestPaused') this.paused(message.params).catch(() => {});
    };
    page.cdp.listeners.push(this.listener);
  }

  async paused({ requestId, request, responseStatusCode }) {
    const path = new URL(request.url).pathname;
    const matches = (rule) => rule && request.method === rule.method && path.endsWith(rule.suffix);
    if (responseStatusCode !== undefined) {
      if (matches(this.drop)) {
        this.drop = null;
        this.dropped += 1;
        await this.page.send('Fetch.failRequest', { requestId, errorReason: 'ConnectionClosed' });
      } else if (matches(this.holdResponse) && !this.heldResponse) {
        this.heldResponse = requestId;
      } else await this.page.send('Fetch.continueRequest', { requestId });
    } else if (matches(this.failBefore)) {
      this.failBefore = null;
      await this.page.send('Fetch.failRequest', { requestId, errorReason: 'ConnectionRefused' });
    } else if (matches(this.hold) && !this.held) {
      this.held = requestId;
    } else await this.page.send('Fetch.continueRequest', { requestId });
  }

  enable() {
    const patterns = [];
    for (const urlPattern of ['*/api/v1/works/*', '*/api/v1/drafts/*']) {
      patterns.push({ urlPattern, requestStage: 'Request' }, { urlPattern, requestStage: 'Response' });
    }
    return this.page.send('Fetch.enable', { patterns });
  }

  async close() {
    this.page.cdp.listeners = this.page.cdp.listeners.filter((l) => l !== this.listener);
    await this.page.send('Fetch.disable').catch(() => {});
  }

  count(method, suffix) {
    return this.requests.filter((r) => r.method === method && r.path.endsWith(suffix)).length;
  }

  async heldRequest() {
    const deadline = Date.now() + 10000;
    while (!this.held) {
      if (Date.now() > deadline) throw new Error('timed out waiting for the held request');
      await sleep(50);
    }
    return this.held;
  }

  async release() {
    const id = this.held;
    this.held = null;
    this.hold = null;
    await this.page.send('Fetch.continueRequest', { requestId: id });
  }

  async heldAnswer() {
    const deadline = Date.now() + 10000;
    while (!this.heldResponse) {
      if (Date.now() > deadline) throw new Error('timed out waiting for the held response');
      await sleep(50);
    }
    return this.heldResponse;
  }

  async releaseAnswer() {
    const id = this.heldResponse;
    this.heldResponse = null;
    this.holdResponse = null;
    await this.page.send('Fetch.continueRequest', { requestId: id });
  }
}

async function enter(page, ctx, address, penName) {
  await openAccount(page, ctx.base, 'anonymous');
  await signIn(page, ctx, address);
  await page.waitFor(`${STANDING} === 'needs_principal' && ${q('#pen-name')}`, `pen-name step for ${address}`);
  await fill(page, '#pen-name', penName);
  await page.click('#pen-name-save');
  await page.waitFor(`${STANDING} === 'active'`, `active account ${penName}`);
  return (await account(page)).body.principal_id;
}

async function newDraft(page, base, title, body) {
  await page.goto(`${base}/ban-viet`);
  await page.waitFor(`${HYDRATED} && ${q('#create-draft')}`, 'the desk');
  await page.click('#create-draft');
  await page.waitFor(`location.pathname.startsWith('/ban-viet/') && ${EDITOR_READY} && document.querySelector('${PUBLICATION}')`, 'the editor with its publication region');
  const work = await page.eval(`location.pathname.split('/').pop()`);
  await fill(page, '#draft-title', title);
  await page.typeInBody(body);
  return work;
}

async function pickFormTag(page) {
  await page.waitFor(`${q('#work-tags-form .vk-ui-chip')}`, 'form tags from the catalog');
  await page.click('#work-tags-edit > summary');
  await page.click('#work-tags-form .vk-ui-chip');
  await page.waitFor(`document.querySelector('[data-testid=tag-status]')?.dataset.status === 'unsaved'`, 'the chosen tag is unsaved');
}

/** The moderator opens the queue page, then the case of exactly this revision (found in the
 * queue the server lists), and reads its snapshot. */
async function openCase(mod, base, work, revision) {
  await mod.goto(`${base}/kiem-duyet`);
  await mod.waitFor(`${HYDRATED} && document.querySelector('#moderation-home')`, 'the moderation queue');
  const queue = (await call(mod, '/api/v1/moderation/works/queue')).body;
  const item = queue.find((c) => c.work_id === work && c.revision_id === revision);
  if (!item) throw new Error(`no queued case for revision ${revision}: ${JSON.stringify(queue.map((c) => [c.work_id, c.revision_id, c.state]))}`);
  await mod.waitFor(`document.querySelector('a[href="/kiem-duyet/ca/${item.id}"]')`, 'the case link in the queue');
  await mod.eval(`document.querySelector('a[href="/kiem-duyet/ca/${item.id}"]').click()`);
  await mod.waitFor(`${q('#case-claim')} && document.querySelector('.vk-moderation__snapshot')`, 'the case page with its snapshot');
}

async function conclude(mod, verdict, reason) {
  await mod.click('#case-claim');
  await mod.waitFor(`document.querySelector('#moderation-case')?.textContent.includes('Phiên bản ca 2')`, 'claim acknowledged');
  await mod.typeAtEnd('#case-reason', reason);
  await mod.click(verdict === 'approved' ? '#case-approve' : '#case-reject');
  await mod.waitFor(`document.querySelector('#moderation-case')?.textContent.includes('Đã kết luận')`, `${verdict} acknowledged`);
}

export async function publicationAuthor(page, { args, check, openPage }) {
  if (!args.outbox) throw new Error('phase publication-author needs --outbox');
  if (!process.env.VINK_E2E_APP_BIN) throw new Error('phase publication-author needs VINK_E2E_APP_BIN (the operator CLI that seats the admin)');
  const base = args.base;
  const ctx = { args, base, seen: new Set() };
  const stamp = Date.now().toString(36);
  const author = page;
  console.log('Publication author (PUB-AUTHOR-WEB-01): two real people, real Identity:');

  // ---- three real people, each with its own session --------------------------------------------
  // The author; an admin who only does setup (pauses the evaluator, grants work moderation); and
  // a moderator who holds that grant and gives every verdict through the pages. An admin's own
  // verdict would be final and not appealable (MOD-10), so the moderator is not the admin.
  const contexts = [];
  for (let i = 0; i < 2; i++) contexts.push((await page.cdp.send('Target.createBrowserContext', { disposeOnDetach: false })).browserContextId);
  const admin = await openPage(contexts[0]);
  const mod = await openPage(contexts[1]);
  const principalA = await enter(author, ctx, `tac-gia-${stamp}@vi-du.vn`, 'Tác giả [Mẫu]');
  const principalAdmin = await enter(admin, ctx, `quan-tri-${stamp}@vi-du.vn`, 'Quản trị [Mẫu]');
  const principalM = await enter(mod, ctx, `nguoi-duyet-${stamp}@vi-du.vn`, 'Người duyệt [Mẫu]');
  check(new Set([principalA, principalAdmin, principalM]).size === 3, 'the author, the admin and the moderator are three real principals with separate sessions');
  // Setup through the operator CLI (the admin seat has no web way to be granted).
  execFileSync(process.env.VINK_E2E_APP_BIN, ['admin', 'add', principalAdmin, 'verify web PUB-AUTHOR-WEB-01'], { env: process.env, stdio: 'pipe' });
  await admin.goto(`${base}/kiem-duyet`);
  await admin.waitFor(`${HYDRATED} && ${q('#automation-toggle')}`, 'the admin controls');
  if (!(await admin.eval(`document.querySelector('#moderation-home').textContent.includes('Đang dừng')`))) {
    await admin.typeAtEnd('#automation-reason', 'verify web PUB-AUTHOR-WEB-01: người duyệt xử lý');
    await admin.click('#automation-toggle');
    await admin.waitFor(`document.querySelector('#moderation-home')?.textContent.includes('Đang dừng')`, 'automation paused through the page');
  }
  check(true, 'the admin pauses the automatic evaluator through the page: submissions wait for a person');

  // ---- AC-01: write, tag, submit, approve, read ------------------------------------------------
  console.log('  author writes, tags and submits from Bàn viết:');
  const work = await newDraft(author, base, TITLE_1, BODY_1);
  const grant = await post(admin, '/api/v1/moderation/grants', { grantee_kind: 'principal', grantee_id: principalM, capability: 'works', scope_kind: 'work', scope_work_id: work, expires_at: null });
  check(grant.status === 201, 'the admin grants the moderator work moderation for this work');
  await pickFormTag(author);
  await author.waitFor(`document.querySelector('${PUBLICATION}')?.dataset.phase === 'ready'`, 'the region has read the state');
  check((await primary(author)) === null, 'a private draft offers no action before the author chooses');
  const before = await stateOf(author, work);
  check(before.audience === 'private' && before.revisions.length === 0 && !before.pointer, 'the server shows a private draft with no revision before the submission');
  await choose(author, 'public');
  await author.waitFor(`document.querySelector('#publication-primary')?.innerText === 'Lưu và gửi duyệt'`, 'unsaved text and tags: the action says it saves first');
  check(await author.eval(`document.querySelector('[data-testid=publication-draft]')?.innerText.includes('chưa lưu')`), 'the region tells the author unsaved text is not in the sent version');
  const wire = new Wire(author);
  await author.click('#publication-primary');
  await waitPhase(author, 'reconciled', 'the submission settles', 30000);
  const submitted = await stateOf(author, work);
  const draft = await author.draft(work);
  const tagsNow = (await call(author, `/api/v1/drafts/${work}/tags`)).body;
  check(wire.count('PUT', `/drafts/${work}`) === 1 && wire.count('PUT', `/drafts/${work}/tags`) === 1 && wire.count('POST', '/snapshots') === 1 && wire.count('POST', '/publication/request-public') === 1, 'one press: one text save, one tag save, one snapshot, one request-public');
  check(submitted.revisions.length === 1 && submitted.revisions[0].title === TITLE_1 && submitted.revisions[0].moderation === 'pending' && submitted.intent?.revision_id === submitted.revisions[0].revision_id && !submitted.pointer, 'the server holds exactly one revision with the typed title, pending, and it is the intent; nothing is public yet');
  check(draft.title === TITLE_1 && storedText(draft) === BODY_1 && Boolean(submitted.revisions[0].metadata_snapshot_id) && tagsNow.form, 'the revision froze the saved text and is bound to the saved tags');
  const r1 = submitted.revisions[0].revision_id;
  check(!(await panelText(author)).includes('Đang công khai'), 'while pending the region does not say the work is public');
  await wire.close();
  await refreshInvariant(author, check);
  await matrix(author, 'pending', check);

  console.log('  the moderator approves it from the human queue:');
  await openCase(mod, base, work, r1);
  check(await mod.eval(`document.querySelector('.vk-moderation__snapshot')?.textContent.includes(${JSON.stringify(LINE_1A)}) && document.querySelector('#moderation-case h1')?.textContent.includes(${JSON.stringify(TITLE_1)})`), 'the reviewer reads the exact submitted title and lines');
  await conclude(mod, 'approved', 'Phù hợp chính sách (verify web)');

  console.log('  the author reads the result; Explore and the reader show that revision:');
  check((await publicRead(base, work)).status === 200, 'the work is public after the approval');
  await author.click('#publication-refresh');
  await author.waitFor(`document.querySelector('#publication-public')?.innerText.includes(${JSON.stringify(`Đang công khai: «${TITLE_1}»`)})`, 'the region reads the approval from the server');
  const approved = await stateOf(author, work);
  check(approved.pointer?.revision_id === r1 && approved.revisions[0].moderation === 'approved', 'the server pointer is the captured revision and it is approved');
  const publication = await publicRead(base, work);
  check(publication.body.title === TITLE_1 && JSON.stringify(publication.body.document.blocks.map((b) => b.lines)) === JSON.stringify([[LINE_1A], [LINE_1B]]) && publication.body.revision_id === r1, 'the public read is the captured revision, text for text');
  check(publication.body.tags.some((t) => t.label === FORM_TAG), `the public metadata carries the chosen tag (${FORM_TAG})`);
  const list = await (await fetch(`${base}/api/v1/works`)).json();
  const card = list.works.find((w) => w.work_id === work);
  check(card && card.title === TITLE_1 && card.tags.some((t) => t.label === FORM_TAG), 'Explore lists the card with the same title and tag');
  const html = await (await fetch(`${base}/tac-pham/${work}`)).text();
  check(html.includes(TITLE_1) && html.includes(LINE_1A) && html.includes(FORM_TAG), 'the reader page renders that revision and its tag');
  check(await author.eval(`document.querySelector('#editor-review-status')?.textContent.includes('kiểm duyệt')`), 'the link to the existing review page stays in the region');

  await rest({ author, mod, work, r1, base, ctx, check, openPage });
}

const countRevisions = async (page, work) => (await stateOf(page, work)).revisions.length;

async function rest(env) {
  const { author, mod, work, r1, base, check, openPage } = env;
  const editorPath = `/ban-viet/${work}`;
  const reopen = async (page) => {
    // Leaving a page that still holds unsaved text raises the browser's own unload prompt;
    // this scenario leaves on purpose (the text it wrote is already checked), so accept only that.
    const accept = (message) => {
      if (message.sessionId === page.sessionId && message.method === 'Page.javascriptDialogOpening' && message.params.type === 'beforeunload') {
        page.send('Page.handleJavaScriptDialog', { accept: true }).catch(() => {});
      }
    };
    page.cdp.listeners.push(accept);
    await page.goto(`${base}${editorPath}`);
    page.cdp.listeners = page.cdp.listeners.filter((item) => item !== accept);
    await page.waitFor(`${EDITOR_READY} && ${HYDRATED} && document.querySelector('${PUBLICATION}')?.dataset.phase === 'ready'`, 'the editor and the region read again');
  };

  // ---- AC-03: r1 public, r2 pending then rejected ----------------------------------------------
  console.log('  r1 stays public while r2 waits and is rejected:');
  await fill(author, '#draft-title', TITLE_2);
  await author.typeInBody(`\n\n${LINE_2}`);
  check((await primary(author)) === 'Lưu và gửi duyệt', 'a work that is public still offers to send a new version, saving first');
  await author.click('#publication-primary');
  await waitPhase(author, 'reconciled', 'r2 submitted', 30000);
  const two = await stateOf(author, work);
  const r2 = two.intent.revision_id;
  check(two.revisions.length === 2 && r2 !== r1 && two.pointer.revision_id === r1 && two.revisions.find((r) => r.revision_id === r2).moderation === 'pending', 'the server holds r1 as the public pointer and r2 as the pending intent');
  const said = await panelText(author);
  check(said.includes(`Đang công khai: «${TITLE_1}».`) && said.includes(`Đang gửi: «${TITLE_2}» — Đang chờ kiểm duyệt.`) && said.includes('Người đọc vẫn đọc bản đang công khai.'), 'the region names r1 as read now and r2 as waiting; r1 is not shown as hidden');
  check((await publicRead(base, work)).body.revision_id === r1 && (await publicRead(base, work)).body.title === TITLE_1, 'readers still get r1, text for text');
  await matrix(author, 'public-r1-pending-r2', check);
  await openCase(mod, base, work, r2);
  await conclude(mod, 'rejected', 'Nội dung cần chỉnh theo chính sách (verify web)');
  await author.click('#publication-refresh');
  await author.waitFor(`document.querySelector('#publication-requested')?.innerText.includes('Bị từ chối. Lý do: Nội dung cần chỉnh theo chính sách')`, 'the rejection and its reason from the server');
  check((await panelText(author)).includes(`Đang công khai: «${TITLE_1}».`) && (await publicRead(base, work)).body.revision_id === r1, 'r1 is still the public revision after r2 is rejected');
  check(await author.eval(`document.querySelector('#editor-review-status')?.textContent.trim() === 'Xem lý do và gửi phản đối'`), 'the link says a reason and an appeal exist');
  await author.click('#editor-review-status');
  await author.waitFor(`location.pathname === '${editorPath}/kiem-duyet' && document.querySelector('#author-reviews') && document.querySelector('[id^="file-appeal-"]')`, 'the existing review page offers the appeal');
  const reviewText = await author.eval(`document.querySelector('#author-reviews').innerText`);
  check(reviewText.includes('Đã được duyệt') && reviewText.includes('Đang công khai.') && reviewText.includes('Bị từ chối') && reviewText.includes('Nội dung cần chỉnh theo chính sách'), 'the review page uses the same words: approved, public, rejected with its reason');
  await author.click('[id^="file-appeal-"]');
  await author.waitFor(`document.querySelector('#author-reviews')?.textContent.includes('Đã ghi nhận, chờ quản trị viên')`, 'the appeal is recorded through the existing flow');
  check(true, 'can_appeal and decision_version lead to the existing appeal flow (no second form)');
  await reopen(author);
  await exactVersion({ ...env, reopen, editorPath });
  await retries({ ...env, reopen, editorPath });
  await twoTabs({ ...env, reopen, editorPath });
  await withdrawal({ ...env, reopen, editorPath });
  await sessions({ ...env, reopen, editorPath });
}

const press = (page) => page.click('#publication-primary');
const intentOf = async (page, work) => (await stateOf(page, work)).intent?.revision_id;
const caseOf = async (mod, work, revision) =>
  (await call(mod, '/api/v1/moderation/works/queue')).body.find((c) => c.work_id === work && c.revision_id === revision);
/** How many review cases the moderator's queue holds for exactly this revision (an independent
 * oracle for "one ticket": the server opens one case per accepted request while paused). */
const casesFor = async (mod, work, revision) =>
  (await call(mod, '/api/v1/moderation/works/queue')).body.filter((c) => c.work_id === work && c.revision_id === revision).length;

async function exactVersion({ author, mod, work, base, check, openPage, reopen }) {
  console.log('  exact version: typing during the save, a conflict at capture, an immutable snapshot:');
  const before = await countRevisions(author, work);
  await author.typeInBody(' [x]');
  await author.waitStatus('unsaved');
  let wire = new Wire(author);
  wire.hold = { method: 'PUT', suffix: `/drafts/${work}` };
  await wire.enable();
  await press(author);
  await wire.heldRequest();
  await author.typeInBody(' [y]');
  await wire.release();
  await waitPhase(author, 'failed', 'the submission stops');
  const stored = storedText(await author.draft(work));
  check(wire.count('POST', '/snapshots') === 0 && (await countRevisions(author, work)) === before, 'text typed while the save was in flight: no snapshot was taken, nothing was sent');
  check(stored.endsWith('[x]') && !stored.includes('[y]') && (await author.body()).endsWith('[y]') && (await author.status()) === 'unsaved', 'the server holds only what was pressed on; the newer text stays in the editor, still unsaved');
  check((await panelText(author)).includes('Bạn đã gõ thêm khi đang lưu') && (await panelText(author)).includes('Chưa gửi bản nào'), 'the region says so and that nothing was sent');
  await author.click('#publication-dismiss');
  await author.waitFor(`document.activeElement?.id === 'publication-primary'`, 'focus returns to the action');
  check(true, 'dismissing the notice returns focus to the primary action');
  await wire.close();

  // IME (simulated composition through CDP; not a real OS input method): a press while the body
  // is composing waits for the commit. Nothing half-typed is saved or frozen.
  await author.click('#save');
  await author.waitStatus('saved', 'saved text before the IME check');
  await author.eval(`document.querySelector('.cm-content').focus()`);
  await author.key(author.mac ? 'ArrowDown' : 'End', author.mod);
  await author.send('Input.imeSetComposition', { text: 'hồ', selectionStart: 2, selectionEnd: 2 });
  wire = new Wire(author);
  await wire.enable();
  await author.eval(`document.querySelector('#publication-primary').click()`);
  await sleep(500);
  const ignored = wire.count('POST', '/snapshots') === 0 && wire.count('PUT', `/drafts/${work}`) === 0 && (await phase(author)) === 'ready';
  await author.send('Input.insertText', { text: 'hồi' });
  await author.eval(`document.querySelector('#publication-primary').click()`);
  await waitPhase(author, 'reconciled', 'the press after the commit', 30000);
  const frozen = storedText(await author.draft(work));
  check(ignored && wire.count('POST', '/snapshots') === 1 && frozen.endsWith('hồi') && !frozen.endsWith('hồhồi'), 'a press during composition starts nothing; after the commit the same press saves and freezes the committed text (CDP-simulated IME)');
  await wire.close();

  await author.click('#save');
  await author.waitStatus('saved', 'saved text');
  await press(author);
  await waitPhase(author, 'reconciled', 'r3 submitted', 30000);
  const r3 = await intentOf(author, work);
  const r3Before = (await stateOf(author, work)).revisions.find((r) => r.revision_id === r3);
  await author.typeInBody(' [z]');
  await author.click('#save');
  await author.waitStatus('saved', 'later text saved');
  const r3After = (await stateOf(author, work)).revisions.find((r) => r.revision_id === r3);
  const asked = await caseOf(mod, work, r3);
  const detail = (await call(mod, `/api/v1/moderation/works/cases/${asked.id}`)).body;
  const reviewed = detail.document.blocks.map((b) => b.lines.join('\n')).join('\n\n');
  check(r3After.content_hash === r3Before.content_hash && reviewed.includes('[x] [y]') && reviewed.endsWith('hồi') && !reviewed.includes('[z]'), 'the snapshot did not change while the draft kept changing: the reviewer reads what was saved at the press, not the later [z]');

  // A conflict at capture: another tab saves the text between the press and the snapshot.
  await reopen(author);
  const other = await openPage();
  await other.goto(`${base}/`);
  await other.waitFor(HYDRATED_EXPR, 'the second tab');
  wire = new Wire(author);
  wire.hold = { method: 'POST', suffix: '/snapshots' };
  await wire.enable();
  const rev0 = await countRevisions(author, work);
  await press(author);
  await wire.heldRequest();
  const d = await other.draft(work);
  const saved = await put(other, `/api/v1/drafts/${work}`, { expected_version: d.version, title: `${d.title} (thẻ khác)`, document: d.document });
  check(saved.status === 200, 'another tab saved the text meanwhile');
  await wire.release();
  await waitPhase(author, 'conflict', 'a draft conflict');
  check((await panelText(author)).includes(`Bản nháp trên máy chủ đã đổi ở nơi khác (phiên bản ${saved.body.version})`) && (await countRevisions(author, work)) === rev0 && wire.count('POST', '/publication/request-public') === 0, 'a stale draft version is a draft conflict: nothing captured, nothing requested, nothing overwritten');
  await wire.close();

  // And a conflict of the tags only.
  await reopen(author);
  wire = new Wire(author);
  wire.hold = { method: 'POST', suffix: '/snapshots' };
  await wire.enable();
  await press(author);
  await wire.heldRequest();
  const tags = (await call(other, `/api/v1/drafts/${work}/tags`)).body;
  const moved = await put(other, `/api/v1/drafts/${work}/tags`, { expected_version: tags.version, form: tags.form?.tag_id ?? null, themes: [] });
  check(moved.status === 200, 'another tab saved the tags meanwhile');
  await wire.release();
  await waitPhase(author, 'conflict', 'a tag conflict');
  check((await panelText(author)).includes('Thông tin tác phẩm (thẻ) đã đổi ở nơi khác') && (await countRevisions(author, work)) === rev0 && wire.count('POST', '/publication/request-public') === 0, 'a stale tag version is told apart from the text: its own wording, nothing written');
  await wire.close();
  await reopen(author);
  return { other };
}

const HYDRATED_EXPR = `document.documentElement.dataset.hydrated === 'true'`;
const put = (page, path, body) => call(page, path, { method: 'PUT', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) });

async function retries({ author, mod, work, base, check, reopen }) {
  console.log('  double press and lost answers at every step:');
  // One press, pressed twice in the same task: one snapshot, one request.
  let wire = new Wire(author);
  await wire.enable();
  await author.eval(`(() => { const b = document.querySelector('#publication-primary'); b.click(); b.click(); })()`);
  await waitPhase(author, 'reconciled', 'a double press settles', 30000);
  check(wire.count('POST', '/snapshots') === 1 && wire.count('POST', '/publication/request-public') === 1, 'a double click sends one snapshot and one request-public');
  await wire.close();

  // Lost answer of the snapshot: the server committed it; the check replays the same command.
  await reopen(author);
  const before = await countRevisions(author, work);
  wire = new Wire(author);
  wire.drop = { method: 'POST', suffix: '/snapshots' };
  await wire.enable();
  await press(author);
  await waitPhase(author, 'uncertain', 'a lost snapshot answer');
  check(wire.dropped === 1 && (await countRevisions(author, work)) === before + 1 && (await panelText(author)).includes('Chưa xác nhận bản đã lưu được chụp'), 'the snapshot committed and the answer was lost: the region says it is unconfirmed, never done');
  await author.click('#publication-retry');
  await waitPhase(author, 'reconciled', 'the check settles it', 30000);
  const mid = await stateOf(author, work);
  check(wire.count('POST', '/snapshots') === 2 && mid.revisions.length === before + 1 && wire.count('POST', '/publication/request-public') === 1 && mid.intent.revision_id === mid.revisions.at(-1).revision_id, 'checking replays the same snapshot command: the server returns the first revision, no duplicate, and that revision is the one requested');
  await wire.close();

  // Lost answer of request-public: it committed; the check reads first and replays nothing.
  await reopen(author);
  const gen0 = (await stateOf(author, work)).generation;
  wire = new Wire(author);
  wire.drop = { method: 'POST', suffix: '/publication/request-public' };
  await wire.enable();
  await press(author);
  await waitPhase(author, 'uncertain', 'a lost request-public answer');
  const committed = await stateOf(author, work);
  check(committed.generation === gen0 + 1 && committed.intent.revision_id === committed.revisions.at(-1).revision_id && (await panelText(author)).includes('Chưa xác nhận yêu cầu công khai'), 'the request committed and the answer was lost: unconfirmed in the region');
  await matrix(author, 'uncertain', check);
  await author.click('#publication-retry');
  await waitPhase(author, 'reconciled', 'settled from the state', 30000);
  check(wire.count('POST', '/publication/request-public') === 1 && (await stateOf(author, work)).generation === gen0 + 1 && (await casesFor(mod, work, committed.intent.revision_id)) === 1, 'checking settled it from the state: no second request, one review case for that revision, generation moved once');
  await wire.close();

  // A request that never reached the server: the check reads, sees the pinned generation stands,
  // and replays the original key and payload.
  await reopen(author);
  const gen1 = (await stateOf(author, work)).generation;
  wire = new Wire(author);
  wire.failBefore = { method: 'POST', suffix: '/publication/request-public' };
  await wire.enable();
  await press(author);
  await waitPhase(author, 'uncertain', 'a request that never arrived');
  check((await stateOf(author, work)).generation === gen1, 'nothing reached the server: the generation did not move');
  await author.click('#publication-retry');
  await waitPhase(author, 'reconciled', 'replayed once', 30000);
  const replayed = await stateOf(author, work);
  check(wire.count('POST', '/publication/request-public') === 2 && replayed.generation === gen1 + 1 && replayed.intent.revision_id === replayed.revisions.at(-1).revision_id && (await casesFor(mod, work, replayed.intent.revision_id)) === 1, 'the check replayed the original command once: one effect, one review case for that revision');
  check(await author.eval(`document.querySelector('#publication-live').innerText.includes('Đã ghi nhận yêu cầu công khai')`), 'only then does the region confirm, from the state it read');
  await wire.close();
}

async function twoTabs({ author, mod, work, base, check, openPage, reopen, editorPath }) {
  console.log('  two tabs: a slow submission does not win over the other tab’s withdrawal:');
  await reopen(author);
  const other = await openPage();
  await other.goto(`${base}${editorPath}`);
  await other.waitFor(`${EDITOR_READY} && ${HYDRATED_EXPR} && document.querySelector('${PUBLICATION}')?.dataset.phase === 'ready'`, 'the second tab');
  const wire = new Wire(author);
  wire.hold = { method: 'POST', suffix: '/publication/request-public' };
  await wire.enable();
  const rev0 = await countRevisions(author, work);
  await press(author);
  await wire.heldRequest();
  const captured = (await stateOf(author, work)).revisions.at(-1).revision_id;
  check((await countRevisions(author, work)) === rev0 + 1, 'the first tab froze its snapshot and its request is on the way');
  await choose(other, 'private');
  await other.waitFor(`document.querySelector('#publication-primary')?.innerText === 'Chuyển riêng tư'`, 'the withdrawal action');
  await press(other);
  await waitPhase(other, 'reconciled', 'the withdrawal settles', 30000);
  const priv = await stateOf(author, work);
  check(priv.audience === 'private' && !priv.pointer, 'the other tab made the work private');
  await wire.release();
  await waitPhase(author, 'conflict', 'the slow request is refused');
  const after = await stateOf(author, work);
  check(after.audience === 'private' && !after.intent && after.generation === priv.generation, 'the slow request did not win: the work is still private, no intent, generation unchanged');
  check((await panelText(author)).includes('Trạng thái công khai đã đổi ở nơi khác') && (await panelText(author)).includes('bản đã chụp vẫn giữ nguyên'), 'the first tab names a generation conflict and keeps its frozen revision');
  await author.waitFor(`document.querySelector('#audience-applied')?.innerText.includes('Chỉ mình tôi')`, 'the first tab reads the new state');
  check(await author.eval(`Boolean(document.querySelector('#publication-resume'))`), 'sending the frozen revision again is an explicit choice');
  await matrix(author, 'conflict', check);
  await author.click('#publication-resume');
  await waitPhase(author, 'reconciled', 'sent again on purpose', 30000);
  const resumed = await stateOf(author, work);
  check(resumed.audience === 'public' && resumed.intent.revision_id === captured && resumed.revisions.length === rev0 + 1 && wire.count('POST', '/snapshots') === 1, 'it sent the same captured revision against the generation now shown; no new snapshot was taken');
  await matrix(author, 'resumed', check);
  await wire.close();
  await other.send('Target.closeTarget', { targetId: undefined }).catch(() => {});
}

async function withdrawal({ author, mod, work, base, check, reopen, editorPath }) {
  console.log('  withdrawal with unsaved edits, a lost answer, and what readers get afterwards:');
  const intent = await intentOf(author, work);
  await openCase(mod, base, work, intent);
  await conclude(mod, 'approved', 'Phù hợp (verify web)');
  await reopen(author);
  const title = (await stateOf(author, work)).revisions.find((r) => r.revision_id === intent).title;
  check((await panelText(author)).includes(`Đang công khai: «${title}».`), 'the approved revision is the one readers get');
  const before = await publicRead(base, work);
  check(before.status === 200 && before.etag, 'readers have an ETag for it');
  await author.typeInBody(' [chưa lưu]');
  await author.waitStatus('unsaved');
  await choose(author, 'private');
  await author.waitFor(`document.querySelector('#publication-primary')?.innerText === 'Chuyển riêng tư'`, 'the withdrawal action');
  const wire = new Wire(author);
  wire.drop = { method: 'POST', suffix: '/publication/set-private' };
  await wire.enable();
  const gen0 = (await stateOf(author, work)).generation;
  await press(author);
  await waitPhase(author, 'uncertain', 'a lost withdrawal answer');
  const priv = await stateOf(author, work);
  check(priv.audience === 'private' && priv.generation === gen0 + 1 && (await panelText(author)).includes('Chưa xác nhận việc chuyển riêng tư'), 'the withdrawal committed and the answer was lost: unconfirmed, not claimed');
  await author.click('#publication-retry');
  await waitPhase(author, 'reconciled', 'settled', 30000);
  check(wire.count('POST', '/publication/set-private') === 1 && (await stateOf(author, work)).generation === gen0 + 1, 'the check settled it from the state: no second withdrawal');
  await wire.close();
  check((await author.body()).endsWith('[chưa lưu]') && (await author.status()) === 'unsaved' && !storedText(await author.draft(work)).includes('[chưa lưu]'), 'the unsaved text is still in the editor, still unsaved, and was not saved by the withdrawal');
  const gone = await publicRead(base, work);
  check(gone.status === 404, 'GET …/publication is 404');
  const revalidated = await fetch(`${base}/api/v1/works/${work}/publication`, { headers: { 'If-None-Match': before.etag } });
  check(revalidated.status === 404, 'the old ETag is not answered 304: it is 404');
  const list = await (await fetch(`${base}/api/v1/works`)).json();
  check(!list.works.some((w) => w.work_id === work), 'Explore no longer lists the work');
  const reader = await (await fetch(`${base}/api/v1/works/${work}/reader`)).json();
  check(JSON.stringify(Object.keys(reader).sort()) === JSON.stringify(['status', 'work_id']) && reader.status === 'hidden', 'the reader read model is the hidden page: no title, body, tags or author');
  const html = await (await fetch(`${base}/tac-pham/${work}`)).text();
  check(html.includes('Nội dung đã được ẩn') && !html.includes(title) && !html.includes(LINE_1A), 'the server-rendered reader carries neither title nor body');
  await matrix(author, 'private', check);
}

// ---- invariants -----------------------------------------------------------------------------------

/** Region geometry in CSS px. Nothing here is a screenshot comparison. */
async function geometry(page) {
  return page.eval(`(() => {
    const de = document.documentElement;
    const root = document.querySelector('#publication');
    const probe = document.createElement('div');
    probe.style.cssText = 'position:absolute;visibility:hidden;width:0;height:var(--vk-size-hit-target)';
    document.body.append(probe);
    const hit = probe.getBoundingClientRect().height;
    probe.remove();
    const rr = root.getBoundingClientRect();
    const name = (el) => el.id || el.getAttribute('data-testid') || el.textContent.trim().slice(0, 28);
    const out = { hit, horizontal: de.scrollWidth - de.clientWidth, region: root.scrollWidth - root.clientWidth, outside: [], small: [], clipped: [], ellipsis: [] };
    for (const el of root.querySelectorAll('button, a, label.vk-ui-choice__target, summary')) {
      if (!el.getClientRects().length) continue;
      const r = el.getBoundingClientRect();
      if (r.left < rr.left - 1 || r.right > rr.right + 1) out.outside.push(name(el));
      if (r.height < hit - 1) out.small.push(name(el) + ':' + Math.round(r.height));
    }
    for (const el of root.querySelectorAll('p, h2, legend, .vk-ui-button__label, .vk-ui-notice *')) {
      if (!el.getClientRects().length || getComputedStyle(el).display === 'inline') continue;
      if (el.scrollHeight > el.clientHeight + 1 || el.scrollWidth > el.clientWidth + 1) out.clipped.push(name(el));
      if (getComputedStyle(el).textOverflow === 'ellipsis') out.ellipsis.push(name(el));
    }
    return out;
  })()`);
}

const SHOTS = [320, 390, 1024];

/** The documented matrix for one real state: 320/390/1024 CSS px × light/dark × text 100%/200%.
 * Each cell is measured (the invariants of the LayoutSpec) and captured; the PNGs are opened by a
 * person, they are not baselines. */
async function matrix(page, name, check) {
  const failures = [];
  const smallest = new Set();
  for (const theme of ['light', 'dark']) {
    for (const scale of [100, 200]) {
      for (const width of SHOTS) {
        await page.viewport(width, scale === 200 ? (width < 700 ? 4200 : 2600) : 1300);
        await page.eval(`document.documentElement.dataset.theme = ${JSON.stringify(theme)}; document.documentElement.style.fontSize = '${scale}%'; document.querySelector('#publication').scrollIntoView({ block: 'start' })`);
        await sleep(150);
        const geo = await geometry(page);
        const cell = `${width}px ${theme} ${scale}%`;
        if (geo.horizontal > 0 || geo.region > 0) failures.push(`${cell}: horizontal scroll (${geo.horizontal}/${geo.region})`);
        if (geo.outside.length) failures.push(`${cell}: outside ${geo.outside.join(',')}`);
        if (geo.clipped.length) failures.push(`${cell}: clipped ${geo.clipped.join(',')}`);
        if (geo.ellipsis.length) failures.push(`${cell}: truncated ${geo.ellipsis.join(',')}`);
        for (const item of geo.small) smallest.add(item);
        await page.capture(`publication-${name}-${width}-${theme}-${scale}`, { source: 'publication-author', state: name, cell });
      }
    }
  }
  await page.send('Emulation.clearDeviceMetricsOverride');
  await page.eval(`document.documentElement.removeAttribute('data-theme'); document.documentElement.style.fontSize = ''; scrollTo(0, 0)`);
  check(failures.length === 0, `publication region [${name}] holds the layout invariants in 12 cells (320/390/1024 × light/dark × 100%/200%)${failures.length ? `: ${failures.slice(0, 6).join(' | ')}` : ''}`);
  if (smallest.size) console.log(`  ..  [${name}] targets under the hit-target size (measured, px): ${[...smallest].join(', ')}`);
}

/** A refresh of the state updates the region in place: no remounted editor, no scroll, no focus. */
async function refreshInvariant(page, check) {
  await page.eval(`document.querySelector('.cm-editor').__mark = 'kept'; document.querySelector('.cm-content').focus(); scrollTo(0, 120)`);
  const before = await page.eval(`({ y: scrollY, active: document.activeElement?.className, save: document.querySelector('#save') !== null })`);
  const wire = new Wire(page);
  await wire.enable();
  const reads = wire.count('GET', '/publication/state');
  await page.eval(`window.dispatchEvent(new Event('focus'))`);
  await page.waitFor(`true`, 'tick');
  const deadline = Date.now() + 8000;
  while (wire.count('GET', '/publication/state') === reads && Date.now() < deadline) await sleep(50);
  await sleep(300);
  const after = await page.eval(`({ y: scrollY, active: document.activeElement?.className, mark: document.querySelector('.cm-editor')?.__mark, editors: document.querySelectorAll('.cm-editor').length })`);
  check(wire.count('GET', '/publication/state') > reads && after.mark === 'kept' && after.editors === 1 && after.y === before.y && after.active === before.active, 'returning to the window reads the state again without remounting the editor, moving the scroll or taking the focus');
  await wire.close();
  await page.eval(`scrollTo(0, 0)`);
}

// ---- AC-07: the session ---------------------------------------------------------------------------

async function sessions({ author, work, base, check, openPage, reopen, ctx }) {
  console.log('  the session: refusals, an answer that arrives after the session changed, another account:');
  await reopen(author);
  // Fabricated (contract-shaped; the request never reaches the App): a temporary outage and a
  // refusal of the state read. Every other answer in this scenario is real.
  const title = await author.eval(`document.querySelector('#draft-title').value`);
  let done = await fabricate(author, 'GET', `/api/v1/works/${work}/publication/state`, 503, { error: { code: 'dependency_unavailable', message: 'test fixture' } });
  await author.click('#publication-refresh');
  await done();
  await author.waitFor(`document.querySelector('[data-testid=publication-stale]')`, 'a temporary outage keeps the last facts');
  check((await panelText(author)).includes('đang giữ lần đọc gần nhất') && (await author.eval(`document.querySelector('#publication-summary').innerText.length > 0`)), '503 on the read (fabricated): the last facts stay and the region says they are the last read');
  done = await fabricate(author, 'GET', `/api/v1/works/${work}/publication/state`, 403, { error: { code: 'forbidden', message: 'test fixture' } });
  await author.click('#publication-refresh');
  await done();
  await author.waitFor(`document.querySelector('${PUBLICATION}')?.dataset.phase === 'unavailable'`, 'a refusal clears the facts');
  check((await panelText(author)).includes('Yêu cầu bị từ chối') && !(await panelText(author)).includes('Chưa công khai') && !(await panelText(author)).includes('Đang công khai'), '403 on the read (fabricated): the facts leave the page and the refusal is named');
  await author.click('#publication-reload');
  await waitPhase(author, 'ready', 'a real read restores it');

  // A real answer that arrives after the session ended in another tab.
  const other = await openPage();
  await other.goto(`${base}/toi`);
  await other.waitFor(`${HYDRATED_EXPR} && ${STANDING} === 'active'`, 'the same account in a second tab');
  await author.typeInBody(' [phiên]');
  await author.waitStatus('unsaved');
  const wire = new Wire(author);
  wire.holdResponse = { method: 'GET', suffix: '/publication/state' };
  await wire.enable();
  await author.click('#publication-refresh');
  await wire.heldAnswer();
  const out = await signOut(other, base);
  check(out.includes(204), 'the session ends for real (sign-out in the other tab, 204)');
  await author.waitFor(`document.querySelector('${PUBLICATION}')?.dataset.phase === 'unavailable'`, 'the first tab learns the session ended');
  await wire.releaseAnswer();
  await sleep(500);
  const late = await panelText(author);
  check(late.includes('Phiên đăng nhập đã hết') && !late.includes('Đang công khai') && !late.includes('Chưa công khai') && !late.includes(title), 'the late answer of the old session changes nothing: no facts, only the session notice');
  check((await author.body()).endsWith('[phiên]') && (await author.status()) === 'unsaved', 'the unsaved text stays on the page when the session ends');
  await wire.close();

  // Another account takes the browser: nothing of the author remains.
  const mail = `nguoi-khac-${Date.now().toString(36)}@vi-du.vn`;
  await openAccount(other, base, 'anonymous');
  await signIn(other, ctx, mail);
  await other.waitFor(`${STANDING} === 'needs_principal' && ${q('#pen-name')}`, 'pen name of the other account');
  await fill(other, '#pen-name', 'Người khác [Mẫu]');
  await other.click('#pen-name-save');
  await other.waitFor(`${STANDING} === 'active'`, 'the other account is active');
  await author.waitFor(`!document.querySelector('#publication') || document.querySelector('#publication')?.dataset.phase !== 'unavailable'`, 'the first tab rebuilds for the new session');
  await sleep(800);
  const gone = await author.eval(`document.body.innerText`);
  check(!gone.includes(title) && !gone.includes('[phiên]') && !gone.includes('Đang công khai') && !gone.includes(LINE_1A), 'another account in the browser: none of the author’s text, title or publication facts remain on the page');
  const foreign = await call(other, `/api/v1/works/${work}/publication/state`);
  check(foreign.status === 404 && foreign.cache === 'no-store', 'the other account asking for the author’s state gets 404 no-store');
  const html = await (await fetch(`${base}/ban-viet/${work}`)).text();
  check(!html.includes(title) && !html.includes('Đang công khai') && !html.includes('audience'), 'the server-rendered editor carries no publication state or private text');
}
