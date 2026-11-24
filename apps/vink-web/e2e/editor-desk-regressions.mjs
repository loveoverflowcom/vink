import { checkSelectGeometry, selectCharacter } from '../../../packages/ui-system/leptos/preview/e2e/select-geometry.mjs';

const CM = "document.querySelector('.cm-content')?.cmTile?.root?.view";
const SAVE = '#save';

export async function firstDraftFormat(page, { args, check }) {
  const format = args['first-format'];
  const picker = '#create-draft-format';
  await page.viewport(320, 900);
  await page.goto(args.base + '/ban-viet');
  await page.waitFor(`document.querySelector('[data-testid=draft-empty] ${picker}:enabled')`, 'real zero-draft account');
  check(await page.eval(`document.querySelectorAll('${picker}').length === 1 &&
    document.querySelectorAll('#create-draft').length === 1 && document.querySelector('${picker}').value === 'default'`),
  '#262 first-use owns one format picker and one CTA, initially Mặc định');
  await page.eval(`document.documentElement.dataset.text = '200'; document.documentElement.dataset.theme = '${format === 'markdown' ? 'dark' : 'light'}'`);
  await page.eval("document.querySelector('#desk-title').focus()");
  await page.key('Tab');
  await page.waitFor(`document.activeElement === document.querySelector('${picker}')`, 'first-use Select focused by Tab');
  const focus = await page.eval(`(() => {
    const style = getComputedStyle(document.activeElement);
    return { width: parseFloat(style.outlineWidth), style: style.outlineStyle, color: style.outlineColor };
  })()`);
  check(focus.width > 0 && focus.style !== 'none' && focus.color !== 'rgba(0, 0, 0, 0)',
    `#262 first-use Select shows its keyboard focus ring (${JSON.stringify(focus)})`);
  if (format === 'markdown') {
    await selectCharacter(page, 'm');
    await page.waitFor(`document.querySelector('${picker}').value === 'markdown'`, 'native Markdown option selected by keyboard');
  }
  check(await page.eval(`(() => {
    const radius = getComputedStyle(document.querySelector('#create-draft')).getPropertyValue('--vk-radius-control-48').trim();
    return ['#create-draft-format', '#create-draft'].every(id => {
      const style = getComputedStyle(document.querySelector(id));
      return ['borderTopLeftRadius','borderTopRightRadius','borderBottomLeftRadius','borderBottomRightRadius']
        .every(corner => style[corner] === radius);
    });
  })()`), '#409 native format Select and create use the symmetric R4 control radius');
  await checkSelectGeometry(page, picker, check, '#262 first draft at 320px / text200', { icon: 16, inset: 16, reserve: 52, gap: 20 });
  await page.key('Tab');
  check(await page.eval(`(() => {
    const cta = document.querySelector('#create-draft'), r = cta.getBoundingClientRect();
    const bar = document.querySelector('.vk-ui-bottom-nav');
    const bottom = bar?.getClientRects().length ? bar.getBoundingClientRect().top : innerHeight;
    return document.activeElement === cta && r.width >= 44 && r.height >= 44 && r.left >= 0 &&
      r.right <= innerWidth + 1 && r.top >= 0 && r.bottom <= bottom + 1;
  })()`), '#262 keyboard reaches the complete first-use CTA above navigation at 320px / text200');
  await page.capture(`first-draft-${format}-320-200`, {
    scenario: 'first-draft-format', requiredTargets: [picker, '#create-draft'], boundary: 'Chrome CDP, fresh PostgreSQL fixture principal',
  });
  await page.key('Enter');
  await page.waitFor(CM, 'first draft opens through the hydrated CTA');
  const id = await page.eval("location.pathname.split('/').pop()");
  const created = await page.draft(id);
  check(created.version === 1 && created.title === '' && created.document.format === format && created.document.source === '',
    `#262 actual POST created one empty ${format} draft`);
  await page.reload();
  await page.waitFor(CM, 'first draft reloads from PostgreSQL');
  check(await page.eval(`document.querySelector('.vk-editor').dataset.format === '${format}' &&
    (document.querySelector('#draft-title') !== null) === ${format === 'default'} &&
    (document.querySelector('#editor-markdown-toolbar') !== null) === ${format === 'markdown'} &&
    document.querySelectorAll('.cm-editor').length === 1`), `#262 reload opens the stored ${format} editor and controls`);
}

