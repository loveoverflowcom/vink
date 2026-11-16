// #259: use the existing CDP runner and private-list transport gate. These fixtures
// prove presentation/interaction, not database search or publication semantics.
import { readFileSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { dirname, join } from 'node:path';

export async function deskEmptyState(page, { args, check, DraftGate, textContrast }) {
  const baseline = args['empty-baseline'] === 'true';
  const gate = new DraftGate(page);
  gate.list = { body: { drafts: [] } };
  gate.holdGets = true;
  const settled = () => page.waitFor(`document.querySelector('[data-testid=draft-content]')?.getAttribute('aria-busy') === 'false'`, 'empty-state read settles');
  const refresh = async body => {
    gate.list = { raw: true, body };
    await page.click('#refresh-drafts');
    await settled();
  };
  const geometry = [];
  const network = [];
  const listen = event => {
    if (event.sessionId === page.sessionId && event.method === 'Network.requestWillBeSent'
        && new URL(event.params.request.url).pathname.startsWith('/pkg/empty/')) {
      network.push(event.params.request.url);
    }
  };
  page.cdp.listeners.push(listen);
  await page.send('Network.setCacheDisabled', { cacheDisabled: true });
  await page.send('Network.clearBrowserCache');
  await gate.enable();
  try {
    await page.viewport(390);
    await page.goto(args.base + '/ban-viet');
    const initial = await gate.next('gets');
    await page.waitFor(`document.querySelector('[data-testid=draft-loading]')`, 'held initial read');
    check(network.length === 0 && await page.eval(`!document.querySelector('[data-testid=draft-empty], [data-testid=draft-no-results], #create-draft')`), '#259 loading requests no artwork and claims no empty corpus');
    await page.eval(`window.__emptySearch=document.querySelector('#draft-search'); window.__emptySearch.focus(); window.__emptySearch.value='không khớp'; window.__emptySearch.dispatchEvent(new InputEvent('input',{bubbles:true})); window.__emptySearch.setSelectionRange(2,5)`);
    gate.holdGets = false;
    await gate.answerGet(initial);
    await settled();
    await page.waitFor(`document.querySelector('[data-testid=draft-empty]')`, 'successful unfiltered empty head');
    check(await page.eval(`document.querySelector('#draft-search')===window.__emptySearch && document.activeElement===window.__emptySearch && window.__emptySearch.value==='không khớp' && window.__emptySearch.selectionStart===2 && window.__emptySearch.selectionEnd===5`), '#259 loading-to-empty preserves the input node, unapplied query, focus and caret');
    await page.key('Enter');
    await page.waitFor(`document.querySelector('[data-testid=draft-no-results]')`, 'applied search miss');

    for (const state of ['no-results', 'first-draft']) {
      if (state === 'first-draft') {
        await page.click('#clear-draft-search');
        await page.waitFor(`document.querySelector('[data-testid=draft-empty]')`, 'clear search returns to first-use');
        check(await page.eval(`document.querySelector('#draft-search')===window.__emptySearch && document.activeElement===window.__emptySearch && window.__emptySearch.value===''`), '#259 clear keeps input identity and restores search focus');
      }
      const target = state === 'first-draft' ? '[data-testid=draft-empty]' : '[data-testid=draft-no-results]';
      const action = state === 'first-draft' ? '#create-draft' : '#clear-draft-search';
      if (!baseline) {
        check(await page.eval(`!!document.querySelector(${JSON.stringify(target + ' img')})`), `#259 ${state}: semantic artwork is rendered`);
        await page.eval(`document.querySelector(${JSON.stringify(target + ' img')}).decode()`);
        check(await page.eval(`(() => {const root=document.querySelector(${JSON.stringify(target)}),img=root.querySelector('img'); return root.querySelectorAll('img').length===1 && img.getAttribute('src')==='/pkg/empty/vink-empty-${state}.webp' && img.naturalWidth===512 && img.naturalHeight===427 && img.alt==='' && !root.querySelector('h2,h3,.vk-ui-panel') && document.querySelectorAll('#drafts [role=status]').length===1 && document.querySelectorAll('#create-draft').length===1;})()`), `#259 ${state}: shared artwork, decoded dimensions, decoration and one result status/create target`);
      }
      for (const width of [320, 390, 834, 1440]) for (const theme of ['light', 'dark']) for (const scale of [100, 200]) {
        await page.viewport(width);
        await page.eval(`document.documentElement.dataset.theme='${theme}'; document.documentElement.style.fontSize='${scale}%'; scrollTo(0,0); document.activeElement?.blur()`);
        await page.eval(`document.fonts.ready.then(()=>new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r))))`);
        await page.eval(`Promise.all(document.getAnimations().filter(a=>Number.isFinite(a.effect.getComputedTiming().endTime)).map(a=>a.finished.catch(()=>{})))`);
        const measured = await page.eval(`(() => {
          const root=document.querySelector(${JSON.stringify(target)}), img=root.querySelector('img'),p=root.querySelector('.vk-empty-state__message');
          const r=el=>el?.getBoundingClientRect().toJSON();
          return {width:innerWidth,theme:document.documentElement.dataset.theme,scale:${scale},state:${JSON.stringify(state)},art:r(img),message:r(p),messageFont:p&&getComputedStyle(p).fontSize,scrollWidth:document.documentElement.scrollWidth};
        })()`);
        geometry.push(measured);
        check(measured.scrollWidth <= width, `#259 ${state} ${width}/${theme}/${scale}: no horizontal overflow`);
        if (!baseline) check(Math.abs(measured.art.width - (width < 760 ? 192 : 240)) < 1 && Math.abs(measured.art.height / measured.art.width - 427 / 512) < .01 && measured.messageFont === (scale === 100 ? '16px' : '32px'), `#259 ${state} ${width}/${theme}/${scale}: fixed artwork and scaled body type`);
        if (!baseline && scale===100) await textContrast(page,theme,'desk-empty');
        await page.capture(`desk-art-${state}-${width}-${theme}-${scale}`, { scenario: '#259-empty-artwork', state, target, boundary: 'intercepted-private-summary-transport', requestedScene: {width,height:900,theme,scale} });
        await page.eval(`document.querySelector(${JSON.stringify(action)}).focus(); document.querySelector(${JSON.stringify(action)}).scrollIntoView({block:'center'})`);
        check(await page.eval(`(() => {const r=document.querySelector(${JSON.stringify(action)}).getBoundingClientRect(),b=document.querySelector('.vk-ui-bottom-nav'),limit=b?.getClientRects().length?b.getBoundingClientRect().top:innerHeight; return r.top>=0 && r.bottom<=limit+1 && r.width>=44 && r.height>=44;})()`), `#259 ${state} ${width}/${theme}/${scale}: recovery reachable above navigation`);
        await page.eval(`scrollTo(0,document.documentElement.scrollHeight)`);
        check(await page.eval(`(() => {const p=document.querySelector('.vk-desk-privacy'),r=p.getBoundingClientRect(),b=document.querySelector('.vk-ui-bottom-nav'),limit=b?.getClientRects().length?b.getBoundingClientRect().top:innerHeight;return r.bottom<=limit+1 && r.top>=0 && p.textContent.includes('Bản nháp chỉ bạn đọc được');})()`), `#259 ${state} ${width}/${theme}/${scale}: privacy copy reachable above navigation`);
        if (width < 760 && scale === 200) await page.capture(`desk-art-${state}-${width}-${theme}-${scale}-privacy`, { scenario: '#259-private-copy', state, target: '.vk-desk-privacy', boundary: 'intercepted-private-summary-transport', requestedScene: {width,height:900,theme,scale} });
      }
    }
    await page.viewport(390);
    await page.eval(`document.documentElement.style.fontSize=''; document.documentElement.dataset.theme='light'; scrollTo(0,0)`);
    if (!baseline) {
      const manifest = JSON.parse(readFileSync('apps/vink-web/assets/empty/asset-manifest.json'));
      for (const id of ['no-results','first-draft']) {
        const asset = manifest.runtime_assets.find(a=>a.id===id);
        const response = await fetch(args.base + '/pkg/empty/vink-empty-' + id + '.webp');
        const bytes = Buffer.from(await response.arrayBuffer());
        check(response.status===200 && response.headers.get('content-type')?.startsWith('image/webp') && bytes.length===asset.bytes && createHash('sha256').update(bytes).digest('hex')===asset.sha256, `#259 ${id}: actual HTTP MIME, bytes and dependency hash`);
      }
      check(network.length === 3 && network.every(u => /vink-empty-(first-draft|no-results)\.webp$/.test(u)), '#259 exactly one artwork request per empty transition; no public-corpus or theme-specific image');
      const original = await page.eval(`document.querySelector('[data-testid=draft-empty] img').src`);
      await page.send('Network.setBlockedURLs', { urls: ['*vink-empty-first-draft.webp?blocked259*'] });
      await page.eval(`document.querySelector('[data-testid=draft-empty] img').src=${JSON.stringify(original + '?blocked259')}`);
      await page.waitFor(`document.querySelector('[data-testid=draft-empty] img').complete && document.querySelector('[data-testid=draft-empty] img').naturalWidth===0`, 'blocked artwork really fails');
      check(await page.eval(`document.querySelector('[data-testid=draft-empty] .vk-empty-state__message').textContent.length>0 && document.querySelector('#create-draft').getAttribute('aria-disabled')!=='true' && document.querySelectorAll('#drafts [role=status]').length===1`), '#259 blocked image preserves meaningful HTML, status and enabled recovery');
      await page.capture('desk-art-blocked', {target:'[data-testid=draft-empty]', boundary:'blocked-image-real-browser'});
      await page.send('Network.setBlockedURLs', { urls: [] });
      await page.eval(`const img=document.querySelector('[data-testid=draft-empty] img'); img.src=${JSON.stringify(original)}; img.decode()`);
      // Retained successful empty is labelled while pending; an error remains an error.
      gate.holdGets = true;
      await page.click('#refresh-drafts');
      const pending = await gate.next('gets');
      check(await page.eval(`document.querySelector('[data-testid=draft-empty]') && document.querySelector('[data-testid=author-work-stale]') && document.querySelector('#create-draft').getAttribute('aria-disabled')==='true'`), '#259 retained empty during refresh keeps its stale qualifier and disables create');
      await gate.fulfill(pending, 503, {error:{code:'dependency_unavailable'}});
      await settled();
      check(await page.eval(`document.querySelector('[data-testid=desk-error]') && document.querySelector('[data-testid=author-work-stale]') && !document.querySelector('#drafts img')`), '#259 load error preserves warning and is not artwork empty');
      gate.holdGets = false;
      gate.list = { raw:true, body:{drafts:[],next_cursor:'fixture:24'} };
      await page.click('#retry-drafts'); await settled();
      check(await page.eval(`!document.querySelector('[data-testid=draft-empty], [data-testid=draft-no-results], #drafts img') && document.querySelector('[data-testid=desk-error]') && document.querySelector('[data-testid=author-work-stale]')`), '#259 empty page with continuation is rejected by existing PageChain; remains an error, never first-use');
      await page.eval(`window.__emptySearch.focus(); window.__emptySearch.dispatchEvent(new CompositionEvent('compositionstart',{bubbles:true})); window.__emptySearch.setSelectionRange(0,0)`);
      const before = gate.requests.length;
      await page.key('Enter');
      check(gate.requests.length===before && await page.eval(`document.activeElement===window.__emptySearch && document.querySelector('#draft-search')===window.__emptySearch`), '#259 simulated composition never submits or remounts search');
      await page.eval(`window.__emptySearch.dispatchEvent(new CompositionEvent('compositionend',{bubbles:true}))`);
      await refresh({drafts:[]});
      await gate.disable();
      let corrupt = 0;
      const corruptListener = event => {
        if (event.sessionId===page.sessionId && event.method==='Fetch.requestPaused') {
          corrupt++;
          page.send('Fetch.fulfillRequest',{requestId:event.params.requestId,responseCode:200,responseHeaders:[{name:'Content-Type',value:'image/webp'}],body:Buffer.from('not a WebP').toString('base64')}).catch(e=>{gate.error=e;});
        }
      };
      page.cdp.listeners.push(corruptListener);
      try {
        await page.send('Fetch.enable',{patterns:[{urlPattern:'*vink-empty-first-draft.webp?corrupt259*',requestStage:'Request'}]});
        await page.eval(`document.querySelector('[data-testid=draft-empty] img').src=${JSON.stringify(original + '?corrupt259')}`);
        await page.waitFor(`document.querySelector('[data-testid=draft-empty] img').complete && document.querySelector('[data-testid=draft-empty] img').naturalWidth===0`, 'corrupt image really fails');
        check(corrupt===1 && await page.eval(`document.querySelector('[data-testid=draft-empty] .vk-empty-state__message').textContent.length>0 && document.querySelector('#create-draft').getAttribute('aria-disabled')!=='true'`), '#259 one injected corrupt response preserves HTML and create');
        await page.capture('desk-art-corrupt', {target:'[data-testid=draft-empty]',boundary:'one-injected-corrupt-image-response'});
      } finally {
        await page.send('Fetch.disable');
        page.cdp.listeners=page.cdp.listeners.filter(l=>l!==corruptListener);
      }
    }
    writeFileSync(join(dirname(args.state), 'desk-empty-results.json'), JSON.stringify({baseline,geometry,network,boundary:'Chrome-CDP; private-summary transport fixtures; root font-size text200; no real OS IME'},null,2));
  } finally {
    await gate.disable();
    page.cdp.listeners=page.cdp.listeners.filter(l=>l!==listen);
    await page.send('Network.setBlockedURLs',{urls:[]});
    await page.send('Network.setCacheDisabled',{cacheDisabled:false});
    await page.eval(`document.documentElement.style.fontSize=''; delete document.documentElement.dataset.theme`);
  }
}
