// Use only with local-edge, native Identity with its private local outbox and a disposable PostgreSQL cluster.
import assert from 'node:assert/strict';
import { randomBytes, randomUUID } from 'node:crypto';

import { readFile, stat } from 'node:fs/promises';
import { join } from 'node:path';
import { setTimeout as sleep } from 'node:timers/promises';

const outbox = process.argv[3];
const directory = await stat(outbox);
assert.ok(directory.isDirectory() && (directory.mode & 0o077) === 0, 'private scratch outbox required');
const base = new URL(process.argv[2]);
assert.equal(base.hostname, '127.0.0.1');
assert.equal(base.protocol, 'http:');
const sessions = [];
async function request(path, cookie, data) {
  const headers = { origin:'https://vink.forum' };
  if (cookie) headers.cookie = cookie;
  if (data) headers['content-type'] = 'application/json';
  const response = await fetch(new URL(path, base), {headers, method:data ? 'POST' : 'GET',
    body:data ? JSON.stringify(data) : undefined, redirect:'manual'});
  assert.equal(response.headers.get('cache-control'), 'no-store');
  assert.ok(!['HIT','STALE','REVALIDATED','UPDATING'].includes(response.headers.get('cf-cache-status')));
  return response;
}
for (const penName of ['Tác giả A', 'Tác giả B']) {
  const email = randomUUID() + '@example.test';
  const challenge = await request('/api/v1/auth/password/register/challenges', '', {email});
  assert.equal(challenge.status, 202);
  const proof = challenge.headers.getSetCookie().find(v => v.startsWith('__Host-vink_register='))?.split(';')[0];
  const id = /^__Host-vink_register=vink_c1\.([0-9a-f]{32})\./.exec(proof)?.[1];
  assert.ok(id, 'scratch registration challenge');
  let code;
  for (let tries = 0; tries < 100 && !code; tries++) {
    try {
      const mail = await readFile(join(outbox, id + '.txt'), 'utf8');
      code = /^X-Vink-Code: ([0-9]{6})$/m.exec(mail)?.[1];
    } catch (err) { if (err.code !== 'ENOENT') throw err; }
    if (!code) await sleep(100);
  }
  assert.ok(code, 'registration mail delivered locally');
  const registered = await request('/api/v1/auth/register', proof, {
    email, code, username:'edge' + randomBytes(6).toString('hex'), password:randomUUID(),
  });
  assert.equal(registered.status, 200, 'scratch registration');
  const session = registered.headers.getSetCookie().find(v => v.startsWith('__Host-vink_session='));
  assert.ok(session && /HttpOnly/i.test(session) && /Secure/i.test(session), 'session cookie survives edge');
  const cookie = session.split(';')[0];
  const principal = await request('/api/v1/auth/principal', cookie, {pen_name:penName});
  assert.equal(principal.status, 201);
  sessions.push({cookie, penName, id:(await principal.json()).principal_id});
}
assert.notEqual(sessions[0].id,sessions[1].id);
for (const {cookie,penName,id} of sessions) {
  const profile = await request('/api/v1/me/profile',cookie);
  assert.equal(profile.status,200);
  const own = await profile.json();
  assert.equal(own.principal_id,id);
  assert.equal(own.pen_name,penName);
}
assert.equal((await request('/api/v1/me/profile','')).status,401);
console.log('PASS two scratch Identity sessions then guest on the same profile URL; Set-Cookie, no-store and distinct principals');
