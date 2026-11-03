// Real Quint ITF output is the oracle; JS projects data and never reimplements transitions.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
const dir = dirname(fileURLToPath(import.meta.url));
const cli = join(dir, 'node_modules/@informalsystems/quint/dist/src/cli.js');
const temp = mkdtempSync(join(tmpdir(), 'vink-emo-model-'));
const write = process.argv.includes('--write');
assert(process.argv.slice(2).every(a => a === '--write'));
function run(args, status = 0) {
  const p = spawnSync(process.execPath, [cli, ...args], { cwd: dir, encoding: 'utf8', timeout: 120000 });
  assert(!p.error, String(p.error));
  assert.equal(p.status, status, (p.stdout ?? '') + (p.stderr ?? ''));
  return (p.stdout ?? '') + (p.stderr ?? '');
}
function simulation(step, count, samples, file, negative = false) {
  const args = ['run', 'lifecycle.qnt', '--backend=typescript', '--seed=25', `--step=${step}`, `--max-steps=${count}`, `--max-samples=${samples}`, '--invariant=lifecycleSafe', '--verbosity=1'];
  if (file) args.push(`--out-itf=${file}`);
  const output = run(args, negative ? 1 : 0);
  assert.match(output, negative ? /Invariant violated/ : /No violation found/);
  return args.filter(a => !a.startsWith('--out-itf='));
}
const int = v => { assert.equal(typeof v?.['#bigint'], 'string'); const n = Number(v['#bigint']); assert(Number.isSafeInteger(n)); return n; };
function steps(file, length, status) {
  const trace = JSON.parse(readFileSync(file));
  assert.equal(trace['#meta'].status, status); assert.equal(trace.states.length, length + 1);
  return trace.states.slice(1).map((state, i) => {
    assert.equal(int(state.cursor), i + 1);
    return { command: { ...state.command, id: int(state.command.id), expected: int(state.command.expected) },
      outcome: state.outcome, active: state.players['#map'].filter(([, p]) => ['loading', 'playing'].includes(p.phase)).length,
      players: state.players['#map'].map(([id, p]) => ({ id: int(id), phase: p.phase, generation: int(p.generation) })).sort((a, b) => a.id - b.id) };
  });
}
function fixture(file, value) {
  const text = JSON.stringify(value, null, 2) + '\n';
  if (write) writeFileSync(join(dir, file), text);
  else assert.equal(readFileSync(join(dir, file), 'utf8'), text, `trace drift: ${file}`);
}
try {
  assert.equal(run(['--version']).trim(), '0.33.1');
  run(['typecheck', 'lifecycle.qnt']);
  simulation('step', 40, 1000);
  const path = join(temp, 'scenario.itf.json');
  simulation('scenarioStep', 33, 1, path);
  const scenario = steps(path, 33, 'ok');
  for (const phase of ['loading', 'playing', 'static', 'error', 'disposed']) assert(scenario.some(s => s.players.some(p => p.phase === phase)), `unreachable phase: ${phase}`);
  for (const outcome of ['load', 'play', 'budget', 'already_active', 'complete', 'ignored', 'cancelled', 'error', 'disposed', 'static']) assert(scenario.some(s => s.outcome === outcome), `unreachable outcome: ${outcome}`);
  fixture('fixtures.json', { format: 'vink-expression-lifecycle-v1', quint_version: '0.33.1', capacity: 2, scenarios: [{ name: 'lifecycle', steps: scenario }] });
  const latePath = join(temp, 'late.itf.json');
  simulation('lateStep', 3, 1, latePath);
  const late = steps(latePath, 3, 'ok');
  assert.equal(late.at(-1).outcome, 'ignored');
  const badPath = join(temp, 'bad.itf.json');
  const command = simulation('unguardedLateStep', 3, 1, badPath, true);
  const bad = steps(badPath, 3, 'violation');
  assert.equal(bad.at(-2).players[0].phase, 'disposed'); assert.equal(bad.at(-1).players[0].phase, 'playing');
  fixture('late-callback-counterexample.json', { format: 'vink-expression-counterexample-v1', command: ['quint', ...command], status: 'violation', steps: bad });
  console.log('PASS Quint 0.33.1: typecheck; simulation seed=25 samples=1000 steps=40; 33-step production replay fixture; reachable phases/outcomes; missing guard negative control at step 3');
} finally { rmSync(temp, { recursive: true, force: true }); }
