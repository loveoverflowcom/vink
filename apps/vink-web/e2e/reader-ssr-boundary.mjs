// #235: progressive enhancement on the real App/PG fixture through the existing CDP Page.
// The only injected transport is the bootstrap asset hold; publication authority stays real.
import { writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { execFileSync } from 'node:child_process';

const WORK = '33333333-3333-4333-8333-333333333333';
const PATH = '/tac-pham/' + WORK;
const TEXT = 'Phố cũ mưa giăng sợi mỏng,';
const READY = "document.documentElement.dataset.hydrated==='true'";
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));

export async function readerBeforeHydration(page, { args, check }) {
  console.log('#235 Reader before hydration (real App/PG, blocked or held bootstrap assets):');
  const raw = { source_sha: execFileSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' }).trim(),
    dirty: execFileSync('git', ['status', '--porcelain'], { encoding: 'utf8' }).trim(),
    boundary: 'real App/PostgreSQL; CDP bootstrap failure/delay; no SSR timing comparison',
    browser: await page.cdp.send('Browser.getVersion'), phases: [], transport: [] };
  let phase = 'blocked-bootstrap';
  const errorsBefore = page.errors.length;
  const blockedImport = 'TypeError: Failed to fetch dynamically imported module: ' + args.base + '/pkg/vink.js';
  const network = message => {
    if (message.sessionId !== page.sessionId || message.method !== 'Network.requestWillBeSent') return;
    const r = message.params.request, url = new URL(r.url);
    if (url.origin === args.base) raw.transport.push({ phase, path: url.pathname, method: r.method });
  };
  const assets = [args.base + '/pkg/vink.js*', args.base + '/pkg/vink_bg.wasm*'];
  const held = new Set();
  const bootstrap = message => {
    if (message.sessionId === page.sessionId && message.method === 'Fetch.requestPaused') held.add(message.params.requestId);
  };
  page.cdp.listeners.push(network);
  const flush = () => writeFileSync(join(dirname(args.state), 'reader-ssr-boundary.json'), JSON.stringify(raw, null, 2));
  const noMutation = label => check(!raw.transport.some(r => r.phase === phase && /^(POST|PUT|PATCH|DELETE)$/.test(r.method)), label);
  try {
    await page.send('Network.setBlockedURLs', { urls: assets });
    await page.goto(args.base + PATH);
    check(await page.eval(`!(${READY}) && document.querySelector('.vk-document')?.textContent.includes(${JSON.stringify(TEXT)})`), 'failed bootstrap retains the complete SSR article');
    check(await page.eval(`document.querySelector('link[rel=canonical]')?.href===${JSON.stringify(args.base + PATH)} && !!document.querySelector('meta[name=description]') && !document.documentElement.outerHTML.includes('Khổ đang viết dở — chưa công khai.')`), 'unhydrated document retains canonical/description and excludes the private draft');
    check(await page.eval("document.querySelector('#root-composer-send')?.getAttribute('aria-disabled')==='true' && document.querySelector('#root-composer-send')?.type==='button'"), 'unready composer send is unavailable and has no native form submission');
    await page.click('#root-composer-send');
    await page.click('#discussion-refresh');
    noMutation('clicks before bootstrap do not submit a mutation');
    await page.click('#reader-return');
    await page.waitFor("location.pathname==='/' && !!document.querySelector('#public-query')", 'native Reader return without WASM');
    check(await page.eval(`!(${READY}) && document.querySelector('#public-search')?.method==='get'`), 'native Reader return reaches the real GET search without hydration');
    await page.typeAtEnd('#public-query', 'Mưa');
    await page.eval("document.querySelector('#public-search').requestSubmit()");
    await page.waitFor("new URL(location.href).searchParams.get('q')==='Mưa' && document.readyState==='complete'", 'native GET search before hydration');
    check(await page.eval(`!(${READY}) && !!document.querySelector('.vk-public-row__link')`), 'native GET search renders results without the bootstrap');
    // As in the existing brand bootstrap checks, Leptos reports one rejected import per
    // blocked document. Account for precisely those errors; preserve any unexpected error.
    for (const deadline = Date.now() + 5000; page.errors.length < errorsBefore + 3 && Date.now() < deadline;) await sleep(50);
    const blockedErrors = page.errors.slice(errorsBefore);
    check(blockedErrors.length === 3 && blockedErrors.every(error => error === blockedImport),
      'exactly the three intentionally blocked document imports fail; no unrelated error is ignored');
    page.errors.splice(errorsBefore, 3);
    raw.expected_bootstrap_errors = blockedErrors;
    raw.phases.push({ phase, main_text: 'PASS', links_get_search: 'PASS', private_draft_absent: 'PASS', mutation: 'none' });
    await page.send('Network.setBlockedURLs', { urls: [] });

    page.cdp.listeners.push(bootstrap);
    for (const holdMs of [200, 1000]) {
      phase = 'held-bootstrap-' + holdMs;
      await page.goto('about:blank');
      await page.send('Network.clearBrowserCache');
      await page.send('Fetch.enable', { patterns: assets.map(urlPattern => ({ urlPattern, requestStage: 'Request' })) });
      await page.send('Page.navigate', { url: args.base + PATH });
      await page.waitFor("!!document.querySelector('.vk-document') && !!document.querySelector('#root-composer-send')", 'complete SSR Reader during bootstrap hold');
      check(held.size > 0 && await page.eval(`!(${READY})`), 'bootstrap is actually paused before the click (' + holdMs + 'ms)');
      await page.click('#root-composer-send');
      await page.click('#discussion-refresh');
      await sleep(holdMs);
      noMutation('no mutation while bootstrap is held (' + holdMs + 'ms)');
      // Disable interception before continuing: modules/WASM requested afterwards flow normally.
      await page.send('Fetch.disable');
      held.clear();
      await page.waitFor(READY, 'hydration after held bootstrap', 120000);
      await page.eval('new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)))');
      await sleep(300);
      noMutation('hydration does not replay a pre-hydration click (' + holdMs + 'ms)');
      const readerGets = raw.transport.filter(r => r.phase === phase && r.path === '/api/v1/works/' + WORK + '/reader' && r.method === 'GET');
      check(readerGets.length === 0, 'fresh SSR Reader handoff makes no duplicate typed /reader request (' + holdMs + 'ms)');
      raw.phases.push({ phase, injected_hold_ms: holdMs, hydrated: 'PASS', mutation_replay: 'none', fresh_reader_gets: readerGets.length,
        observation_after_hydration_ms: 300 });
      flush();
    }
  } finally {
    await page.send('Network.setBlockedURLs', { urls: [] });
    await page.send('Fetch.disable');
    page.cdp.listeners = page.cdp.listeners.filter(listener => listener !== network && listener !== bootstrap);
    flush();
  }
}
