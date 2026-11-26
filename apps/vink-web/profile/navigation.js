const returnKey = 'vinkProfileReturn';
const feedReturnKey = 'vinkFeedReturn';
const aliasFocusKey = 'vinkProfileAliasFocus';
const recheckKey = 'vinkProfileRecheck';
let pendingDestination;

const savedReturnKey = 'vinkSavedReturnEntry';
const savedReturns = new Map();

// Only an opaque entry marker goes into history. Work, scope and position live in this
// document's memory. No cursor or previous response travels with the entry.
export function rememberSavedReturn(path, scope, work, pages) {
  const anchor = document.getElementById('saved-open-' + work);
  if (path !== location.pathname || !anchor || !globalThis.crypto?.randomUUID) return;
  const entry = crypto.randomUUID();
  savedReturns.set(entry, { path, scope, work, pages, top: anchor.getBoundingClientRect().top });
  if (savedReturns.size > 16) savedReturns.delete(savedReturns.keys().next().value);
  history.replaceState({ ...history.state, [savedReturnKey]: entry }, '');
}

export function fenceSavedReturns(scope) {
  for (const [key, item] of savedReturns) {
    if (item.scope !== scope) savedReturns.delete(key);
  }
}

function savedPosition(path, scope) {
  if (path !== location.pathname || !scope) return undefined;
  const item = savedReturns.get(history.state?.[savedReturnKey]);
  return item?.path === path && item.scope === scope ? item : undefined;
}

export function savedReturnPages(path, scope) {
  return savedPosition(path, scope)?.pages ?? 0;
}

// Called by the current route only after its owner check and (when authorized)
// a new cursor chain. An absent or neutral projection has no reader anchor.
export function focusSavedReturn(path, scope) {
  const entry = history.state?.[savedReturnKey];
  if (!entry || path !== location.pathname) return;
  requestAnimationFrame(() => {
    if (location.pathname !== path || entry !== history.state?.[savedReturnKey]) return;
    const confirmed = savedPosition(path, scope);
    const active = document.activeElement;
    const candidate = confirmed && document.getElementById('saved-open-' + confirmed.work);
    const anchor = candidate?.getAttribute('href') === '/tac-pham/' + confirmed?.work ? candidate : null;
    const {[savedReturnKey]: _, ...state} = history.state;
    history.replaceState(state, '');
    savedReturns.delete(entry);
    if (active !== document.body && active?.id !== 'noi-dung' && active?.id !== 'not-found-title') return;
    if (anchor) {
      window.scrollBy({top: anchor.getBoundingClientRect().top - confirmed.top, behavior: 'instant'});
      anchor.focus({preventScroll: true});
    } else {
      const heading = document.getElementById('saved-heading') || document.getElementById('not-found-title');
      heading?.focus({preventScroll: true});
      heading?.scrollIntoView({block: 'nearest', behavior: 'instant'});
    }
  });
}


const address = () => location.pathname + location.search + location.hash;

// Back can change the active history entry while an SSR document waits for WASM.
// Restart at the current URL before mounting a router against another route's DOM.
export function canHydrateCurrentEntry(fingerprint) {
  const rendered = document.documentElement.dataset.renderedRoute;
  if (!rendered || rendered === fingerprint) return true;
  const main = document.getElementById('noi-dung');
  if (main) {
    main.style.visibility = 'hidden';
    main.inert = true;
  }
  history.replaceState({...history.state, [recheckKey]: address()}, '');
  location.reload();
  return false;
}

export function replaceProfileAlias(sourcePath, href) {
  const target = new URL(href, location.origin);
  if (target.origin !== location.origin) return;
  const expected = pendingDestination;
  const source = expected && new URL(expected.href, location.origin);
  const current = address();
  const fromLink = source?.pathname === sourcePath &&
    (expected.source === current || expected.href === current);
  // A resource from an abandoned route can finish before the next URL is committed.
  if (!fromLink && (location.pathname !== sourcePath ||
      (expected?.source === current && expected.href !== current))) return;
  target.hash = fromLink ? source.hash : location.hash;
  if (fromLink && expected.heading) {
    try {
      sessionStorage.setItem(aliasFocusKey, JSON.stringify({
        address: target.pathname + target.search + target.hash,
        heading: expected.heading,
        expires: Date.now() + 10000,
      }));
    } catch { /* Navigation still works with storage disabled. */ }
  }
  // Blocking route data can resolve before the router commits the alias URL. The prior
  // document may already have the canonical address. Force a clean document in that case.
  if (target.pathname === location.pathname && target.search === location.search) {
    history.replaceState(history.state, '', target.href);
    location.reload();
  } else {
    location.replace(target.href);
  }
}

