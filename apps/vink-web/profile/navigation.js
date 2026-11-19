// Focus belongs to the traversed history entry and to a still-present, validated link.
export function mountProfileNavigation() {
  const stateKey = 'vinkProfileReturn';
  let cancelFocus = () => {};
  const address = () => location.pathname + location.search + location.hash;

  function focusWhenReady(expected, find, fallback, source) {
    cancelFocus();
    let timer;
    let frame;
    const observer = new MutationObserver(schedule);
    const cancel = () => {
      observer.disconnect();
      clearTimeout(timer);
      cancelAnimationFrame(frame);
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
      if (active === document.body || active?.id === 'noi-dung' || active === source || active === target) {
        target.scrollIntoView({block: 'nearest'});
        target.focus({preventScroll: true});
      }
      cancel();
    }

    function schedule() {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => apply());
    }
    observer.observe(document.getElementById('noi-dung'), {subtree: true, childList: true});
    timer = setTimeout(() => apply(true), 4000);
    schedule();
  }

  document.addEventListener('click', event => {
    if (event.defaultPrevented || event.button !== 0 || event.altKey || event.metaKey ||
        event.ctrlKey || event.shiftKey) return;
    const link = event.target.closest?.('a[data-profile-return][id][href]');
    if (!link || link.target) return;
    const href = link.getAttribute('href');
    if (!href || link.origin !== location.origin) return;
    const source = address();
    history.replaceState({...history.state, [stateKey]: {id: link.id, href, source}}, '');
    const heading = href.startsWith('/@') ? 'author-profile-heading' :
      href === '/toi/chinh-ho-so' ? 'edit-profile-heading' :
      href === '/toi#edit-profile-link' ? 'edit-profile-link' : '';
    if (heading) focusWhenReady(href, () => document.getElementById(heading), '', link);
  });

  function restore() {
    cancelFocus();
    const entry = history.state?.[stateKey];
    if (!entry || entry.source !== address()) return;
    const fallback = entry.id.startsWith('profile-work-') ? 'author-works-heading' :
      entry.id.startsWith('work-') ? 'community-stream' :
      entry.id.startsWith('comment-') ? 'feedback-heading' :
      entry.id === 'edit-profile-link' ? 'me-title' :
      entry.id === 'own-profile-edit-link' ? 'author-profile-heading' : 'noi-dung';
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
  window.addEventListener('pagehide', event => {
    const path = location.pathname;
    if (!event.persisted || !(path === '/' || path === '/toi/chinh-ho-so' ||
        ['/the/', '/tac-pham/', '/@'].some(prefix => path.startsWith(prefix)))) return;
    parked = true;
    const main = document.getElementById('noi-dung');
    main.style.visibility = 'hidden';
    main.inert = true;
  });
  window.addEventListener('popstate', restore);
  window.addEventListener('pageshow', event => {
    // A document can have several SPA history entries. The entry being restored need
    // not be the entry that was current when the document was frozen.
    if (event.persisted && parked) {
      history.replaceState({...history.state, [recheck]: address()}, '');
      location.reload();
    } else {
      restore();
    }
  });
  // A frozen document has an old projection even when a tab's RAM freshness window has
  // not elapsed. The fresh HTTP document may re-focus the surviving link.
  const state = history.state;
  if (state?.[recheck] === address()) {
    const {[recheck]: _, ...rest} = state;
    history.replaceState(rest, '');
    restore();
  } else if (performance.getEntriesByType('navigation')[0]?.type === 'back_forward') {
    restore();
  }
}
