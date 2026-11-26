import { deepStrictEqual, rejects, strictEqual } from 'node:assert';
import { EDITOR_READY, HYDRATED, account } from './auth-smoke.mjs';
import { LITERARY_CORPUS, MARKDOWN, PROSE, VERSE } from './fixtures/literary-review.mjs';

const BOUNDARY = 'Product SSR/hydrate · Identity email-code · scratch PostgreSQL';
const DESK_READY = `${HYDRATED} && document.querySelector('[data-testid=draft-content][aria-busy=false]')`;
const CM = "document.querySelector('.cm-content')?.cmTile?.root?.view";
const OUTLINE = '#editor-outline-panel';
const DESKTOP = { width: 1440, height: 900, theme: 'light', scale: 100 };
const MOBILE = { width: 390, height: 844, theme: 'dark', scale: 100 };
const row = id => `#draft-row-${id}`;
const rowLink = id => `${row(id)} .vk-draft-row__link`;

async function setScene(page, { width, height, theme, scale }) {
  await page.viewport(width, height);
  await page.eval(`Object.assign(document.documentElement.dataset, {
    theme: ${JSON.stringify(theme)}, text: ${JSON.stringify(String(scale))}
  }); document.fonts.ready.then(() => new Promise(resolve =>
    requestAnimationFrame(() => requestAnimationFrame(resolve))))`);
}

function capture(page, scenario, state, scene, targets, fixture, runtime) {
  const { width, height, theme, scale } = scene;
  return page.capture(`literary-${scenario}-${width}x${height}-${theme}-${scale}`, {
    scenario, state, boundary: BOUNDARY, fixture: `${LITERARY_CORPUS}/${fixture}`,
    requestedScene: scene, requiredTargets: targets, runtime,
  });
}

async function authorWorks(page, call) {
  const result = await call(page, '/api/v1/author/works?limit=24');
  strictEqual(result.status, 200, 'the authenticated Desk list read succeeds');
  strictEqual(result.body.next_cursor, null, 'the scratch list fits in its first page');
  return new Map(result.body.works.map(work => [work.work_id, work]));
}

export async function literaryFirstUse(page, { base, call, check }) {
  await page.goto(base + '/ban-viet');
  await page.waitFor(`${DESK_READY} && document.querySelector('[data-testid=draft-empty] #create-draft:enabled')`,
    'freshly registered author sees first-use after the real Desk read');
  const works = await authorWorks(page, call);
  check(works.size === 0 && await page.eval("document.querySelectorAll('[data-testid=draft-row]').length === 0"),
    '#556 real first-use has an empty author works head');
  await setScene(page, DESKTOP);
  await capture(page, 'desk-first-use', 'fresh authenticated author, zero works', DESKTOP,
    ['[data-testid=draft-empty]', '#create-draft'], 'first-use', { works: works.size });
}

async function createPrivateDraft(page, call, fixture) {
  const { format, source, title } = fixture;
  const created = await call(page, '/api/v1/drafts', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ format }),
  });
  strictEqual(created.status, 201, `${fixture.id} draft created through App`);
  const id = created.body.work_id;
  const saved = await call(page, `/api/v1/drafts/${id}`, {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      expected_version: 1,
      document: { format, source },
      ...(format === 'default' ? { title } : {}),
    }),
  });
  strictEqual(saved.status, 200, `${fixture.id} draft saved through App`);
  strictEqual(saved.body.version, 2, `${fixture.id} draft version advanced`);
  return id;
}

