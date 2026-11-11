// Shared Quint runner for the five remaining target specifications and the canonical M1
// in ../publication. It exports real ITF states without recomputing transitions. M1 has
// scoped Rust replay (#150); see ../publication/README.md for its exact action coverage.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { basename, dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const directory = dirname(fileURLToPath(import.meta.url));
// One Quint pin for the repository: the locked package of the discussion models.
const tools = join(directory, '../discussion');
const quint = join(tools, 'node_modules/@informalsystems/quint/dist/src/cli.js');
const version = JSON.parse(readFileSync(join(tools, 'package.json'), 'utf8'))
  .dependencies['@informalsystems/quint'];
const seed = '41';
const samples = 1000;
const maxSteps = 40;
const status = 'TARGET_SPEC_NOT_IMPLEMENTED';
const write = process.argv.slice(2).includes('--write');
assert(process.argv.slice(2).every(arg => arg === '--write'), 'Usage: node verify.mjs [--write]');
const temporary = mkdtempSync(join(tmpdir(), 'vink-target-'));

function run(args, expectedExit = 0) {
  const result = spawnSync(process.execPath, [quint, ...args], {
    cwd: directory, encoding: 'utf8', timeout: 300_000,
  });
  const output = (result.stdout ?? '') + (result.stderr ?? '');
  if (result.error || result.status !== expectedExit) {
    throw new Error(`quint ${args.join(' ')}\n${output}\n${result.error ?? `exit ${result.status}`}`);
  }
  return output;
}

function simulation(model, step, steps, runs, invariant, out, expectedExit = 0) {
  const args = ['run', model, '--backend=typescript', `--seed=${seed}`, `--step=${step}`,
    `--max-steps=${steps}`, `--max-samples=${runs}`, `--invariant=${invariant}`, '--verbosity=1'];
  if (out) args.push(`--out-itf=${out}`);
  const output = run(args, expectedExit);
  // A nonzero exit alone is no sensitivity evidence: a parse or runtime error also exits 1.
  if (expectedExit === 1) assert.match(output, /Invariant violated/);
  else assert.match(output, /No violation found/);
  return args;
}

// ITF values to plain JSON: integers, sorted sets, tuples as arrays, maps as objects when
// their keys are scalars and as sorted {key, value} lists otherwise.
function plain(value) {
  if (Array.isArray(value)) return value.map(plain);
  if (value === null || typeof value !== 'object') return value;
  if (typeof value['#bigint'] === 'string') {
    const number = Number(value['#bigint']);
    assert(Number.isSafeInteger(number), 'ITF integer must fit the vector schema');
    return number;
  }
  const order = (left, right) => JSON.stringify(left).localeCompare(JSON.stringify(right), 'en');
  if (value['#set']) return value['#set'].map(plain).sort((l, r) => (typeof l === 'number' ? l - r : order(l, r)));
  if (value['#tup']) return value['#tup'].map(plain);
  if (value['#map']) {
    const entries = value['#map'].map(([key, item]) => [plain(key), plain(item)]);
    if (entries.every(([key]) => typeof key !== 'object')) {
      return Object.fromEntries(entries.sort(([l], [r]) => (typeof l === 'number' ? l - r : order(l, r))));
    }
    return entries.sort(([l], [r]) => order(l, r)).map(([key, item]) => ({ key, value: item }));
  }
  return Object.fromEntries(Object.entries(value).filter(([key]) => !key.startsWith('#'))
    .map(([key, item]) => [key, plain(item)]));
}

// One step = the command, its outcome and the exported state after the commit.
function project(file, length, expected, fields) {
  const trace = JSON.parse(readFileSync(file, 'utf8'));
  assert.equal(trace['#meta'].status, expected);
  assert.equal(trace['#meta'].format, 'ITF');
  assert.equal(trace.states.length, length + 1, 'No empty/truncated scenario passes');
  assert.equal(trace.states[0].outcome, 'init');
  return trace.states.slice(1).map((state, index) => {
    assert.equal(plain(state.cursor), index + 1);
    return {
      command: plain(state.command), outcome: state.outcome,
      state: Object.fromEntries(fields.map(field => {
        assert(field in state, `state has no field ${field}`);
        return [field, plain(state[field])];
      })),
    };
  });
}

