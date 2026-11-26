import { execFileSync } from 'node:child_process';

const scratch = process.env.VINK_HANDLE_SCRATCH;
const deletionScratch = process.env.VINK_ACCOUNT_DELETION_SCRATCH;
delete process.env.VINK_HANDLE_SCRATCH;
delete process.env.VINK_ACCOUNT_DELETION_SCRATCH;

export function authorFixtureSql(sql) {
  return fixtureSql(scratch, sql, false);
}

export function accountDeletionFixtureSql(sql) {
  return fixtureSql(deletionScratch, sql, true);
}

function fixtureSql(connection, sql, loopback) {
  const db = connection && new URL(connection);
  if (!db || db.pathname !== '/vink_verify_web_password_auth' ||
      (loopback && !['127.0.0.1', 'localhost', '[::1]'].includes(db.hostname)))
    throw new Error('missing expected scenario scratch profile');
  return execFileSync('psql', ['-XAtq', '-v', 'ON_ERROR_STOP=1'], {
    env: {
      ...process.env,
      PGDATABASE: db.pathname.slice(1),
      PGHOST: db.hostname.replace(/^\[(.*)\]$/, '$1'),
      PGPORT: db.port,
      PGUSER: decodeURIComponent(db.username),
      PGPASSWORD: decodeURIComponent(db.password),
    },
    input: sql,
    encoding: 'utf8',
  }).trim();
}
