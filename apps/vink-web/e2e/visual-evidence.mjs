// Visual QA evidence, geometry, overflow, accessibility and manifest oracles for Vink Web CDP smoke.
// Zero npm dependencies (Node.js 22+). Keeps DOM-override geometry checks distinct from real app
// appearance controls, browser zoom, OS text scale/IME, screen readers and physical devices.

import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

export const MANIFEST_SCHEMA_VERSION = 2;

export const DEFAULT_TEXT_INPUT_SELECTORS = Object.freeze([
  'input',
  'textarea',
  '[contenteditable]',
  '.cm-scroller',
  '.cm-content',
]);

export const LEAVE_MODAL_BRANCHES = Object.freeze({
  unsaved: Object.freeze({
    expectedIds: ['leave-stay', 'leave-save'],
    canDiscard: false,
    disabledIds: [],
  }),
  failed: Object.freeze({
    expectedIds: ['leave-stay', 'leave-save', 'leave-discard'],
    canDiscard: true,
    disabledIds: [],
  }),
  saving: Object.freeze({
    expectedIds: ['leave-stay', 'leave-save', 'leave-discard'],
    canDiscard: true,
    disabledIds: ['leave-save'],
  }),
  'text-conflict': Object.freeze({
    expectedIds: ['leave-stay', 'leave-save', 'leave-discard'],
    canDiscard: true,
    disabledIds: ['leave-save'],
  }),
  'tag-conflict': Object.freeze({
    expectedIds: ['leave-stay', 'leave-save', 'leave-discard'],
    canDiscard: true,
    disabledIds: ['leave-save'],
  }),
  confirm: Object.freeze({
    expectedIds: ['discard-back', 'discard-confirm'],
    canDiscard: true,
    disabledIds: [],
  }),
});

export function sha256Hex(bytes) {
  return createHash('sha256').update(bytes).digest('hex');
}

/**
 * Collects git SHA, dirty UI inputs and a deterministic source fingerprint without exposing
 * secrets or private draft contents.
 */
export function collectSourceIdentity({
  rootDir = process.cwd(),
  buildSha = process.env.GITHUB_SHA,
  exec = (args) => execFileSync('git', args, { cwd: rootDir, encoding: 'utf8' }),
} = {}) {
  const sourceSHA = (buildSha || exec(['rev-parse', 'HEAD'])).trim();
  const scopedRoots = [
    'apps/vink-web/src',
    'apps/vink-web/style',
    'apps/vink-web/e2e',
    'packages/ui-system/shared',
    'packages/ui-system/leptos',
    'packages/vink-patterns/leptos',
  ];
  let statusLines = [];
  try {
    const rawStatus = exec([
      'status',
      '--porcelain=v1',
      '--untracked-files=all',
      '--',
      ...scopedRoots,
      'xtask/src',
    ]);
    statusLines = rawStatus
      .split('\n')
      .map((line) => line.trimEnd())
      .filter(Boolean)
      .sort();
  } catch {
    statusLines = [];
  }
  const dirtyInputs = statusLines.map((line) => ({
    status: line.slice(0, 2).trim() || '??',
    path: line.slice(3).trim(),
  }));

  let sourceFingerprint = null;
  try {
    const listed = exec(['ls-files', '--cached', '--others', '--exclude-standard', '--', ...scopedRoots])
      .split('\n')
      .map((p) => p.trim())
      .filter(Boolean)
      .sort();
    const hashes = Object.fromEntries(
      listed.map((relPath) => [relPath, sha256Hex(readFileSync(join(rootDir, relPath)))]),
    );
    sourceFingerprint = sha256Hex(JSON.stringify(hashes));
  } catch {
    sourceFingerprint = null;
  }

  return {
    sourceSHA,
    dirty: dirtyInputs.length > 0,
    dirtyInputs,
    sourceFingerprint,
  };
}

/**
 * Parses and validates PNG header, IHDR dimensions and presence of at least one IDAT chunk.
 */
export function parsePngMetadata(bytes) {
  const buf = Buffer.isBuffer(bytes) ? bytes : Buffer.from(bytes);
  const signature = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);
  if (buf.length < 33 || !buf.subarray(0, 8).equals(signature)) {
    throw new Error('invalid PNG artifact: missing PNG file signature');
  }
  if (buf.subarray(12, 16).toString('ascii') !== 'IHDR') {
    throw new Error('invalid PNG artifact: first chunk is not IHDR');
  }
  const width = buf.readUInt32BE(16);
  const height = buf.readUInt32BE(20);
  if (!Number.isInteger(width) || !Number.isInteger(height) || width <= 0 || height <= 0) {
    throw new Error(`invalid PNG dimensions: ${width}x${height}`);
  }
  let offset = 8;
  let hasIdat = false;
  while (offset + 12 <= buf.length) {
    const chunkLength = buf.readUInt32BE(offset);
    const chunkType = buf.subarray(offset + 4, offset + 8).toString('ascii');
    if (chunkType === 'IDAT' && chunkLength > 0) {
      hasIdat = true;
      break;
    }
    const next = offset + 12 + chunkLength;
    if (next <= offset || next > buf.length) break;
    offset = next;
  }
  if (!hasIdat) {
    throw new Error('invalid PNG artifact: missing image data (IDAT)');
  }
  return {
    width,
    height,
    byteSize: buf.length,
    sha256: sha256Hex(buf),
  };
}

/**
 * Extracts explicit WxH dimension pairs from a capture filename when encoded as `<W>x<H>`.
 * Width-only tokens (e.g. `desk-error-320-light` or `390-me-system-text100`) intentionally do
 * not constrain height so variable-height scenarios remain valid unless requestedScene sets height.
 */
export function parseSceneFilenameExpectations(name) {
  const match = String(name).match(/(?:^|[-_])(\d{3,4})x(\d{3,4})(?=$|[-_])/);
  if (!match) return null;
  return {
    width: Number(match[1]),
    height: Number(match[2]),
  };
}