// Steps stay one per line: reviewable diffs without a 2 MB pretty-printed file.
function fixture(name, header, lists) {
  const body = Object.entries(lists).map(([key, groups]) => `  ${JSON.stringify(key)}: [\n${groups.map(group =>
    `    {${Object.entries(group).filter(([field]) => field !== 'steps')
      .map(([field, item]) => `${JSON.stringify(field)}: ${JSON.stringify(item)}`).join(', ')}, "steps": [\n${
      group.steps.map(step => `      ${JSON.stringify(step)}`).join(',\n')}\n    ]}`).join(',\n')}\n  ]`);
  const content = `{\n${Object.entries(header).map(([key, item]) =>
    `  ${JSON.stringify(key)}: ${JSON.stringify(item)}`).concat(body).join(',\n')}\n}\n`;
  JSON.parse(content);
  const path = join(directory, name.startsWith('publication-') || name.startsWith('tombstone-') ? '../publication' : '.', name);
  if (write) {
    writeFileSync(path, content);
    console.log(`WRITE ${name}`);
  } else {
    assert.equal(readFileSync(path, 'utf8'), content,
      `${name} differs from Quint output. Review the model change, then use --write explicitly.`);
    console.log(`PASS ${name}: matches generated Quint output`);
  }
}

const reached = (steps, outcomes, label) => {
  const seen = new Set(steps.map(step => step.outcome));
  for (const expected of outcomes) assert(seen.has(expected), `${label} never reaches outcome ${expected}`);
};
const at = (steps, outcome, nth = 0) => {
  const found = steps.filter(step => step.outcome === outcome)[nth];
  assert(found, `no step ${nth} with outcome ${outcome}`);
  return found;
};
const pair = (list, ...key) => list.find(entry => JSON.stringify(entry.key) === JSON.stringify(key)).value;

