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
  check(await page.eval("!!document.querySelector('#clear-public-results')"), 'no-results offers query recovery');
  await page.click('#clear-public-results');
  await page.waitFor("location.search === '' && document.querySelector('.vk-public-row')", 'query recovery');
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
    check(await page.eval("(() => { const heading=document.querySelector('#explore-heading h1'), walker=document.createTreeWalker(heading,NodeFilter.SHOW_TEXT); for(let node=walker.nextNode();node;node=walker.nextNode()) for(const word of node.textContent.matchAll(/\\S+/g)) { const range=document.createRange(); range.setStart(node,word.index); range.setEnd(node,word.index+word[0].length); const rects=[...range.getClientRects()]; if(rects.some(r=>Math.abs(r.top-rects[0].top)>1)) return false; } return true; })()"), 'Vietnamese heading words remain intact '+[theme,width,scale].join('/'));

    const dock = await page.eval("document.querySelector('#tag-dock').getBoundingClientRect().width>0");
    check(dock === (width>=1200 && scale===100), 'dock/dialog breakpoint ' + [theme,width,scale].join('/'));

    check(await page.eval("(() => { const row=document.querySelector('.vk-public-row'), t=row.querySelector('h3').getBoundingClientRect(), n=row.querySelector('.vk-ui-byline__name').getBoundingClientRect(), a=row.querySelector('.vk-ui-avatar').getBoundingClientRect(); return t.bottom<=n.top && Math.abs(a.width-32)<1 && Math.abs(a.height-32)<1; })()"), 'title first and avatar retained ' + [theme,width,scale].join('/'));
    if ([390,1440].includes(width)) await page.capture('discovery-' + [theme,width,scale].join('-'), { boundary: 'real-public-api', scenario: 'TAG-PR-06' });
  }
  await page.viewport(390);
  await page.eval("document.documentElement.style.fontSize='100%'");
  await page.click('#open-tag-filters');
  await page.waitFor("document.querySelector('#tag-picker')?.open && document.querySelector('#modal-tag-form input[type=checkbox]')", 'tag picker');
  const before = await page.eval('location.href');
  await page.click('#modal-tag-form .vk-discovery-option');
  check(await page.eval('location.href') === before, 'draft selection does not change applied URL');
  await page.key('Escape');
  await page.waitFor("!document.querySelector('#tag-picker').open && document.activeElement.id === 'open-tag-filters'", 'Escape restores filter trigger');
  await page.click('#open-tag-filters');
  await page.waitFor("document.querySelector('#tag-picker').open", 'reopen picker');
  await page.click('#modal-tag-form button[type=submit]');
  await page.waitFor("new URL(location.href).searchParams.has('tag') && document.querySelector('#public-results') && !document.querySelector('#tag-picker').open", 'Apply changes the URL');
  check(await page.eval("document.querySelectorAll('#applied-filters .vk-applied-tag').length === 1"), 'applied filter is visible and removable');
  await page.click('#applied-filters .vk-applied-tag');
  await page.waitFor("!new URL(location.href).searchParams.has('tag') && document.querySelector('.vk-public-row')", 'remove filter');

  // IME event simulation is distinct from a named OS IME session.
  const old = await page.eval('location.href');
  await page.eval("(() => { const i=document.querySelector('#public-query'); i.focus(); i.dispatchEvent(new CompositionEvent('compositionstart',{bubbles:true})); i.dispatchEvent(new KeyboardEvent('keydown',{key:'Enter',bubbles:true,cancelable:true,isComposing:true})); i.dispatchEvent(new CompositionEvent('compositionend',{bubbles:true})); })()");
  check(await page.eval('location.href') === old, 'composition Enter does not commit a partial search');

  await goto('/?q=mua');
  await page.waitFor("document.querySelector('.vk-public-row__link')", 'real title search');
  const returnPath = await page.eval('location.pathname + location.search');
  await page.click('.vk-public-row__link');
  await page.waitFor("document.querySelector('#reader-return') && document.querySelector('#reader-paper')", 'reader from search');
  check(await page.eval("document.querySelector('#reader-return').getAttribute('href').startsWith(" + JSON.stringify(returnPath) + ")"), 'reader return preserves applied query');
  check(await page.eval("Math.abs(document.querySelector('#reader-header .vk-ui-avatar').getBoundingClientRect().width-44)<1"), 'reader avatar is exactly 44px');
  await page.capture('discovery-reader', { boundary: 'real-public-api', scenario: 'reader-return' });
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
  for (const theme of ['light','dark']) for (const width of [390,1440]) {
    await page.viewport(width);
    await page.eval("document.documentElement.dataset.theme="+JSON.stringify(theme));
    check(await page.eval("document.documentElement.scrollWidth<=innerWidth"), 'tag detail reflow '+theme+'/'+width);
    await page.capture('discovery-tag-detail-'+theme+'-'+width, {boundary:'real-public-api',scenario:'tag-detail-empty'});
  }

  const alias = await fetch(args.base + '/the/' + encodeURIComponent('hồi-ức'), { redirect:'manual' });
  check(alias.status === 308 && alias.headers.get('location')?.startsWith('/the/'), 'alias tag route redirects using server canonical identity');
  for (const spelling of [tag.slug.toUpperCase(), tag.slug.normalize('NFD')]) {
    const normalized = await fetch(args.base+'/the/'+encodeURIComponent(spelling), {redirect:'manual'});
    check(normalized.status===308 && normalized.headers.get('location')==='/the/'+encodeURIComponent(tag.slug), 'uppercase/NFD uses the provider canonical tag URL');
  }
  const unknown = await fetch(args.base + '/the/khong-ton-tai-100');
  check(unknown.status === 404 && (await unknown.text()).includes('Không tìm thấy thẻ này'), 'unknown tag has its own SSR 404');
  const invalid = await fetch(args.base + '/?cursor=broken');
  check(invalid.status === 422 && (await invalid.text()).includes('Thử từ trang đầu'), 'invalid cursor is recoverable SSR 422');

  // Native browser GET, with page JS disabled and WASM preloads blocked.
  await page.send('Network.setBlockedURLs', { urls:[args.base+'/pkg/vink.js*',args.base+'/pkg/vink_bg.wasm*'] });
  await page.send('Emulation.setScriptExecutionDisabled', { value:true });
  try {
    await page.goto(args.base + '/?q=mua');
    check(await page.eval("document.querySelector('.vk-public-row__link').href.includes('?q=mua')"), 'no-JS SSR supplies contextual reader links');
    await page.eval("document.querySelector('#public-query').value='khong-co-ket-qua-100'");
    const point = await page.eval("(() => { const b=document.querySelector('#public-search button[type=submit]'); b.scrollIntoView(); const r=b.getBoundingClientRect(); return {x:r.x+r.width/2,y:r.y+r.height/2}; })()");
    for (const type of ['mousePressed','mouseReleased']) await page.send('Input.dispatchMouseEvent', {type,...point,button:'left',clickCount:1});
    await page.waitFor("new URL(location.href).searchParams.get('q')==='khong-co-ket-qua-100' && document.querySelector('[data-testid=public-no-results]')", 'native GET result');
  } finally {
    await page.send('Emulation.setScriptExecutionDisabled',{value:false});
    await page.send('Network.setBlockedURLs',{urls:[]});
  }
  await goto('/');
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
  const context = await page.eval('location.pathname+location.search');
  await page.click('.vk-public-row__link');
  await page.waitFor("document.querySelector('#reader-paper')", 'real fixture reader');
  check(await page.eval("document.querySelector('#reader-return').getAttribute('href').startsWith("+JSON.stringify(context)+")"), 'reader retains query, both tags and opaque cursor');
  check(await page.eval("document.querySelectorAll('.vk-reader-tags a').length===2"), 'reader links the public tags supplied by the snapshot');
  await page.click('#reader-return');
  await page.waitFor("document.activeElement?.matches('.vk-public-row__link')", 'second-page focus restoration');
  await page.reload();
  check(await page.eval('location.pathname+location.search')===context, 'reload preserves filtered second-page URL');
  await page.capture('discovery-real-second-page', {boundary:'real-api+SQL-publication-fixture',scenario:'query-tags-pagination-reader-return'});

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
