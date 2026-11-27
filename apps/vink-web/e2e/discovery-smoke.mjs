// TAG-PR-06. Real server reads unless a case explicitly says transport fixture.
// SQL publications in the dedicated phase qualify public reading, never author publication.
import { discoveryChipGeometry } from './discovery-chip-geometry.mjs';
import { holdAnswer } from './auth-browser-network.mjs';
import { collectElementAccessibility } from './visual-evidence.mjs';
async function publicLoading(page, args, check) {
  // This is a client navigation into public results; the server resolves direct SSR entry.
  await page.goto(args.base + '/?mode=authors');
  await page.waitFor("document.documentElement.dataset.hydrated === 'true' && document.querySelector('#explore-mode-works')", 'authors ready for public navigation');
  const held = await holdAnswer(page, 'GET', '/api/v1/works');
  await page.click('#explore-mode-works');
  check(await held.held() === 200, 'All results waits on the public list provider');
  const arc = "document.querySelector('[data-testid=discovery-loading] .vk-ui-arc svg')";
  await page.waitFor(arc, 'public loading gate elapsed');
  const shell = () => page.eval("JSON.stringify([...document.querySelectorAll('.vk-explore__top, .vk-explore-modes, #bottom-navigation')].map(el=>{const {x,y,width,height}=el.getBoundingClientRect();return [x,y,width,height]}))");
  const before = await shell();
  const focus = await page.eval('document.activeElement.id');
  await page.send('Emulation.setEmulatedMedia', {features:[{name:'prefers-reduced-motion',value:'no-preference'}]});
  check(await page.eval("getComputedStyle("+arc+").animationName==='vk-ui-spin' && getComputedStyle("+arc+").animationIterationCount==='infinite' && !!"+arc+".closest('[aria-busy=true]') && !document.querySelector('[data-feed-state=loading]')"), 'public results expose their own active MicroArc');
  await page.send('Emulation.setEmulatedMedia', {features:[{name:'prefers-reduced-motion',value:'reduce'}]});
  check(await page.eval("getComputedStyle("+arc+").animationIterationCount==='0' || getComputedStyle("+arc+").animationName==='none'"), 'reduced motion leaves a static public indicator');
  await held.finish();
  await page.waitFor("document.querySelector('#public-results') && !document.querySelector('[data-testid=discovery-loading]')", 'All results releases independently');
  check(before === await shell() && focus === await page.eval('document.activeElement.id'), 'loading and success preserve shell geometry and focus');
  await page.send('Emulation.setEmulatedMedia', {features:[]});
}

async function invalidCursorRecovery(page, args, check) {
  const invalid = await fetch(args.base + '/?cursor=khong-hop-le');
  check(invalid.status === 422 && (await invalid.text()).includes('Về trang đầu'), 'invalid cursor is recoverable SSR 422');
  await page.viewport(320, 480);
  await page.goto(args.base + '/?cursor=khong-hop-le');
  await page.waitFor("document.querySelector('#restart-public')", 'native invalid recovery');
  check(await page.eval("!document.querySelector('.vk-empty-state__art')"), '#258 invalid cursor is an error, without artwork');
  check(await page.eval(`(async () => {
    document.documentElement.style.fontSize = '200%';
    await document.fonts.ready;
    const panel = document.querySelector('[data-testid=public-invalid]');
    const restart = panel.querySelector('#restart-public');
    restart.focus();
    const rect = restart.getBoundingClientRect();
    return panel.querySelectorAll('a').length === 1 && restart.getAttribute('href') === '/'
      && restart.textContent.trim() === 'Về trang đầu' && panel.querySelector('h2')
      && panel.querySelectorAll('[role=alert]').length === 1
      && getComputedStyle(restart).textDecorationLine === 'none'
      && rect.width <= innerWidth && rect.height >= 44;
  })()`), 'bare invalid cursor has one usable recovery at 320px / 200%');
  await page.goto(args.base + '/?q=mua&cursor=khong-hop-le');
  await page.waitFor("document.querySelector('#restart-public')", 'filtered invalid recovery');
  check(await page.eval(`(() => {
    const [first, clear] = document.querySelectorAll('[data-testid=public-invalid] a');
    const a = first.getBoundingClientRect(), b = clear.getBoundingClientRect();
    return first.getAttribute('href') === '/?q=mua' && clear.getAttribute('href') === '/'
      && clear.textContent.trim() === 'Xóa tìm kiếm' && (b.top >= a.bottom || a.right <= b.left);
  })()`), 'filtered cursor has two distinct usable native destinations');
  await page.viewport(1440);
}

