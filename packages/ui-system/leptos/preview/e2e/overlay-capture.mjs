// Dialog/sheet regression inside the existing preview smoke. Keep the real viewport: resizing
// to the document height hides focus-induced scroll defects in viewport-anchored overlays.
import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { digest, sourceIdentity } from './focus-capture.mjs';

export function contains(outer, inner) {
  return [outer, inner].every(r => r && [r.x, r.y, r.width, r.height].every(Number.isFinite)
    && r.width > 0 && r.height > 0)
    && inner.x >= outer.x && inner.y >= outer.y
    && inner.x + inner.width <= outer.x + outer.width
    && inner.y + inner.height <= outer.y + outer.height;
}

export function openingInvariants(m) {
  return {
    'INV-1': contains(m.panel, m.title) && contains(m.viewport, m.title),
    'INV-2': contains(m.viewport, m.panel),
    'INV-4': m.documentWidth <= m.viewport.width && m.scrollWidth <= m.clientWidth,
  };
}

async function measure(page) {
  return page.eval(`(() => {
    const d = document.querySelector('dialog[open]'), scope = document.getElementById('specimen-scope');
    const rect = el => { const r = el.getBoundingClientRect(); return { x:r.x,y:r.y,width:r.width,height:r.height }; };
    const style = el => { const s = getComputedStyle(el); return {family:s.fontFamily,size:s.fontSize,lineHeight:s.lineHeight}; };
    return { viewport:{x:0,y:0,width:innerWidth,height:innerHeight}, panel:rect(d),
      title:rect(d.querySelector('h2')), content:rect(d.querySelector('.vk-ui-dialog__panel')),
      actions:rect(d.querySelector('.vk-ui-dialog__actions')), font:style(d.querySelector('h2')),
      scrollTop:d.scrollTop, scrollHeight:d.scrollHeight, clientHeight:d.clientHeight,
      scrollWidth:d.scrollWidth, clientWidth:d.clientWidth, documentWidth:document.documentElement.scrollWidth,
      documentScroll:{x:scrollX,y:scrollY}, active:document.activeElement.id || document.activeElement.textContent.trim(),
      rootFontSize:getComputedStyle(document.documentElement).fontSize, dpr:devicePixelRatio,
      theme:scope.closest('[data-theme]').dataset.theme, surface:getComputedStyle(scope).getPropertyValue('--vk-surface').trim(),
      fonts:['16px "Be Vietnam Pro"','16px "Literata"'].map(f=>[f,document.fonts.check(f)]) };
  })()`);
}

async function capture(page, out, name, metadata) {
  await page.eval('document.fonts.ready');
  await page.settle();
  const measurements = await measure(page);
  const { data } = await page.send('Page.captureScreenshot', { format:'png', captureBeyondViewport:false });
  const bytes = Buffer.from(data, 'base64');
  if (bytes.readUInt32BE(16) !== measurements.viewport.width || bytes.readUInt32BE(20) !== measurements.viewport.height) {
    throw new Error('overlay PNG does not match the real viewport');
  }
  // Draw on a detached canvas from the clean PNG; never inject markers/styles into the UI.
  const annotated = await page.eval(`new Promise((resolve,reject) => {
    const image = new Image(); image.onerror = reject; image.onload = () => {
      const c = document.createElement('canvas'); c.width=image.width; c.height=image.height;
      const ctx=c.getContext('2d'); ctx.drawImage(image,0,0); ctx.lineWidth=2; ctx.font='12px sans-serif';
      const m=${JSON.stringify(measurements)};
      for (const [id,color] of [['panel','#0575e6'],['title','#d00068'],['actions','#00804a']]) {
        const r=m[id]; ctx.strokeStyle=color; ctx.strokeRect(r.x,r.y,r.width,r.height);
        const y=Math.max(14,Math.min(c.height-4,r.y+14)); ctx.fillStyle='white';ctx.fillRect(r.x,y-12,64,15);
        ctx.fillStyle=color;ctx.fillText('#'+id,r.x+2,y);
      }
      resolve(c.toDataURL('image/png').split(',')[1]);
    }; image.src='data:image/png;base64,${data}';
  })`);
  if (JSON.stringify(await measure(page)) !== JSON.stringify(measurements)) {
    throw new Error('overlay capture changed geometry, scroll or focus');
  }
  const overlay = Buffer.from(annotated, 'base64');
  writeFileSync(join(out, `${name}.png`), bytes);
  writeFileSync(join(out, `${name}--overlay.png`), overlay);
  const artifact = (file, content) => ({file, sha256:digest(content), byte_size:content.length,
    image_pixels:{width:measurements.viewport.width,height:measurements.viewport.height}});
  const context = {...metadata, captured_at:new Date().toISOString(), capture_mode:'viewport',
    coordinate_space:'viewport-css', unit:'CSS px', provenance:'measured: DOMRect/computed style/scroll metrics',
    readiness:'fonts.ready + two animation frames', pointer:{x:0,y:0,source:'CDP mouseMoved, outside panel'},
    inspected:false, measurements, clean:artifact(`${name}.png`,bytes),
    overlay:{...artifact(`${name}--overlay.png`,overlay), derived_from:`${name}.png`, provenance:'derived: bounds drawn on clean PNG'}};
  page.contexts[name] = context;
  return context;
}

