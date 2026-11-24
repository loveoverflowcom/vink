import { test } from 'node:test';
import assert from 'node:assert/strict';
import { deflateSync } from 'node:zlib';

import {
  MANIFEST_SCHEMA_VERSION,
  buildLaneSummary,
  classifyTextScaleLane,
  collectSourceIdentity,
  evaluateAccessibleElement,
  evaluateLeaveModalBranch,
  evaluateMePanelReservation,
  evaluateNavAlignment,
  evaluateOverflowReport,
  evaluateTargetGeometry,
  extractAxNodeSummary,
  parsePngMetadata,
  parseSceneFilenameExpectations,
  validateCaptureScene,
  validateManifest,
  validateManifestRecord,
} from './visual-evidence.mjs';

function crc32(bytes) {
  let crc = 0xffffffff;
  for (const byte of bytes) {
    crc ^= byte;
    for (let i = 0; i < 8; i++) {
      crc = crc & 1 ? (crc >>> 1) ^ 0xedb88320 : crc >>> 1;
    }
  }
  return (crc ^ 0xffffffff) >>> 0;
}

function makeChunk(type, data) {
  const typeBytes = Buffer.from(type, 'ascii');
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length, 0);
  const crcInput = Buffer.concat([typeBytes, data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(crcInput), 0);
  return Buffer.concat([len, typeBytes, data, crc]);
}

function makeTestPng(width, height) {
  const signature = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0);
  ihdr.writeUInt32BE(height, 4);
  ihdr[8] = 8; // bit depth
  ihdr[9] = 6; // RGBA
  const raw = Buffer.alloc(height * (1 + width * 4), 120);
  for (let y = 0; y < height; y++) raw[y * (1 + width * 4)] = 0;
  const idat = deflateSync(raw);
  return Buffer.concat([
    signature,
    makeChunk('IHDR', ihdr),
    makeChunk('IDAT', idat),
    makeChunk('IEND', Buffer.alloc(0)),
  ]);
}

test('AC-01: PNG verification parses hash and dimensions and rejects corrupt/empty artifacts', () => {
  const png = makeTestPng(320, 800);
  const meta = parsePngMetadata(png);
  assert.equal(meta.width, 320);
  assert.equal(meta.height, 800);
  assert.match(meta.sha256, /^[0-9a-f]{64}$/);
  assert.equal(meta.byteSize, png.length);

  assert.throws(() => parsePngMetadata(Buffer.from('not-a-png')), /signature/);
  assert.throws(() => parsePngMetadata(makeTestPng(0, 800)), /dimensions/);
  // Signature + IHDR only, missing IDAT.
  const withoutIdat = png.subarray(0, 33);
  assert.throws(() => parsePngMetadata(withoutIdat), /IDAT/);
});

test('AC-01: rejects silent 900-height substitution for 800-height scenes and mismatched viewport filenames', () => {
  const png900 = parsePngMetadata(makeTestPng(320, 900));
  const actual900 = { width: 320, height: 900, dpr: 1, theme: 'dark', fontSize: '32px', hydrated: true };

  // Mismatched WxH filename vs actual viewport height 900.
  assert.throws(
    () =>
      validateCaptureScene({
        name: 'nav-ban-viet-320x800-200-dark',
        requestedScene: { width: 320, height: 800, scale: 200, theme: 'dark' },
        actual: actual900,
        pngMeta: png900,
      }),
    /mismatched viewport filename.*height mismatch/,
  );

  // Requested 800 height fails even if filename omits height.
  assert.throws(
    () =>
      validateCaptureScene({
        name: 'editor-failed-320-dark',
        requestedScene: { width: 320, height: 800, scale: 200, theme: 'dark' },
        actual: actual900,
        pngMeta: png900,
      }),
    /height mismatch: expected 800, got 900/,
  );

  // Explicit 900 height passes when both requested and filename match 900.
  assert.doesNotThrow(() =>
    validateCaptureScene({
      name: 'nav-ban-viet-320x900-200-dark',
      requestedScene: { width: 320, height: 900, scale: 200, theme: 'dark' },
      actual: actual900,
      pngMeta: png900,
    }),
  );

  // Hydration phase mismatch must fail.
  assert.throws(
    () =>
      validateCaptureScene({
        name: 'appearance-first-paint',
        requestedScene: { width: 320, height: 900, scale: 200, theme: 'dark', hydrationPhase: 'before-hydration' },
        actual: actual900,
        pngMeta: png900,
      }),
    /before-hydration/,
  );

  // Width-only legacy names do not invent a height constraint.
  assert.equal(parseSceneFilenameExpectations('390-me-system-text100'), null);
  assert.deepEqual(parseSceneFilenameExpectations('1440x360-fragment-scroll-text200'), { width: 1440, height: 360 });
});