export async function discoveryReading(page, { args, check, textContrast }) {
  const ready = async () => page.waitFor("document.documentElement.dataset.hydrated === 'true' && document.querySelector('#public-query')", 'discovery hydration');
  const goto = async path => { await page.goto(args.base + path); await ready(); };
  const submit = async q => {
    // R4.2 (#404): the search icon opens the dock at every width.
    await page.eval(`(() => {
      const open = document.querySelector('#open-public-search');
      if (open && open.getClientRects().length > 0) open.click();
    })()`);
    await page.waitFor("document.querySelector('#public-query').getClientRects().length > 0", 'visible search field');
    await page.eval("document.querySelector('#public-query').focus(); document.querySelector('#public-query').select()");
    await page.send('Input.insertText', { text: q });
    const hasSubmit = await page.eval("getComputedStyle(document.querySelector('#public-search button[type=submit]')).display !== 'none'");
    if (hasSubmit) {
      await page.click('#public-search button[type=submit]');
    } else {
      await page.key('Enter');
    }
    await page.waitFor("document.querySelector('#public-query')?.value === " + JSON.stringify(q) + " && new URL(location.href).searchParams.get('q') === " + JSON.stringify(q), 'applied query URL');
    await page.waitFor("document.querySelector('#public-results')", 'new query result');
  };
  await goto('/');
  check(await page.eval("document.querySelector('#public-query').placeholder === 'Tìm tác phẩm hoặc tác giả' && document.querySelector('#public-query').labels[0]?.textContent === 'Tìm theo tên hoặc tác giả' && !document.querySelector('#public-query-help, #explore-heading p')"), '#308 r1 search keeps distinct placeholder and accessible label without duplicate scope copy');
  await submit('không có tác phẩm này #100');
  await page.waitFor("document.querySelector('[data-testid=public-no-results]')", 'real no-results');
  check(await page.eval("document.querySelectorAll('#public-results [role=status]').length===1 && document.querySelector('.vk-empty-state__title').textContent==='Không tìm thấy tác phẩm' && document.querySelector('.vk-empty-state__message').textContent==='Thử một từ khóa khác.'"), '#621 query miss offers one concise status without redundant 0-count');
  check(await page.eval("!!document.querySelector('#clear-public-search') && !document.querySelector('#public-results [aria-label=\"Xóa tìm kiếm\"]') && document.querySelector('#find-authors-for-query')?.textContent.trim()==='Tìm tác giả' && new URL(document.querySelector('#find-authors-for-query').href).searchParams.get('mode')==='authors' && new URL(document.querySelector('#find-authors-for-query').href).searchParams.get('q')==='không có tác phẩm này #100'"), '#621 X is the clear action and the concise author link carries q');
  await page.waitFor("document.querySelector('.vk-empty-state__art')?.complete", '#258 empty artwork completes');
  check(await page.eval(`(async () => {
    const art = document.querySelector('.vk-empty-state__art'); await art.decode();
    const region = document.querySelector('#community-stream');
    return art.naturalWidth === 512 && art.naturalHeight === 427 && art.alt === '' &&
      art.getAttribute('width') === '512' && art.getAttribute('height') === '427' &&
      art.decoding === 'async' && art.getAttribute('loading') !== 'lazy' &&
      region.querySelectorAll('h2').length === 1 && !region.querySelector('h3') && !region.textContent.includes('Trang này: 0') &&
      region.getAttribute('aria-label') === 'Tác phẩm khám phá' && region.tabIndex === -1 &&
      performance.getEntriesByType('resource').filter(r => r.name.includes('/pkg/empty/')).length === 1;
  })()`), '#258 actual WebP decode, one image request, HTML status and named focus landmark');
  await page.waitFor("document.activeElement?.id==='community-stream'", '#258 submitted search focuses the named landmark');
  await page.key('Tab');
  check(await page.eval(`(() => {
    const link=document.querySelector('#find-authors-for-query'), style=getComputedStyle(link);
    return document.activeElement===link && link.matches(':focus-visible') &&
      style.outlineStyle!=='none' && parseFloat(style.outlineWidth)>0;
  })()`), '#258 keyboard reaches recovery after the landmark with a visible focus ring');
  const emptyMatrix = async surface => {
  for (const theme of ['light','dark']) for (const width of [320,390,834,1440]) for (const scale of [100,150,200]) {
    await page.viewport(width);
    await page.eval(`document.documentElement.dataset.theme=${JSON.stringify(theme)}; document.documentElement.style.fontSize='${scale}%'; document.fonts.ready`);
    await page.eval("Promise.all(document.getAnimations().filter(a=>Number.isFinite(a.effect?.getComputedTiming().endTime)).map(a=>a.finished.catch(()=>{})))");
    const layout = await page.eval(`(() => {
      const art=document.querySelector('.vk-empty-state__art'), message=document.querySelector('.vk-empty-state__message');
      const r=art.getBoundingClientRect(), m=message.getBoundingClientRect();
      const region=document.querySelector('#community-stream').getBoundingClientRect();
      const state=art.closest('.vk-empty-state').getBoundingClientRect();
      // R4.2 (#404): the header row spans the column; the toolbar is only its right cluster.
      const tools=document.querySelector('#explore > .vk-explore__top').getBoundingClientRect();
      return {width:r.width, height:r.height, font:getComputedStyle(message).fontSize,
        overflow:document.documentElement.scrollWidth-innerWidth,
        order:r.bottom<=m.top && m.bottom<=document.querySelector('.vk-empty-state__actions').getBoundingClientRect().top,
        centerX:(r.left + r.right - tools.left - tools.right)/2,
        centerY:(state.top + state.bottom - region.top - region.bottom)/2};
    })()`);
    check(layout.width <= (width<760?192:240) + 1 && Math.abs(layout.height/layout.width-427/512)<0.01 &&
      layout.font===(16 * scale / 100) + 'px' && layout.overflow<=1 && layout.order,
      '#258 empty composition reflow/ratio/type/order '+[theme,width,scale].join('/')+' '+JSON.stringify(layout));
    check(Math.abs(layout.centerX)<=1 && Math.abs(layout.centerY)<=1, '#308 empty cluster centered on both axes '+[surface,theme,width,scale].join('/')+' CSS px '+[layout.centerX,layout.centerY]);
    if (width===390 && scale===100) await textContrast(page, theme, 'empty');
    await page.capture('empty258-'+surface+'-'+[theme,width,scale].join('-'), {boundary:'real-public-api',scenario:'filtered-empty-WebP'});
  }
  };
  await emptyMatrix('explore');
  await emptyRefreshFocus(page, {args, check});
  await page.viewport(1440);
  await page.eval("document.documentElement.style.fontSize='100%'");

  await page.click('#clear-public-search');
  await page.waitFor("location.search === '' && document.activeElement?.id === 'public-query' && document.querySelector('.vk-public-row')",
    '#621 clearing applied query keeps focus in the search field');
  await page.click('#close-public-search');
  await page.waitFor("document.querySelector('#explore').dataset.search === 'closed' && document.activeElement?.id === 'open-public-search'",
    '#404 closing the cleared search returns to browse and restores launcher focus');
  check(await page.eval("!document.querySelector('.vk-empty-state__art') && performance.getEntriesByType('resource').filter(r=>r.name.includes('/pkg/empty/')).every(r=>r.name.endsWith('vink-empty-no-results.webp'))"), '#258 ready branch mounts no artwork and never fetches unrelated illustrations');
  await page.waitFor("document.querySelector('#open-tag-filters')", 'public tag catalog ready');

  await page.send('Input.dispatchMouseEvent',{type:'mouseMoved',x:5,y:5});
  for (const theme of ['light', 'dark']) {
    await page.eval("document.documentElement.dataset.theme="+JSON.stringify(theme));
    await page.eval("Promise.all(document.getAnimations().filter(a=>Number.isFinite(a.effect?.getComputedTiming().endTime)).map(a=>a.finished.catch(()=>{})))");
    await textContrast(page, theme, 'explore');
  }
  for (const theme of ['light', 'dark']) for (const width of [320,374,390,834,1024,1200,1440]) for (const scale of [100,150,200]) {
    await page.viewport(width, width === 320 ? 800 : width === 390 ? 844 : 900);
    await page.eval(`document.documentElement.dataset.theme=${JSON.stringify(theme)};
      if (${scale} !== 100) document.documentElement.dataset.text = '${scale}';
      else delete document.documentElement.dataset.text;
      document.documentElement.style.fontSize = '';
      scrollTo(0,0); document.fonts.ready`);
    await page.eval("Promise.all(document.getAnimations().filter(a=>Number.isFinite(a.effect?.getComputedTiming().endTime)).map(a=>a.finished.catch(()=>{})))");
    check(await page.eval("document.documentElement.scrollWidth <= innerWidth"), 'Explore reflows ' + [theme,width,scale].join('/'));
    if ([390,1440].includes(width) && scale===100) {
      const wide = width === 1440;
      const header = await page.eval(`(() => {
        const explore=document.querySelector('#explore');
        const input=document.querySelector('#public-query');
        const top=document.querySelector('.vk-explore__top').getBoundingClientRect();
        const h1=document.querySelector('#explore-heading h1'), title=h1.getBoundingClientRect(), font=getComputedStyle(h1);
        const launcherEl=document.querySelector('#open-public-search'), launcher=launcherEl.getBoundingClientRect();
        const filter=document.querySelector('#open-tag-filters').getBoundingClientRect();
        const brand=document.querySelector('#discovery-tools .vk-brand').getBoundingClientRect();
        const row=document.querySelector('.vk-public-row').getBoundingClientRect();
        const modes=document.querySelector('.vk-explore-modes').getBoundingClientRect();
        const mid=r=>(r.top+r.bottom)/2;
        return {order: modes.top >= top.bottom - 1 && row.top >= modes.bottom,
          closed: explore.dataset.search==='closed' && input.getClientRects().length===0 && launcherEl.getAttribute('aria-expanded')==='false' && launcherEl.getAttribute('aria-label')==='Tìm tác phẩm',
          title: h1.textContent==='Khám phá' && font.fontWeight==='600' && font.fontFamily.startsWith('"Be Vietnam Pro"') && font.fontSize===(${wide}?'24px':'22px') && title.width>1,
          row: title.right < launcher.left && Math.abs(mid(title)-mid(launcher))<=2 && Math.abs(mid(modes)-mid(filter))<=2 && filter.left>=modes.right-1 && Math.abs(mid(brand)-mid(launcher))<=2,
          gaps: [brand.left-launcher.right],
          size: [launcher.width, launcher.height, filter.width, filter.height],
          radius: getComputedStyle(launcherEl).borderTopLeftRadius,
          label: input.labels[0].textContent==='Tìm theo tên hoặc tác giả'};
      })()`);
      const gap = wide ? 12 : 8, side = wide ? 56 : 48;
      check(header.order && header.closed && header.title && header.row && header.label &&
        header.gaps.every(value => Math.abs(value - gap) <= 1) && header.size.every(value => Math.abs(value - side) < 1) &&
        header.radius === (wide ? '16px' : '14px'),
        '#404 header: page title left, search and logo in header, filter trailing the modes above results ' + [theme,width,JSON.stringify(header)].join('/'));
    }
    if ([320,390].includes(width) && [100,150].includes(scale)) {
      const mobile = await page.eval(`(() => {
        const h1=document.querySelector('#explore-heading h1').getBoundingClientRect();
        const items=['#open-public-search','#discovery-tools .vk-brand'].map(s=>document.querySelector(s).getBoundingClientRect());
        const first=document.querySelector('.vk-public-row__title').getBoundingClientRect();
        const mid=r=>(r.top+r.bottom)/2;
        return {hidden: h1.width<=1,
          visible: items.every(r=>r.width>0 && r.left>=0 && r.right<=innerWidth),
          oneRow: items.every(r=>Math.abs(mid(r)-mid(items[0]))<=2),
          firstTop: first.top + scrollY,
          usable: innerHeight - (document.querySelector('#bottom-navigation')?.getBoundingClientRect().height ?? 0)};
      })()`);
      check(mobile.visible && mobile.oneRow && (scale !== 150 || mobile.firstTop < mobile.usable && (width !== 320 || mobile.hidden)),
        '#427 AC-01 mobile ' + scale + '%: logo and both buttons stay on one row (and at 320px / 150% H1 is visually hidden with first work above bottom bar) ' + [theme,width,JSON.stringify(mobile)].join('/'));
    }
    if ([320,390].includes(width) && scale === 200) {
      const narrow = await page.eval(`(() => {
        const top=document.querySelector('.vk-explore__top').getBoundingClientRect();
        const h1=document.querySelector('#explore-heading h1').getBoundingClientRect();
        const items=['#open-public-search','#discovery-tools .vk-brand'].map(s=>document.querySelector(s).getBoundingClientRect());
        const first=document.querySelector('.vk-public-row__title').getBoundingClientRect();
        return {hidden: h1.width<=1, oneRow: items.every(r=>Math.abs(r.top+r.height/2-(items[0].top+items[0].height/2))<=2),
          firstTop: first.top + scrollY, usable: innerHeight - (document.querySelector('#bottom-navigation')?.getBoundingClientRect().height ?? 0), top: top.height};
      })()`);
      check(narrow.firstTop < narrow.usable && (width !== 320 || narrow.hidden && narrow.oneRow),
        '#404/#427 stress 200% (dom-override-geometry-only): first work starts above the bottom bar, and at 320px the title is visually hidden with the icons and logo on one row ' + [theme,width,JSON.stringify(narrow)].join('/'));
    }
    if ([320,390,834,1440].includes(width)) {
      check(await page.eval(`(() => {
        const launcher = document.querySelector('#open-public-search');
        const style = getComputedStyle(launcher);
        const box = launcher.getBoundingClientRect();
        return style.backgroundColor === 'rgba(0, 0, 0, 0)' && style.borderTopColor === 'rgba(0, 0, 0, 0)' &&
          style.boxShadow === 'none' && box.width >= 48 && box.height >= 48 &&
          launcher.getAttribute('aria-expanded') === 'false';
      })()`), '#522 flat Explore search launcher ' + [theme,width,scale].join('/'));
      check(await page.eval(`(() => {
        const button=document.querySelector('#open-tag-filters'), r=button.getBoundingClientRect();
        const icon=button.querySelector('svg[data-icon=filter-list][aria-hidden=true]');
        return button.type==='button' && button.getAttribute('aria-label')==='Lọc thẻ' && button.textContent==='' && !!icon &&
          button.querySelectorAll('svg').length===1 && button.classList.contains('vk-ui-icon-button') &&
          getComputedStyle(button).backgroundColor==='rgba(0, 0, 0, 0)' && getComputedStyle(button).boxShadow==='none' && button.getAttribute('aria-expanded')==='false' &&
          !document.querySelector('.vk-tag-launcher__badge') &&
          icon.getBoundingClientRect().left>=r.left && icon.getBoundingClientRect().right<=r.right &&
          !button.hasAttribute('aria-pressed') && !button.querySelector('button,a,input') &&
          r.width>=48 && r.height>=48 && r.left>=0 && r.right<=innerWidth;
      })()`), '#225/#404 Lọc thẻ is one named borderless icon target >=48px '+[theme,width,scale].join('/'));
    }
    check(await page.eval("(() => { const heading=document.querySelector('#explore-heading h1'); if(heading.getBoundingClientRect().width<=1) return true; const walker=document.createTreeWalker(heading,NodeFilter.SHOW_TEXT); for(let node=walker.nextNode();node;node=walker.nextNode()) for(const word of node.textContent.matchAll(/\\S+/g)) { const range=document.createRange(); range.setStart(node,word.index); range.setEnd(node,word.index+word[0].length); const rects=[...range.getClientRects()]; if(rects.some(r=>Math.abs(r.top-rects[0].top)>1)) return false; } return true; })()"), 'Vietnamese heading words remain intact '+[theme,width,scale].join('/'));

    const modes = await page.eval(`(() => {
      const nav = document.querySelector('.vk-explore-modes');
      const links = [...nav.querySelectorAll('a')];
      const current = [...nav.querySelectorAll('[aria-current="page"]')];
      const boxes = links.map(link => link.getBoundingClientRect());
      const owner = document.querySelector('#explore > header').getBoundingClientRect();
      const row = document.querySelector('.vk-public-row').getBoundingClientRect();
      const indicator = getComputedStyle(current[0], '::after');
      const near = (left, right) => Math.abs(left - right) <= 1;
      const intrinsic = links.every((link, index) => {
        const range = document.createRange();
        range.selectNodeContents(link);
        const text = range.getBoundingClientRect();
        const style = getComputedStyle(link);
        const expected = text.width + parseFloat(style.paddingInlineStart) + parseFloat(style.paddingInlineEnd);
        return near(boxes[index].width, Math.max(parseFloat(style.minInlineSize), expected))
          && text.left >= boxes[index].left && text.right <= boxes[index].right;
      });
      // Not expanded and not distributed: links on one row sit exactly one column-gap apart.
      // Free space after the last link depends on label widths (0.23px at 320/100), so it only
      // has to stay inside the header.
      const gap = parseFloat(getComputedStyle(nav).columnGap) || 0;
      const packed = boxes.every((box, index) => index === 0
        || Math.abs(box.top - boxes[index - 1].top) > 1
        || near(box.left, boxes[index - 1].right + gap));
      return {
        current: current.map(link => link.id),
        count: links.length,
        intrinsic,
        packed,
        aligned: near(boxes[0].left, owner.left) && near(boxes[0].left, row.left),
        endSpace: owner.right - Math.max(...boxes.map(box => box.right)),
        height: Math.min(...boxes.map(box => box.height)),
        indicator: indicator.height === '3px' && parseFloat(indicator.insetInlineStart) > 0
          && links.slice(1).every(link => getComputedStyle(link, '::after').height !== '3px'),
        icons: nav.querySelectorAll('svg').length,
      };
    })()`);
    check(modes.current.join() === 'explore-mode-works' && modes.count === 3 && modes.intrinsic && modes.packed
      && modes.aligned && modes.endSpace >= -1 && modes.height >= 48 && modes.indicator && modes.icons === 0,
      '#566 left-aligned intrinsic tabs with a local 3px underline ' + [theme,width,scale,JSON.stringify(modes)].join('/'));
    if (theme === 'light' && ([320,390,1440].includes(width) && [100,150].includes(scale) ||
        [320,390].includes(width) && scale === 200)) {
      await checkExploreModesTabAndAx(page, {check, label: `${width}px/${scale}%`});
    }

    const dock = await page.eval("document.querySelector('#tag-dock').getBoundingClientRect().width>0");
    check(!dock, '#308 hydrated discovery has no filter column ' + [theme,width,scale].join('/'));

    const cardLayout = await page.eval(`(() => {
      const list = document.querySelector('.vk-discovery .vk-public-list');
      const row = document.querySelector('.vk-public-row');
      const rowStyle = getComputedStyle(row);
      const probe = document.createElement('span');
      probe.style.background = 'var(--vk-bg-content)';
      probe.style.color = 'var(--vk-tertiary)';
      document.body.append(probe);
      const expectedBg = getComputedStyle(probe).backgroundColor;
      const expectedTertiary = getComputedStyle(probe).color;
      probe.remove();
      const title = row.querySelector('h2.vk-public-row__title');
      const eyebrow = row.querySelector('.vk-public-row__eyebrow');
      const time = row.querySelector('.vk-public-row__eyebrow time.vk-feed-date');
      // #352/#405 (DQ-09): circular AuthorByline cluster; separate bg-content card bo 20, gap 8.
      const avatar = row.querySelector('.vk-public-row__byline .vk-ui-avatar');
      const circle = avatar.getBoundingClientRect();
      const initials = document.createRange();
      initials.selectNodeContents(avatar.querySelector('.vk-ui-avatar__initials'));
      const ink = initials.getBoundingClientRect();
      const excerpt = row.querySelector('.vk-public-row__excerpt').getBoundingClientRect();
      const expectedDate = time ? new Intl.DateTimeFormat('vi-VN', { timeZone: 'Asia/Ho_Chi_Minh' }).format(new Date(time.dateTime)) : null;
      return {
        named: document.getElementById(row.querySelector('a').getAttribute('aria-labelledby')) === title,
        card: rowStyle.borderTopLeftRadius === '20px' && rowStyle.borderBottomRightRadius === '20px'
          && rowStyle.backgroundColor === expectedBg && rowStyle.borderBottomWidth === '0px'
          && getComputedStyle(list).rowGap === '8px' && row.querySelectorAll('a a').length === 0,
        date: !!time && !time.closest('a') && time.textContent === expectedDate
          && getComputedStyle(eyebrow).color === expectedTertiary,
        // R3/R4 (#336/#405): form tag · date above the title, then excerpt, then the byline; no chevron.
        ordered: (eyebrow?.getBoundingClientRect().bottom ?? -Infinity) <= title.getBoundingClientRect().top + 1
          && title.getBoundingClientRect().bottom <= excerpt.top + 1 && excerpt.height > 0
          && excerpt.bottom <= row.querySelector('.vk-public-row__byline .vk-ui-byline__name').getBoundingClientRect().top + 1
          && !row.querySelector('.vk-public-row__chevron'),
        diameter: circle.width, square: circle.width === circle.height,
        fits: initials.getClientRects().length === 1 && ink.left >= circle.left - 1 && ink.right <= circle.right + 1
          && ink.top >= circle.top - 1 && ink.bottom <= circle.bottom + 1,
      };
    })()`);
    check(cardLayout.named && cardLayout.card && cardLayout.date && cardLayout.ordered && cardLayout.square && cardLayout.fits && Math.abs(cardLayout.diameter - 36 * scale / 100) < 1,
      '#308/#336/#405 separate bg-content card bo 20 gap 8, tertiary eyebrow with HCM date, title and initials fit ' + [theme,width,scale,JSON.stringify(cardLayout)].join('/'));
    if (width === 320 && scale === 200) {
      const card = await page.eval(`(() => {
        const row = document.querySelector('.vk-public-row');
        const box = selector => row.querySelector(selector).getBoundingClientRect();
        const author = box('.vk-public-row__byline .vk-ui-byline__name'), title = box('.vk-public-row__title'), excerpt = box('.vk-public-row__excerpt');
        return {
          ordered: title.bottom <= excerpt.top + 1 && excerpt.bottom <= author.top + 1,
          overflow: document.documentElement.scrollWidth - document.documentElement.clientWidth,
        };
      })()`);
      check(
        card.ordered && card.overflow <= 1,
        `320px / 200% ${theme}: enlarged R3 row stacks without overflow (${JSON.stringify(card)})`,
      );
    }
    if ([320,390,834,1440].includes(width)) await page.capture('discovery-' + [theme,width,scale].join('-'), {
      boundary: 'real-public-api',
      scenario: 'TAG-PR-06',
      textScaleLane: scale === 200 ? 'dom-override-geometry-only' : 'app-text-scale',
    });
  }
  await page.eval("delete document.documentElement.dataset.text");
  await desktopSearchDock(page, {args, check, ready});
  await page.viewport(390);
  await page.eval("document.documentElement.style.fontSize='100%'");

  // Khám phá Web r1 mobile full-screen search, IME/Escape, Back LIFO layering & scroll restoration.
  await page.click('#open-public-search');
  await page.waitFor("document.querySelector('#explore').dataset.search==='open' && document.activeElement?.id==='public-query'", '#308 r1 mobile search opens and focuses query');
  check(await page.eval(`(() => {
    const nav=document.querySelector('#bottom-navigation'), close=document.querySelector('#close-public-search'),
      sub=document.querySelector('#public-search button[type=submit]'), st=document.querySelector('#public-results > .vk-public-scope');
    return getComputedStyle(nav).display==='none' && close.getClientRects().length>0 &&
      getComputedStyle(sub).display==='none' && st.getClientRects().length>0 && st.textContent.includes('Trang này:');
  })()`), '#308 r1 mobile open search hides bottom nav & submit icon, shows back & result count');
  await page.capture('discovery-mobile-search-open', { boundary: 'real-public-api', scenario: 'r1-mobile-search' });
  await page.eval("document.querySelector('#public-query').dispatchEvent(new CompositionEvent('compositionstart',{bubbles:true}))");
  await page.key('Escape');
  await page.eval("new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)))");
  check(await page.eval("document.querySelector('#explore').dataset.search==='open'"), '#308 r1 Escape during IME composition keeps mobile search open');
  await page.eval("document.querySelector('#public-query').dispatchEvent(new CompositionEvent('compositionend',{bubbles:true}))");
  await page.key('Escape');
  await page.waitFor("document.querySelector('#explore').dataset.search==='closed' && document.activeElement?.id==='open-public-search' && getComputedStyle(document.querySelector('#bottom-navigation')).display!=='none'", '#308 r1 Escape closes mobile search, restores bottom nav and launcher focus');
  await page.click('#open-public-search');
  await page.waitFor("document.querySelector('#explore').dataset.search==='open'", 're-open mobile search');
  await page.click('#open-tag-filters');
  await page.waitFor("document.querySelector('#tag-picker')?.open", 'tag picker above mobile search');
  await page.eval('history.back()');
  await page.waitFor("!document.querySelector('#tag-picker').open && document.querySelector('#explore').dataset.search==='open'", '#308 r1 Back closes #tag-picker before mobile search');
  await page.eval('history.back()');
  await page.waitFor("document.querySelector('#explore').dataset.search==='closed' && document.activeElement?.id==='open-public-search'", '#308 r1 second Back closes mobile search and restores trigger focus');

  await submit('mua');
  await page.eval('history.back()');
  await page.waitFor("document.querySelector('#explore')?.dataset.search==='closed' && document.activeElement?.id==='open-public-search' && !location.search", '#308 Back from a submitted mobile search returns to browse');
  await submit('mua');
  await page.click('#clear-public-search');
  await page.waitFor("document.querySelector('#explore')?.dataset.search==='open' && document.activeElement?.id==='public-query' && !location.search && !document.querySelector('#public-query').value", '#308 clearing a submitted query keeps mobile search open for typing');
  await submit('mua');
  await page.waitFor("document.querySelector('.vk-public-row__title mark.vk-search-mark')", '#308 r1 highlights matched query terms in title/author');
  check(await page.eval(`(() => {
    const excerptMarks = document.querySelectorAll('.vk-public-row__excerpt mark');
    const st = document.querySelector('#public-results > .vk-public-scope');
    return excerptMarks.length === 0 && st.getClientRects().length > 0 && st.textContent.includes('Trang này:');
  })()`), '#308 r1 never highlights inside excerpt and shows result count on mobile search');
  await page.waitFor("document.querySelector('#open-tag-filters')", 'catalog for the applied query');
  await page.click('#open-tag-filters');
  const choices = await page.eval(`[...document.querySelectorAll('#modal-tag-form fieldset')].map(group => {
    const link = group.querySelector('a');
    return {id: link.id, href: link.href, label: link.textContent.trim()};
  })`);
  check(choices.length === 2, '#603 live catalog offers form and theme destinations');
  const before = await page.eval('location.href');
  await page.key('Escape');
  await page.waitFor("!document.querySelector('#tag-picker').open && document.activeElement.id==='open-tag-filters'", 'Escape returns to trigger');
  await page.click('#open-tag-filters');
  await page.eval('history.back()');
  await page.waitFor("!document.querySelector('#tag-picker').open && document.activeElement.id==='open-tag-filters'", 'Back closes picker before navigating');
  check(await page.eval('location.href') === before, 'cancelling retains the query');
  await page.click('#open-tag-filters');
  const outside = await page.eval(`(() => {
    const rect = document.querySelector('#tag-picker').getBoundingClientRect();
    return { x: innerWidth / 2, y: Math.max(0, rect.top / 2) };
  })()`);
  await page.send('Input.dispatchMouseEvent', { type: 'mousePressed', ...outside, button: 'left', clickCount: 1 });
  await page.send('Input.dispatchMouseEvent', { type: 'mouseReleased', ...outside, button: 'left', clickCount: 1 });
  await page.waitFor("!document.querySelector('#tag-picker').open && document.activeElement.id==='open-tag-filters'", 'outside click closes the picker');
  check(await page.eval('location.href') === before, 'outside dismissal preserves the applied query');
  for (const [index, choice] of choices.entries()) {
    const theme = index === 0 ? 'light' : 'dark';
    await page.eval(`document.documentElement.dataset.theme = ${JSON.stringify(theme)}`);
    await page.click('#open-tag-filters');
    await textContrast(page, theme, 'filter');
    await page.eval(`document.getElementById(${JSON.stringify(choice.id)}).focus()`);
    await page.key('Enter');
    await page.waitFor(`location.href === ${JSON.stringify(choice.href)} && document.querySelectorAll('#applied-filters .vk-applied-tag').length === 1 &&
      document.querySelector('#applied-filters .vk-applied-tag').getAttribute('aria-label') === ${JSON.stringify('Bỏ lọc '+choice.label)} &&
      !document.querySelector('#tag-picker')?.open && document.activeElement.id === 'community-stream'`, 'Enter applies one tag, closes picker and focuses results');
    const url = new URL(choice.href);
    check(url.searchParams.get('q') === 'mua' && url.searchParams.getAll('tag').length === 1 && !url.searchParams.has('cursor'), 'replacement retains query and resets cursor');
  }
  await page.eval('history.back()');
  await page.waitFor(`location.href === ${JSON.stringify(choices[0].href)} && document.querySelector('#applied-filters a')?.getAttribute('aria-label') === ${JSON.stringify('Bỏ lọc '+choices[0].label)}`, 'Back restores first tag');
  await page.eval('history.forward()');
  await page.waitFor(`location.href === ${JSON.stringify(choices[1].href)} && document.querySelector('#applied-filters a')?.getAttribute('aria-label') === ${JSON.stringify('Bỏ lọc '+choices[1].label)}`, 'Forward restores replacement tag');
  check(await page.eval("document.querySelectorAll('#applied-filters .vk-applied-tag').length === 1"), 'applied filter is visible and removable');
  await page.waitFor("document.querySelector('#open-tag-filters')", 'applied catalog');
  await page.click('#open-tag-filters');
  await page.waitFor("document.querySelector('#tag-picker').open", '#258 reopen unchanged picker for cancellation');
  await page.click('#close-tag-filters');
  await page.waitFor("!document.querySelector('#tag-picker').open && document.activeElement.id==='open-tag-filters'", '#308 Close restores the trigger');
  await page.click('#applied-filters .vk-applied-tag');
  await page.waitFor("!new URL(location.href).searchParams.has('tag') && new URL(location.href).searchParams.get('q')==='mua' && document.querySelector('.vk-public-row')", 'remove filter and keep the query');

  // IME composition is simulated through CDP (`Input.imeSetComposition`); it is not a named OS IME
  // session. Keys are trusted (`Input.dispatchKeyEvent`): unlike a dispatched DOM event they do
  // trigger the form's implicit submission, so a missing guard would navigate here.
  const old = await page.eval('location.href');
  await page.eval("(() => { const i=document.querySelector('#public-query'); i.focus(); i.select(); })()");
  await page.send('Input.imeSetComposition', { text: 'mư', selectionStart: 2, selectionEnd: 2 });
  await page.key('Enter');
  await new Promise((resolve) => setTimeout(resolve, 400));
  check(await page.eval('location.href') === old, 'a trusted Enter while composing does not commit a partial search');
  await page.send('Input.insertText', { text: 'mưa' });
  await page.key('Enter');
  await page.waitFor("new URL(location.href).searchParams.get('q') === 'mưa'", 'the committed text applies with Enter');
  check(true, 'after the commit the same Enter applies the search (the guard only holds composition)');

  await goto('/?q=mua');
  await page.waitFor("document.querySelector('.vk-public-row__link')", 'real title search');
  const returnPath = await page.eval('location.pathname + location.search');
  await page.click('.vk-public-row__link');
  await page.waitFor("document.querySelector('#reader-return') && document.querySelector('#reader-paper')", 'reader from search');
  check(await page.eval("document.querySelector('#reader-return').getAttribute('href').startsWith(" + JSON.stringify(returnPath) + ") && document.querySelector('#reader-return').textContent.includes('Về kết quả tìm kiếm')"), 'reader return preserves applied query and shows search return label');
  check(await page.eval("Math.abs(document.querySelector('#reader-header .vk-ui-avatar').getBoundingClientRect().width-44)<1"), 'reader avatar is exactly 44px');
  await page.capture('discovery-reader', { boundary: 'real-public-api', scenario: 'reader-return' });
  for (const width of [320, 390]) {
    await page.viewport(width, 480);
    const reader = await page.eval(`(async () => {
      document.documentElement.style.fontSize = '200%';
      await document.fonts.ready;
      const header = document.querySelector('#reader-header'), body = document.querySelector('.vk-document');
      return { gap: body.getBoundingClientRect().top - header.getBoundingClientRect().bottom,
        font: getComputedStyle(body).fontSize, context: header.textContent.includes('Bản công khai') };
    })()`);
    check(reader.gap > 0 && reader.gap <= 25 && reader.font === '34px' && reader.context,
      `${width}px / 200%: untagged Reader reaches the full-size text within the header spacing budget (${JSON.stringify(reader)})`);
  }
  await page.viewport(390);
  await page.eval("document.documentElement.style.fontSize='100%'");
  await page.click('#reader-return');
  await page.waitFor("document.activeElement?.matches('.vk-public-row__link')", 'reader return focuses the opened result');
  await page.click('#close-public-search');
  await page.waitFor("location.search==='' && document.querySelector('#explore').dataset.search==='closed' && document.activeElement?.id==='open-public-search' && getComputedStyle(document.querySelector('#bottom-navigation')).display!=='none'", '#308 r1 close-public-search returns to browse with focus on launcher');

  await tagRecovery(page, { args, check }, 'active');

  const catalog = await (await fetch(args.base + '/api/v1/tags')).json();
  const tag = catalog.tags.find(t => t.label === 'Ký ức');
  const route = '/the/' + encodeURIComponent(tag.slug);
  const response = await fetch(args.base + route);
  const html = await response.text();
  check(response.status === 200 && html.includes(tag.label) && html.includes(tag.description), 'tag detail SSR uses canonical catalog name and description');
  await goto(route);
  check(await page.eval("document.querySelector('#navigation-primary a[href=\"/\"]')?.getAttribute('aria-current')==='page'"), 'tag detail belongs to Explore');
  await goto(route+'?q=no-match-empty-258');
  await page.waitFor("document.querySelector('.vk-empty-state__art')?.complete && document.querySelector('.vk-empty-state__art')?.naturalWidth===512", '#258 tag empty artwork');
  check(await page.eval("document.querySelectorAll('#public-results [role=status]').length===1 && document.querySelectorAll('#community-stream h2').length===1 && document.querySelector('.vk-empty-state__message').textContent==='Thử một từ khóa khác.'"), '#258/#405 tag search miss keeps one result heading and status');
  await emptyMatrix('tag');
  await page.viewport(1440);
  await page.eval("document.documentElement.style.fontSize='100%'");

  const alias = await fetch(args.base + '/the/' + encodeURIComponent('hồi-ức'), { redirect:'manual' });
  check(alias.status === 308 && alias.headers.get('location')?.startsWith('/the/'), 'alias tag route redirects using server canonical identity');
  for (const spelling of [tag.slug.toUpperCase(), tag.slug.normalize('NFD')]) {
    const normalized = await fetch(args.base+'/the/'+encodeURIComponent(spelling), {redirect:'manual'});
    check(normalized.status===308 && normalized.headers.get('location')==='/the/'+encodeURIComponent(tag.slug), 'uppercase/NFD uses the provider canonical tag URL');
  }
  const unknown = await fetch(args.base + '/the/khong-ton-tai-100');
  const unknownHtml = await unknown.text();
  check(unknown.status === 404 && unknownHtml.includes('Không tìm thấy thẻ này') && !unknownHtml.includes('/pkg/empty/'), 'unknown tag has its own SSR 404 without absence artwork');
  await invalidCursorRecovery(page, args, check);

  // Native browser GET, with page JS disabled and WASM preloads blocked.
  // A decoded image from an earlier matrix must not bypass either failure fixture.
  await page.send('Network.setCacheDisabled', {cacheDisabled:true});
  await page.send('Network.clearBrowserCache');
  await page.send('Network.setBlockedURLs', { urls:[args.base+'/pkg/vink.js*',args.base+'/pkg/vink_bg.wasm*',args.base+'/pkg/empty/*'] });
  await page.send('Emulation.setScriptExecutionDisabled', { value:true });
  try {
    await page.goto(args.base + '/?q=mua');
    check(await page.eval("[...document.querySelectorAll('#dock-tag-form a')].length>0 && [...document.querySelectorAll('#dock-tag-form a')].every(a=>new URL(a.href).searchParams.getAll('tag').length===1 && new URL(a.href).searchParams.get('q')==='mua')"), '#603 SSR exposes one-tag destinations retaining q');
    check(await page.eval("document.querySelector('.vk-public-row__link').href.includes('?q=mua')"), 'no-JS SSR supplies contextual reader links');
    await page.eval("document.querySelector('#public-query').value='khong-co-ket-qua-100'");
    const point = await page.eval("(() => { const b=document.querySelector('#public-search button[type=submit]'); b.scrollIntoView(); const r=b.getBoundingClientRect(); return {x:r.x+r.width/2,y:r.y+r.height/2}; })()");
    for (const type of ['mousePressed','mouseReleased']) await page.send('Input.dispatchMouseEvent', {type,...point,button:'left',clickCount:1});
    await page.waitFor("new URL(location.href).searchParams.get('q')==='khong-co-ket-qua-100' && document.querySelector('[data-testid=public-no-results]')", 'native GET result');
    await page.waitFor("document.querySelector('.vk-empty-state__art')?.complete", '#258 blocked artwork finishes with failure');
    check(await page.eval("document.querySelector('.vk-empty-state__art').naturalWidth===0 && document.documentElement.dataset.hydrated!=='true' && document.querySelector('.vk-empty-state__message').textContent==='Thử một từ khóa khác.' && document.querySelector('#clear-public-search').getAttribute('href')==='/' && document.querySelectorAll('#public-results [role=status]').length===1"), '#258/#405 blocked artwork before hydration leaves useful HTML and GET recovery');
    // page.click waits for animation frames; those callbacks do not run with page JS disabled.
    // Use the same native pointer path as the no-JS search above, without a JS click fallback.
    for (const selector of ['#tag-dock summary', '#dock-tag-form a']) {
      const target = await page.eval(`(() => { const el=document.querySelector(${JSON.stringify(selector)}); el.scrollIntoView(); const r=el.getBoundingClientRect(); const x=r.x+r.width/2,y=r.y+r.height/2; return {x,y,hit:el.contains(document.elementFromPoint(x,y))}; })()`);
      check(target.hit, '#225 no-JS pointer hits the native filter target '+selector);
      for (const type of ['mousePressed','mouseReleased']) await page.send('Input.dispatchMouseEvent',{type,x:target.x,y:target.y,button:'left',clickCount:1});
    }
    await page.waitFor("new URL(location.href).searchParams.get('q')==='khong-co-ket-qua-100' && new URL(location.href).searchParams.getAll('tag').length===1 && !new URL(location.href).searchParams.has('cursor')", '#225 no-JS catalog link preserves q/tag through native GET and resets cursor');
    // A corrupt response before hydration must not own any state meaning or recovery.
    let corruptRequests=0;
    const corruptArt = ({sessionId,method,params}) => {
      if (sessionId===page.sessionId && method==='Fetch.requestPaused') {
        corruptRequests++;
        page.send('Fetch.fulfillRequest', {requestId:params.requestId,responseCode:200,
          responseHeaders:[{name:'Content-Type',value:'image/webp'},{name:'Cache-Control',value:'no-store'}],
          body:Buffer.from('invalid WebP fixture #258').toString('base64')}).catch(()=>{});
      }
    };
    page.cdp.listeners.push(corruptArt);
    await page.send('Network.setBlockedURLs', {urls:[args.base+'/pkg/vink.js*',args.base+'/pkg/vink_bg.wasm*']});
    await page.send('Fetch.enable', {patterns:[{urlPattern:args.base+'/pkg/empty/*',requestStage:'Request'}]});
    try {
      await page.goto(args.base+'/?q=corrupt-art-258');
      await page.waitFor("document.querySelector('.vk-empty-state__art')?.complete", '#258 corrupt image completes with failure');
      check(corruptRequests===1, '#258 exactly one corrupt artwork response was actually injected');
      check(await page.eval(`(() => {
        const art=document.querySelector('.vk-empty-state__art'), box=art.getBoundingClientRect();
        return art.naturalWidth===0 && box.width>0 && Math.abs(box.height/box.width-427/512)<0.01 &&
          document.querySelector('.vk-empty-state__message').textContent==='Thử một từ khóa khác.' &&
          document.querySelector('#clear-public-search').getAttribute('href')==='/' &&
          document.querySelectorAll('#public-results [role=status]').length===1;
      })()`), '#258 corrupt image before hydration keeps reserved space, one HTML status and real recovery');
    } finally {
      await page.send('Fetch.disable');
      page.cdp.listeners=page.cdp.listeners.filter(listener=>listener!==corruptArt);
    }

  } finally {
    await page.send('Emulation.setScriptExecutionDisabled',{value:false});
    await page.send('Network.setBlockedURLs',{urls:[]});
    await page.send('Network.setCacheDisabled',{cacheDisabled:false});
  }
  await goto('/');
  await catalogRecovery(page, {args, check, goto});
  await goto('/?q=missing-return-258#work-00000000-0000-4000-8000-000000000258');
  await page.waitFor("document.activeElement?.id==='community-stream'", '#258 missing reader-return row focuses the named results landmark');
  check(await page.eval("document.activeElement.getAttribute('aria-label')==='Tác phẩm khám phá' && document.activeElement.getClientRects().length>0"), '#258 reader-return fallback is a visible, named target');
  for (const width of [390, 1440]) {
    await page.viewport(width);
    await goto('/');
    await submittedReadKeepsTyping(page, {args, check});
  }
  await goto('/');
}

