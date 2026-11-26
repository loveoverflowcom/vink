// #264 and #263 AC-04: real App/PG publication, three reading surfaces, shared render corpus.
import { createHash } from 'node:crypto';
import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { collectElementAccessibility, evaluateAccessibleElement } from './visual-evidence.mjs';

const corpus = JSON.parse(readFileSync(new URL('../../../services/vink-app/tests/fixtures/title-authority/render.json', import.meta.url)));
const poem = corpus.find(c => c.id === 'literary_markdown');
const security = corpus.find(c => c.id === 'network_and_html');
const source = poem.source + '\n' + security.source;
const privateTitle = 'Nhịp mưa trong nháp mới';
const privateMarker = 'Dòng chỉ thuộc revision thứ hai.';
const networkHost = 'preview-network.invalid';
const ready = "document.documentElement.dataset.hydrated === 'true'";

async function request(base, path, { method = 'GET', body, etag } = {}) {
  const headers = { Origin: base };
  if (body !== undefined) headers['Content-Type'] = 'application/json';
  if (etag) headers['If-None-Match'] = etag;
  const response = await fetch(base + path, { method, headers, body: JSON.stringify(body) });
  const text = await response.text();
  const data = text && response.headers.get('content-type')?.includes('application/json')
    ? JSON.parse(text) : null;
  return {
    status: response.status, text, data, etag: response.headers.get('etag'),
    cache: response.headers.get('cache-control'),
  };
}

function digest(title, text) {
  const hash = createHash('sha256').update('vink-document-content');
  for (const field of ['markdown', title, text]) {
    const bytes = Buffer.from(field);
    const length = Buffer.alloc(8);
    length.writeBigUInt64BE(BigInt(bytes.length));
    hash.update(length).update(bytes);
  }
  return hash.digest('hex');
}

function inspectSurface(selector, html) {
  const doc = html ? new DOMParser().parseFromString(html, 'text/html') : document;
  const host = doc.querySelector(selector);
  const root = host.querySelector('.vk-document');
  return {
    format: root.dataset.format,
    text: root.textContent,
    structure: [...root.querySelectorAll('h1,h2,h3,h4,h5,h6,blockquote,ol,ul,li,br,strong,em,code,pre,hr')]
      .map(el => [el.tagName, el.id, el.getAttribute('start')]),
    headings: [...host.querySelectorAll('h1,h2,h3,h4,h5,h6')].map(el => [el.id, el.textContent]),
    links: [...root.querySelectorAll('a')].map(a => [a.getAttribute('href'), a.textContent, a.rel]),
    alt: [...root.querySelectorAll('.vk-markdown__image')].map(el => el.textContent),
    executable: Boolean(root.querySelector('img,script,vink-inline,vink-raw,table,del,input,[src]')),
  };
}

const view = (page, selector, html = null) => page.eval(`(${inspectSurface})(${JSON.stringify(selector)}, ${JSON.stringify(html)})`);
const equal = (a, b) => JSON.stringify(a) === JSON.stringify(b);

async function matrix(page, check, textContrast, mode, selector) {
  const cells = [];
  await page.send('Page.bringToFront');
  for (const width of [320, 390, 834, 1024, 1440]) {
    await page.viewport(width, 1000);
    for (const theme of ['light', 'dark']) {
      for (const scale of [100, 200]) {
        const cell = await page.eval(`(async () => {
          const html = document.documentElement;
          html.dataset.theme = '${theme}';
          html.style.fontSize = '${scale}%';
          await document.fonts.ready;
          const region = document.querySelector(${JSON.stringify(selector)});
          const doc = region.querySelector('.vk-document');
          const title = doc.querySelector('#muc-1');
          const code = doc.querySelector('pre');
          const rect = doc.getBoundingClientRect();
          return {
            page: html.scrollWidth - html.clientWidth,
            region: doc.scrollWidth - doc.clientWidth,
            left: rect.left, right: rect.right, width: html.clientWidth,
            code: code.scrollWidth - code.clientWidth,
            codeOverflow: getComputedStyle(code).overflowX,
            title: title.textContent, visible: title.checkVisibility(),
            fontPx: parseFloat(getComputedStyle(html).fontSize),
          };
        })()`);
        cells.push({ width, theme, scale, ...cell });
        const label = `${mode}-${width}-${theme}-${scale}`;
        if (width === 320 && scale === 200) {
          await textContrast(page, theme, `markdown-${mode}`, [
            `${selector} .vk-markdown #muc-1`,
            `${selector} .vk-markdown p`,
            `${selector} .vk-markdown a`,
          ]);
        }
        if ([320, 1440].includes(width)) {
          await page.capture(`markdown-${label}`, {
            scenario: '#264', state: mode, viewport: { width, height: 1000 },
            textScale: scale, boundary: 'Chrome-CDP-root-font-size-geometry',
          });
        }
        check(cell.page <= 1 && cell.region <= 1 && cell.left >= -1 && cell.right <= cell.width + 1, `${label}: no page or reading-region spill ${JSON.stringify(cell)}`);
        check(cell.title === poem.title && cell.visible && cell.fontPx === 16 * scale / 100 && cell.codeOverflow === 'auto', `${label}: title and local code scroller survive real text scaling`);
        if (width === 320) check(cell.code > 0, `${label}: long code scrolls within its own block`);
      }
    }
  }
  await page.eval("document.documentElement.removeAttribute('data-theme'); document.documentElement.style.fontSize = ''");
  await page.viewport(1440);
  return cells;
}

