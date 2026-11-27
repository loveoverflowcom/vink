import { randomUUID } from 'node:crypto';
import { standaloneFixtureSql as sql } from './author-fixtures.mjs';
import { captureRequest, holdAnswer } from './auth-browser-network.mjs';
import { observeVisit } from './work-view-navigation.mjs';
import { wakeDesk } from './desk-lifecycle.mjs';

const sample = '33333333-3333-4333-8333-333333333333';

// Clone the scratch publication for a real native account. No view, session or response
// is seeded. The native author and reader exercise the same API-only host via the CSR proxy.
function seedPublishedWork(principal) {
  if (!/^[0-9a-f-]{36}$/.test(principal)) throw new Error('expected a scratch principal UUID');
  const work = randomUUID();
  const rev = randomUUID();
  sql(`BEGIN;
    INSERT INTO works(id,owner_id) VALUES ('${work}','${principal}');
    INSERT INTO working_drafts(work_id,version,title,document)
      SELECT '${work}',1,title,document FROM working_drafts WHERE work_id='${sample}';
    INSERT INTO revisions(id,work_id,title,document,content_hash,created_by)
      SELECT '${rev}','${work}',title,document,content_hash,'${principal}'
      FROM revisions WHERE work_id='${sample}';
    INSERT INTO work_revision_moderation(work_id,revision_id,decision_version,moderation,provenance_kind)
      VALUES ('${work}','${rev}',2,'approved','legacy_bootstrap');
    INSERT INTO work_publications(work_id,audience,lifecycle,generation,intent_revision_id,intent_generation,
        pointer_revision_id,pointer_intent_generation,ever_public,public_epoch)
      VALUES ('${work}','public','active',2,'${rev}',2,'${rev}',2,ARRAY['${rev}']::uuid[],1);
    INSERT INTO publications(work_id,revision_id,generation,published_at) VALUES ('${work}','${rev}',2,now());
    COMMIT;`);
  return work;
}