export function classifyTextScaleLane({ rootInlineFontSize, dataText, explicitLane } = {}) {
  if (explicitLane) return explicitLane;
  if (typeof rootInlineFontSize === 'string' && rootInlineFontSize.trim().length > 0) {
    return 'dom-override-geometry-only';
  }
  if (dataText === '200') return 'app-appearance-control';
  return 'default-100';
}

/**
 * Verifies capture filename, requested scene parameters, actual DOM geometry/theme/hydration
 * state and PNG pixel dimensions together so 900-height captures can never pass as 800-height.
 */
export function validateCaptureScene({ name, requestedScene, editorScene, actual, pngMeta }) {
  const reasons = [];
  if (!actual || !Number.isFinite(actual.width) || !Number.isFinite(actual.height) || actual.width <= 0 || actual.height <= 0) {
    reasons.push(`invalid actual viewport dimensions: ${JSON.stringify(actual)}`);
  }
  if (!pngMeta || !Number.isInteger(pngMeta.width) || !Number.isInteger(pngMeta.height) || pngMeta.width <= 0 || pngMeta.height <= 0) {
    reasons.push(`invalid PNG dimensions: ${JSON.stringify(pngMeta)}`);
  } else if (actual && Number.isFinite(actual.dpr)) {
    const expectedPxW = Math.round(actual.width * actual.dpr);
    const expectedPxH = Math.round(actual.height * actual.dpr);
    if (pngMeta.width !== expectedPxW || pngMeta.height !== expectedPxH) {
      reasons.push(
        `PNG pixels ${pngMeta.width}x${pngMeta.height} disagree with viewport ${actual.width}x${actual.height} at DPR ${actual.dpr}`,
      );
    }
  }

  const filenameDims = parseSceneFilenameExpectations(name);
  if (filenameDims && actual) {
    if (actual.width !== filenameDims.width || actual.height !== filenameDims.height) {
      reasons.push(
        `mismatched viewport filename: ${name} declares ${filenameDims.width}x${filenameDims.height} but actual viewport is ${actual.width}x${actual.height}`,
      );
    }
    if (pngMeta && Number.isFinite(actual.dpr)) {
      const expectedPxW = Math.round(filenameDims.width * actual.dpr);
      const expectedPxH = Math.round(filenameDims.height * actual.dpr);
      if (pngMeta.width !== expectedPxW || pngMeta.height !== expectedPxH) {
        reasons.push(
          `mismatched PNG dimensions for filename ${name}: expected ${expectedPxW}x${expectedPxH}, got ${pngMeta.width}x${pngMeta.height}`,
        );
      }
    }
  }

  if (editorScene) {
    const { width, height, scale, theme } = editorScene;
    if (!String(name).endsWith(`-${width}x${height}-${scale}-${theme}`)) {
      reasons.push(`editor capture name ${name} does not match scene ${width}x${height}-${scale}-${theme}`);
    }
  }

  const expected = editorScene ?? requestedScene;
  if (expected && actual) {
    if (expected.width !== undefined && actual.width !== expected.width) {
      reasons.push(`width mismatch: expected ${expected.width}, got ${actual.width}`);
    }
    if (expected.height !== undefined && actual.height !== expected.height) {
      reasons.push(`height mismatch: expected ${expected.height}, got ${actual.height}`);
    }
    const expectedScale = expected.scale ?? expected.textScale;
    if (expectedScale !== undefined) {
      const expectedFont = `${(16 * expectedScale) / 100}px`;
      if (actual.fontSize !== expectedFont) {
        reasons.push(`fontSize mismatch: expected ${expectedFont} (${expectedScale}%), got ${actual.fontSize}`);
      }
    }
    if (expected.theme !== undefined && actual.theme !== expected.theme) {
      reasons.push(`theme mismatch: expected ${expected.theme}, got ${actual.theme}`);
    }
    if (expected.hydrationPhase === 'before-hydration' && actual.hydrated !== false) {
      reasons.push('hydration mismatch: expected before-hydration (hydrated=false), got hydrated=true');
    }
    if (expected.hydrationPhase === 'after-hydration' && actual.hydrated !== true) {
      reasons.push('hydration mismatch: expected after-hydration (hydrated=true), got hydrated=false');
    }
  }

  if (reasons.length) {
    throw new Error(`capture scene mismatch for ${name}: ${reasons.join('; ')}`);
  }
}

/**
 * Validates a single manifest entry. Supports strict v2 records and optional backward-compatible
 * Nov 14 legacy v1 records (`allowLegacyV1: true`).
 */
