// Shared Quint runner for the remaining target specifications and the canonical M1, M3 and M4
// in ../publication. It exports real ITF states without recomputing transitions. The canonical
// models have scoped Rust replay; see ../publication/README.md for their exact coverage.
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
  const canonical = ['publication-', 'tombstone-', 'governance-'].some(prefix => name.startsWith(prefix));
  const path = join(directory, canonical ? '../publication' : '.', name);
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
    file: '../publication/publication.qnt', safe: 'publicationSafe', bounds: 'revisions<=4, jobs<=8, policies<=2',
    fields: ['revs', 'jobs', 'audience', 'desired', 'intended', 'pointer', 'lifecycle', 'generation', 'policy'],
    scenarios: [
      ['audience_moderation_override', 'lifecycleStep', 44],
      ['policy_rereview', 'rereviewStep', 9],
      ['moderation_removal', 'removalStep', 11],
    ],
    check(all) {
      const [life, rereview, removal] = all;
      reached([...life, ...rereview, ...removal], ['saved', 'not_submitted', 'not_found', 'submitted', 'waiting',
        'retryable', 'duplicate', 'requeued', 'not_needed', 'wrong_revision', 'unknown_job', 'published',
        'rejected', 'revision_rejected', 'stale', 'policy_changed', 'stale_policy', 'needs_admin',
        'not_admin', 'overridden', 'made_private', 'reaffirmed', 'unpublished', 'approved_unpublished',
        'deleted', 'work_unavailable', 'target_gone', 'removed', 'withdrawn', 'case_closed', 'resubmitted',
        'rereview'], 'publication');
      // Non-vacuity: r1 stays public while r2 is rejected; an approval of a revision the author no
      // longer intends publishes nothing; the admin's approval survives the late rejection; nothing
      // returns after deletion; a private work takes no decision; old-policy approvals are reviewed.
      const rejected = at(life, 'rejected');
      assert.deepEqual([rejected.command.rev, rejected.state.pointer], [2, 1]);
      const withheld = at(life, 'approved_unpublished');
      assert.deepEqual([withheld.command.rev, withheld.state.desired, withheld.state.revs[1].decision],
        [1, 4, 'approved']);
      const overridden = at(life, 'overridden');
      assert.deepEqual([overridden.command.value, overridden.state.revs[3], overridden.state.pointer],
        ['reject', { decision: 'approved', by: 'admin', policy: 2 }, 3]);
      const late = at(life, 'withdrawn', 1);
      assert.deepEqual([late.state.audience, late.state.revs[4].decision, late.state.jobs[7].state],
        ['private', 'pending', 'withdrawn']);
      assert.equal(at(life, 'case_closed').state.revs[4].decision, 'pending');
      const again = at(life, 'resubmitted');
      assert.deepEqual([again.state.jobs[8], again.state.pointer], [{ policy: 2, rev: 4, state: 'open' }, 0]);
      const aiRereview = at(life, 'rereview');
      assert.deepEqual([aiRereview.state.revs[1], aiRereview.state.pointer, aiRereview.state.jobs[6].rev],
        [{ decision: 'pending', by: 'none', policy: 2 }, 0, 1]);
      const humanRereview = at(rereview, 'rereview');
      assert.deepEqual([humanRereview.state.revs[1], humanRereview.state.pointer, Object.keys(humanRereview.state.jobs).length],
        [{ decision: 'needs_admin', by: 'rereview', policy: 2 }, 0, 1]);
      const deleted = life.findIndex(step => step.outcome === 'deleted');
      assert(life.slice(deleted).length >= 3 && life.slice(deleted).every(step => step.state.pointer === 0));
      assert(life.some(step => step.command.op === 'snapshot' && Object.keys(step.state.jobs).length === 0),
        'a private save must exist with no evaluation');
    },
    mutants: [
      ['publication-late-result-counterexample.json', 'mutatedOverrideStep', 4, 'adminOverrideWins', steps => {
        // The admin approved r1; the evaluator's late rejection replaced that decision.
        const [before, last] = steps.slice(-2);
        assert.deepEqual(before.state.revs[1], { decision: 'approved', by: 'admin', policy: 1 });
        assert.deepEqual([last.command.op, last.state.revs[1]], ['ai_result', { decision: 'rejected', by: 'ai', policy: 1 }]);
      }],
      ['publication-moved-intent-counterexample.json', 'mutatedIntentStep', 5, 'pointerMovesLawfully', steps => {
        // The author's public intent names r2; the approval of r1 published r1 anyway.
        const last = steps.at(-1);
        assert.deepEqual([last.outcome, last.state.desired, last.state.pointer], ['published', 2, 1]);
      }],
    ],
  },
  {
    file: '../publication/tombstone.qnt', safe: 'tombstoneSafe', bounds: 'comments<=4, viewers=guest+2',
    fields: ['status', 'everPublic', 'comments', 'views', 'notified'],
    scenarios: [
      ['author_deleted_public_work', 'deletedStep', 14],
      ['hidden_then_public_then_deleted', 'hiddenStep', 16],
      ['never_public', 'neverPublicStep', 4],
      ['moderation_removed', 'removedStep', 7],
      ['review_withdrawn_then_resubmitted', 'withdrawnStep', 12],
    ],
    check(all) {
      const [deleted, hidden, never, removed, withdrawn] = all;
      reached(all.flat(), ['published', 'created', 'reviewed', 'deleted_with_tombstone', 'tombstone_read_only',
        'hidden_read_only', 'comment_unavailable', 'comment_removed', 'work_unavailable', 'made_private',
        'unchanged', 'deleted', 'removed_by_moderation', 'duplicate'], 'tombstone');
      // A: the old URL is a tombstone without body, preview or listing; the approved comment
      // stays, a rejected or pending one is only its writer's.
      assert.deepEqual(at(withdrawn, 'review_withdrawn').state.views[0], { page: 'review_withdrawn', listed: false, snippet: false, comments: [1] });
      assert.equal(at(withdrawn, 'waiting').state.views[0].page, 'review_withdrawn');
      assert.equal(withdrawn.at(-1).state.views[0].page, 'not_found');
      const tomb = at(deleted, 'deleted_with_tombstone').state.views;
      assert.deepEqual(tomb[0], { page: 'tombstone', listed: false, snippet: false, comments: [1] });
      assert.deepEqual([tomb[2].comments, tomb[3].comments], [[1, 3], [1, 2]]);
      // B: the hidden page keeps the approved comment for guests and the pending one for its writer.
      const page = at(hidden, 'made_private').state.views;
      assert.deepEqual(page[0], { page: 'hidden', listed: false, snippet: false, comments: [1] });
      assert.deepEqual(page[3].comments, [1, 2, 3]);
      // A comment approved while hidden is shown but its approval does not notify the author,
      // not even after republication; an approval in the new public period does.
      const quiet = at(hidden, 'reviewed', 1);
      assert.deepEqual([quiet.state.status, quiet.state.views[0].comments, quiet.state.notified], ['hidden', [1, 2], [1]]);
      assert.deepEqual(at(hidden, 'published', 1).state.notified, [1]);
      assert.deepEqual(at(hidden, 'reviewed', 2).state.notified, [1, 4]);
      // Hidden then deleted keeps the thread on the tombstone.
      assert.deepEqual(at(hidden, 'deleted_with_tombstone').state.views[0].comments, [1, 2, 4]);
      // C and D: no page and no comment for anybody else.
      const nothing = { page: 'not_found', listed: false, snippet: false, comments: [] };
      for (const state of [at(never, 'deleted').state, at(removed, 'removed_by_moderation').state, deleted[0].state]) {
        assert(Object.keys(state.comments).length > 0 || ['unpublished', 'deleted_hidden'].includes(state.status));
        for (const viewer of [0, 2, 3]) assert.deepEqual(state.views[viewer], nothing);
      }
    },
    mutants: [
      ['tombstone-withdrawn-body-counterexample.json', 'mutatedWithdrawnStep', 4, 'bodyOnlyWhilePublic', steps => {
        assert.equal(steps.at(-1).state.views[0].page, 'body');
        assert.equal(steps.at(-1).state.status, 'review_withdrawn');
      }],
      ['tombstone-removed-as-hidden-counterexample.json', 'mutatedRemovedStep', 4, 'noExceptionForUnavailableWork', steps => {
        // An admin removed the work, yet a guest reads its comment through the old link.
        const last = steps.at(-1);
        assert.deepEqual([last.state.status, last.state.views[0].page, last.state.views[0].comments],
          ['moderation_removed', 'hidden', [1]]);
      }],
      ['tombstone-hidden-notice-counterexample.json', 'mutatedNoticeStep', 5, 'noNoticeFromHiddenPeriod', steps => {
        // Republication told the author about an approval made while the work was hidden.
        const last = steps.at(-1);
        assert.deepEqual([last.command.op, last.state.notified], ['publish', [1]]);
      }],
    ],
  },
  {
    file: '../publication/governance.qnt', safe: 'governanceSafe',
    bounds: 'principals=5, grants=5, targets=4, cases<6, jobs<6, appeals<4, investigations=5, ticks<=6',
    fields: ['granted', 'members', 'now', 'automation', 'workPublic', 'decisions', 'jobs', 'cases', 'appeals',
      'investigations', 'reads'],
    scenarios: [
      ['grants_and_claims', 'claimsStep', 39],
      ['automation_and_late_results', 'automationStep', 16],
      ['appeals', 'appealsStep', 31],
      ['investigation', 'investigationStep', 19],
    ],
    check(all) {
      const [claims, automation, appeals, investigation] = all;
      reached(all.flat(), ['granted', 'revoked', 'joined', 'left', 'ticked', 'submitted_ai', 'needs_human',
        'applied', 'claimed', 'released', 'noted', 'escalated', 'taken_over', 'basis_changed', 'authority_lost',
        'concluded', 'intervened', 'paused', 'resumed', 'made_private', 'made_public', 'appeal_recorded',
        'upheld', 'reversed', 'investigation_opened', 'read_audited', 'investigation_closed', 'not_admin',
        'not_authorized', 'recused', 'stale_case', 'already_claimed', 'not_claimant', 'escalated_to_admin',
        'overridden', 'automation_paused', 'withdrawn', 'not_needed', 'stale_decision', 'appeal_open',
        'appeal_replayed', 'final_decision', 'nothing_to_appeal', 'already_resolved', 'not_found', 'conflict',
        'condition_lapsed', 'never_public', 'expired', 'not_investigator', 'wrong_revision', 'closed'], 'governance');
      // The race: one claimant, the other is stale.
      assert.deepEqual(at(claims, 'stale_case').state.cases[1].claimant, 2);
      // Kept through another grant, then lost and queued.
      assert.deepEqual(at(claims, 'basis_changed').state.cases[1].basis, 4);
      const lost = at(claims, 'authority_lost');
      assert.deepEqual([lost.state.cases[1].state, lost.state.cases[1].claimant], ['queued', 0]);
      assert.deepEqual(at(claims, 'concluded').state.decisions[1], { version: 3, verdict: 'approved', by: 'admin' });
      // Late AI results write nothing.
      assert.deepEqual(at(automation, 'overridden').state.decisions[1], { version: 2, verdict: 'approved', by: 'moderator' });
      const paused = at(automation, 'paused').state;
      assert.deepEqual([paused.jobs[2].state, paused.jobs[3].state, Object.keys(paused.cases).length], ['rerouted', 'rerouted', 3]);
      assert.equal(at(automation, 'automation_paused', 1).state.automation, 'enabled');
      assert.equal(at(automation, 'withdrawn').state.jobs[4].state, 'withdrawn');
      // Appeals: one per decision, nothing delivered, a moderator's decision is appealable.
      assert.deepEqual(at(appeals, 'appeal_replayed').state.appeals, { 1: { target: 1, decision: 2, by: 'ai', state: 'open', notice: 'not_sent' } });
      assert.deepEqual(at(appeals, 'upheld').state.decisions[1], { version: 3, verdict: 'rejected', by: 'admin' });
      assert.equal(at(appeals, 'appeal_recorded', 1).state.appeals[2].by, 'moderator');
      assert.deepEqual(at(appeals, 'reversed').state.decisions[2], { version: 3, verdict: 'approved', by: 'admin' });
      assert.equal(at(appeals, 'made_private').state.appeals[3].state, 'closed');
      // Investigation: each read is audited, only the exact once-public revision.
      assert.deepEqual(at(investigation, 'read_audited', 1).state.reads, [[1, 1], [5, 1]]);
      assert.deepEqual(at(investigation, 'closed').state.reads, [[1, 1], [5, 1]]);
    },
    mutants: [
      ['governance-lost-authority-counterexample.json', 'mutatedAuthorityStep', 6, 'conclusionNeedsLiveAuthority', steps => {
        // Grant 1 was revoked, yet moderator 2 concluded the case it held.
        const last = steps.at(-1);
        assert.deepEqual([last.outcome, last.state.granted, last.state.decisions[1].by], ['concluded', [], 'moderator']);
      }],
      ['governance-stolen-claim-counterexample.json', 'mutatedClaimStep', 6, 'claimNeverStolen', steps => {
        // Moderator 3 took the case moderator 2 held, with a stale version.
        const last = steps.at(-1);
        assert.deepEqual([last.outcome, last.state.cases[1].claimant], ['claimed', 3]);
      }],
      ['governance-duplicate-appeal-counterexample.json', 'mutatedAppealStep', 4, 'oneAppealPerDecision', steps => {
        // The retried appeal opened a second one on the same decision version.
        assert.equal(Object.keys(steps.at(-1).state.appeals).length, 2);
      }],
      ['governance-paused-result-counterexample.json', 'mutatedPauseStep', 3, 'pausedAiWritesNothing', steps => {
        // Automation is paused, yet the evaluator's approval became the decision.
        const last = steps.at(-1);
        assert.deepEqual([last.state.automation, last.state.decisions[1].by], ['paused', 'ai']);
      }],
      ['governance-wrong-read-counterexample.json', 'mutatedReadStep', 3, 'readsAreAuthorized', steps => {
        // The file names revision 1; the read returned the never-public revision 2.
        assert.deepEqual(steps.at(-1).state.reads, [[1, 2]]);
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
];

// The scripted evaluator of the contract, read back from the model's own traces: which result
// was delivered, in which order, and what the application made of it.
function evaluatorVectors(results) {
  const classify = step => {
    if (step.outcome === 'wrong_revision') return 'WrongRevision';
    if (step.outcome === 'duplicate') return 'DuplicateResult';
    if (['overridden', 'target_gone', 'superseded', 'stale_policy', 'withdrawn'].includes(step.outcome)) return 'DelayedResult';
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
    const modelStatus = ['publication', 'tombstone', 'governance'].includes(name) ? 'PARTIAL_PURE_CORE_CONFORMANCE' : status;
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
  console.log('PARTIAL publication (M1), tombstone/hidden access (M3) and moderation governance (M4) replay through the Rust pure core; all other target models remain NOT_IMPLEMENTED');
  rmSync(temporary, { recursive: true });
} catch (error) {
  console.error(`FAIL ${error.message}\nIntermediate ITF traces, if any: ${temporary}`);
  process.exitCode = 1;
}