export async function markdownReader(page, { args, check, textContrast }) {
  console.log('#264 Markdown reader / snapshot on App and PostgreSQL:');
  const admin = args.base;
  const author = args['author-base'];
  const reviewer = args['reviewer-base'];
  const post = (base, path, body = {}) => request(base, path, { method: 'POST', body });
  const put = (path, body) => request(author, path, { method: 'PUT', body });
  const network = [];
  const observe = event => {
    if (event.sessionId === page.sessionId && event.method === 'Network.requestWillBeSent'
        && event.params.request.url.includes(networkHost)) network.push(event.params.request.url);
  };
  page.cdp.listeners.push(observe);
  await page.send('Network.setBlockedURLs', { urls: [`*${networkHost}*`] });
  const geometry = {};
  try {
    const draft = await post(author, '/api/v1/drafts', { format: 'markdown' });
    check(draft.status === 201 && draft.data.document.format === 'markdown', 'the author creates a private Markdown draft');
    const work = draft.data.work_id;
    const draftApi = `/api/v1/drafts/${work}`;
    const workApi = `/api/v1/works/${work}`;
    const readerPath = `/tac-pham/${work}`;
    const readerAPI = workApi + '/reader';
    const save = await put(draftApi, { expected_version: draft.data.version, document: { format: 'markdown', source } });
    const stored = await request(author, draftApi);
    check(save.status === 200 && stored.status === 200 && stored.data.title === poem.title
      && stored.data.document.source === source && stored.data.version === save.data.version,
    'the saved title and source agree with the corpus byte for byte');
    const catalog = await request(author, '/api/v1/tags?facet=form');
    const firstTag = catalog.data.tags.find(t => t.label === 'Thơ');
    const nextTag = catalog.data.tags.find(t => t.label === 'Tản văn');
    check(firstTag && nextTag, 'the scratch catalog has the two form tags');
    const tags = await request(author, draftApi + '/tags');
    const firstMetadata = await put(draftApi + '/tags', { expected_version: tags.data.version, form: firstTag.tag_id, themes: [] });
    check(firstMetadata.status === 200, 'the author saves the first tag projection');

    const publicBefore = await request(admin, readerAPI, { etag: '*' });
    const draftHtml = await request(author, '/ban-viet/' + work);
    check(publicBefore.status === 404 && publicBefore.cache === 'no-store' && !publicBefore.etag, 'a private Markdown work has no public validator');
    check(!draftHtml.text.includes(poem.title) && !draftHtml.text.includes(networkHost), 'the private draft is absent from SSR and hydration strings');
    await page.goto(author + '/ban-viet/' + work);
    await page.waitFor(`${ready} && document.querySelector('.cm-content')`, 'real Markdown editor');
    check(!draftHtml.text.includes('id="draft-preview"') && !draftHtml.text.includes('id="preview"')
      && await page.eval("!document.querySelector('#preview, #draft-preview') && document.querySelector('.cm-content').textContent.includes('Lời đề')"),
    'SSR and hydrated Markdown retain the live editor without a standalone preview');

    const capture = (version, metadata, name) => post(author, workApi + '/snapshots', {
      command_key: name, expected_draft_version: version, expected_metadata_version: metadata,
    });
    const submit = (revision, generation, key) => post(author, workApi + '/publication/request-public', {
      command_key: key, revision_id: revision, expected_generation: generation,
    });
    const r1 = await capture(save.data.version, firstMetadata.data.version, 'markdown-reader-r1');
    check(r1.status === 201 && r1.data.content_hash === digest(poem.title, source), 'snapshot r1 binds the canonical format, title and exact source');
    const start = `# ${privateTitle}\n\n${privateMarker}\n\n`;
    const end = '\n\n## Hết bản dài\n';
    const changedSource = start + 'Việt viết văn. '.repeat(15_000).slice(0, 200_000 - [...(start + end)].length) + end;
    check([...changedSource].length === 200_000, 'the subsequent revision reaches 200,000 authored scalars');
    const edited = await put(draftApi, {
      expected_version: save.data.version, document: { format: 'markdown', source: changedSource },
    });
    const changedTags = await put(draftApi + '/tags', { expected_version: firstMetadata.data.version, form: nextTag.tag_id, themes: [] });
    const newerDraft = await request(author, draftApi);
    check(edited.status === 200 && newerDraft.data.title === privateTitle && changedTags.status === 200,
      'a later save changes the draft H1 and draft tag only');

    const automation = await request(admin, '/api/v1/moderation/automation');
    if (automation.data.mode !== 'paused') {
      const paused = await post(admin, '/api/v1/moderation/automation/mode', { expected_version: automation.data.version, mode: 'paused', reason: '#264 real human queue' });
      check(paused.status === 200, 'the admin pauses automation for the human review fixture');
    }
    const initial = await request(author, workApi + '/publication/state');
    const sent = await submit(r1.data.revision_id, initial.data.generation, 'markdown-reader-submit-r1');
    check(sent.status === 200 && (await request(admin, readerAPI)).status === 404, 'request-public admits Markdown to review and keeps the body private');
    const reviewCase = async revision => {
      const queue = await request(admin, '/api/v1/moderation/works/queue');
      return queue.data.find(c => c.work_id === work && c.revision_id === revision);
    };
    const queued = await reviewCase(r1.data.revision_id);
    check(queued?.origin === 'automation_paused', 'the exact Markdown snapshot goes to the human queue');
    const caseAPI = '/api/v1/moderation/works/cases/' + queued.id;
    const detail = await request(admin, caseAPI);
    check(detail.status === 200 && detail.cache === 'no-store' && detail.data.title === poem.title && detail.data.document.source === source, 'staff sees captured r1 with no-store');
    const firstClaim = await post(admin, caseAPI + '/claim', { expected_version: detail.data.version });
    check(firstClaim.status === 200, 'the admin claims the Markdown snapshot');
    const firstApproval = await post(admin, caseAPI + '/conclude', { expected_version: firstClaim.data.version, verdict: 'approved', reason: 'Duyệt đúng bản đã chụp.' });
    check(firstApproval.status === 200, 'the admin approves the exact revision through the API');

    const live = await request(reviewer, readerAPI);
    check(live.status === 200 && live.data.revision_id === r1.data.revision_id
      && live.data.title === poem.title && live.data.document.source === source
      && equal(live.data.tags.map(t => t.tag_id), [firstTag.tag_id]),
    'public Reader uses r1 title, source and captured tag, ignoring newer draft metadata');
    const readTag = '"' + createHash('sha256').update(live.text).digest('hex').slice(0, 32) + '"';
    check(live.etag === readTag && live.cache === 'private, no-cache'
      && (await request(reviewer, readerAPI, { etag: live.etag })).status === 304, 'unchanged JSON validates the bytes with a strong ETag and 304');
    const ssr = await request(reviewer, readerPath, { etag: live.etag });
    check(ssr.status === 200 && !ssr.etag && !ssr.text.includes(privateMarker), 'HTML reads public r1 afresh and has no JSON ETag or private revision');
    await page.goto(reviewer + readerPath);
    await page.waitFor(`${ready} && document.querySelector('#reader-paper .vk-markdown')`, 'hydrated real Markdown Reader');
    const beforeWasm = await view(page, '#reader-paper', ssr.text);
    const afterWasm = await view(page, '#reader-paper');
    check(afterWasm.format === 'markdown' && afterWasm.headings[0][0] === 'muc-1'
      && afterWasm.headings.filter(([, text]) => text === poem.title).length === 1
      && afterWasm.text.indexOf('Lời đề') < afterWasm.text.indexOf(poem.title), 'Reader shows the canonical H1 once and at its source position');
    check(!afterWasm.executable && equal(afterWasm.alt, ['Ảnh bên ngoài', 'Ảnh'])
      && equal(afterWasm.links.map(([href]) => href), ['#muc-2', `https://${networkHost}/link`, '/', '#muc-1'])
      && afterWasm.links.every(([, text, rel]) => text && rel.includes('noreferrer'))
      && afterWasm.text.includes('<vink-inline') && afterWasm.text.includes('<img src='),
    'only named safe links remain; raw HTML and image alt are inert');
    check(afterWasm.structure.some(([tag, , start]) => tag === 'OL' && start === '3')
      && afterWasm.structure.some(([tag]) => tag === 'BR') && afterWasm.text.includes('Dòng đầu ❤️‍🔥\ndòng tiếp.')
      && afterWasm.text.includes('| --- | --- |') && afterWasm.text.includes('[ ] Việc vẫn bằng chữ'),
    'CommonMark structure, line breaks and unsupported extensions match the shared corpus');

    check(equal(beforeWasm, afterWasm), 'SSR and hydrated Reader have identical projections');
    const expectedTitle = JSON.stringify(poem.title + ' — Vink');
    check(await page.eval(`new DOMParser().parseFromString(${JSON.stringify(ssr.text)}, 'text/html').title === ${expectedTitle}
      && document.title === ${expectedTitle}
      && document.querySelector('meta[property="og:title"]').content === ${JSON.stringify(poem.title)}
      && document.querySelector('meta[name="description"]').content === ${JSON.stringify(poem.description)}
      && document.querySelector('link[rel="canonical"]').href === ${JSON.stringify(reviewer + readerPath)}
      && document.querySelector('#reader-paper').dataset.revision === ${JSON.stringify(r1.data.revision_id)}`),
    'SSR and hydration expose the title, plain description, canonical URL and the approved revision');
    geometry.reader = await matrix(page, check, textContrast, 'reader', '#reader-paper');
    const sectionLink = '#reader-paper a[href="#muc-2"]';
    const ax = await collectElementAccessibility(page, sectionLink);
    const accessible = evaluateAccessibleElement(ax, { role: 'link', name: 'về mục' });
    check(accessible.ok, `a safe Markdown link has the expected Chrome AX role and name ${accessible.reasons.join('; ')}`);
    await page.key('Tab');
    await page.eval(`document.querySelector(${JSON.stringify(sectionLink)}).focus()`);
    check(await page.eval("document.activeElement.matches(':focus-visible') && getComputedStyle(document.activeElement).outlineStyle === 'solid'"), 'keyboard focus is visible on a named document link');
    await page.key('Enter');
    await page.waitFor("location.hash === '#muc-2' && document.querySelector('#muc-2')", 'fragment link lands at the projected heading');
    check(!network.length, 'no external manuscript request across editor and published Reader');

    await page.goto(reviewer + '/?q=' + encodeURIComponent('mua tren bac cua'));
    const exploreLink = `a.vk-public-row__link[href^="${readerPath}"]`;
    await page.waitFor(`${ready} && document.querySelector(${JSON.stringify(exploreLink)})`, 'public title search at the stable work URL');
    check(await page.eval(`document.querySelector(${JSON.stringify(exploreLink)}).textContent.includes(${JSON.stringify(poem.title)})`), 'Khám phá uses the captured title');
    const draftOnlySearch = await request(reviewer, '/api/v1/works?q=' + encodeURIComponent(privateTitle));
    check(!draftOnlySearch.data.works.some(card => card.work_id === work), 'public search cannot match a draft H1');

    await page.goto(author + '/ban-viet');
    await page.waitFor(`${ready} && document.querySelector('#draft-search')`, 'Desk search');
    await page.click('#open-draft-search');
    await page.typeAtEnd('#draft-search', privateTitle);
    await page.click('.vk-draft-search button[type=submit]');
    const deskRow = `[data-testid="draft-row"] a[href="/ban-viet/${work}"]`;
    await page.waitFor(`document.querySelector(${JSON.stringify(deskRow)})?.textContent.includes(${JSON.stringify(privateTitle)})`, 'Desk search shows the later draft title');

    const r2 = await capture(edited.data.version, changedTags.data.version, 'markdown-reader-r2');
    check(r2.status === 201 && r2.data.content_hash === digest(privateTitle, changedSource), 'new title and tag bind to a new immutable r2');
    const publicState = await request(author, workApi + '/publication/state');
    const resubmitted = await submit(r2.data.revision_id, publicState.data.generation, 'markdown-reader-submit-r2');
    check(resubmitted.status === 200 && (await request(reviewer, readerAPI, { etag: live.etag })).status === 304, 'r1 remains current while r2 waits for a decision');
    const next = await reviewCase(r2.data.revision_id);
    const nextAPI = '/api/v1/moderation/works/cases/' + next.id;
    const claim = await post(admin, nextAPI + '/claim', { expected_version: next.version });
    const approved = await post(admin, nextAPI + '/conclude', { expected_version: claim.data.version, verdict: 'approved', reason: '#264 r2 review' });
    check(approved.status === 200, 'r2 gets its own verdict');
    const changed = await request(reviewer, readerAPI, { etag: live.etag });
    check(changed.status === 200 && changed.etag !== live.etag && changed.data.work_id === work
      && changed.data.revision_id === r2.data.revision_id && changed.data.title === privateTitle
      && changed.data.document.source === changedSource && changed.data.tags[0].tag_id === nextTag.tag_id,
    'a new public pointer changes JSON and keeps the stable URL');
    await page.goto(reviewer + readerPath);
    await page.waitFor(`${ready} && document.querySelector('#muc-2')?.textContent === 'Hết bản dài'`, 'the full 200k Markdown revision hydrates');
    const beforeHide = await request(author, workApi + '/publication/state');
    const hidden = await post(author, workApi + '/publication/set-private', {
      expected_generation: beforeHide.data.generation, command_key: 'markdown-reader-hide',
    });
    check(hidden.status === 200, 'the author moves the Markdown work to private');
    const notice = await request(reviewer, readerAPI, { etag: changed.etag });
    const hiddenHtml = await request(reviewer, readerPath, { etag: changed.etag });
    check(notice.data.status === 'hidden' && notice.cache === 'no-store' && !notice.etag
      && hiddenHtml.cache === 'no-store' && !hiddenHtml.etag && !hiddenHtml.text.includes(privateMarker)
      && (await request(reviewer, workApi + '/publication', { etag: changed.etag })).status === 404,
    'old JSON validators never bypass the hidden projection or expose either revision');
    await page.reload();
    await page.waitFor("document.querySelector('[data-testid=reader-hidden]') && !document.querySelector('.vk-document, link[rel=canonical]')", 'the hydrated hidden page has no Markdown body');

    const restored = await submit(r1.data.revision_id, hidden.data.generation, 'markdown-reader-restore');
    const beforeReject = await request(reviewer, readerAPI);
    check(restored.status === 200 && beforeReject.data.revision_id === r1.data.revision_id,
      'the author can request the previously approved revision again by its exact ID');
    const grant = await post(admin, '/api/v1/moderation/grants', {
      grantee_kind: 'principal', grantee_id: '77777777-7777-4777-8777-777777777777',
      capability: 'works', scope_kind: 'work', scope_work_id: work, expires_at: null,
    });
    check(grant.status === 201, 'the admin grants the independent reviewer authority on the Markdown work');
    const reviewPath = `/api/v1/moderation/works/${work}/revisions/${r1.data.revision_id}/cases`;
    const intervention = await post(reviewer, reviewPath);
    check(intervention.status === 201, 'the reviewer opens a case on the public Markdown revision');
    const interventionAPI = '/api/v1/moderation/works/cases/' + intervention.data.case_id;
    const held = await post(reviewer, interventionAPI + '/claim', { expected_version: 1 });
    const denied = await post(reviewer, interventionAPI + '/conclude', {
      expected_version: held.data.version, verdict: 'rejected', reason: '#264 review withdrawal',
    });
    const withdrawn = await request(reviewer, readerAPI, { etag: beforeReject.etag });
    check(denied.status === 200 && withdrawn.data.status === 'review_withdrawn'
      && withdrawn.cache === 'no-store' && !withdrawn.etag && !withdrawn.data.document,
    'a real Markdown rejection beats the old validator and exposes only the withdrawal notice');
  } finally {
    await page.send('Network.setBlockedURLs', { urls: [] });
    const index = page.cdp.listeners.indexOf(observe);
    if (index >= 0) page.cdp.listeners.splice(index, 1);
    writeFileSync(join(dirname(args.state), 'markdown-reader-matrix.json'), JSON.stringify({ geometry, network }, null, 2));
  }
}