export function validateManifestRecord(name, record, { allowLegacyV1 = false } = {}) {
  const errors = [];
  if (!record || typeof record !== 'object') {
    return { ok: false, errors: [`${name}: manifest record must be an object`] };
  }
  const hasPositiveDimensions =
    Number.isFinite(record.width) &&
    record.width > 0 &&
    Number.isFinite(record.height) &&
    record.height > 0 &&
    Number.isFinite(record.layoutWidth) &&
    record.layoutWidth > 0 &&
    Number.isFinite(record.dpr) &&
    record.dpr > 0;

  if (!hasPositiveDimensions) {
    errors.push(`${name}: invalid dimensions width/layoutWidth/height/dpr`);
  }
  if (typeof record.route !== 'string' || !record.route.startsWith('/')) {
    errors.push(`${name}: missing or invalid route`);
  }
  if (typeof record.theme !== 'string' || record.theme.length === 0) {
    errors.push(`${name}: missing theme`);
  }
  if (typeof record.fontSize !== 'string' || !record.fontSize.endsWith('px')) {
    errors.push(`${name}: missing fontSize`);
  }
  if (typeof record.captured_at !== 'string' || Number.isNaN(Date.parse(record.captured_at))) {
    errors.push(`${name}: missing or invalid captured_at timestamp`);
  }

  const filenameDims = parseSceneFilenameExpectations(name);
  if (filenameDims && (record.width !== filenameDims.width || record.height !== filenameDims.height)) {
    errors.push(
      `${name}: filename declares ${filenameDims.width}x${filenameDims.height}, record has ${record.width}x${record.height}`,
    );
  }

  const isLegacyV1 = record.schemaVersion === undefined;
  if (isLegacyV1) {
    if (!allowLegacyV1) {
      errors.push(`${name}: missing schemaVersion=${MANIFEST_SCHEMA_VERSION}`);
    }
    return { ok: errors.length === 0, version: 1, errors };
  }

  if (record.schemaVersion !== MANIFEST_SCHEMA_VERSION) {
    errors.push(`${name}: unsupported schemaVersion ${record.schemaVersion}`);
  }
  if (typeof record.scenarioId !== 'string' || record.scenarioId.length === 0) {
    errors.push(`${name}: missing scenarioId`);
  }
  if (record.captureStatus !== 'CAPTURED') {
    errors.push(`${name}: captureStatus is ${record.captureStatus ?? 'missing'}, expected CAPTURED`);
  }
  if (typeof record.state !== 'string' || record.state.length === 0) {
    errors.push(`${name}: missing state`);
  }
  if (typeof record.hydrated !== 'boolean') {
    errors.push(`${name}: missing hydrated boolean`);
  }
  if (
    !record.viewport ||
    !Number.isFinite(record.viewport.clientWidth) ||
    record.viewport.clientWidth <= 0 ||
    record.viewport.width !== record.width ||
    record.viewport.height !== record.height
  ) {
    errors.push(`${name}: invalid structured viewport/clientWidth`);
  }
  if (!record.scrollOwner || typeof record.scrollOwner.tagName !== 'string') {
    errors.push(`${name}: missing scrollOwner`);
  }
  if (
    !record.source ||
    typeof record.source.sourceSHA !== 'string' ||
    record.source.sourceSHA.length === 0 ||
    !Array.isArray(record.source.dirtyInputs)
  ) {
    errors.push(`${name}: missing source.sourceSHA or source.dirtyInputs`);
  }
  if (
    !record.png ||
    typeof record.png.sha256 !== 'string' ||
    !/^[0-9a-f]{64}$/.test(record.png.sha256) ||
    !Number.isInteger(record.png.width) ||
    !Number.isInteger(record.png.height) ||
    !Number.isInteger(record.png.byteSize) ||
    record.png.byteSize <= 0
  ) {
    errors.push(`${name}: missing verified PNG hash/dimension/byteSize metadata`);
  } else if (hasPositiveDimensions) {
    if (
      record.png.width !== Math.round(record.width * record.dpr) ||
      record.png.height !== Math.round(record.height * record.dpr)
    ) {
      errors.push(`${name}: PNG pixel dimensions disagree with viewport * dpr`);
    }
  }
  if (record.targets && Array.isArray(record.targets.required) && record.targets.required.some((t) => !t.present || !t.visible)) {
    errors.push(`${name}: one or more required scenario targets were missing or invisible`);
  }

  return { ok: errors.length === 0, version: record.schemaVersion, errors };
}

export function validateManifest(manifest, options = {}) {
  const entries = Object.entries(manifest ?? {});
  const errors = [];
  for (const [name, record] of entries) {
    const res = validateManifestRecord(name, record, options);
    if (!res.ok) errors.push(...res.errors);
  }
  return {
    ok: entries.length > 0 && errors.length === 0,
    total: entries.length,
    errors,
  };
}

/**
 * Browser script evaluating document/container horizontal overflow and clipping while keeping
 * allowed internal text-input scrolling distinct from unintended page/container overflow.
 */
export function buildOverflowInspectionScript({
  containers = [],
  targetScopeSelectors = [
    '.vk-ui-bottom-nav a',
    '.vk-ui-rail a, .vk-ui-rail button',
    '.vk-me button, .vk-me a, .vk-me summary',
    '#profile-menu button, #profile-menu a, #profile-menu input:checked',
    '#leave-editor button, #draft-preview button',
    '.vk-editor-recovery button, .vk-editor-recovery summary',
    '.vk-discussion-root button',
  ],
  allowedScrollSelectors = DEFAULT_TEXT_INPUT_SELECTORS,
  tolerancePx = 1,
} = {}) {
  const defaultContainers = [
    'html',
    'body',
    '.vk-ui-scaffold',
    '.vk-ui-scaffold__content',
    '.vk-main',
    '.vk-ui-rail',
    '.vk-ui-bottom-nav',
    '.vk-me',
    '.vk-account',
    '.vk-account-status',
    '.vk-appearance-card',
    '#profile-menu',
    '#leave-editor',
    '#draft-preview',
    '#desk',
    '#explore',
    '#reader-paper',
    '.vk-editor',
    '.vk-editor-paper',
    '[data-testid=save-failed]',
    '[data-testid=conflict]',
    '.vk-discussion-root',
    '.vk-discussion-thread',
    '.vk-feedback-composer',
    ...containers,
  ];
  return `(() => {
    const tol = ${Number(tolerancePx)};
    const root = document.documentElement;
    const body = document.body;
    const allowedExpr = ${JSON.stringify(allowedScrollSelectors.join(','))};
    const seen = new Set();
    const unintendedOverflows = [];
    const allowedInputScrolls = [];
    const describe = (el) => {
      if (el === root) return 'html';
      if (el === body) return 'body';
      if (el.id) return '#' + el.id;
      const cls = [...el.classList].slice(0, 2).join('.');
      return el.tagName.toLowerCase() + (cls ? '.' + cls : '');
    };
    const inspectNode = (el) => {
      if (!el || seen.has(el)) return;
      seen.add(el);
      if (!el.checkVisibility?.({ visibilityProperty: true, opacityProperty: true })) return;
      const style = getComputedStyle(el);
      const sw = el.scrollWidth, cw = el.clientWidth;
      if (cw <= 0 || sw <= cw + tol) return;
      const entry = {
        selector: describe(el),
        scrollWidth: sw,
        clientWidth: cw,
        overflowPx: sw - cw,
        overflowX: style.overflowX,
        hiddenOverflow: style.overflowX === 'hidden' || style.overflowX === 'clip',
      };
      if (el.matches(allowedExpr)) {
        allowedInputScrolls.push({ ...entry, reason: 'text-input-internal-scroll' });
      } else {
        unintendedOverflows.push(entry);
      }
    };
    for (const sel of ${JSON.stringify(defaultContainers)}) {
      for (const el of document.querySelectorAll(sel)) inspectNode(el);
    }
    for (const el of document.querySelectorAll(allowedExpr)) inspectNode(el);
    const clippedTargets = [];
    for (const sel of ${JSON.stringify(targetScopeSelectors)}) {
      for (const el of document.querySelectorAll(sel)) {
        if (!el.checkVisibility?.({ visibilityProperty: true, opacityProperty: true })) continue;
        const r = el.getBoundingClientRect();
        if (r.width === 0 || r.height === 0) continue;
        const clip = { left: 0, right: root.clientWidth };
        for (let node = el.parentElement; node && node !== root; node = node.parentElement) {
          const css = getComputedStyle(node);
          if (css.overflowX !== 'visible') {
            const b = node.getBoundingClientRect();
            const l = b.left + node.clientLeft;
            clip.left = Math.max(clip.left, l);
            clip.right = Math.min(clip.right, l + node.clientWidth);
          }
        }
        if (r.left < clip.left - tol || r.right > clip.right + tol) {
          clippedTargets.push({ selector: describe(el), rect: { left: r.left, right: r.right }, clip });
        }
      }
    }
    return {
      document: {
        scrollX,
        scrollY,
        innerWidth,
        innerHeight,
        scrollWidth: root.scrollWidth,
        clientWidth: root.clientWidth,
        overflowX: getComputedStyle(root).overflowX,
      },
      body: {
        scrollWidth: body ? body.scrollWidth : 0,
        clientWidth: body ? body.clientWidth : 0,
        overflowX: body ? getComputedStyle(body).overflowX : 'visible',
      },
      unintendedOverflows,
      allowedInputScrolls,
      clippedTargets,
    };
  })()`;
}