// A cursor is bound by the provider to its author, direction and read scope. It is only a
// position hint. The list owner reads it again before this module restores the anchor.
export function relationshipReturnCursor(path) {
  const entry = history.state?.[returnKey];
  if (location.pathname !== path || entry?.source !== address() ||
      !entry.id.startsWith('rel-person-') || typeof entry.cursor !== 'string' ||
      !entry.cursor.length || entry.cursor.length > 1024) return undefined;
  return entry.cursor;
}

export function focusRelationshipReturn(cursor) {
  const entry = history.state?.[returnKey];
  const path = location.pathname;
  if (relationshipReturnCursor(path) !== cursor) return;
  requestAnimationFrame(() => {
    const current = history.state?.[returnKey];
    if (current?.id !== entry.id || relationshipReturnCursor(path) !== cursor) return;
    const active = document.activeElement;
    if (active !== document.body && !['noi-dung', 'relationship-title'].includes(active?.id)) return;
    const node = document.getElementById(entry.id);
    const anchor = node?.getAttribute('href') === entry.href ? node :
      document.getElementById('relationship-title');
    anchor?.scrollIntoView({block: 'nearest'});
    anchor?.focus({preventScroll: true});
  });
}

export function authorSearchReturnCursor() {
  const entry = history.state?.[returnKey];
  if (entry?.source !== address() || !entry.id.startsWith('search-author-') ||
      typeof entry.cursor !== 'string' || !entry.cursor.length || entry.cursor.length > 80) return undefined;
  return entry.cursor;
}

export function hasProfileReturn() {
  return history.state?.[returnKey]?.source === address();
}

// The private Feed rechecks this position with its current viewer before rendering the row.
export function profileReturnWork() {
  const entry = history.state?.[returnKey];
  if (entry?.source !== address()) return undefined;
  return /^work-([0-9a-f-]{36})-author$/.exec(entry.id)?.[1];
}

export function profileReturnHref() {
  return hasProfileReturn() ? history.state[returnKey].href : undefined;
}

export function rememberFeedReturn(work, scope) {
  history.replaceState({...history.state, [feedReturnKey]: {work, scope}}, '');
}

export function feedReturnScope(work) {
  const hint = history.state?.[feedReturnKey];
  return hint?.work === work && typeof hint.scope === 'string' &&
    /^[0-9a-f]{64}$/.test(hint.scope) ? hint.scope : undefined;
}

export function clearProfileReturn() {
  if (!hasProfileReturn()) return;
  const {[returnKey]: _, ...rest} = history.state;
  history.replaceState(rest, '');
}