async function verifyDesk(page, base, call, fixtures, published, pending) {
  await page.goto(base + '/ban-viet');
  await page.waitFor(DESK_READY, 'the authenticated Desk finished loading');
  const head = await authorWorks(page, call);
  for (const [id, fixture] of fixtures) {
    const work = head.get(id);
    strictEqual(work?.title, fixture.title, `${fixture.id} title in the private Desk head`);
    strictEqual(work?.draft_version, 2, `${fixture.id} version in the private Desk head`);
    strictEqual(work?.publication?.audience, 'private', `${fixture.id} is private`);
  }
  strictEqual(head.get(published)?.publication?.current_public?.revision_id != null, true, 'public pointer is present in Desk head');
  strictEqual(head.get(pending)?.publication?.intent_moderation, 'pending', 'another Desk item is pending review');
  await page.waitFor(`[${[...fixtures.keys(), published, pending].map(id => JSON.stringify(row(id))).join(',')}]
    .every(selector => document.querySelector(selector)?.checkVisibility()) &&
    !document.querySelector('[data-testid=draft-empty], [data-testid=desk-error], [data-testid=author-work-stale]')`,
  'the expected real Desk rows are visible and the list is current');
  for (const [id, fixture] of fixtures) {
    const title = await page.eval(`document.querySelector('${row(id)} .vk-draft-row__title')?.textContent.trim()`);
    strictEqual(title, fixture.title, `${fixture.id} title in its rendered Desk row`);
    strictEqual(await page.eval(`document.querySelector('${rowLink(id)}')?.getAttribute('href')`),
      `/ban-viet/${id}`, `${fixture.id} row opens its own saved editor`);
  }
  for (const [id, label] of [[published, 'Đã đăng'], [pending, 'Bản mới chờ duyệt']]) {
    strictEqual(await page.eval(`document.querySelector('${row(id)} .vk-author-work__status')?.textContent.includes(${JSON.stringify(label)})`),
      true, `Desk renders the observed ${label} state`);
  }
}

async function assertEditor(page, call, fixtures, id, fixture) {
  strictEqual(fixtures.get(id)?.id, fixture.id, 'literary fixture is missing from the verified Desk head');
  const draft = await call(page, `/api/v1/drafts/${id}`);
  strictEqual(draft.status, 200, 'the expected scratch draft is readable');
  strictEqual(draft.body.version, 2, 'editor draft is the saved fixture version');
  strictEqual(draft.body.title, fixture.title, 'saved fixture title');
  deepStrictEqual(draft.body.document, { format: fixture.format, source: fixture.source }, 'saved fixture document mismatch');
  strictEqual(await page.eval('location.pathname'), `/ban-viet/${id}`, 'editor is on the fixture work route');
  const actual = await page.eval(`({
    format: document.querySelector('.vk-editor')?.dataset.format,
    source: ${CM}?.state.doc.toString(),
    editorCount: document.querySelectorAll('.cm-editor').length,
    visible: ${EDITOR_READY}?.checkVisibility()
  })`);
  deepStrictEqual(actual, { format: fixture.format, source: fixture.source, editorCount: 1, visible: true },
    'one visible editor must render the exact saved fixture');
  if (!fixture.headings) {
    strictEqual(await page.eval(`Boolean(document.querySelector('#outline-toggle, ${OUTLINE}'))`), false,
      'Default text has no Markdown outline');
    return;
  }
  await page.waitFor(`document.querySelector('${OUTLINE} [data-heading-from="${fixture.headings.at(-1)[2]}"]') &&
    !document.querySelector('${OUTLINE} .vk-editor-outline__empty')`, 'the Markdown AST is complete');
  const headings = await page.eval(`[...document.querySelectorAll('${OUTLINE} [data-heading-from]')]
    .map(el => [el.textContent, Number(el.dataset.headingLevel), Number(el.dataset.headingFrom)])`);
  deepStrictEqual(headings, fixture.headings, 'exact Markdown AST, excluding the fenced false heading');
  strictEqual(await page.eval(`document.querySelector('${OUTLINE}[open]')?.checkVisibility()`), true,
    'the completed outline is visible');
}

async function captureEditor(page, call, fixtures, id, fixture, scene) {
  await assertEditor(page, call, fixtures, id, fixture);
  const targets = fixture.headings ? ['#editor-outline-title', `${OUTLINE} [data-heading-from="0"]`] : ['#draft-title', '.cm-content'];
  const kind = fixture.headings ? 'saved Markdown reopened with full outline' : 'saved Default draft reopened';
  return capture(page, `editor-${fixture.id}`, kind, scene, targets, fixture.id,
    { version: 2, format: fixture.format, headings: fixture.headings ?? [], deskRow: true });
}