export function evaluateOverflowReport(report, { tolerancePx = 1 } = {}) {
  const reasons = [];
  if (!report || !report.document) {
    return { ok: false, reasons: ['missing overflow report'], unintendedOverflows: [], clippedTargets: [], allowedInputScrolls: [] };
  }
  const d = report.document;
  if (d.scrollWidth > d.clientWidth + tolerancePx) {
    reasons.push(`document scrollWidth ${d.scrollWidth} > clientWidth ${d.clientWidth} (scrollX=${d.scrollX})`);
  }
  if (d.scrollWidth > d.innerWidth + tolerancePx) {
    reasons.push(`document scrollWidth ${d.scrollWidth} > innerWidth ${d.innerWidth} (scrollX=${d.scrollX})`);
  }
  if (report.body && report.body.scrollWidth > report.body.clientWidth + tolerancePx) {
    reasons.push(`body scrollWidth ${report.body.scrollWidth} > clientWidth ${report.body.clientWidth}`);
  }
  for (const item of report.unintendedOverflows ?? []) {
    reasons.push(
      `container overflow at ${item.selector}: scrollWidth=${item.scrollWidth} > clientWidth=${item.clientWidth} (overflowX=${item.overflowX}, scrollX=${d.scrollX})`,
    );
  }
  for (const target of report.clippedTargets ?? []) {
    reasons.push(
      `clipped target ${target.selector}: [${target.rect.left.toFixed(1)}, ${target.rect.right.toFixed(1)}] outside [${target.clip.left.toFixed(1)}, ${target.clip.right.toFixed(1)}]`,
    );
  }
  return {
    ok: reasons.length === 0,
    reasons,
    unintendedOverflows: report.unintendedOverflows ?? [],
    clippedTargets: report.clippedTargets ?? [],
    allowedInputScrolls: report.allowedInputScrolls ?? [],
  };
}

/**
 * Browser script evaluating a target's visibility, full clipping-ancestor containment,
 * 3-point pointer hit-testability, label fit, focus/disabled state and scroll owner metrics.
 */
