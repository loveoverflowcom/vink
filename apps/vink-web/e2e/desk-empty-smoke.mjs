// #259: use the existing CDP runner and private-list transport gate. These fixtures
// prove presentation/interaction, not database search or publication semantics.
import { wakeDesk } from './desk-lifecycle.mjs';
import { readFileSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { dirname, join } from 'node:path';

export async function deskGuest(page, {args, check}, signInOpen) {
  const path = args.base+'/ban-viet';
  const html = await (await fetch(path)).text();
  check(html.includes('data-testid="draft-loading"') && !html.includes('id="draft-search"') &&
    !html.includes('id="desk-sign-in"') && !html.includes('id="draft-list-title"'),
  '#308 SSR Desk shows a session check without the private heading, controls or a guessed standing');
  const draftReads = [];
  const observe = ({sessionId,method,params}) => {
    if (sessionId!==page.sessionId || method!=='Network.requestWillBeSent') return;
    const path = new URL(params.request.url).pathname;
    if (path==='/api/v1/author/works' || path==='/api/v1/drafts') draftReads.push(path);
  };
  page.cdp.listeners.push(observe);
  try {
    await page.goto(path);
    await page.waitFor("document.querySelector('[data-testid=desk-access]') && document.querySelector('#navigation-account a[href=\"/toi\"]')", '#308 confirmed guest Desk and rail');
    const title = signInOpen ? 'Đăng nhập để mở Bàn viết' : 'Vink chưa mở đăng nhập';
    check(await page.eval(`document.querySelector('.vk-desk-access h2').textContent===${JSON.stringify(title)}
      && !document.querySelector('#draft-search, #draft-sort, #retry-drafts, #create-draft')
      && !document.querySelector('#draft-list-title') && document.querySelectorAll('#desk h1').length===1
      && Boolean(document.querySelector('#desk-sign-in'))===${signInOpen}
      && document.querySelector('.vk-desk-access .vk-empty-state__actions a[href="/"]')?.textContent==='Khám phá tác phẩm'
      && document.querySelectorAll('.vk-desk-access .vk-empty-state__actions a').length===${signInOpen ? 2 : 1}`),
      '#308 guest actions reflect this deployment’s sign-in capability');
    if (signInOpen) {
      check(await page.eval(`(() => {
        const login=document.querySelector('#desk-sign-in');
        return login.textContent.trim()==='Đăng nhập' && login.getAttribute('href')==='/toi?next=/ban-viet' &&
          login.classList.contains('vk-ui-button--filled') &&
          login.firstElementChild.matches('svg[data-icon=lock][aria-hidden=true]') &&
          login.firstElementChild.getBoundingClientRect().right <= login.lastElementChild.getBoundingClientRect().left &&
          document.querySelectorAll('.vk-desk-access .vk-ui-button--filled').length===1;
      })()`), '#411 the primary guest action signs in and returns to the Desk');
    }
    const label = signInOpen ? 'Đăng nhập' : 'Tài khoản';
    const { root } = await page.send('DOM.getDocument');
    for (const width of [320,390,834,1440]) for (const theme of ['light','dark']) for (const scale of [100,150,200]) {
      const navSelector = (width<760 ? '#bottom-navigation' : '#navigation-account')+' a[data-destination=me]';
      await page.viewport(width, width<760 ? 844 : 760);
      await page.eval(`document.documentElement.dataset.theme='${theme}'; document.documentElement.style.fontSize='${scale}%';
        document.fonts.ready.then(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))))`);
      const layout = await page.eval(`(() => {
        const box = selector => document.querySelector(selector).getBoundingClientRect();
        const panel = document.querySelector('.vk-desk-access'), panelStyle=getComputedStyle(panel);
        const nav = document.querySelector(${JSON.stringify(navSelector)});
        const actions = [...panel.querySelectorAll('a')].map(a=>a.getBoundingClientRect());
        return {
          centerDelta:(box('.vk-desk-access').top+box('.vk-desk-access').bottom
            -box('#heading').bottom-24-box('.vk-main').bottom+parseFloat(getComputedStyle(document.querySelector('.vk-main')).paddingBottom))/2,
          border:panelStyle.borderWidth, width:document.documentElement.scrollWidth,
          transparent:panelStyle.backgroundColor==='rgba(0, 0, 0, 0)',
          nav:nav.textContent.trim(),
          target:actions.every(r=>r.height>=48 && r.width>=48 && r.left>=0 && r.right<=innerWidth),
        };
      })()`);
      const { nodeId } = await page.send('DOM.querySelector', { nodeId: root.nodeId, selector: navSelector });
      const { nodes: [guestLink] } = await page.send('Accessibility.getPartialAXTree', { nodeId, fetchRelatives: false });
      const name = guestLink?.role.value==='link' ? guestLink.name?.value : undefined;
      check(layout.width<=width && layout.transparent && layout.border==='0px' && layout.target &&
        layout.nav===label && name===label &&
        (scale===200 || Math.abs(layout.centerDelta)<=2),
        '#308 guest Desk geometry/labels '+[width,theme,scale].join('/')+' '+JSON.stringify({ ...layout, name }));
      if (width===390 || width===1440 && scale===100) {
        await page.capture('desk-guest-'+[signInOpen?'open':'closed',width,theme,scale].join('-'), {
          scenario:'#308-guest-desk', boundary:'real-App-session',
        });
      }
      if (width<=390 && scale===200) {
        check(await page.eval(`(() => {
          const cta=document.querySelector('.vk-desk-access .vk-empty-state__actions a');
          cta.scrollIntoView({block:'center'});
          const rect=cta.getBoundingClientRect(), bar=document.querySelector('#bottom-navigation').getBoundingClientRect();
          const hit=document.elementFromPoint(rect.left+rect.width/2, rect.top+rect.height/2);
          return rect.top>=0 && rect.bottom<=bar.top+1 && cta.contains(hit);
        })()`), '#308 guest primary action scrolls clear of the bottom navigation '+width+'/'+theme);
        await page.eval('scrollTo(0,0)');
      }
    }
    await page.eval("document.documentElement.style.fontSize=''; delete document.documentElement.dataset.theme; document.querySelector('#navigation-primary a[href=\"/ban-viet\"]').focus()");
    const guestIcons = scope => page.eval(`(() => {
      const root = document.querySelector(${JSON.stringify(scope)});
      const get = key => {
        const link = root?.querySelector('a[data-destination=' + JSON.stringify(key) + ']');
        const svg = link?.querySelector('svg[data-icon]');
        const path = svg?.querySelector('path:not([fill=none])') ?? [...(svg?.querySelectorAll('path') ?? [])].at(-1);
        return { current: link?.getAttribute('aria-current') === 'page', icon: svg?.getAttribute('data-icon') ?? null, path: path?.getAttribute('d') ?? null };
      };
      return { explore: get('explore'), desk: get('desk'), me: get('me') };
    })()`);
    const inactiveAccountIcon = signInOpen ? 'lock' : 'person-outline';
    const activeAccountIcon = signInOpen ? 'lock' : 'person';
    const guestDeskRail = await guestIcons('#navigation');
    const guestDeskBar = await guestIcons('#bottom-navigation');
    check(guestDeskRail.explore.icon === 'explore-outline' && guestDeskRail.desk.icon === 'edit' && guestDeskRail.desk.current &&
      guestDeskRail.me.icon === inactiveAccountIcon && !guestDeskRail.me.current && Boolean(guestDeskRail.me.path) &&
      guestDeskBar.explore.icon === 'explore-outline' && guestDeskBar.desk.icon === 'edit' && guestDeskBar.me.icon === inactiveAccountIcon,
      `#427 AC-04 guest /ban-viet uses outlined Explore, filled Desk and the gate-appropriate account icon (${JSON.stringify({ guestDeskRail, guestDeskBar })})`);
    check(await page.eval(`!document.querySelector('#profile-trigger, #navigation .vk-ui-nav-utility')`),
      'no appearance utility button remains on the rail');
    await page.key('Tab');
    check(await page.eval(`document.activeElement.matches('#navigation-account a:focus-visible') &&
      parseFloat(getComputedStyle(document.activeElement).outlineWidth)>0`), '#308 guest rail link is keyboard reachable with a focus ring');
    check(draftReads.length===0, '#308 guest Desk sends zero private list requests');
    await page.key('Enter');
    await page.waitFor(`location.pathname==='/toi' &&
      document.querySelector('${signInOpen?'#password-login':'#sign-in-closed'}')`, '#308 rail goes to Account');
    const guestMeRail = await guestIcons('#navigation');
    const guestMeBar = await guestIcons('#bottom-navigation');
    check(guestMeRail.explore.icon === 'explore-outline' && guestMeRail.desk.icon === 'edit-outline' &&
      guestMeRail.me.icon === activeAccountIcon && guestMeRail.me.current && Boolean(guestMeRail.me.path) &&
      (guestMeRail.me.path === guestDeskRail.me.path) === signInOpen && guestMeRail.desk.path !== guestDeskRail.desk.path &&
      guestMeBar.explore.icon === 'explore-outline' && guestMeBar.desk.icon === 'edit-outline' && guestMeBar.me.icon === activeAccountIcon,
      `#427 AC-04 guest /toi selects Account and keeps the correct gate-specific icon (${JSON.stringify({ guestMeRail, guestMeBar })})`);
    check(await page.eval(`!document.querySelector('#page-appearance, #auth-appearance') &&
      !document.documentElement.dataset.text && getComputedStyle(document.documentElement).fontSize === '16px'`),
      '#576 guest settings are absent and leave the default text size');
    if (!signInOpen) {
      check(await page.eval(`!document.querySelector('#account-session form')
        && document.querySelector('#sign-in-closed').textContent.includes('Vink chưa mở đăng nhập')`),
        '#327 real sign-in-disabled App has only availability copy');
    }
  } finally {
    page.cdp.listeners = page.cdp.listeners.filter(handler=>handler!==observe);
  }
}

// Records every matching node added or removed under the chosen owner, including a node that is
// mounted and dropped again within one task (the observer keeps the record).
const watchNodes = (page, name, selector, kind, owner = '#drafts') => page.eval(`(() => {
  window.__${name}?.observer?.disconnect();
  const hits = [], sel = ${JSON.stringify(selector)};
  const observer = new MutationObserver(records => { for (const record of records) for (const node of record[${JSON.stringify(kind)}])
    if (node.nodeType === 1 && (node.matches(sel) || node.querySelector(sel))) hits.push(performance.now()); });
  observer.observe(document.querySelector(${JSON.stringify(owner)}), { childList: true, subtree: true });
  hits.observer = observer; window.__${name} = hits;
})()`);

// Visual cluster (artwork top → actions bottom) against the free region: from the last visible
// control above the slot (after its margin) to the end of `.vk-main`'s content box minus the privacy paragraph.
// Document coordinates, so the result does not depend on the scroll position.
const PLACEMENT = `(() => {
  const box = el => el.getBoundingClientRect(), state = root.querySelector('.vk-empty-state');
  let prev = root.previousElementSibling; while (prev && !box(prev).height) prev = prev.previousElementSibling;
  const main = document.querySelector('.vk-main'), privacy = document.querySelector('.vk-desk-privacy'), ps = getComputedStyle(privacy);
  const top = (prev ? box(prev).bottom + parseFloat(getComputedStyle(prev).marginBlockEnd)
    : box(root.parentElement).top) + scrollY;
  const bottom = box(main).bottom + scrollY - parseFloat(getComputedStyle(main).paddingBottom) - privacy.offsetHeight - parseFloat(ps.marginTop) - parseFloat(ps.marginBottom);
  const groupTop = box(state.querySelector('img')).top + scrollY, groupBottom = box(state.querySelector('.vk-empty-state__actions')).bottom + scrollY;
  const round = n => Math.round(n * 10) / 10;
  return { region: [round(top), round(bottom)], group: [round(groupTop), round(groupBottom)], gapAbove: round(groupTop - top), gapBelow: round(bottom - groupBottom), centerDelta: round((groupTop + groupBottom - top - bottom) / 2) };
})()`;

export async function deskEmptyState(page, { args, check, DraftGate, textContrast }) {
  const baseline = args['empty-baseline'] === 'true';
  const gate = new DraftGate(page);
  gate.list = { body: { drafts: [] } };
  gate.holdGets = true;
  const settled = () => page.waitFor(`document.querySelector('[data-testid=draft-content]')?.getAttribute('aria-busy') === 'false'`, 'empty-state read settles');
  const refresh = async body => {
    gate.list = { raw: true, body };
    await wakeDesk(page);
    await settled();
  };
  const geometry = [];
  const network = [];
  let loader;
  const listen = event => {
    if (event.sessionId === page.sessionId && event.method === 'Network.requestWillBeSent'
        && event.params.loaderId === loader
        && new URL(event.params.request.url).pathname.startsWith('/pkg/empty/')) {
      network.push(event.params.request.url);
    }
  };
  page.cdp.listeners.push(listen);
  await page.send('Network.setCacheDisabled', { cacheDisabled: true });
  await page.send('Network.clearBrowserCache');
  await gate.enable();
  try {
    if (!baseline) {
      await page.viewport(1440);
      await page.goto(args.base + '/ban-viet');
      await gate.fulfill(await gate.next('gets'), 503, {error:{code:'dependency_unavailable'}});
      await settled();
      check(await page.eval(`document.querySelectorAll('[data-testid=desk-error][role=alert]').length===1 &&
        !document.querySelector('#draft-scope').checkVisibility() &&
        document.querySelector('#draft-sort').checkVisibility() &&
        document.querySelector('#open-draft-search').checkVisibility()`),
        '#593 initial failure has one error owner while stable search/sort remain available');
      const { nodes: errorAx } = await page.send('Accessibility.getFullAXTree');
      check(!errorAx.some(node => !node.ignored && node.name?.value === 'Chưa tải được kết quả.'),
        '#593 initial error does not duplicate the alert in the accessibility tree');
      for (const [width,theme,scale] of [[390,'light',100],[320,'dark',200]]) {
        await page.viewport(width);
        await page.eval(`document.documentElement.dataset.theme='${theme}'; document.documentElement.style.fontSize='${scale}%';
          document.querySelector('#retry-drafts').focus(); document.querySelector('#retry-drafts').scrollIntoView({block:'center'}); document.fonts.ready`);
        await page.capture(`desk-initial-error-${width}-${theme}-${scale}`, {scenario:'#593-initial-error',target:'[data-testid=desk-error]',boundary:'intercepted-private-summary-transport'});
      }
      await page.viewport(1440);
      await page.eval(`document.documentElement.style.fontSize='100%'; document.documentElement.dataset.theme='light'`);
      await page.click('#retry-drafts');
      await gate.answerGet(await gate.next('gets'));
      await settled();
      check(await page.eval(`!document.querySelector('[data-testid=desk-error]') &&
        document.querySelector('[data-testid=draft-empty]') && !document.querySelector('#draft-scope').hidden`),
        '#593 retry clears the error and restores the settled live status');
      await page.reload();
      const rail = '#navigation-primary a[href="/ban-viet"]';
      for (const selector of ['#open-draft-search', '#draft-sort', rail]) {
        const held = await gate.next('gets');
        const target = JSON.stringify(selector);
        await page.eval(`window.__heldSearch = document.querySelector('#draft-search');
          document.querySelector(${target}).focus()`);
        check(await page.eval(`document.activeElement === document.querySelector(${target})`),
          `#411 focus starts on ${selector} before the empty page returns`);
        await gate.answerGet(held);
        await settled();
        await page.waitFor(`document.querySelector('[data-testid=draft-empty]') && document.activeElement ===
          document.querySelector(${selector === rail ? target : "'#desk-title'"})`,
          `#411 initial empty result settles focus from ${selector}`);
        if (selector !== rail) {
          await page.key('Tab');
          check(await page.eval(`document.activeElement?.id === 'create-draft-format'`),
            `#411 from ${selector}, Tab reaches the format Select after recovery`);
        }
        check(await page.eval(`document.querySelector('#draft-search') === window.__heldSearch`),
          '#411 the initial empty response leaves the search node mounted');
        await page.reload();
      }
      await page.viewport(390);
    } else {
      await page.viewport(390);
      await page.goto(args.base + '/ban-viet');
    }
    loader = (await page.send('Page.getFrameTree')).frameTree.frame.loaderId;
    const initial = await gate.next('gets');
    await page.waitFor(`document.querySelector('[data-testid=draft-loading]')`, 'held initial read');
    await page.waitFor(`document.querySelector('.vk-desk-loading__placeholder:not([hidden]) .vk-ui-skeleton')`, '#411 a held read shows the UIS-03 skeleton');
    await page.capture('desk-loading-held-390', {scenario:'#411-initial-load',target:'.vk-desk-loading',boundary:'intercepted-private-summary-transport'});
    check(network.length === 0 && await page.eval(`!document.querySelector('[data-testid=draft-empty], [data-testid=draft-no-results], #create-draft')`), '#259 loading requests no artwork and claims no empty corpus');
    await page.click('#open-draft-search');
    await watchNodes(page, 'searchRemoved', '#draft-search', 'removedNodes', '#heading');
    await page.eval(`window.__emptySearch=document.querySelector('#draft-search'); window.__emptySearch.focus(); window.__emptySearch.value='không khớp'; window.__emptySearch.dispatchEvent(new InputEvent('input',{bubbles:true})); window.__emptySearch.setSelectionRange(2,5)`);
    gate.holdGets = false;
    await gate.answerGet(initial);
    await settled();
    await page.waitFor(`document.querySelector('[data-testid=draft-empty]')`, 'successful unfiltered empty head');
    check(await page.eval(`document.querySelector('#draft-search')===window.__emptySearch && document.activeElement===window.__emptySearch && window.__emptySearch.value==='không khớp' && window.__emptySearch.selectionStart===2 && window.__emptySearch.selectionEnd===5`), '#259 loading-to-empty preserves the input node, unapplied query, focus and caret');
    if (!baseline) {
      await page.key('Escape');
      await page.waitFor(`document.activeElement?.id === 'open-draft-search' &&
        document.querySelector('#draft-sort')?.checkVisibility()`, '#411 text in the field keeps closed search and sort available on first-use');
      await page.key('Enter');
      await page.waitFor(`document.activeElement === window.__emptySearch &&
        document.querySelector('#draft-search') === window.__emptySearch &&
        window.__emptySearch.value === 'không khớp' && !window.__searchRemoved.length`,
        '#411 reopening the dock retains the same text and search node');
    }
    await page.key('Enter');
    await page.waitFor(`document.querySelector('[data-testid=draft-no-results]')`, 'applied search miss');
    // UIS-03 `Empty + Loading`: a same-query refresh keeps the settled miss (with its stale
    // qualifier) instead of removing and re-adding the slot.
    await watchNodes(page, 'missRemoved', '[data-testid=draft-no-results]', 'removedNodes');
    gate.holdGets = true;
    await wakeDesk(page);
    const sameQuery = await gate.next('gets');
    check(await page.eval(`window.__missRemoved.length===0 && !!document.querySelector('[data-testid=draft-no-results]') && !!document.querySelector('[data-testid=author-work-stale]')`), 'audit: retained no-results stays, labelled stale, while the same query refreshes');
    gate.holdGets = false;
    await gate.answerGet(sameQuery);
    await settled();
    check(await page.eval(`window.__missRemoved.length===0 && !!document.querySelector('[data-testid=draft-no-results]') && !document.querySelector('[data-testid=author-work-stale]')`), 'audit: same-query refresh settles without remounting the no-results slot');

    for (const state of ['no-results', 'first-draft']) {
      if (state === 'first-draft') {
        // A search miss is never reused as the unfiltered head: nothing claims first-use (not
        // even for one frame, nor by requesting its artwork) until that head is answered.
        await watchNodes(page, 'falseFirstUse', '[data-testid=draft-empty]', 'addedNodes');
        const artBefore = network.length;
        gate.holdGets = true;
        await page.click('#clear-draft-search');
        const unfiltered = await gate.next('gets');
        await page.eval(`new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)))`);
        check(await page.eval(`window.__falseFirstUse.length===0 && !document.querySelector('[data-testid=draft-empty]')`) && network.length===artBefore, 'audit: clearing a search miss shows no first-use or its artwork before the unfiltered head answers');
        gate.holdGets = false;
        await gate.answerGet(unfiltered);
        await settled();
        await page.waitFor(`document.querySelector('[data-testid=draft-empty]')`, 'clear search returns to first-use');
        check(await page.eval(`document.querySelector('#draft-search')===window.__emptySearch && document.activeElement===window.__emptySearch && window.__emptySearch.value===''`), '#259 clear keeps input identity and restores search focus');
        await page.click('#close-draft-search');
        if (!baseline) {
          await page.waitFor(`document.activeElement?.id === 'desk-title' &&
            document.querySelector('#desk-search-form').inert &&
            ['#open-draft-search', '#draft-sort'].every(selector => !document.querySelector(selector).checkVisibility())`,
            '#411 closing a blank first-use dock focuses the heading and hides search and sort');
          const { nodes } = await page.send('Accessibility.getFullAXTree');
          check(!nodes.some(node => !node.ignored && ['Tìm tác phẩm', 'Sắp xếp'].includes(node.name?.value)),
            '#411 hidden tools are absent from the accessibility tree');
          await page.key('Tab');
          check(await page.eval(`document.activeElement?.id === 'create-draft-format' &&
            document.querySelector('#draft-search') === window.__emptySearch &&
            window.__searchRemoved.length === 0`), '#411 Tab skips the hidden tools and reaches the native format Select without remounting search');
        }
      }
      const target = state === 'first-draft' ? '[data-testid=draft-empty]' : '[data-testid=draft-no-results]';
      const action = state === 'first-draft' ? '#create-draft' : '#clear-draft-search';
      if (!baseline) {
        check(await page.eval(`!!document.querySelector(${JSON.stringify(target + ' img')})`), `#259 ${state}: semantic artwork is rendered`);
        await page.eval(`document.querySelector(${JSON.stringify(target + ' img')}).decode()`);
        check(await page.eval(`(() => {const root=document.querySelector(${JSON.stringify(target)}),img=root.querySelector('img'); return root.querySelectorAll('img').length===1 && img.getAttribute('src')==='/pkg/empty/vink-empty-${state}.webp' && img.naturalWidth===512 && img.naturalHeight===427 && img.alt==='' && !root.querySelector('h2,.vk-ui-panel') && (root.querySelector('h3')?.textContent ?? '')===${JSON.stringify(state === 'first-draft' ? 'Chưa có tác phẩm nào' : '')} && document.querySelectorAll('#drafts [role=status]').length===1 && document.querySelectorAll('#create-draft').length===1;})()`), `#308 ${state}: shared artwork and one creation target, with a title for first use`);
      }
      const beforeMatrix = network.length;
      for (const width of [320, 390, 834, 1440]) for (const theme of ['light', 'dark']) for (const scale of [100, 150, 200]) {
        await page.viewport(width);
        await page.eval(`document.documentElement.dataset.theme='${theme}'; document.documentElement.style.fontSize='${scale}%'; scrollTo(0,0); document.activeElement?.blur()`);
        await page.eval(`document.fonts.ready.then(()=>new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r))))`);
        await page.eval(`Promise.all(document.getAnimations().filter(a=>Number.isFinite(a.effect.getComputedTiming().endTime)).map(a=>a.finished.catch(()=>{})))`);
        const measured = await page.eval(`(() => {
          const root=document.querySelector(${JSON.stringify(target)}), img=root.querySelector('img'),p=root.querySelector('.vk-empty-state__message');
          const r=el=>el?.getBoundingClientRect().toJSON();
          return {width:innerWidth,theme:document.documentElement.dataset.theme,scale:${scale},state:${JSON.stringify(state)},art:r(img),message:r(p),messageFont:p&&getComputedStyle(p).fontSize,scrollWidth:document.documentElement.scrollWidth,placement:${PLACEMENT}};
        })()`);
        geometry.push(measured);
        check(measured.scrollWidth <= width, `#259 ${state} ${width}/${theme}/${scale}: no horizontal overflow`);
        check(Math.abs(measured.placement.centerDelta) <= 2, `audit ${state} ${width}/${theme}/${scale}: cluster centred in the free region below the controls (delta ${measured.placement.centerDelta}px, gaps ${measured.placement.gapAbove}/${measured.placement.gapBelow})`);
        if (!baseline) check(Math.abs(measured.art.width - (width < 760 ? 192 : 240)) < 1 && Math.abs(measured.art.height / measured.art.width - 427 / 512) < .01 && measured.messageFont === (16 * scale / 100 + 'px'), `#259 ${state} ${width}/${theme}/${scale}: fixed artwork and scaled body type`);
        if (!baseline && scale===100) await textContrast(page,theme,'desk-empty');
        await page.capture(`desk-art-${state}-${width}-${theme}-${scale}`, { scenario: '#259-empty-artwork', state, target, boundary: 'intercepted-private-summary-transport', requestedScene: {width,height:900,theme,scale} });
        await page.eval(`document.querySelector(${JSON.stringify(action)}).focus(); document.querySelector(${JSON.stringify(action)}).scrollIntoView({block:'center'})`);
        check(await page.eval(`(() => {const r=document.querySelector(${JSON.stringify(action)}).getBoundingClientRect(),b=document.querySelector('.vk-ui-bottom-nav'),limit=b?.getClientRects().length?b.getBoundingClientRect().top:innerHeight; return r.top>=0 && r.bottom<=limit+1 && r.width>=44 && r.height>=44;})()`), `#259 ${state} ${width}/${theme}/${scale}: recovery reachable above navigation`);
        await page.eval(`scrollTo(0,document.documentElement.scrollHeight)`);
        check(await page.eval(`(() => {const p=document.querySelector('.vk-desk-privacy'),r=p.getBoundingClientRect(),b=document.querySelector('.vk-ui-bottom-nav'),limit=b?.getClientRects().length?b.getBoundingClientRect().top:innerHeight;return r.bottom<=limit+1 && r.top>=0 && p.textContent.includes('bản đang công khai') && document.querySelector('#heading [title="Bản nháp chỉ bạn đọc được"]');})()`), `#259 ${state} ${width}/${theme}/${scale}: list caption is reachable and the header explains draft privacy`);
        if (width < 760 && scale === 200) await page.capture(`desk-art-${state}-${width}-${theme}-${scale}-privacy`, { scenario: '#259-private-copy', state, target: '.vk-desk-privacy', boundary: 'intercepted-private-summary-transport', requestedScene: {width,height:900,theme,scale} });
      }
      if (!baseline) check(network.length===beforeMatrix, `#259 ${state}: viewport, text scale and theme changes request no additional artwork`);
    }
    await page.viewport(390);
    await page.eval(`document.documentElement.style.fontSize=''; document.documentElement.dataset.theme='light'; scrollTo(0,0)`);
    await page.eval(`Promise.all(document.getAnimations().filter(a=>Number.isFinite(a.effect.getComputedTiming().endTime)).map(a=>a.finished.catch(()=>{})))`);
    if (!baseline) {
      const manifest = JSON.parse(readFileSync('apps/vink-web/assets/empty/asset-manifest.json'));
      for (const id of ['no-results','first-draft']) {
        const asset = manifest.runtime_assets.find(a=>a.id===id);
        const response = await fetch(args.base + '/pkg/empty/vink-empty-' + id + '.webp');
        const bytes = Buffer.from(await response.arrayBuffer());
        check(response.status===200 && response.headers.get('content-type')?.startsWith('image/webp') && bytes.length===asset.bytes && createHash('sha256').update(bytes).digest('hex')===asset.sha256, `#259 ${id}: actual HTTP MIME, bytes and dependency hash`);
      }
      check(network.length <= 3 && network.filter(u=>u.endsWith('vink-empty-first-draft.webp')).length<=2 && network.filter(u=>u.endsWith('vink-empty-no-results.webp')).length<=1 && network.every(u => /vink-empty-(first-draft|no-results)\.webp$/.test(u)), `#259 only the current artwork is requested, at most once per transition (decoded-cache reuse allowed): ${JSON.stringify(network)}`);
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
      await wakeDesk(page);
      const pending = await gate.next('gets');
      check(await page.eval(`document.querySelector('[data-testid=draft-empty]') && document.querySelector('#draft-scope [data-testid=author-work-stale]') && document.querySelector('#create-draft').getAttribute('aria-disabled')!=='true'`), '#259 retained empty during refresh keeps its stale qualifier and allows create');
      await gate.fulfill(pending, 503, {error:{code:'dependency_unavailable'}});
      await settled();
      check(await page.eval(`document.querySelector('[data-testid=desk-error]') && document.querySelector('[data-testid=author-work-stale]') && !document.querySelector('#drafts img')`), '#259 load error preserves warning and is not artwork empty');
      check(await page.eval(`document.querySelector('#open-draft-search')?.checkVisibility() &&
        document.querySelector('#draft-sort')?.checkVisibility() &&
        document.querySelector('#draft-search') === window.__emptySearch &&
        window.__searchRemoved.length === 0`), '#411 a transient error keeps tools reachable and retains search');
      for (const [width, theme, scale] of [[320, 'dark', 200], [1440, 'light', 100]]) {
        await page.viewport(width);
        await page.eval(`document.documentElement.dataset.theme='${theme}'; document.documentElement.style.fontSize='${scale}%'; scrollTo(0,0); document.fonts.ready`);
        check(await page.eval(`(() => {
          const error=document.querySelector('#drafts .vk-draft-list [role=alert] .vk-empty-state');
          const retry=error?.querySelector('#retry-drafts');
          if (!retry || retry.getAttribute('aria-disabled')==='true' || document.documentElement.scrollWidth>innerWidth) return false;
          retry.scrollIntoView({block:'center'});
          const rect=retry.getBoundingClientRect(), bottom=document.querySelector('.vk-ui-bottom-nav');
          return rect.top>=0 && rect.height>=48 && rect.bottom<=(bottom?.getClientRects().length?bottom.getBoundingClientRect().top:innerHeight)+1;
        })()`), `#411 error offers a reachable retry at ${width}/${theme}/text${scale}`);
        await page.eval(`Promise.all(document.getAnimations().filter(a => Number.isFinite(a.effect.getComputedTiming().endTime)).map(a => a.finished.catch(() => {})))`);
        await textContrast(page, theme, 'desk-retry', ['#retry-drafts .vk-ui-button__label']);
        await page.capture(`desk-list-error-${width}-${theme}-${scale}`, {scenario:'#411-retained-error',target:'[data-testid=desk-error]',boundary:'intercepted-private-summary-transport'});
      }
      await page.viewport(390);
      await page.eval(`document.documentElement.style.fontSize='100%'; document.documentElement.dataset.theme='light'; scrollTo(0,0)`);
      gate.holdGets = false;
      gate.list = { raw:true, body:{drafts:[],next_cursor:'fixture:24'} };
      await page.click('#retry-drafts'); await settled();
      check(await page.eval(`!document.querySelector('[data-testid=draft-empty], [data-testid=draft-no-results], #drafts img') && document.querySelector('[data-testid=desk-error]') && document.querySelector('[data-testid=author-work-stale]')`), '#259 empty page with continuation is rejected by existing PageChain; remains an error, never first-use');
      // A proxy 502 (or 429) is `Unexpected`, not `Unavailable`: it may pass on the next try.
      // The rejected chain is `Invalid` (no panel retry); a stale visibility wake still rechecks the list.
      check(await page.eval(`!document.querySelector('[data-testid=desk-error] #retry-drafts')`), '#308 a rejected page chain has no transient error-panel retry');
      gate.holdGets = true;
      await wakeDesk(page);
      await gate.fulfill(await gate.next('gets'), 502, {error:{code:'bad_gateway'}});
      gate.holdGets = false;
      await settled();
      check(await page.eval(`!!document.querySelector('[data-testid=desk-error] #retry-drafts') && document.querySelector('#draft-search')===window.__emptySearch`), '#308 a transient 502 keeps the error-panel retry and the search input');
      await page.click('#open-draft-search');
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
        await page.eval(`document.activeElement?.blur(); scrollTo(0,0)`);
        await page.capture('desk-art-corrupt', {target:'[data-testid=draft-empty]',boundary:'one-injected-corrupt-image-response'});
      } finally {
        await page.send('Fetch.disable');
        page.cdp.listeners=page.cdp.listeners.filter(l=>l!==corruptListener);
      }
      await sessionFence(page, {args,check,gate,settled});
    }
    writeFileSync(join(dirname(args.state), 'desk-empty-results.json'), JSON.stringify({baseline,geometry,network,boundary:'Chrome-CDP; private-summary transport fixtures; root font-size text200; no real OS IME'},null,2));
  } finally {
    writeFileSync(join(dirname(args.state), 'desk-empty-network.json'), JSON.stringify({loader,network,boundary:'current document loader only; zero requests allowed for browser-decoded cache reuse'},null,2));
    await gate.disable();
    page.cdp.listeners=page.cdp.listeners.filter(l=>l!==listen);
    await page.send('Network.setBlockedURLs',{urls:[]});
    await page.send('Network.setCacheDisabled',{cacheDisabled:false});
    await page.eval(`document.documentElement.style.fontSize=''; delete document.documentElement.dataset.theme`);
  }
}

