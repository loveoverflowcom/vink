// The caller has completed the real email-code and profile flow on the scratch Reader.
import { holdAnswer } from './auth-browser-network.mjs';
import { writeFileSync } from 'node:fs';
import { join, dirname } from 'node:path';

export async function richDiscussion(page,{args,check,rootId,workId}) {
  const endpoint='/api/v1/works/'+workId+'/discussion';
  const root='#root-composer', reply='#reply-composer-'+rootId;
  const source = async selector => page.eval('document.querySelector('+JSON.stringify(selector)+').value');
  const ready = selector => page.waitFor('document.querySelector('+JSON.stringify(selector+'-rich .cm-content')+') === document.activeElement','focused Comment view');
  const open = async selector => {await page.click(selector+'-format'); await ready(selector);};
  const enter = text => page.send('Input.insertText',{text});
  const requests=[];
  const listener = event => {
    if(event.sessionId!==page.sessionId || event.method!=='Network.requestWillBeSent')return;
    const req=event.params.request;
    if(req.method==='POST' && new URL(req.url).pathname===endpoint+'/comments') requests.push(JSON.parse(req.postData));
  };
  page.cdp.listeners.push(listener);
  try {
    await page.goto(args.base+'/tac-pham/'+workId);
    await page.waitFor("document.querySelector('#reader-auth')?.dataset.standing==='active' && document.querySelector('#root-composer-format')",'member Reader ready');
    await open(root);
    const toolbar='.vk-compact-field:has(#root-composer) [data-format-action=';
    for(const [kind,label] of [['bold','Đậm'],['italic','Nghiêng'],['link','Liên kết'],['quote','Trích dẫn'],['list','Danh sách']]){
      const button=toolbar+kind+']';
      check(await page.eval('document.querySelector('+JSON.stringify(button)+').getAttribute("aria-label")')===label,'compact action '+label);
      await page.click(button);
      check((await source(root)).length>0,'command changes source '+kind);
      await page.key('z',page.mod);
      await page.waitFor("document.querySelector('#root-composer').value===''",'undo '+kind);
    }
    await page.send('Input.imeSetComposition',{text:'đ',selectionStart:1,selectionEnd:1});
    check(await page.eval("document.querySelector('#root-composer-send').getAttribute('aria-disabled')==='true'"),'CDP composing blocks send');
    await enter('đêm');
    await page.key('Enter');
    await page.send('Browser.grantPermissions',{permissions:['clipboardReadWrite','clipboardSanitizedWrite'],origin:args.base});
    await page.eval("navigator.clipboard.writeText('**Mưa** 👩‍💻')");
    for(const type of ['rawKeyDown','keyUp']) await page.send('Input.dispatchKeyEvent',{type,key:'v',code:'KeyV',modifiers:page.mod,windowsVirtualKeyCode:86,nativeVirtualKeyCode:86,...(type==='rawKeyDown'?{commands:['Paste']}:{})});
    await page.waitFor("document.querySelector('#root-composer').value.includes('**Mưa**')",'real clipboard paste to Comment');
    await page.key('Enter');
    await page.waitFor("document.querySelector('#root-composer-rich .cm-writing-bold')",'caret-aware preview on previous line');
    check(requests.length===0,'Enter and editing generated no POST');
    const held=await holdAnswer(page,'POST',endpoint+'/comments');
    await page.click(root+'-send');
    check(await held.held()===201,'real root commit');
    await page.click(root+'-rich .cm-content'); await enter('nháp mới');
    const newer=await source(root);
    await held.finish(true);
    const recovery='.vk-discussion-composer:has(#root-composer) .vk-discussion-delivery .vk-ui-button--filled';
    await page.waitFor("document.querySelector("+JSON.stringify(recovery)+")?.textContent.includes('Kiểm tra lần gửi')",'uncertain command');
    await page.click(recovery);
    await page.waitFor("document.querySelector('#own-comments .vk-comment-markdown strong')?.textContent==='Mưa'",'rendered own Markdown root');
    check(requests.length===2 && JSON.stringify(requests[0])===JSON.stringify(requests[1]) && requests[0].body_format==='markdown' && await source(root)===newer,'exact retry and late ACK preserve newer draft');
    await page.click('#reply-to-'+rootId); await open(reply);
    await enter('**Bình minh**');
    await page.click(reply+'-send');
    await page.waitFor("[...document.querySelectorAll('#own-comments strong')].some(x=>x.textContent==='Bình minh')",'formatted own reply');
    check(requests[2].target_id===rootId && requests[2].body_format==='markdown','flat reply snapshot');
    await page.reload();
    await page.waitFor("[...document.querySelectorAll('#own-comments strong')].some(x=>x.textContent==='Bình minh')",'reload real private overlay');
    check(await page.eval("performance.getEntriesByType('resource').every(r=>!r.name.endsWith('/vink-editor.js'))"),'reading submitted Markdown uses zero editor resources');
    const own=await page.eval("fetch("+JSON.stringify(endpoint+'/me')+").then(r=>r.json())");
    const publicPage=await page.eval("fetch("+JSON.stringify(endpoint+'/public')+").then(r=>r.json())");
    check(own.comments.some(c=>c.body_format==='markdown' && c.body==='**Bình minh**') && !JSON.stringify(publicPage).includes('**Bình minh**'),'moderation scope and stored source');
    await page.click('#reply-to-'+rootId); await open(reply);
    const before=await page.send('Memory.getDOMCounters');
    const heapBefore=await page.send('Runtime.getHeapUsage');
    await enter('Giữ nháp');
    for(let i=0;i<40;i++) {
      await page.click(reply+'-close');
      await page.click('#reopen-reply-'+rootId);
    }
    const viewCount=await page.eval("document.querySelectorAll('[data-vink-editor=comment]').length");
    check(viewCount===1 && await source(reply)==='Giữ nháp','40 reply collapse/reopen cycles keep one view and its draft');
    const ax=await page.send('Accessibility.getFullAXTree');
    const box=ax.nodes.find(n=>n.role.value==='textbox' && n.name?.value==='Viết phản hồi' && !n.ignored);
    check(box?.description?.value?.includes('Trả lời'),'active reply accessible name and description');
    for(const [width,height] of [[320,700],[390,844],[834,1100],[1440,900]]) for(const scale of [100,200]) {
      await page.viewport(width,height);
      await page.eval('document.documentElement.style.fontSize='+JSON.stringify(scale+'%')+';document.documentElement.dataset.theme='+JSON.stringify(scale===100?'light':'dark'));
      await page.eval("document.querySelector("+JSON.stringify(reply+'-rich')+").scrollIntoView({block:'center'})");
      check(await page.eval('document.documentElement.scrollWidth<=innerWidth'),'no horizontal overflow '+width+'/'+scale);
      await page.capture('618-reply-'+width+'-'+scale,{target:reply+'-rich',textScaleLane:'dom-override-geometry-only'});
    }
    await page.eval("document.querySelector('a[href=\"/\"]').click()");
    await page.waitFor("!document.querySelector('[data-vink-editor=comment]') && !document.querySelector('#root-composer')",'SPA unmount releases all editor hosts');
    await page.send('HeapProfiler.collectGarbage');
    writeFileSync(join(dirname(args.state),'comment-lifecycle.json'),JSON.stringify({cycles:40,before,heapBefore,viewCount,after:await page.send('Memory.getDOMCounters'),heapAfter:await page.send('Runtime.getHeapUsage'),textbox:{name:box.name,description:box.description}},null,2));
  } finally {page.cdp.listeners=page.cdp.listeners.filter(fn=>fn!==listener);}
}