// Both loopback servers read the same seeded catalog. The guest server has no dev principal.
export async function tagRecovery(page, { args, check }, standing) {
  const poetry = '/the/' + encodeURIComponent('thơ');
  const goto = async path => {
    await page.goto(args.base + path);
    await page.waitFor("document.documentElement.dataset.hydrated === 'true' && document.querySelector('#public-query')", 'tag discovery hydration');
  };
  await goto('/');
  await page.waitFor("document.querySelector('#open-tag-filters')", 'works mode exposes its catalog');
  const otherModes = await page.eval(`[...document.querySelectorAll('.vk-explore-modes a')]
    .filter(link => link.id !== 'explore-mode-works').map(link => link.id)`);
  for (const mode of otherModes) {
    await page.click('#' + mode);
    await page.waitFor(`document.getElementById(${JSON.stringify(mode)})?.getAttribute('aria-current') === 'page'`,
      'selected mode on its own URL');
    check(await page.eval("!document.querySelector('#open-tag-filters, #tag-dock, #tag-picker')"),
      'tag filter belongs only to All works: ' + standing + ' / ' + mode);
    await page.click('#explore-mode-works');
    await page.waitFor("document.querySelector('#open-tag-filters')", 'returning to works restores tag launcher');
  }
  await goto(poetry);
  const session = await page.eval("fetch('/api/v1/auth/session').then(response => response.json()).then(session => session.state)");
  check(session === standing, 'tag capture uses the requested local session: ' + standing);
  await page.waitFor("document.querySelector('.vk-empty-state__art')?.naturalWidth === 512", 'c049 true empty tag retains artwork');
  for (const [width,height,scale,theme] of [[390,844,100,'light'],[320,540,150,'light'],[320,540,200,'dark'],[1440,900,100,'dark']]) {
    await page.viewport(width,height);
    await page.eval(`document.documentElement.dataset.theme=${JSON.stringify(theme)}; document.documentElement.style.fontSize='${scale}%'; document.fonts.ready`);
    check(await page.eval(`(() => {
      const a=document.querySelector('.vk-ui-back-link'), h=document.querySelector('#explore-heading h1'),
        input=document.querySelector('#public-query'), search=input.closest('form'), main=document.querySelector('#community-stream');
      return a.getAttribute('href')==='/' && a.textContent.trim()==='Về Khám phá' && a.firstElementChild.matches('[data-icon=arrow-back][aria-hidden=true]') &&
        h.textContent==='Thơ' && h.classList.contains('vk-ui-type-title') &&
        getComputedStyle(h).fontSize === (22 * ${scale} / 100) + 'px' && a.getBoundingClientRect().bottom<=h.getBoundingClientRect().top+1 &&
        search.getBoundingClientRect().top>=h.getBoundingClientRect().bottom-1 && input.labels[0].textContent==='Tìm trong thẻ' &&
        !document.querySelector('#open-tag-filters,.vk-explore-modes') && !main.querySelector('a[href="/"]') &&
        document.documentElement.scrollWidth<=innerWidth;
    })()`), 'c049 parent, separate heading, native search and sole recovery '+[width,scale,theme]);
    await page.capture('603-c049-'+[standing,width,scale,theme].join('-'), {scenario:'tag-empty',boundary:'Product SSR + hydrated Chrome · '+standing,requestedScene:{width,height,scale,theme}});
  }
  await goto(poetry+'?q=muc-603');
  check(await page.eval(`new URL(document.querySelector('#clear-public-tags').href).searchParams.get('q')==='muc-603' &&
    document.querySelector('#clear-public-search').getAttribute('href')===${JSON.stringify(poetry)}`), 'tag page clear-filter retains q and clear-query retains tag');
}

