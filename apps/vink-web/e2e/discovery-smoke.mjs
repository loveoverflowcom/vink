// TAG-PR-06. Real server reads unless a case explicitly says transport fixture.
// SQL publications in the dedicated phase qualify public reading, never author publication.
export async function discoveryReading(page, { args, check, textContrast }) {
  const ready = async () => page.waitFor("document.documentElement.dataset.hydrated === 'true' && document.querySelector('#public-query')", 'discovery hydration');
  const goto = async path => { await page.goto(args.base + path); await ready(); };
  const submit = async q => {
    await page.eval("document.querySelector('#public-query').focus(); document.querySelector('#public-query').select()");
    await page.send('Input.insertText', { text: q });
    await page.click('#public-search button[type=submit]');
    await page.waitFor("document.querySelector('#public-query')?.value === " + JSON.stringify(q) + " && new URL(location.href).searchParams.get('q') === " + JSON.stringify(q), 'applied query URL');
    await page.waitFor("document.querySelector('#public-results')", 'new query result');
  };
  await goto('/');
  check(await page.eval("document.querySelector('#public-query-help').textContent === 'Trong toàn thư viện công khai'"), 'search explicitly names the whole public library');
  await submit('không có tác phẩm này #100');
  await page.waitFor("document.querySelector('[data-testid=public-no-results]')", 'real no-results');
  check(await page.eval("document.querySelectorAll('#public-results [role=status]').length===1 && document.querySelector('.vk-empty-state__message').textContent==='Chưa có tác phẩm phù hợp.'"), '#225/#258 empty page has one truthful result announcement without redundant 0-count');
  check(await page.eval("!!document.querySelector('#clear-public-results')"), 'no-results offers query recovery');
  await page.waitFor("document.querySelector('.vk-empty-state__art')?.complete", '#258 empty artwork completes');
  check(await page.eval(`(async () => {
    const art = document.querySelector('.vk-empty-state__art'); await art.decode();
    const region = document.querySelector('#community-stream');
    return art.naturalWidth === 512 && art.naturalHeight === 427 && art.alt === '' &&
      art.getAttribute('width') === '512' && art.getAttribute('height') === '427' &&
      art.decoding === 'async' && art.getAttribute('loading') !== 'lazy' &&
      !region.querySelector('h2,h3') && !region.textContent.includes('Trang này: 0') &&
      region.getAttribute('aria-label') === 'Tác phẩm khám phá' && region.tabIndex === -1 &&
      performance.getEntriesByType('resource').filter(r => r.name.includes('/pkg/empty/')).length === 1;
  })()`), '#258 actual WebP decode, one image request, HTML status and named focus landmark');
  await page.waitFor("document.activeElement?.id==='community-stream'", '#258 submitted search focuses the named landmark');
  await page.key('Tab');
  check(await page.eval(`(() => {
    const link=document.querySelector('#clear-public-results'), style=getComputedStyle(link);
    return document.activeElement===link && link.matches(':focus-visible') &&
      style.outlineStyle!=='none' && parseFloat(style.outlineWidth)>0;
  })()`), '#258 keyboard reaches recovery after the landmark with a visible focus ring');
  const emptyMatrix = async surface => {
  for (const theme of ['light','dark']) for (const width of [320,390,834,1440]) for (const scale of [100,200]) {
    await page.viewport(width);
    await page.eval(`document.documentElement.dataset.theme=${JSON.stringify(theme)}; document.documentElement.style.fontSize='${scale}%'; document.fonts.ready`);
    const layout = await page.eval(`(() => {
      const art=document.querySelector('.vk-empty-state__art'), message=document.querySelector('.vk-empty-state__message');
      const r=art.getBoundingClientRect(), m=message.getBoundingClientRect();
      return {width:r.width, height:r.height, font:getComputedStyle(message).fontSize,
        overflow:document.documentElement.scrollWidth-innerWidth,
        order:r.bottom<=m.top && m.bottom<=document.querySelector('.vk-empty-state__actions').getBoundingClientRect().top};
    })()`);
    check(layout.width <= (width<760?192:240) + 1 && Math.abs(layout.height/layout.width-427/512)<0.01 &&
      layout.font===(scale===100?'16px':'32px') && layout.overflow<=1 && layout.order,
      '#258 empty composition reflow/ratio/type/order '+[theme,width,scale].join('/')+' '+JSON.stringify(layout));
    if (width===390 && scale===100) await textContrast(page, theme, 'empty');
    await page.capture('empty258-'+surface+'-'+[theme,width,scale].join('-'), {boundary:'real-public-api',scenario:'filtered-empty-WebP'});
  }
  };
  await emptyMatrix('explore');
  await emptyRefreshFocus(page, {args, check});
  await page.viewport(1440);
  await page.eval("document.documentElement.style.fontSize='100%'");

  await page.click('#clear-public-results');
  await page.waitFor("location.search === '' && document.querySelector('.vk-public-row')", 'query recovery');
  check(await page.eval("!document.querySelector('.vk-empty-state__art') && performance.getEntriesByType('resource').filter(r=>r.name.includes('/pkg/empty/')).every(r=>r.name.endsWith('vink-empty-no-results.webp'))"), '#258 ready branch mounts no artwork and never fetches unrelated illustrations');
  await page.waitFor("document.querySelector('#dock-tag-form input[type=checkbox]')", 'public tag catalog ready');

  await page.send('Input.dispatchMouseEvent',{type:'mouseMoved',x:5,y:5});
  for (const theme of ['light', 'dark']) {
    await page.eval("document.documentElement.dataset.theme="+JSON.stringify(theme));
    await textContrast(page, theme, 'explore');
  }
  for (const theme of ['light', 'dark']) for (const width of [320,390,834,1024,1200,1440]) for (const scale of [100,200]) {
    await page.viewport(width);
    await page.eval("document.documentElement.dataset.theme=" + JSON.stringify(theme) + "; document.documentElement.style.fontSize=" + JSON.stringify(scale + '%') + "; scrollTo(0,0); document.fonts.ready");
    check(await page.eval("document.documentElement.scrollWidth <= innerWidth"), 'Explore reflows ' + [theme,width,scale].join('/'));
    if ([320,390,834,1440].includes(width) && !await page.eval("document.querySelector('#tag-dock').getBoundingClientRect().width>0")) {
      check(await page.eval(`(() => {
        const button=document.querySelector('#open-tag-filters'), r=button.getBoundingClientRect();
        const icon=button.querySelector('svg[data-icon=filter-list][aria-hidden=true]');
        return button.type==='button' && button.textContent==='Thẻ' && !!icon &&
          button.querySelectorAll('svg').length===1 && button.classList.contains('vk-ui-button--tonal') &&
          icon.getBoundingClientRect().left>=r.left && icon.getBoundingClientRect().right<=r.right &&
          !button.hasAttribute('aria-pressed') && !button.querySelector('button,a,input') &&
          r.width>=48 && r.height>=48 && r.left>=0 && r.right<=innerWidth;
      })()`), '#225 filter launcher keeps one named native target >=48px '+[theme,width,scale].join('/'));
    }
    check(await page.eval("(() => { const heading=document.querySelector('#explore-heading h1'), walker=document.createTreeWalker(heading,NodeFilter.SHOW_TEXT); for(let node=walker.nextNode();node;node=walker.nextNode()) for(const word of node.textContent.matchAll(/\\S+/g)) { const range=document.createRange(); range.setStart(node,word.index); range.setEnd(node,word.index+word[0].length); const rects=[...range.getClientRects()]; if(rects.some(r=>Math.abs(r.top-rects[0].top)>1)) return false; } return true; })()"), 'Vietnamese heading words remain intact '+[theme,width,scale].join('/'));

    const dock = await page.eval("document.querySelector('#tag-dock').getBoundingClientRect().width>0");
    check(dock === (width>=1200 && scale===100), 'dock/dialog breakpoint ' + [theme,width,scale].join('/'));

    check(await page.eval("(() => { const row=document.querySelector('.vk-public-row'), t=row.querySelector('h3').getBoundingClientRect(), n=row.querySelector('.vk-ui-byline__name').getBoundingClientRect(), a=row.querySelector('.vk-ui-avatar').getBoundingClientRect(); return t.bottom<=n.top && Math.abs(a.width-32)<1 && Math.abs(a.height-32)<1; })()"), 'title first and avatar retained ' + [theme,width,scale].join('/'));
    if (width === 320 && scale === 200) {
      const card = await page.eval(`(() => {
        const row = document.querySelector('.vk-public-row');
        const box = selector => row.querySelector(selector).getBoundingClientRect();
        const byline = box('.vk-work-byline'), name = box('.vk-ui-byline__name');
        const date = box('time'), avatar = box('.vk-ui-avatar');
        return {
          nameWidth: name.width, dateWidth: date.width, bylineWidth: byline.width,
          ordered: avatar.bottom <= name.top && name.bottom <= date.top,
          font: getComputedStyle(row.querySelector('.vk-ui-byline__name')).fontSize,
          overflow: document.documentElement.scrollWidth - document.documentElement.clientWidth,
        };
      })()`);
      check(
        Math.abs(card.nameWidth - card.bylineWidth) <= 1 &&
          Math.abs(card.dateWidth - card.bylineWidth) <= 1 && card.ordered &&
          card.font === '32px' && card.overflow <= 1,
        `320px / 200% ${theme}: the full-size byline uses the card measure below the avatar (${JSON.stringify(card)})`,
      );
    }
    if ([320,390,834,1440].includes(width)) await page.capture('discovery-' + [theme,width,scale].join('-'), { boundary: 'real-public-api', scenario: 'TAG-PR-06' });
  }
  await page.viewport(390);
  await page.eval("document.documentElement.style.fontSize='100%'");
  await page.click('#open-tag-filters');
  await page.waitFor("document.querySelector('#tag-picker')?.open && document.querySelector('#modal-tag-form input[type=checkbox]')", 'tag picker');
  const captureSelection = async count => {
    for (const [width,scale,theme] of [[390,100,'light'],[320,200,'dark']]) {
      await page.viewport(width);
      await page.eval(`document.documentElement.dataset.theme=${JSON.stringify(theme)}; document.documentElement.style.fontSize='${scale}%'; document.fonts.ready`);
      // Theme changes animate action colors; inspect the settled surface, not an in-flight frame.
      await page.eval("new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)))");
      await page.eval("Promise.all(document.getAnimations().filter(a=>Number.isFinite(a.effect.getComputedTiming().endTime)).map(a=>a.finished.catch(()=>{})))");
      await textContrast(page, theme, 'filter');
      check(await page.eval(`(() => {
        const form=document.querySelector('#modal-tag-form'), help=document.getElementById(form.getAttribute('aria-describedby'));
        return help.textContent==='Khớp tất cả thẻ · Tối đa 4' && help.getBoundingClientRect().width>0 &&
          form.querySelectorAll('input:checked').length===${count} && document.documentElement.scrollWidth<=innerWidth;
      })()`), '#225 helper retains AND/max4 and selection at '+[count,width,scale,theme].join('/'));
      await page.capture('action225-filter-'+[count,width,scale,theme].join('-'), {boundary:'real-public-api',scenario:'tag-draft-selection'});
    }
    await page.viewport(390);
    await page.eval("document.documentElement.style.fontSize='100%'");
  };
  await captureSelection(0);
  const before = await page.eval('location.href');
  check(await page.eval(`(() => {
    const form=document.querySelector('#modal-tag-form'), apply=form.querySelector('button[type=submit]');
    return form.method==='get' && form.querySelectorAll('button[type=submit]').length===1 &&
      apply.textContent==='Áp dụng' && apply.classList.contains('vk-ui-button--tonal') &&
      !apply.querySelector('button,a,input') && !form.querySelector('[name=cursor]') &&
      [...form.querySelectorAll('input[type=checkbox]')].every(i=>i.name==='tag' && !!i.labels?.length);
  })()`), '#225 unchanged Apply stays Tonal; one native GET submit, labelled checkbox choices and no cursor');
  // Count outside the page: a native GET may replace its Window and erase page globals.
  let applySubmits = 0;
  await page.send('Runtime.addBinding', {name:'action225Submit'});
  const countApply = message => {
    if (message.sessionId===page.sessionId && message.method==='Runtime.bindingCalled' &&
        message.params.name==='action225Submit') applySubmits++;
  };
  page.cdp.listeners.push(countApply);
  const choices = await page.eval(`(() => {
    const form=document.querySelector('#modal-tag-form');
    window.__action225Choice=form.querySelector('input[type=checkbox]');
    window.__action225Changes=0; window.__action225Submits=0;
    form.addEventListener('change',()=>window.__action225Changes++);
    document.addEventListener('submit',e=>{if(e.target.id==='modal-tag-form'){window.__action225Submits++;window.action225Submit('apply')}},true);
    return [...form.querySelectorAll('input[type=checkbox]')].slice(0,4).map(i=>i.id);
  })()`);
  await page.eval('window.__action225Choice.focus()');
  await page.key('Space');
  check(await page.eval(`window.__action225Changes===1 && window.__action225Submits===0 &&
    window.__action225Choice===document.getElementById(${JSON.stringify(choices[0])}) &&
    document.querySelector('#modal-tag-form button[type=submit]').classList.contains('vk-ui-button--filled')`),
    '#225 one Space changes one stable checkbox, never submits; changed Apply is Filled');
  await captureSelection(1);
  check(await page.eval('location.href') === before, 'draft selection does not change applied URL');
  check(choices.length===4, '#225 catalog has at least four real choices for the max4 browser check');
  for (const id of choices.slice(1)) await page.click('label:has(#'+id+')');
  check(await page.eval(`document.querySelectorAll('#modal-tag-form input[type=checkbox]:checked').length===4 &&
    document.querySelectorAll('#modal-tag-form input[type=checkbox]:not(:checked)').length>0 &&
    [...document.querySelectorAll('#modal-tag-form input[type=checkbox]:not(:checked)')].every(i=>i.disabled) &&
    [...document.querySelectorAll('#modal-tag-form input[type=checkbox]:checked')].every(i=>!i.disabled)`),
    '#225 max4 blocks extra choices while selected choices remain removable');
  await page.click('#modal-tag-form label:has(input:disabled)');
  check(await page.eval("document.querySelectorAll('#modal-tag-form input:checked').length===4 && window.__action225Submits===0"), '#225 attempting a fifth tag cannot select or submit');
  await captureSelection(4);
  for (const id of choices.slice(1)) await page.click('label:has(#'+id+')');
  await page.click('label:has(#'+choices[0]+')');
  check(await page.eval(`document.querySelectorAll('#modal-tag-form input[type=checkbox]:checked').length===0 &&
    document.querySelector('#modal-tag-form button[type=submit]').classList.contains('vk-ui-button--tonal')`),
    '#225 reverting to the applied set restores Tonal without a submit');
  await page.click('label:has(#'+choices[0]+')');
  await page.key('Escape');
  await page.waitFor("!document.querySelector('#tag-picker').open && document.activeElement.id === 'open-tag-filters'", 'Escape restores filter trigger');
  await page.click('#open-tag-filters');
  await page.waitFor("document.querySelector('#tag-picker').open && document.querySelectorAll('#modal-tag-form input[type=checkbox]:checked').length===1 && document.querySelector('#modal-tag-form button[type=submit]')?.classList.contains('vk-ui-button--filled')", 'reopen picker with the refreshed catalog and draft choice ready');
  await page.eval("document.querySelector('#modal-tag-form button[type=submit]').focus()");
  await page.key('Enter');
  await page.waitFor("new URL(location.href).searchParams.has('tag') && document.querySelector('#public-results') && !document.querySelector('#tag-picker').open", 'Apply changes the URL');
  check(applySubmits===1, '#225 Enter on Apply submits exactly once, including native GET navigation (observed '+applySubmits+')');
  page.cdp.listeners = page.cdp.listeners.filter(listener=>listener!==countApply);
  await page.send('Runtime.removeBinding', {name:'action225Submit'});
  check(await page.eval("document.querySelectorAll('#applied-filters .vk-applied-tag').length === 1"), 'applied filter is visible and removable');
  await page.click('#open-tag-filters');
  await page.waitFor("document.querySelector('#tag-picker').open && document.querySelector('#modal-tag-form button[type=submit]')?.classList.contains('vk-ui-button--tonal')", '#225 applied set is unchanged after navigation');
  await page.key('Escape');
  await page.waitFor("!document.querySelector('#tag-picker').open && document.activeElement.id==='open-tag-filters'", '#225 unchanged filter dismiss restores trigger');
  await page.click('#applied-filters .vk-applied-tag');
  await page.waitFor("!new URL(location.href).searchParams.has('tag') && document.querySelector('.vk-public-row')", 'remove filter');

  // IME composition is simulated through CDP (`Input.imeSetComposition`); it is not a named OS IME
  // session. Keys are trusted (`Input.dispatchKeyEvent`): unlike a dispatched DOM event they do
  // trigger the form's implicit submission, so a missing guard would navigate here.
  const old = await page.eval('location.href');
  await page.eval("(() => { const i=document.querySelector('#public-query'); i.focus(); i.select(); })()");
  await page.send('Input.imeSetComposition', { text: 'mư', selectionStart: 2, selectionEnd: 2 });
  await page.key('Enter');
  await new Promise((resolve) => setTimeout(resolve, 400));
  check(await page.eval('location.href') === old, 'a trusted Enter while composing does not commit a partial search');
  await page.send('Input.insertText', { text: 'mưa' });
  await page.key('Enter');
  await page.waitFor("new URL(location.href).searchParams.get('q') === 'mưa'", 'the committed text applies with Enter');
  check(true, 'after the commit the same Enter applies the search (the guard only holds composition)');

  await goto('/?q=mua');
  await page.waitFor("document.querySelector('.vk-public-row__link')", 'real title search');
  const returnPath = await page.eval('location.pathname + location.search');
  await page.click('.vk-public-row__link');
  await page.waitFor("document.querySelector('#reader-return') && document.querySelector('#reader-paper')", 'reader from search');
  check(await page.eval("document.querySelector('#reader-return').getAttribute('href').startsWith(" + JSON.stringify(returnPath) + ")"), 'reader return preserves applied query');
  check(await page.eval("Math.abs(document.querySelector('#reader-header .vk-ui-avatar').getBoundingClientRect().width-44)<1"), 'reader avatar is exactly 44px');
  await page.capture('discovery-reader', { boundary: 'real-public-api', scenario: 'reader-return' });
  for (const width of [320, 390]) {
    await page.viewport(width, 480);
    const reader = await page.eval(`(async () => {
      document.documentElement.style.fontSize = '200%';
      await document.fonts.ready;
      const header = document.querySelector('#reader-header'), body = document.querySelector('.vk-document');
      return { gap: body.getBoundingClientRect().top - header.getBoundingClientRect().bottom,
        font: getComputedStyle(body).fontSize, context: header.textContent.includes('Bản công khai') };
    })()`);
    check(reader.gap > 0 && reader.gap <= 25 && reader.font === '34px' && reader.context,
      `${width}px / 200%: untagged Reader reaches the full-size text within the header spacing budget (${JSON.stringify(reader)})`);
  }
  await page.viewport(390);
  await page.eval("document.documentElement.style.fontSize='100%'");
  await page.click('#reader-return');
  await page.waitFor("document.activeElement?.matches('.vk-public-row__link')", 'reader return focuses the opened result');

  const catalog = await (await fetch(args.base + '/api/v1/tags')).json();
  const tag = catalog.tags.find(t => t.label === 'Ký ức');
  const route = '/the/' + encodeURIComponent(tag.slug);
  const response = await fetch(args.base + route);
  const html = await response.text();
  check(response.status === 200 && html.includes(tag.label) && html.includes(tag.description), 'tag detail SSR uses canonical catalog name and description');
  await goto(route);
  check(await page.eval("document.querySelector('#navigation-primary a[href=\"/\"]')?.getAttribute('aria-current')==='page'"), 'tag detail belongs to Explore');
  await goto(route+'?q=no-match-empty-258');
  await page.waitFor("document.querySelector('.vk-empty-state__art')?.complete && document.querySelector('.vk-empty-state__art')?.naturalWidth===512", '#258 tag empty artwork');
  check(await page.eval("document.querySelectorAll('#public-results [role=status]').length===1 && !document.querySelector('#community-stream h2,#community-stream h3') && document.querySelector('.vk-empty-state__message').textContent==='Chưa có tác phẩm phù hợp.'"), '#258 tag empty keeps one concise status without heading stack');
  await emptyMatrix('tag');
  await page.viewport(1440);
  await page.eval("document.documentElement.style.fontSize='100%'");

  const alias = await fetch(args.base + '/the/' + encodeURIComponent('hồi-ức'), { redirect:'manual' });
  check(alias.status === 308 && alias.headers.get('location')?.startsWith('/the/'), 'alias tag route redirects using server canonical identity');
  for (const spelling of [tag.slug.toUpperCase(), tag.slug.normalize('NFD')]) {
    const normalized = await fetch(args.base+'/the/'+encodeURIComponent(spelling), {redirect:'manual'});
    check(normalized.status===308 && normalized.headers.get('location')==='/the/'+encodeURIComponent(tag.slug), 'uppercase/NFD uses the provider canonical tag URL');
  }
  const unknown = await fetch(args.base + '/the/khong-ton-tai-100');
  const unknownHtml = await unknown.text();
  check(unknown.status === 404 && unknownHtml.includes('Không tìm thấy thẻ này') && !unknownHtml.includes('/pkg/empty/'), 'unknown tag has its own SSR 404 without absence artwork');
  const invalid = await fetch(args.base + '/?cursor=broken');
  check(invalid.status === 422 && (await invalid.text()).includes('Thử từ trang đầu'), 'invalid cursor is recoverable SSR 422');
  await page.viewport(320, 480);
  await goto('/?cursor=broken');
  check(await page.eval("!document.querySelector('.vk-empty-state__art')"), '#258 invalid cursor is an error, without artwork');
  check(await page.eval(`(async () => {
    document.documentElement.style.fontSize = '200%';
    await document.fonts.ready;
    const restart = document.querySelector('#restart-public'), clear = restart.nextElementSibling;
    return clear.matches('a[href="/"]') &&
      clear.getBoundingClientRect().top - restart.getBoundingClientRect().bottom >= 7;
  })()`), 'invalid cursor recovery links have distinct rows at 320px / 200%');
  await page.viewport(1440);

  // Native browser GET, with page JS disabled and WASM preloads blocked.
  // A decoded image from an earlier matrix must not bypass either failure fixture.
  await page.send('Network.setCacheDisabled', {cacheDisabled:true});
  await page.send('Network.clearBrowserCache');
  await page.send('Network.setBlockedURLs', { urls:[args.base+'/pkg/vink.js*',args.base+'/pkg/vink_bg.wasm*',args.base+'/pkg/empty/*'] });
  await page.send('Emulation.setScriptExecutionDisabled', { value:true });
  try {
    await page.goto(args.base + '/?q=mua');
    check(await page.eval("document.getElementById(document.querySelector('#dock-tag-form').getAttribute('aria-describedby')).textContent==='Khớp tất cả thẻ · Tối đa 4'"), '#225 no-JS filter explains AND/max4 before choosing');
    check(await page.eval("document.querySelector('.vk-public-row__link').href.includes('?q=mua')"), 'no-JS SSR supplies contextual reader links');
    await page.eval("document.querySelector('#public-query').value='khong-co-ket-qua-100'");
    const point = await page.eval("(() => { const b=document.querySelector('#public-search button[type=submit]'); b.scrollIntoView(); const r=b.getBoundingClientRect(); return {x:r.x+r.width/2,y:r.y+r.height/2}; })()");
    for (const type of ['mousePressed','mouseReleased']) await page.send('Input.dispatchMouseEvent', {type,...point,button:'left',clickCount:1});
    await page.waitFor("new URL(location.href).searchParams.get('q')==='khong-co-ket-qua-100' && document.querySelector('[data-testid=public-no-results]')", 'native GET result');
    await page.waitFor("document.querySelector('.vk-empty-state__art')?.complete", '#258 blocked artwork finishes with failure');
    check(await page.eval("document.querySelector('.vk-empty-state__art').naturalWidth===0 && document.documentElement.dataset.hydrated!=='true' && document.querySelector('.vk-empty-state__message').textContent==='Chưa có tác phẩm phù hợp.' && document.querySelector('#clear-public-results').getAttribute('href')==='/' && document.querySelectorAll('#public-results [role=status]').length===1"), '#258 blocked artwork before hydration leaves useful HTML and GET recovery');
    // page.click waits for animation frames; those callbacks do not run with page JS disabled.
    // Use the same native pointer path as the no-JS search above, without a JS click fallback.
    for (const selector of ['#dock-tag-form label:has(input[type=checkbox])', '#dock-tag-form button[type=submit]']) {
      const target = await page.eval(`(() => { const el=document.querySelector(${JSON.stringify(selector)}); el.scrollIntoView(); const r=el.getBoundingClientRect(); const x=r.x+r.width/2,y=r.y+r.height/2; return {x,y,hit:el.contains(document.elementFromPoint(x,y))}; })()`);
      check(target.hit, '#225 no-JS pointer hits the native filter target '+selector);
      for (const type of ['mousePressed','mouseReleased']) await page.send('Input.dispatchMouseEvent',{type,x:target.x,y:target.y,button:'left',clickCount:1});
    }
    await page.waitFor("new URL(location.href).searchParams.get('q')==='khong-co-ket-qua-100' && new URL(location.href).searchParams.getAll('tag').length===1 && !new URL(location.href).searchParams.has('cursor')", '#225 no-JS checkbox + Apply preserves q/tag through native GET and resets cursor');
    // A corrupt response before hydration must not own any state meaning or recovery.
    let corruptRequests=0;
    const corruptArt = ({sessionId,method,params}) => {
      if (sessionId===page.sessionId && method==='Fetch.requestPaused') {
        corruptRequests++;
        page.send('Fetch.fulfillRequest', {requestId:params.requestId,responseCode:200,
          responseHeaders:[{name:'Content-Type',value:'image/webp'},{name:'Cache-Control',value:'no-store'}],
          body:Buffer.from('invalid WebP fixture #258').toString('base64')}).catch(()=>{});
      }
    };
    page.cdp.listeners.push(corruptArt);
    await page.send('Network.setBlockedURLs', {urls:[args.base+'/pkg/vink.js*',args.base+'/pkg/vink_bg.wasm*']});
    await page.send('Fetch.enable', {patterns:[{urlPattern:args.base+'/pkg/empty/*',requestStage:'Request'}]});
    try {
      await page.goto(args.base+'/?q=corrupt-art-258');
      await page.waitFor("document.querySelector('.vk-empty-state__art')?.complete", '#258 corrupt image completes with failure');
      check(corruptRequests===1, '#258 exactly one corrupt artwork response was actually injected');
      check(await page.eval(`(() => {
        const art=document.querySelector('.vk-empty-state__art'), box=art.getBoundingClientRect();
        return art.naturalWidth===0 && box.width>0 && Math.abs(box.height/box.width-427/512)<0.01 &&
          document.querySelector('.vk-empty-state__message').textContent==='Chưa có tác phẩm phù hợp.' &&
          document.querySelector('#clear-public-results').getAttribute('href')==='/' &&
          document.querySelectorAll('#public-results [role=status]').length===1;
      })()`), '#258 corrupt image before hydration keeps reserved space, one HTML status and real recovery');
    } finally {
      await page.send('Fetch.disable');
      page.cdp.listeners=page.cdp.listeners.filter(listener=>listener!==corruptArt);
    }

  } finally {
    await page.send('Emulation.setScriptExecutionDisabled',{value:false});
    await page.send('Network.setBlockedURLs',{urls:[]});
    await page.send('Network.setCacheDisabled',{cacheDisabled:false});
  }
  await goto('/');
  // The always-visible rule must survive a failed catalog refresh as well as no-JS.
  const catalogFailure = ({sessionId,method,params}) => {
    if (sessionId===page.sessionId && method==='Fetch.requestPaused') {
      page.send('Fetch.fulfillRequest', {requestId:params.requestId,responseCode:503,
        responseHeaders:[{name:'Content-Type',value:'application/json'}],
        body:Buffer.from(JSON.stringify({error:{code:'dependency_unavailable',message:'catalog fixture'}})).toString('base64')}).catch(()=>{});
    }
  };
  await page.viewport(390);
  page.cdp.listeners.push(catalogFailure);
  await page.send('Fetch.enable', {patterns:[{urlPattern:args.base+'/api/v1/tags*',requestStage:'Request'}]});
  try {
    await page.click('#open-tag-filters');
    await page.waitFor("document.querySelector('#modal-tag-form [role=alert]')", '#225 catalog refresh failure');
    check(await page.eval("document.querySelector('#modal-tag-help').textContent==='Khớp tất cả thẻ · Tối đa 4' && document.querySelector('#modal-tag-form [role=alert]').textContent.includes('Chưa tải được danh mục thẻ')"), '#225 catalog failure keeps both the rule and an honest error');
    await page.key('Escape');
    await page.waitFor("!document.querySelector('#tag-picker').open && document.activeElement.id==='open-tag-filters'", '#225 failed-catalog dismiss restores trigger');
  } finally {
    await page.send('Fetch.disable');
    page.cdp.listeners = page.cdp.listeners.filter(listener=>listener!==catalogFailure);
  }
  await goto('/?q=missing-return-258#work-00000000-0000-4000-8000-000000000258');
  await page.waitFor("document.activeElement?.id==='community-stream'", '#258 missing reader-return row focuses the named results landmark');
  check(await page.eval("document.activeElement.getAttribute('aria-label')==='Tác phẩm khám phá' && document.activeElement.getClientRects().length>0"), '#258 reader-return fallback is a visible, named target');
  await goto('/');
}