test('AC-01: manifest schema supports legacy v1 compatibility and marks missing-target captures INCOMPLETE', () => {
  const legacyRecord = {
    route: '/ban-viet',
    width: 390,
    layoutWidth: 375,
    height: 900,
    dpr: 1,
    scrollX: 0,
    scrollY: 0,
    theme: 'dark',
    fontSize: '32px',
    captured_at: '2026-11-14T10:00:00.000Z',
  };
  assert.equal(validateManifestRecord('nav-ban-viet-390-200-dark', legacyRecord, { allowLegacyV1: true }).ok, true);
  assert.equal(validateManifestRecord('nav-ban-viet-390-200-dark', legacyRecord, { allowLegacyV1: false }).ok, false);

  const png = parsePngMetadata(makeTestPng(360, 800));
  const v2Valid = {
    schemaVersion: MANIFEST_SCHEMA_VERSION,
    scenarioId: 'bottom-nav-360x800',
    captureStatus: 'CAPTURED',
    route: '/ban-viet',
    state: 'hydrated',
    hydrated: true,
    width: 360,
    layoutWidth: 360,
    height: 800,
    dpr: 1,
    viewport: { width: 360, height: 800, layoutWidth: 360, clientWidth: 360, dpr: 1 },
    theme: 'light',
    fontSize: '16px',
    scrollOwner: { tagName: 'HTML', scrollTop: 0, scrollHeight: 800, clientHeight: 800, scrollWidth: 360, clientWidth: 360 },
    source: { sourceSHA: '09ebede781ccf4eed6cfb5e73872f277bee24287', dirty: true, dirtyInputs: [{ status: 'M', path: 'apps/vink-web/e2e/browser-smoke.mjs' }] },
    png,
    targets: { required: [{ selector: '.vk-ui-bottom-nav', present: true, visible: true }] },
    captured_at: '2026-11-16T08:00:00.000Z',
  };
  assert.equal(validateManifest({ 'nav-ban-viet-360x800-100-light': v2Valid }).ok, true);

  // Scenario target missing makes capture INCOMPLETE, never PASS.
  const incomplete = {
    ...v2Valid,
    captureStatus: 'INCOMPLETE',
    targets: { required: [{ selector: '[data-testid=missing-recovery]', present: false, visible: false }] },
  };
  const checkIncomplete = validateManifestRecord('recovery-missing-360x800-100-light', incomplete);
  assert.equal(checkIncomplete.ok, false);
  assert.ok(checkIncomplete.errors.some((e) => e.includes('INCOMPLETE')));
  assert.ok(checkIncomplete.errors.some((e) => e.includes('missing or invisible')));
});

test('AC-01: collectSourceIdentity tracks dirty UI inputs deterministically without secrets', () => {
  const id = collectSourceIdentity({
    rootDir: process.cwd(),
    buildSha: 'abc12345',
    exec: (args) => {
      if (args[0] === 'status') return ' M apps/vink-web/e2e/browser-smoke.mjs\n?? apps/vink-web/e2e/visual-evidence.mjs\n';
      return '';
    },
  });
  assert.equal(id.sourceSHA, 'abc12345');
  assert.equal(id.dirty, true);
  assert.deepEqual(id.dirtyInputs, [
    { status: 'M', path: 'apps/vink-web/e2e/browser-smoke.mjs' },
    { status: '??', path: 'apps/vink-web/e2e/visual-evidence.mjs' },
  ]);
});