async function write(page, path, method, body) {
  return page.eval(`fetch(${JSON.stringify(path)}, {
    method: ${JSON.stringify(method)}, headers: {'Content-Type': 'application/json'},
    body: ${JSON.stringify(JSON.stringify(body))}
  }).then(async r => ({status: r.status, body: await r.json()}))`);
}

async function openDraft(page, base, check, document, title) {
  const created = await write(page, '/api/v1/drafts', 'POST', { format: document.format });
  check(created.status === 201, `scratch ${document.format} fixture created through the API`);
  const id = created.body.work_id;
  const saved = await write(page, '/api/v1/drafts/' + id, 'PUT', { expected_version: 1, document, title });
  check(saved.status === 200 && saved.body.version === 2, 'scratch source persisted before opening the editor');
  await page.goto(`${base}/ban-viet/${id}`);
  await page.waitFor(CM, 'saved source opens on the writing desk');
  await page.eval(`window.regressionEditor = ${CM}; true`);
  return id;
}

const SAME_VIEW = `${CM} === window.regressionEditor && document.querySelectorAll('.cm-editor').length === 1`;
const SOURCE = `${CM}.state.doc.toString()`;
const twoFrames = page => page.eval('new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)))');

async function replaceSource(page, source, anchor = 0) {
  await page.eval(`(() => {
    const view = ${CM};
    view.dispatch({changes: {from: 0, to: view.state.doc.length, insert: ${JSON.stringify(source)}}, selection: {anchor: ${anchor}}});
  })()`);
}

async function selectSource(page, anchor, head = anchor) {
  await page.eval(`(() => {
    const view = ${CM};
    view.focus();
    view.dispatch({selection: {anchor: ${anchor}, head: ${head}}, scrollIntoView: true});
  })()`);
}

async function save(page) {
  await page.click(SAVE);
  await page.waitStatus('saved');
}

async function markdownLinks(page, { args, check }) {
  const multiline = '[Thơ](\n/tho\n)';
  const source = '# Tên\n\n' + multiline + '\n\n[Vink](/)\n\nMột đoạn văn.\n';
  const id = await openDraft(page, args.base, check, { format: 'markdown', source });
  const shownLabels = "Array.from(document.querySelectorAll('.cm-writing-link'), el => el.textContent).join('|')";
  const lineContent = "[...document.querySelectorAll('.cm-line')].map(el => el.textContent).join('\\n')";
  await page.waitFor(`${shownLabels} === 'Thơ|Vink' && !${lineContent}.includes('/tho')`, 'links have formatted labels with caret outside');
  const link = source.indexOf(multiline);
  await selectSource(page, link + 4, link + 1);
  await page.waitFor(`${lineContent}.includes('/tho')`, 'selection inside the link reveals its URL');
  check(await page.eval(`${CM}.state.selection.main.anchor === ${link + 4} &&
    ${CM}.state.selection.main.head === ${link + 1} && ${SOURCE} === ${JSON.stringify(source)}`),
  '#256 rearward selection survives revealing multiline syntax without changing the source');
  const normal = source.indexOf('[Vink]');
  await selectSource(page, normal + 3);
  await page.waitFor(`${lineContent}.includes('[Vink](/)') && !${lineContent}.includes('/tho')`, 'standard link reveals independently');
  await selectSource(page, source.length);
  const addition = '\n' + multiline;
  const pasted = source + addition;
  await page.eval(`(() => {
    const data = new DataTransfer();
    data.setData('text/plain', ${JSON.stringify(addition)});
    ${CM}.contentDOM.dispatchEvent(new ClipboardEvent('paste', {bubbles: true, cancelable: true, clipboardData: data}));
  })()`);
  await page.waitFor(`${SOURCE} === ${JSON.stringify(pasted)}`, 'CodeMirror paste handler preserves literal line breaks');
  await selectSource(page, 0);
  await page.waitFor(`${shownLabels} === 'Thơ|Vink|Thơ' && !${lineContent}.includes('/tho')`, 'pasted link formats after leaving its range');
  await page.eval('new Promise(r => setTimeout(r, 550))');
  await selectSource(page, pasted.lastIndexOf('Thơ') + 3);
  await page.send('Input.imeSetComposition', { text: 'd', selectionStart: 1, selectionEnd: 1 });
  await page.send('Input.imeSetComposition', { text: 'đ', selectionStart: 1, selectionEnd: 1 });
  await page.waitFor(`${CM}.composing`, 'CDP composition active inside multiline link');
  await page.key('s', page.mod);
  check((await page.draft(id)).version === 2, '#256 save waits for the active composition inside a link');
  await page.send('Input.insertText', { text: ' dấu' });
  await page.waitFor(`!${CM}.composing && ${SOURCE}.includes('[Thơ dấu](')`, 'CDP commit keeps Vietnamese text in the link');
  await twoFrames(page);
  await page.key('z', page.mod);
  await page.waitFor(`${SOURCE} === ${JSON.stringify(pasted)}`, 'Undo restores the pre-composition link byte for byte');
  await page.key('z', page.mod);
  await page.waitFor(`${SOURCE} === ${JSON.stringify(source)}`, 'Undo restores the pre-paste draft byte for byte');
  check(await page.eval(SAME_VIEW), '#256 reopen, reveal, paste and undo kept a single live EditorView for this activation');
  await save(page);
  check((await page.draft(id)).document.source === source, '#256 persisted bytes still contain the multiline URL and both line breaks');
}