// Revalidation changes freshness text, not the identity of focused recovery controls.
async function emptyRefreshFocus(page, {args, check}) {
  const held = [];
  const gate = ({sessionId, method, params}) => {
    if (sessionId === page.sessionId && method === 'Fetch.requestPaused') held.push(params.requestId);
  };
  page.cdp.listeners.push(gate);
  await page.send('Fetch.enable', {patterns:[{urlPattern:args.base+'/api/v1/works*', requestStage:'Request'}]});
  try {
    await page.eval(`(() => {
      const recovery = document.querySelector('#clear-public-results');
      window.__empty258 = { recovery, art: document.querySelector('.vk-empty-state__art'), now: performance.now.bind(performance) };
      recovery.focus();
      performance.now = () => window.__empty258.now() + 121000;
      window.dispatchEvent(new Event('focus'));
    })()`);
    const deadline = Date.now()+10000;
    while (!held.length && Date.now()<deadline) await new Promise(resolve=>setTimeout(resolve, 25));
    check(held.length===1, '#258 empty revalidation is actually held');
    await page.waitFor("document.querySelector('#public-collection').dataset.refreshing==='true'", '#258 retained empty refreshing');
    const stable = () => page.eval(`document.querySelector('#clear-public-results')===window.__empty258.recovery &&
      document.activeElement===window.__empty258.recovery && document.querySelector('.vk-empty-state__art')===window.__empty258.art &&
      document.querySelector('.vk-empty-state__message').textContent==='Bản đã tải: Chưa có tác phẩm phù hợp.' &&
      document.querySelectorAll('#public-results [role=status]').length===1`);
    check(await stable(), '#258 retained empty refresh preserves recovery focus, artwork node and one qualified status');
    await page.send('Fetch.fulfillRequest', {requestId:held.shift(), responseCode:503,
      responseHeaders:[{name:'Content-Type',value:'application/json'},{name:'Retry-After',value:'30'}],
      body:Buffer.from(JSON.stringify({error:{code:'dependency_unavailable'}})).toString('base64')});
    await page.waitFor("document.querySelector('[data-testid=public-refresh-failed]') && document.querySelector('#public-collection').dataset.refreshing==='false'", '#258 retained empty refresh failure');
    check(await stable(), '#258 failed empty refresh preserves recovery focus and truthful absence qualifier');
  } finally {
    for (const requestId of held) await page.send('Fetch.failRequest', {requestId,errorReason:'Aborted'}).catch(()=>{});
    await page.send('Fetch.disable');
    page.cdp.listeners=page.cdp.listeners.filter(listener=>listener!==gate);
    await page.eval("if(window.__empty258){performance.now=window.__empty258.now; delete window.__empty258}");
  }
}