test('AC-03: overflow oracle catches hidden overflow with scrollX=0 while allowing text input internal scroll', () => {
  // Negative case: scrollX is 0 because container has overflow-x: hidden, yet component overflows!
  const hiddenOverflow = evaluateOverflowReport({
    document: { scrollX: 0, scrollY: 0, innerWidth: 320, innerHeight: 800, scrollWidth: 320, clientWidth: 320, overflowX: 'visible' },
    body: { scrollWidth: 320, clientWidth: 320, overflowX: 'visible' },
    unintendedOverflows: [{ selector: '#leave-editor', scrollWidth: 640, clientWidth: 296, overflowPx: 344, overflowX: 'hidden', hiddenOverflow: true }],
    allowedInputScrolls: [{ selector: '#draft-title', scrollWidth: 980, clientWidth: 273, overflowPx: 707, overflowX: 'auto', reason: 'text-input-internal-scroll' }],
    clippedTargets: [],
  });
  assert.equal(hiddenOverflow.ok, false);
  assert.ok(hiddenOverflow.reasons[0].includes('#leave-editor'));

  // Positive case: only text input (#draft-title / .cm-scroller) scrolls internally.
  const validInputScroll = evaluateOverflowReport({
    document: { scrollX: 0, scrollY: 0, innerWidth: 320, innerHeight: 800, scrollWidth: 320, clientWidth: 320, overflowX: 'visible' },
    body: { scrollWidth: 320, clientWidth: 320, overflowX: 'visible' },
    unintendedOverflows: [],
    allowedInputScrolls: [
      { selector: '#draft-title', scrollWidth: 980, clientWidth: 273, overflowPx: 707, reason: 'text-input-internal-scroll' },
      { selector: '.cm-scroller', scrollWidth: 420, clientWidth: 273, overflowPx: 147, reason: 'text-input-internal-scroll' },
    ],
    clippedTargets: [],
  });
  assert.equal(validInputScroll.ok, true);
  assert.equal(validInputScroll.allowedInputScrolls.length, 2);

  // Negative case: hidden/clipped target.
  const clipped = evaluateTargetGeometry({
    selector: '#leave-discard',
    present: true,
    visible: true,
    full: false,
    hit: false,
    labelFits: true,
    rect: { x: -400, y: 100, width: 120, height: 48 },
    clip: { left: 0, right: 320, top: 0, bottom: 800 },
    page: { scrollX: 0, scrollWidth: 320, clientWidth: 320 },
    owner: { id: 'leave-editor', scrollWidth: 320, clientWidth: 320 },
  });
  assert.equal(clipped.ok, false);
  assert.equal(clipped.status, 'FAIL');
  assert.ok(clipped.reasons.some((r) => r.includes('clipped')));
});

test('AC-03: AX oracle separates raw textContent from computed AX name and catches altered accessible name', () => {
  const validAx = extractAxNodeSummary({
    nodeId: '22',
    ignored: false,
    role: { value: 'link' },
    name: { value: 'Tôi', sources: [{ type: 'contents', value: { value: 'Tôi' } }] },
    properties: [{ name: 'focusable', value: { value: true } }],
  });
  const goodRecord = {
    selector: '.vk-ui-bottom-nav a[href="/toi"]',
    present: true,
    rawTextContent: 'TTTôi',
    ax: validAx,
    dom: { ariaCurrent: 'page' },
  };
  assert.equal(
    evaluateAccessibleElement(goodRecord, {
      role: 'link',
      name: 'Tôi',
      excludeSubstrings: ['TT', '•'],
      ariaCurrent: 'page',
    }).ok,
    true,
  );

  // Negative case: decorative initials leaked into accessible name -> "TTTôi".
  const altered = {
    ...goodRecord,
    ax: { ...validAx, name: 'TTTôi' },
  };
  const res = evaluateAccessibleElement(altered, {
    role: 'link',
    name: 'Tôi',
    excludeSubstrings: ['TT', '•'],
  });
  assert.equal(res.ok, false);
  assert.ok(res.reasons.some((r) => r.includes('TTTôi')));
});

