// Chrome probes live only in the disposable test document. Measure the sample's first text
// frame and the timer handles in the duration/backoff bands without shipping instrumentation.
const sample = '33333333-3333-4333-8333-333333333333';
const path = '/tac-pham/' + sample;
const body = "document.querySelector('.vk-document')?.textContent.includes('Phố cũ mưa giăng sợi mỏng,')";
const link = `a[href^="${path}"]`;
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));

function installProbe() {
  const pending = new Map();
  window.workViewArms = [];
  const start = window.setTimeout.bind(window);
  const clear = window.clearTimeout.bind(window);
  window.setTimeout = (fn, ms, ...args) => {
    if (typeof fn !== 'function' || !(ms >= 4900 && ms <= 5000 || [1000, 3000, 9000].includes(ms))) {
      return start(fn, ms, ...args);
    }
    if (ms === 5000) window.workViewArms.push(Boolean(document.querySelector('.vk-document')?.textContent));
    const id = start(function (...values) {
      pending.delete(id);
      return fn.apply(this, values);
    }, ms, ...args);
    pending.set(id, ms);
    return id;
  };
  window.clearTimeout = id => {
    pending.delete(id);
    clear(id);
  };
  window.workViewTimers = () => [...pending.values()];
  const observer = new MutationObserver(() => {
    if (document.querySelector('.vk-document')?.textContent.includes('Phố cũ mưa giăng sợi mỏng,')) {
      observer.disconnect();
      requestAnimationFrame(() => { window.workViewTextMs = performance.now(); });
    }
  });
  observer.observe(document, {childList:true, subtree:true});
}

export async function observeVisit(page) {
  return page.send('Page.addScriptToEvaluateOnNewDocument', {source:`(${installProbe})()`});
}

async function visitResources(page) {
  const { result } = await page.send('Runtime.evaluate', {
    expression: `({timers: workViewTimers(), activation: (getEventListeners(document).prerenderingchange || []).length,
      pagehide: (getEventListeners(window).pagehide || []).length})`,
    includeCommandLineAPI: true, returnByValue: true,
  });
  return result.value;
}

export async function tap(page, selector) {
  const point = await page.eval(`(() => {
    const el = document.querySelector(${JSON.stringify(selector)});
    el.scrollIntoView({block:'center'});
    const r = el.getBoundingClientRect();
    return {x:r.x + r.width / 2, y:r.y + r.height / 2};
  })()`);
  await page.send('Emulation.setTouchEmulationEnabled', {enabled:true});
  try {
    await page.send('Input.dispatchTouchEvent', {type:'touchStart', touchPoints:[point]});
    await page.send('Input.dispatchTouchEvent', {type:'touchEnd', touchPoints:[]});
  } finally {
    await page.send('Emulation.setTouchEmulationEnabled', {enabled:false});
  }
}

// Reuse the already-counted guest's cookie jar, as when a reader opens a shared URL in a
// second tab. Each eligible open must submit a new candidate, still inside the same 24h window.
export async function navigationChecks(page, {base, ledger, check, readCount, expected}) {
  await observeVisit(page);
  const settled = async (n, label) => {
    const end = Date.now() + 10000;
    while (ledger.rows[n-1]?.status !== 204 && Date.now() < end) await sleep(100);
    check(ledger.rows.length === n && ledger.rows[n-1]?.status === 204, label + ' sends one candidate');
  };
  await page.goto(base + path);
  await page.waitFor(`window.workViewTextMs && ${body}`, 'first frame of the shared publication');
  await settled(1, 'shared URL in a new tab');
  const timing = await page.eval(`({
    textMs:Math.round(workViewTextMs),
    api:performance.getEntriesByType('resource').filter(r=>new URL(r.name).pathname.startsWith('/api/'))
      .map(r=>[new URL(r.name).pathname.replace(/\\/api\\/v1\\//, ''), Math.round(r.startTime), Math.round(r.responseEnd)])
  })`);
  check(timing.api.filter(([p])=>p.endsWith('/views')).length === 1 &&
    timing.api.find(([p])=>p.endsWith('/views'))[1] > timing.textMs, 'intake starts after the first Reader text frame');
  await page.reload();
  await page.waitFor(body, 'Reader after document reload');
  await settled(2, 'reload');
  await page.click('#reader-return');
  await page.waitFor(`document.querySelector('${link}')`, 'Explore link');
  const idle = await visitResources(page);
  const cycles = [];
  for (const method of ['keyboard', 'touch', 'mouse']) {
    if (method === 'keyboard') {
      await page.eval(`document.querySelector('${link}').focus()`);
      await page.key('Enter');
    } else if (method === 'touch') {
      await tap(page, link);
    } else {
      await page.click(link);
    }
    await page.waitFor(body, method + ' Reader');
    const mounted = await visitResources(page);
    if (method === 'keyboard') await settled(3, 'Enter');
    if (method === 'touch') await settled(4, 'CDP touch');
    await page.eval('history.back()');
    await page.waitFor(`document.querySelector('${link}')`, 'Back to Explore');
    const left = await visitResources(page);
    cycles.push({mounted, left});
    check(mounted.activation === idle.activation + 1 && mounted.pagehide === idle.pagehide + 1 &&
      mounted.timers.filter(ms=>ms >= 4900).length === 1 &&
      left.activation === idle.activation && left.pagehide === idle.pagehide &&
      !left.timers.some(ms=>ms >= 4900), method + ' mounts one countdown and releases its listeners and timer on Back');
  }
  await page.eval('history.forward()');
  await page.waitFor(body, 'Forward to Reader');
  await settled(5, 'Forward after short mouse visit');
  const ids = ledger.rows.map(row=>JSON.parse(row.postData).event_id);
  check(new Set(ids).size === 5 && (await readCount()) === expected,
    'independent navigation candidates keep the original 24h total');
  console.log('WORK_VIEW_NAV ' + JSON.stringify({timing, idle, cycles, posts:ids.length}));
}
