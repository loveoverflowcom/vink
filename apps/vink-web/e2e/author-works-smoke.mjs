import { wakeDesk } from './desk-lifecycle.mjs';

// #214. Real provider/Reader anchor check, then explicitly fabricated state/race/geometry cases.
export async function authorWorks(page, {args,check,DraftGate,sleep}) {
  console.log('Author works #214: current-public facts, readonly metrics, failure and reflow:');
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
    check(await page.eval(`document.querySelector('#draft-row-${published.work_id} [data-testid=author-work-metrics]').textContent.includes('Thấm')`), 'real public metrics render in Desk');
  }
  const title='Những ngày mưa ở khuỷu đường cũ, nơi người trở về vẫn nhớ tiếng Việt và những khổ thơ chưa kịp đặt tên'.normalize('NFD');
  const state={audience:'public',lifecycle:'active',generation:3,intent_revision_id:'r2',intent_moderation:'pending',latest_revision_id:'r2',latest_moderation:'pending',review_withdrawn:false,has_draft_changes:true,current_public:{revision_id:'r1',title:'Tên của bản đang công khai',tham:0,comments:123456789}};
  const card=(i,publication=state)=>({work_id:`88888888-8888-4888-8888-${String(i).padStart(12,'0')}`,title:i===0?title:`[214] Tác phẩm ${i}`,draft_version:3,updated_at:'2026-11-15T12:00:00Z',publication});
  const works=[card(0),card(1,{...state,audience:'private',current_public:null,intent_revision_id:null,intent_moderation:null,latest_revision_id:null,latest_moderation:null,generation:1}),card(2,{...state,intent_moderation:'rejected',latest_moderation:'rejected'}),card(3,{...state,audience:'future'}),card(4,{...state,current_public:{...state.current_public,tham:null,comments:null}})];
  works[1].title = "";
  works.push({...card(5,works[1].publication),draft_version:null});
  const gate=new DraftGate(page); gate.list={raw:true,body:{works,next_cursor:null}};await gate.enable();
  const settled=()=>page.waitFor(`document.querySelector('[data-testid=draft-content]')?.getAttribute('aria-busy')==='false'`,'author summary settles');
  try {
    await wakeDesk(page);await settled();
    check(await page.eval(`document.querySelectorAll('[data-testid=draft-row]').length===6 && document.querySelectorAll('[data-testid=author-work-metrics]').length===3`), 'private/unknown rows hide metrics; known zero and unavailable remain distinct');
    check(await page.eval(`(() => {
      const row=document.querySelector('#draft-row-${works[1].work_id}');
      const title=row.querySelector('.vk-draft-row__title'), time=row.querySelector('time');
      return title.textContent==='Chưa đặt tên' && getComputedStyle(title).fontStyle==='italic' &&
        time.dateTime==='2026-11-15T12:00:00Z' && time.title===time.dateTime && time.textContent.includes('15/11/2026');
    })()`), '#410 untitled works use an italic placeholder and retain the authoritative timestamp');
    check(await page.eval(`document.querySelector('#draft-row-${works[5].work_id} .vk-author-work__status').querySelector('[title="Chỉ mình tôi"]') && document.querySelector('#draft-row-${works[5].work_id}').textContent.includes('Không có bản nháp để sửa') && !document.querySelector('#draft-row-${works[5].work_id} a')`), 'a private work without a draft has truthful status and no fabricated editor target');
    check(await page.eval(`document.querySelector('#draft-row-${works[0].work_id}').textContent.includes('Đã đăng · Có bản sửa') && document.querySelector('#draft-row-${works[2].work_id}').textContent.includes('không được duyệt')`), 'public pointer coexists with pending/rejected newer revisions');
    check(await page.eval(`document.querySelector('#draft-row-${works[4].work_id}').querySelector('[title="Chưa tải được số Thấm"]') && !!document.querySelector('#draft-row-${works[4].work_id} a[href^="/ban-viet/"]')`), 'unavailable metrics keep the editor link usable');
    check(await page.eval(`document.querySelectorAll('#draft-results a a, #draft-results a button, #draft-results button').length===0`), 'readonly counters offer no endorsement mutation or nested targets');
    check(await page.eval(`(() => {
      const row=document.querySelector('#draft-row-${works[0].work_id}');
      const title=row.querySelector('.vk-draft-row__title'), metadata=row.querySelector('.vk-author-work__status');
      const editor=row.querySelector('.vk-draft-row__link'), reader=row.querySelector('.vk-author-work__reader');
      return editor.contains(title) && editor.contains(metadata) && reader.closest('a')===reader && !editor.contains(reader) && reader.textContent==='Đọc' && reader.getAttribute('aria-label').startsWith('Đọc bản công khai của ');
    })()`), 'F-13 editor covers status and title, and published works retain an independent reader link');
    check(await page.eval(`(() => {
      const cards=[...document.querySelectorAll('.vk-author-work')], first=cards[0];
      const [tham,comments]=first.querySelectorAll('.vk-author-work__count');
      const shape=getComputedStyle(first);
      return shape.borderRadius==='20px' && shape.backgroundColor!=='rgba(0, 0, 0, 0)' &&
        getComputedStyle(document.querySelector('#draft-results')).rowGap==='8px' &&
        tham.title==='0 Thấm' && comments.title==='123456789 nhận xét' &&
        tham.querySelector('[data-icon=pen-flight]').getBoundingClientRect().width===26 &&
        comments.querySelector('[data-icon=chat-bubble-outline]').getBoundingClientRect().width===24 &&
        first.querySelectorAll('.vk-author-work__label').length===3 &&
        first.querySelector('.vk-author-work__reader').getBoundingClientRect().height>=48;
    })()`), '#410 separate 20px cards with an 8px gap, factual badges and public counters');
    for (const width of [320,390,834,1440]) for (const theme of ['light','dark']) for (const scale of [100,150,200]) {
      await page.viewport(width);
      await page.send('Input.dispatchMouseEvent',{type:'mouseMoved',x:0,y:0});
      await page.eval(`document.documentElement.dataset.theme='${theme}';document.documentElement.style.fontSize='${scale}%';document.querySelector('#drafts').scrollIntoView({block:'start'})`);
      await page.eval(`document.fonts.ready`);
      check(await page.eval(`document.documentElement.scrollWidth<=innerWidth && [...document.querySelectorAll('.vk-author-work')].every(e=>e.getBoundingClientRect().right<=innerWidth+1)`), `author rows reflow ${width}/${theme}/text${scale}`);
      await page.capture(`author-works-${width}-${theme}-${scale}`,{boundary:'fabricated-author-summary-real-hydration',scenario:'#214-states-reflow'});
    }
    await page.viewport(390);await page.eval(`document.documentElement.dataset.theme='dark';document.documentElement.style.fontSize='200%';document.querySelector('#draft-row-${works[5].work_id}').scrollIntoView({block:'center'})`);
    await page.capture('author-works-no-draft-390-dark-200',{boundary:'fabricated-author-summary-real-hydration',scenario:'#214-private-without-draft'});
    await page.eval(`document.querySelector('[data-testid=author-work-metrics]').scrollIntoView({block:'center'})`);
    await page.capture('author-works-metrics-390-dark-200',{boundary:'fabricated-author-summary-real-hydration',scenario:'#214-readable-public-metrics'});
    await page.eval(`[...document.querySelectorAll('.vk-author-work__reader')].at(-1).focus();[...document.querySelectorAll('.vk-author-work__reader')].at(-1).scrollIntoView({block:'center'})`);
    check(await page.eval(`(()=>{const r=document.activeElement.getBoundingClientRect(),bar=document.querySelector('.vk-ui-bottom-nav');return r.height>=44&&r.top>=0&&r.bottom<=(bar?.getClientRects().length?bar.getBoundingClientRect().top:innerHeight)&&r.right<=innerWidth;})()`),'last public-reader target is reachable above bottom navigation at 390/text200');
    await page.click('#open-draft-search');
    await page.viewport(390);await page.eval(`document.documentElement.style.fontSize='100%';document.querySelector('#draft-search').focus();document.querySelector('#draft-search').setSelectionRange(0,0);window.__authorInput=document.querySelector('#draft-search');window.__authorEditor=document.querySelector('.vk-draft-row__link')`);
    gate.list={raw:true,body:{works:[{...works[0],publication:{...state,current_public:{revision_id:'r2',title:'Bản mới công khai',tham:8,comments:2},has_draft_changes:false,intent_revision_id:'r2',intent_moderation:'approved'}}],next_cursor:null}};
    await wakeDesk(page);await settled();
    check(await page.eval(`[...document.querySelectorAll('[data-testid=author-work-metrics] .vk-author-work__count')].map(c=>c.title).join('|')==='8 Thấm|2 nhận xét' && !document.querySelector('#draft-results').textContent.includes('Có bản sửa') && document.querySelector('#draft-search')===window.__authorInput && document.activeElement===window.__authorInput && document.querySelector('.vk-draft-row__link')===window.__authorEditor`), 'refresh replaces pointer+counts together without remounting input or stealing focus');
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
