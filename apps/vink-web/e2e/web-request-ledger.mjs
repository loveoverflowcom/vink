// CDP evidence only. A request is a browser transport attempt; it isn't a SQL read or
// a Vink owner flight. Store no headers, query values, credentials or payloads.
export function classifyTransport({ path, resource_type }) {
  if (resource_type === 'Document') return 'document';
  if (path === '/api/v1/works') return 'public-list';
  if (/^\/api\/v1\/works\/[^/]+\/reader$/.test(path)) return 'reader';
  if (/^\/api\/v1\/works\/[^/]+\/discussion\/public(\/roots\/[^/]+\/replies)?$/.test(path)) return 'public-discussion';
  if (/^\/api\/v1\/works\/[^/]+\/discussion\/me$/.test(path)) return 'own-discussion';
  if (/^\/api\/v1\/works\/[^/]+\/discussion$/.test(path)) return 'actor-discussion';
  if (/^\/api\/v1\/works\/[^/]+\/endorsements\/me$/.test(path)) return 'own-endorsement';
  if (/^\/api\/v1\/works\/[^/]+\/endorsements$/.test(path)) return 'public-endorsement';
  if (path.startsWith('/api/v1/auth/session')) return 'session';
  if (path.startsWith('/api/v1/tags')) return 'catalog';
  if (path.startsWith('/pkg/')) return 'bundle';
  return 'other';
}

const hasValidator = headers => Object.keys(headers ?? {}).some(name => name.toLowerCase() === 'if-none-match');

export function createTransportLedger(origin, currentPhase) {
  const siteOrigin = new URL(origin).origin;
  const rows = [];
  const requests = new Map();
  const keys = new Map();
  const extraInfo = record => {
    // CDP reuses requestId across redirects. Avoid applying any wire extra info to the
    // wrong hop; redirect rows keep their status from redirectResponse instead.
    for (const row of record.rows) {
      const single = record.hops === 1;
      row.validator_sent = single ? (record.validator ?? (row.validator_in_request || null)) : null;
      row.wire_status = single ? (record.wire ?? null) : null;
    }
  };
  const feed = (method, params) => {
    if (!method.startsWith('Network.') || !params.requestId) return;
    const id = params.requestId;
    if (!requests.has(id)) requests.set(id, { hops: 0, rows: [] });
    const record = requests.get(id);
    let row = record.current;
    if (method === 'Network.requestWillBeSent') {
      if (row && params.redirectResponse) {
        row.response_status = params.redirectResponse.status;
        row.redirected = true;
      }
      record.hops++;
      record.current = null;
      const { request, type, initiator, timestamp } = params;
      const url = new URL(request.url);
      if (url.origin === siteOrigin) {
        const key = request.method + ' ' + url.pathname + '?' + url.searchParams.toString();
        if (!keys.has(key)) keys.set(key, keys.size + 1);
        row = {
          phase: currentPhase(), request_id: id, hop: record.hops,
          method: request.method, path: url.pathname, key_id: keys.get(key),
          query_parameter_keys: [...url.searchParams.keys()].sort(),
          resource_type: type ?? null, initiator: initiator?.type ?? null, started_s: timestamp,
          validator_in_request: hasValidator(request.headers),
          response_status: null, disk_cache: null, service_worker: null, memory_cache_event: false,
          encoded_bytes: null, finished_s: null, failed: false, redirected: false,
        };
        row.kind = classifyTransport(row);
        record.current = row;
        record.rows.push(row);
        rows.push(row);
      }
      extraInfo(record);
    } else if (method === 'Network.requestWillBeSentExtraInfo') {
      record.validator = hasValidator(params.headers);
      extraInfo(record);
    } else if (method === 'Network.responseReceivedExtraInfo') {
      record.wire = params.statusCode;
      extraInfo(record);
    } else if (row && method === 'Network.responseReceived') {
      row.response_status = params.response.status;
      row.disk_cache = !!params.response.fromDiskCache;
      row.service_worker = !!params.response.fromServiceWorker;
    } else if (row && method === 'Network.requestServedFromCache') {
      row.memory_cache_event = true;
    } else if (row && method === 'Network.loadingFinished') {
      row.encoded_bytes = params.encodedDataLength;
      row.finished_s = params.timestamp;
    } else if (row && method === 'Network.loadingFailed') {
      row.failed = true;
      row.finished_s = params.timestamp;
    }
  };
  return { rows, feed };
}

export function recordTransport(page, origin, currentPhase) {
  const ledger = createTransportLedger(origin, currentPhase);
  const onEvent = ({ sessionId, method, params }) => {
    if (sessionId === page.sessionId) ledger.feed(method, params ?? {});
  };
  page.cdp.listeners.push(onEvent);
  return { rows: ledger.rows, stop: () => {
    page.cdp.listeners = page.cdp.listeners.filter(item => item !== onEvent);
  } };
}

export function countTransport(rows, phase, kind) {
  return rows.filter(row => row.phase === phase && row.kind === kind && row.method === 'GET').length;
}

export function assertNoHydrationRead(rows, phase, kind) {
  const count = countTransport(rows, phase, kind);
  if (count !== 0) throw new Error(`${phase}: SSR handoff sent ${count} unexpected ${kind} GET(s)`);
}