export function buildTargetGeometryScript(selector) {
  return `(() => {
    const sel = ${JSON.stringify(selector)};
    const el = document.querySelector(sel);
    const root = document.documentElement;
    const page = { scrollX, scrollY, scrollWidth: root.scrollWidth, clientWidth: root.clientWidth, innerWidth, innerHeight };
    if (!el) return { selector: sel, present: false, visible: false, full: false, hit: false, page };
    const visible = Boolean(el.checkVisibility?.({ visibilityProperty: true, opacityProperty: true }));
    if (!visible) return { selector: sel, present: true, visible: false, full: false, hit: false, page };
    const rect = el.getBoundingClientRect();
    const clip = { left: 0, right: root.clientWidth, top: 0, bottom: innerHeight };
    let owner = document.scrollingElement || root;
    for (let node = el.parentElement; node && node !== root; node = node.parentElement) {
      const css = getComputedStyle(node);
      const bounds = node.getBoundingClientRect();
      const left = bounds.left + node.clientLeft;
      const top = bounds.top + node.clientTop;
      if (css.overflowX !== 'visible') {
        clip.left = Math.max(clip.left, left);
        clip.right = Math.min(clip.right, left + node.clientWidth);
      }
      if (css.overflowY !== 'visible') {
        clip.top = Math.max(clip.top, top);
        clip.bottom = Math.min(clip.bottom, top + node.clientHeight);
      }
      if (owner === (document.scrollingElement || root) && /auto|scroll/.test(css.overflowY)) owner = node;
    }
    const full = rect.width > 0 && rect.height > 0 &&
      rect.left >= clip.left - 1 && rect.right <= clip.right + 1 &&
      rect.top >= clip.top - 1 && rect.bottom <= clip.bottom + 1;
    const x = (rect.left + rect.right) / 2;
    const inset = Math.min(6, rect.height / 4);
    const hit = full && [rect.top + inset, (rect.top + rect.bottom) / 2, rect.bottom - inset]
      .every((y) => {
        const hitEl = document.elementFromPoint(x, y);
        return Boolean(hitEl && el.contains(hitEl));
      });
    const style = getComputedStyle(el);
    const label = el.querySelector('.vk-ui-button__label') ?? el;
    const describedByIds = (el.getAttribute('aria-describedby') || '').split(/\\s+/).filter(Boolean);
    const description = describedByIds.length
      ? describedByIds.map((id) => document.getElementById(id)?.textContent?.trim() ?? '').filter(Boolean).join(' ')
      : null;
    return {
      selector: sel,
      id: el.id || null,
      present: true,
      visible: true,
      full,
      hit,
      rect: rect.toJSON(),
      clip,
      page,
      focused: el === document.activeElement,
      focusRing: el.matches(':focus-visible') && parseFloat(style.outlineWidth) > 0,
      labelFits: label.scrollWidth <= label.clientWidth + 1,
      disabled: el.matches(':disabled,[aria-disabled=true]'),
      ariaDisabled: el.getAttribute('aria-disabled'),
      description,
      owner: {
        id: owner.id || null,
        tagName: owner.tagName,
        scrollTop: owner.scrollTop,
        scrollHeight: owner.scrollHeight,
        clientHeight: owner.clientHeight,
        scrollWidth: owner.scrollWidth,
        clientWidth: owner.clientWidth,
        overflowX: getComputedStyle(owner).overflowX,
        overflowY: getComputedStyle(owner).overflowY,
      },
      scrollWidth: el.scrollWidth,
      clientWidth: el.clientWidth,
    };
  })()`;
}

export function evaluateTargetGeometry(target, { requireFull = true, requireHit = true, minWidth = 0, minHeight = 0 } = {}) {
  const reasons = [];
  if (!target || !target.present) {
    return { ok: false, status: 'INCOMPLETE', reasons: [`missing target ${target?.selector ?? '<unknown>'}`] };
  }
  if (!target.visible) reasons.push(`hidden target ${target.selector}`);
  if (requireFull && !target.full) {
    reasons.push(`clipped or out-of-bounds target ${target.selector}: rect=${JSON.stringify(target.rect)} clip=${JSON.stringify(target.clip)}`);
  }
  if (requireHit && !target.hit) reasons.push(`target ${target.selector} is occluded or not hit-testable`);
  if (!target.labelFits) reasons.push(`target ${target.selector} label overflows its box`);
  if (target.rect && (target.rect.width < minWidth || target.rect.height < minHeight)) {
    reasons.push(`target ${target.selector} below ${minWidth}x${minHeight}: ${target.rect.width}x${target.rect.height}`);
  }
  if (target.page && target.page.scrollWidth > target.page.clientWidth + 1) {
    reasons.push(`page horizontal overflow (${target.page.scrollWidth} > ${target.page.clientWidth})`);
  }
  if (target.owner && target.owner.scrollWidth > target.owner.clientWidth + 1) {
    reasons.push(`scroll owner ${target.owner.id || target.owner.tagName} horizontal overflow (${target.owner.scrollWidth} > ${target.owner.clientWidth})`);
  }
  return { ok: reasons.length === 0, status: reasons.length === 0 ? 'PASS' : 'FAIL', reasons };
}

/**
 * Extracts normalized summary from a Chrome DevTools Protocol AXNode.
 */
export function extractAxNodeSummary(axNode) {
  if (!axNode) {
    return { ignored: true, role: null, name: null, description: null, states: {} };
  }
  const states = {};
  for (const prop of axNode.properties ?? []) {
    states[prop.name] = prop.value?.value;
  }
  return {
    nodeId: axNode.nodeId,
    ignored: Boolean(axNode.ignored),
    role: axNode.role?.value ?? null,
    name: axNode.name?.value ?? null,
    description: axNode.description?.value ?? null,
    states,
    nameSources: (axNode.name?.sources ?? []).map((s) => ({
      type: s.type,
      attribute: s.attribute ?? null,
      value: s.value?.value ?? null,
    })),
  };
}

/**
 * Queries a DOM element's raw text/attributes alongside Chrome's computed AX tree node.
 */
export async function collectElementAccessibility(page, selector) {
  const dom = await page.eval(`(() => {
    const el = document.querySelector(${JSON.stringify(selector)});
    if (!el) return { present: false };
    return {
      present: true,
      tagName: el.tagName,
      id: el.id || null,
      href: el.getAttribute('href'),
      rawTextContent: el.textContent,
      ariaLabel: el.getAttribute('aria-label'),
      ariaCurrent: el.getAttribute('aria-current'),
      ariaHidden: el.getAttribute('aria-hidden'),
      ariaDisabled: el.getAttribute('aria-disabled'),
      disabled: Boolean(el.disabled),
    };
  })()`);
  if (!dom.present) {
    return { selector, present: false, rawTextContent: null, ax: extractAxNodeSummary(null), dom };
  }
  const { result } = await page.send('Runtime.evaluate', {
    expression: `document.querySelector(${JSON.stringify(selector)})`,
    returnByValue: false,
  });
  let axNode = null;
  try {
    if (result?.objectId) {
      const partial = await page.send('Accessibility.getPartialAXTree', {
        objectId: result.objectId,
        fetchRelatives: false,
      });
      axNode = partial.nodes?.[0] ?? null;
    }
  } finally {
    if (result?.objectId) {
      await page.send('Runtime.releaseObject', { objectId: result.objectId }).catch(() => {});
    }
  }
  return {
    selector,
    present: true,
    rawTextContent: dom.rawTextContent,
    ax: extractAxNodeSummary(axNode),
    dom,
  };
}