async function catalogRecovery(page, {args, check, goto}) {
  const catalog = await (await fetch(args.base+'/api/v1/tags')).json();
  const tag = catalog.tags.find(tag=>tag.facet==='form');
  await goto('/?q=mua&tag='+encodeURIComponent(tag.slug));
  const requests = [];
  const gate = ({sessionId,method,params}) => {
    if (sessionId===page.sessionId && method==='Fetch.requestPaused') requests.push(params.requestId);
  };
  const answer = async (status, body) => {
    const deadline = Date.now()+10000;
    while (!requests.length && Date.now()<deadline) await new Promise(resolve=>setTimeout(resolve,25));
    check(requests.length===1, '#308 one catalog request awaits an answer');
    await page.send('Fetch.fulfillRequest', {requestId:requests.shift(), responseCode:status,
      responseHeaders:[{name:'Content-Type',value:'application/json'}],
      body:Buffer.from(JSON.stringify(body)).toString('base64')});
  };
  await page.viewport(390);
  page.cdp.listeners.push(gate);
  await page.send('Fetch.enable', {patterns:[{urlPattern:args.base+'/api/v1/tags*',requestStage:'Request'}]});
  try {
    await page.eval("document.querySelector('#public-query').value='mot-tu-khoa-moi'; document.querySelector('#public-search').requestSubmit()");
    await page.waitFor("document.querySelector('[data-testid=tag-catalog-loading]') && !document.querySelector('#open-tag-filters')", '#308 pending catalog announces loading');
    await answer(503, {error:{code:'dependency_unavailable'}});
    await page.waitFor("document.querySelector('#open-tag-filters')", '#308 failed catalog retains entry');
    await page.click('#open-tag-filters');
    await page.waitFor("document.querySelector('#tag-picker [role=alert]') && document.querySelector('#retry-tag-catalog')", '#308 catalog error offers retry');
    check(requests.length===0 && await page.eval("!!document.querySelector('#applied-filters a') && !document.querySelector('#modal-tag-form')"), '#308 opening does not refetch or replace applied tags');
    await page.click('#retry-tag-catalog');
    await page.waitFor("document.querySelector('#tag-picker [role=status]')?.textContent.includes('Đang tải')", '#308 loading on explicit retry');
    await answer(200, {tags:[],truncated:false});
    await page.waitFor("document.querySelector('#tag-picker [role=status]')?.textContent.includes('Chưa có thẻ')", '#308 empty successful result');
    await page.click('#close-tag-filters');
    await page.waitFor("document.activeElement?.id==='public-query'", '#308 close focuses search when the launcher has been removed');
    check(await page.eval("!document.querySelector('#open-tag-filters, #tag-dock, #modal-tag-form') && !!document.querySelector('#applied-filters a')"), '#308 empty catalog removes filter surfaces and keeps the active tag removal link');
    await page.click('#applied-filters a');
    await answer(200, {tags:[tag],truncated:false});
    await page.waitFor("document.querySelector('#open-tag-filters')", '#308 one-facet catalog');
    await page.click('#open-tag-filters');
    check(await page.eval(`(() => {
      const url = new URL(location.href), groups = document.querySelectorAll('#modal-tag-form legend');
      return url.searchParams.get('q')==='mot-tu-khoa-moi' && !url.searchParams.has('tag') &&
        groups.length===1 && groups[0].textContent==='Dạng bài';
    })()`), '#308 removing an applied tag keeps q, and only available groups render');
    await page.key('Escape');
  } finally {
    await page.send('Fetch.disable');
    page.cdp.listeners = page.cdp.listeners.filter(listener=>listener!==gate);
  }
}

