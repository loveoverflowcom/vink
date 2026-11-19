const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

export function captureRequest(page, method, path) {
  let requests = 0;
  const listener = (event) => {
    if (event.sessionId !== page.sessionId || event.method !== 'Network.requestWillBeSent') return;
    const { request } = event.params;
    if (request.method === method && new URL(request.url).pathname === path) requests++;
  };
  page.cdp.listeners.push(listener);
  return {
    count: () => requests,
    close: () => { page.cdp.listeners = page.cdp.listeners.filter((item) => item !== listener); },
  };
}

/** Hold the first matching real answer before delivery. Can release it or lose the ACK. */
export async function holdAnswer(page, method, path) {
  let held;
  let networkDone;
  const listener = (event) => {
    if (event.sessionId !== page.sessionId) return;
    if (held && event.params?.requestId === held.networkId &&
        (event.method === 'Network.loadingFinished' || event.method === 'Network.loadingFailed')) {
      networkDone = event.method;
    }
    if (event.method !== 'Fetch.requestPaused') return;
    const { request, requestId, networkId, responseStatusCode } = event.params;
    if (!held && request.method === method && new URL(request.url).pathname === path) {
      held = { requestId, networkId, status: responseStatusCode };
    } else {
      page.send('Fetch.continueRequest', { requestId }).catch(() => {});
    }
  };
  page.cdp.listeners.push(listener);
  await page.send('Fetch.enable', { patterns: [{ urlPattern: `*${path}*`, requestStage: 'Response' }] });
  return {
    async held() {
      for (let i = 0; !held && i < 160; i++) await sleep(50);
      if (!held) throw new Error(`timed out holding ${method} ${path}`);
      return held.status;
    },
    async finish(drop = false, waitForDelivery = false) {
      try {
        if (drop) await page.send('Fetch.failRequest', { requestId: held.requestId, errorReason: 'ConnectionClosed' });
        else if (networkDone !== 'Network.loadingFailed') {
          await page.send('Fetch.continueRequest', { requestId: held.requestId });
        }
        if (waitForDelivery) {
          for (let i = 0; !networkDone && i < 100; i++) await sleep(50);
          if (!networkDone) throw new Error(`no network disposition for ${method} ${path}`);
          await page.eval('new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))');
        }
        return networkDone;
      } finally {
        page.cdp.listeners = page.cdp.listeners.filter((item) => item !== listener);
        await page.send('Fetch.disable');
      }
    },
  };
}
