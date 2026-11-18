// Loopback-only controls for the guest journey. Keep navigation intent, owner and wire counts separate.
import { countTransport } from './web-request-ledger.mjs';

export const readOwnerCounts = page => page.eval(
  "import('/pkg/vink.js').then(m => JSON.parse(m.public_read_counts()))",
);

const clockAdvance = page => page.eval(`(() => {
  const real = performance.now.bind(performance);
  performance.now = () => real() + 121000;
  for (const event of ['focus','online','visibilitychange','pageshow'])
    (event === 'visibilitychange' ? document : window).dispatchEvent(new Event(event));
})()`);

const metrics = async page => {
  const { metrics } = await page.send('Performance.getMetrics');
  return Object.fromEntries(metrics.filter(m =>
    ['LayoutDuration', 'RecalcStyleDuration', 'ScriptDuration', 'TaskDuration'].includes(m.name),
  ).map(m => [m.name, m.value * 1000]));
};

export async function measureUncachedReader(page, { origin, query, secondary, check, phase, results }) {
  const samples = results.uncached_reader = [];
  await page.send('Performance.enable');
  try {
    for (let i = 0; i < 10; i++) {
      phase.current = 'uncached-prepare-' + i;
      await page.goto('about:blank');
      await page.send('Network.clearBrowserCache');
      await page.goto(origin + query);
      await page.waitFor("document.documentElement.dataset.hydrated==='true' && document.querySelector('.vk-public-row__link')", 'fresh scope');
      let paused;
      const listener = event => {
        if (event.sessionId === page.sessionId && event.method === 'Fetch.requestPaused') paused = event.params.requestId;
      };
      page.cdp.listeners.push(listener);
      try {
        // Hold the real secondary response. The critical Reader request goes directly to App.
        await page.send('Fetch.enable', { patterns: [{ urlPattern: origin + secondary + '*', requestStage: 'Response' }] });
        phase.current = 'uncached-reader-' + i;
        results.intended_reads.push({ phase: phase.current, reason: 'first_client_open', representation: 'reader' });
        await page.click('.vk-public-row__link');
        await page.waitFor("!!document.querySelector('.vk-reader .vk-document')", 'article before secondary');
        const deadline = Date.now() + 10000;
        while (!paused && Date.now() < deadline) await new Promise(resolve => setTimeout(resolve, 20));
        check(!!paused && await page.eval("!document.querySelector('.vk-discussion-comment')"), 'held actual discussion before mount');
        const before = await metrics(page);
        await page.send('Fetch.continueResponse', { requestId: paused });
        await page.waitFor("document.querySelectorAll('.vk-discussion-comment').length===40", 'first public page');
        await page.eval('new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)))');
        const after = await metrics(page);
        const delta = Object.fromEntries(Object.entries(after).map(([k,v]) => [k, v - before[k]]));
        const timing = await page.eval(`(() => {
          const entry = performance.getEntriesByType('resource').find(r=>new URL(r.name).pathname.endsWith('/reader'));
          const p = window.__vinkPerfBaseline;
          return { click_to_article_ms:p.article_at-p.clicked_at, reader_duration_ms:entry?.duration,
            reader_transfer_bytes:entry?.transferSize, reader_decoded_bytes:entry?.decodedBodySize,
            resource_ended_before_article:entry?.responseEnd <= p.article_at };
        })()`);
        check(timing.click_to_article_ms > 0 && timing.reader_decoded_bytes > 0 && timing.resource_ended_before_article,
          'first client Reader has a timed public representation before main text');
        samples.push({ iteration:i, ...timing, secondary_window_ms:delta, owner:await readOwnerCounts(page) });
      } finally {
        await page.send('Fetch.disable');
        page.cdp.listeners = page.cdp.listeners.filter(item => item !== listener);
      }
    }
  } finally { await page.send('Performance.disable'); }
}