export async function nativeAuthorMetrics(owner, reader, {base, check}) {
  const principal = await owner.eval("fetch('/api/v1/auth/session').then(r=>r.json()).then(s=>s.principal_id)");
  const work = seedPublishedWork(principal);
  const readList = page => page.eval(`fetch('/api/v1/author/works').then(async r => ({
    status:r.status, cache:r.headers.get('cache-control'), card:(await r.json()).works?.find(w=>w.work_id==='${work}')
  }))`);
  const row = `#draft-row-${work}`;
  const metric = `document.querySelector('${row} [data-testid=author-work-views]')`;
  const api = `/api/v1/works/${work}`;
  const etag = (await fetch(base + api + '/reader')).headers.get('etag');
  const denied = await fetch(base + '/api/v1/author/works');
  const other = await readList(reader);
  check(denied.status === 401 && other.status === 200 && !other.card,
    'guest and other native account cannot read the author’s metric');
  const ack = await holdAnswer(reader, 'POST', api + '/views');
  const { identifier } = await observeVisit(reader);
  await reader.goto(base + '/tac-pham/' + work);
  await reader.send('Page.removeScriptToEvaluateOnNewDocument', {identifier});
  await reader.waitFor("Boolean(document.querySelector('.vk-document'))", 'member text before intake');
  check(await ack.held() === 204, 'member intake committed before delivery of its ACK');
  const timing = await reader.eval(`({textMs:Math.round(workViewTextMs), mounted:workViewArms,
    api:performance.getEntriesByType('resource').filter(r=>new URL(r.name).pathname.startsWith('/api/'))
      .map(r=>[new URL(r.name).pathname, Math.round(r.startTime), Math.round(r.responseEnd)])})`);
  console.log('WORK_VIEW_CSR ' + JSON.stringify(timing));
  check(timing.textMs > 0 && timing.mounted.length > 0 && timing.mounted.every(Boolean),
    'CSR arms the foreground countdown only after the public article exists');
  const first = await readList(owner);
  check(first.cache === 'no-store' && first.card?.views.count === 1 &&
    sql(`SELECT views FROM work_view_totals WHERE work_id='${work}'`) === '1',
    'PostgreSQL and the native author API independently read one while ACK is held');
  await ack.finish(true, true);
  // One distinct receipt after replay also excludes a fresh event skipped by the 24h window.
  await reader.waitFor(`performance.getEntriesByType('resource').filter(e=>new URL(e.name).pathname==='${api}/views').length===2`,
    'member retry settled after lost ACK');
  check(sql(`SELECT views, (SELECT count(*) FROM work_view_receipts WHERE work_id='${work}')
    FROM work_view_totals WHERE work_id='${work}'`) === '1|1', 'member replay leaves one count and one receipt');
  const conditional = await fetch(base + api + '/reader', {headers:{'If-None-Match':etag}});
  check(Boolean(etag) && conditional.status === 304, 'public Reader validator survives the private write');

  await owner.goto(base + '/ban-viet');
  await owner.waitFor(`${metric}?.title.startsWith('1 lượt xem')`, 'native author reads the committed total on Desk');
  const hidden = await owner.eval(`fetch('${api}/publication/set-private', {
    method:'POST', headers:{'Content-Type':'application/json'},
    body:JSON.stringify({command_key:crypto.randomUUID(), expected_generation:2})
  }).then(r=>r.status)`);
  check(hidden === 200, 'native author withdraws the publication');
  await owner.reload();
  await owner.waitFor(`${metric}?.title.startsWith('1 lượt xem') && ${metric}.classList.contains('vk-author-work__views--historical')`,
    'old count remains on the native author’s private Desk');
  // Two WorkIds through the same router instance. Use a test link because this Reader
  // has no product sibling-work link. The destination's real projection is now Hidden.
  await reader.send('Page.bringToFront');
  const oldVisit = captureRequest(reader, 'POST', `/api/v1/works/${sample}/views`);
  const hiddenVisit = captureRequest(reader, 'POST', api + '/views');
  try {
    await reader.goto(base + '/tac-pham/' + sample);
    await reader.waitFor("Boolean(document.querySelector('.vk-document'))", 'first WorkId body');
    const documentStart = await reader.eval('performance.timeOrigin');
    await reader.eval(`(() => {
      const link = document.createElement('a');
      link.href = '/tac-pham/${work}';
      document.body.append(link);
      link.click();
      link.remove();
    })()`);
    await reader.waitFor("Boolean(document.querySelector('[data-testid=reader-hidden]'))", 'other WorkId replaces article');
    await new Promise(resolve=>setTimeout(resolve, 5300));
    check(await reader.eval('performance.timeOrigin') === documentStart &&
      !oldVisit.count() && !hiddenVisit.count(), 'WorkId switch cancels the old timer and the hidden work emits no request');
  } finally {
    oldVisit.close();
    hiddenVisit.close();
  }
  const late = await holdAnswer(owner, 'GET', '/api/v1/author/works');
  await wakeDesk(owner);
  check(await late.held() === 200, 'hold the old author’s successful list refresh');
  // Deliver the real reader cookie as a sibling-tab login would. The in-app channel forces
  // a fresh authority read while the original successful list response is still in flight.
  const { cookies } = await reader.send('Network.getCookies', {urls:[base]});
  const session = cookies.find(c=>c.name==='__Host-vink_session');
  await owner.send('Network.setCookie', {name:session.name, value:session.value, url:base, path:'/', secure:true, httpOnly:true});
  await owner.eval(`(() => {
    const original = window.fetch;
    window.fetch = async (...args) => {
      const result = await original.apply(window, args);
      if (new URL(result.url).pathname === '/api/v1/auth/session') {
        window.fetch = original;
        await new Promise(resolve => { window.releaseViewSession = resolve; });
      }
      return result;
    };
    const channel = new BroadcastChannel('vink-session');
    channel.postMessage('changed');
    channel.close();
  })()`);
  await owner.waitFor("typeof window.releaseViewSession === 'function'", 'real replacement session answer held');
  check(!await owner.eval(`Boolean(document.querySelector('${row}'))`),
    'cookie-change signal hides the previous author’s row while the session check is pending');
  await late.finish(false, true);
  check(!await owner.eval(`Boolean(document.querySelector('${row}'))`),
    'late successful refresh cannot restore the previous scope');
  await owner.eval('window.releaseViewSession()');
  await owner.waitFor(`!document.querySelector('${row}') && document.querySelector('[data-testid=draft-empty]')`,
    'confirmed reader sees its own empty Desk');
  check(!(await readList(owner)).card, 'other account still cannot read the author metric');
}
