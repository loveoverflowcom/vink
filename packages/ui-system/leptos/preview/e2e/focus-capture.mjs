// Optional evidence helpers for preview-smoke.mjs, using the same page and assertions.
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

export const digest = bytes => createHash('sha256').update(bytes).digest('hex');

export function sourceIdentity() {
  const git = (...args) => execFileSync('git', args, { encoding: 'utf8' }).trim();
  const paths = git('ls-files', '--cached', '--others', '--exclude-standard', '--',
    'packages/ui-system/shared', 'packages/ui-system/leptos', 'packages/vink-patterns/leptos');
  const files = Object.fromEntries(paths.split('\n').filter(Boolean).sort().map(path => [path, digest(readFileSync(path))]));
  return { git_sha: git('rev-parse', 'HEAD'), dirty: Boolean(git('status', '--porcelain')),
    source_fingerprint: digest(JSON.stringify(files)), source_files: files };
}

// Integer CSS bounds, padded and clamped to the real document. Tall specimens are tiled,
// never shrunk or silently truncated; each tile retains its original physical scale.
export function specimenClips(bounds, documentSize, padding = 12, maxHeight = 4096) {
  const values = [bounds.x, bounds.y, bounds.width, bounds.height, documentSize.width,
    documentSize.height, padding, maxHeight];
  if (!values.every(Number.isFinite) || bounds.width <= 0 || bounds.height <= 0
      || documentSize.width <= 0 || documentSize.height <= 0 || padding < 0
      || !Number.isInteger(maxHeight) || maxHeight < 1) throw new Error('invalid specimen geometry');
  const x = Math.max(0, Math.floor(bounds.x - padding));
  const y = Math.max(0, Math.floor(bounds.y - padding));
  const right = Math.min(Math.floor(documentSize.width), Math.ceil(bounds.x + bounds.width + padding));
  const bottom = Math.min(Math.floor(documentSize.height), Math.ceil(bounds.y + bounds.height + padding));
  if (right <= x || bottom <= y) throw new Error('specimen is outside the document');
  const clips = [];
  const step = Math.ceil((bottom - y) / Math.ceil((bottom - y) / maxHeight));
  for (let top = y; top < bottom; top += step) {
    clips.push({ x, y: top, width: right - x, height: Math.min(step, bottom - top), scale: 1 });
  }
  return clips;
}

// Viewport changes can clamp scroll. Keep exact state and the actual focused DOM node;
// no IDs, fabricated clicks or permanent capture-only styles are needed.
export async function captureTransaction(page, capture) {
  const before = await page.eval('({width:innerWidth,height:innerHeight,x:scrollX,y:scrollY})');
  const { result: focus } = await page.send('Runtime.evaluate', { expression: 'document.activeElement' });
  try {
    return await capture(before);
  } finally {
    try {
      await page.viewport(before.width, before.height);
      if (focus.objectId) {
        await page.send('Runtime.callFunctionOn', { objectId: focus.objectId,
          functionDeclaration: 'function(){if(this.isConnected && document.activeElement!==this) this.focus({preventScroll:true})}' });
      }
      await page.eval(`scrollTo(${before.x},${before.y})`);
      await page.settle();
      const restored = await page.eval(`innerWidth===${before.width} && innerHeight===${before.height}
        && Math.abs(scrollX-${before.x})<1 && Math.abs(scrollY-${before.y})<1`);
      const focused = !focus.objectId || (await page.send('Runtime.callFunctionOn', {
        objectId: focus.objectId, functionDeclaration: 'function(){return document.activeElement===this}', returnByValue: true,
      })).result.value;
      if (!restored || !focused) throw new Error('capture did not restore viewport, scroll and focus');
    } finally {
      if (focus.objectId) await page.send('Runtime.releaseObject', { objectId: focus.objectId });
    }
  }
}

async function assertImage(page, bytes, expected) {
  const signature = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);
  if (!bytes.subarray(0, 8).equals(signature) || bytes.readUInt32BE(16) !== expected.width
      || bytes.readUInt32BE(20) !== expected.height) throw new Error('focused PNG dimensions disagree with clip');
  // Decode the actual captured bytes in this browser; a solid/transparent image cannot pass.
  const varied = await page.eval(`new Promise((resolve,reject)=>{
    const image=new Image(); image.onerror=()=>reject(new Error('PNG decode failed'));
    image.onload=()=>{ const canvas=document.createElement('canvas'); canvas.width=96; canvas.height=96;
      const ctx=canvas.getContext('2d'); ctx.drawImage(image,0,0,96,96);
      const data=ctx.getImageData(0,0,96,96).data, colors=new Set();
      for(let i=0;i<data.length;i+=4) if(data[i+3]) colors.add(data[i]+','+data[i+1]+','+data[i+2]);
      resolve(colors.size>8); }; image.src='data:image/png;base64,${bytes.toString('base64')}';
  })`);
  if (!varied) throw new Error('focused PNG is blank or uniform');
}

