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
      expected: integer(c.expected), value: string(c.value), key: integer(c.key),
      body: integer(c.body),
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
    const receipts = state.receipts['#map'].map(([key, receipt]) => {
      const [actor, commandKey] = tuple(key);
      const target = integer(receipt.target);
      return {
        actor, key: commandKey, target: target === 0 ? null : target, body: integer(receipt.body),
        comment: integer(receipt.comment),
      };
    }).sort((left, right) => left.actor - right.actor || left.key - right.key);
    const answer = integer(state.answer);
    return {
      command, outcome: string(state.outcome), answer: answer === 0 ? null : answer,
      mode: string(state.mode), generation: integer(state.generation),
      grants: sortedSet(state.grants), receipts, comments,
    };
  });
}

function tuple(value) {
  assert(Array.isArray(value?.['#tup']), 'Expected an ITF tuple');
  return value['#tup'].map(integer);
}

const kindMap = value => Object.fromEntries(value['#map'].map(([kind, count]) =>
  [string(kind), integer(count)]).sort(([left], [right]) => left.localeCompare(right)));

// Reaction states: comment status, mode, every (actor, comment) reaction and both tallies.
function projectReaction(file, expectedSteps, status) {
  const trace = JSON.parse(readFileSync(file, 'utf8'));
  assert.equal(trace['#meta'].status, status);
  assert.equal(trace['#meta'].format, 'ITF');
  assert.equal(trace.states.length, expectedSteps + 1, 'No empty/truncated scenario passes');
  const [init] = trace.states;
  assert.equal(init.outcome, 'init');
  assert.equal(init.mode, 'open', 'A new public work starts open');
  return trace.states.slice(1).map((state, index) => {
    assert.equal(integer(state.cursor), index + 1);
    const c = state.command;
    const command = {
      op: string(c.op), actor: integer(c.actor), id: integer(c.id),
      expected: integer(c.expected), value: string(c.value),
    };
    const comments = state.comments['#map'].map(([id, status]) => ({
      id: integer(id), status: string(status),
      tally: kindMap(state.tally['#map'].find(([tallied]) => integer(tallied) === integer(id))[1]),
    })).sort((left, right) => left.id - right.id);
    const reactions = state.reactions['#map'].map(([key, reaction]) => {
      const [actor, id] = tuple(key);
      return { actor, id, kind: string(reaction.kind), generation: integer(reaction.generation) };
    }).sort((left, right) => left.id - right.id || left.actor - right.actor);
    const visible = state.visibleTally['#map'].map(([id, tally]) => ({
      id: integer(id), tally: kindMap(tally),
    })).sort((left, right) => left.id - right.id);
    return {
      command, outcome: string(state.outcome), mode: string(state.mode), comments, reactions,
      visible_tally: visible,
    };
  });
}

const pairsOf = value => value['#map'].map(([key, item]) => [tuple(key), item]);

