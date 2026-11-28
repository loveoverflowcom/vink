// Real author session, provider-only mock. The browser, App, Identity, worker, relay
// and receiver use the same scratch deployment throughout this journey.
import assert from 'node:assert/strict';
import { spawn, execFileSync } from 'node:child_process';
import { once } from 'node:events';
import { openSync, closeSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { enter, newDraft, pickFormTag } from './publication-author-smoke.mjs';

async function get(page, path) {
  return page.eval(`fetch(${JSON.stringify(path)}).then(async r => ({status:r.status, cache:r.headers.get('cache-control'), body:await r.json()}))`);
}

async function eventually(read, label) {
  const deadline = Date.now() + 45000;
  do {
    const value = await read();
    if (value) return value;
    await delay(250);
  } while (Date.now() < deadline);
  throw new Error(`timed out waiting for ${label}`);
}

function sql(connection, query) {
  const url = new URL(connection);
  const env = { PATH: process.env.PATH, PGHOST:url.hostname, PGPORT:url.port,
    PGDATABASE:url.pathname.slice(1), PGUSER:decodeURIComponent(url.username), PGPASSWORD:decodeURIComponent(url.password) };
  return execFileSync('psql', ['-XAt', '-v', 'ON_ERROR_STOP=1', '-c', query], { env, encoding:'utf8' }).trim();
}

function uuid(value) {
  assert.match(value, /^[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}$/);
  return value;
}

async function stop(child) {
  if (!child || child.exitCode !== null || child.signalCode !== null) return;
  const end = once(child, 'exit');
  child.kill('SIGTERM');
  const timer = setTimeout(() => child.kill('SIGKILL'), 8000);
  try { await end; } finally { clearTimeout(timer); }
}

async function reflow(page, message) {
  for (const theme of ['light','dark']) for (const width of [320,390,834,1440]) for (const scale of [100,150,200]) {
    await page.viewport(width, 900);
    await page.eval(`document.documentElement.dataset.theme='${theme}'; document.documentElement.style.fontSize='${scale}%'`);
    const geometry = await page.eval(`new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(() => {
      const text = document.querySelector('.vk-notif-decision__reason p');
      const sheet = document.querySelector('#desk-review-content');
      resolve({ message:text?.innerText, clipped:text?.scrollWidth > text?.clientWidth+1,
        overflow:sheet?.scrollWidth > sheet?.clientWidth+1 });
    })))`);
    assert.deepEqual(geometry, {message, clipped:false, overflow:false}, `${width}/${theme}/${scale}`);
  }
  await page.send('Emulation.clearDeviceMetricsOverride');
  await page.eval("delete document.documentElement.dataset.theme; document.documentElement.style.fontSize=''");
}

export async function evaluatorMessage(page, args, openPage) {
  const {base, state} = args;
  const ctx = {args, base, seen:new Set()};
  const appDb = process.env.DATABASE_URL;
  const receiverDb = process.env.VINK_E2E_RECEIVER_DB;
  const sockets = process.env.VINK_E2E_SOCKET_DIR;
  const uid = String(process.getuid());
  const ingest = join(sockets, 'ingest.sock');
  const common = { PATH:process.env.PATH, VINK_LOG_FORMAT:'text', VINK_PROFILE:'local' };
  const start = (bin, argv, env, label) => {
    const fd = openSync(join(dirname(state), label+'.log'), 'a', 0o600);
    try {
      const child = spawn(bin, argv, {env:{...common,...env}, stdio:['ignore',fd,fd]});
      console.log(`  ${label} pid=${child.pid}`);
      return child;
    } finally { closeSync(fd); }
  };
  const startReceiver = () => start(process.env.VINK_E2E_RECEIVER_BIN, ['serve'], {
    VINK_NOTIFICATIONS_DATABASE_URL:receiverDb, VINK_NOTIFICATIONS_INGEST_SOCKET:ingest,
    VINK_NOTIFICATIONS_INBOX_SOCKET:join(sockets,'inbox.sock'), VINK_NOTIFICATIONS_RELAY_UID:uid,
    VINK_NOTIFICATIONS_GATEWAY_UID:uid }, 'receiver');
  const startWorker = reject => start(process.env.VINK_E2E_WORKER_BIN, [], {
    DATABASE_URL:appDb, VINK_NOTIFICATIONS_INGEST_SOCKET:ingest, VINK_NOTIFICATIONS_SERVER_UID:uid,
    ...(reject ? {VINK_EVALUATOR:'mock', VINK_MOCK_EVALUATION_SCENARIO:'reject'} : {}) }, 'worker');
  let receiver = startReceiver();
  let worker;
  try {
    await enter(page, ctx, `evaluator-${Date.now()}@vi-du.vn`, 'Tác giả thử nghiệm');
    const context = await page.cdp.send('Target.createBrowserContext', {disposeOnDetach:true});
    const other = await openPage(context.browserContextId);
    await enter(other, ctx, `autre-${Date.now()}@vi-du.vn`, 'Bạn đọc thử nghiệm');
    await page.send('Page.bringToFront');
    for (const rejected of [false, true]) {
      const outcome = rejected ? 'rejected' : 'approved';
      const title = `[Mẫu] Quyết định ${outcome}`;
      const body = `Nội dung riêng của bản gửi ${outcome}.`;
      const work = uuid(await newDraft(page, base, title, body));
      await pickFormTag(page);
      await page.waitFor("document.querySelector('#publication')?.dataset.phase === 'ready'", 'private draft state');
      await page.click('label:has(#audience-public)');
      await page.waitFor("document.querySelector('#publication-primary') && !document.querySelector('#publication-primary').disabled", 'submit available');
      await page.click('#publication-primary');
      await page.waitFor("document.querySelector('#publication')?.dataset.phase === 'reconciled'", 'submitted in Chrome', 30000);
      const statePath = `/api/v1/works/${work}/publication/state`;
      const pending = (await get(page, statePath)).body;
      const revision = uuid(pending.intent.revision_id);
      assert.equal(pending.revisions[0].moderation,'pending');
      assert.ok(!pending.pointer);
      await page.capture(`evaluator-${outcome}-before`);
      worker = startWorker(rejected);
      const current = await eventually(async () => {
        const res = await get(page, statePath);
        return res.body.revisions?.[0]?.moderation === outcome && res.body;
      }, outcome);
      const row = current.revisions[0];
      const version = row.decision_version;
      assert.ok(Number.isSafeInteger(version) && version > 0);
      const message = sql(appDb, `SELECT h.evaluator_message FROM work_revision_decision_history h
        JOIN work_evaluation_jobs j ON j.id=h.evaluation_id AND j.revision_id=h.revision_id AND j.work_id=h.work_id
        WHERE h.work_id='${work}' AND h.revision_id='${revision}' AND h.decision_version=${version} AND j.state='applied'`);
      assert.match(message, /thử nghiệm/u);
      assert.equal(row.reason_message, message);
      assert.equal(row.decision_reason, message);
      assert.equal(row.reason_code ?? null, rejected ? 'safety_policy' : null);
      assert.equal(current.pointer?.revision_id ?? null, rejected ? null : revision);
      const event = uuid(await eventually(() => sql(appDb, `SELECT e.id FROM notification_events e
        JOIN notification_deliveries d ON d.event_id=e.id WHERE e.event_type='work.moderation.decided'
        AND e.envelope::jsonb->'target'->>'revision_id'='${revision}' AND d.state='succeeded'`), 'relay acknowledgement'));
      const envelope = sql(appDb, `SELECT envelope FROM notification_events WHERE id='${event}'`);
      assert.ok(!envelope.includes(message) && !envelope.includes(body));
      const receipt = () => sql(receiverDb, `SELECT count(*) FROM ingestion_receipts WHERE event_id='${event}' AND effect='signal'`);
      assert.equal(receipt(),'1');
      const detailPath = `/api/v1/me/notifications/decisions/works/${work}/revisions/${revision}/${version}`;
      for (const path of [statePath, detailPath, `/api/v1/works/${work}/publication/review`]) {
        const hidden = await get(other,path);
        assert.equal(hidden.status,404);
        assert.equal(hidden.cache,'no-store');
        assert.ok(!JSON.stringify(hidden.body).includes(message));
      }
      const exact = await get(page, detailPath);
      assert.equal(exact.cache,'no-store');
      assert.equal(exact.body.reason.message,message);
      assert.equal(exact.body.reason.code,row.reason_code ?? null);
      const desk = (await get(page, `/api/v1/works/${work}/publication/review`)).body;
      assert.equal(desk.decisions.find(d => d.revision_id===revision && d.decision_version===version)?.reason_message,message);
      await page.goto(`${base}/ban-viet?review=${work}`);
      await page.waitFor(`document.querySelector('#desk-review-content')?.innerText.includes(${JSON.stringify(message)})`, 'private Desk message');
      await page.capture(`evaluator-${outcome}-desk`);
      await page.goto(`${base}/thong-bao`);
      const link = `main .vk-notif-row__link[href*="review=${work}"][href*="revision=${revision}"][href*="decision=${version}"]`;
      await page.waitFor(`document.querySelector(${JSON.stringify(link)})`, 'real receiver row in inbox');
      assert.ok(!(await page.eval('document.querySelector("main").innerText')).includes(message));
      await page.click(link);
      await page.waitFor(`document.querySelector('.vk-notif-decision__reason p')?.innerText === ${JSON.stringify(message)}`, 'exact decision message');
      await page.capture(`evaluator-${outcome}-detail`);
      await reflow(page,message);
      const pub = await fetch(`${base}/api/v1/works/${work}/publication`);
      assert.equal(pub.status,rejected ? 404 : 200);
      if (!rejected) {
        assert.equal((await pub.json()).revision_id,revision);
        await page.goto(`${base}/tac-pham/${work}`);
        await page.waitFor(`document.querySelector('main')?.innerText.includes(${JSON.stringify(body)})`, 'public Reader reads approved text');
        assert.ok(!(await page.eval('document.body.innerText')).includes(message));
      }
      await stop(worker); worker = undefined;
      await stop(receiver); receiver = startReceiver();
      await eventually(async () => (await get(page, '/api/v1/me/notifications?limit=20')).body.items?.some(i => i.target?.revision_id === revision), 'inbox after receiver restart');
      assert.equal((await get(page, detailPath)).body.reason.message,message);
      assert.equal(receipt(),'1');
      assert.equal(sql(receiverDb,`SELECT count(*) FROM notification_groups WHERE work_id='${work}'`),'1');
      console.log(JSON.stringify({work, revision, version, outcome, event, message, receiptCount:1, groups:1, matrix:24}));
    }
  } finally {
    await stop(worker);
    await stop(receiver);
  }
}
