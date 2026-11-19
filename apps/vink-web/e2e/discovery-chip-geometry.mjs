// #260: exercise FilterForm's actual native Checkbox DOM, not preview FilterChip.
export async function discoveryChipGeometry(page, { args, check }) {
  const settle = async () => {
    await page.eval(`document.fonts.ready`);
    await page.eval(`new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)))`);
    await page.eval(`Promise.all(document.getAnimations().filter(a=>Number.isFinite(a.effect.getComputedTiming().endTime)).map(a=>a.finished.catch(()=>{})))`);
  };
  const inspect = selector => page.eval(`(() => {
    const form=document.querySelector(${JSON.stringify(selector)}), rect=e=>{
      const r=e.getBoundingClientRect(); return {x:r.x,y:r.y,width:r.width,height:r.height};
    };
    const origin=rect(form), local=r=>({...r,x:r.x-origin.x,y:r.y-origin.y});
    const chips=[...form.querySelectorAll('.vk-discovery-option')].map(option=>{
      const target=option.querySelector('label'), label=target.querySelector('.vk-ui-choice__label'), input=target.querySelector('input');
      const t=rect(target), l=rect(label), range=document.createRange(); range.selectNodeContents(label);
      const text=range.getBoundingClientRect(), s=getComputedStyle(target), mark=getComputedStyle(target,'::before');
      const reserved=parseFloat(s.paddingLeft)+parseFloat(s.borderLeftWidth)+parseFloat(mark.width);
      return {target:local(t),label:local(l),text:label.textContent,checked:input.checked,disabled:input.disabled,
        centerError:Math.abs(l.x+l.width/2-t.x-t.width/2),
        textCenterError:Math.abs((text.left+text.right)/2-t.x-t.width/2),
        textFits:text.left>=l.x-1 && text.right<=l.x+l.width+1 && text.top>=l.y-1 && text.bottom<=l.y+l.height+1,
        markClear:mark.visibility==='hidden' || text.left>=t.x+reserved-1,
        markVisible:mark.visibility==='visible',
        native:input.type==='checkbox' && input.name==='tag' && !!input.value && input.labels.length===1 && !input.hasAttribute('role'),
        inside:l.x>=t.x && l.x+l.width<=t.x+t.width+1};
    });
    const overlap=chips.some((a,i)=>chips.slice(i+1).some(b=>
      Math.min(a.target.x+a.target.width,b.target.x+b.target.width)-Math.max(a.target.x,b.target.x)>1 &&
      Math.min(a.target.y+a.target.height,b.target.y+b.target.height)-Math.max(a.target.y,b.target.y)>1));
    return {chips,overlap,viewport:{width:innerWidth,height:innerHeight,clientWidth:document.documentElement.clientWidth,dpr:devicePixelRatio},
      font:getComputedStyle(form.querySelector('.vk-ui-choice__label')).font,theme:document.documentElement.dataset.theme,
      overflow:document.documentElement.scrollWidth-document.documentElement.clientWidth,
      formOverflow:form.scrollWidth-form.clientWidth,selected:chips.filter(c=>c.checked).length};
  })()`);
  const unchanged = (a,b) => a.chips.every((chip,i)=>['target','label'].every(key=>
    ['x','y','width','height'].every(axis=>Math.abs(chip[key][axis]-b.chips[i][key][axis])<=1)));
  const geometry = (m,scene) => {
    console.log('  ..  #260 measurements '+scene+' '+JSON.stringify(m));
    check(m.chips.length>=6 && m.chips.every(c=>c.centerError<=1 && c.textCenterError<=1 && c.target.width>=48 && c.target.height>=48 &&
      c.textFits && c.markClear && c.inside && c.native && c.markVisible===c.checked) &&
      !m.overlap && m.overflow<=1 && m.formOverflow<=1, '#260 centered label, separate mark, >=48px nonoverlapping targets and reflow '+scene);
  };
  for (const [width,height] of [[1440,900],[320,900],[390,844],[800,1112]]) {
    for (const theme of ['light','dark']) for (const scale of [100,200]) {
      const scene=[width,height,theme,scale].join('-');
      await page.viewport(width,height);
      await page.goto(args.base+'/');
      await page.waitFor(`document.documentElement.dataset.hydrated==='true' && document.querySelector('#open-tag-filters')`, 'chip hydration/catalog');
      await page.eval(`document.documentElement.dataset.theme=${JSON.stringify(theme)}; document.documentElement.style.fontSize='${scale}%'; scrollTo(0,0)`);
      await settle();
      const form='#modal-tag-form';
      await page.click('#open-tag-filters');
      await page.waitFor(`document.querySelector('#tag-picker').open && document.querySelector('#modal-tag-form input[type=checkbox]')`, 'chip dialog');
      check(await page.eval(`(() => {
        const dialog=document.querySelector('#tag-picker'), r=dialog.getBoundingClientRect();
        const viewportWidth=document.documentElement.clientWidth;
        const centered=Math.abs((r.left+r.right-viewportWidth)/2)<=1 && Math.abs((r.top+r.bottom-innerHeight)/2)<=1;
        const sheet=r.left===0 && Math.abs(r.right-viewportWidth)<=1 && Math.abs(r.bottom-innerHeight)<=1;
        return getComputedStyle(dialog).borderTopWidth==='0px' && dialog.scrollTop===0 &&
          document.activeElement?.id==='tag-picker-title' && (innerWidth<760 ? sheet : centered);
      })()`), '#308 tonal dialog or bottom sheet opens at its title '+scene);
      await settle();
      const initial=await inspect(form), url=await page.eval('location.href');
      await page.capture('chip260-'+scene+'-0',{boundary:'real-FilterForm-native-Checkbox',scenario:'discovery-chip-geometry'});
      geometry(initial,scene+'/0');
      check(initial.selected===0, '#260 initial selection is empty '+scene);
      await page.click(form+' .vk-discovery-option');
      await settle();
      const one=await inspect(form); geometry(one,scene+'/1');
      check(one.selected===1 && unchanged(initial,one) && await page.eval('location.href')===url,
        '#260 one pointer event changes draft once with stable label/chip/row and URL '+scene);
      await page.capture('chip260-'+scene+'-1',{boundary:'real-FilterForm-native-Checkbox',scenario:'discovery-chip-geometry'});
      for (let i=1;i<4;i++) {
        await page.eval(`document.querySelectorAll('${form} input[type=checkbox]')[${i}].focus()`);
        await page.key('Space');
      }
      await settle();
      const four=await inspect(form); geometry(four,scene+'/4');
      check(four.selected===4 && unchanged(initial,four) && four.chips.filter(c=>!c.checked).every(c=>c.disabled),
        '#260 Space toggles once, max4 disables only unselected, geometry stable '+scene);
      await page.capture('chip260-'+scene+'-4',{boundary:'real-FilterForm-native-Checkbox',scenario:'discovery-chip-geometry'});
      const fifth=await page.eval(`(() => {const e=document.querySelectorAll('${form} input[type=checkbox]')[4]; e.labels[0].scrollIntoView({block:'nearest'}); const s=e.labels[0].getBoundingClientRect(); return {x:s.x+s.width/2,y:s.y+s.height/2};})()`);
      await page.send('Input.dispatchMouseEvent',{type:'mousePressed',button:'left',clickCount:1,...fifth});
      await page.send('Input.dispatchMouseEvent',{type:'mouseReleased',button:'left',clickCount:1,...fifth});
      check((await inspect(form)).selected===4, '#260 attempted fifth remains unchecked '+scene);
      await page.eval(`document.querySelector('${form} input[type=checkbox]').focus()`);
      await page.key('Space');
      check((await inspect(form)).selected===3 && await page.eval(`!document.querySelectorAll('${form} input[type=checkbox]')[4].disabled`),
        '#260 deselect at limit re-enables options '+scene);
      // Synthetic long Vietnamese strings exercise the same rendered label element/CSS.
      for (const normalization of ['NFC','NFD']) {
        await page.eval(`document.querySelectorAll('${form} .vk-ui-choice__label').forEach((e,i)=>{e.textContent=('Ký ức mùa thu — gia đình ở đô thị '+i).normalize('${normalization}')})`);
        await settle();
        geometry(await inspect(form),scene+'/long-'+normalization+'-DOM-label-fixture');
        if (normalization==='NFD') await page.capture('chip260-'+scene+'-long',{boundary:'real-FilterForm-with-synthetic-DOM-label-text',scenario:'discovery-chip-geometry'});
      }
      await page.key('Escape');
      await page.waitFor(`!document.querySelector('#tag-picker').open && document.activeElement?.id==='open-tag-filters'`, 'chip Escape focus return');
    }
  }
  await page.viewport(390,844);
  await page.goto(args.base+'/');
  await page.waitFor(`document.documentElement.dataset.hydrated==='true' && document.querySelector('#open-tag-filters')`, 'keyboard chip catalog');
  await page.click('#open-tag-filters');
  await page.waitFor(`document.querySelector('#modal-tag-form input[type=checkbox]')`, 'keyboard chips');
  const form='#modal-tag-form';
  const touch = await page.eval(`(() => {
    const r=document.querySelector('#modal-tag-form label').getBoundingClientRect();
    return {x:r.left+r.width/2,y:r.top+r.height/2};
  })()`);
  await page.send('Emulation.setTouchEmulationEnabled',{enabled:true,maxTouchPoints:1});
  await page.send('Input.dispatchTouchEvent',{type:'touchStart',touchPoints:[touch]});
  await page.send('Input.dispatchTouchEvent',{type:'touchEnd',touchPoints:[]});
  await page.waitFor(`document.querySelector('#modal-tag-form input[type=checkbox]').checked`, '#308 touch selects the native chip');
  await page.send('Emulation.setTouchEmulationEnabled',{enabled:false});
  for (const forced of [false,true]) {
    await page.send('Emulation.setEmulatedMedia',{features:[{name:'forced-colors',value:forced?'active':'none'},{name:'prefers-reduced-motion',value:'reduce'}]});
    await page.eval(`document.querySelector('${form} input[type=checkbox]').focus()`);
    await page.key('Tab'); await page.key('Space'); await settle();
    check(await page.eval(`(() => {const e=document.activeElement,t=e.closest('label'),s=getComputedStyle(t),selected=document.querySelector('${form} input:checked').closest('label');return e.type==='checkbox' && e.matches(':focus-visible') && parseFloat(s.outlineWidth)>=2 && s.outlineStyle!=='none' && getComputedStyle(selected,'::before').visibility==='visible' && getComputedStyle(t,'::before').visibility===(e.checked?'visible':'hidden');})()`),
      '#260 Tab/Space/focus and non-color selection cue under reduced motion / forced-colors='+forced);
    const ax=await page.send('Accessibility.getFullAXTree');
    const names=ax.nodes.filter(n=>n.role?.value==='checkbox').map(n=>n.name?.value);
    const labels=await page.eval(`[...document.querySelectorAll('${form} .vk-ui-choice__label')].map(e=>e.textContent)`);
    // Same >=6 fixture floor as geometry; the page-wide AX names must equal this form's labels
    // exactly, so a missing, duplicated (e.g. hidden dock exposed) or "✓"-prefixed name fails.
    check(labels.length>=6 && names.length===labels.length && JSON.stringify(names.sort())===JSON.stringify(labels.sort()),
      '#260 native checkbox accessible names contain each label once, no decorative mark '+forced+' '+JSON.stringify(names));
    await page.capture('chip260-keyboard-forced-'+forced,{boundary:'Chrome-CDP-accessibility-tree-and-emulated-media',scenario:'discovery-chip-keyboard'});
  }
  await page.send('Emulation.setEmulatedMedia',{features:[]});
  await page.key('Escape');
  await page.viewport(1440,900);
  await page.goto(args.base+'/');
}