// Endorsement states: the request the step carried and every projection the replay compares.
function projectEndorsement(file, expectedSteps, status) {
  const trace = JSON.parse(readFileSync(file, 'utf8'));
  assert.equal(trace['#meta'].status, status);
  assert.equal(trace['#meta'].format, 'ITF');
  assert.equal(trace.states.length, expectedSteps + 1, 'No empty/truncated scenario passes');
  const [init] = trace.states;
  assert.equal(init.outcome, 'init');
  assert.equal(integer(init.published), 0, 'A work starts without a public publication');
  return trace.states.slice(1).map((state, index) => {
    assert.equal(integer(state.cursor), index + 1);
    const c = state.command;
    assert.equal(typeof c.keep, 'boolean');
    assert.equal(typeof c.heard, 'boolean');
    const command = {
      op: string(c.op), stamp: integer(c.stamp), stream: integer(c.stream),
      actor: integer(c.actor), target: integer(c.target), expected: integer(c.expected),
      desire: string(c.desire), observed: integer(c.observed), keep: c.keep, heard: c.heard,
      value: string(c.value),
    };
    // Keyed objects keep the fixture small; an absent row is `Endorsement::ABSENT` (sel 0,
    // version 1), so listing only the others still pins every row.
    const keyed = entries => Object.fromEntries(entries.sort(([left], [right]) =>
      left.localeCompare(right)));
    const comments = keyed(state.status['#map'].map(([id, value]) =>
      [String(integer(id)), string(value)]));
    const rows = keyed(pairsOf(state.rows).map(([[actor, target], row]) =>
      [`${actor}/${target}`, { sel: integer(row.sel), version: integer(row.version) }])
      .filter(([, row]) => row.sel !== 0 || row.version !== 1));
    const counts = keyed(pairsOf(state.counts).map(([[target, anchor], count]) =>
      [`${target}/${anchor}`, integer(count)]));
    const summary = keyed(state.summary['#map'].map(([target, count]) => {
      const value = integer(count);
      return [String(integer(target)), value < 0 ? null : value];
    }));
    const views = keyed(pairsOf(state.views).map(([[actor, target], view]) =>
      [`${actor}/${target}`, string(view)]));
    return {
      command, outcome: string(state.outcome), answer: integer(state.answer),
      published: integer(state.published), mode: string(state.mode), comments, rows, counts,
      summary, views,
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
  run(['typecheck', 'reaction.qnt']);
  run(['typecheck', 'endorsement.qnt']);
  console.log(`PASS Quint ${version}: typecheck all four models (including the mutation actions)`);

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
    ['comments_mode', 'modeStep', 18],
    ['submission_retry', 'retryStep', 12],
  ]) {
    const out = join(temporary, `${name}.itf.json`);
    simulation('lifecycle.qnt', step, length, 1, 'lifecycleSafe', out);
    lifecycle.push({ name, steps: projectLifecycle(out, length, 'ok') });
  }
  // Non-vacuity: the scenarios together reach every outcome and every viewer projection.
  const outcomes = new Set(lifecycle.flatMap(s => s.steps.map(step => step.outcome)));
  for (const expected of ['created', 'closed', 'target_unavailable', 'changed', 'unchanged',
    'reaffirmed', 'already_current', 'stale', 'removed', 'not_work_author', 'not_comment_writer',
    'hidden', 'not_moderator', 'recused', 'granted', 'already_active', 'revoked', 'not_active',
    'replayed', 'key_reused']) {
    assert(outcomes.has(expected), `lifecycle scenarios never reach outcome ${expected}`);
  }
  // Non-vacuity of confirmedModeFence: a reaffirmation followed by the older request it fences.
  const modeSteps = lifecycle.find(s => s.name === 'comments_mode').steps;
  assert(modeSteps.some((step, index) => step.outcome === 'reaffirmed' &&
    modeSteps[index + 1]?.outcome === 'stale' &&
    modeSteps[index + 1].command.expected === step.command.expected &&
    modeSteps[index + 1].command.value !== step.command.value),
    'comments_mode never delays an older request behind a reaffirmation');
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

  const receipt = join(temporary, 'receipt-retry.itf.json');
  const receiptCommand = simulation('lifecycle.qnt', 'mutatedReceiptStep', 2, 1,
    'idempotentSubmission', receipt, 1);
  const receiptSteps = projectLifecycle(receipt, 2, 'violation');
  const duplicated = receiptSteps.at(-1);
  // The counterexample must be a second durable effect for one (actor, key, payload).
  assert.equal(duplicated.outcome, 'created');
  assert.deepEqual(receiptSteps.map(step => [step.command.actor, step.command.key, step.command.body]),
    [[2, 5, 1], [2, 5, 1]]);
  assert.equal(duplicated.comments.filter(comment => comment.writer === 2).length, 2);
  fixture('receipt-retry-counterexample.json', {
    format: 'vink-discussion-counterexample-v1', quint_version: version,
    command: ['quint', ...receiptCommand.filter(arg => !arg.startsWith('--out-itf='))],
    status: 'violation', steps: receiptSteps,
  });
  console.log('PASS negative control: ignoring the receipt creates a second comment for one key and violates idempotentSubmission at transition 2');

  const noopMode = join(temporary, 'mode-noop-fence.itf.json');
  const noopModeCommand = simulation('lifecycle.qnt', 'mutatedNoopModeStep', 3, 1,
    'confirmedModeFence', noopMode, 1);
  const noopModeSteps = projectLifecycle(noopMode, 3, 'violation');
  // The counterexample must be the COM-07 trace: tab B's request for the current mode at
  // generation 2 fenced nothing, so tab A's older reopen at generation 2 was applied after it.
  const [, keepClosed, reopened] = noopModeSteps;
  assert.deepEqual([keepClosed.command.expected, keepClosed.command.value, keepClosed.outcome,
    keepClosed.mode, keepClosed.generation], [2, 'closed', 'already_current', 'closed', 2]);
  assert.deepEqual([reopened.command.expected, reopened.command.value, reopened.outcome,
    reopened.mode, reopened.generation], [2, 'open', 'changed', 'open', 3]);
  fixture('mode-noop-fence-counterexample.json', {
    format: 'vink-discussion-counterexample-v1', quint_version: version,
    command: ['quint', ...noopModeCommand.filter(arg => !arg.startsWith('--out-itf='))],
    status: 'violation', steps: noopModeSteps,
  });
  console.log('PASS negative control: a no-op for the current mode lets an older reopen through and violates confirmedModeFence at transition 3');

  simulation('reaction.qnt', 'step', 40, 1000, 'reactionSafe');
  console.log(`PASS reaction simulation: seed=${seed}, samples=1000, max_steps=40, principals=1..3, comments=1..2, kinds=heart/moved`);

  const reactionLength = 28;
  const reactionOut = join(temporary, 'desired_state.itf.json');
  simulation('reaction.qnt', 'desiredStateStep', reactionLength, 1, 'reactionSafe', reactionOut);
  const reactionSteps = projectReaction(reactionOut, reactionLength, 'ok');
  const reactionOutcomes = new Set(reactionSteps.map(step => step.outcome));
  for (const expected of ['changed', 'reaffirmed', 'already_current', 'stale',
    'target_unavailable', 'own_comment', 'closed', 'moderated', 'mode_set', 'mode_reaffirmed']) {
    assert(reactionOutcomes.has(expected), `reaction scenario never reaches outcome ${expected}`);
  }
  // Non-vacuity: a reaffirmation while closed, and an older request delayed behind one.
  assert(reactionSteps.some(step => step.outcome === 'reaffirmed' && step.mode === 'closed' &&
    step.command.value !== 'none'));
  assert(reactionSteps.some((step, index) => step.outcome === 'reaffirmed' &&
    reactionSteps[index + 1]?.outcome === 'stale' &&
    reactionSteps[index + 1].command.actor === step.command.actor &&
    reactionSteps[index + 1].command.id === step.command.id &&
    reactionSteps[index + 1].command.expected === step.command.expected));
  // Non-vacuity: replacement, a hidden comment that still stores reactions, a nonzero count.
  assert(reactionSteps.some(step => step.command.op === 'react' && step.outcome === 'changed' &&
    step.command.value !== 'none' && step.command.expected > 1));
  assert(reactionSteps.some(step => step.comments.some(comment => comment.status === 'removed' &&
    Object.values(comment.tally).some(count => count > 0))));
  fixture('reaction-fixtures.json', {
    format: 'vink-discussion-reaction-conformance-v1', quint_version: version, work_author: 1,
    writers: { 1: 2, 2: 3 }, scenarios: [{ name: 'desired_state', steps: reactionSteps }],
  });

  for (const [name, step, length, invariant, check] of [
    ['reaction-toggle-counterexample.json', 'mutatedToggleStep', 3, 'noBlindToggle', steps => {
      // A retry of Heart cleared it: the toggle the desired-state rule forbids.
      const last = steps.at(-1);
      assert.equal(last.command.value, 'heart');
      assert.equal(steps.at(-2).reactions.find(r => r.actor === 3 && r.id === 1).kind, 'heart');
      assert.equal(last.reactions.find(r => r.actor === 3 && r.id === 1).kind, 'none');
    }],
    ['reaction-unfenced-counterexample.json', 'mutatedUnfencedStep', 4, 'generationFence', steps => {
      // A delayed request from generation 1 brought Heart back after the clear at generation 2.
      const last = steps.at(-1);
      assert.equal(last.command.expected, 1);
      assert.equal(steps.at(-2).reactions.find(r => r.actor === 3 && r.id === 1).generation, 3);
      assert.equal(last.reactions.find(r => r.actor === 3 && r.id === 1).kind, 'heart');
    }],
    ['reaction-noop-fence-counterexample.json', 'mutatedNoopStep', 3, 'confirmedIntentFence', steps => {
      // Tab B's "no reaction" at generation 1 was a no-op that fenced nothing, so tab A's older
      // Heart at generation 1 was applied after it; generationFence alone does not see this.
      const [, keepNone, late] = steps;
      const mine = step => step.reactions.find(r => r.actor === 3 && r.id === 1);
      assert.deepEqual([keepNone.command.expected, keepNone.command.value, keepNone.outcome],
        [1, 'none', 'already_current']);
      assert.deepEqual(mine(keepNone), { actor: 3, id: 1, kind: 'none', generation: 1 });
      assert.deepEqual([late.command.expected, late.command.value, late.outcome], [1, 'heart', 'changed']);
      assert.deepEqual(mine(late), { actor: 3, id: 1, kind: 'heart', generation: 2 });
    }],
  ]) {
    const out = join(temporary, name.replace('.json', '.itf.json'));
    const args = simulation('reaction.qnt', step, length, 1, invariant, out, 1);
    const steps = projectReaction(out, length, 'violation');
    check(steps);
    fixture(name, {
      format: 'vink-discussion-counterexample-v1', quint_version: version,
      command: ['quint', ...args.filter(arg => !arg.startsWith('--out-itf='))],
      status: 'violation', steps,
    });
    console.log(`PASS negative control: ${step} violates ${invariant} at transition ${length}`);
  }
  simulation('endorsement.qnt', 'step', 40, 1000, 'endorsementSafe');
  console.log(`PASS endorsement simulation: seed=${seed}, samples=1000, max_steps=40, principals=1..3, streams=1..4, targets=work/root/reply, revisions=1..2`);

  const endorsement = [];
  for (const [name, step, length] of [
    ['reorder_and_retry', 'reorderStep', 21],
    ['revision_anchor', 'anchorStep', 21],
    ['comment_targets', 'commentStep', 28],
  ]) {
    const out = join(temporary, `${name}.itf.json`);
    simulation('endorsement.qnt', step, length, 1, 'endorsementSafe', out);
    endorsement.push({ name, steps: projectEndorsement(out, length, 'ok') });
  }
  // Non-vacuity: every decision outcome and projection the model distinguishes is reached.
  const endorsementOutcomes = new Set(endorsement.flatMap(s => s.steps.map(step => step.outcome)));
  for (const expected of ['changed', 'reaffirmed', 'already_current', 'stale',
    'target_unavailable', 'own_content', 'revision_changed', 'comments_closed', 'issued',
    'read', 'read_hidden', 'published', 'withdrawn', 'moderated', 'mode_set']) {
    assert(endorsementOutcomes.has(expected), `endorsement scenarios never reach outcome ${expected}`);
  }
  const endorsementViews = new Set(endorsement.flatMap(s => s.steps.flatMap(step =>
    Object.values(step.views))));
  for (const expected of ['unavailable', 'none', 'endorsed', 'earlier_revision']) {
    assert(endorsementViews.has(expected), `endorsement scenarios never reach view ${expected}`);
  }
  // The adversarial trace itself: B (clear@1, stamp 2) is reaffirmed before the older A
  // (endorse@1, stamp 1) arrives, which is then stale. Each target kind is endorsed.
  const [reorder] = endorsement;
  assert.deepEqual(reorder.steps.slice(3, 5).map(step =>
    [step.command.stamp, step.command.desire, step.command.expected, step.outcome]),
  [[2, 'clear', 1, 'reaffirmed'], [1, 'select', 1, 'stale']]);
  for (const target of [0, 1, 2]) {
    assert(endorsement.some(s => s.steps.some(step => step.outcome === 'changed' &&
      step.command.target === target && step.command.desire === 'select')),
    `no endorsement of target ${target} is applied`);
  }
  assert(endorsement.some(s => s.steps.some(step => step.views['2/0'] === 'earlier_revision' &&
    step.summary['0'] !== null)), 'an earlier-revision endorsement next to a visible count');
  fixture('endorsement-fixtures.json', {
    format: 'vink-endorsement-conformance-v1', quint_version: version, policy: 'lve-core-v1',
    work_author: 1, writers: { 1: 2, 2: 3 }, scenarios: endorsement,
  });

  for (const [name, step, length, invariant, check] of [
    ['endorsement-unfenced-counterexample.json', 'mutatedUnfencedStep', 5, 'noRevival', steps => {
      // Without the fence the newer clear was a no-op, so the older endorse (stamp 1) applied
      // after the server had confirmed the clear (stamp 2).
      const [, , , confirmedClear, revived] = steps;
      assert.deepEqual([confirmedClear.command.stamp, confirmedClear.outcome], [2, 'already_current']);
      assert.deepEqual([revived.command.stamp, revived.command.expected, revived.outcome],
        [1, 1, 'changed']);
      assert.equal(revived.rows['3/0'].sel, 1);
    }],
    ['endorsement-version-reset-counterexample.json', 'mutatedResetStep', 6, 'noRevival', steps => {
      // Deleting the cleared row restarted its version at 1, so the duplicate select applied.
      assert.equal(steps[3].rows['2/0'].version, 2);
      assert.equal(steps[4].rows['2/0'], undefined, 'the cleared row is gone: version 1 again');
      const last = steps.at(-1);
      assert.deepEqual([last.command.stamp, last.outcome], [1, 'changed']);
      assert.equal(last.rows['2/0'].sel, 1);
    }],
    ['endorsement-transfer-counterexample.json', 'mutatedTransferStep', 4, 'noSilentTransfer', steps => {
      // The reader read revision 1; the endorsement landed on revision 2.
      const last = steps.at(-1);
      assert.deepEqual([last.command.observed, last.published, last.outcome], [1, 2, 'changed']);
      assert.equal(last.rows['2/0'].sel, 2);
    }],
  ]) {
    const out = join(temporary, name.replace('.json', '.itf.json'));
    const args = simulation('endorsement.qnt', step, length, 1, invariant, out, 1);
    const steps = projectEndorsement(out, length, 'violation');
    check(steps);
    fixture(name, {
      format: 'vink-endorsement-counterexample-v1', quint_version: version,
      command: ['quint', ...args.filter(arg => !arg.startsWith('--out-itf='))],
      status: 'violation', steps,
    });
    console.log(`PASS negative control: ${step} violates ${invariant} at transition ${length}`);
  }
  console.log('NOT_RUN Apalache/model checking; Rust conformance is a separate cargo test');
  rmSync(temporary, { recursive: true });
} catch (error) {
  console.error(`FAIL ${error.message}\nIntermediate ITF traces, if any: ${temporary}`);
  process.exitCode = 1;
}
