// #193: the title stays a native single-line input. Its expanded reading projection is
// deliberately not another editing buffer. Real App/PostgreSQL, synthetic input via CDP.
export async function editorWriting(page, { args, check, setEditorScene, captureEditorScene, editorTarget, fitsEditorAction }) {
  const titleSelector = '#draft-title';
  const summary = '#draft-title-details > summary';
  const full = '#draft-title-full';
  const disclosureVisible = `document.querySelector('${summary}')?.checkVisibility() === true`;
  const ready = `document.querySelector('.cm-content[contenteditable=true]')`;
  // Pinned CM 6 DOM bridge (also used by EditorView.findFromDOM). Read the actual view,
  // including non-rendered lines: DOM text alone cannot prove exact raw-buffer retention.
  const view = `document.querySelector('.cm-content').cmTile.root.view`;
  const raw = `${view}.state.doc.toString()`;
  const selection = `JSON.stringify([__writingTitle.selectionStart,__writingTitle.selectionEnd,__writingTitle.selectionDirection])`;
  const title = 'Những ngày mưa ở khuỷu đường cũ — '.repeat(6).slice(0, 200);
  const body = '  Thơ nghiêng — tiếng Việt\n\n\n    Khổ hai\n  dòng thụt vào\n\n' + 'Đoạn văn dài vẫn giữ dấu, khoảng trắng và dấu chấm. '.repeat(20);
  await page.viewport(1440);
  await page.goto(`${args.base}/ban-viet`);
  await page.waitFor(`document.querySelector('#create-draft') && document.documentElement.dataset.hydrated === 'true'`, 'writing desk');
  await page.click('#create-draft'); await page.waitFor(ready, 'writing editor');
  const workId = await page.eval(`location.pathname.split('/').pop()`);
  check(!await page.eval(disclosureVisible), 'empty title has no full-title disclosure');
  const shortTitle = 'Buổi tối an lành';
  await page.typeAtEnd(titleSelector, shortTitle);
  await setEditorScene(page,{width:1440,height:900,scale:100,theme:'light'});
  check(!await page.eval(disclosureVisible), 'audit short title fits at desktop and has no disclosure');
  const {nodes: shortTree}=await page.send('Accessibility.getFullAXTree');
  check(!shortTree.some(n=>!n.ignored && n.name?.value==='Xem trọn tiêu đề'), 'fitting title omits the disclosure from the accessibility tree');
  await page.key('Tab');
  check(await page.eval(`document.activeElement.matches('.cm-content')`), 'Tab from a fitting title reaches the editor body directly');

  await setEditorScene(page,{width:320,height:900,scale:100,theme:'light'});
  check(!await page.eval(disclosureVisible), 'the short title still fits at 320px and 100% text');
  // Swap to a face with larger advances at the same CSS font size and viewport width.
  await page.eval(`(async () => {
    const input=document.querySelector('${titleSelector}'), paper=input.closest('.vk-editor-paper');
    window.__titleFitWidth=input.clientWidth;
    const fallback=getComputedStyle(input).fontFamily;
    paper.style.setProperty('--vk-font-reading','"Vink Wide Title Test", '+fallback);
    const face=new FontFace('Vink Wide Title Test','url(/pkg/fonts/literata-variable.woff2)',{sizeAdjust:'240%',weight:'200 900'});
    window.__wideTitleFace=await face.load();
    document.fonts.add(__wideTitleFace);
  })()`);
  await page.waitFor(disclosureVisible,'wider loaded font clips the same title');
  check(await page.eval(`document.querySelector('${titleSelector}').clientWidth===__titleFitWidth`), 'font metrics reveal disclosure without changing the input width');
  await page.eval(`document.fonts.delete(__wideTitleFace); document.querySelector('.vk-editor-paper').style.removeProperty('--vk-font-reading')`);
  await page.waitFor(`!(${disclosureVisible})`,'original font fits the same title again');

  await setEditorScene(page,{width:320,height:900,scale:200,theme:'light'});
  await page.waitFor(disclosureVisible,'the same title clips after increasing the text size');
  await page.eval(`window.__writingTitle=document.querySelector('${titleSelector}'); __writingTitle.focus(); __writingTitle.setSelectionRange(1,5,'backward')`);
  const shortSelection=await page.eval(selection);
  await page.key('Tab'); await page.key('Enter');
  check(await page.eval(`document.activeElement.matches('${summary}') && document.querySelector('#draft-title-details').open`), 'Tab and Enter reach and open the disclosure for the clipped title');
  await setEditorScene(page,{width:1440,height:900,scale:200,theme:'light'});
  await page.waitFor(`!(${disclosureVisible})`,'wider viewport fits the title again');
  check(await page.eval(`document.activeElement===__writingTitle && ${selection}===${JSON.stringify(shortSelection)}`), 'hiding the focused disclosure returns focus to the same title and keeps directional selection');
  await setEditorScene(page,{width:320,height:900,scale:200,theme:'light'});
  await page.waitFor(disclosureVisible,'narrowing the viewport clips the title again');
  check(await page.eval(`document.activeElement===__writingTitle && !document.querySelector('#draft-title-details').open`), 'showing the disclosure again leaves the caret and returns the disclosure to collapsed');

  await setEditorScene(page,{width:1440,height:900,scale:100,theme:'light'});
  await page.eval(`__writingTitle.select()`);
  await page.send('Input.insertText',{text:title});
  await page.waitFor(disclosureVisible,'editing from a fitting title to a long title reveals its disclosure');
  await page.typeInBody(body);
  await page.click('#save'); await page.waitStatus('saved');
  const saved = await page.draft(workId);
  await page.eval(`window.__writingTitle=document.querySelector('#draft-title'); window.__writingView=${view}; __writingTitle.focus(); __writingTitle.setSelectionRange(10,24,'backward')`);
  const selected = await page.eval(selection);
  const beforeRaw = await page.eval(raw);
  const bodySelection = await page.eval(`${view}.state.selection.toJSON()`);
  check(beforeRaw === body, 'raw poem retains multiple blank lines, indent and prose after ACK');
  let writes = 0;
  const listener = event => {
    if (event.sessionId === page.sessionId && event.method === 'Network.requestWillBeSent' && event.params.request.method === 'PUT' && event.params.request.url.endsWith(`/drafts/${workId}`)) writes++;
  };
  page.cdp.listeners.push(listener);
  const same = `__writingTitle===document.querySelector('#draft-title') && __writingView===${view} && document.querySelectorAll('.cm-editor').length===1`;
  try {
    await page.click(summary);
    for (const width of [320,390,834,1440]) for (const scale of [100,200]) for (const theme of ['light','dark']) {
      const scene = {width,height:900,scale,theme};
      await setEditorScene(page,scene);
      const geometry = await page.eval(`(() => {
        const p=document.querySelector('${full}'), r=p.getBoundingClientRect(), range=document.createRange(); range.selectNodeContents(p);
        const top=document.querySelector('${summary}').getBoundingClientRect().bottom, bottom=document.querySelector('#edit-full-title').getBoundingClientRect().top;
        const owners=['.vk-main','.vk-desk--editor','.vk-editor-paper','.vk-ui-field__control','.vk-editor-body','.cm-scroller','.cm-content'];
        return {width:document.querySelector('.cm-content').getBoundingClientRect().width,
          titleFont:getComputedStyle(__writingTitle).fontSize, bodyFont:getComputedStyle(document.querySelector('.cm-scroller')).fontSize,
          owners:owners.map(s=>{const e=document.querySelector(s);return {selector:s,client:e.clientWidth,scroll:e.scrollWidth}}), rects:[...range.getClientRects()].map(t=>t.toJSON()), text:p.textContent, title:r.toJSON(), lineHeight:getComputedStyle(p).lineHeight,
          fits:[...range.getClientRects()].every(t=>t.left>=r.left-1 && t.right<=r.right+1 && t.top>=top-1 && t.bottom<=bottom+1),
          overflow:document.documentElement.scrollWidth>document.documentElement.clientWidth+1 || owners.some(s=>{const e=document.querySelector(s);return e.scrollWidth>e.clientWidth+1})};
      })()`);
      check(geometry.text === title && geometry.fits && !geometry.overflow, `${width}/${scale}/${theme}: entire title wraps without clipping or horizontal page/owner overflow${geometry.fits && !geometry.overflow ? '' : ' '+JSON.stringify(geometry)}`);
      // Reproduced a677b0d computed fonts: the reading token is fluid with viewport.
      const baselineBody = scale===100 ? {320:17,390:17,834:18.085,1440:19.6} : {320:34,390:34,834:34.085,1440:35.6};
      check(geometry.titleFont === `${28*scale/100}px` && Math.abs(parseFloat(geometry.bodyFont)-baselineBody[width])<0.02, 'title and body keep the reproduced font scale');
      if (width <= 390 && scale === 200) check(geometry.width > (width === 320 ? 193 : 263), 'mobile writing width exceeds the reproduced baseline through insets');
      check(await page.eval(same) && await page.eval(selection) === selected && await page.eval(raw) === beforeRaw && JSON.stringify(await page.eval(`${view}.state.selection.toJSON()`)) === JSON.stringify(bodySelection), 'layout keeps actual EditorView, exact raw buffer and directional title selection');
      for (const [stem,selector] of [['writing-title-full',full],['writing-body','.cm-content']]) {
        await page.eval(`document.querySelector('${selector}').scrollIntoView({block:'start',behavior:'instant'})`);
        await captureEditorScene(page,stem,scene,{fixture:{title,body},geometry,anchor:selector});
        if (selector===full && width<=390 && scale===200) {
          const start=await page.eval('scrollY');
          const height=geometry.title.height;
          for(let offset=700,tile=2; offset<height; offset+=700,tile++) {
            await page.eval(`scrollTo({top:${start+offset},behavior:'instant'})`);
            await captureEditorScene(page,'writing-title-tile-'+tile,scene,{fixture:{title,body},geometry,anchor:selector,tile});
          }
        }
      }
      await page.eval(`document.querySelector('#edit-full-title').scrollIntoView({block:'center',behavior:'instant'})`);
      check(fitsEditorAction(await editorTarget(page,'#edit-full-title')), 'edit-title action remains reachable and hit-testable');
    }
    await page.click('#edit-full-title');
    check(await page.eval(`document.activeElement===__writingTitle && ${selection}===${JSON.stringify(selected)}`), 'explicit edit returns to the same native input and backward selection');
    await page.click(summary); await page.key('Space');
    check(await page.eval(`document.querySelector('#draft-title-details').open && document.activeElement.matches('${summary}')`), 'native title disclosure supports Space and retains trigger focus');
    const after = await page.draft(workId);
    check(writes===0 && after.version===saved.version && JSON.stringify(after.document)===JSON.stringify(saved.document) && await page.status()==='saved', 'layout/disclosure neither marks dirty nor writes nor changes the saved document projection');

    // Native title history and caret in the middle of an NFD string, mirrored verbatim.
    await setEditorScene(page,{width:390,height:420,scale:200,theme:'dark'});
    const replace = async text => {
      await page.eval(`__writingTitle.focus(); __writingTitle.select()`);
      await page.send('Input.insertText',{text});
    };
    const nfd='Tiếng Việt — ở khuỷu đường'.normalize('NFD');
    await replace(nfd);
    await page.eval(`__writingTitle.setSelectionRange(5,5)`);
    await page.send('Input.insertText',{text:'X'});
    const edited=nfd.slice(0,5)+'X'+nfd.slice(5);
    check(await page.eval(`__writingTitle.value===${JSON.stringify(edited)} && document.querySelector('${full}').textContent===__writingTitle.value && __writingTitle.selectionStart===6`), 'middle insertion preserves NFD, caret and the live full-title projection');
    for (const [command,expected] of [['undo',nfd],['redo',edited]]) {
      await page.send('Input.dispatchKeyEvent',{type:'keyDown',key:'z',code:'KeyZ',modifiers:page.mod,commands:[command]});
      await page.send('Input.dispatchKeyEvent',{type:'keyUp',key:'z',code:'KeyZ',modifiers:page.mod});
      check(await page.eval(`__writingTitle.value===${JSON.stringify(expected)} && document.querySelector('${full}').textContent===__writingTitle.value`), `native ${command} updates the same title state`);
    }
    await page.key('ArrowLeft',8);
    check(await page.eval(`__writingTitle.selectionDirection==='backward' && __writingTitle.selectionEnd>__writingTitle.selectionStart`), 'Shift+Left retains a directional native selection');
    await page.click('.vk-editor-heading__back'); await page.waitFor(`document.querySelector('#leave-editor').open`,'title leave guard');
    const held=await page.eval(selection); await page.key('Escape');
    await page.waitFor(`document.activeElement===__writingTitle`,'cancel returns title focus');
    check(await page.eval(selection)===held && await page.eval(same), 'cancel preserves title selection and editor identity');

    // Compare committed insertion with an unbound native input, including its real UTF-16
    // maxlength behavior. This is not clipboard/Word/Docs or OS IME evidence.
    await page.eval(`window.__nativeTitle=document.createElement('input'); __nativeTitle.maxLength=200; document.body.append(__nativeTitle)`);
    for (const sample of ['ắ'.repeat(199),'ắ'.repeat(200),'ắ'.repeat(201),'a'.repeat(198)+'😀','a'.repeat(199)+'😀',nfd,'a'.repeat(200),'  Tiếng  Việt  ','a\nb','a\r\nb','a\tb','a\u2028b','a\u2029b']) {
      await page.eval(`__nativeTitle.focus(); __nativeTitle.select()`); await page.send('Input.insertText',{text:sample});
      const native=await page.eval(`__nativeTitle.value`);
      await replace(sample);
      check(await page.eval(`__writingTitle.value===${JSON.stringify(native)} && document.querySelector('${full}').textContent===__writingTitle.value && __writingTitle.maxLength===200 && __writingTitle.name==='title' && __writingTitle.placeholder==='Chưa đặt tên' && __writingTitle.autocomplete==='off' && __writingTitle.labels[0].textContent==='Tiêu đề' && ${same}`), 'title retains native insertion/limit/label/name/identity for '+JSON.stringify(sample.slice(0,24)));
    }
    await page.eval(`__nativeTitle.remove()`);
    // API boundary is deliberately independent of HTML's UTF-16 restriction.
    for (const [sample,accepted] of [['ắ'.repeat(199),true],['ắ'.repeat(200),true],['ắ'.repeat(201),false],['ắ'.repeat(200).normalize('NFD'),true],['😀'.repeat(200),true],['  Tiếng Việt  ',true],['a\nb',false],['a\r\nb',false],['a\tb',false],['a\u2028b',false],['a\u2029b',false]]) {
      const result=await page.eval(`(async()=>{ const created=await fetch('/api/v1/drafts',{method:'POST'}).then(r=>r.json()); const r=await fetch('/api/v1/drafts/'+created.work_id,{method:'PUT',headers:{'content-type':'application/json'},body:JSON.stringify({expected_version:created.version,title:${JSON.stringify(sample)},document:${JSON.stringify(saved.document)}})}); return {ok:r.ok,status:r.status,body:await r.json(),stored:await fetch('/api/v1/drafts/'+created.work_id).then(r=>r.json())}; })()`);
      check(result.ok===accepted && (accepted ? result.stored.title===sample.trim().normalize('NFC') && result.stored.version===2 : result.status===422 && result.stored.version===1), `real API title trim/NFC/scalar boundary ${sample.length}/${[...sample].length} accepted=${accepted} ${result.ok===accepted ? '' : JSON.stringify(result)}`);
    }
    await replace(title);
    await page.waitFor(disclosureVisible,'the long title is clipped before shortening');
    await replace('m');
    await page.waitFor(`!(${disclosureVisible})`,'shortening the title hides its disclosure');
    check(await page.eval(`document.activeElement===__writingTitle && __writingTitle.selectionStart===1 && __writingTitle.selectionEnd===1`), 'hiding the disclosure during typing leaves the caret in the title');
    const priorWrites=writes;
    await page.send('Input.imeSetComposition',{text:'ư',selectionStart:1,selectionEnd:1});
    check(await page.eval(`document.activeElement===__writingTitle && __writingTitle.value==='mư' && ${same}`), 'composition keeps focus and preedit text in the same title input');
    await page.key('Enter'); await page.key('s',page.mod);
    check(writes===priorWrites,'title composition Enter/shortcut never saves partial syllables');
    await page.send('Input.insertText',{text:'ưa'});
    await page.key('Enter'); await page.waitStatus('saved');
    check(writes===priorWrites+1 && (await page.draft(workId)).title===await page.eval(`__writingTitle.value`),'title Enter after composition sends exactly one acknowledged save');
    await replace(nfd);
    await page.key('s',page.mod); await page.waitStatus('saved');
    check((await page.draft(workId)).title===nfd.normalize('NFC') && await page.eval(`__writingTitle.value`)===nfd,'shortcut saves NFC at server without overwriting the live NFD title');
    await page.click('#preview'); await page.waitFor(`document.querySelector('#draft-preview').open`,'title preview');
    check(await page.eval(`document.querySelector('#draft-preview').textContent.includes(${JSON.stringify(nfd)})`),'preview reads the current title state');
    await page.key('Escape');
    check(!await page.eval(`Boolean(document.querySelector('#export, #export-failed, #export-conflict, #export-tags'))`), 'no draft export button remains in quiet editor');

    check(await page.eval(raw)===beforeRaw && await page.eval(same),'title corpus, preview and save do not change raw body or view');
    const beforeEnter=writes;
    const tail='\n  Dòng cuối thụt vào';
    await page.typeInBody(tail);
    // Separate the two real history groups; Enter itself must not copy the indent.
    await new Promise(resolve=>setTimeout(resolve,700));
    await page.key('Enter');
    check(await page.eval(raw)===beforeRaw+tail+'\n' && writes===beforeEnter,'body Enter adds only a newline, never submits or copies indent');
    await page.key('z',page.mod);
    check(await page.eval(raw)===beforeRaw+tail,'body undo reverses Enter after title and layout interactions');
    await page.key('Z',page.mod|8);
    check(await page.eval(raw)===beforeRaw+tail+'\n','body redo retains the same history');
    await page.key('z',page.mod); await page.key('z',page.mod);
    check(await page.eval(raw)===beforeRaw && await page.eval(same),'body undo restores the exact original raw buffer');
    await page.key('s',page.mod); await page.waitStatus('saved');
    await page.waitFor(disclosureVisible,'the long title disclosure remains reachable for focus testing');
    for (const forced of [false,true]) {
      await page.send('Emulation.setEmulatedMedia',{features:[{name:'forced-colors',value:forced?'active':'none'},{name:'prefers-reduced-motion',value:'reduce'}]});
      await page.eval(`document.querySelector('${summary}').focus()`); await page.key('Tab'); await page.key('Tab',8);
      const g=await editorTarget(page,summary);
      check(g.focused && g.focusRing && g.full && g.hit,'title disclosure keyboard focus is visible '+(forced?'in forced colors':'in dark theme'));
      await captureEditorScene(page,'writing-focus-'+(forced?'forced':'normal'),{width:390,height:420,scale:200,theme:'dark'},{target:g});
    }
    const {nodes}=await page.send('Accessibility.getFullAXTree');
    check(nodes.some(n=>n.role?.value==='textbox' && n.name?.value==='Tiêu đề') && nodes.some(n=>n.name?.value==='Xem trọn tiêu đề'),'accessibility tree exposes title label and full-title disclosure');
  } catch(error) {
    await page.capture('writing-failure');
    throw error;
  } finally {
    page.cdp.listeners=page.cdp.listeners.filter(x=>x!==listener);
    await page.send('Emulation.setEmulatedMedia',{features:[]});
    await setEditorScene(page,{width:1440,height:900,scale:100,theme:'light'});
  }
}
