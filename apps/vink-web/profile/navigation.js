const returnKey = 'vinkProfileReturn';
const aliasFocusKey = 'vinkProfileAliasFocus';
let pendingDestination;

const address = () => location.pathname + location.search + location.hash;

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

export function hasProfileReturn() {
  return history.state?.[returnKey]?.source === address();
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
      href === '/toi#edit-profile-link' ? 'edit-profile-link' : '';
    pendingDestination = {href, heading, source};
    if (!preserve) return;
    const cursor = link.id.startsWith('rel-person-') ? link.closest('li')?.dataset.relCursor : undefined;
    history.replaceState({...history.state, [returnKey]: {id: link.id, href, source, cursor}}, '');
    if (heading) focusWhenReady(href, () => document.getElementById(heading), '', link);
  });

  function restore() {
    cancelFocus();
    const entry = history.state?.[returnKey];
    if (!entry || entry.source !== address() || relationshipReturnCursor(location.pathname)) return;
    const fallback = entry.id.startsWith('profile-work-') ? 'author-works-heading' :
      entry.id.startsWith('work-') ? 'community-stream' :
      entry.id.startsWith('comment-') ? 'feedback-heading' :
      entry.id.startsWith('rel-person-') || ['rel-back-profile', 'rel-other-direction', 'rel-first-page'].includes(entry.id) ? 'relationship-title' :
      entry.id === 'profile-followers-link' || entry.id === 'profile-following-link' || entry.id === 'own-profile-edit-link' || entry.id === 'follow-sign-in' ? 'author-profile-heading' :
      entry.id === 'edit-profile-link' ? 'me-title' : 'noi-dung';
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

  const recheck = 'vinkProfileRecheck';
  let parked = false;
  let reloading = false;
  window.addEventListener('pagehide', event => {
    const path = location.pathname;
    if (!event.persisted || !(path === '/' || path === '/toi/chinh-ho-so' ||
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
      history.replaceState({...history.state, [recheck]: address()}, '');
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
  if (state?.[recheck] === address()) {
    const {[recheck]: _, ...rest} = state;
    history.replaceState(rest, '');
    restore();
  } else if (performance.getEntriesByType('navigation')[0]?.type === 'back_forward') {
    restore();
  }
}