async function submittedReadKeepsTyping(page, {args, check}) {
  const query='pending-submit-258', draft='chữ đang gõ trong lúc tìm kiếm';
  const response=await (await fetch(args.base+'/api/v1/works?q='+query)).json();
  // R4.2 (#404): the search dock opens from its icon at every width; Enter submits.
  await page.click('#open-public-search');
  await page.waitFor("document.activeElement?.id==='public-query'", 'search dock input ready for typing');
  const commit = async () => {
    await page.eval("document.querySelector('#public-query').focus()");
    await page.key('Enter');
  };
  const held=[];
  const gate=({sessionId,method,params})=>{
    if(sessionId!==page.sessionId || method!=='Fetch.requestPaused') return;
    if(new URL(params.request.url).searchParams.get('q')===query) held.push(params.requestId);
    else page.send('Fetch.continueRequest',{requestId:params.requestId}).catch(()=>{});
  };
  page.cdp.listeners.push(gate);
  await page.send('Fetch.enable',{patterns:[{urlPattern:args.base+'/api/v1/works*',requestStage:'Request'}]});
  try {
    await page.eval("document.querySelector('#public-query').focus(); document.querySelector('#public-query').select()");
    await page.send('Input.insertText',{text:query});
    await commit();
    const deadline=Date.now()+10000;
    while(!held.length && Date.now()<deadline) await new Promise(resolve=>setTimeout(resolve,25));
    check(held.length===1,'#258 submitted search response is actually held');
    await page.waitFor("new URL(location.href).searchParams.get('q')==='pending-submit-258' && document.querySelector('#public-query')",'#258 submitted scope mounts while its read is pending');
    await page.eval("window.__pending258Input=document.querySelector('#public-query'); window.__pending258Input.focus(); window.__pending258Input.select()");
    await page.send('Input.insertText',{text:draft});
    await page.send('Fetch.fulfillRequest',{requestId:held.shift(),responseCode:200,
      responseHeaders:[{name:'Content-Type',value:'application/json'}],body:Buffer.from(JSON.stringify(response)).toString('base64')});
    await page.waitFor("document.querySelector('[data-testid=public-no-results]')",'#258 submitted search resolves while a new query is being typed');
    await page.eval('new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)))');
    check(await page.eval(`document.activeElement===window.__pending258Input && document.querySelector('#public-query')===window.__pending258Input && window.__pending258Input.value===${JSON.stringify(draft)} && window.__pending258Input.selectionStart===${draft.length}`),
      '#258 a submitted read finishing cannot steal focus/caret from a newer unapplied query');
    await commit();
    await page.waitFor(`new URL(location.href).searchParams.get('q')===${JSON.stringify(draft)} && document.querySelector('#public-results') && document.activeElement?.id==='community-stream'`,'#258 the newer query commits with landmark focus');
    await commit();
    await page.waitFor("document.activeElement?.id==='community-stream'",'#258 repeating the same submitted query still focuses the landmark');
  } finally {
    for(const requestId of held) await page.send('Fetch.failRequest',{requestId,errorReason:'Aborted'}).catch(()=>{});
    await page.send('Fetch.disable');
    page.cdp.listeners=page.cdp.listeners.filter(listener=>listener!==gate);
    await page.eval('delete window.__pending258Input');
  }
}