const models = [
  {
    file: '../publication/publication.qnt', safe: 'publicationSafe', bounds: 'revisions<=4, jobs<=6, policies<=2',
    fields: ['revs', 'jobs', 'audience', 'desired', 'intended', 'pointer', 'lifecycle', 'generation', 'policy'],
    scenarios: [
      ['audience_moderation_override', 'lifecycleStep', 40],
      ['moderation_removal', 'removalStep', 11],
    ],
    check(all) {
      const [life, removal] = all;
      reached([...life, ...removal], ['saved', 'not_submitted', 'not_found', 'submitted', 'waiting',
        'retryable', 'duplicate', 'requeued', 'not_needed', 'wrong_revision', 'unknown_job', 'published',
        'rejected', 'revision_rejected', 'stale', 'policy_changed', 'stale_policy', 'needs_admin',
        'not_admin', 'overridden', 'made_private', 'reaffirmed', 'unpublished', 'approved_unpublished',
        'deleted', 'work_unavailable', 'target_gone', 'removed'], 'publication');
      // Non-vacuity: r1 stays public while r2 is rejected; an approval without intent publishes
      // nothing; the admin's approval survives the late rejection; nothing returns after deletion.
      const rejected = at(life, 'rejected');
      assert.deepEqual([rejected.command.rev, rejected.state.pointer], [2, 1]);
      const withheld = at(life, 'approved_unpublished');
      assert.deepEqual([withheld.state.pointer, withheld.state.audience, withheld.state.revs[4].decision],
        [0, 'private', 'approved']);
      const overridden = at(life, 'overridden');
      assert.deepEqual([overridden.command.value, overridden.state.revs[3], overridden.state.pointer],
        ['reject', { decision: 'approved', by: 'admin' }, 3]);
      const deleted = life.findIndex(step => step.outcome === 'deleted');
      assert(life.slice(deleted).length > 3 && life.slice(deleted).every(step => step.state.pointer === 0));
      assert(life.some(step => step.command.op === 'snapshot' && Object.keys(step.state.jobs).length === 0),
        'a private save must exist with no evaluation');
    },
    mutants: [
      ['publication-late-result-counterexample.json', 'mutatedOverrideStep', 4, 'adminOverrideWins', steps => {
        // The admin approved r1; the evaluator's late rejection replaced that decision.
        const [before, last] = steps.slice(-2);
        assert.deepEqual(before.state.revs[1], { decision: 'approved', by: 'admin' });
        assert.deepEqual([last.command.op, last.state.revs[1]], ['ai_result', { decision: 'rejected', by: 'ai' }]);
      }],
      ['publication-withdrawn-intent-counterexample.json', 'mutatedIntentStep', 4, 'publicNeedsEverything', steps => {
        // The author went back to "only me"; the approval published the work anyway.
        const last = steps.at(-1);
        assert.deepEqual([last.outcome, last.state.audience, last.state.pointer], ['published', 'private', 1]);
      }],
    ],
  },
  {
    file: '../publication/tombstone.qnt', safe: 'tombstoneSafe', bounds: 'comments<=4, viewers=guest+2',
    fields: ['status', 'everPublic', 'comments', 'views'],
    scenarios: [
      ['author_deleted_public_work', 'deletedStep', 14],
      ['private_then_deleted', 'privateStep', 10],
      ['moderation_removed', 'removedStep', 6],
    ],
    check(all) {
      const [deleted, hidden, removed] = all;
      reached(all.flat(), ['published', 'created', 'reviewed', 'deleted_with_tombstone', 'tombstone_read_only',
        'comment_unavailable', 'comment_removed', 'work_unavailable', 'made_private', 'unchanged', 'deleted',
        'removed_by_moderation'], 'tombstone');
      // A: the old URL is a tombstone without body, preview or listing; the approved comment
      // stays, a rejected or pending one is only its writer's.
      const tomb = at(deleted, 'deleted_with_tombstone').state.views;
      assert.deepEqual(tomb[0], { page: 'tombstone', listed: false, snippet: false, comments: [1] });
      assert.deepEqual([tomb[2].comments, tomb[3].comments], [[1, 3], [1, 2]]);
      // B, C, D: no page and no comment for anybody else.
      const nothing = { page: 'not_found', listed: false, snippet: false, comments: [] };
      for (const state of [at(hidden, 'made_private').state, at(hidden, 'deleted').state,
        at(removed, 'removed_by_moderation').state, deleted[0].state]) {
        assert(Object.keys(state.comments).length > 0 || state.status === 'unpublished');
        for (const viewer of [0, 2, 3]) assert.deepEqual(state.views[viewer], nothing);
      }
      assert.deepEqual(at(hidden, 'published', 1).state.views[0].comments, [1], 'public again restores the thread');
    },
    mutants: [
      ['tombstone-private-as-deleted-counterexample.json', 'mutatedPrivateStep', 4, 'noExceptionForHiddenWork', steps => {
        // The work is private, yet a guest reads its comment through the old link.
        const last = steps.at(-1);
        assert.deepEqual([last.state.status, last.state.views[0].page, last.state.views[0].comments],
          ['private', 'tombstone', [1]]);
      }],
    ],
  },
  {
    file: 'collections.qnt', safe: 'collectionsSafe', bounds: 'posts=3, collections=2, viewers=guest+2',
    fields: ['postState', 'shelf', 'views', 'counts', 'badges', 'links', 'served'],
    scenarios: [['permission_truth_table', 'truthStep', 36]],
    check([steps]) {
      reached(steps, ['not_found', 'created', 'already_exists', 'added', 'already_member', 'not_own_post',
        'post_changed', 'audience_changed', 'unchanged', 'page', 'end', 'cursor_invalid', 'removed',
        'not_member', 'post_unavailable', 'collection_deleted'], 'collections');
      const row = (step, viewer) => [pair(step.state.views, viewer, 1), pair(step.state.counts, viewer, 1)];
      // The four rows of the truth table for a guest, and the owner's own view.
      const bothPrivate = steps[4];
      assert.deepEqual([bothPrivate.state.postState, bothPrivate.state.shelf[1].audience, row(bothPrivate, 0)],
        [{ 1: 'private', 2: 'private', 3: 'private' }, 'private', [[], 0]]);
      const publicPostPrivateShelf = at(steps, 'post_changed');
      assert.deepEqual([row(publicPostPrivateShelf, 0), pair(publicPostPrivateShelf.state.badges, 0, 1)], [[[], 0], []]);
      const publicShelf = at(steps, 'audience_changed');
      assert.deepEqual([row(publicShelf, 0), row(publicShelf, 1)], [[[1], 1], [[1, 2], 2]]);
      const bothPublic = at(steps, 'post_changed', 1);
      assert.deepEqual([row(bothPublic, 0), pair(bothPublic.state.links, 0, 1), pair(bothPublic.state.links, 0, 2)],
        [[[1, 2], 2], [2], [1]]);
      // Paging decides against the state of the request, not what was visible before.
      const pages = steps.filter(step => step.command.op === 'read').map(step => [step.outcome, step.state.served]);
      assert(pages.some(([outcome, served]) => outcome === 'page' && served.length === 1));
      assert(steps.some((step, index) => step.outcome === 'end' && steps[index - 1].command.value === 'pending'));
    },
    mutants: [
      ['collections-or-counterexample.json', 'mutatedOrStep', 3, 'closedCollectionLeaksNothing', steps => {
        // A guest sees the public post listed inside the private collection.
        const last = steps.at(-1);
        assert.equal(last.state.shelf[1].audience, 'private');
        assert.deepEqual([pair(last.state.views, 0, 1), pair(last.state.badges, 0, 1)], [[1], [1]]);
      }],
    ],
  },
  {
    file: 'comment_edit.qnt', safe: 'commentEditSafe', bounds: 'revisions<=4, jobs<=6, keys=1..5',
    fields: ['revs', 'jobs', 'shown', 'candidate', 'firstShown', 'mark', 'version', 'removal', 'receipts',
      'votes', 'seen'],
    scenarios: [
      ['edit_review_override', 'editStep', 31],
      ['removal_during_review', 'removalStep', 9],
    ],
    check(all) {
      const [edit, removal] = all;
      reached(all.flat(), ['target_unavailable', 'published', 'endorsed', 'own_content', 'not_comment_writer',
        'accepted', 'replayed', 'key_reused', 'stale', 'reaffirmed', 'retryable', 'duplicate', 'requeued',
        'not_needed', 'wrong_revision', 'rejected', 'superseded', 'not_admin', 'not_current', 'overridden',
        'upheld', 'unpublished', 'removed', 'target_gone', 'needs_admin', 'unknown_job'], 'comment_edit');
      // The edit waits; people still read the first text, without the mark.
      const waiting = at(edit, 'accepted');
      assert.deepEqual([waiting.state.candidate, waiting.state.seen], [2, { body: 1, edited: false, helpful: 1 }]);
      // Saving the same text: a newer version, no revision, no mark.
      const same = at(edit, 'reaffirmed');
      assert.deepEqual([same.state.version, Object.keys(same.state.revs).length, same.state.seen.edited], [3, 2, false]);
      // The rejected edit leaves the old text; the approved rewrite carries the mark and no old vote.
      assert.deepEqual(at(edit, 'rejected').state.seen, { body: 1, edited: false, helpful: 1 });
      const rewritten = at(edit, 'published', 1);
      assert.deepEqual([rewritten.state.shown, rewritten.state.seen, rewritten.state.votes],
        [4, { body: 1, edited: true, helpful: 0 }, [[3, 1]]]);
      assert.deepEqual(at(removal, 'target_gone').state.seen, { body: 0, edited: false, helpful: 0 });
    },
    mutants: [
      ['comment_edit-overwrite-counterexample.json', 'mutatedOverwriteStep', 2, 'pendingNeverPublic', steps => {
        // The unreviewed edit is what people read.
        const last = steps.at(-1);
        assert.deepEqual([last.state.revs[2].decision, last.state.shown, last.state.seen.body], ['pending', 2, 2]);
      }],
    ],
  },
  {
    file: 'discovery.qnt', safe: 'discoverySafe', bounds: 'posts=4, cap=3 (stands for 100), page=2, votes<=3, ticks<=8',
    fields: ['store', 'now', 'recommended', 'snapshot', 'snapshotFilter', 'snapshotLive', 'offset', 'served'],
    scenarios: [['recommend_and_top', 'rankingStep', 25]],
    check([steps]) {
      reached(steps, ['refresh_required', 'unchanged', 'votes_changed', 'opened', 'eligibility_changed', 'page',
        'end', 'edited', 'ticked', 'expired'], 'discovery');
      // The two lists are different policies: same set, different head.
      const first = at(steps, 'opened');
      assert.deepEqual([first.state.snapshot, first.state.recommended], [[1, 4, 3], [4, 3, 1, 2]]);
      // The filter is applied before the cap: post 3 is in, untagged post 2 is not.
      const filtered = at(steps, 'opened', 1);
      assert.deepEqual([filtered.state.snapshotFilter, filtered.state.snapshot, filtered.state.served],
        [true, [1, 4, 3], [1, 4]]);
      // Revoked between pages: dropped, not replaced. Votes changed between pages: no repeat.
      const pages = steps.filter(step => step.outcome === 'page');
      assert.deepEqual(pages.slice(0, 2).map(step => step.state.served), [[], [4]]);
      assert.equal(pages[0].state.store[3].eligible, false);
      const edited = at(steps, 'edited');
      assert.deepEqual([edited.state.store[4].votes, edited.state.store[4].publishedAt], [0, 2]);
    },
    mutants: [
      ['discovery-stale-access-counterexample.json', 'mutatedStaleAccessStep', 3, 'servedIsReadableNow', steps => {
        // Post 2 was hidden after the snapshot; the next page still served it.
        const last = steps.at(-1);
        assert.deepEqual([last.state.store[2].eligible, last.state.served], [false, [2]]);
      }],
      ['discovery-filter-after-cap-counterexample.json', 'mutatedFilterLastStep', 2, 'snapshotIsTopOfFilteredSet', steps => {
        // Tagged post 1 is eligible but missing: the cap was taken before the filter.
        const last = steps.at(-1);
        assert.deepEqual([last.state.snapshot, last.state.store[1].eligible, last.state.store[1].tagged],
          [[4, 3], true, true]);
      }],
    ],
  },
  {
    file: 'appeal.qnt', safe: 'appealSafe', bounds: 'cases<=4, decision versions<=6, keys=1..4',
    fields: ['decision', 'cases', 'receipts', 'answer'],
    scenarios: [
      ['appeal_escalation_overturn', 'overturnStep', 21],
      ['appeal_upheld', 'upholdStep', 5],
    ],
    check(all) {
      const [overturn, uphold] = all;
      reached(all.flat(), ['dialog_shown', 'not_found', 'recorded_not_delivered', 'replayed', 'key_reused',
        'already_open', 'not_admin', 'reason_required', 'unknown_case', 'not_needed', 'decided',
        'stale_decision', 'awaiting_admin', 'escalated', 'case_superseded', 'overturned', 'already_resolved',
        'overridden', 'nothing_to_appeal', 'stale', 'upheld', 'final_decision'], 'appeal');
      // Dialog ≠ case ≠ sent ≠ seen ≠ resolved.
      assert.deepEqual(at(overturn, 'dialog_shown').state.cases, {});
      const recorded = at(overturn, 'recorded_not_delivered');
      assert.deepEqual(recorded.state.cases[1], { appellant: 2, decision: 1, state: 'open', notice: 'not_sent', seen: false });
      for (const outcome of ['replayed', 'already_open']) {
        const again = at(overturn, outcome);
        assert.deepEqual([Object.keys(again.state.cases).length, again.state.answer], [1, 1]);
      }
      const flagged = at(overturn, 'escalated');
      assert.deepEqual([flagged.state.cases[2].notice, flagged.state.cases[2].seen], ['not_sent', false]);
      const overturned = at(overturn, 'overturned');
      assert.deepEqual(overturned.state.decision, { version: 3, verdict: 'approved', by: 'admin', reason: 'context' });
      assert.deepEqual(at(overturn, 'overridden').state.decision, overturned.state.decision);
      assert.equal(at(uphold, 'upheld').state.decision.verdict, 'rejected');
    },
    mutants: [
      ['appeal-duplicate-counterexample.json', 'mutatedDuplicateStep', 2, 'oneOpenCasePerDecision', steps => {
        // The retried request opened a second case for the same decision.
        const cases = Object.values(steps.at(-1).state.cases);
        assert.deepEqual(cases.map(item => [item.appellant, item.decision, item.state]),
          [[2, 1, 'open'], [2, 1, 'open']]);
      }],
      ['appeal-fake-delivery-counterexample.json', 'mutatedDeliveryStep', 2, 'nothingIsDelivered', steps => {
        // A mock claimed a notification was sent although no channel is connected.
        assert.equal(steps.at(-1).state.cases[1].notice, 'sent');
      }],
    ],
  },
];

