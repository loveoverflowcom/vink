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

  window.addEventListener('popstate', restore);
  window.addEventListener('pageshow', restore);
  // A no-store Profile can reload on a bfcache return before this module hydrates.
  if (performance.getEntriesByType('navigation')[0]?.type === 'back_forward') restore();
}