export async function measureReaderLifecycle(page, { origin, work, results, phase, check }) {
  const reader = '/tac-pham/' + work;
  const api = '/api/v1/works/' + work + '/reader';
  const settled = "document.querySelector('#reader-freshness')?.dataset.refreshing==='false'";
  const article = "!!document.querySelector('.vk-reader .vk-document')";
  const note = (label, reason) => {
    phase.current = label;
    results.intended_reads.push({phase:label, reason, representation:'reader'});
  };
  const counts = results.controls.reader_owner = {};
  note('reader-history-prime', 'real HTTP cache primed by a client read');
  // The preceding sample has a fresh HTTP response and a mounted article.
  await page.waitFor(settled, 'Reader settled before signals');
  counts.initial = await readOwnerCounts(page);
  note('reader-stale-burst', 'clock +121s and four lifecycle hints');
  await clockAdvance(page);
  const waitWire = async label => {
    const deadline = Date.now() + 10000;
    while (!results.transport.some(r => r.phase === label && r.path === api && r.wire_status === 304 && r.response_status === 200 && r.finished_s != null)
        && Date.now() < deadline) await new Promise(resolve => setTimeout(resolve, 25));
    const rows = results.transport.filter(r => r.phase === label && r.path === api);
    check(rows.length === 1 && rows[0].validator_sent && rows[0].wire_status === 304 && rows[0].response_status === 200,
      label + ' one validator, wire 304, delivered 200');
  };
  await waitWire(phase.current);
  await page.waitFor(settled, 'burst settled');
  counts.after_burst = await readOwnerCounts(page);
  check(counts.after_burst.fetch_starts === counts.initial.fetch_starts + 1, 'burst starts one coordinator flight');

  note('reader-offline', 'CDP offline and stale signal');
  await page.send('Network.emulateNetworkConditions', {offline:true,latency:0,downloadThroughput:-1,uploadThroughput:-1});
  try {
    await clockAdvance(page);
    await page.waitFor("!!document.querySelector('[data-testid=reader-refresh-failed]')", 'offline failure reported');
    check(await page.eval(article), 'temporary failure keeps current public text');
  } finally {
    await page.send('Network.emulateNetworkConditions', {offline:false,latency:0,downloadThroughput:-1,uploadThroughput:-1});
  }
  note('reader-manual', 'manual retry after offline');
  await page.click('[data-testid=reader-refresh-failed] button');
  await waitWire(phase.current);
  await page.waitFor(settled, 'manual confirmation settled');

  // A previously successful, slow revalidation must not restore the body after disposal
  // and a newer withdrawal. Release an actual 200 response after the newer scope settles.
  phase.current = 'reader-prime-before-disposal';
  await page.goto('about:blank');
  await page.goto(origin + '/toi');
  await page.waitFor("document.documentElement.dataset.hydrated==='true'", 'new tab owner');
  const go = path => page.eval(`(() => { const a=document.createElement('a'); a.href=${JSON.stringify(path)};
    document.body.append(a); a.click(); a.remove(); })()`);
  const attempts = [];
  const onPause = event => {
    if (event.sessionId === page.sessionId && event.method === 'Fetch.requestPaused') attempts.push(event.params);
  };
  const waitAttempt = async n => {
    const deadline = Date.now()+10000;
    while (attempts.length<n && Date.now()<deadline) await new Promise(resolve=>setTimeout(resolve,20));
    check(attempts.length===n, 'observed Reader response ' + n);
    return attempts[n-1];
  };
  page.cdp.listeners.push(onPause);
  try {
    await page.send('Network.clearBrowserCache');
    await page.send('Fetch.enable', {patterns:[{urlPattern:origin+api, requestStage:'Response'}]});
    note('reader-dispose-A', 'hold real Published A');
    await go(reader);
    const old = await waitAttempt(1);
    check(old.responseStatusCode===200, 'held real Published 200');
    await go('/toi');
    await page.waitFor("location.pathname==='/toi' && !document.querySelector('.vk-reader')", 'Reader disposed');
    counts.disposed = await readOwnerCounts(page);
    note('reader-reopen-A', 'substitute Hidden on the new read');
    await go(reader);
    const fresh = await waitAttempt(2);
    await page.send('Fetch.fulfillRequest', {requestId:fresh.requestId, responseCode:200,
      responseHeaders:[{name:'Content-Type',value:'application/json'},{name:'Cache-Control',value:'no-store'}],
      body:Buffer.from(JSON.stringify({status:'hidden',work_id:work})).toString('base64')});
    const hidden = "!!document.querySelector('[data-testid=reader-hidden]') && !document.querySelector('.vk-document,link[rel=canonical]')";
    await page.waitFor(hidden, 'new read removes article and canonical');
    const completed = (await readOwnerCounts(page)).fetch_finishes;
    await page.send('Fetch.continueResponse', {requestId:old.requestId});
    const deadline = Date.now()+10000;
    while ((await readOwnerCounts(page)).fetch_finishes === completed && Date.now()<deadline) await new Promise(r=>setTimeout(r,20));
    check((await readOwnerCounts(page)).fetch_finishes > completed && await page.eval(hidden),
      'processing the obsolete reply preserves the new Hidden state');
    check(countTransport(results.transport,'reader-reopen-A','reader')===1, 'reopen starts a distinct Reader flight');
    counts.after_late = await readOwnerCounts(page);
    results.controls.late_publication = { held_real_status:200, injected_new_status:'hidden', processed:true };
  } finally {
    await page.send('Fetch.disable');
    page.cdp.listeners = page.cdp.listeners.filter(item=>item!==onPause);
  }
}
