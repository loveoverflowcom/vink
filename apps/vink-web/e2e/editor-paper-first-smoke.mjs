// #218 & #254: real App/PG/Chrome. Scale and theme overrides keep the editor mounted.
// The /toi tests exercise the actual app preference.
export async function editorPaperFirst(page, { args, check, editorTarget, fitsEditorAction }) {
  const summary = '#editor-save-details > summary';
  const ready = `document.querySelector('.cm-content[contenteditable=true]')`;
  const view = `document.querySelector('.cm-content').cmTile.root.view`;
  const title = 'Mưa ở khuỷu đường cũ — tiếng Việt còn nguyên dấu';
  const poem = '  Tiếng Việt — e\u0302\u0301\n\n    Khổ hai vẫn đợi,\n  người về qua mưa.\n';
  await page.viewport(1440);
  await page.goto(`${args.base}/ban-viet`);
  await page.waitFor(`document.querySelector('#create-draft')`, 'Paper First desk');
  await page.click('#create-draft'); await page.waitFor(ready, 'Paper First editor');
  const workId = await page.eval(`location.pathname.split('/').pop()`);
  const context = `document.querySelectorAll('.vk-desk--editor h1').length===1 && document.querySelector('#editor-heading').textContent==='Bản nháp' && document.querySelector('.vk-editor-heading__back').getAttribute('href')==='/ban-viet'`;
  let heldRead;
  const hold = event => {
    if (event.sessionId===page.sessionId && event.method==='Fetch.requestPaused') heldRead=event.params.requestId;
  };
  page.cdp.listeners.push(hold);
  await page.send('Fetch.enable',{patterns:[{urlPattern:`*/api/v1/drafts/${workId}`,requestStage:'Request'}]});
  await page.reload();
  const deadline=Date.now()+10000;
  while (!heldRead && Date.now()<deadline) await new Promise(r=>setTimeout(r,20));
  check(heldRead && await page.eval(`${context} && !document.querySelector('.cm-editor') && document.body.textContent.includes('Đang mở bản nháp…')`),'loading retains a real back link and one document heading');
  await page.capture('paper-loading',{scenario:'held draft GET'});
  await page.send('Fetch.fulfillRequest',{requestId:heldRead,responseCode:503,responseHeaders:[{name:'content-type',value:'application/json'}],body:Buffer.from(JSON.stringify({error:{code:'dependency_unavailable',message:'fixture'}})).toString('base64')});
  await page.waitFor(`document.querySelector('[data-testid=desk-error]')`,'draft read error');
  check(await page.eval(context),'failed load retains back/context independently of the editor form');
  await page.capture('paper-load-error',{scenario:'draft GET 503'});
  await page.send('Fetch.disable'); page.cdp.listeners=page.cdp.listeners.filter(x=>x!==hold);
  await page.send('Network.setBlockedURLs',{urls:[`${args.base}/pkg/vink.js*`,`${args.base}/pkg/vink_bg.wasm*`]});
  await page.send('Emulation.setScriptExecutionDisabled',{value:true});
  try {
    await page.goto(`${args.base}/ban-viet/${workId}`);
    check(await page.eval(`${context} && document.documentElement.dataset.hydrated!=='true' && !document.querySelector('.cm-editor') && document.body.textContent.includes('Bàn viết cần JavaScript')`),'no-JS SSR retains back/H1 and explains how private drafts open');
    await page.capture('paper-no-js',{scenario:'page scripts disabled'});
  } finally {
    await page.send('Emulation.setScriptExecutionDisabled',{value:false});
    await page.send('Network.setBlockedURLs',{urls:[]});
  }
  await page.reload(); await page.waitFor(ready,'editor after load/no-JS probes');
  await page.typeAtEnd('#draft-title', title); await page.typeInBody(poem);
  await page.click('#save'); await page.waitStatus('saved');
  await page.eval(`window.__paperView=${view}; window.__paperTitle=document.querySelector('#draft-title')`);
  const same = `__paperView===${view} && __paperTitle===document.querySelector('#draft-title') && document.querySelectorAll('.cm-editor').length===1`;
  let writes = 0;
  const listener = event => {
    if (event.sessionId===page.sessionId && event.method==='Network.requestWillBeSent' && ['PUT','POST'].includes(event.params.request.method)) writes++;
  };
  page.cdp.listeners.push(listener);
  try {
    for (const state of ['clean','dirty']) {
      if (state==='dirty') await page.typeInBody('Chữ mới chưa lưu.');
      const raw = await page.eval(`${view}.state.doc.toString()`);
      const selection = await page.eval(`${view}.state.selection.toJSON()`);
      for (const scale of [100,150]) for (const theme of ['light','dark']) {
        // The live editor cannot reach Giao diện without leaving. Override the DOM for geometry
        // only; accountAppearance exercises the real radio and persistence on /toi.
        await page.viewport(1440);
        await page.eval('document.documentElement.style.fontSize="' + scale + '%"; document.documentElement.dataset.theme="' + theme + '"');
        await page.waitFor('getComputedStyle(document.documentElement).fontSize==="' + (16 * scale / 100) + 'px"', 'editor root font-size fixture');
        for (const width of [320,390,1024,1440]) {
          await page.viewport(width,900);
          await page.eval(`document.activeElement?.blur(); scrollTo(0,0); document.fonts.ready.then(()=>new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r))))`);
          const stem=`paper-${state}-${width}-${scale}-${theme}`;
          const geometry=await page.eval(`(() => {
            const toolbar=document.querySelector('#editor-toolbar'), paper=document.querySelector('.vk-editor-paper'), back=document.querySelector('.vk-editor-heading__back');
            const t=toolbar.getBoundingClientRect(), p=paper.getBoundingClientRect(), b=back.getBoundingClientRect(), s=getComputedStyle(toolbar), bs=getComputedStyle(back);
            return {toolbar:t.toJSON(),paper:p.toJSON(),back:b.toJSON(),width:document.querySelector('.cm-content').getBoundingClientRect().width,
              transparent:s.backgroundColor==='rgba(0, 0, 0, 0)',inFlow:s.position==='static',
              paperSurface:getComputedStyle(paper).backgroundColor,canvas:getComputedStyle(document.querySelector('.vk-ui-scaffold')).backgroundColor,
              backFilled:bs.backgroundColor!=='rgba(0, 0, 0, 0)' && b.height>=48 && Math.abs(b.left-p.left)<=1.5 && Boolean(back.querySelector('svg')),
              overflow:document.documentElement.scrollWidth>document.documentElement.clientWidth+1,
              h1:document.querySelectorAll('.vk-desk--editor h1').length,
              oneStatus:document.querySelectorAll('#editor-toolbar [role=status]').length===1,
              summaryVisible:document.querySelector('${summary}').checkVisibility(),
              noBannerOrHelpOrExport:!document.querySelector('#build-info, .vk-editor-help, #export, #export-failed, #export-conflict, #export-tags')};
          })()`);
          check(geometry.transparent && geometry.inFlow && geometry.paperSurface!==geometry.canvas && geometry.backFilled && !geometry.overflow && geometry.h1===1 && geometry.oneStatus && geometry.summaryVisible && geometry.noBannerOrHelpOrExport,
            `${stem}: quiet editor keeps chromeless tools, tonal icon Back aligned with paper, separate paper, one H1/readout, visible native trigger and removes banner/help/export ${JSON.stringify(geometry)}`);
          check(await page.eval(same) && await page.eval(`${view}.state.doc.toString()`)===raw && JSON.stringify(await page.eval(`${view}.state.selection.toJSON()`))===JSON.stringify(selection), `${stem}: root-scale override and resize retain raw NFD, stanzas, selection and EditorView`);
          await page.capture(stem,{scenario:'Editor Paper First',textScaleLane:'dom-override-geometry-only',fixture:{title,raw},geometry});
          // Start at the skip-link destination, then let Chrome perform native tab navigation.
          await page.eval("document.querySelector('#noi-dung').focus({ preventScroll: true })");
          await page.key('Tab');
          const backTarget=await editorTarget(page,'.vk-editor-heading__back');
          const backOutline=await page.eval(`(() => {
            const { outlineStyle, outlineWidth } = getComputedStyle(document.querySelector('.vk-editor-heading__back'));
            return { outlineStyle, outlineWidth };
          })()`);
          check(fitsEditorAction(backTarget) && backTarget.focused && backTarget.focusRing &&
            backTarget.rect.width>=48 && backTarget.rect.height>=48 &&
            backOutline.outlineStyle==='solid' && backOutline.outlineWidth==='2px',
            `${stem}: Tab reaches the tonal Back control with a visible 2 px outline ${JSON.stringify({backTarget, backOutline})}`);

          const summaryFocused = `document.activeElement?.matches(${JSON.stringify(summary)})`;
          for (let step = 0; step < 8 && !await page.eval(summaryFocused); step++) await page.key('Tab');
          const closedToolsTarget=await editorTarget(page,summary);
          check(fitsEditorAction(closedToolsTarget) && closedToolsTarget.focused && closedToolsTarget.focusRing &&
            closedToolsTarget.rect.width>=48 && closedToolsTarget.rect.height>=48,
            `${stem}: Tab reaches the closed tools trigger with visible focus ${JSON.stringify(closedToolsTarget)}`);
          await page.key('Enter');
          await page.waitFor(`document.querySelector('#editor-save-details').open`, 'native tools open');
          const toolsTarget=await editorTarget(page,summary);
          check(fitsEditorAction(toolsTarget) && toolsTarget.focused && toolsTarget.focusRing && toolsTarget.rect.width>=48 && toolsTarget.rect.height>=48 && !await page.eval(`Boolean(document.querySelector('#editor-save-details button'))`), `${stem}: opened tools keep server details without download action`);
          await page.key('Escape');
          check(await page.eval(`document.querySelector('#editor-save-details').open && document.activeElement.matches('${summary}') && !document.querySelector('#draft-preview, #preview')`), `${stem}: Escape respects native disclosure semantics`);
          await page.capture(stem+'-tools',{scenario:'Editor native tools',textScaleLane:'dom-override-geometry-only',action:toolsTarget});
          await page.key('Space');
          check(await page.eval(`!document.querySelector('#editor-save-details').open && document.activeElement.matches('${summary}')`), `${stem}: Space closes native disclosure on trigger`);
        }
      }
    }
    // Negative control for #217's focus seam: a summary nested in closed overflow is
    // present in the DOM but cannot receive visible focus. The oracle must reject it.
    check(await page.eval(`document.querySelector('${summary}').checkVisibility()`), 'save handoff starts with a visible destination');
    await page.eval(`window.__hiddenTools=document.createElement('details'); const tools=document.querySelector('#editor-save-details'); tools.before(__hiddenTools); __hiddenTools.append(tools)`);
    check(!await page.eval(`document.querySelector('${summary}').checkVisibility()`), 'focus oracle rejects a summary inside closed overflow');
    await page.eval(`__hiddenTools.replaceWith(document.querySelector('#editor-save-details'))`);

    await page.viewport(320,420);
    await page.eval("document.documentElement.style.fontSize='200%'");
    await page.eval(`document.querySelector('${summary}').scrollIntoView({block:'center'})`);
    const point=await page.eval(`(() => {const r=document.querySelector('${summary}').getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2}})()`);
    await page.send('Emulation.setTouchEmulationEnabled',{enabled:true});
    await page.send('Input.dispatchTouchEvent',{type:'touchStart',touchPoints:[{...point,id:1}]});
    await page.send('Input.dispatchTouchEvent',{type:'touchEnd',touchPoints:[]});
    await page.waitFor(`document.querySelector('#editor-save-details').open`,'touch opens tools in short 320/text200 viewport');
    await page.send('Emulation.setTouchEmulationEnabled',{enabled:false});
    await page.typeInBody(' Lấy chữ mới khi công cụ còn mở.');
    check(await page.eval(`document.querySelector('#editor-save-details').open`),'typing leaves the native tools open');
    check(await page.eval(`!document.querySelector('#preview, #draft-preview') && ${same} && ${view}.state.doc.toString().endsWith(' Lấy chữ mới khi công cụ còn mở.')`),
      'tools retain unsaved text and the only EditorView while standalone preview is absent');
    const {nodes}=await page.send('Accessibility.getFullAXTree');
    check(nodes.some(n=>n.name?.value==='Chi tiết lưu' && n.role?.value==='DisclosureTriangle')
      && !nodes.some(n=>/^Xem trước(?: bản nháp)?$/.test(n.name?.value ?? ''))
      && nodes.some(n=>n.name?.value==='Bàn viết' && n.role?.value==='link'),
    'AX exposes the save disclosure and Back link without a standalone preview action');
    check(writes===0 && await page.status()==='unsaved','tools, resize and typing perform no mutation');
  } finally {
    page.cdp.listeners=page.cdp.listeners.filter(x=>x!==listener);
    await page.viewport(1440);
    await page.eval("document.documentElement.style.fontSize=''; delete document.documentElement.dataset.theme");
  }
  await page.click('#save'); await page.waitStatus('saved');
  check((await page.draft(workId)).title===title,'subsequent explicit save still persists the same draft');
}