// The scripted evaluator of the contract, read back from the model's own traces: which result
// was delivered, in which order, and what the application made of it.
function evaluatorVectors(results) {
  const classify = step => {
    if (step.outcome === 'wrong_revision') return 'WrongRevision';
    if (step.outcome === 'duplicate') return 'DuplicateResult';
    if (['overridden', 'target_gone', 'superseded', 'stale_policy'].includes(step.outcome)) return 'DelayedResult';
    return { approve: 'Approve', reject: 'Reject', uncertain: 'NeedsAdmin', timeout: 'Timeout',
      provider_error: 'ProviderError', invalid_output: 'InvalidOutput' }[step.command.value];
  };
  const vectors = [];
  for (const [model, scenarios] of results) {
    for (const scenario of scenarios) {
      scenario.steps.forEach((step, index) => {
        if (step.command.op !== 'ai_result' || step.outcome === 'unknown_job') return;
        const publicNow = 'pointer' in step.state ? step.state.pointer : step.state.shown;
        vectors.push({ model, scenario: scenario.name, step: index + 1, script: classify(step),
          result: { job: step.command.job, revision: step.command.rev, outcome: step.command.value },
          applied: step.outcome, public_revision: publicNow });
      });
    }
  }
  const classes = new Set(vectors.map(vector => vector.script));
  for (const expected of ['Approve', 'Reject', 'NeedsAdmin', 'Timeout', 'ProviderError', 'InvalidOutput',
    'WrongRevision', 'DuplicateResult', 'DelayedResult']) {
    assert(classes.has(expected), `no evaluator vector scripts ${expected}`);
  }
  // No scripted failure, invalid output or late result ever made something public.
  for (const vector of vectors) {
    if (vector.script !== 'Approve') assert.notEqual(vector.applied, 'published', JSON.stringify(vector));
  }
  return vectors;
}