export async function delayedCommentImport(page,{args,check,workId}) {
  const editor='#root-composer';
  const path='/pkg/vink-editor.js';
  await page.send('Network.clearBrowserCache');
  await page.goto(args.base+'/tac-pham/'+workId);
  await page.waitFor("document.documentElement.dataset.hydrated==='true' && document.querySelector('#root-composer-format')",'cold textarea hydrated');
  const pending=await holdAnswer(page,'GET',path);
  await page.click(editor+'-format');
  check(await pending.held()===200,'held actual editor asset before import');
  await page.click(editor);
  await page.send('Input.imeSetComposition',{text:'ơ',selectionStart:1,selectionEnd:1});
  await pending.finish(false,true);
  await page.send('Input.insertText',{text:'mưa'});
  check(await page.eval("document.activeElement.id==='root-composer' && document.activeElement.value==='mưa' && !document.querySelector('[data-vink-editor=comment]')"),'late runtime preserves focused native IME and committed text');
  await page.goto('about:blank');
  await page.send('Network.clearBrowserCache');
  await page.goto(args.base+'/tac-pham/'+workId);
  await page.waitFor("document.documentElement.dataset.hydrated==='true' && document.querySelector('#root-composer-format')",'fresh document after prior editing session');
  const failed=await holdAnswer(page,'GET',path);
  await page.click(editor+'-format'); await failed.held(); await failed.finish(true);
  await page.waitFor("document.querySelector('.vk-compact-field [role=status]')?.textContent.includes('Chưa tải')",'import failure feedback');
  await page.typeAtEnd(editor,'Nháp khi tải lỗi');
  check(await page.eval("document.querySelector('#root-composer').value==='Nháp khi tải lỗi' && !document.querySelector('[data-vink-editor=comment]')"),'failed import leaves editable textarea');
}
