// Local #473 comparison. Measure the first frame after the sample text enters the DOM.
// A warm new tab reuses the browser profile. It has its own document and WASM instance.
export async function readerStartup(page, {args, check, newPage}) {
  const url = args.base + '/tac-pham/33333333-3333-4333-8333-333333333333';
  const setup = `(() => {
    const timing = window.readerStartup = {text:0, ready:0, lcp:0};
    new PerformanceObserver(list => {
      timing.lcp = list.getEntries().at(-1)?.startTime || timing.lcp;
    }).observe({type:'largest-contentful-paint', buffered:true});
    const scan = () => {
      if (!timing.text && document.querySelector('.vk-document')?.textContent.includes('Phố cũ mưa giăng sợi mỏng,')) {
        requestAnimationFrame(() => { if (!timing.text) timing.text = performance.now(); });
      }
      if (!timing.ready && (document.documentElement?.dataset.hydrated ||
        (document.documentElement?.dataset.renderMode === 'csr' && document.querySelector('#noi-dung')))) {
        timing.ready = performance.now();
      }
      if (timing.text && timing.ready) observer.disconnect();
    };
    const observer = new MutationObserver(scan);
    observer.observe(document, {childList:true, subtree:true, attributes:true});
  })();`;
  await page.send('Page.addScriptToEvaluateOnNewDocument', {source:setup});
  for (const cpu of [1, 4]) {
    await page.send('Network.clearBrowserCache');
    for (const phase of ['cold', 'reload', 'new-tab']) {
      const tab = phase === 'new-tab' ? await newPage() : page;
      await tab.send('Emulation.setCPUThrottlingRate', {rate:cpu});
      if (phase === 'new-tab') await tab.send('Page.addScriptToEvaluateOnNewDocument', {source:setup});
      if (phase === 'reload') await tab.reload();
      else await tab.goto(url);
      await tab.waitFor('window.readerStartup?.text && window.readerStartup?.ready', 'Reader text and app ready');
      const timings = await tab.eval(`(async () => {
        await document.fonts.ready;
        await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
        const nav = performance.getEntriesByType('navigation')[0];
        const resources = performance.getEntriesByType('resource');
        return {...readerStartup, ttfb:nav.responseStart, fcp:performance.getEntriesByName('first-contentful-paint')[0]?.startTime,
          bytes:nav.transferSize + resources.reduce((sum,r) => sum + r.transferSize, 0),
          api:resources.filter(r => new URL(r.name).pathname.startsWith('/api/')).map(r =>
            ({path:new URL(r.name).pathname,start:Math.round(r.startTime),end:Math.round(r.responseEnd)}))};
      })()`);
      check(timings.text > 0 && timings.ready > 0, 'measured rendered publication and mounted app');
      console.log('READER_STARTUP ' + JSON.stringify({cpu,phase,...timings}));
    }
  }
}