export async function captureSpecimen(page, record, out) {
  return captureTransaction(page, async before => {
    await page.eval('document.fonts.ready');
    await page.settle();
    const context = await page.eval(`(() => {
      const scope=document.getElementById('specimen-scope'), specimen=document.getElementById('specimen');
      const r=specimen.getBoundingClientRect(), style=getComputedStyle(scope);
      return { catalog_id:scope.dataset.catalogId, scenario_id:scope.dataset.scenarioId,
        requested_theme:document.querySelector('input[name=theme]:checked').dataset.label,
        effective_theme:scope.closest('[data-theme]').dataset.theme, surface:style.getPropertyValue('--vk-surface').trim(),
        root_font_size:getComputedStyle(document.documentElement).fontSize, dpr:devicePixelRatio,
        bounds_css:{x:r.x+scrollX,y:r.y+scrollY,width:r.width,height:r.height},
        document_css:{width:document.documentElement.scrollWidth,height:document.documentElement.scrollHeight},
        readiness:'fonts.ready + two animation frames',
        fonts:['16px "Be Vietnam Pro"','16px "Literata"'].map(f=>[f,document.fonts.check(f)]) };
    })()`);
    if (context.catalog_id !== record.catalog_id || context.scenario_id !== record.scenario_id
        || context.dpr !== 1 || context.effective_theme !== record.theme) {
      throw new Error('focused specimen identity/theme/DPR disagrees with selection');
    }
    const clips = specimenClips(context.bounds_css, context.document_css, 12, Math.min(4096, before.height));
    const artifacts = [];
    for (const [index, clip] of clips.entries()) {
      // CDP captureBeyondViewport can resize the surface and remove the scrollbar, reflowing
      // the long catalog above the specimen. Scroll real viewports instead; never resize.
      await page.eval(`scrollTo(0,${clip.y})`);
      await page.settle();
      const scroll = await page.eval('({x:scrollX,y:scrollY})');
      const viewportClip = { ...clip, x: clip.x - scroll.x, y: clip.y - scroll.y };
      if (viewportClip.x < 0 || viewportClip.y < 0 || viewportClip.x + clip.width > before.width
          || viewportClip.y + clip.height > before.height) throw new Error('focused tile is outside the actual viewport');
      // CDP's clip remains in document coordinates even when captureBeyondViewport is false.
      const { data } = await page.send('Page.captureScreenshot', { format: 'png', clip, captureBeyondViewport: false });
      const bytes = Buffer.from(data, 'base64');
      await assertImage(page, bytes, clip);
      const unchanged = await page.eval(`(() => {const r=document.getElementById('specimen').getBoundingClientRect();
        return Math.abs(r.x+scrollX-${context.bounds_css.x})<1 && Math.abs(r.y+scrollY-${context.bounds_css.y})<1
          && Math.abs(r.width-${context.bounds_css.width})<1 && Math.abs(r.height-${context.bounds_css.height})<1;})()`);
      if (!unchanged) throw new Error('capture changed specimen geometry');
      const file = `${record.raw_name}--focus${clips.length > 1 ? `--part${index + 1}` : ''}.png`;
      writeFileSync(join(out, file), bytes);
      artifacts.push({ path: file, sha256: digest(bytes), byte_size: bytes.length,
        image_pixels: { width: clip.width, height: clip.height }, clip_css: clip, viewport_clip_css: viewportClip,
        scroll_css: scroll, tile: index + 1, tiles: clips.length });
    }
    const raw = readFileSync(join(out, `${record.raw_name}.png`));
    return { ...context, viewport_css: { width: before.width, height: before.height },
      scroll_css: { x: before.x, y: before.y }, padding_css: 12, capture_mode: 'specimen',
      raw_artifact: { path: `${record.raw_name}.png`, sha256: digest(raw), byte_size: raw.length,
        image_pixels: { width: raw.readUInt32BE(16), height: raw.readUInt32BE(20) }, capture_mode: 'document-capped' },
      artifacts, captured_at: new Date().toISOString(), inspected: false };
  });
}