async function expectMissingSnapshotTitle(page, check, id, version) {
  const tags = await page.eval(`fetch('/api/v1/drafts/${id}/tags').then(r => r.json())`);
  const { status, body } = await write(page, `/api/v1/works/${id}/snapshots`, 'POST', {
    command_key: 'title-refusal-' + version, expected_draft_version: version, expected_metadata_version: tags.version,
  });
  check(status === 422 && body.error?.field === 'title' && body.error.message === 'a revision needs a title',
    '#263 snapshot rejects the saved empty authoritative title with MissingTitle');
}

async function titleValidation(page, { args, check }) {
  const notes = 'Một đoạn văn có thân bài.';
  const source = 'Mở đầu trước tiêu đề.\n\n# Tên hợp lệ\n\n' + notes;
  const id = await openDraft(page, args.base, check, { format: 'markdown', source });
  const bad = source.replace('Tên hợp lệ', 'ế'.repeat(201));
  await replaceSource(page, bad);
  await page.eval("document.querySelector('#save').focus()");
  await page.key('Enter');
  await page.waitFor("document.querySelector('#editor-body-title-error')?.checkVisibility()", '#263 visible H1 error beside the body');
  const shown = await page.draft(id);
  check(shown.version === 2 && shown.document.source === source &&
    await page.eval(`${SAME_VIEW} && ${SOURCE} === ${JSON.stringify(bad)} &&
      document.querySelector('.cm-content').getAttribute('aria-describedby').split(' ').includes('editor-body-title-error') &&
      document.querySelector('#editor-body-title-error').textContent.includes('200')`),
  '#263 long H1 refusal preserves the typed buffer, saved source and version, with linked field guidance');
  await page.waitFor("document.activeElement?.id === 'save-recovery-jump'", 'focused save action hands focus to the title recovery action');
  await page.key('Enter');
  await page.waitFor(`document.activeElement === ${CM}.contentDOM &&
    ${CM}.state.selection.main.head === ${bad.indexOf('\n\n', bad.indexOf('# '))}`, 'title recovery puts the caret on the first H1');
  await page.capture('markdown-title-error', { target: '#editor-body-title-error', boundary: 'Chrome CDP, actual App 422 on PostgreSQL scratch' });
  const writes = [];
  const observe = event => {
    const request = event.params?.request;
    if (event.sessionId === page.sessionId && event.method === 'Network.requestWillBeSent' && request.method === 'PUT') {
      writes.push(new URL(request.url).pathname);
    }
  };
  page.cdp.listeners.push(observe);
  try {
    await page.key('s', page.mod);
    await page.eval(`const options = document.querySelector('#work-options'); if (!options.open) options.querySelector('summary').click();
      const select = document.querySelector('#work-tags-form-select');
      select.value = select.options[1].value;
      select.dispatchEvent(new Event('change', { bubbles: true }));`);
    await page.waitFor("document.querySelector('#save')?.textContent.includes('Lưu thẻ')", 'the eligible save action names the tags');
    await page.click(SAVE);
    await page.waitFor("document.querySelector('#save-recovery-jump')?.textContent.includes('Sửa tiêu đề')", 'saving tags preserves title recovery');
    check(writes.length === 1 && writes[0] === `/api/v1/drafts/${id}/tags`
      && (await page.draft(id)).version === shown.version && await page.eval(`${SOURCE} === ${JSON.stringify(bad)}`),
    '#263 shortcut and tag-only save never resend the rejected text or replace the buffer');
  } finally {
    page.cdp.listeners.splice(page.cdp.listeners.indexOf(observe), 1);
  }
  await replaceSource(page, source.replace('Tên hợp lệ', 'Đã sửa H1'));
  await page.waitStatus('unsaved');
  check(await page.eval(`${SAME_VIEW} && document.querySelector('#editor-body-title-error').textContent.trim() === ''`),
    '#263 editing clears the field guidance on the same editor');
  await save(page);
  check((await page.draft(id)).title === 'Đã sửa H1' && await page.eval(SAME_VIEW), '#263 corrected H1 persists from the same buffer');

  for (const untitled of ['#\n\n# H1 thứ hai\n\n' + notes, notes]) {
    await replaceSource(page, untitled);
    await save(page);
    const saved = await page.draft(id);
    check(saved.title === '' && saved.document.source === untitled,
      '#263 an empty first H1 or absent H1 saves as an untitled draft with its exact source');
    check(await page.eval("!document.querySelector('#preview, #draft-preview')"), 'untitled drafts have no standalone preview');
    await expectMissingSnapshotTitle(page, check, id, saved.version);
  }

  const plain = '# Dấu thăng là chữ\n\n' + notes;
  const plainId = await openDraft(page, args.base, check, { format: 'default', source: plain }, 'Tên riêng');
  const changeTitle = value => page.eval(`(() => {
    const input = document.querySelector('#draft-title');
    input.value = ${JSON.stringify(value)};
    input.dispatchEvent(new InputEvent('input', {bubbles: true}));
  })()`);
  await changeTitle('Tên\u2028lỗi');
  await page.click(SAVE);
  await page.waitFor("document.querySelector('#draft-title[aria-invalid=true]')", 'Default invalid state is attached to the native title field');
  await page.click('#save-recovery-jump');
  check(await page.eval("document.activeElement?.id === 'draft-title'") && (await page.draft(plainId)).title === 'Tên riêng',
    '#263 Default title refusal and recovery stay on its own field');
  await changeTitle('Tên riêng đã sửa');
  await save(page);
  check((await page.draft(plainId)).title === 'Tên riêng đã sửa' && await page.eval(`${SAME_VIEW} && ${SOURCE} === ${JSON.stringify(plain)}`),
    '#263 a saved Default title remains independent of literal # source');
  await changeTitle('');
  await save(page);
  const untitled = await page.draft(plainId);
  check(untitled.title === '', '#263 blank Default title remains valid for a working draft');
  await expectMissingSnapshotTitle(page, check, plainId, untitled.version);
}

