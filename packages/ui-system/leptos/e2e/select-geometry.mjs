// Browser geometry shared by the app and gallery. Canvas measures the native selected
// option because Chromium does not expose its text fragment as a DOM Range.
export async function selectGeometry(page, selector) {
  return page.eval(`(() => {
    const s=document.querySelector(${JSON.stringify(selector)}), i=s.parentElement.querySelector('svg');
    const a=s.getBoundingClientRect(),b=i.getBoundingClientRect(),c=getComputedStyle(s);
    const canvas=document.createElement('canvas'),ctx=canvas.getContext('2d');ctx.font=[c.fontStyle,c.fontWeight,c.fontSize,c.fontFamily].join(' ');
    const text=s.selectedOptions[0].textContent, spacing=parseFloat(c.letterSpacing)||0;
    return {select:a.toJSON(),icon:b.toJSON(),inset:c.direction==='rtl'?b.left-a.left:a.right-b.right,
      reserve:parseFloat(c.paddingInlineEnd),appearance:c.appearance,
      center:(b.top+b.bottom-a.top-a.bottom)/2,
      gap:c.direction==='rtl'?a.left+parseFloat(c.paddingLeft)-b.right:b.left-(a.right-parseFloat(c.paddingRight)),
      contentWidth:a.width-parseFloat(c.paddingLeft)-parseFloat(c.paddingRight)-parseFloat(c.borderLeftWidth)-parseFloat(c.borderRightWidth),
      textWidth:ctx.measureText(text).width+spacing*text.length,selected:text,
      optionWidths:[...s.options].filter(o=>o.value&&!o.disabled).map(o=>ctx.measureText(o.textContent).width+spacing*o.textContent.length),
      pointerEvents:getComputedStyle(i).pointerEvents, decorative:i.getAttribute('aria-hidden'),
      native:s.tagName==='SELECT',label:s.labels[0]?.textContent,overflow:document.documentElement.scrollWidth>innerWidth};
  })()`);
}

export async function checkSelectGeometry(page, selector, check, scenario, expected = {icon:18,inset:12,gap:8,reserve:38}) {
  await page.eval('document.fonts.ready');
  const r=await selectGeometry(page,selector);
  check(r.native && r.label && r.decorative==='true' && r.pointerEvents==='none' && r.appearance==='none' &&
    Math.abs(r.icon.width-expected.icon)<0.5 && Math.abs(r.icon.height-expected.icon)<0.5 && Math.abs(r.inset-expected.inset)<0.5 &&
    Math.abs(r.reserve-expected.reserve)<0.5 &&
    Math.abs(r.center)<0.5 && Math.abs(r.gap-expected.gap)<0.5 && r.textWidth<=r.contentWidth+1 && r.optionWidths.every(w=>w<=r.contentWidth+1) && !r.overflow,
    `${scenario}: native Select / ${expected.icon}px icon / ${expected.inset}px inset / ${expected.reserve}px reserve / ${expected.gap}px label gap / no clipped option ${JSON.stringify(r)}`);
  return r;
}

// Chromium on macOS handles type-ahead on the closed native control. Its OS popup
// is outside CDP's DOM; do not claim popup arrow navigation from a synthetic change.
export async function selectCharacter(page, letter) {
  // Separate independent prefixes past Chromium's one-second type-ahead buffer.
  await page.eval('new Promise(r=>setTimeout(r,1100))');
  const key = { key: letter, code: `Key${letter.toUpperCase()}`, windowsVirtualKeyCode: letter.toUpperCase().charCodeAt(0) };
  await page.send('Input.dispatchKeyEvent', { type: 'keyDown', ...key, text: letter });
  await page.send('Input.dispatchKeyEvent', { type: 'keyUp', ...key });
  await page.eval('new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)))');
}