// The dock opens below the tabs from the search icon or “/” and restores focus on Escape.
async function desktopSearchDock(page, {args, check, ready}) {
  await page.viewport(1440);
  await page.eval("document.documentElement.dataset.theme='light'; document.documentElement.style.fontSize='100%'; scrollTo(0,0)");
  await page.goto(args.base + '/');
  await ready();
  const url = await page.eval('location.href');
  await page.eval("const probe=document.createElement('input'); probe.id='dock-probe'; document.body.append(probe); probe.focus()");
  await page.key('/');
  check(await page.eval("document.querySelector('#explore').dataset.search==='closed' && document.activeElement.id==='dock-probe' && document.activeElement.value==='/'"),
    '#404 “/” typed in another field stays text and does not open the dock');
  await page.eval("document.querySelector('#dock-probe').remove(); document.activeElement?.blur()");
  await page.key('/');
  await page.waitFor("document.querySelector('#explore').dataset.search==='open' && document.activeElement?.id==='public-query'", '#404 “/” opens the dock and focuses its field');
  const dock = await page.eval(`(() => {
    const top=document.querySelector('.vk-explore__top').getBoundingClientRect();
    const field=document.querySelector('#public-search .vk-ui-search__shell'), shell=field.getBoundingClientRect();
    const modes=document.querySelector('.vk-explore-modes').getBoundingClientRect();
    const close=document.querySelector('#close-public-search');
    return {url: location.href, height: shell.height, radius: getComputedStyle(field).borderTopLeftRadius,
      headerVisible: document.querySelector('#open-public-search').getClientRects().length>0 && document.querySelector('#discovery-tools').getClientRects().length>0,
      order: close.getClientRects().length>0 && top.bottom <= modes.top && modes.bottom <= shell.top,
      submitHidden: getComputedStyle(document.querySelector('#public-search button[type=submit]')).display==='none'};
  })()`);
  check(dock.url === url && Math.abs(dock.height - 56) < 1 && parseFloat(dock.radius) >= dock.height / 2 &&
    dock.headerVisible && dock.order && dock.submitHidden,
    '#621 Search opens below stable tabs with a 56px pill and no history entry ' + JSON.stringify(dock));
  await page.capture('discovery-r4-dock-1440-light', { boundary: 'real-public-api', scenario: 'r4-search-dock' });
  await page.send('Input.insertText', { text: 'chưa gửi' });
  await page.eval("document.querySelector('#public-query').dispatchEvent(new CompositionEvent('compositionstart',{bubbles:true}))");
  await page.key('Escape');
  await page.eval("new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)))");
  check(await page.eval("document.querySelector('#explore').dataset.search==='open'"), '#404 Escape during IME composition keeps the dock open');
  await page.eval("document.querySelector('#public-query').dispatchEvent(new CompositionEvent('compositionend',{bubbles:true}))");
  await page.key('Escape');
  await page.waitFor(`document.querySelector('#explore').dataset.search==='closed' && document.activeElement?.id==='open-public-search' && location.href===${JSON.stringify(url)}`,
    '#404 Escape closes the dock and returns focus to the search icon');
  await page.eval("document.querySelector('#open-public-search').click()");
  await page.waitFor("document.querySelector('#explore').dataset.search==='open' && document.activeElement?.id==='public-query'", '#404 the icon reopens the dock');
  check(await page.eval("document.querySelector('#public-query').value===''"), '#404 closing dropped the unsent text');
  await page.click('#close-public-search');
  await page.waitFor("document.querySelector('#explore').dataset.search==='closed' && document.activeElement?.id==='open-public-search'", '#404 ← closes the dock and returns focus');

  // R4.3 (#427 AC-03, R4-INV-06): SSR /?q=… opens the dock with q; closing via ← or Escape drops
  // q (and cursor), keeps applied tags, and never leaves a hidden active filter.
  const ssrQueryRes = await fetch(args.base + '/?q=mua');
  const ssrQueryHtml = await ssrQueryRes.text();
  check(ssrQueryRes.status === 200 &&
    /<div\b[^>]*\bid="explore"[^>]*\bdata-search="open"/.test(ssrQueryHtml) &&
    /<input\b[^>]*\bid="public-query"[^>]*\bvalue="mua"/.test(ssrQueryHtml),
    '#427 AC-03 SSR /?q=mua renders with the search dock open and value="mua" before hydration');

  await page.goto(args.base + '/?q=mua');
  await ready();
  check(await page.eval("document.querySelector('#explore').dataset.search==='open' && document.querySelector('#public-query').value==='mua' && document.querySelector('#public-query').getClientRects().length>0 && document.querySelector('#open-public-search').getClientRects().length>0"),
    '#404/#427 AC-03 desktop SSR /?q=mua opens the dock with q kept');
  await page.click('#close-public-search');
  await page.waitFor("location.search==='' && document.querySelector('#explore').dataset.search==='closed' && document.querySelector('#public-query').getClientRects().length===0 && document.activeElement?.id==='open-public-search'",
    '#427 AC-03 closing desktop /?q=mua via #close-public-search drops q, closes dock and restores launcher focus');

  await page.goto(args.base + '/?q=mua');
  await ready();
  await page.eval("document.querySelector('#public-query').focus()");
  await page.key('Escape');
  await page.waitFor("location.search==='' && document.querySelector('#explore').dataset.search==='closed' && document.querySelector('#public-query').getClientRects().length===0 && document.activeElement?.id==='open-public-search'",
    '#427 AC-03 closing desktop /?q=mua via Escape drops q, closes dock and restores launcher focus');

  const catalog = await (await fetch(args.base + '/api/v1/tags')).json();
  const keptTag = catalog.tags.find(tag => tag.label === 'Ký ức');
  check(Boolean(keptTag), '#427 AC-03 uses the seeded Ký ức tag');
  // A stale cursor still belongs to the incoming URL and must be removed when closing the dock.
  const taggedQuery = new URLSearchParams({q: 'mua', tag: keptTag.slug, cursor: 'stale-discovery'});
  for (const width of [1440,390]) {
    await page.viewport(width, 844);
    for (const via of ['button', 'Escape']) {
      await page.goto(args.base + '/?' + taggedQuery);
      await ready();
      const label = `${width}px / ${via}`;
      check(await page.eval(`(() => {
        const query = new URL(location.href).searchParams;
        return document.querySelector('#explore').dataset.search === 'open' &&
          document.querySelector('#public-query').value === 'mua' &&
          document.querySelector('#public-query').getClientRects().length > 0 &&
          query.get('tag') === ${JSON.stringify(keptTag.slug)} && query.has('cursor') &&
          (${width} > 759 || getComputedStyle(document.querySelector('#bottom-navigation')).display === 'none');
      })()`), `#427 AC-03 ${label}: direct q/tag/cursor URL opens the dock`);
      if (via === 'button') {
        await page.click('#close-public-search');
      } else {
        await page.eval("document.querySelector('#public-query').focus()");
        await page.key('Escape');
      }
      await page.waitFor(`(() => {
        const query = new URL(location.href).searchParams;
        return !query.has('q') && !query.has('cursor') &&
          query.get('tag') === ${JSON.stringify(keptTag.slug)} &&
          document.querySelector('#explore').dataset.search === 'closed' &&
          document.querySelector('#public-query').getClientRects().length === 0 &&
          document.activeElement?.id === 'open-public-search' &&
          (${width} > 759 || getComputedStyle(document.querySelector('#bottom-navigation')).display !== 'none');
      })()`, `#427 AC-03 ${label}: closing drops q/cursor, retains tag and restores launcher focus`);
    }
  }

  await page.viewport(1440);
  await page.goto(args.base + '/');
  await ready();
}

// #427 AC-02 / DQ-03: the same mode destinations are reachable at every width.
async function checkExploreModesTabAndAx(page, {check, label, member = true}) {
  const destinations = [
    ['explore-mode-works', 'Tất cả'],
    ...(member ?
      [['explore-mode-following', 'Đang theo dõi']] : []),
    ['explore-mode-authors', 'Tác giả'],
  ];
  const stops = [];
  await page.eval("document.querySelector('#discovery-tools .vk-brand').focus()");
  for (let step = 0; step <= destinations.length; step++) {
    await page.key('Tab');
    stops.push(await page.eval(`(() => {
      const active = document.activeElement;
      return active?.closest('.vk-explore-modes') ? active.id : null;
    })()`));
  }
  check(JSON.stringify(stops) === JSON.stringify([...destinations.map(([id]) => id), null]),
    `#427 AC-02 ${label}: Tab visits each mode once in order and leaves the navigation (${JSON.stringify(stops)})`);

  await page.send('Accessibility.enable');
  for (const [id, name] of destinations) {
    const {present, ax, dom} = await collectElementAccessibility(page, '#' + id);
    check(present && !ax.ignored && ax.role === 'link' && ax.name === name &&
      dom.ariaCurrent === (id === 'explore-mode-works' ? 'page' : null),
      `#427 AC-02 ${label}: #${id} has the expected AX link name and current state (${JSON.stringify(ax)})`);
  }
  const {nodes} = await page.send('Accessibility.getFullAXTree');
  const nav = nodes.filter(node => !node.ignored && node.role?.value === 'navigation' &&
    node.name?.value === 'Chế độ Khám phá');
  const byId = new Map(nodes.map(node => [node.nodeId, node]));
  const exposed = [];
  const queue = [...(nav[0]?.childIds ?? [])];
  while (queue.length) {
    const node = byId.get(queue.shift());
    if (!node) continue;
    if (!node.ignored && ['link', 'button', 'DisclosureTriangle'].includes(node.role?.value)) {
      exposed.push([node.role.value, node.name?.value]);
    }
    queue.push(...(node.childIds ?? []));
  }
  check(nav.length === 1 && JSON.stringify(exposed) === JSON.stringify(destinations.map(([, name]) => ['link', name])),
    `#427 AC-02 ${label}: AX navigation exposes one set of mode links (${JSON.stringify(exposed)})`);
  await page.eval("document.activeElement?.blur(); scrollTo(0,0)");
}

// Revalidation changes freshness text, not the identity of focused recovery controls.
async function emptyRefreshFocus(page, {args, check}) {
  const held = [];
  const gate = ({sessionId, method, params}) => {
    if (sessionId === page.sessionId && method === 'Fetch.requestPaused') held.push(params.requestId);
  };
  page.cdp.listeners.push(gate);
  await page.send('Fetch.enable', {patterns:[{urlPattern:args.base+'/api/v1/works*', requestStage:'Request'}]});
  try {
    await page.eval(`(() => {
      const recovery = document.querySelector('#find-authors-for-query');
      window.__empty258 = { recovery, art: document.querySelector('.vk-empty-state__art'), now: performance.now.bind(performance) };
      recovery.focus();
      performance.now = () => window.__empty258.now() + 121000;
      window.dispatchEvent(new Event('focus'));
    })()`);
    const deadline = Date.now()+10000;
    while (!held.length && Date.now()<deadline) await new Promise(resolve=>setTimeout(resolve, 25));
    check(held.length===1, '#258 empty revalidation is actually held');
    await page.waitFor("document.querySelector('#public-collection').dataset.refreshing==='true'", '#258 retained empty refreshing');
    const stable = () => page.eval(`document.querySelector('#find-authors-for-query')===window.__empty258.recovery &&
      document.activeElement===window.__empty258.recovery && document.querySelector('.vk-empty-state__art')===window.__empty258.art &&
      document.querySelector('.vk-empty-state__message').textContent==='Kết quả này là bản đã tải.' &&
      document.querySelectorAll('#public-results [role=status]').length===1`);
    check(await stable(), '#258 retained empty refresh preserves recovery focus, artwork node and one qualified status');
    await page.send('Fetch.fulfillRequest', {requestId:held.shift(), responseCode:503,
      responseHeaders:[{name:'Content-Type',value:'application/json'},{name:'Retry-After',value:'30'}],
      body:Buffer.from(JSON.stringify({error:{code:'dependency_unavailable'}})).toString('base64')});
    await page.waitFor("document.querySelector('[data-testid=public-refresh-failed]') && document.querySelector('#public-collection').dataset.refreshing==='false'", '#258 retained empty refresh failure');
    check(await stable(), '#258 failed empty refresh preserves recovery focus and truthful absence qualifier');
  } finally {
    for (const requestId of held) await page.send('Fetch.failRequest', {requestId,errorReason:'Aborted'}).catch(()=>{});
    await page.send('Fetch.disable');
    page.cdp.listeners=page.cdp.listeners.filter(listener=>listener!==gate);
    await page.eval("if(window.__empty258){performance.now=window.__empty258.now; delete window.__empty258}");
  }
}