try {
  assert.equal(run(['--version']).trim(), version, 'Use the Quint version pinned in ../discussion');
  const results = [];
  for (const model of models) {
    run(['typecheck', model.file]);
    simulation(model.file, 'step', maxSteps, samples, model.safe);
    console.log(`PASS ${model.file}: typecheck; simulation ${model.safe} seed=${seed}, samples=${samples}, max_steps=${maxSteps}, ${model.bounds}`);
    const name = basename(model.file, '.qnt');
    const modelStatus = ['publication', 'tombstone'].includes(name) ? 'PARTIAL_PURE_CORE_CONFORMANCE' : status;
    const scenarios = model.scenarios.map(([scenario, step, length]) => {
      const out = join(temporary, `${name}-${scenario}.itf.json`);
      simulation(model.file, step, length, 1, model.safe, out);
      return { name: scenario, steps: project(out, length, 'ok', model.fields) };
    });
    model.check(scenarios.map(scenario => scenario.steps));
    fixture(`${name}-vectors.json`, { format: 'vink-target-spec-vectors-v1', status: modelStatus, quint_version: version,
      model: model.file }, { scenarios });
    for (const [file, step, length, invariant, check] of model.mutants) {
      const out = join(temporary, file.replace('.json', '.itf.json'));
      const args = simulation(model.file, step, length, 1, invariant, out, 1);
      const steps = project(out, length, 'violation', model.fields);
      check(steps);
      fixture(file, { format: 'vink-target-spec-counterexample-v1', status: modelStatus, quint_version: version,
        model: model.file, invariant, command: ['quint', ...args.filter(arg => !arg.startsWith('--out-itf='))] },
        { violation: [{ name: step, steps }] });
      console.log(`PASS negative control: ${step} violates ${invariant} at transition ${length}`);
    }
    if (['publication', 'comment_edit'].includes(name)) results.push([basename(model.file), scenarios]);
  }
  const vectors = evaluatorVectors(results);
  fixture('evaluator-vectors.json', { format: 'vink-moderation-evaluator-script-v1', status,
    quint_version: version, note: 'Scripted evaluator results from the model traces; no provider, network or clock.' },
    { scripts: [{ name: 'publication_and_comment_edit', steps: vectors }] });
  console.log(`PASS evaluator script: ${vectors.length} scripted results cover all nine mock outcome classes`);
  console.log('NOT_RUN Apalache/model checking');
  console.log('PARTIAL publication + moderation pure-core and scripted-evaluator replay runs in Rust (#150, #151); delete/remove lifecycle and all other target models remain NOT_IMPLEMENTED');
  rmSync(temporary, { recursive: true });
} catch (error) {
  console.error(`FAIL ${error.message}\nIntermediate ITF traces, if any: ${temporary}`);
  process.exitCode = 1;
}
