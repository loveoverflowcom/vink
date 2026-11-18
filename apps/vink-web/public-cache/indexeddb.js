// #203: storage only. No fetch, credentials, DOM, HTTP validators or retry coordinator.
// ACK is transaction.complete, never request.success. See public-read-freshness.md.
export const DATABASE = 'vink-public-snapshots';
export const VERSION = 2;
export const LIMITS = Object.freeze({ entries: 32, bytes: 4 * 1024 * 1024, record: 512 * 1024, queue: 8, retention: 86400000 });
const STORES = ['meta', 'snapshots'];
const POLICY = 'public-snapshot-v1';
const TIMEOUT = 1500;
const encoder = new TextEncoder();
const size = text => encoder.encode(text).byteLength;
const string = x => typeof x === 'string';
const nonempty = x => string(x) && x.length > 0;
const nullable = (x, f) => x === null || f(x);
const array = (x, f) => Array.isArray(x) && x.every(f);
const fields = (x, spec) => x !== null && typeof x === 'object' && !Array.isArray(x)
  && Object.keys(x).length === Object.keys(spec).length
  && Object.entries(spec).every(([k, f]) => Object.hasOwn(x, k) && f(x[k]));
const tagRef = x => fields(x, { tag_id: nonempty, label: string, slug: string });
const publicTag = x => fields(x, { tag_id: nonempty, facet: string, label: string, slug: string });
const tag = x => fields(x, { tag_id: nonempty, facet: string, label: string, slug: string,
  description: x => nullable(x, string), status: string, successor: x => nullable(x, tagRef) });
const filter = x => fields(x, { requested: string, resolution: string, tag: x => nullable(x, tag) });
const card = x => fields(x, { work_id: nonempty, title: string, author_display_name: string,
  published_at: string, tags: x => array(x, publicTag) });
// The one document shape; only the default format has a public Reader so far (#264).
const document = x => fields(x, { format: x => x === 'default', source: string });
const publication = x => fields(x, { work_id: nonempty, revision_id: nonempty, title: string,
  author_display_name: string, published_at: string, document, tags: x => array(x, publicTag) });
const list = x => fields(x, { works: x => array(x, card) && x.length <= 100 && new Set(x.map(c => c.work_id)).size === x.length,
  filters: x => fields(x, { q: x => nullable(x, string), tags: x => array(x, filter) }),
  next_cursor: x => nullable(x, nonempty) });
const query = x => fields(x, { q: string, tags: x => array(x, nonempty) && x.every((v, i) => i === 0 || x[i - 1] < v),
  cursor: x => nullable(x, nonempty) });
const keyValid = x => fields(x, { Reader: nonempty }) || fields(x, { Works: query });
// Fixed key order avoids multiple physical rows for the same logical page.
export function keyOf(key) {
  if (!keyValid(key)) return null;
  return JSON.stringify(key.Reader !== undefined ? { Reader: key.Reader }
    : { Works: { q: key.Works.q, tags: key.Works.tags, cursor: key.Works.cursor } });
}
export function decode(text, origin = globalThis.location?.origin) {
  if (!string(text) || size(text) > LIMITS.record) return null;
  try {
    const e = JSON.parse(text);
    if (!fields(e, { schema: x => x === 1, origin: x => x === origin, policy: x => x === POLICY,
      key: keyValid, provenance: x => x === 'origin_response',
      validated_at_ms: x => Number.isSafeInteger(x) && x > 0,
      payload: x => fields(x, { kind: x => x === 'published', data: publication })
        || fields(x, { kind: x => x === 'works', data: list }) })) return null;
    if (e.payload.kind === 'published') {
      if (e.key.Reader !== e.payload.data.work_id) return null;
    } else if (!e.key.Works || (e.payload.data.next_cursor !== null && e.payload.data.next_cursor === e.key.Works.cursor)) return null;
    return e;
  } catch { return null; }
}
export function age(e, now = Date.now()) {
  const elapsed = now - e.validated_at_ms;
  return Number.isSafeInteger(now) && elapsed >= 0 && elapsed < LIMITS.retention ? elapsed : null;
}
const members = e => e.payload.kind === 'published' ? [e.payload.data.work_id] : e.payload.data.works.map(w => w.work_id);
const validMeta = m => fields(m, { incarnation: nonempty, epoch: Number.isSafeInteger, sequence: Number.isSafeInteger, seen_at_ms: Number.isSafeInteger })
  && m.epoch >= 0 && m.sequence >= 0 && m.sequence < Number.MAX_SAFE_INTEGER && m.epoch < Number.MAX_SAFE_INTEGER;
const sameEpoch = (a, b) => !!a && !!b && a.incarnation === b.incarnation && a.epoch === b.epoch;
const validTicket = (m, t) => validMeta(m) && validMeta(t) && sameEpoch(m, t) && t.sequence <= m.sequence;

