import { holdAnswer } from './auth-browser-network.mjs';

async function checkPresets() {
  const core = await import('/pkg/vink-editor.js');
  const host = document.createElement('div');
  document.body.append(host);
  const results = [];
  try {
    for (const preset of ['document', 'comment']) {
      for (const format of ['plain', 'markdown']) {
        const composition = [];
        let changes = 0;
        let outlines = 0;
        const handle = core.createEditor(host, {
          preset, format, text: '# Mưa\n\n**Phố**', labelledBy: 'label', describedBy: 'hint',
          onChange: () => changes++, onComposition: active => composition.push(active),
          onOutline: () => outlines++,
        });
        try {
          const { view } = handle;
          const heading = Boolean(host.querySelector('.cm-writing-h1'));
          const bold = Boolean(host.querySelector('.cm-writing-bold'));
          view.contentDOM.dispatchEvent(new CompositionEvent('compositionstart', { bubbles: true }));
          view.contentDOM.dispatchEvent(new CompositionEvent('compositionend', { bubbles: true }));
          await new Promise(r => setTimeout(r, 55));
          const noEdit = changes === 0;
          view.dispatch({ selection: { anchor: 2, head: 5 } });
          const formatted = core.applyInlineFormat(handle, 'italic');
          const changed = changes;
          const headingAllowed = core.applyHeading(handle, 2);
          const before = core.getText(handle);
          core.applyConvertedDocument(handle, 'plain', 'Văn xuôi');
          results.push({ preset, format, heading, bold, noEdit, composition,
            formatted, changed, headingAllowed, outlines,
            converted: preset === 'document' ? core.getText(handle) === 'Văn xuôi' : core.getText(handle) === before,
            formatChangedInPlace: view === handle.view && view.dom.isConnected });
        } finally {
          core.destroyEditor(handle);
        }
      }
    }
    const options = { preset: 'comment', format: 'markdown', text: 'Trăng',
      readOnly: true, labelledBy: 'label', describedBy: 'hint', onChange() {} };
    const locked = core.createEditor(host, options);
    locked.view.contentDOM.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    locked.view.dispatch({ changes: { from: 0, insert: 'x' }, userEvent: 'input.type' });
    const readonly = !locked.view.contentDOM.isContentEditable && locked.view.state.readOnly &&
      !core.applyInlineFormat(locked, 'bold') && core.getText(locked) === options.text;
    let duplicateRefused = false;
    try { core.createEditor(host, options); } catch { duplicateRefused = true; }
    core.destroyEditor(locked);
    core.destroyEditor(locked);
    let failedCleanly = false;
    try {
      core.createEditor(host, { ...options, preset: 'document', onOutline() { throw Error('fixture callback'); } });
    } catch {
      failedCleanly = !host.firstChild && !host.hasAttribute('data-vink-editor');
    }
    return { results, readonly, duplicateRefused, failedCleanly };
  } finally {
    host.remove();
  }
}

export async function editorCoreContracts(page, { args, check, sampleWork }) {
  const witness = await page.eval('(' + checkPresets + ')()');
  for (const row of witness.results) {
    const markdown = row.format === 'markdown';
    const documentMarkdown = row.preset === 'document' && markdown;
    check(row.heading === documentMarkdown && row.bold === markdown &&
      row.headingAllowed === documentMarkdown && row.formatted === markdown &&
      row.changed === Number(markdown) && row.noEdit && row.converted && row.formatChangedInPlace &&
      JSON.stringify(row.composition) === '[true,false]' && (row.outlines > 0) === (row.preset === 'document'),
    'shared editor ' + row.preset + '/' + row.format + ': decorations, composition-only notification and command fences');
  }
  check(witness.readonly && witness.duplicateRefused && witness.failedCleanly,
    'read-only and mount fences preserve one live owner and release a failed initial callback');

  const path = '/pkg/vink-editor.js';
  for (const leave of [false, true]) {
    const gate = await holdAnswer(page, 'GET', path);
    await page.goto(args.base + '/ban-viet/' + sampleWork);
    await gate.held();
    await page.waitFor("document.querySelector('[data-testid=editor-body][data-state=loading]')", 'loading editor host');
    if (leave) {
      await page.eval(`(() => {
        const host = document.querySelector('[data-testid=editor-body]');
        window.pendingProbe = { host, mounted: false, initialChildren: [...host.childNodes].map(n => n.nodeName) };
        const observer = new MutationObserver(records => {
          for (const record of records) {
            if (record.type === 'attributes' || [...record.addedNodes].some(n =>
              n.matches?.('.cm-editor') || n.querySelector?.('.cm-editor'))) pendingProbe.mounted = true;
          }
        });
        observer.observe(host, { childList: true, attributes: true, attributeFilter: ['data-vink-editor'] });
        pendingProbe.observer = observer;
        document.querySelector('#back-to-desk').click();
      })()`);
      await page.waitFor("location.pathname === '/ban-viet' && window.pendingProbe && !pendingProbe.host.isConnected", 'client navigation disposed the original editor host');
    }
    await gate.finish(!leave, true);
    if (leave) {
      const result = await page.eval(`(() => {
        const {host, mounted, initialChildren, observer} = pendingProbe;
        observer.disconnect();
        delete window.pendingProbe;
        return {mounted, initialChildren, finalChildren: [...host.childNodes].map(n => n.nodeName),
          detachedViews: host.querySelectorAll('.cm-editor').length,
          connectedViews: document.querySelectorAll('.cm-editor').length, marked: host.hasAttribute('data-vink-editor')};
      })()`);
      check(!result.mounted && !result.marked && !result.detachedViews && !result.connectedViews,
        'late import after navigation creates no transient, detached or connected view: ' + JSON.stringify(result));
    } else {
      await page.waitFor("document.querySelector('[data-testid=editor-body][data-state=failed]')", 'module failure reaches the Product body state');
      check(await page.eval("!document.querySelector('.cm-editor')"), 'failed module never exposes an editable partial body');
    }
  }
}