// Confirm a changed session through the real tab Session owner/BroadcastChannel.
// Auth answers are explicit transport fixtures, not a real cookie/login claim.
async function sessionFence(page, {args,check,gate,settled}) {
  let principal='99999999-9999-4999-8999-000000000259';
  let error;
  const listener = event => {
    if(event.sessionId!==page.sessionId || event.method!=='Fetch.requestPaused') return;
    const path=new URL(event.params.request.url).pathname;
    let result;
    if(path==='/api/v1/auth/session') result=gate.fulfill(event.params.requestId,200,{state:'active',principal_id:principal});
    else if(path==='/api/v1/auth/session/account') result=gate.fulfill(event.params.requestId,200,{principal_id:principal,display_name:'[Fixture #259] Changed account'});
    else result=gate.handle(event.params);
    result.catch(e=>{error=e;});
  };
  const announce=()=>page.eval(`(() => {const c=new BroadcastChannel('vink-session');c.postMessage('changed');c.close();})()`);
  page.cdp.listeners.push(listener);
  await page.send('Fetch.enable',{patterns:['/api/v1/auth/session','/api/v1/auth/session/account','/api/v1/author/works*','/api/v1/drafts'].map(path=>({urlPattern:args.base+path,requestStage:'Request'}))});
  try {
    gate.holdGets=true;
    await page.eval(`window.__emptySearch.focus();window.__emptySearch.value='PRIVATE-OLD-QUERY';window.__emptySearch.dispatchEvent(new InputEvent('input',{bubbles:true}))`);
    await page.key('Enter');
    const old=await gate.next('gets');
    await announce();
    const fresh=await gate.next('gets');
    check(new URL(gate.getURLs.get(fresh)).searchParams.get('q')==='' && await page.eval(`!window.__emptySearch.isConnected && document.querySelector('#draft-search')?.value==='' && document.querySelector('#draft-sort').value==='recent' && !document.querySelector('#drafts img')`), '#308 confirmed account change replaces the previous private input and clears query/artwork');
    await gate.fulfill(fresh,200,{drafts:[]}); await settled();
    await gate.fulfill(old,200,{drafts:[{work_id:'99999999-9999-4999-8999-000000000258',title:'PRIVATE-OLD-REPLY',version:1,updated_at:'2026-11-16T00:00:00Z'}]}).catch(()=>{});
    await page.eval(`new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)))`);
    check(await page.eval(`document.querySelector('[data-testid=draft-empty]') && !document.querySelector('#desk').textContent.includes('PRIVATE-OLD-REPLY') && document.querySelectorAll('#create-draft').length===1`), '#259 late old-session read cannot replace new-session first-use');
    await page.click('#create-draft');
    const post=await gate.next('posts'),count=gate.postCount;
    principal='99999999-9999-4999-8999-000000000260';
    await announce();
    await gate.fulfill(await gate.next('gets'),200,{drafts:[]}); await settled();
    await gate.fulfill(post,201,{work_id:'99999999-9999-4999-8999-000000000258',title:'',version:1,document:{format:'default',source:''},updated_at:'2026-11-16T00:00:00Z'});
    await page.eval(`new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)))`);
    check(gate.postCount===count && await page.eval(`location.pathname==='/ban-viet' && document.querySelectorAll('#create-draft').length===1 && document.querySelector('#create-draft').getAttribute('aria-disabled')!=='true' && document.querySelector('#create-draft').textContent==='Viết bản thảo đầu tiên'`), '#259 late successful create from old session cannot navigate, retry POST or change new first-use state');
    if(error) throw error;
  } finally {
    await page.send('Fetch.disable');
    page.cdp.listeners=page.cdp.listeners.filter(l=>l!==listener);
    // Restore server-confirmed local fixture standing for later scenarios.
    gate.holdGets=false;
    await announce();
  }
}