async function outlineOffsets(page, { args, check }) {
  const label = 'Mục trùng';
  const first = '# Tên\n\n👩🏽‍💻 Vie\u0323\u0302t\n\n## ' + label + '\n\n';
  const source = first + 'Một đoạn đi ngang cánh đồng.\n\n'.repeat(6520) + '## ' + label + '\n\n### Mục sau\n';
  const tail = source.lastIndexOf('## ' + label);
  const labelStart = tail + 3;
  const end = labelStart + label.length;
  const item = from => `#editor-outline-panel button:has([data-heading-from="${from}"])`;
  const at = from => `document.querySelector(${JSON.stringify(item(from))})`;
  const id = await openDraft(page, args.base, check, { format: 'markdown', source });
  await page.click('#outline-toggle');
  await page.waitFor(at(tail), 'outline completes parsing to the final headings');
  check(await page.eval(`(() => {
    const items = [...document.querySelectorAll('#editor-outline-panel [data-heading-from]')].filter(el => el.textContent === ${JSON.stringify(label)});
    return items.map(el => Number(el.dataset.headingFrom)).join(',') === '${first.indexOf('## ')},${tail}';
  })()`), '#257 duplicate labels retain distinct UTF-16 offsets after emoji and decomposed Vietnamese');

  check(await page.eval(`(() => {
    const v = ${CM}, old = ${at(tail)};
    const original = v.state.sliceDoc(2, 5);
    v.dispatch({changes: {from: 2, to: 5, insert: 'Tên khác'}, selection: {anchor: 0}});
    v.dispatch({changes: {from: 2, to: 2 + 'Tên khác'.length, insert: original}, selection: {anchor: 0}});
    const connected = old.isConnected;
    old.click();
    return connected && v.state.selection.main.head === 0 && v.state.doc.toString() === ${JSON.stringify(source)};
  })()`), '#257 connected pre-edit outline command is refused across A→B→A even when the source matches again');
  await twoFrames(page);
  await page.click(item(tail));
  await page.waitFor(`${at(tail)}.closest('[aria-current="location"]') && ${CM}.state.selection.main.head === ${end}`, 'current outline command jumps to the second duplicate at the document tail');

  await page.eval(`${CM}.dispatch({changes: {from: ${labelStart}, to: ${end}, insert: 'Mục vừa sửa'}, userEvent: 'input'})`);
  await page.waitFor(`${at(tail)}.textContent === 'Mục vừa sửa'`, 'outline tracks a local edit at the end of the document');
  await page.key('z', page.mod);
  await page.waitFor(`${at(tail)}.textContent === '${label}' && ${SOURCE} === ${JSON.stringify(source)}`, 'Undo restores outline label and source at the same offset');

  await page.viewport(320, 900);
  await page.eval("document.documentElement.dataset.text = '200'");
  await page.waitFor("document.querySelector('#editor-outline-panel:modal')?.contains(document.activeElement)", 'wide outline becomes a focused sheet at 320px / text200');
  await page.key('Escape');
  await page.waitFor("document.activeElement?.id === 'outline-toggle' && !document.querySelector('#editor-outline-panel')", 'Escape restores the sheet trigger');
  await page.key('Enter');
  await page.waitFor("document.querySelector('#editor-outline-panel:modal')?.contains(document.activeElement)", 'keyboard reopens the outline sheet');
  await page.click(item(tail));
  await page.waitFor(`!document.querySelector('#editor-outline-panel') && document.activeElement === ${CM}.contentDOM &&
    ${CM}.state.selection.main.head === ${end}`, 'sheet selection returns focus to the chosen heading');
  await page.viewport(1440, 900);
  await page.eval("delete document.documentElement.dataset.text");

  const advanced = await write(page, '/api/v1/drafts/' + id, 'PUT', {
    expected_version: 2, document: { format: 'markdown', source: '# Bản khác\n\nMới hơn.' },
  });
  check(advanced.status === 200 && advanced.body.version === 3, '#257 separate write advances the scratch draft version');
  await page.click(SAVE);
  await page.waitStatus('conflict');
  check(await page.eval(`${SAME_VIEW} && ${SOURCE} === ${JSON.stringify(source)}`), '#257 a version conflict preserves the local source and editor');
  await page.click('#outline-toggle');
  await page.waitFor(at(tail), 'the local outline remains usable during a save conflict');
  await page.click('#save-recovery-jump');
  await page.waitFor("document.querySelector('#editor-text-conflict')?.open", 'text conflict confirmation');
  await page.click('#keep-mine');
  await page.waitStatus('saved');
  await page.reload();
  await page.waitFor(CM, 'resolved draft reloads');
  await page.click('#outline-toggle');
  await page.waitFor(at(tail), 'reloaded long draft rebuilds the final heading');
  check(await page.eval(`${SOURCE} === ${JSON.stringify(source)}`), '#257 reload restores full source and rebuilds the tail outline after explicit conflict recovery');
}

export async function editorDeskRegressions(page, options) {
  await markdownLinks(page, options);
  await titleValidation(page, options);
  await outlineOffsets(page, options);
}
