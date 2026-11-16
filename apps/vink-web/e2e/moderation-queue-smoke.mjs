// #205 browser transport fixtures. Real API/governance is tested separately in PostgreSQL
// and in review-withdrawn. These fixtures exercise delayed/error pages without claiming auth E2E.
export async function moderationQueues(page, {args,check}) {
  console.log('#205: independent queue continuation/recovery transport fixtures:');
  let head=0, appealMode='ok', tailMode='ok', held=null, old=null, requests=0, automation=0;
  const row=(id,title)=>({id,work_id:id,revision_id:id,decision_version:1,version:1,state:'queued',origin:'automation_paused',claimed_by:null,title});
  const first=row('00000000-0000-4000-8000-000000000001','[Mẫu] Hàng chờ ỹ ỷ — bản thứ nhất');
  const second=row('00000000-0000-4000-8000-000000000002','[Mẫu] Trang tiếp theo');
  const fresh=row('00000000-0000-4000-8000-000000000003','[Mẫu] Đầu hàng mới');
  const fulfill=(id,status,body)=>page.send('Fetch.fulfillRequest',{requestId:id,responseCode:status,responseHeaders:[{name:'Content-Type',value:'application/json'},{name:'Cache-Control',value:'no-store'}],body:Buffer.from(JSON.stringify(body)).toString('base64')});
  const listener=m=>{
    if(m.sessionId!==page.sessionId || m.method!=='Fetch.requestPaused') return;
    const {requestId,request}=m.params,u=new URL(request.url);
    if(u.pathname.endsWith('/automation')) {automation++;page.send('Fetch.continueRequest',{requestId}).catch(()=>{});return;}
    requests++;
    if(u.pathname.includes('/appeals/') && appealMode==='forbidden') {fulfill(requestId,403,{error:{code:'forbidden'}}).catch(()=>{});return;}
    if(u.pathname.includes('/appeals/')) {fulfill(requestId,200,{authority_key:'fixture-authority-205',items:[],next_cursor:null}).catch(()=>{});return;}
    if(u.searchParams.has('cursor')) {
      if(tailMode==='hold') {held=requestId;return;}
      if(tailMode==='error') {fulfill(requestId,503,{error:{code:'dependency_unavailable'}}).catch(()=>{});return;}
      if(tailMode==='invalid') {fulfill(requestId,422,{error:{code:'validation_failed',field:'cursor'}}).catch(()=>{});return;}
      if(tailMode==='scope') {fulfill(requestId,200,{authority_key:'narrower-authority',items:[second],next_cursor:null}).catch(()=>{});return;}
      if(tailMode==='forbidden') {fulfill(requestId,403,{error:{code:'forbidden'}}).catch(()=>{});return;}
      fulfill(requestId,200,{authority_key:'fixture-authority-205',items:[second],next_cursor:null}).catch(()=>{});return;
    }
    head++;fulfill(requestId,200,{authority_key:'fixture-authority-205',items:[head>1?fresh:first],next_cursor:'fixture-205-next'}).catch(()=>{});
  };
  page.cdp.listeners.push(listener);
  await page.send('Fetch.enable',{patterns:[{urlPattern:'*/api/v1/moderation/*/page*',requestStage:'Request'},{urlPattern:'*/api/v1/moderation/automation',requestStage:'Request'}]});
  const settled=()=>page.waitFor(`document.querySelector('#case-queue')?.getAttribute('aria-busy')==='false'`,'queue request settles');
  const ids=()=>page.eval(`[...document.querySelectorAll('#case-queue li a')].map(a=>a.getAttribute('href').split('/').at(-1))`);
  try {
    await page.goto(args.base+'/kiem-duyet');await page.waitFor(`document.querySelector('#case-queue li a') && document.querySelector('#automation-reason')`,'fixture queues and real automation');await settled();
    await page.eval(`document.querySelector('#automation-reason').focus()`);await page.send('Input.insertText',{text:'Lý do ỹ ỷ chưa gửi'});
    await page.eval(`window.__reason205=document.querySelector('#automation-reason')`);
    const originalAutomation=automation;
    await page.eval(`document.querySelector('#case-queue-more').focus()`);await page.key('Enter');await settled();
    check((await ids()).length===2,'keyboard append retains first page and accepts the next');
    check(await page.eval(`document.querySelector('#automation-reason')===window.__reason205 && window.__reason205.value==='Lý do ỹ ỷ chưa gửi'`),'queue paging preserves the actual reason element and Vietnamese unsent text');
    check(automation===originalAutomation,'paging never reloads automation');
    check(await page.eval(`document.activeElement.id==='case-queue-refresh'`),'exhausted continuation restores focus to its queue refresh');
    // A refresh can update the state of a row whose ID stays the same.
    head=0;first.state='claimed';
    await page.eval(`window.__case205=document.querySelector('#case-queue li a')`);
    await page.click('#case-queue-refresh');await settled();
    check(await page.eval(`document.querySelector('#case-queue li a').textContent.includes('Đã có người nhận')`),'refresh updates the state label of an existing case ID');
    check(await page.eval(`document.querySelector('#case-queue li a')===window.__case205`),'refresh keeps the same case link element');
    await page.click('#case-queue-refresh');await settled();
    check((await ids()).length===1 && (await ids())[0]===fresh.id,'refresh replaces the whole chain and discards the old tail');
    tailMode='error';await page.click('#case-queue-more');await settled();
    check((await ids())[0]===fresh.id && await page.eval(`!!document.querySelector('#case-queue [role=alert]') && !!document.querySelector('#case-queue-more')`),'temporary failure keeps accepted data/cursor and exposes error');
    tailMode='hold';await page.click('#case-queue-more');
    for(let i=0;i<100 && !held;i++) await new Promise(r=>setTimeout(r,20));
    check(!!held,'append response is held');old=held;held=null;
    await page.click('#case-queue-refresh');await settled();
    await fulfill(old,200,{authority_key:'fixture-authority-205',items:[second],next_cursor:null});old=null;
    await page.eval(`new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)))`);
    check((await ids()).length===1,'a superseded late append cannot resurrect the old tail');
    tailMode='invalid';await page.click('#case-queue-more');await settled();
    check(await page.eval(`!!document.querySelector('#case-queue-restart') && !!document.querySelector('#case-queue [role=alert]')`),'invalid cursor exposes restart and is never interpreted as end');
    await page.click('#case-queue-restart');await settled();
    for(const width of [390,1280]) for(const scale of [100,200]) {
      await page.viewport(width,900);await page.eval(`document.documentElement.style.fontSize='${scale}%';document.querySelector('#case-queue-more').scrollIntoView({block:'center'})`);
      check(await page.eval(`document.documentElement.scrollWidth<=innerWidth && [...document.querySelectorAll('#case-queue button,#appeal-queue button')].every(b=>{const r=b.getBoundingClientRect();return r.left>=0 && r.right<=innerWidth && r.width>0 && r.height>0;})`),`queue controls fit ${width}px text${scale}`);
      await page.click('#appeal-queue-refresh');await page.waitFor(`document.querySelector('#appeal-queue')?.getAttribute('aria-busy')==='false'`,'appeal refresh at text scale');
      await page.eval(`document.querySelector('#case-queue-more').scrollIntoView({block:'center'})`);
      await page.capture(`moderation-queues-${width}-${scale}`,{boundary:'intercepted-queue-fixtures-local-principal-real-automation',scenario:'#205',textScale:scale});
    }
    check(await page.eval(`document.querySelector('#automation-reason')===window.__reason205 && window.__reason205.value==='Lý do ỹ ỷ chưa gửi'`),'appeal refresh also preserves the unsent reason element and text');
    await page.eval(`document.documentElement.style.fontSize=''`);await page.viewport(1280);
    tailMode='hold';await page.click('#case-queue-more');
    for(let i=0;i<100 && !held;i++) await new Promise(r=>setTimeout(r,20));
    check(!!held,'session generation test holds an old page');old=held;held=null;
    const beforeHead=head;
    await page.eval(`const c=new BroadcastChannel('vink-session');c.postMessage('changed');c.close()`);
    for(let i=0;i<100 && head===beforeHead;i++) await new Promise(r=>setTimeout(r,20));
    await settled();check(head>beforeHead,'same-account session invalidation obtains a new head');
    await fulfill(old,200,{authority_key:'fixture-authority-205',items:[second],next_cursor:null});old=null;
    await page.eval(`new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)))`);
    check((await ids()).length===1,'late private data is fenced across session generations A→unknown→A');
    tailMode='scope';await page.click('#case-queue-more');await settled();
    check((await ids()).length===0 && await page.eval(`!!document.querySelector('#case-queue-restart')`),'partial grant loss on 200 drops the old authorized chain');
    await page.click('#case-queue-restart');await settled();
    tailMode='forbidden';await page.click('#case-queue-more');await settled();
    check((await ids()).length===0 && await page.eval(`!!document.querySelector('#case-queue [role=alert]') && !document.querySelector('#case-queue-more')`),'permission loss clears private rows and continuation');
    check(await page.eval(`!!document.querySelector('#appeal-queue') && document.querySelector('#appeal-queue').textContent.includes('Không có hồ sơ')`),'appeal queue remains independent of work queue failure');
    appealMode='forbidden';await page.click('#appeal-queue-refresh');
    await page.waitFor(`document.querySelector('#appeal-queue')?.getAttribute('aria-busy')==='false' && !document.querySelector('#automation-toggle')`,'admin capability loss clears controls');
    check(await page.eval(`!document.querySelector('#automation-reason')`),'admin queue denial clears automation capabilities without putting automation reads in the page loop');
    const before=requests;await page.send('Network.enable');await page.eval(`window.dispatchEvent(new Event('online'))`);await page.eval(`new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)))`);
    check(requests===before,'online burst respects backoff and does not replay commands');
  } finally {
    if(old) await fulfill(old,503,{error:{code:'dependency_unavailable'}}).catch(()=>{});
    if(held) await fulfill(held,503,{error:{code:'dependency_unavailable'}}).catch(()=>{});
    await page.send('Fetch.disable');page.cdp.listeners.splice(page.cdp.listeners.indexOf(listener),1);
  }
}
