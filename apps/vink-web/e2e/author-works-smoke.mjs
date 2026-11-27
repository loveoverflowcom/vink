import { wakeDesk } from './desk-lifecycle.mjs';

// #214 & #514 (Work Views Design r3). Real provider/Reader anchor check, then explicitly fabricated state/race/geometry/explainer cases.
export async function authorWorks(page, {args,check,DraftGate,sleep}) {
  console.log('Author works #214 / #514 (Work Views r3): current-public facts, D-01/D-02, views states, explainer and reflow:');
  await page.goto(args.base+'/ban-viet');
  await page.waitFor(`document.querySelector('[data-testid=draft-content]')?.getAttribute('aria-busy')==='false'`, 'real author list');
  const real=await page.eval(`fetch('/api/v1/author/works?limit=24').then(async r=>({status:r.status,cache:r.headers.get('cache-control'),body:await r.json()}))`);
  check(real.status===200 && real.cache==='no-store' && !JSON.stringify(real.body).includes('document'), 'real author API is no-store and contains no manuscript');
  const published=real.body.works.find(w=>w.publication?.current_public);
  if (published) {
    const reader=await page.eval(`fetch('/api/v1/works/${published.work_id}/reader').then(r=>r.json())`);
    check(reader.revision_id===published.publication.current_public.revision_id, 'real Reader and Desk share the current public revision');
    const endorsements=await page.eval(`fetch('/api/v1/works/${published.work_id}/endorsements').then(r=>r.json())`);
    check(endorsements.work.count===published.publication.current_public.tham, 'real Reader and Desk share the revision-scoped Thấm count');
    check(await page.eval(`(() => {
      const row=document.querySelector('#draft-row-${published.work_id}');
      const metrics=row?.querySelector('[data-testid=author-work-metrics]');
      const icons=[...metrics.querySelectorAll('[data-icon]')].map(el=>el.dataset.icon);
      return Boolean(metrics) && metrics.textContent.includes('Thấm') &&
        !row.querySelector('.vk-author-work__reader') &&
        icons[0]==='pen-flight' && icons[1]==='chat-bubble-outline' && icons[2]==='visibility-outline';
    })()`), 'real public row renders Thấm -> Nhận xét -> Lượt xem and has no Desk Read button (D-01/D-02)');
    await page.click(`#draft-row-${published.work_id} .vk-draft-row__link`);
    await page.waitFor(`location.pathname==='/ban-viet/${published.work_id}'`, '#514 D-02 selecting a work row opens the Editor');
    await page.eval('history.back()');
    await page.waitFor(`location.pathname==='/ban-viet' && document.querySelector('#draft-row-${published.work_id} .vk-draft-row__link') && document.querySelector('[data-testid=draft-content]')?.getAttribute('aria-busy')==='false'`, '#514 browser Back restores the Desk');
  }
  const title='Những ngày mưa ở khuỷu đường cũ, nơi người trở về vẫn nhớ tiếng Việt và những khổ thơ chưa kịp đặt tên'.normalize('NFD');
  const state={audience:'public',lifecycle:'active',generation:3,intent_revision_id:'r2',intent_moderation:'pending',latest_revision_id:'r2',latest_moderation:'pending',review_withdrawn:false,has_draft_changes:true,current_public:{revision_id:'r1',title:'Tên của bản đang công khai',tham:0,comments:123456789}};
  const countedViews=(count,counted_since=null)=>({state:'counted',count,counted_since});
  const card=(i,publication=state,views=countedViews(1234,'2026-12-01T01:00:00Z'))=>({work_id:`88888888-8888-4888-8888-${String(i).padStart(12,'0')}`,title:i===0?title:`[214] Tác phẩm ${i}`,draft_version:3,updated_at:'2026-11-15T12:00:00Z',publication,views});
  const privatePub={...state,audience:'private',current_public:null,intent_revision_id:null,intent_moderation:null,latest_revision_id:null,latest_moderation:null,generation:1};
  const withdrawnPub={...state,audience:'public',current_public:null,review_withdrawn:true,intent_revision_id:null,intent_moderation:null,latest_moderation:'rejected'};
  const works=[
    card(0,state,countedViews(1234,'2026-12-01T01:00:00Z')),
    card(1,privatePub,{state:'never_public',count:null,counted_since:null}),
    card(2,{...state,current_public:{revision_id:'r1',title:'[214] Tác phẩm 2',tham:12,comments:3},intent_moderation:'rejected',latest_moderation:'rejected'},countedViews(0,null)),
    card(3,{...state,audience:'future',current_public:null},{state:'not_started',count:null,counted_since:null}),
    card(4,{...state,current_public:{...state.current_public,tham:null,comments:null}},{state:'unavailable',count:null,counted_since:null}),
    {...card(5,privatePub,countedViews(3,null)),draft_version:null},
    card(6,withdrawnPub,countedViews(5,'2026-11-21T00:00:00Z')),
  ];
  works[1].title = "";
  const gate=new DraftGate(page); gate.list={raw:true,body:{works,next_cursor:null}};await gate.enable();
  const settled=()=>page.waitFor(`document.querySelector('[data-testid=draft-content]')?.getAttribute('aria-busy')==='false'`,'author summary settles');
  try {
    await wakeDesk(page);await settled();
    check(await page.eval(`(() => {
      const rows=document.querySelectorAll('[data-testid=draft-row]').length;
      const metrics=document.querySelectorAll('[data-testid=author-work-metrics]').length;
      const neverRow=document.querySelector('#draft-row-${works[1].work_id}');
      const histPriv=document.querySelector('#draft-row-${works[5].work_id}');
      const histWithdrawn=document.querySelector('#draft-row-${works[6].work_id}');
      const notStarted=document.querySelector('#draft-row-${works[3].work_id}');
      const unavail=document.querySelector('#draft-row-${works[4].work_id}');
      return rows===7 && metrics===6 &&
        !neverRow.querySelector('[data-testid=author-work-metrics]') &&
        Boolean(neverRow.querySelector('[data-testid=author-work-views-never-public]')) &&
        histPriv.querySelector('[data-testid=author-work-views]')?.dataset.viewsState==='counted' &&
        histPriv.querySelector('[data-testid=author-work-views]')?.classList.contains('vk-author-work__views--historical') &&
        !histPriv.querySelector('[data-icon=pen-flight], [data-icon=chat-bubble-outline]') &&
        histWithdrawn.querySelector('[data-testid=author-work-views]')?.textContent.includes('Tính từ 21/11/2026') &&
        notStarted.querySelector('[data-testid=author-work-views]')?.dataset.viewsState==='not_started' &&
        notStarted.querySelector('[data-testid=author-work-views]')?.textContent.includes('Chưa bắt đầu thống kê') &&
        unavail.querySelector('[data-testid=author-work-views]')?.dataset.viewsState==='unavailable' &&
        unavail.querySelector('[data-testid=author-work-views]')?.textContent.includes('Chưa tải được');
    })()`), '#514 all 4 view states (counted + since, zero, not_started, never_public, unavailable) and OQ-1=A historical views render distinctly');
    check(await page.eval(`(() => {
      const row=document.querySelector('#draft-row-${works[1].work_id}');
      const title=row.querySelector('.vk-draft-row__title'), time=row.querySelector('time');
      return title.textContent==='Chưa đặt tên' && getComputedStyle(title).fontStyle==='italic' &&
        time.dateTime==='2026-11-15T12:00:00Z' && time.title===time.dateTime && time.textContent.includes('15/11/2026');
    })()`), '#410 untitled works use an italic placeholder and retain the authoritative timestamp');
    check(await page.eval(`document.querySelector('#draft-row-${works[5].work_id} .vk-author-work__status').querySelector('[title="Chỉ mình tôi"]') && document.querySelector('#draft-row-${works[5].work_id}').textContent.includes('Không có bản nháp để sửa') && !document.querySelector('#draft-row-${works[5].work_id} a')`), 'a private work without a draft has truthful status and no fabricated editor target');
    check(await page.eval(`document.querySelector('#draft-row-${works[0].work_id}').textContent.includes('Đã đăng · Có bản sửa') && document.querySelector('#draft-row-${works[2].work_id}').textContent.includes('không được duyệt') && document.querySelector('#draft-row-${works[6].work_id}').textContent.includes('Đã ẩn')`), 'public pointer coexists with pending/rejected newer revisions and withdrawn status');
    check(await page.eval(`document.querySelector('#draft-row-${works[4].work_id}').querySelector('[title="Chưa tải được số Thấm"]') && !!document.querySelector('#draft-row-${works[4].work_id} a[href^="/ban-viet/"]')`), 'unavailable metrics keep the editor link usable');
    check(await page.eval(`(() => {
      const list = document.querySelector('#draft-results');
      const interactive = 'a, button, input, select, textarea, [role=button], [role=link]';
      const metrics = [...list.querySelectorAll('[data-testid=author-work-metrics]')];
      return metrics.every(group => !group.matches(interactive) && !group.querySelector(interactive)) &&
        [...list.querySelectorAll(interactive)].every(target => !target.querySelector(interactive));
    })()`), 'readonly counters offer no endorsement mutation or nested targets');
    check(await page.eval(`[...document.querySelectorAll('#draft-results [data-testid=draft-row]')].every(row => {
      const buttons = row.querySelectorAll('button');
      const links = row.querySelectorAll('a');
      const workId = row.id.slice('draft-row-'.length);
      const review = buttons[0];
      const noReaderLink = !row.querySelector('.vk-author-work__reader') && ![...links].some(a => a.pathname.startsWith('/tac-pham/') || a.textContent.trim()==='Đọc');
      return noReaderLink && buttons.length === 1 && review.id === 'desk-review-trigger-' + workId &&
        review.classList.contains('vk-ui-button--text') &&
        Boolean(review.querySelector('[data-icon=verified-user]')) &&
        review.getAttribute('aria-haspopup') === 'dialog' && review.textContent.trim() === 'Xem kiểm duyệt';
    })`), '#514 D-02 zero Read button/link and every row exposes one quiet verified-user review button (#593)');
    check(await page.eval(`(() => {
      const row=document.querySelector('#draft-row-${works[0].work_id}');
      const title=row.querySelector('.vk-draft-row__title'), metadata=row.querySelector('.vk-author-work__status');
      const editor=row.querySelector('.vk-draft-row__link');
      return editor.contains(title) && editor.contains(metadata) && !row.querySelector('.vk-author-work__reader') &&
        editor.getAttribute('href')==='/ban-viet/${works[0].work_id}';
    })()`), '#514 D-02 editor link covers status, title and metrics with no sibling Read link');
    // #593 compares the resting row, independently of the prior pointer position.
    await page.send('Input.dispatchMouseEvent',{type:'mouseMoved',x:0,y:0});
    await page.eval(`Promise.all(document.getAnimations().filter(a=>Number.isFinite(a.effect.getComputedTiming().endTime)).map(a=>a.finished.catch(()=>{})))`);
    check(await page.eval(`(() => {
      const cards=[...document.querySelectorAll('.vk-author-work')], first=cards[0];
      const [tham,comments,views]=first.querySelectorAll('.vk-author-work__count');
      const shape=getComputedStyle(first);
      const rT=tham.getBoundingClientRect(), rC=comments.getBoundingClientRect(), rV=views.getBoundingClientRect();
      const labels=[...first.querySelectorAll('.vk-author-work__label')];
      const zeroViews=document.querySelector('#draft-row-${works[2].work_id} [data-testid=author-work-views]');
      return shape.borderRadius==='0px' && shape.backgroundColor==='rgba(0, 0, 0, 0)' &&
        getComputedStyle(document.querySelector('#draft-results')).rowGap==='8px' &&
        getComputedStyle(tham).fontVariantNumeric==='tabular-nums' &&
        getComputedStyle(views).fontVariantNumeric==='tabular-nums' &&
        tham.title==='0 Thấm' && comments.title==='123.456.789 nhận xét' &&
        views.title==='1.234 lượt xem, tính từ 01/12/2026' &&
        zeroViews.title==='0 lượt xem' && !zeroViews.textContent.includes('Tính từ') &&
        rT.left < rC.left && rC.left < rV.left &&
        tham.querySelector('[data-icon=pen-flight]').getBoundingClientRect().width===22 &&
        comments.querySelector('[data-icon=chat-bubble-outline]').getBoundingClientRect().width===20 &&
        views.querySelector('[data-icon=visibility-outline]').getBoundingClientRect().width===20 &&
        labels.length===3 && labels.map(l=>l.dataset.tone).join(',')==='ok,quiet,attn';
    })()`), '#593 flat Desk rows: transparent resting surface, 8px gap, tabular Vietnamese grouping, D-01 horizontal order, 22/20/20 icons and selective tonal badges');

    // #514 UX-07 accessible Views explainer trigger & dialog (desktop light + mobile dark captures).
    await page.viewport(1440);
    await page.eval(`document.documentElement.dataset.theme='light';document.documentElement.style.fontSize='100%';document.querySelector('#drafts').scrollIntoView({block:'start'})`);
    check(await page.eval(`(() => { const r = document.querySelector('#views-explainer-trigger').getBoundingClientRect(); return r.height >= 44 && r.height <= 48; })()`), '#514 explainer trigger keeps 44px hit target on a single line at 1440px');
    await page.eval(`document.querySelector('#views-explainer-trigger').focus()`);
    await page.key('Enter');
    await page.waitFor(`document.querySelector('#views-explainer')?.open === true`, '#514 Enter opens Views explainer dialog');
    check(await page.eval(`(() => {
      const dlg=document.querySelector('#views-explainer');
      const text=dlg.textContent;
      return document.activeElement?.id==='views-explainer-title' &&
        text.includes('5 giây') && text.includes('24 giờ') && text.includes('trình duyệt') &&
        text.includes('tự xem') && text.includes('JavaScript') && text.includes('Tính từ 01/12/2026');
    })()`), '#514 UX-07 explainer dialog focuses title and includes 5s policy, 24h, browser, self-view, JS and counted_since');
    await page.capture('author-works-explainer-1440-light-100',{boundary:'fabricated-author-summary-real-hydration',scenario:'#514-views-explainer-sheet'});
    await page.key('Escape');
    await page.waitFor(`document.querySelector('#views-explainer')?.open === false && document.activeElement?.id==='views-explainer-trigger'`, '#514 Escape closes explainer and restores focus to trigger');
    await page.viewport(390);
    await page.eval(`document.documentElement.dataset.theme='dark';document.documentElement.style.fontSize='100%'`);
    await page.click('#views-explainer-trigger');
    await page.waitFor(`document.querySelector('#views-explainer')?.open === true`, '#514 click opens explainer on mobile');
    await page.capture('author-works-explainer-390-dark-100',{boundary:'fabricated-author-summary-real-hydration',scenario:'#514-views-explainer-sheet'});
    await page.click('#views-explainer-close');
    await page.waitFor(`document.querySelector('#views-explainer')?.open === false && document.activeElement?.id==='views-explainer-trigger'`, '#514 Close button closes explainer and restores focus');

    // The sheet above an open search consumes the first Back.
    await page.click('#open-draft-search');
    await page.waitFor(`document.activeElement.id === 'draft-search'`, 'search open below the explainer');
    await page.eval(`history.pushState(null, '', location.href)`);
    await page.click('#views-explainer-trigger');
    await page.waitFor(`document.querySelector('#views-explainer')?.open === true`, '#514 explainer is open before Back');
    await page.eval(`history.back()`);
    await page.waitFor(`location.pathname==='/ban-viet' && document.querySelector('#views-explainer')?.open === false && document.activeElement?.id==='views-explainer-trigger'`, '#514 Back closes explainer on Desk and restores focus');

    await page.eval(`new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))`);
    check(await page.eval(`document.activeElement?.id==='views-explainer-trigger' && document.querySelector('#views-explainer')?.open === false`),
      '#514 Back keeps the explainer closed and focus restored after paint');
    check(await page.eval(`!document.querySelector('#desk-search-form').inert`), '#531 Back in the sheet preserves open search');
    await page.click('#close-draft-search');

    for (const width of [320,390,834,1024,1440]) for (const theme of ['light','dark']) for (const scale of [100,150,200]) {
      await page.viewport(width);
      await page.send('Input.dispatchMouseEvent',{type:'mouseMoved',x:0,y:0});
      await page.eval(`document.documentElement.dataset.theme='${theme}';document.documentElement.style.fontSize='${scale}%';document.querySelector('#drafts').scrollIntoView({block:'start'})`);
      await page.eval(`document.fonts.ready.then(()=>new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r))))`);
      await page.eval(`Promise.all(document.getAnimations().filter(a=>Number.isFinite(a.effect.getComputedTiming().endTime)).map(a=>a.finished.catch(()=>{})))`);
      const overflow = await page.eval(`(() => {
        const wide = [...document.querySelectorAll('.vk-author-work, .vk-author-work .vk-draft-row__text, .vk-author-work__status, .vk-author-work__metrics')]
          .filter(e => e.scrollWidth > e.clientWidth + 1 || e.getBoundingClientRect().right > innerWidth + 1)
          .map(e => ({ block: e.classList[0], scroll: e.scrollWidth, width: e.clientWidth }));
        if (document.documentElement.scrollWidth > innerWidth) wide.push({ block: 'document' });
        return wide;
      })()`);
      check(overflow.length === 0, `author rows reflow ${width}/${theme}/text${scale} ${JSON.stringify(overflow)}`);
      await page.capture(`author-works-${width}-${theme}-${scale}`,{boundary:'fabricated-author-summary-real-hydration',scenario:'#514-work-views-r3-reflow'});
    }
    await page.viewport(390);await page.eval(`document.documentElement.dataset.theme='dark';document.documentElement.style.fontSize='200%';document.querySelector('#draft-row-${works[5].work_id}').scrollIntoView({block:'center'})`);
    await page.capture('author-works-no-draft-390-dark-200',{boundary:'fabricated-author-summary-real-hydration',scenario:'#514-private-historical-without-draft'});
    await page.eval(`document.querySelector('[data-testid=author-work-metrics]').scrollIntoView({block:'center'})`);
    await page.capture('author-works-metrics-390-dark-200',{boundary:'fabricated-author-summary-real-hydration',scenario:'#514-readable-public-metrics'});
    await page.eval(`[...document.querySelectorAll('#draft-results .vk-desk-review-action button')].at(-1).focus();[...document.querySelectorAll('#draft-results .vk-desk-review-action button')].at(-1).scrollIntoView({block:'center'})`);
    check(await page.eval(`(()=>{const r=document.activeElement.getBoundingClientRect(),bar=document.querySelector('.vk-ui-bottom-nav');return r.height>=44&&r.top>=0&&r.bottom<=(bar?.getClientRects().length?bar.getBoundingClientRect().top:innerHeight)&&r.right<=innerWidth;})()`),'last row review target is reachable above bottom navigation at 390/text200');
    await page.click('#open-draft-search');
    await page.viewport(390);await page.eval(`document.documentElement.style.fontSize='100%';document.querySelector('#draft-search').focus();document.querySelector('#draft-search').setSelectionRange(0,0);window.__authorInput=document.querySelector('#draft-search');window.__authorEditor=document.querySelector('.vk-draft-row__link')`);
    gate.list={raw:true,body:{works:[{...works[0],publication:{...state,current_public:{revision_id:'r2',title:'Bản mới công khai',tham:8,comments:2},has_draft_changes:false,intent_revision_id:'r2',intent_moderation:'approved'},views:countedViews(1235,'2026-12-01T01:00:00Z')}],next_cursor:null}};
    await wakeDesk(page);await settled();
    check(await page.eval(`[...document.querySelectorAll('[data-testid=author-work-metrics] .vk-author-work__count')].map(c=>c.title).join('|')==='8 Thấm|2 nhận xét|1.235 lượt xem, tính từ 01/12/2026' && !document.querySelector('#draft-results').textContent.includes('Có bản sửa') && document.querySelector('#draft-search')===window.__authorInput && document.activeElement===window.__authorInput && document.querySelector('.vk-draft-row__link')===window.__authorEditor`), 'refresh replaces pointer+counts+views together without remounting input or stealing focus');
    gate.holdGets=true;await wakeDesk(page);const held=await gate.next('gets');await gate.fulfill(held,503,{error:{code:'dependency_unavailable'}});await settled();
    check(await page.eval(`!!document.querySelector('[data-testid=author-work-stale]') && !!document.querySelector('.vk-draft-row__link')`),'read outage retains same-account snapshot with an explicit stale label');
    await wakeDesk(page);const denied=await gate.next('gets');await gate.fulfill(denied,403,{error:{code:'forbidden'}});
    await page.waitFor(`document.querySelector('[data-testid=desk-access]')`,'#308 scoped access refusal');
    check(await page.eval(`!document.querySelector('#draft-search, #retry-drafts, [data-testid=draft-row]')`),'authority refusal clears summaries, metrics and private controls immediately');
    check(gate.postCount===0,'all summary refreshes remain read-only');
  } finally {
    await gate.disable();await page.eval(`document.documentElement.style.fontSize=''`);
  }
}
