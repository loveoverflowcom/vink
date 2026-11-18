// Isolated CSS experiment on the guest scratch thread. All CSS dies with this page.
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { dirname, join } from 'node:path';
import { readFileSync, writeFileSync } from 'node:fs';

const work = 'e4202094-9563-02e5-519d-6f8fa6a1af4c';
const ready = "document.documentElement.dataset.hydrated==='true'";
const frames = 'new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)))';
const metrics = async (page) =>
  Object.fromEntries(
    (await page.send('Performance.getMetrics')).metrics
      .filter((m) =>
        ['LayoutDuration', 'RecalcStyleDuration', 'ScriptDuration', 'TaskDuration'].includes(m.name),
      )
      .map((m) => [m.name, m.value * 1000]),
  );
const css = (height) =>
  `@media screen { #root-list { content-visibility:auto; contain-intrinsic-block-size:auto ${height}px } }`;

export async function readerContainment(page, { args, check }) {
  if (!/^http:\/\/127\.0\.0\.1:\d+$/.test(args.base)) throw new Error('loopback scratch only');
  const folder = dirname(args.state);
  const data = await (await fetch(args.base + '/api/v1/works/' + work + '/reader')).json();
  const digest = createHash('sha256').update(data.document.source).digest('hex');
  const paragraphs = data.document.source.split('\n\n').length;
  const large = paragraphs === 200000;
  const raw = {
    source_sha: execFileSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' }).trim(),
    browser: await page.cdp.send('Browser.getVersion'),
    corpus_sha256: digest,
    paragraphs,
    samples: [],
    checks: [],
    status: 'FAIL',
  };
  const flush = () => writeFileSync(join(folder, 'reader-containment.json'), JSON.stringify(raw, null, 2));
  const record = (name, ok) => {
    raw.checks.push({ name, ok });
    check(ok, name);
  };
  let intrinsic;
  await page.viewport(1280, 900);
  await page.send('Performance.enable');
  try {
    // Alternate pair order, calibrated by one excluded A mount on the same content and device.
    for (const [i, mode] of [
      'calibrate',
      ...Array.from({ length: 10 }, (_, i) => (i % 2 ? ['B', 'A'] : ['A', 'B'])).flat(),
    ].entries()) {
      await page.goto('about:blank');
      await page.goto(args.base + '/?q=kiem%20thu');
      await page.waitFor(
        ready + ' && !!document.querySelector(".vk-public-row__link")',
        'hydrated discovery',
      );
      if (mode === 'B')
        await page.eval(`(()=>{const s=document.createElement('style');s.id='contain-probe';
        s.textContent=${JSON.stringify(css(intrinsic))};document.head.append(s)})()`);
      let paused;
      const listener = (e) => {
        if (e.sessionId === page.sessionId && e.method === 'Fetch.requestPaused') paused = e.params.requestId;
      };
      page.cdp.listeners.push(listener);
      try {
        await page.send('Fetch.enable', {
          patterns: [
            {
              urlPattern: args.base + '/api/v1/works/' + work + '/discussion/public*',
              requestStage: 'Response',
            },
          ],
        });
        await page.click('.vk-public-row__link');
        await page.waitFor("!!document.querySelector('.vk-document')", 'full authoritative article', 120000);
        const end = Date.now() + 10000;
        while (!paused && Date.now() < end) await new Promise((r) => setTimeout(r, 20));
        check(
          !!paused && (await page.eval("!document.querySelector('.vk-discussion-comment')")),
          'secondary pending after article',
        );
        // Exclude initial article layout, including the 200k paragraphs on the #229 control.
        await page.eval(frames);
        const before = await metrics(page);
        await page.send('Fetch.disable');
        await page.waitFor(
          "document.querySelectorAll('.vk-discussion-comment').length===40",
          'one mounted discussion page',
          120000,
        );
        await page.eval(frames);
        const after = await metrics(page);
        const dom =
          await page.eval(`(()=>{const list=document.querySelector('#root-list'),r=list.getBoundingClientRect();
          return {height:r.height,top:r.top,visibility:getComputedStyle(list).contentVisibility,
            paragraphs:document.querySelectorAll('.vk-document p').length,
            roots:list.children.length,comments:list.querySelectorAll('.vk-discussion-comment').length}})()`);
        check(
          dom.paragraphs === paragraphs && dom.roots === 20 && dom.comments === 40,
          'complete article and bounded first page',
        );
        if (mode === 'calibrate') {
          intrinsic = Math.ceil(dom.height);
          raw.intrinsic_css_px = intrinsic;
          continue;
        }
        const delta = Object.fromEntries(Object.entries(after).map(([k, v]) => [k, v - before[k]]));
        const discussionResponse = await page.eval(`performance.getEntriesByType('resource')
          .filter(e=>new URL(e.name).pathname.endsWith('/discussion/public'))
          .map(e=>({encoded_body_bytes:e.encodedBodySize,transfer_bytes:e.transferSize}))`);
        check(
          discussionResponse.length === 1 && discussionResponse[0].encoded_body_bytes > 0,
          'one measured discussion response for this mount',
        );
        raw.samples.push({
          i,
          mode,
          ...dom,
          ms: delta,
          discussion_response: discussionResponse[0],
          dom_counters: await page.send('Memory.getDOMCounters'),
          heap: await page.send('Runtime.getHeapUsage'),
        });
        if (i <= 2)
          await page.capture('containment-' + mode, { scenario: 'Reader top', boundary: 'real-public-api' });
        flush();
      } finally {
        await page.send('Fetch.disable');
        page.cdp.listeners = page.cdp.listeners.filter((l) => l !== listener);
      }
    }

    // Bring B back for semantic checks irrespective of the final pair order.
    await page.eval(`(()=>{let s=document.querySelector('#contain-probe');if(!s){s=document.createElement('style');s.id='contain-probe';document.head.append(s)}
      s.textContent=${JSON.stringify(css(intrinsic))}})()`);
    record(
      'full DOM text digest',
      await page.eval(`(async()=>{const text=Array.from(document.querySelectorAll('.vk-document p'),p=>p.textContent).join('\\n\\n');
      const bytes=new Uint8Array(await crypto.subtle.digest('SHA-256',new TextEncoder().encode(text)));
      return Array.from(bytes,v=>v.toString(16).padStart(2,'0')).join('')===${JSON.stringify(digest)}})()`),
    );
    const comment = await page.eval("document.querySelector('.vk-discussion-body').textContent");
    record(
      'browser find in secondary',
      await page.eval(`window.find(${JSON.stringify(comment)},false,false,true)`),
    );
    await page.eval('getSelection().removeAllRanges()');
    await page.cdp.send('Browser.grantPermissions', {
      origin: args.base,
      permissions: ['clipboardReadWrite', 'clipboardSanitizedWrite'],
    });
    const selected =
      await page.eval(`(()=>{const r=document.createRange();r.setStart(document.querySelector('.vk-document p:last-of-type'),0);
      const last=document.querySelector('.vk-discussion-body');r.setEnd(last,last.childNodes.length);
      getSelection().removeAllRanges();getSelection().addRange(r);return getSelection().toString()})()`);
    await page.send('Input.dispatchKeyEvent', {
      type: 'rawKeyDown',
      key: 'c',
      code: 'KeyC',
      windowsVirtualKeyCode: 67,
      modifiers: 2,
    });
    await page.send('Input.dispatchKeyEvent', {
      type: 'keyUp',
      key: 'c',
      code: 'KeyC',
      windowsVirtualKeyCode: 67,
      modifiers: 2,
    });
    const copied = await page.eval('navigator.clipboard.readText()');
    const normalized = (text) => text.replace(/\s+/gu, ' ').trim();
    raw.clipboard = {
      selected_chars: selected.length,
      copied_chars: copied.length,
      same_text: normalized(copied) === normalized(selected),
      ends_in_comment: normalized(selected).endsWith(normalized(comment)),
    };
    record(
      'clipboard spans article and discussion',
      raw.clipboard.ends_in_comment && raw.clipboard.same_text,
    );
    await page.eval(
      "getSelection().removeAllRanges();location.hash=document.querySelector('.vk-discussion-comment').id",
    );
    await page.eval(frames);
    record(
      'fragment exposes comment',
      await page.eval(
        "Math.abs(document.querySelector('.vk-discussion-comment').getBoundingClientRect().top)<innerHeight",
      ),
    );
    await page.key('Tab');
    record('keyboard enters discussion', await page.eval("!!document.activeElement?.closest('#root-list')"));
    if (!large) {
      await page.send('Emulation.setEmulatedMedia', { media: 'print' });
      record(
        'print uses ordinary layout',
        await page.eval(
          "getComputedStyle(document.querySelector('#root-list')).contentVisibility==='visible'",
        ),
      );
      const pdf = await page.send('Page.printToPDF', { printBackground: true });
      writeFileSync(join(folder, 'print.pdf'), Buffer.from(pdf.data, 'base64'));
      await page.send('Emulation.setEmulatedMedia', { media: 'screen' });
    }
    const vectors = JSON.parse(readFileSync('packages/ui-system/shared/fixtures/identity.json', 'utf8'));
    for (const [name, fixture] of [
      ['ready', vectors.sample_image],
      ['broken', vectors.broken_image],
    ]) {
      let held;
      const imageUrl = args.base + '/fixtures/containment-' + name + '.webp';
      const onPause = (e) => {
        if (e.sessionId === page.sessionId && e.method === 'Fetch.requestPaused') held = e.params.requestId;
      };
      page.cdp.listeners.push(onPause);
      try {
        await page.send('Fetch.enable', { patterns: [{ urlPattern: imageUrl, requestStage: 'Request' }] });
        await page.eval(`(()=>{const img=new Image(256,171);img.alt='Minh họa mẫu';img.id='media-probe';
          img.onload=()=>img.dataset.result='ready';img.onerror=()=>img.dataset.result='broken';
          img.src=${JSON.stringify(imageUrl)};document.querySelector('#root-list .vk-discussion-comment').append(img);
          document.querySelector('.vk-document p').scrollIntoView()})()`);
        await page.eval(frames);
        const top = await page.eval("document.querySelector('.vk-document p').getBoundingClientRect().top");
        const deadline = Date.now() + 5000;
        while (!held && Date.now() < deadline) await new Promise((r) => setTimeout(r, 20));
        check(!!held, 'actual image request held');
        await page.send('Fetch.fulfillRequest', {
          requestId: held,
          responseCode: 200,
          responseHeaders: [{ name: 'Content-Type', value: 'image/webp' }],
          body: fixture.webp_base64,
        });
        await page.waitFor(
          `document.querySelector('#media-probe')?.dataset.result===${JSON.stringify(name)}`,
          'image decoded or rejected',
        );
        await page.eval(frames);
        record(
          'reserved media ' + name,
          await page.eval(`Math.abs(document.querySelector('.vk-document p').getBoundingClientRect().top-${top})<=2 &&
          document.querySelector('#media-probe').getBoundingClientRect().height===171`),
        );
      } finally {
        await page.send('Fetch.disable');
        page.cdp.listeners = page.cdp.listeners.filter((l) => l !== onPause);
        await page.eval("document.querySelector('#media-probe')?.remove()");
      }
    }
    for (const scale of [100, 200]) {
      await page.eval(
        `document.documentElement.style.fontSize='${scale}%';document.querySelector('.vk-document p').scrollIntoView()`,
      );
      await page.eval(frames);
      record(
        'no horizontal overflow at ' + scale,
        await page.eval('document.documentElement.scrollWidth<=innerWidth+1'),
      );
      if (scale === 200 && !large)
        await page.capture('containment-text200', {
          scenario: 'Reader article at 200%',
          boundary: 'real-public-api',
        });
    }
    await page.eval(
      "document.documentElement.style.fontSize='';document.querySelector('.vk-document p').scrollIntoView()",
    );
    const top = () => page.eval("document.querySelector('.vk-document p').getBoundingClientRect().top");
    await page.eval(frames);
    const anchor = await top();
    // The intent is paging from the article viewport. Native focus/fragment were checked above.
    await page.eval("document.querySelector('#root-pagination button').click()");
    await page.waitFor("document.querySelectorAll('.vk-discussion-root').length===40", 'second bounded page');
    await page.eval(frames);
    record('paging preserves article anchor', Math.abs((await top()) - anchor) <= 2);
    await page.eval("document.querySelector('#discussion-refresh').click()");
    await page.waitFor(
      "document.querySelectorAll('.vk-discussion-root').length===20",
      'refresh replaces the tail',
    );
    await page.eval(frames);
    record('refresh preserves article anchor', Math.abs((await top()) - anchor) <= 2);
    await page.eval("document.querySelector('#contain-probe').remove()");
    await page.eval(frames);
    record(
      'remove CSS restores A',
      await page.eval(`getComputedStyle(document.querySelector('#root-list')).contentVisibility==='visible' &&
      document.querySelectorAll('.vk-document p').length===${paragraphs}`),
    );
    // Reinstall the trial style, then replace the published control under it.
    await page.eval(
      `(()=>{const s=document.createElement('style');s.textContent=${JSON.stringify(css(intrinsic))};document.head.append(s)})()`,
    );
    let held;
    const onPause = (e) => {
      if (e.sessionId === page.sessionId && e.method === 'Fetch.requestPaused') held = e.params.requestId;
    };
    page.cdp.listeners.push(onPause);
    try {
      await page.send('Fetch.enable', {
        patterns: [{ urlPattern: args.base + '/api/v1/works/' + work + '/reader', requestStage: 'Request' }],
      });
      await page.eval(
        "const now=performance.now.bind(performance);performance.now=()=>now()+121000;window.dispatchEvent(new Event('focus'))",
      );
      const deadline = Date.now() + 5000;
      while (!held && Date.now() < deadline) await new Promise((r) => setTimeout(r, 20));
      check(!!held, 'stale live article revalidates');
      await page.send('Fetch.fulfillRequest', {
        requestId: held,
        responseCode: 200,
        responseHeaders: [
          { name: 'Content-Type', value: 'application/json' },
          { name: 'Cache-Control', value: 'no-store' },
        ],
        body: Buffer.from(JSON.stringify({ status: 'hidden', work_id: work })).toString('base64'),
      });
      await page.waitFor(
        "!!document.querySelector('[data-testid=reader-hidden]')",
        'control replaces article under containment',
      );
      record(
        'removed article and metadata absent',
        await page.eval(
          "!document.querySelector('.vk-document,link[rel=canonical]') && !document.title.includes('[Mẫu #100]')",
        ),
      );
    } finally {
      await page.send('Fetch.disable');
      page.cdp.listeners = page.cdp.listeners.filter((l) => l !== onPause);
    }
    raw.status = 'PASS';
  } catch (error) {
    raw.error = error.message;
    throw error;
  } finally {
    await page.send('Performance.disable');
    flush();
  }
}
