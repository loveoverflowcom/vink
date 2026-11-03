// One model oracle: export real Quint ITF states; never recompute transitions in JS.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const directory = dirname(fileURLToPath(import.meta.url));
const quint = join(directory, 'node_modules/@informalsystems/quint/dist/src/cli.js');
const version = '0.33.1';
const seed = '41';
const write = process.argv.slice(2).includes('--write');
assert(process.argv.slice(2).every(arg => arg === '--write'), 'Usage: node verify.mjs [--write]');
const temporary = mkdtempSync(join(tmpdir(), 'vink-discussion-'));

function run(args, expectedExit = 0) {
  const result = spawnSync(process.execPath, [quint, ...args], {
    cwd: directory, encoding: 'utf8', timeout: 120_000,
  });
  const output = (result.stdout ?? '') + (result.stderr ?? '');
  if (result.error || result.status !== expectedExit) {
    throw new Error(`quint ${args.join(' ')}\n${output}\n${result.error ?? `exit ${result.status}`}`);
  }
  return output;
}

function simulation(model, step, maxSteps, samples, invariant, out, expectedExit = 0) {
  const args = ['run', model, '--backend=typescript', `--seed=${seed}`,
    `--step=${step}`, `--max-steps=${maxSteps}`, `--max-samples=${samples}`,
    `--invariant=${invariant}`, '--verbosity=1'];
  if (out) args.push(`--out-itf=${out}`);
  const output = run(args, expectedExit);
  // Exit failure alone cannot count as sensitivity evidence (syntax/runtime failure, etc.).
  if (expectedExit === 1) assert.match(output, /Invariant violated/);
  else assert.match(output, /No violation found/);
  return args;
}

function integer(value) {
  assert(value && typeof value['#bigint'] === 'string', 'Expected an ITF integer');
  const result = Number(value['#bigint']);
  assert(Number.isSafeInteger(result), 'ITF integer must fit the fixture schema');
  return result;
}

function projectTrace(file, expectedSteps, status) {
  const trace = JSON.parse(readFileSync(file, 'utf8'));
  assert.equal(trace['#meta'].status, status);
  assert.equal(trace['#meta'].format, 'ITF');
  assert.equal(trace.states.length, expectedSteps + 1, 'No empty/truncated scenario passes');
  assert.equal(trace.states[0].outcome, 'init');
  assert.deepEqual(trace.states[0].placements['#map'], []);
  return trace.states.slice(1).map((state, index) => {
    assert.equal(integer(state.cursor), index + 1);
    assert(['root', 'reply'].includes(state.command.op));
    assert(['created', 'duplicate', 'missing'].includes(state.outcome));
    const command = {
      op: state.command.op, id: integer(state.command.id), target: integer(state.command.target),
    };
    const placements = state.placements['#map'].map(([id, placement]) => {
      const replyTo = integer(placement.reply_to);
      return { id: integer(id), root: integer(placement.root), reply_to: replyTo === 0 ? null : replyTo };
    }).sort((left, right) => left.id - right.id);
    return { command, outcome: state.outcome, state: placements };
  });
}

const string = value => { assert.equal(typeof value, 'string'); return value; };
const sortedSet = value => value['#set'].map(integer).sort((left, right) => left - right);

// Lifecycle states: comments with their exported viewer projection (viewers 0..3, 0 anonymous).
function projectLifecycle(file, expectedSteps, status) {
  const trace = JSON.parse(readFileSync(file, 'utf8'));
  assert.equal(trace['#meta'].status, status);
  assert.equal(trace['#meta'].format, 'ITF');
  assert.equal(trace.states.length, expectedSteps + 1, 'No empty/truncated scenario passes');
  const [init] = trace.states;
  assert.equal(init.outcome, 'init');
  assert.deepEqual(init.comments['#map'], []);
  assert.equal(init.mode, 'open', 'A new public work starts open');
  assert.equal(integer(init.generation), 1);
  return trace.states.slice(1).map((state, index) => {
    assert.equal(integer(state.cursor), index + 1);
    const c = state.command;
    const command = {
      op: string(c.op), actor: integer(c.actor), id: integer(c.id), target: integer(c.target),
      expected: integer(c.expected), value: string(c.value),
    };
    const views = new Map(state.views['#map'].map(([id, byViewer]) => [integer(id),
      byViewer['#map'].map(([viewer, seen]) => [integer(viewer), string(seen)])
        .sort((left, right) => left[0] - right[0]).map(([, seen]) => seen)]));
    const comments = state.comments['#map'].map(([id, comment]) => {
      const target = integer(comment.target);
      return {
        id: integer(id), writer: integer(comment.writer), target: target === 0 ? null : target,
        review: string(comment.review), removal: string(comment.removal),
        version: integer(comment.version), views: views.get(integer(id)),
      };
    }).sort((left, right) => left.id - right.id);
    assert.equal(views.size, comments.length);
    assert(comments.every(comment => comment.views?.length === 4));
    return {
      command, outcome: string(state.outcome), mode: string(state.mode),
      generation: integer(state.generation), grants: sortedSet(state.grants), comments,
    };
  });
}