export async function seedLiteraryDrafts(page, call) {
  const fixtures = new Map();
  for (const fixture of [PROSE, VERSE, MARKDOWN]) {
    fixtures.set(await createPrivateDraft(page, call, fixture), fixture);
  }
  return fixtures;
}

export async function literaryPopulated(page, { base, call, check, fixtures, published, pending, principal }) {
  await page.send('Page.bringToFront');
  strictEqual((await account(page)).body?.principal_id, principal, 'the corpus uses the first-use author');
  await verifyDesk(page, base, call, fixtures, published, pending);
  for (const scene of [DESKTOP, MOBILE]) {
    await setScene(page, scene);
    await capture(page, 'desk-populated', 'private drafts, a pending work and a public work', scene,
      [...fixtures.keys(), published, pending].map(row), 'desk-populated',
      { privateDrafts: [...fixtures.values()].map(fixture => fixture.id), pending: true, published: true });
  }

  for (const [id, fixture] of fixtures) {
    await page.click(rowLink(id));
    await page.waitFor(`location.pathname === '/ban-viet/${id}' && ${EDITOR_READY}`, 'Desk row opens the saved editor');
    await page.reload();
    await page.waitFor(EDITOR_READY, 'saved editor reopens after reload');
    const firstScene = fixture === VERSE ? MOBILE : DESKTOP;
    await setScene(page, firstScene);
    if (fixture === MARKDOWN) {
      await page.click('#outline-toggle');
      await captureEditor(page, call, fixtures, id, fixture, DESKTOP);
      await rejects(captureEditor(page, call, fixtures, id, { ...fixture, source: '# Nội dung sai\n' }, DESKTOP),
        /saved fixture document mismatch/);
      await rejects(captureEditor(page, call, fixtures, published, fixture, DESKTOP),
        /literary fixture is missing from the verified Desk head/);
      check(await page.eval(`Boolean(${EDITOR_READY})`), '#556 wrong and missing fixture stop before capture even with CodeMirror mounted');

      const [, , duplicateFrom] = fixture.headings[3];
      const duplicateEnd = fixture.source.indexOf('\n', duplicateFrom);
      const duplicate = `${OUTLINE} button:has([data-heading-from="${duplicateFrom}"])`;
      const active = `document.querySelector('${duplicate}')?.closest('[aria-current="location"]')?.getAttribute('aria-current')`;
      const caret = `${CM}.state.selection.main.head`;
      await page.click(duplicate);
      try {
        await page.waitFor(`${active} === 'location' && ${caret} === ${duplicateEnd}`,
          'desktop jumps to the second duplicate and marks it active');
      } catch (cause) {
        const state = await page.eval(`({ active: ${active}, caret: ${caret}, modal: Boolean(document.querySelector('${OUTLINE}:modal')) })`);
        throw new Error(`outline jump from ${duplicateFrom}, expected caret ${duplicateEnd}: ${JSON.stringify(state)}`, { cause });
      }
      for (const scene of [
        { ...DESKTOP, theme: 'dark' }, { ...MOBILE, theme: 'light' }, MOBILE,
        { ...MOBILE, scale: 150 }, { ...MOBILE, width: 320, height: 800, scale: 200 },
      ]) {
        await setScene(page, scene);
        await page.waitFor(`document.querySelector('${OUTLINE}${scene.width < 760 ? ':modal' : ':not(:modal)'}')?.open`,
          'outline surface follows viewport');
        await captureEditor(page, call, fixtures, id, fixture, scene);
      }
      await page.click(duplicate);
      await page.waitFor(`!document.querySelector('${OUTLINE}') &&
        document.activeElement === ${CM}.contentDOM && ${caret} === ${duplicateEnd}`,
      'mobile sheet closes and focuses the second duplicate in the editor');
    } else {
      await captureEditor(page, call, fixtures, id, fixture, firstScene);
    }
    await page.goto(base + '/ban-viet');
    await page.waitFor(`${DESK_READY} && document.querySelector('${rowLink(id)}')`, 'return to the populated Desk');
  }
  await page.send('Emulation.clearDeviceMetricsOverride');
  console.log('  ok  #556 all three corpus drafts reopen with exact source and the expected outline');
}