export async function discoveryCorpus(page, { args, check }) {
  const load = async path => (await fetch(args.base+path)).json();
  const catalog = await load('/api/v1/tags');
  const form = catalog.tags.find(t=>t.label==='Thơ'), theme = catalog.tags.find(t=>t.label==='Ký ức');
  const initial = await load('/api/v1/works');
  check(initial.works.length === 12 && initial.next_cursor, 'real corpus has a bounded first page and a real cursor');
  const found = await load('/api/v1/works?q=ben%20hiem%20sau%20trang%20dau');
  check(found.works.length===1 && !initial.works.some(w=>w.work_id===found.works[0].work_id), 'whole-library search finds a work beyond the first 12');
  const tags = '&tag='+encodeURIComponent(form.slug)+'&tag='+encodeURIComponent(theme.slug);
  await page.goto(args.base+'/?q=kiem%20thu'+tags);
  await page.waitFor("document.documentElement.dataset.hydrated==='true' && document.querySelector('#public-next')", 'real filtered page');
  check(await page.eval("document.querySelectorAll('.vk-public-row').length===12"), 'first filtered page renders 12 real rows');
  await page.click('#public-next');
  await page.waitFor("new URL(location.href).searchParams.has('cursor') && document.querySelectorAll('.vk-public-row').length===2", 'next real page');
  check(await page.eval("document.querySelectorAll('#public-results [role=status]').length===1 && document.querySelector('.vk-public-scope').textContent==='Trang này: 2 tác phẩm'"), '#225 final-page count stays visible and never claims a library total');
  const context = await page.eval('location.pathname+location.search');
  check(await page.eval("document.querySelector('#public-next')===null && [...document.querySelectorAll('.vk-public-row__link')].every(a=>a.hasAttribute('href'))"), '#225 continuation ends honestly and results remain native links');
  await page.eval('history.back()');
  await page.waitFor("!new URL(location.href).searchParams.has('cursor') && document.querySelectorAll('.vk-public-row').length===12", '#225 Back restores filtered first page');
  await page.eval('history.forward()');
  await page.waitFor("location.pathname+location.search==="+JSON.stringify(context)+" && document.querySelectorAll('.vk-public-row').length===2", '#225 Forward restores q/tags/cursor and second-page results');
  await page.click('.vk-public-row__link');
  await page.waitFor("document.querySelector('#reader-paper')", 'real fixture reader');
  check(await page.eval("document.querySelector('#reader-return').getAttribute('href').startsWith("+JSON.stringify(context)+")"), 'reader retains query, both tags and opaque cursor');
  check(await page.eval("document.querySelectorAll('.vk-reader-tags a').length===2"), 'reader links the public tags supplied by the snapshot');
  await page.click('#reader-return');
  await page.waitFor("document.activeElement?.matches('.vk-public-row__link')", 'second-page focus restoration');
  await page.reload();
  check(await page.eval('location.pathname+location.search')===context, 'reload preserves filtered second-page URL');
  await page.capture('discovery-real-second-page', {boundary:'real-api+SQL-publication-fixture',scenario:'query-tags-pagination-reader-return'});
  await page.viewport(1440);
  await page.waitFor("document.querySelector('#dock-tag-form input[type=checkbox]:checked')", '#225 dock with applied real tags');
  const appliedChoice = '#dock-tag-form label:has(input[value='+JSON.stringify(form.slug)+'])';
  await page.click(appliedChoice);
  check(await page.eval("document.querySelector('#dock-tag-form button[type=submit]').classList.contains('vk-ui-button--filled')"), '#225 removing an applied choice makes Apply Filled');
  await page.click(appliedChoice);
  check(await page.eval("document.querySelector('#dock-tag-form button[type=submit]').classList.contains('vk-ui-button--tonal') && location.pathname+location.search==="+JSON.stringify(context)), '#225 the same applied set in a different selection order is Tonal without URL mutation');
  await page.click(appliedChoice);
  await page.click('#dock-tag-form button[type=submit]');
  await page.waitFor("!new URL(location.href).searchParams.has('cursor') && new URL(location.href).searchParams.get('q')==='kiem thu' && new URL(location.href).searchParams.getAll('tag').length===1 && document.querySelector('#public-results')", '#225 changed Apply resets second-page cursor and keeps q plus remaining tag');

  const tagContext='/the/'+encodeURIComponent(form.slug)+'?q=kiem%20thu&tag='+encodeURIComponent(theme.slug);
  await page.goto(args.base+tagContext);
  await page.waitFor("document.documentElement.dataset.hydrated==='true' && document.querySelector('.vk-public-row__link')", 'real tag page');
  await page.viewport(1440);
  await page.capture('discovery-real-tag-desktop', {boundary:'real-api+SQL-publication-fixture',scenario:'tag-detail-with-results'});
  await page.click('.vk-public-row__link');
  await page.waitFor("document.querySelector('#reader-return')", 'reader from tag page');
  check(await page.eval("document.querySelector('#reader-return').getAttribute('href').startsWith("+JSON.stringify(tagContext)+")"), 'reader returns to the same tag page and extra filters');
  await page.click('#reader-return');
  await page.waitFor("document.activeElement?.matches('.vk-public-row__link')", 'tag-page focus restored');

  // Orchestrated transport race: A is held while B completes. No persistence claim.
  const held=[];
  const gate = ({sessionId,method,params}) => {
    if(sessionId!==page.sessionId || method!=='Fetch.requestPaused') return;
    if((new URL(params.request.url).searchParams.get('q')==='race-100-a' && !new URL(params.request.url).searchParams.has('tag')) || new URL(params.request.url).searchParams.get('q')==='race-100-c') held.push(params.requestId);
    else page.send('Fetch.continueRequest',{requestId:params.requestId}).catch(()=>{});
  };
  page.cdp.listeners.push(gate);
  await page.send('Fetch.enable',{patterns:[{urlPattern:args.base+'/api/v1/works*',requestStage:'Request'}]});
  const navigate = async href => page.eval("(() => {const a=document.createElement('a');a.href="+JSON.stringify(href)+";document.body.append(a);a.click();a.remove();})()");
  try {
    await navigate('/?q=race-100-a');
    await page.waitFor("location.search==='?q=race-100-a'",'A navigation');
    const deadline=Date.now()+10000;
    while(!held.length && Date.now()<deadline) await new Promise(r=>setTimeout(r,50));
    check(held.length===1,'older query request is held deterministically');
    check(await page.eval("!document.querySelector('.vk-empty-state__art')"), '#258 unresolved read never mounts absence artwork');
    await page.waitFor("document.querySelector('#dock-tag-form input[type=checkbox]')", 'filter controls available while A is pending');
    await page.click('#dock-tag-form .vk-discovery-option');
    await page.click('#dock-tag-form button[type=submit]');
    await page.waitFor("new URL(location.href).searchParams.has('tag') && document.querySelector('[data-testid=public-no-results]')",'B filter completes while A remains held');
    const body={...found,filters:{q:'race-100-a',tags:[]},works:[{...found.works[0],title:'STALE RACE RESULT #100'}]};
    await page.send('Fetch.fulfillRequest',{requestId:held.shift(),responseCode:200,responseHeaders:[{name:'Content-Type',value:'application/json'},{name:'Cache-Control',value:'no-store'}],body:Buffer.from(JSON.stringify(body)).toString('base64')});
    await page.eval('new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)))');
    check(await page.eval("new URL(location.href).searchParams.has('tag') && !document.querySelector('#public-results').textContent.includes('STALE RACE RESULT')"),'late A cannot overwrite B or acquire B filter labels');
    await navigate('/?q=race-100-c');
    const until=Date.now()+10000;
    while(!held.length && Date.now()<until) await new Promise(r=>setTimeout(r,50));
    check(held.length===1,'C is held before entering a new draft query');
    await page.eval("document.querySelector('#public-query').focus(); document.querySelector('#public-query').select()");
    await page.send('Input.insertText',{text:'chữ đang gõ chưa áp dụng'});
    await page.send('Fetch.fulfillRequest',{requestId:held.shift(),responseCode:200,responseHeaders:[{name:'Content-Type',value:'application/json'},{name:'Cache-Control',value:'no-store'}],body:Buffer.from(JSON.stringify({works:[],filters:{q:'race-100-c',tags:[]},next_cursor:null})).toString('base64')});
    await page.waitFor("document.querySelector('[data-testid=public-no-results]')", 'C result ready');
    check(await page.eval("document.querySelector('#public-query').value==='chữ đang gõ chưa áp dụng' && document.activeElement?.id==='public-query' && document.querySelector('#public-query').selectionStart==='chữ đang gõ chưa áp dụng'.length && new URL(location.href).searchParams.get('q')==='race-100-c'"),'a result arriving during typing preserves the unapplied query buffer');
  } finally {
    for(const requestId of held) await page.send('Fetch.failRequest',{requestId,errorReason:'Aborted'}).catch(()=>{});
    await page.send('Fetch.disable');
    page.cdp.listeners.splice(page.cdp.listeners.indexOf(gate),1);
  }
}