export function evaluateAccessibleElement(record, expected = {}) {
  const reasons = [];
  if (!record || !record.present) {
    return { ok: false, reasons: [`missing accessible element ${record?.selector ?? ''}`] };
  }
  if (record.ax?.ignored) {
    reasons.push(`${record.selector}: element is ignored in Chrome AX tree`);
  }
  if (expected.role !== undefined && record.ax?.role !== expected.role) {
    reasons.push(`${record.selector}: AX role expected ${expected.role}, got ${record.ax?.role}`);
  }
  if (expected.name !== undefined && record.ax?.name !== expected.name) {
    reasons.push(
      `${record.selector}: AX name expected ${JSON.stringify(expected.name)}, got ${JSON.stringify(record.ax?.name)} (rawTextContent=${JSON.stringify(record.rawTextContent)})`,
    );
  }
  for (const forbidden of expected.excludeSubstrings ?? []) {
    if (typeof record.ax?.name === 'string' && record.ax.name.includes(forbidden)) {
      reasons.push(`${record.selector}: AX name ${JSON.stringify(record.ax.name)} leaked forbidden substring ${JSON.stringify(forbidden)}`);
    }
  }
  if (expected.ariaCurrent !== undefined && record.dom?.ariaCurrent !== expected.ariaCurrent) {
    reasons.push(`${record.selector}: aria-current expected ${JSON.stringify(expected.ariaCurrent)}, got ${JSON.stringify(record.dom?.ariaCurrent)}`);
  }
  if (expected.disabled !== undefined && Boolean(record.ax?.states?.disabled) !== expected.disabled) {
    reasons.push(`${record.selector}: AX disabled expected ${expected.disabled}, got ${record.ax?.states?.disabled}`);
  }
  return { ok: reasons.length === 0, reasons };
}

/**
 * Browser script inspecting nav item targets, icon centres, Range first-line coordinates,
 * actual `.vk-ui-nav-item__label` computed fonts (separate from container font), line counts and
 * the bottom-bar height CSS variable observer.
 */
export function buildNavInspectionScript(containerSelector = '.vk-ui-bottom-nav') {
  return `(() => {
    const container = document.querySelector(${JSON.stringify(containerSelector)});
    if (!container || !container.getClientRects().length) {
      return { present: false, containerSelector: ${JSON.stringify(containerSelector)}, items: [] };
    }
    const containerRect = container.getBoundingClientRect();
    const containerStyle = getComputedStyle(container);
    const segmenter = typeof Intl !== 'undefined' && Intl.Segmenter ? new Intl.Segmenter('vi', { granularity: 'word' }) : null;
    const items = [...container.querySelectorAll('.vk-ui-nav-item')].map((item) => {
      const target = item.getBoundingClientRect();
      const iconEl = item.querySelector('.vk-ui-icon');
      const icon = iconEl ? iconEl.getBoundingClientRect() : null;
      const labelEl = item.querySelector('.vk-ui-nav-item__label') ?? item;
      const labelRect = labelEl.getBoundingClientRect();
      const labelStyle = getComputedStyle(labelEl);
      const range = document.createRange();
      range.selectNodeContents(labelEl);
      const lineRects = [...range.getClientRects()].filter((r) => r.width > 0 && r.height > 0);
      const firstLine = lineRects[0] ?? null;
      const lineCount = new Set(lineRects.map((r) => Math.round(r.top))).size;
      let wordsIntact = true;
      const walker = document.createTreeWalker(labelEl, NodeFilter.SHOW_TEXT);
      for (let node = walker.nextNode(); node; node = walker.nextNode()) {
        const text = node.textContent || '';
        const chunks = segmenter
          ? [...segmenter.segment(text)].filter((s) => s.isWordLike).map((s) => [s.index, s.index + s.segment.length])
          : [...text.matchAll(/\\S+/g)].map((m) => [m.index, m.index + m[0].length]);
        for (const [start, end] of chunks) {
          const wr = document.createRange();
          wr.setStart(node, start);
          wr.setEnd(node, end);
          const tops = new Set([...wr.getClientRects()].filter((q) => q.width > 0).map((q) => Math.round(q.top)));
          if (tops.size > 1) wordsIntact = false;
        }
      }
      return {
        id: item.id || null,
        href: item.getAttribute('href'),
        ariaCurrent: item.getAttribute('aria-current'),
        labelText: labelEl.textContent,
        rawTextContent: item.textContent,
        x: target.x,
        y: target.y,
        width: target.width,
        height: target.height,
        iconCenterX: icon ? icon.x + icon.width / 2 : null,
        iconCenterY: icon ? icon.y + icon.height / 2 : null,
        firstLineX: firstLine ? firstLine.x : null,
        firstLineY: firstLine ? firstLine.y : null,
        labelBoxHeight: labelRect.height,
        lineCount,
        wordsIntact,
        labelScrollWidth: labelEl.scrollWidth,
        labelClientWidth: labelEl.clientWidth,
        labelFont: labelStyle.font,
        labelFontFamily: labelStyle.fontFamily,
        labelFontSize: labelStyle.fontSize,
        labelLineHeight: labelStyle.lineHeight,
        containerFont: containerStyle.font,
        containerFontSize: containerStyle.fontSize,
      };
    });
    const scaffold = document.querySelector('.vk-ui-scaffold');
    const barVar = scaffold ? parseFloat(getComputedStyle(scaffold).getPropertyValue('--vk-ui-bar-height')) : null;
    return {
      present: true,
      containerSelector: ${JSON.stringify(containerSelector)},
      containerRect: containerRect.toJSON(),
      containerFont: containerStyle.font,
      containerFontSize: containerStyle.fontSize,
      barHeightObserver: Number.isFinite(barVar)
        ? { cssVarHeight: barVar, measuredHeight: containerRect.height, delta: Math.abs(barVar - containerRect.height) }
        : null,
      items,
    };
  })()`;
}

