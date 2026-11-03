// Host facts and effects only. Every admission, callback, release and cancellation is decided
// by the production Rust closure. No parallel JS playback policy, worker or persistence.
const animationFiles = __ANIMATION_FILES__;
const engineUrl = new URL('./runtime/__ENGINE_FILE__', import.meta.url).href;
const wasmUrl = new URL('./runtime/__WASM_FILE__', import.meta.url).href;
let enginePromise;
function engine() {
  enginePromise ??= import(engineUrl).then(module => {
    module.DotLottie.setWasmUrl(wasmUrl); // disables the upstream backup CDN, too
    return module.DotLottie;
  }).catch(error => { enginePromise = undefined; throw error; });
  return enginePromise;
}

export function mountPilot(root, decide) {
  const cells = [...root.querySelectorAll('[data-expression-index]')];
  const handles = new Map();
  const listeners = [];
  const reduced = matchMedia('(prefers-reduced-motion: reduce)');
  let alive = true;
  let state;
  const listen = (element, type, listener) => {
    element.addEventListener(type, listener);
    listeners.push(() => element.removeEventListener(type, listener));
  };
  function remove(index) {
    const handle = handles.get(index);
    if (!handle) return;
    handles.delete(index); // remove before destroy events can reenter
    clearTimeout(handle.timer);
    handle.abort.abort();
    for (const [type, listener] of handle.events) handle.player?.removeEventListener(type, listener);
    handle.player?.pause();
    handle.player?.destroy();
    handle.canvas?.remove();
  }
  function dispatch(event, index = 0, generation = 0, flag = false) {
    if (!alive) return { action: 'none' };
    state = JSON.parse(decide(event, index, generation, flag));
    for (const [i, handle] of handles) {
      const player = state.players[i];
      if (!player || !['loading', 'playing'].includes(player.phase) || player.generation !== handle.generation) remove(i);
    }
    cells.forEach((cell, i) => {
      const player = state.players[i];
      cell.dataset.phase = player.phase;
      cell.dataset.generation = String(player.generation);
      cell.querySelector('[data-player-status]').textContent = player.label;
      cell.querySelector('button').setAttribute('aria-busy', String(player.phase === 'loading'));
    });
    root.dataset.activePlayers = String(state.active);
    root.querySelector('#expression-budget').textContent = `${state.active} / ${state.capacity} lượt hoạt động · ${cells.length} biểu cảm Google Noto`;
    if (['activate', 'error', 'ready', 'complete', 'reduced'].includes(event)) root.querySelector('#expression-status').textContent = state.players[index]?.label ?? 'Hình tĩnh';
    return state;
  }
  function current(index, generation) {
    const player = state?.players[index];
    return alive && root.isConnected && player?.generation === generation && ['loading', 'playing'].includes(player.phase);
  }
  async function start(index, generation) {
    const abort = new AbortController();
    const handle = { generation, abort, events: [], timer: null, player: null, canvas: null };
    handles.set(index, handle);
    handle.timer = setTimeout(() => dispatch('error', index, generation), 12000);
    try {
      // Files are generated from the trusted manifest, never accepted from attributes/URLs.
      const cancelled = new Promise((_, reject) => abort.signal.addEventListener('abort', () => reject(new DOMException('Cancelled', 'AbortError')), { once: true }));
      const [DotLottie, data] = await Promise.race([cancelled, Promise.all([
        engine(), fetch(new URL(`./assets/${animationFiles[index]}`, import.meta.url), { signal: abort.signal, credentials: 'omit' })
          .then(response => { if (!response.ok) throw new Error('Animation unavailable'); return response.text(); }),
      ])]);
      if (!current(index, generation)) return;
      const canvas = document.createElement('canvas');
      canvas.width = 192; canvas.height = 192; canvas.setAttribute('aria-hidden', 'true');
      handle.canvas = canvas;
      cells[index].querySelector('.vk-expression-art').append(canvas);
      const player = new DotLottie({ canvas, data, autoplay: false, loop: false,
        renderConfig: { autoResize: false, freezeOnOffscreen: false, devicePixelRatio: 1 } });
      handle.player = player;
      const on = (type, listener) => { handle.events.push([type, listener]); player.addEventListener(type, listener); };
      on('load', () => {
        if (!current(index, generation)) return;
        clearTimeout(handle.timer);
        if (dispatch('ready', index, generation).action === 'play') {
          // Bound even a broken/missing completion signal; a slot cannot leak indefinitely.
          handle.timer = setTimeout(() => dispatch('error', index, generation), 30000);
          player.play();
        }
      });
      on('complete', () => { if (current(index, generation)) dispatch('complete', index, generation); });
      on('loadError', () => { if (current(index, generation)) dispatch('error', index, generation); });
      on('renderError', () => { if (current(index, generation)) dispatch('error', index, generation); });
      on('render', () => { if (current(index, generation)) cells[index].dataset.frames = String(Number(cells[index].dataset.frames ?? 0) + 1); });
    } catch (error) {
      if (error.name !== 'AbortError' && current(index, generation)) dispatch('error', index, generation);
    }
  }
  function facts() {
    if (!alive) return;
    dispatch('runtime', 0, 0, typeof WebAssembly === 'object' && typeof IntersectionObserver === 'function');
    dispatch('reduced', 0, 0, reduced.matches || !!root.closest('[data-motion="reduce"]'));
    dispatch('foreground', 0, 0, document.visibilityState === 'visible' && !root.closest('[data-active="false"]'));
  }
  cells.forEach((cell, index) => {
    const button = cell.querySelector('button');
    listen(button, 'click', event => {
      if (!event.isTrusted) return;
      facts(); // refresh synchronous host facts before asking for admission
      const decision = dispatch('activate', index);
      if (decision.action === 'load') start(index, decision.generation);
    });
    const image = cell.querySelector('img');
    const failedPoster = () => { cell.dataset.posterFailed = 'true'; };
    listen(image, 'error', failedPoster);
    if (image.complete && !image.naturalWidth) failedPoster();
    // Associate the quiet status with its button without per-frame announcements.
    const status = cell.querySelector('[data-player-status]');
    status.id = `expression-${index}-status`;
    button.setAttribute('aria-describedby', status.id);
  });
  const observer = typeof IntersectionObserver === 'function' ? new IntersectionObserver(entries => {
    for (const entry of entries) dispatch('visible', Number(entry.target.dataset.expressionIndex), 0, entry.isIntersecting);
  }, { threshold: 0.01 }) : null;
  cells.forEach(cell => observer?.observe(cell));
  const motionObserver = new MutationObserver(facts);
  motionObserver.observe(document.documentElement, { subtree: true, attributes: true, attributeFilter: ['data-motion', 'data-active'] });
  listen(document, 'visibilitychange', facts);
  listen(reduced, 'change', facts);
  listen(window, 'pagehide', () => { cells.forEach((_, index) => dispatch('cancel', index)); });
  listen(window, 'pageshow', facts);
  facts();
  root.dataset.ready = 'true';
  return { destroy() {
    if (!alive) return;
    // Synchronously dispose Rust reservations and all JS resources before releasing closure.
    cells.forEach((_, index) => dispatch('dispose', index));
    alive = false;
    observer?.disconnect(); motionObserver.disconnect();
    listeners.splice(0).forEach(removeListener => removeListener());
    for (const index of handles.keys()) remove(index);
    root.dataset.ready = 'false';
  } };
}