// Shared transaction algorithm across Memory / NoOp / IDB. Only IDB can ACK durability.
// A global epoch per barrier avoids an unbounded per-work tombstone ledger. Unrelated rows
// survive purge, but all pre-barrier writers/restore leases must be reacquired BEFORE a new read.
export function transition(meta, rows, op, now, origin) {
  if (!validMeta(meta) || !Number.isSafeInteger(now) || now <= 0) throw new Error('cache_metadata');
  if (now < meta.seen_at_ms) {
    // A known wall-clock rollback must not rejuvenate data. Keep the high-water clock and
    // fence every outstanding ticket; do not claim a successful capture/read/write.
    return { meta: { ...meta, epoch: meta.epoch + 1 }, rows: [], result: op.kind === 'current' ? false : null, reset: true };
  }
  meta = { ...meta, seen_at_ms: now };
  const alive = rows.filter(row => {
    const e = decode(row.text, origin);
    return e && row.key === keyOf(e.key) && Number.isSafeInteger(row.sequence)
      && age(e, now) !== null;
  });
  let result = op.kind === 'current' ? false : null;
  if (op.kind === 'capture') {
    meta = { ...meta, sequence: meta.sequence + 1 };
    result = { ...meta };
  } else if (op.kind === 'barrier') {
    meta = { ...meta, epoch: meta.epoch + 1 };
    return { meta, rows: op.work === null ? [] : alive.filter(row => !members(decode(row.text, origin)).includes(op.work)), result: true };
  } else if (validTicket(meta, op.ticket)) {
    if (op.kind === 'current') result = true;
    if (op.kind === 'read') result = alive.find(r => r.key === op.key)?.text ?? null;
    if (op.kind === 'restore') {
      const hit = alive.find(r => r.key === op.key);
      const previous = rows.find(r => r.key === op.key);
      const e = previous && decode(previous.text, origin);
      result = hit ? { state: 'hit', text: hit.text }
        : { state: e && now - e.validated_at_ms >= LIMITS.retention ? 'expired' : 'miss' };
    }
    if (op.kind === 'write') {
      const e = decode(op.text, origin);
      if (e && age(e, now) !== null) {
        const key = keyOf(e.key);
        const previous = alive.find(r => r.key === key);
        if (!previous || previous.sequence <= op.ticket.sequence) {
          const rows = alive.filter(r => r.key !== key);
          rows.push({ key, text: op.text, sequence: op.ticket.sequence });
          // Oldest validation first, stable tie break; reads do not extend retention.
          rows.sort((a, b) => decode(a.text, origin).validated_at_ms - decode(b.text, origin).validated_at_ms || a.key.localeCompare(b.key));
          let bytes = rows.reduce((n, r) => n + size(JSON.stringify(r)), 0);
          while (rows.length > LIMITS.entries || bytes > LIMITS.bytes) bytes -= size(JSON.stringify(rows.shift()));
          return { meta, rows, result: rows.some(r => r.key === key) };
        }
      }
    }
  }
  return { meta, rows: op.kind === 'capture' || op.kind === 'current' ? rows : alive, result };
}

export class MemoryStorage {
  constructor(origin = globalThis.location?.origin, retain = true) {
    this.origin = origin; this.retain = retain;
    this.meta = { incarnation: crypto.randomUUID(), epoch: 0, sequence: 0, seen_at_ms: Date.now() }; this.rows = [];
  }
  async run(op) {
    const next = transition(this.meta, this.rows, op, Date.now(), this.origin);
    this.meta = next.meta; this.rows = this.retain ? next.rows : [];
    // Memory/NoOp never return a durable write acknowledgement.
    return op.kind === 'write' ? false : next.result;
  }
}
export class NoOpStorage extends MemoryStorage {
  constructor(origin) { super(origin, false); }
}