/**
 * Evaluates nav alignment via icon centres and Range first-line Y, intentionally allowing
 * 1-line and 2-line labels to have different total label box heights.
 */
export function evaluateNavAlignment(
  items,
  {
    tolerancePx = 1,
    maxLines = 2,
    checkSameRow = true,
    expectedCount = 3,
    requireConsistentFont = true,
  } = {},
) {
  const reasons = [];
  if (!Array.isArray(items) || items.length !== expectedCount) {
    return { ok: false, reasons: [`expected ${expectedCount} nav items, got ${items?.length ?? 0}`] };
  }
  const spread = (key) => {
    const vals = items.map((item) => item[key]);
    if (vals.some((v) => !Number.isFinite(v))) return Infinity;
    return Math.max(...vals) - Math.min(...vals);
  };
  const maxIconCenterDelta = spread('iconCenterY');
  const maxFirstLineDelta = spread('firstLineY');
  const maxTargetYDelta = spread('y');
  const maxTargetHeightDelta = spread('height');

  if (!Number.isFinite(maxIconCenterDelta) || maxIconCenterDelta > tolerancePx) {
    reasons.push(`iconCenterY spread ${maxIconCenterDelta}px exceeds ${tolerancePx}px`);
  }
  if (checkSameRow && (!Number.isFinite(maxFirstLineDelta) || maxFirstLineDelta > tolerancePx)) {
    reasons.push(`firstLineY spread ${maxFirstLineDelta}px exceeds ${tolerancePx}px`);
  }
  if (checkSameRow && (!Number.isFinite(maxTargetYDelta) || maxTargetYDelta > tolerancePx)) {
    reasons.push(`target y spread ${maxTargetYDelta}px exceeds ${tolerancePx}px`);
  }
  if (checkSameRow && (!Number.isFinite(maxTargetHeightDelta) || maxTargetHeightDelta > tolerancePx)) {
    reasons.push(`target height spread ${maxTargetHeightDelta}px exceeds ${tolerancePx}px`);
  }
  for (const item of items) {
    if (Number.isFinite(item.lineCount) && item.lineCount > maxLines) {
      reasons.push(`nav label ${JSON.stringify(item.labelText)} wrapped to ${item.lineCount} lines (max ${maxLines})`);
    }
    if (item.wordsIntact === false) {
      reasons.push(`nav label ${JSON.stringify(item.labelText)} broke a Vietnamese word mid-word`);
    }
    if (Number.isFinite(item.labelScrollWidth) && Number.isFinite(item.labelClientWidth) && item.labelScrollWidth > item.labelClientWidth + 1) {
      reasons.push(`nav label ${JSON.stringify(item.labelText)} overflows horizontally (${item.labelScrollWidth} > ${item.labelClientWidth})`);
    }
  }
  if (requireConsistentFont) {
    const base = items[0];
    for (const item of items.slice(1)) {
      if (item.labelFont !== base.labelFont || item.labelLineHeight !== base.labelLineHeight) {
        reasons.push(`label font mismatch on ${item.labelText}: ${item.labelFont} / ${item.labelLineHeight} vs ${base.labelFont} / ${base.labelLineHeight}`);
      }
    }
  }
  return {
    ok: reasons.length === 0,
    reasons,
    maxIconCenterDelta,
    maxFirstLineDelta,
    maxTargetYDelta,
    labelHeights: items.map((i) => i.labelBoxHeight ?? null),
    lineCounts: items.map((i) => i.lineCount ?? null),
  };
}

/**
 * Browser script inspecting a modal's scroll box and every applicable action inside it.
 */
export function buildModalInspectionScript(modalSelector = '#leave-editor') {
  return `(() => {
    const d = document.querySelector(${JSON.stringify(modalSelector)});
    if (!d) return { selector: ${JSON.stringify(modalSelector)}, present: false, open: false, actions: [] };
    const rect = d.getBoundingClientRect();
    const root = document.documentElement;
    const actions = [...d.querySelectorAll('button, a[href], summary')].map((el) => {
      const r = el.getBoundingClientRect();
      const s = getComputedStyle(el);
      const visible = Boolean(el.checkVisibility?.({ visibilityProperty: true, opacityProperty: true }));
      const full = visible && r.width > 0 && r.height > 0 &&
        r.left >= Math.max(0, rect.left) - 1 && r.right <= Math.min(root.clientWidth, rect.right) + 1 &&
        r.top >= Math.max(0, rect.top) - 1 && r.bottom <= Math.min(innerHeight, rect.bottom) + 1;
      const x = (r.left + r.right) / 2;
      const inset = Math.min(6, r.height / 4);
      const hit = full && [r.top + inset, (r.top + r.bottom) / 2, r.bottom - inset]
        .every((y) => {
          const hitEl = document.elementFromPoint(x, y);
          return Boolean(hitEl && el.contains(hitEl));
        });
      const ids = (el.getAttribute('aria-describedby') || '').split(/\\s+/).filter(Boolean);
      const disabledReason = ids.map((id) => document.getElementById(id)?.textContent?.trim() ?? '').filter(Boolean).join(' ') || null;
      return {
        id: el.id || null,
        tagName: el.tagName,
        text: el.textContent.trim(),
        classes: [...el.classList],
        disabled: el.matches(':disabled,[aria-disabled=true]'),
        ariaDisabled: el.getAttribute('aria-disabled'),
        disabledReason,
        visible,
        full,
        hit,
        focused: el === document.activeElement,
        focusRing: el.matches(':focus-visible') && parseFloat(s.outlineWidth) > 0,
        rect: r.toJSON(),
      };
    });
    return {
      selector: ${JSON.stringify(modalSelector)},
      present: true,
      open: Boolean(d.open),
      isModal: d.matches(':modal'),
      rect: rect.toJSON(),
      scroll: {
        scrollTop: d.scrollTop,
        scrollHeight: d.scrollHeight,
        clientHeight: d.clientHeight,
        scrollWidth: d.scrollWidth,
        clientWidth: d.clientWidth,
      },
      actions,
    };
  })()`;
}