// Focus belongs to the traversed history entry and to a still-present, validated link.
export function mountProfileNavigation() {
  let cancelFocus = () => {};

  function focusWhenReady(expected, find, fallback, source) {
    cancelFocus();
    let timer;
    let frame;
    const observer = new MutationObserver(schedule);
    const cancel = () => {
      observer.disconnect();
      clearTimeout(timer);
      cancelAnimationFrame(frame);
      document.removeEventListener('pointerdown', cancel, true);
      document.removeEventListener('keydown', cancel, true);
    };
    cancelFocus = cancel;

    function apply(failed = false) {
      if (address() !== expected) {
        // Navigation may wait for a suspense resource before it commits the URL.
        if (failed) cancel();
        return;
      }
      const target = find() || (failed && document.getElementById(fallback));
      if (!target) {
        if (failed) cancel();
        return;
      }
      const active = document.activeElement;
      if (active !== target && (active === document.body || active?.id === 'noi-dung' || active === source)) {
        target.scrollIntoView({block: 'nearest'});
        target.focus({preventScroll: true});
      } else if (active !== target) {
        cancel();
        return;
      }
      // A revalidation can replace the focused node. Continue while this entry is
      // current and there has been no new input.
      if (failed) cancel();
    }

    function schedule() {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => apply());
    }
    observer.observe(document.getElementById('noi-dung'), {subtree: true, childList: true,
      attributes: true, attributeFilter: ['href']});
    document.addEventListener('pointerdown', cancel, true);
    document.addEventListener('keydown', cancel, true);
    timer = setTimeout(() => apply(true), 4000);
    schedule();
  }

  document.addEventListener('click', event => {
    if (event.defaultPrevented || event.button !== 0 || event.altKey || event.metaKey ||
        event.ctrlKey || event.shiftKey) return;
    const link = event.target.closest?.('a[href]');
    if (!link || link.target || link.origin !== location.origin || link.hasAttribute('download')) return;
    const source = address();
    const target = new URL(link.href);
    const href = target.pathname + target.search + target.hash;
    const preserve = link.matches('[data-profile-return][id]');
    const heading = !preserve ? '' :
      /^\/@[^/]+\/(nguoi-theo-doi|dang-theo-doi)(?:[?#]|$)/.test(href) ? 'relationship-title' :
      href.startsWith('/@') ? 'author-profile-heading' :
      href === '/toi/chinh-ho-so' ? 'edit-profile-heading' :
      href === '/toi/chinh-ho-so#quyen-rieng-tu' ? 'quyen-rieng-tu' :
      href === '/toi#edit-profile-link' ? 'edit-profile-link' : '';
    pendingDestination = {href, heading, source};
    if (!preserve) return;
    const cursor = link.id.startsWith('rel-person-') ? link.closest('li')?.dataset.relCursor :
      link.id.startsWith('search-author-') ? link.closest('li')?.dataset.authorCursor : undefined;
    history.replaceState({...history.state, [returnKey]: {id: link.id, href, source, cursor}}, '');
    if (heading) focusWhenReady(href, () => document.getElementById(heading), '', link);
  });

  function restore() {
    cancelFocus();
    const entry = history.state?.[returnKey];
    if (!entry || entry.source !== address() || relationshipReturnCursor(location.pathname)) return;
    // The private Feed owns its byline return and waits for an accepted scoped read.
    if (location.pathname === '/' && new URLSearchParams(location.search).get('mode') === 'following' &&
        profileReturnWork()) return;
    const fallback = entry.id.startsWith('profile-work-') ? 'author-works-heading' :
      entry.id.startsWith('search-author-') ? 'author-search-results-heading' :
      entry.id.startsWith('work-') ? 'community-stream' :
      entry.id.startsWith('comment-') ? 'feedback-heading' :
      entry.id.startsWith('rel-person-') || ['rel-back-profile', 'rel-other-direction', 'rel-first-page'].includes(entry.id) ? 'relationship-title' :
      entry.id === 'profile-followers-link' || entry.id === 'profile-following-link' || entry.id === 'own-profile-edit-link' || entry.id === 'follow-sign-in' ? 'author-profile-heading' :
      ['edit-profile-link', 'view-profile-link', 'profile-privacy-link'].includes(entry.id) ? 'me-title' : 'noi-dung';
    focusWhenReady(entry.source, () => {
      const node = document.getElementById(entry.id);
      return node?.getAttribute('href') === entry.href ? node : null;
    }, fallback);
  }

  const viewport = window.visualViewport;
  if (viewport) {
    const updateKeyboardInset = () => {
      const inset = viewport.scale === 1 ? Math.max(0, innerHeight - viewport.height - viewport.offsetTop) : 0;
      document.documentElement.style.setProperty('--vk-profile-keyboard-inset', `${inset}px`);
    };
    viewport.addEventListener('resize', updateKeyboardInset);
    viewport.addEventListener('scroll', updateKeyboardInset);
    updateKeyboardInset();
  }

  let parked = false;
  let reloading = false;
  window.addEventListener('pagehide', event => {
    const path = location.pathname;
    if (!event.persisted || !(path === '/' || path === '/toi' || path === '/toi/chinh-ho-so' ||
        ['/the/', '/tac-pham/', '/@'].some(prefix => path.startsWith(prefix)))) return;
    parked = true;
    const main = document.getElementById('noi-dung');
    main.style.visibility = 'hidden';
    main.inert = true;
  });
  let aliasEntryFocus = false;
  function resume(event) {
    if (event.type === 'popstate') pendingDestination = undefined;
    if (event.type === 'pageshow' && !event.persisted && aliasEntryFocus) return;
    // A document can have several SPA history entries. The entry being restored need
    // not be the entry that was current when the document was frozen.
    if (!parked) {
      restore();
    } else if (!reloading) {
      reloading = true;
      history.replaceState({...history.state, [recheckKey]: address()}, '');
      location.reload();
    }
  }
  window.addEventListener('popstate', resume);
  window.addEventListener('pageshow', resume);
  // A frozen document has an old projection even when a tab's RAM freshness window has
  // not elapsed. The fresh HTTP document may re-focus the surviving link.
  let aliasFocus;
  try {
    aliasFocus = JSON.parse(sessionStorage.getItem(aliasFocusKey));
    sessionStorage.removeItem(aliasFocusKey);
  } catch { /* No focus handoff available. */ }
  if (aliasFocus?.address === address() && aliasFocus.expires >= Date.now()) {
    aliasEntryFocus = true;
    focusWhenReady(address(), () => document.getElementById(aliasFocus.heading), '');
  }
  const state = history.state;
  if (state?.[recheckKey] === address()) {
    const {[recheckKey]: _, ...rest} = state;
    history.replaceState(rest, '');
    restore();
  } else if (performance.getEntriesByType('navigation')[0]?.type === 'back_forward') {
    restore();
  }
}