function fixture(name, data) {
  const path = join(directory, name);
  const content = JSON.stringify(data, null, 2) + '\n';
  if (write) {
    writeFileSync(path, content);
    console.log(`WRITE ${name}`);
  } else {
    assert.equal(readFileSync(path, 'utf8'), content,
      `${name} differs from Quint output. Review the model change, then use --write explicitly.`);
    console.log(`PASS ${name}: matches generated Quint output`);
  }
}

try {
  assert.equal(run(['--version']).trim(), version, 'Use the pinned Quint version');
  run(['typecheck', 'structure.qnt']);
  run(['typecheck', 'lifecycle.qnt']);
  console.log(`PASS Quint ${version}: typecheck both models (including the mutation actions)`);

  simulation('structure.qnt', 'step', 40, 1000, 'structureSafe');
  console.log(`PASS simulation: seed=${seed}, samples=1000, max_steps=40, IDs=1..4, backend=typescript`);

  const scenarios = [];
  for (const [name, step, length] of [
    ['chain_and_rejections', 'chainStep', 10],
    ['separate_roots', 'rootsStep', 5],
  ]) {
    const out = join(temporary, `${name}.itf.json`);
    simulation('structure.qnt', step, length, 1, 'structureSafe', out);
    scenarios.push({ name, steps: projectTrace(out, length, 'ok') });
  }
  fixture('fixtures.json', {
    format: 'vink-discussion-conformance-v1', quint_version: version, scenarios,
  });

  const out = join(temporary, 'root-normalization.itf.json');
  const command = simulation('structure.qnt', 'mutatedChainStep', 10, 1, 'rootIsRoot', out, 1);
  const steps = projectTrace(out, 6, 'violation');
  const last = steps.at(-1);
  // Validate the actual counterexample, not merely a nonzero process exit.
  assert(last.state.some(p => last.state.some(root => root.id === p.root && root.root !== root.id)));
  fixture('root-normalization-counterexample.json', {
    format: 'vink-discussion-counterexample-v1', quint_version: version,
    command: ['quint', ...command.filter(arg => !arg.startsWith('--out-itf='))],
    status: 'violation', steps,
  });
  console.log('PASS negative control: type-correct unnormalized root violates rootIsRoot at transition 6');

  simulation('lifecycle.qnt', 'step', 40, 1000, 'lifecycleSafe');
  console.log(`PASS lifecycle simulation: seed=${seed}, samples=1000, max_steps=40, principals=1..3, IDs=1..3`);

  const lifecycle = [];
  for (const [name, step, length] of [
    ['moderation_authority', 'moderationStep', 26],
    ['comments_mode', 'modeStep', 16],
  ]) {
    const out = join(temporary, `${name}.itf.json`);
    simulation('lifecycle.qnt', step, length, 1, 'lifecycleSafe', out);
    lifecycle.push({ name, steps: projectLifecycle(out, length, 'ok') });
  }
  // Non-vacuity: the scenarios together reach every outcome and every viewer projection.
  const outcomes = new Set(lifecycle.flatMap(s => s.steps.map(step => step.outcome)));
  for (const expected of ['created', 'closed', 'target_unavailable', 'changed', 'unchanged',
    'stale', 'removed', 'not_work_author', 'not_comment_writer', 'hidden', 'not_moderator',
    'recused', 'granted', 'already_active', 'revoked', 'not_active']) {
    assert(outcomes.has(expected), `lifecycle scenarios never reach outcome ${expected}`);
  }
  const seen = new Set(lifecycle.flatMap(s => s.steps.flatMap(step =>
    step.comments.flatMap(comment => comment.views))));
  for (const expected of ['public', 'own_pending', 'own_rejected', 'unavailable']) {
    assert(seen.has(expected), `lifecycle scenarios never reach view ${expected}`);
  }
  fixture('lifecycle-fixtures.json', {
    format: 'vink-discussion-lifecycle-conformance-v1', quint_version: version,
    work_author: 1, scenarios: lifecycle,
  });

  const recusal = join(temporary, 'author-recusal.itf.json');
  const recusalCommand = simulation('lifecycle.qnt', 'mutatedRecusalStep', 3, 1, 'writeAuthority',
    recusal, 1);
  const recusalSteps = projectLifecycle(recusal, 3, 'violation');
  const violated = recusalSteps.at(-1);
  // The counterexample must be the rule itself: the work author, holding a grant, changed
  // someone else's comment on their own work.
  assert.equal(violated.command.op, 'moderate');
  assert.equal(violated.command.actor, 1);
  assert.deepEqual(violated.grants, [1]);
  assert(violated.comments.some(comment => comment.writer !== 1 && comment.removal === 'moderation'));
  fixture('author-recusal-counterexample.json', {
    format: 'vink-discussion-counterexample-v1', quint_version: version,
    command: ['quint', ...recusalCommand.filter(arg => !arg.startsWith('--out-itf='))],
    status: 'violation', steps: recusalSteps,
  });
  console.log('PASS negative control: a work author moderating their own work violates writeAuthority at transition 3');
  console.log('NOT_RUN Apalache/model checking; Rust conformance is a separate cargo test');
  rmSync(temporary, { recursive: true });
} catch (error) {
  console.error(`FAIL ${error.message}\nIntermediate ITF traces, if any: ${temporary}`);
  process.exitCode = 1;
}