test('AC-03: nav alignment accepts 1-line vs 2-line height delta when first lines align and catches misaligned first line', () => {
  // Exact Nov 14 nav-ban-viet-390-200-dark geometry: 2-line "Khám phá" (72.78px) vs 1-line "Tôi" (36.39px)
  // with iconCenterY and firstLineY aligned at 0px delta.
  const validItems = [
    {
      labelText: 'Khám phá',
      y: 771.21875,
      width: 130,
      height: 128.78125,
      iconCenterY: 791.21875,
      firstLineY: 812.21875,
      labelBoxHeight: 72.78125,
      lineCount: 2,
      wordsIntact: true,
      labelScrollWidth: 110,
      labelClientWidth: 130,
      labelFont: '500 28px/36.39px "Be Vietnam Pro"',
      labelLineHeight: '36.39px',
    },
    {
      labelText: 'Bàn viết',
      y: 771.21875,
      width: 130,
      height: 128.78125,
      iconCenterY: 791.21875,
      firstLineY: 812.21875,
      labelBoxHeight: 72.78125,
      lineCount: 2,
      wordsIntact: true,
      labelScrollWidth: 110,
      labelClientWidth: 130,
      labelFont: '500 28px/36.39px "Be Vietnam Pro"',
      labelLineHeight: '36.39px',
    },
    {
      labelText: 'Tôi',
      y: 771.21875,
      width: 130,
      height: 128.78125,
      iconCenterY: 791.21875,
      firstLineY: 812.21875,
      labelBoxHeight: 36.390625,
      lineCount: 1,
      wordsIntact: true,
      labelScrollWidth: 40,
      labelClientWidth: 130,
      labelFont: '500 28px/36.39px "Be Vietnam Pro"',
      labelLineHeight: '36.39px',
    },
  ];
  const pass = evaluateNavAlignment(validItems);
  assert.equal(pass.ok, true);
  assert.equal(pass.maxIconCenterDelta, 0);
  assert.equal(pass.maxFirstLineDelta, 0);

  // Negative case: misaligned nav first line on one tab (e.g. centered single-line vertically -> +18px).
  const misaligned = validItems.map((item, idx) =>
    idx === 2 ? { ...item, firstLineY: item.firstLineY + 18 } : item,
  );
  const fail = evaluateNavAlignment(misaligned);
  assert.equal(fail.ok, false);
  assert.ok(fail.reasons.some((r) => r.includes('firstLineY spread')));
});

test('AC-02: modal oracle enforces all can_discard/confirm branches and catches missing last modal action', () => {
  const modal = {
    present: true,
    open: true,
    scroll: { scrollTop: 0, scrollHeight: 520, clientHeight: 380, scrollWidth: 340, clientWidth: 340 },
    actions: [
      { id: 'leave-stay', disabled: false, disabledReason: null },
      { id: 'leave-save', disabled: true, disabledReason: 'Đang có bản mới trên máy chủ.' },
      { id: 'leave-discard', disabled: false, disabledReason: null },
    ],
  };
  assert.equal(evaluateLeaveModalBranch(modal, 'text-conflict').ok, true);
  assert.equal(evaluateLeaveModalBranch(modal, 'saving').ok, true);

  // Failed state keeps leave-save enabled alongside discard (can_discard=true).
  const failedModal = {
    ...modal,
    actions: modal.actions.map((a) => (a.id === 'leave-save' ? { ...a, disabled: false, disabledReason: null } : a)),
  };
  assert.equal(evaluateLeaveModalBranch(failedModal, 'failed').ok, true);

  // Negative case: missing last modal action (#leave-discard dropped, leaving only 2 buttons).
  const missingLast = { ...modal, actions: modal.actions.slice(0, 2) };
  const missingResult = evaluateLeaveModalBranch(missingLast, 'text-conflict');
  assert.equal(missingResult.ok, false);
  assert.ok(missingResult.reasons[0].includes('leave-discard'));

  // Negative case: guard-disabled button without aria-describedby reason.
  const missingReason = {
    ...modal,
    actions: modal.actions.map((a) => (a.id === 'leave-save' ? { ...a, disabledReason: null } : a)),
  };
  assert.equal(evaluateLeaveModalBranch(missingReason, 'text-conflict').ok, false);
});

