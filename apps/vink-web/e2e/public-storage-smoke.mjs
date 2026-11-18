// Synthetic public-only fixtures. Same adapter shipped by wasm-bindgen; no fake IndexedDB.
export async function setup(page, args) {
  await page.goto(`${args.base}/`);
  await page.waitFor(`document.documentElement.dataset.hydrated === 'true'`, 'storage test hydration');
  await page.eval(String.raw`(async () => {
    const bundle = await (await fetch('/pkg/vink.js')).text();
    const path = bundle.match(/['"](\.\/snippets\/[^'"]+\/public-cache\/indexeddb.js)['"]/)[1];
    window.pc = await import(new URL(path, location.origin + '/pkg/vink.js'));
    window.fixture = (id = 'synthetic-public', cursor = null, length = 20) => JSON.stringify({
      schema: 1, origin: location.origin, policy: 'public-snapshot-v1', key: { Reader: id },
      provenance: 'origin_response', validated_at_ms: Date.now(), payload: { kind: 'published', data: {
        work_id: id, revision_id: 'revision-1', title: 'Thơ mẫu', author_display_name: 'Tác giả mẫu',
        published_at: '2026-11-15T00:00:00Z', tags: [], document: { format: 'default', source: 'Mưa '.repeat(length) }
      }}
    });
    window.pageFixture = () => {
      const e = JSON.parse(fixture()); e.key = { Works: { q: '', tags: [], cursor: null } };
      e.payload = { kind: 'works', data: { works: [{ work_id: 'synthetic-public', title: 'Thơ mẫu', author_display_name: 'Tác giả mẫu', published_at: '2026-11-15T00:00:00Z', tags: [] }], filters: { q: null, tags: [] }, next_cursor: 'opaque-next' } };
      return JSON.stringify(e);
    };
    window.raw = async fn => {
      const db = await new Promise((resolve, reject) => { const q = indexedDB.open(pc.DATABASE, pc.VERSION); q.onsuccess = () => resolve(q.result); q.onerror = () => reject(q.error); });
      try { return await new Promise((resolve, reject) => {
        const tx = db.transaction(['meta','snapshots'], 'readwrite'); let result;
        tx.oncomplete = () => resolve(result); tx.onabort = () => reject(tx.error);
        fn(tx, value => result = value);
      }); } finally { db.close(); }
    };
  })()`);
}
export async function publicStorage(page, { args, check, openPage }) {
  await setup(page, args);
  check(await page.eval(`(async () => { return !(await indexedDB.databases()).some(d => d.name === pc.DATABASE); })()`), 'POLICY-01/02 gate: production hydration does not open persistent storage');
  const peers = await openPage();
  await setup(peers, args);
  check(await page.eval(`(async () => {
    for (const Adapter of [pc.MemoryStorage, pc.NoOpStorage, pc.IndexedDbStorage]) {
      const s = new Adapter(); await s.run({kind:'barrier',work:null});
      const t = await s.run({kind:'capture'}); const text = pageFixture(); const key = pc.keyOf(JSON.parse(text).key);
      const ack = await s.run({kind:'write',ticket:t,text}); const saved = await s.run({kind:'read',ticket:t,key});
      if (s instanceof pc.NoOpStorage ? saved !== null : saved !== text) return false;
      if (ack !== (s instanceof pc.IndexedDbStorage)) return false;
      if (await s.run({kind:'read',ticket:t,key:pc.keyOf({Reader:'wrong'})}) !== null) return false;
      await s.run({kind:'write',ticket:t,text:fixture()});
      await s.run({kind:'write',ticket:t,text:fixture('unrelated')});
      const later = await s.run({kind:'capture'});
      await s.run({kind:'write',ticket:later,text});
      if (await s.run({kind:'write',ticket:t,text}) === true) return false;
      await s.run({kind:'barrier',work:'synthetic-public'});
      if (await s.run({kind:'write',ticket:t,text}) || await s.run({kind:'current',ticket:t})) return false;
      const fresh = await s.run({kind:'capture'});
      if (await s.run({kind:'read',ticket:fresh,key}) !== null) return false;
      if (await s.run({kind:'read',ticket:fresh,key:pc.keyOf({Reader:'synthetic-public'})}) !== null) return false;
      if (!(s instanceof pc.NoOpStorage) && !await s.run({kind:'read',ticket:fresh,key:pc.keyOf({Reader:'unrelated'})})) return false;
      await s.run({kind:'write',ticket:fresh,text:fixture()});
      await s.run({kind:'barrier',work:null});
      if (await s.run({kind:'read',ticket:fresh,key:pc.keyOf({Reader:'synthetic-public'})}) !== null) return false;
      s.close?.();
    }
    return true;
  })()`), 'shared Memory/NoOp/IndexedDB suite: scope, page atomicity, authority barrier and honest durability');

  check(await page.eval(`(async () => {
    window.s = new pc.IndexedDbStorage(); window.t = await s.run({kind:'capture'});
    window.text = pageFixture(); window.key = pc.keyOf(JSON.parse(text).key);
    for (const name of ['token','capability','draft','own','pending','rejected','moderation','password']) {
      const e = JSON.parse(text); e.payload.data[name] = 'must-not-store';
      if (pc.decode(JSON.stringify(e)) || await s.run({kind:'write',ticket:t,text:JSON.stringify(e)})) return false;
    }
    const wrong = JSON.parse(fixture()); wrong.key.Reader = 'other';
    if (pc.decode(JSON.stringify(wrong))) return false;
    const missing = JSON.parse(text); delete missing.payload.data.works;
    if (pc.decode(JSON.stringify(missing))) return false;
    await s.run({kind:'write',ticket:t,text});
    const original = IDBObjectStore.prototype.put;
    let success = false;
    IDBObjectStore.prototype.put = function(...args) {
      const request = original.apply(this,args);
      if (this.name === 'snapshots') request.addEventListener('success', () => { success = true; this.transaction.abort(); }, {once:true});
      return request;
    };
    let ack;
    try { const e = JSON.parse(text); e.payload.data.next_cursor = 'should-abort'; ack = await s.run({kind:'write',ticket:t,text:JSON.stringify(e)}); }
    finally { IDBObjectStore.prototype.put = original; }
    return success && ack !== true && await s.run({kind:'read',ticket:t,key}) === text;
  })()`), 'strict allowlist; actual transaction abort AFTER request success preserves entire prior page/cursor and never ACKs');

  check(await page.eval(`(async () => {
    const original = IDBObjectStore.prototype.put;
    IDBObjectStore.prototype.put = function(...args) {
      const request = original.apply(this,args);
      if(this.name === 'meta') request.addEventListener('success', () => this.transaction.abort(), {once:true});
      return request;
    };
    let unavailable;
    try { unavailable = await s.run({kind:'current',ticket:t}); }
    finally { IDBObjectStore.prototype.put = original; }
    const changed = await s.run({kind:'current',ticket:{...t,epoch:t.epoch+1}});
    const valid = await s.run({kind:'current',ticket:t});
    return unavailable === null && changed === false && valid === true;
  })()`), 'lease check distinguishes storage failure from durable barrier; only the latter revokes RAM');

  // Set quota BEFORE opening this separate origin's IDB bucket. Chrome caches an existing
  // bucket's space grant; changing quota after opening it is not a reliable exhaustion test.
  const quotaPage = await openPage();
  const quotaArgs = { ...args, base: args.base.replace('127.0.0.1', 'localhost') };
  const quotaOrigin = new URL(quotaArgs.base).origin;
  await quotaPage.send('Storage.overrideQuotaForOrigin', { origin: quotaOrigin, quotaSize: 512 * 1024 });
  try {
    await setup(quotaPage, quotaArgs);
    check(await quotaPage.eval(`(async () => {
      const s = new pc.IndexedDbStorage(); const t = await s.run({kind:'capture'});
      if (!t) return false;
      try {
        for (let i=0;i<12;i++) {
          const e = JSON.parse(fixture('quota-'+i,null,0));
          e.payload.data.document.source = Array.from({length:13000},()=>crypto.randomUUID()).join('');
          if(await s.run({kind:'write',ticket:t,text:JSON.stringify(e)}) !== true) return true;
        }
        return false;
      } finally { s.close(); }
    })()`), 'actual Chrome quota exhaustion fails write without durable ACK');
  } finally { await quotaPage.send('Storage.overrideQuotaForOrigin', { origin: quotaOrigin }); }
  check(await page.eval(`(async () => { return await s.run({kind:'read',ticket:t,key}) === text; })()`), 'prior data/cursor survives independent quota failure');

  check(await page.eval(`(async () => {
    const token = await s.run({kind:'capture'});
    const original = IDBFactory.prototype.open;
    IDBFactory.prototype.open = () => { throw new DOMException('blocked','SecurityError'); };
    const denied = new pc.IndexedDbStorage();
    let first; try { first = await denied.run({kind:'capture'}); } finally { IDBFactory.prototype.open = original; }
    if (first !== null || await denied.run({kind:'capture'}) !== null) return false;
    await raw((tx) => tx.objectStore('snapshots').put({key:'corrupt', text:'{bad', sequence:1}));
    if (await s.run({kind:'read',ticket:token,key:'corrupt'}) !== null) return false;
    const rows = await raw((tx,done) => { const q = tx.objectStore('snapshots').getAll(); q.onsuccess = () => done(q.result); });
    return !rows.some(r => r.key === 'corrupt');
  })()`), 'injected open denial is finite NoOp; corrupt persisted record is discarded as a whole');

  // Two independent tabs, intentionally without BroadcastChannel listeners.
  await page.eval(`(async () => { window.delayed = await s.run({kind:'capture'}); window.delayedText = fixture(); await s.run({kind:'write',ticket:delayed,text:delayedText}); window.restored = await s.run({kind:'read',ticket:delayed,key:pc.keyOf({Reader:'synthetic-public'})}); })()`);
  await page.send('Page.setWebLifecycleState', { state: 'frozen' });
  await peers.eval(`(async () => { window.peer = new pc.IndexedDbStorage(); await peer.run({kind:'barrier',work:'synthetic-public'}); })()`);
  await page.send('Page.setWebLifecycleState', { state: 'active' });
  check(await page.eval(`(async () => { return await s.run({kind:'write',ticket:delayed,text:delayedText}) !== true && await s.run({kind:'current',ticket:delayed}) !== true; })()`), 'tab B purge fences tab A delayed write AND delayed restore, with no broadcast delivery');
  await page.eval(`(async () => { window.delayed = await s.run({kind:'capture'}); })()`);
  await peers.eval(`(async () => { await peer.run({kind:'barrier',work:null}); })()`);
  check(await page.eval(`(async () => { return await s.run({kind:'write',ticket:delayed,text:delayedText}) !== true; })()`), 'cross-tab clear wins delayed response writer');

  check(await page.eval(`(async () => {
    await s.run({kind:'barrier',work:null});
    const newer = [];
    for (let i=0;i<40;i++) { const ticket = await s.run({kind:'capture'}); await s.run({kind:'write',ticket,text:fixture('small-'+i,null,0)}); }
    const small = await raw((tx,done) => { const q=tx.objectStore('snapshots').count(); q.onsuccess=()=>done(q.result); });
    if (small !== pc.LIMITS.entries) return false;
    await s.run({kind:'barrier',work:null});
    for (let i=0;i<40;i++) { const ticket = await s.run({kind:'capture'}); await s.run({kind:'write',ticket,text:fixture('budget-'+i,null,45000)}); }
    const rows = await raw((tx,done) => { const q = tx.objectStore('snapshots').getAll(); q.onsuccess = () => done(q.result); });
    window.storageMeasurement = { small_corpus_entries:small, entries:rows.length, logical_bytes: rows.reduce((n,r)=>n+new TextEncoder().encode(JSON.stringify(r)).length,0) };
    if (rows.length > pc.LIMITS.entries || storageMeasurement.logical_bytes > pc.LIMITS.bytes) return false;
    for (const row of rows) if (!pc.decode(row.text)) return false;
    const q = new pc.WriteQueue({run:() => new Promise(resolve => newer.push(resolve))});
    const ticket = await s.run({kind:'capture'});
    const jobs = [];
    for(let i=0;i<30;i++) jobs.push(q.write(ticket, fixture('queue-'+i)));
    if(q.pending.size !== pc.LIMITS.queue) return false;
    const duplicate = q.write(ticket, fixture('queue-1')); jobs.push(duplicate);
    while(newer.length || q.pending.size) { newer.shift()?.(true); await Promise.resolve(); await Promise.resolve(); }
    const outcomes = await Promise.all(jobs);
    if (outcomes.filter(Boolean).length !== pc.LIMITS.queue + 1) return false;
    const e = JSON.parse(fixture()); const date = e.validated_at_ms;
    if (pc.age(e,date-1) !== null || pc.age(e,date+pc.LIMITS.retention) !== null || pc.age(e,NaN) !== null || pc.age(e,date+50)!==50) return false;
    return true;
  })()`), 'hard entry/byte budget, whole-page eviction, bounded/coalesced queue, restart-age/rollback/expiry');
  console.log('  measurement', await page.eval(`storageMeasurement`));

  check(await page.eval(`(async () => {
    const ticket = await s.run({kind:'capture'}); const text = fixture('expired');
    await s.run({kind:'write',ticket,text});
    const original = Date.now; const then = JSON.parse(text).validated_at_ms;
    let expired, rollback;
    try {
      Date.now = () => then + pc.LIMITS.retention;
      expired = await s.run({kind:'restore',ticket,key:pc.keyOf({Reader:'expired'})});
      Date.now = () => then + 5;
      rollback = await s.run({kind:'capture'});
    } finally { Date.now = original; }
    return expired?.state === 'expired' && !Object.hasOwn(expired,'text') && rollback === null;
  })()`), '24-hour expiry is a separate body-free signal; durable clock high-water rejects rollback');

  // Versioned namespace migration, blocked open, eviction ABA, and old application rejection.
  await page.eval(`s.close(); localStorage.setItem('vink-private-sentinel','untouched');`);
  await peers.eval(`peer.close();`);
  await page.eval(`(async () => { await new Promise((resolve,reject)=>{const q=indexedDB.deleteDatabase(pc.DATABASE);q.onsuccess=resolve;q.onerror=reject;}); window.legacy = await new Promise(resolve=>{const q=indexedDB.open(pc.DATABASE,1);q.onupgradeneeded=()=>q.result.createObjectStore('legacy');q.onsuccess=()=>resolve(q.result);}); legacy.close(); await new Promise(resolve=>{const q=indexedDB.open(pc.DATABASE,2);q.onupgradeneeded=()=>{q.result.deleteObjectStore('legacy');q.transaction.abort();};q.onerror=resolve;}); window.legacy = await new Promise(resolve=>{const q=indexedDB.open(pc.DATABASE,1);q.onsuccess=()=>resolve(q.result);}); })()`);
  check(await page.eval(`legacy.version === 1 && legacy.objectStoreNames.contains('legacy')`), 'aborted upgrade rolls back schema transaction atomically');
  check(await peers.eval(`(async () => { window.blocked = new pc.IndexedDbStorage(); return await blocked.run({kind:'capture'}) === null; })()`), 'upgrade blocked by old tab falls back finitely');
  await page.eval(`legacy.close();`);
  check(await peers.eval(`(async () => { window.peer = new pc.IndexedDbStorage(); window.migrated = await peer.run({kind:'capture'}); return !!migrated; })()`), 'new open migrates after blocked attempt aborts; no reset loop');
  await page.eval(`(async () => { window.s = new pc.IndexedDbStorage(); window.beforeEviction = await s.run({kind:'capture'}); s.close(); })()`);
  await peers.eval(`(async () => { peer.close(); await new Promise((resolve,reject)=>{const q=indexedDB.deleteDatabase(pc.DATABASE);q.onsuccess=resolve;q.onerror=reject;}); window.peer = new pc.IndexedDbStorage(); await peer.run({kind:'capture'}); })()`);
  check(await page.eval(`(async () => { window.s = new pc.IndexedDbStorage(); return await s.run({kind:'write',ticket:beforeEviction,text:fixture()}) !== true; })()`), 'browser namespace eviction/recreation uses new incarnation, rejecting old epoch zero writer');
  await peers.eval(`(async () => { peer.close(); window.future = await new Promise(resolve=>{const q=indexedDB.open(pc.DATABASE,3);q.onsuccess=()=>resolve(q.result);}); })()`);
  check(await page.eval(`(async () => { return await s.run({kind:'capture'}) === null && await new pc.IndexedDbStorage().run({kind:'capture'}) === null && localStorage.getItem('vink-private-sentinel') === 'untouched'; })()`), 'versionchange closes old connection; unsupported newer schema remains untouched, private namespace preserved');
  await peers.eval(`future.close();`);
  await page.eval(`(async () => { await new Promise(resolve=>{const q=indexedDB.deleteDatabase(pc.DATABASE);q.onsuccess=resolve;}); window.s = new pc.IndexedDbStorage(); const ticket = await s.run({kind:'capture'}); window.restartText = fixture('restart'); await s.run({kind:'write',ticket,text:restartText}); s.close(); })()`);
  return { text: await page.eval(`restartText`) };
}
export async function storageReopened(page, args, check, state) {
  await setup(page, args);
  const result = await page.eval(`(async()=>{const s=new pc.IndexedDbStorage();const t=await s.run({kind:'capture'});const text=await s.run({kind:'read',ticket:t,key:pc.keyOf({Reader:'restart'})});s.close();return text;})()`);
  check(result === state.text, 'Chrome process restart with same profile restores exact envelope and original validated_at; no monotonic timestamp persisted');
}