export class IndexedDbStorage {
  constructor() { this.opening = null; this.db = null; this.failed = false; this.invalidated = false; }
  async open() {
    if (this.failed) return null;
    if (this.db) return this.db;
    this.opening ??= new Promise(resolve => {
      let request, settled = false;
      const finish = db => {
        if (settled) { db?.close(); return; }
        settled = true; clearTimeout(timer);
        if (!db) this.failed = true;
        this.db = db; resolve(db);
      };
      const timer = setTimeout(() => finish(null), TIMEOUT);
      try { request = indexedDB.open(DATABASE, VERSION); } catch { finish(null); return; }
      request.onblocked = () => finish(null);
      request.onerror = () => finish(null);
      request.onupgradeneeded = () => {
        if (settled) { request.transaction.abort(); return; }
        // Deliberate v1 -> v2 discard migration in this dedicated public namespace only.
        for (const name of STORES) if (request.result.objectStoreNames.contains(name)) request.result.deleteObjectStore(name);
        request.result.createObjectStore('snapshots', { keyPath: 'key' });
        request.result.createObjectStore('meta').put({ incarnation: crypto.randomUUID(), epoch: 0, sequence: 0, seen_at_ms: Date.now() }, 'state');
      };
      request.onsuccess = () => {
        const db = request.result;
        db.onversionchange = () => { this.invalidated = true; this.failed = true; this.db = null; db.close(); };
        db.onclose = () => { this.failed = true; this.db = null; };
        finish(db);
      };
    });
    return this.opening;
  }
  async run(op) {
    if (this.invalidated && op.kind === 'current') return false;
    const db = await this.open();
    if (!db || this.failed) return null;
    return new Promise(resolve => {
      let tx, result = null, settled = false;
      const finish = value => { if (!settled) { settled = true; clearTimeout(timer); resolve(value); } };
      const timer = setTimeout(() => {
        try { tx?.abort(); } catch { /* already finished */ }
        this.failed = true; db.close(); finish(null);
      }, TIMEOUT);
      try {
        // Readwrite serializes barriers/capture/write and cleanup across connections. Requests
        // are enqueued synchronously from IDB callbacks; never await within a transaction.
        tx = db.transaction(STORES, 'readwrite', { durability: 'strict' });
        tx.oncomplete = () => finish(result);
        tx.onabort = () => finish(null);
        tx.onerror = () => {}; // abort is the only failure settlement; no payload logging.
        const m = tx.objectStore('meta').get('state');
        m.onsuccess = () => {
          const apply = rows => {
            try {
              const next = transition(m.result, rows, op, Date.now(), location.origin);
              result = next.result;
              tx.objectStore('meta').put(next.meta, 'state');
              // Metadata checks must not rewrite megabytes or need payload quota on each
              // lifecycle event. A detected clock rollback still atomically clears all rows.
              if (next.reset || (op.kind !== 'capture' && op.kind !== 'current')) {
                const store = tx.objectStore('snapshots');
                store.clear();
                for (const row of next.rows) store.put(row);
              }
            } catch { tx.abort(); }
          };
          if (op.kind === 'capture' || op.kind === 'current') apply([]);
          else {
            const r = tx.objectStore('snapshots').getAll();
            r.onsuccess = () => apply(r.result);
          }
        };
      } catch { finish(null); }
    });
  }
  close() { this.failed = true; this.db?.close(); this.db = null; }
}

// Bounded pending writes, coalesced per query/page. At most eight pending envelopes + one
// executing. Overflow is a cache miss, never backpressure on network/editor work.
export class WriteQueue {
  constructor(storage) { this.storage = storage; this.pending = new Map(); this.running = false; }
  write(ticket, text) {
    const e = decode(text);
    if (!e) return Promise.resolve(false);
    const key = keyOf(e.key);
    const old = this.pending.get(key);
    if (old && sameEpoch(old.ticket, ticket) && old.ticket.sequence > ticket.sequence) return Promise.resolve(false);
    if (old) { old.resolve(false); this.pending.delete(key); }
    if (this.pending.size >= LIMITS.queue) return Promise.resolve(false);
    return new Promise(resolve => {
      this.pending.set(key, { ticket, text, resolve }); this.drain();
    });
  }
  async drain() {
    if (this.running) return;
    this.running = true;
    while (this.pending.size) {
      const [key, item] = this.pending.entries().next().value;
      this.pending.delete(key);
      try { item.resolve((await this.storage.run({ kind: 'write', ticket: item.ticket, text: item.text })) === true); }
      catch { item.resolve(false); }
    }
    this.running = false;
  }
}
const storage = new IndexedDbStorage();
const queue = new WriteQueue(storage);
export const capture = () => storage.run({ kind: 'capture' });
export const read = (ticket, key) => {
  try { key = keyOf(JSON.parse(key)); } catch { return Promise.resolve(null); }
  return key ? storage.run({ kind: 'read', ticket, key }) : Promise.resolve(null);
};
// Separate expiry signal for the future UX consumer. Expired responses contain no body/cursor.
export async function restore(ticket, key) {
  try { key = keyOf(JSON.parse(key)); } catch { return JSON.stringify({ state: 'miss' }); }
  const result = key && await storage.run({ kind: 'restore', ticket, key });
  return JSON.stringify(result || { state: 'unavailable' });
}
export const write = (ticket, text) => queue.write(ticket, text);
// null = storage unavailable, false = a known durable barrier. The owner must not confuse
// a quota/open failure with authoritative removal of the active online snapshot.
export const current = ticket => storage.run({ kind: 'current', ticket });
let channel;
let listener;
export function listen(callback) {
  listener = callback;
  try {
    channel ??= new BroadcastChannel('vink-public-snapshots');
    channel.onmessage = () => listener?.(); // hint only: receiver MUST read durable metadata.
  } catch { /* transaction fences still work; lifecycle resume checks metadata */ }
}
export async function barrier(work = null) {
  if (work !== null && !nonempty(work)) return false;
  const committed = await storage.run({ kind: 'barrier', work });
  if (committed === true) { try { channel?.postMessage('barrier'); } catch { /* hint only */ } }
  return committed === true;
}
