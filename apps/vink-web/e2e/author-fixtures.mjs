import { execFileSync } from 'node:child_process';

const scratch = process.env.VINK_HANDLE_SCRATCH;
delete process.env.VINK_HANDLE_SCRATCH; // The fixture helper is the only consumer.

export function authorFixtureSql(sql) {
  const db = scratch && new URL(scratch);
  if (!db || db.pathname !== '/vink_verify_web_password_auth')
    throw new Error('missing password scenario scratch profile');
  return execFileSync('psql', ['-XAtq', '-v', 'ON_ERROR_STOP=1'], {
    env: {
      ...process.env,
      PGDATABASE: db.pathname.slice(1),
      PGHOST: db.hostname,
      PGPORT: db.port,
      PGUSER: decodeURIComponent(db.username),
      PGPASSWORD: decodeURIComponent(db.password),
    },
    input: sql,
    encoding: 'utf8',
  }).trim();
}