export async function discoveryCorpus(page, { args, check }) {
  await publicLoading(page, args, check);
  await invalidCursorRecovery(page, args, check);
  await discoveryChipGeometry(page, { args, check });
  const load = async path => (await fetch(args.base+path)).json();
  const catalog = await load('/api/v1/tags');
  const form = catalog.tags.find(t=>t.label==='Thơ'), theme = catalog.tags.find(t=>t.label==='Ký ức');
  const initial = await load('/api/v1/works');
  check(initial.works.length === 12 && initial.next_cursor, 'real corpus has a bounded first page and a real cursor');
  const found = await load('/api/v1/works?q=ben%20hiem%20sau%20trang%20dau');
  check(found.works.length===1 && !initial.works.some(w=>w.work_id===found.works[0].work_id), 'whole-library search finds a work beyond the first 12');
  const tags = '&tag='+encodeURIComponent(form.slug)+'&tag='+encodeURIComponent(theme.slug);
  await page.goto(args.base+'/?q=kiem%20thu'+tags);
  await page.waitFor("document.documentElement.dataset.hydrated==='true' && document.querySelector('#public-next')", 'real filtered page');
  check(await page.eval("document.querySelectorAll('.vk-public-row').length===12"), 'first filtered page renders 12 real rows');
  await page.click('#public-next');
  await page.waitFor("new URL(location.href).searchParams.has('cursor') && document.querySelectorAll('.vk-public-row').length===2", 'next real page');
  check(await page.eval("document.querySelectorAll('#public-results [role=status]').length===1 && document.querySelector('.vk-public-scope').textContent==='Trang này: 2 tác phẩm'"), '#225 final-page count stays visible and never claims a library total');
  const context = await page.eval('location.pathname+location.search');
  check(await page.eval("document.querySelector('#public-next')===null && [...document.querySelectorAll('.vk-public-row__link')].every(a=>a.hasAttribute('href'))"), '#225 continuation ends honestly and results remain native links');
  await page.eval('history.back()');
  await page.waitFor("!new URL(location.href).searchParams.has('cursor') && document.querySelectorAll('.vk-public-row').length===12", '#225 Back restores filtered first page');
  await page.eval('history.forward()');
  await page.waitFor("location.pathname+location.search==="+JSON.stringify(context)+" && document.querySelectorAll('.vk-public-row').length===2", '#225 Forward restores q/tags/cursor and second-page results');
  await page.click('.vk-public-row__link');
  await page.waitFor("document.querySelector('#reader-paper')", 'real fixture reader');
  check(await page.eval("document.querySelector('#reader-return').getAttribute('href').startsWith("+JSON.stringify(context)+")"), 'reader retains query, both tags and opaque cursor');
  check(await page.eval("document.querySelectorAll('.vk-reader-tags a').length===2"), 'reader links the public tags supplied by the snapshot');
  await page.click('#reader-return');
  await page.waitFor("document.activeElement?.matches('.vk-public-row__link')", 'second-page focus restoration');
  await page.reload();
  check(await page.eval('location.pathname+location.search')===context, 'reload preserves filtered second-page URL');
  await page.capture('discovery-real-second-page', {boundary:'real-api+SQL-publication-fixture',scenario:'query-tags-pagination-reader-return'});
  await page.viewport(1440);
  const retainedTags = [form.slug, theme.slug].sort();
  const clearsOnlySearch = params => !params.has('q') && !params.has('cursor') &&
    JSON.stringify(params.getAll('tag').sort()) === JSON.stringify(retainedTags);
  const clearHref = await page.eval("document.querySelector('#clear-public-search').href");
  check(clearsOnlySearch(new URL(clearHref).searchParams), '#621 native X keeps both legacy tags and drops q/cursor');
  await page.click('#clear-public-search');
  await page.waitFor("!new URL(location.href).searchParams.has('q') && document.activeElement?.id === 'public-query'",
    '#621 clear after pagination restores input focus');
  check(clearsOnlySearch(new URL(await page.eval('location.href')).searchParams),
    '#621 hydrated clear preserves the same tag set as the native X');
  await page.eval('history.back()');
  await page.waitFor("location.pathname+location.search==="+JSON.stringify(context)+" && document.querySelectorAll('.vk-public-row').length===2",
    '#621 Back after clearing restores the same paginated results');
  await page.waitFor("document.querySelector('#open-tag-filters')", '#308 filter entry on the second page');
  await page.click('#open-tag-filters');
  await page.waitFor("document.querySelector('#tag-picker')?.open && document.querySelector('#modal-tag-form a')", 'legacy URL opens single-tag picker');
  check(await page.eval("document.querySelectorAll('#applied-filters .vk-applied-tag').length===2 && !document.querySelector('#modal-tag-form [aria-current]')"), 'legacy AND results retain both labels, with no single active option');
  await page.click('#modal-tag-form a[href*='+JSON.stringify(encodeURIComponent(form.slug))+']');
  await page.waitFor("!new URL(location.href).searchParams.has('cursor') && new URL(location.href).searchParams.get('q')==='kiem thu' && new URL(location.href).searchParams.getAll('tag').length===1 && document.querySelector('#public-results')", '#225 changed Apply resets second-page cursor and keeps q plus remaining tag');

  // Verify real non-zero browse scroll restoration on 390px across open -> close search.
  await page.goto(args.base + '/');
  await page.waitFor("document.documentElement.dataset.hydrated==='true' && document.querySelectorAll('.vk-public-row').length>=6", 'multi-card real browse page');
  await page.viewport(390);
  await page.eval('scrollTo(0, 360)');
  await page.waitFor('Math.abs(scrollY - 360) <= 2', 'scrolled browse feed');
  // R4.2 (#404): the phone header scrolls with the page (no sticky band); opening search from it
  // still remembers and restores the browse position.
  check(await page.eval(`(() => {
    const r = document.querySelector('#open-public-search').getBoundingClientRect();
    return r.bottom <= 0 && getComputedStyle(document.querySelector('.vk-explore__top')).position === 'static' && Math.abs(scrollY - 360) <= 2;
  })()`), '#404 phone header is not sticky while scrolled');
  await page.eval("document.querySelector('#open-public-search').click()");
  await page.waitFor("document.querySelector('#explore').dataset.search==='open' && scrollY === 0 && document.activeElement?.id==='public-query'", '#308 r1 mobile search scrolls to top on open');
  await page.click('#close-public-search');
  await page.waitFor("document.querySelector('#explore').dataset.search==='closed' && Math.abs(scrollY - 360) <= 2 && document.activeElement?.id==='open-public-search'", '#308 r1 closing mobile search restores browse scrollY and trigger focus');

  const tagContext='/the/'+encodeURIComponent(form.slug)+'?q=kiem%20thu&tag='+encodeURIComponent(theme.slug);
  await page.goto(args.base+tagContext);
  await page.waitFor("document.documentElement.dataset.hydrated==='true' && document.querySelector('.vk-public-row__link')", 'real tag page');
  await page.viewport(1440);
  await page.capture('discovery-real-tag-desktop', {boundary:'real-api+SQL-publication-fixture',scenario:'tag-detail-with-results'});
  await page.click('.vk-public-row__link');
  await page.waitFor("document.querySelector('#reader-return')", 'reader from tag page');
  check(await page.eval("document.querySelector('#reader-return').getAttribute('href').startsWith("+JSON.stringify(tagContext)+")"), 'reader returns to the same tag page and extra filters');
  await page.click('#reader-return');
  await page.waitFor("document.activeElement?.matches('.vk-public-row__link')", 'tag-page focus restored');

  // Orchestrated transport race: A is held while B completes. No persistence claim.
  const held=[];
  const gate = ({sessionId,method,params}) => {
    if(sessionId!==page.sessionId || method!=='Fetch.requestPaused') return;
    if((new URL(params.request.url).searchParams.get('q')==='race-100-a' && !new URL(params.request.url).searchParams.has('tag')) || new URL(params.request.url).searchParams.get('q')==='race-100-c') held.push(params.requestId);
    else page.send('Fetch.continueRequest',{requestId:params.requestId}).catch(()=>{});
  };
  page.cdp.listeners.push(gate);
  await page.send('Fetch.enable',{patterns:[{urlPattern:args.base+'/api/v1/works*',requestStage:'Request'}]});
  const navigate = async href => page.eval("(() => {const a=document.createElement('a');a.href="+JSON.stringify(href)+";document.body.append(a);a.click();a.remove();})()");
  try {
    await navigate('/?q=race-100-a');
    await page.waitFor("location.search==='?q=race-100-a'",'A navigation');
    const deadline=Date.now()+10000;
    while(!held.length && Date.now()<deadline) await new Promise(r=>setTimeout(r,50));
    check(held.length===1,'older query request is held deterministically');
    check(await page.eval("!document.querySelector('.vk-empty-state__art')"), '#258 unresolved read never mounts absence artwork');
    await page.waitFor("document.querySelector('#open-tag-filters')", 'filter entry available while A is pending');
    await page.click('#open-tag-filters');
    await page.waitFor("document.querySelector('#tag-picker')?.open && document.querySelector('#modal-tag-form a')", 'filter controls available while A is pending');
    await page.click('#modal-tag-form .vk-discovery-option');
    await page.waitFor("new URL(location.href).searchParams.has('tag') && document.querySelector('[data-testid=public-no-results]')",'B filter completes while A remains held');
    const body={...found,filters:{q:'race-100-a',tags:[]},works:[{...found.works[0],title:'STALE RACE RESULT #100'}]};
    await page.send('Fetch.fulfillRequest',{requestId:held.shift(),responseCode:200,responseHeaders:[{name:'Content-Type',value:'application/json'},{name:'Cache-Control',value:'no-store'}],body:Buffer.from(JSON.stringify(body)).toString('base64')});
    await page.eval('new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)))');
    check(await page.eval("new URL(location.href).searchParams.has('tag') && !document.querySelector('#public-results').textContent.includes('STALE RACE RESULT')"),'late A cannot overwrite B or acquire B filter labels');
    await navigate('/?q=race-100-c');
    const until=Date.now()+10000;
    while(!held.length && Date.now()<until) await new Promise(r=>setTimeout(r,50));
    check(held.length===1,'C is held before entering a new draft query');
    await page.eval("document.querySelector('#public-query').focus(); document.querySelector('#public-query').select()");
    await page.send('Input.insertText',{text:'chữ đang gõ chưa áp dụng'});
    await page.send('Fetch.fulfillRequest',{requestId:held.shift(),responseCode:200,responseHeaders:[{name:'Content-Type',value:'application/json'},{name:'Cache-Control',value:'no-store'}],body:Buffer.from(JSON.stringify({works:[],filters:{q:'race-100-c',tags:[]},next_cursor:null})).toString('base64')});
    await page.waitFor("document.querySelector('[data-testid=public-no-results]')", 'C result ready');
    check(await page.eval("document.querySelector('#public-query').value==='chữ đang gõ chưa áp dụng' && document.activeElement?.id==='public-query' && document.querySelector('#public-query').selectionStart==='chữ đang gõ chưa áp dụng'.length && new URL(location.href).searchParams.get('q')==='race-100-c'"),'a result arriving during typing preserves the unapplied query buffer');
  } finally {
    for(const requestId of held) await page.send('Fetch.failRequest',{requestId,errorReason:'Aborted'}).catch(()=>{});
    await page.send('Fetch.disable');
    page.cdp.listeners.splice(page.cdp.listeners.indexOf(gate),1);
  }
}