test('AC-04 & AC-05: separates DOM override from app controls and validates Me illustration reservation', () => {
  assert.equal(classifyTextScaleLane({ rootInlineFontSize: '200%', dataText: null }), 'dom-override-geometry-only');
  assert.equal(classifyTextScaleLane({ rootInlineFontSize: '', dataText: '200' }), 'dom-override-geometry-only');
  assert.equal(classifyTextScaleLane({ rootInlineFontSize: '', dataText: '150' }), 'app-appearance-control');
  assert.equal(classifyTextScaleLane({ rootInlineFontSize: '200%', dataText: '150' }), 'dom-override-geometry-only');
  assert.equal(classifyTextScaleLane({
    rootInlineFontSize: '', dataText: '200', explicitLane: 'app-appearance-control',
  }), 'dom-override-geometry-only');
  assert.equal(classifyTextScaleLane({
    rootInlineFontSize: '200%', dataText: '150', explicitLane: 'app-appearance-control',
  }), 'dom-override-geometry-only');
  assert.equal(classifyTextScaleLane({ rootInlineFontSize: '', dataText: null }), 'default-100');

  // Me page (>= 32rem container): action is null, illustration visible 210px, paddingInlineEnd 225px -> PASS!
  const validWideMe = {
    prefix: 'page',
    action: null,
    illustration: { present: true, visible: true, display: 'block', ariaHidden: 'true', pointerEvents: 'none', rect: { x: 800, y: 150, width: 210, height: 80 } },
    reservation: { paddingInlineEnd: 225, accountTextContentWidth: 420, textOverlapsIllustration: false },
  };
  assert.equal(evaluateMePanelReservation(validWideMe).ok, true);

  // Narrow Me (< 32rem): illustration hidden, paddingInlineEnd 0 -> PASS!
  const validNarrowMe = {
    prefix: 'page',
    action: null,
    illustration: { present: true, visible: false, display: 'none', ariaHidden: 'true', pointerEvents: 'none', rect: null },
    reservation: { paddingInlineEnd: 0, accountTextContentWidth: 240, textOverlapsIllustration: false },
  };
  assert.equal(evaluateMePanelReservation(validNarrowMe).ok, true);

  // Unused reservation: 225px padding when illustration is NOT visible -> FAIL!
  const unusedReservation = {
    ...validNarrowMe,
    reservation: { paddingInlineEnd: 225, accountTextContentWidth: 15, textOverlapsIllustration: false },
  };
  assert.equal(evaluateMePanelReservation(unusedReservation).ok, false);

  // Summary keeps unavailable/unrun lanes explicit.
  const summary = buildLaneSummary({
    phase: 'appearance',
    lanes: { appAppearanceControl: 'PASS', domOverrideGeometryOnly: 'PASS' },
  });
  assert.equal(summary.lanes.browserZoom.status, 'NOT_RUN');
  assert.equal(summary.lanes.osTextScaleAndIme.status, 'NOT_RUN');
  assert.equal(summary.lanes.screenReader.status, 'NOT_RUN');
  assert.equal(summary.lanes.realDevice.status, 'NOT_RUN');
  assert.equal(summary.fullWebVerify, 'NOT_RUN');
});

test('geometry-only capture still rejects a wrong viewport, theme or scale', () => {
  const name = 'reader-320x800-dark';
  const requestedScene = { width: 320, height: 800, theme: 'dark', textScale: 200 };
  const actual = { width: 320, height: 800, dpr: 1, theme: 'dark', fontSize: '32px' };
  const input = { name, requestedScene, actual, pngMeta: null };
  assert.doesNotThrow(() => validateCaptureScene({ ...input, checkPng: false }));
  for (const [field, value] of [['height', 900], ['theme', 'light'], ['fontSize', '16px']]) {
    assert.throws(() => validateCaptureScene({
      ...input, actual: { ...actual, [field]: value }, checkPng: false,
    }), /capture scene mismatch/);
  }
  assert.throws(() => validateCaptureScene(input), /invalid PNG dimensions/);
});