export function evaluateLeaveModalBranch(modalReport, branchName) {
  const spec = LEAVE_MODAL_BRANCHES[branchName];
  if (!spec) {
    return { ok: false, reasons: [`unknown leave modal branch: ${branchName}`] };
  }
  const reasons = [];
  if (!modalReport || !modalReport.present || !modalReport.open) {
    return { ok: false, reasons: [`leave modal is not open for branch ${branchName}`] };
  }
  const actualIds = (modalReport.actions ?? []).map((a) => a.id);
  if (actualIds.join(',') !== spec.expectedIds.join(',')) {
    reasons.push(
      `branch ${branchName} (canDiscard=${spec.canDiscard}) expected actions ${JSON.stringify(spec.expectedIds)}, got ${JSON.stringify(actualIds)}`,
    );
  }
  if (!modalReport.scroll || !Number.isFinite(modalReport.scroll.scrollHeight) || !Number.isFinite(modalReport.scroll.clientHeight)) {
    reasons.push('missing modal scrollHeight/clientHeight metrics');
  } else if (modalReport.scroll.scrollWidth > modalReport.scroll.clientWidth + 1) {
    reasons.push(`modal horizontal overflow ${modalReport.scroll.scrollWidth} > ${modalReport.scroll.clientWidth}`);
  }
  for (const action of modalReport.actions ?? []) {
    const expectDisabled = spec.disabledIds.includes(action.id);
    if (Boolean(action.disabled) !== expectDisabled) {
      reasons.push(`action ${action.id} disabled expected=${expectDisabled}, got=${action.disabled}`);
    }
    if (expectDisabled && !action.disabledReason) {
      reasons.push(`guard-disabled action ${action.id} is missing aria-describedby reason`);
    }
  }
  return { ok: reasons.length === 0, canDiscard: spec.canDiscard, reasons };
}

/**
 * Evaluates Me account card illustration visibility and `padding-inline-end: 225px` reservation.
 * `action === null` on `/toi` is normal and never treated as a bug when the illustration explains
 * the reserved end space.
 */
export function evaluateMePanelReservation(mePanel) {
  const reasons = [];
  if (!mePanel) {
    return { ok: false, reasons: ['missing mePanel'] };
  }
  if (mePanel.prefix !== 'page') {
    return { ok: true, reasons: [], note: 'overlay utility omits account card' };
  }
  const ill = mePanel.illustration;
  const res = mePanel.reservation;
  if (!ill || !ill.present) {
    reasons.push('missing .vk-account-illustration element on /toi');
    return { ok: false, reasons };
  }
  if (!res || !Number.isFinite(res.paddingInlineEnd)) {
    reasons.push('missing mePanel.reservation metrics');
    return { ok: false, reasons };
  }
  if (ill.ariaHidden !== 'true') {
    reasons.push(`illustration aria-hidden expected 'true', got ${JSON.stringify(ill.ariaHidden)}`);
  }
  if (ill.visible) {
    if (!ill.rect || ill.rect.width <= 0 || ill.rect.height <= 0) {
      reasons.push('visible illustration has empty rect');
    } else if (res.paddingInlineEnd < ill.rect.width) {
      reasons.push(`accountText paddingInlineEnd (${res.paddingInlineEnd}px) < visible illustration width (${ill.rect.width}px)`);
    }
    if (res.textOverlapsIllustration) {
      reasons.push('accountText overlaps visible .vk-account-illustration');
    }
    if (!Number.isFinite(res.accountTextContentWidth) || res.accountTextContentWidth <= 0) {
      reasons.push(`invalid accountText contentWidth: ${res.accountTextContentWidth}`);
    }
  } else if (res.paddingInlineEnd > 1) {
    reasons.push(`unused reservation: paddingInlineEnd=${res.paddingInlineEnd}px while illustration is hidden`);
  }
  return {
    ok: reasons.length === 0,
    reasons,
    illustrationVisible: ill.visible,
    paddingInlineEnd: res.paddingInlineEnd,
    accountTextContentWidth: res.accountTextContentWidth,
  };
}

/**
 * Builds an honest boundary/lane summary without promoting partial, DOM-override or unavailable
 * lanes to full acceptance.
 */
export function buildLaneSummary({
  phase,
  captures = {},
  lanes = {},
  negativeControls = {},
  fullWebVerify = 'NOT_RUN',
} = {}) {
  const records = Object.values(captures);
  const incomplete = Object.entries(captures)
    .filter(([, r]) => r.captureStatus && r.captureStatus !== 'CAPTURED')
    .map(([name, r]) => ({ name, captureStatus: r.captureStatus, reason: r.targetError ?? null }));
  return {
    schemaVersion: MANIFEST_SCHEMA_VERSION,
    phase,
    fullWebVerify,
    captureCounts: {
      total: records.length,
      captured: records.filter((r) => (r.captureStatus ?? 'CAPTURED') === 'CAPTURED').length,
      incomplete: incomplete.length,
    },
    incomplete,
    negativeControls,
    lanes: {
      appAppearanceControl: lanes.appAppearanceControl ?? 'NOT_RUN',
      domOverrideGeometryOnly: lanes.domOverrideGeometryOnly ?? 'NOT_RUN',
      browserZoom: { status: 'NOT_RUN', prerequisite: 'explicit browser page zoom / Page.setPageScaleFactor lane' },
      textOnlyResize: { status: 'NOT_RUN', prerequisite: 'browser text-only resize setting separate from CSS root fontSize override' },
      osTextScaleAndIme: { status: 'NOT_RUN', prerequisite: 'real OS font scale and real macOS/Android/iOS Telex/VNI IME' },
      screenReader: { status: 'NOT_RUN', prerequisite: 'real VoiceOver / NVDA / TalkBack; Chrome AX tree covers structural AX names only' },
      realDevice: { status: 'NOT_RUN', prerequisite: 'physical mobile device or native host' },
    },
  };
}