// #621 · Guest Product search through actual SSR reads and Chrome hydration.
export async function searchR5(page, {base, check, textContrast}) {
  const query = 'không có tác phẩm này #100';
  const destination = '/?q=' + encodeURIComponent(query);
  const ready = "document.querySelector('#explore')?.dataset.enhanced === 'true'";
  const {identifier} = await page.send('Page.addScriptToEvaluateOnNewDocument', {source: `
    window.followingModeExposed = false;
    new MutationObserver(records => {
      window.followingModeExposed ||= records.some(record => [...record.addedNodes].some(node =>
        node.nodeType === Node.ELEMENT_NODE &&
        (node.id === 'explore-mode-following' || node.querySelector('#explore-mode-following'))));
    }).observe(document, {subtree: true, childList: true});
  `});
  try {
    await page.goto(base + '/');
    await page.waitFor(ready, 'plain guest Discovery hydrates');
    await checkExploreModesTabAndAx(page, {check, label: 'R5 guest after hydration', member: false});
    check(await page.eval("window.followingModeExposed === false && !document.querySelector('#explore-mode-following')"),
      '#621 no Following link mounts during guest hydration');
  } finally {
    await page.send('Page.removeScriptToEvaluateOnNewDocument', {identifier});
  }

  await page.send('Emulation.setScriptExecutionDisabled', {value: true});
  try {
    await page.goto(base + destination);
    check(await page.eval(`(() => {
      const clear = document.querySelector('#clear-public-search');
      return clear.getAttribute('href') === '/' && clear.getClientRects().length === 1 &&
        document.querySelectorAll('[aria-label="Xóa tìm kiếm"]').length === 1 &&
        document.querySelector('#find-authors-for-query').getAttribute('href').startsWith('/?mode=authors&q=');
    })()`), '#621 native SSR keeps one visible clear link and the author destination');
    await page.eval("document.querySelector('#clear-public-search').focus()");
    await page.key('Enter', 0, {settle: false});
    await page.waitFor("location.search === '' && document.querySelector('#explore')?.dataset.enhanced === 'false'",
      '#621 keyboard clear follows its native href without JavaScript');
  } finally {
    await page.send('Emulation.setScriptExecutionDisabled', {value: false});
  }

  await page.goto(base + destination);
  await page.waitFor(ready + " && document.querySelector('#find-authors-for-query')", 'applied guest query hydrates');
  for (const [width, height] of [[320, 800], [390, 844], [834, 900], [1440, 900]]) {
    await page.viewport(width, height);
    for (const theme of ['light', 'dark']) for (const scale of [100, 150, 200]) {
      const scene = [width + 'x' + height, theme, scale].join('-');
      const facts = await page.eval(`(async () => {
        const root = document.documentElement;
        root.dataset.theme = '${theme}';
        root.dataset.text = '${scale}';
        await document.fonts.ready;
        await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
        await Promise.allSettled(document.getAnimations()
          .filter(animation => animation instanceof CSSTransition).map(animation => animation.finished));
        scrollTo(0, 0);
        const box = selector => document.querySelector(selector).getBoundingClientRect();
        const mid = rect => (rect.top + rect.bottom) / 2;
        const bar = box('#public-search .vk-ui-search__shell');
        const field = box('#public-query');
        const action = box('#find-authors-for-query');
        const icons = ['#close-public-search', '#clear-public-search'].map(selector => ({
          button: box(selector), icon: box(selector + ' svg'),
        }));
        const barStyle = getComputedStyle(document.querySelector('#public-search .vk-ui-search__shell'));
        const inputStyle = getComputedStyle(document.querySelector('#public-query'));
        const actionStyle = getComputedStyle(document.querySelector('#find-authors-for-query'));
        const result = document.querySelector('[data-testid="public-no-results"]');
        const link = document.querySelector('#find-authors-for-query');
        return {
          overflow: root.scrollWidth - root.clientWidth,
          barHeight: bar.height, radius: parseFloat(barStyle.borderTopLeftRadius),
          border: barStyle.borderWidth, fieldWidth: field.width,
          inputSize: inputStyle.fontSize, inputWeight: inputStyle.fontWeight,
          uiFont: inputStyle.fontFamily.startsWith('"Be Vietnam Pro"'),
          targets: icons.map(({button,icon}) => [button.width, button.height, icon.width, mid(icon) - mid(bar)]),
          inset: [icons[0].button.left - bar.left, bar.right - icons[1].button.right],
          actionHeight: action.height, actionRadius: parseFloat(actionStyle.borderTopLeftRadius),
          resultActions: result.querySelectorAll('.vk-empty-state__actions a').length,
          copy: result.querySelector('.vk-empty-state__title')?.textContent === 'Không tìm thấy tác phẩm' &&
            result.querySelector('.vk-empty-state__message')?.textContent === 'Thử một từ khóa khác.' &&
            link.textContent.trim() === 'Tìm tác giả' && link.classList.contains('vk-ui-button--tonal') &&
            link.firstElementChild.matches('[data-icon="person-outline"][aria-hidden="true"]') &&
            new URL(link.href).searchParams.get('q') === ${JSON.stringify(query)} &&
            !result.textContent.includes(${JSON.stringify(query)}),
        };
      })()`);
      check(facts.overflow <= 1 && facts.barHeight >= 56 && facts.radius >= facts.barHeight / 2 &&
        facts.border === '0px' && facts.fieldWidth > 0 && facts.uiFont && facts.inputWeight === '400' &&
        facts.inputSize === (16 * scale / 100) + 'px' && Math.abs(facts.inset[0] - facts.inset[1]) <= 1 &&
        facts.targets.every(([w, h, glyph, center]) => w >= 48 && h >= 48 && glyph >= 24 && Math.abs(center) <= 1) &&
        facts.targets[0][2] === facts.targets[1][2] && facts.copy && facts.resultActions === 1 &&
        facts.actionHeight >= 48 && facts.actionRadius >= facts.actionHeight / 2,
        '#621 Search R5 geometry and recovery ' + scene + ' ' + JSON.stringify(facts));
      if (width === 834 && scale === 100) {
        await textContrast(page, theme, 'search-r5', ['#public-query', '#find-authors-for-query',
          '.vk-discovery-query-miss .vk-empty-state__message']);
      }
      await page.capture('search-r5-' + scene, {
        requiredTargets: ['#close-public-search', '#public-query', '#clear-public-search', '#find-authors-for-query'],
        boundary: 'Product SSR + Chrome hydration + App PostgreSQL scratch',
        scenario: 'guest-search-query-miss',
      });
    }
  }

  await page.viewport(320, 800);
  await page.eval(`(async () => {
    document.documentElement.dataset.text = '200';
    await document.fonts.ready;
    document.querySelector('#find-authors-for-query').scrollIntoView({block: 'center'});
  })()`);
  check(await page.eval(`(() => {
    const a = document.querySelector('#find-authors-for-query');
    const r = a.getBoundingClientRect();
    return r.top >= 0 && r.bottom <= innerHeight &&
      a.contains(document.elementFromPoint(r.x + r.width / 2, r.y + r.height / 2));
  })()`), '#621 at 320px and 200%, scrolling reaches the full author recovery target');
  await page.capture('search-r5-320x800-dark-200-recovery', {
    requiredTargets: ['#find-authors-for-query'],
    boundary: 'Product SSR + Chrome hydration + App PostgreSQL scratch',
    scenario: 'guest-search-query-miss-scrolled-to-recovery',
  });

  await page.viewport(834, 900);
  await page.eval(`(async () => {
    const root = document.documentElement;
    root.dataset.theme = 'light';
    delete root.dataset.text;
    root.style.fontSize = '200%';
    await document.fonts.ready;
    await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
    await Promise.allSettled(document.getAnimations()
      .filter(animation => animation instanceof CSSTransition).map(animation => animation.finished));
    scrollTo(0, 0);
  })()`);
  await page.capture('search-r5-834x900-light-200-reference-replay', {
    requiredTargets: ['#public-query', '#clear-public-search', '#find-authors-for-query'],
    boundary: 'Product SSR + Chrome hydration + App PostgreSQL scratch',
    scenario: 'guest-search-query-miss-historical-scale-method',
    textScaleLane: 'dom-override-geometry-only',
  });
  await page.eval("document.documentElement.style.fontSize=''; document.documentElement.dataset.text='100'");
  try {
    await page.send('Emulation.setEmulatedMedia', {features: [
      {name: 'forced-colors', value: 'active'}, {name: 'prefers-reduced-motion', value: 'reduce'},
    ]});
    await page.eval("document.querySelector('#close-public-search').focus()");
    for (const selector of ['#public-query', '#clear-public-search']) {
      await page.key('Tab');
      check(await page.eval(`(() => {
        const shell = document.querySelector('#public-search .vk-ui-search__shell');
        const field = document.querySelector('#public-query');
        const active = document.querySelector('${selector}');
        const outline = el => getComputedStyle(el).outlineStyle === 'solid' &&
          parseFloat(getComputedStyle(el).outlineWidth) > 0;
        return active === document.activeElement && active.matches(':focus-visible') &&
          getComputedStyle(shell).borderWidth === '1px' &&
          (active === field ? outline(shell) && !outline(field) : outline(active) && !outline(shell));
      })()`), '#621 forced colors keep a visible keyboard perimeter on ' + selector);
    }
  } finally {
    await page.send('Emulation.setEmulatedMedia', {features: []});
  }

  await page.eval("document.querySelector('#public-query').focus(); document.querySelector('#public-query').select()");
  await page.key('Backspace');
  await page.waitFor("document.querySelector('#public-query').value === '' && document.querySelector('#clear-public-search')",
    '#621 editing the draft to blank keeps the applied query recoverable');
  await page.key('Tab');
  await page.key('Enter');
  await page.waitFor("location.search === '' && document.activeElement?.id === 'public-query'",
    '#621 keyboard clear resets applied q and restores input focus');
  await page.key('Escape');
  await page.waitFor("document.activeElement?.id === 'open-public-search'", '#621 Escape focuses the search launcher');

  const started = Date.now();
  let reads = 0;
  const observe = ({sessionId, method, params}) => {
    if (sessionId === page.sessionId && method === 'Network.requestWillBeSent' &&
        new URL(params.request.url).pathname === '/api/v1/works') reads++;
  };
  page.cdp.listeners.push(observe);
  try {
    await page.eval("window.originalSearchInput = document.querySelector('#public-query')");
    for (let cycle = 0; cycle < 30; cycle++) {
      await page.key('Enter');
      await page.waitFor("document.activeElement?.id === 'public-query'", 'R5 open cycle ' + cycle);
      await page.send('Input.insertText', {text: 'Tiếng Việt chưa gửi'});
      await page.key('Tab');
      await page.key('Enter');
      await page.waitFor("document.activeElement === originalSearchInput && originalSearchInput.value === ''",
        'R5 clear cycle ' + cycle);
      await page.key('Escape');
      await page.waitFor("document.activeElement?.id === 'open-public-search'", 'R5 close cycle ' + cycle);
    }
    check(reads === 0, '#621 30 open/type/clear/close cycles in ' + (Date.now() - started) + ' ms, 0 work reads');
  } finally {
    page.cdp.listeners = page.cdp.listeners.filter(listener => listener !== observe);
  }
}
