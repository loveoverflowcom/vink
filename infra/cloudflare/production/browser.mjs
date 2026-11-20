import { readFileSync } from 'node:fs';

// A read-only journey for the release controller and its isolated qualification host.
export async function productionEdge(page, { args, check }) {
  const reader = args.reader;
  check(/^[0-9a-f-]{36}$/.test(reader) && /^[0-9a-f]{40}$/.test(args.revision), 'reader and release provided');
  if (args['cached-html']) {
    check(new URL(args.base).hostname === '127.0.0.1' && /^[0-9a-f]{40}$/.test(args['cached-revision']),
      'stale document replay is local only');
    const html = readFileSync(args['cached-html']);
    const old = '/_v/' + args['cached-revision'] + '/pkg/';
    check(html.includes(old + 'vink.js'), 'captured SSR document references previous release');
    await page.send('Fetch.enable', { patterns:[{urlPattern:args.base + '/', resourceType:'Document'}] });
    const held = message => {
      if (message.sessionId === page.sessionId && message.method === 'Fetch.requestPaused') {
        page.send('Fetch.fulfillRequest', { requestId:message.params.requestId, responseCode:200,
          responseHeaders:[{name:'Content-Type',value:'text/html; charset=utf-8'}], body:html.toString('base64'),
        }).catch(e => page.errors.push(e.message));
      }
    };
    page.cdp.listeners.push(held);
    await page.goto(args.base + '/');
    await page.waitFor("document.documentElement.dataset.hydrated === 'true'", 'cached HTML hydrates on current edge');
    check(await page.eval(`performance.getEntriesByType('resource').some(e => new URL(e.name).pathname === ${JSON.stringify(old + 'vink_bg.wasm')})`),
      'cached HTML loads its original WASM');
    await page.send('Fetch.disable');
    page.cdp.listeners.splice(page.cdp.listeners.indexOf(held), 1);
  }
  const prefix = `/_v/${args.revision}/pkg/`;
  const hydrated = "document.documentElement.dataset.hydrated === 'true'";
  for (const route of ['/', `/tac-pham/${reader}`, '/ban-viet', '/toi']) {
    await page.goto(args.base + route);
    await page.waitFor(hydrated, `hydrated ${route}`);
    check(await page.eval(`document.querySelector('link[rel=modulepreload]').getAttribute("href") === ${JSON.stringify(prefix + 'vink.js')}`),
      `SSR and browser load the selected JS on ${route}`);
    const paths = await page.eval("performance.getEntriesByType('resource').map(e => new URL(e.name).pathname)");
    check(paths.includes(prefix + 'vink_bg.wasm') && paths.includes(prefix + 'vink.css'), 'versioned WASM and CSS loaded');
    if (route.startsWith('/tac-pham/')) {
      check(await page.eval(`(async () => (await fetch('/api/v1/works/${reader}/reader')).ok)()`), 'reader has public API data');
      await page.reload();
    }
  }
  await page.eval(`document.querySelector('a[href="/ban-viet"]').click()`);
  await page.waitFor(`location.pathname === '/ban-viet' && ${hydrated}`, 'navigation from account to desk');
}