export async function overlayCaptures(page, {out, version, check}) {
  mkdirSync(out, {recursive:true});
  const run = {...sourceIdentity(), browser:version.product, host:'headless-chrome', renderer:'leptos-web',
    data_mode:'synthetic', locale:'vi-VN', text_scale_method:'root font size; not browser zoom'};
  const cases = [], failures = [];
  const save = () => writeFileSync(join(out, 'overlay-contexts.json'), JSON.stringify({run, cases}, null, 2));
  try {
    for (const theme of ['light','dark']) for (const scale of [100,200]) {
      for (const [width,height] of [[320,700],[390,844]]) for (const scenario of ['default','sheet']) {
        const name = `state.dialog--${scenario}--open--${theme}--${width}x${height}--text${scale}`;
        const item = {name, theme, text_scale:scale, catalog_id:'state.dialog', scenario_id:scenario,
          expected_viewport:{width,height}, checks:{}, captures:[]};
        cases.push(item);
        const record = (id, result) => { item.checks[id]=Boolean(result); if (!result) failures.push(`${name}: ${id}`); };
        await page.choose('theme', theme === 'dark' ? 'Tối' : 'Sáng');
        await page.choose('scale', `${scale}%`);
        await page.viewport(width,height);
        await page.open('state.dialog',scenario);
        const open = async () => {
          await page.eval(`document.querySelector('[data-testid=dialog-open] button').focus()`);
          await page.key('Enter');
          await page.send('Input.dispatchMouseEvent', {type:'mouseMoved',x:0,y:0});
          await page.settle();
        };
        await open();
        const initial = await capture(page,out,name,{theme,text_scale:scale,state:'just opened; no corrective scroll'});
        item.captures.push(initial);
        for (const [id,result] of Object.entries(openingInvariants(initial.measurements))) record(id,result);
        record('identity/fonts', initial.measurements.theme === theme && initial.measurements.rootFontSize === `${scale * .16}px`
          && initial.measurements.viewport.width === width && initial.measurements.viewport.height === height
          && initial.measurements.dpr === 1 && initial.measurements.fonts.every(([,ready])=>ready));
        record('initial focus', await page.eval(`(() => {const d=document.querySelector('dialog[open]');
          return d.scrollHeight>d.clientHeight ? document.activeElement===d.querySelector('h2')
            : document.activeElement===d.querySelector('.vk-ui-dialog__actions button');})()`));
        const count = await page.eval(`document.querySelectorAll('dialog[open] button').length`);
        const reached = new Set();
        let contained = true, visible = true;
        for (let n=0;n<2*(count+1);n++) {
          await page.key('Tab');
          const state = await page.eval(`(() => {const d=document.querySelector('dialog[open]'), a=document.activeElement;
            const index=[...d.querySelectorAll('button')].indexOf(a), r=a.getBoundingClientRect(), p=d.getBoundingClientRect();
            return {index,contained:d.contains(a)||a===document.body,visible:index<0||
              (r.x>=p.x&&r.y>=p.y&&r.right<=p.right&&r.bottom<=p.bottom)};})()`);
          if (state.index>=0) reached.add(state.index);
          contained &&= state.contained; visible &&= state.visible;
        }
        record('Tab containment',contained);
        record('INV-3 actions reachable by keyboard',visible && reached.size===count);
        await page.eval(`[...document.querySelectorAll('dialog[open] .vk-ui-dialog__actions button')].at(-1).scrollIntoView({block:'end'})`);
        item.captures.push(await capture(page,out,`${name}--actions`,{theme,text_scale:scale,state:'actions reached; explicit scroll after initial capture'}));
        await page.key('Escape');
        record('Escape + return focus',await page.eval(`!document.querySelector('dialog[open]') &&
          document.activeElement===document.querySelector('[data-testid=dialog-open] button')`));
        await open();
        const reopened = await measure(page);
        record('reopen starts at title',openingInvariants(reopened)['INV-1'] && reopened.scrollTop===0);
        for (const type of ['mousePressed','mouseReleased']) {
          await page.send('Input.dispatchMouseEvent',{type,x:1,y:1,button:'left',clickCount:1});
        }
        await page.settle();
        record('outside policy',await page.eval(`Boolean(document.querySelector('dialog[open]'))`) === (scenario==='default'));
        if (scenario==='default') {
          await page.click('[data-testid=dialog-keep] button');
          await page.settle();
          record('safe action closes',!(await page.eval(`Boolean(document.querySelector('dialog[open]'))`)));
        }
        save();
      }
    }
    check(sourceIdentity().source_fingerprint === run.source_fingerprint, 'overlay source stayed unchanged');
    check(failures.length===0, `overlay matrix 16 cases: ${failures.length ? failures.join('; ') : 'opening geometry, keyboard, cancel and reopen PASS'}`);
  } finally {
    save();
  }
  await page.choose('scale','100%');
  await page.viewport(1440);
}
