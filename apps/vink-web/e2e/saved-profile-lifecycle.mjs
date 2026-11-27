import { standaloneFixtureSql as sql } from './author-fixtures.mjs';
import { createHash, randomUUID } from 'node:crypto';
import { captureRequest, holdAnswer } from './auth-browser-network.mjs';
import { q } from './auth-smoke.mjs';

const list = '/api/v1/me/saved-works';
const open = (id) => '#saved-open-' + id;
const remove = (id) => '#saved-remove-' + id;
const row = (id) => '#saved-row-' + id;
const count = (n) => `document.querySelectorAll('#saved-list > li').length === ${n}`;

function seed(principal) {
  if (!/^[0-9a-f-]{36}$/.test(principal)) throw new Error('expected a scratch principal UUID');
  const records = Array.from({length: 23}, (_, n) => ({
    id: randomUUID(), title: 'Đọc lại trong một chiều mưa ' + (n + 1), order: n,
  }));
  const statements = records.map(({ id, title, order }) => {
    const source = 'Mưa bên bậc cửa rồi lặng.';
    const hash = createHash('sha256').update('vink-document-content');
    for (const text of ['default', title, source]) {
      const bytes = Buffer.from(text);
      const length = Buffer.alloc(8);
      length.writeBigUInt64BE(BigInt(bytes.length));
      hash.update(length).update(bytes);
    }
    return `
      INSERT INTO works(id,owner_id) VALUES ('${id}', '${principal}');
      INSERT INTO revisions(id,work_id,title,document,content_hash,created_by) VALUES
        ('${id}', '${id}', '${title}', '${JSON.stringify({format:'default', source})}', '${hash.digest('hex')}', '${principal}');
      INSERT INTO work_publications(work_id,audience,lifecycle,generation,intent_revision_id,intent_generation,
        pointer_revision_id,pointer_intent_generation,ever_public,public_epoch)
      VALUES ('${id}', 'public', 'active', 2, '${id}', 2, '${id}', 2, ARRAY['${id}'::uuid], 1);
      INSERT INTO publications(work_id,revision_id,generation,published_at)
      VALUES ('${id}', '${id}', 2, '2026-01-01');
      INSERT INTO saved_works(principal_id,work_id,generation,saved_at)
      VALUES ('${principal}', '${id}', 2, '2026-02-01 00:00:00Z'::timestamptz + ${order} * interval '1 minute');`;
  }).join('\n');
  sql(`BEGIN; ${statements} COMMIT;`);
  return records;
}

function hide(id) {
  sql(`BEGIN;
    DELETE FROM publications WHERE work_id='${id}';
    UPDATE work_publications SET audience='private', generation=generation+1,
      hidden_revision_id=pointer_revision_id, hidden_intent_generation=pointer_intent_generation,
      intent_revision_id=NULL, intent_generation=NULL, pointer_revision_id=NULL, pointer_intent_generation=NULL
    WHERE work_id='${id}';
    COMMIT;`);
}

function deleteByAuthor(id) {
  sql(`BEGIN;
    DELETE FROM publications WHERE work_id='${id}';
    UPDATE work_publications SET audience='private', lifecycle='author_deleted', generation=generation+1,
      deletion_provenance='tombstone', tombstone_revision_id=pointer_revision_id,
      tombstone_intent_generation=pointer_intent_generation,
      intent_revision_id=NULL, intent_generation=NULL, pointer_revision_id=NULL, pointer_intent_generation=NULL
    WHERE work_id='${id}';
    COMMIT;`);
}

function removeByAdmin(id) {
  sql(`BEGIN;
    DELETE FROM publications WHERE work_id='${id}';
    UPDATE work_publications SET audience='private', lifecycle='removed', generation=generation+1,
      intent_revision_id=NULL, intent_generation=NULL, pointer_revision_id=NULL, pointer_intent_generation=NULL
    WHERE work_id='${id}';
    COMMIT;`);
}

function restore(id) {
  sql(`BEGIN;
    UPDATE work_publications SET audience='public', generation=generation+1,
      intent_revision_id='${id}', intent_generation=generation+1,
      pointer_revision_id='${id}', pointer_intent_generation=generation+1,
      hidden_revision_id=NULL, hidden_intent_generation=NULL WHERE work_id='${id}';
    INSERT INTO publications(work_id,revision_id,generation,published_at) VALUES ('${id}','${id}',4,'2026-01-01');
    COMMIT;`);
}

async function projection(page, id) {
  return page.eval(`fetch('${list}').then(r => r.json()).then(p => {
    const row = p.items.find(row => row.work_id === '${id}');
    return row && {kind:row.kind, keys:Object.keys(row).sort(), savedAt:row.saved_at};
  })`);
}

export async function savedProfileLifecycle(page, { args, check, paths }) {
  const { base } = args;
  const { path } = paths;
  const principal = await page.eval("fetch('/api/v1/auth/session').then(r => r.json()).then(s => s.principal_id)");
  const records = seed(principal);
  const [secondPage] = records;
  const [withdrawn, deleted, admin] = records.slice(-3);
  const latest = records.at(-4);
  try {
    removeByAdmin(admin.id);
    await page.goto(base + path);
    await page.waitFor(`${count(20)} && ${q('#saved-more')}`, '#389 first page has 20 shown rows');
    check(await page.eval(`${q('#saved-list li:first-child')}?.id === 'saved-row-${deleted.id}' && !${q(row(admin.id))}`),
      '#389 newest first and admin-removed source is omitted');
    const original = await projection(page, withdrawn.id);
    const append = await holdAnswer(page, 'GET', list);
    await page.click('#saved-more');
    check(await append.held() === 200, '#389 holds the real cursor page before testing transport failure');
    await append.finish(true, true);
    await page.waitFor(`${count(20)} && ${q('#saved-more')}?.textContent.trim() === 'Thử lại'`, '#389 append failure keeps the current page');
    await page.click('#saved-more');
    await page.waitFor(`${count(records.length - 1)} && ${q(open(secondPage.id))}`, '#389 cursor retry appends each remaining row once');

    await page.click(open(secondPage.id));
    await page.waitFor(`location.pathname === '/tac-pham/${secondPage.id}' && ${q('#reader-paper')}`, '#389 opens a second-page work');
    await page.eval('history.back()');
    await page.waitFor(`${count(records.length - 1)} && document.activeElement?.id === 'saved-open-${secondPage.id}'`,
      '#389 revalidates both pages and restores a second-page anchor');

    await page.click(open(withdrawn.id));
    await page.waitFor(`location.pathname === '/tac-pham/${withdrawn.id}' && ${q('#reader-paper')}`, '#389 prepares Back to a withdrawn work');
    hide(withdrawn.id);
    deleteByAuthor(deleted.id);
    await page.eval('history.back()');
    await page.waitFor(`${q(row(withdrawn.id))}?.textContent.includes('Nội dung đã được ẩn') &&
      ${q(row(deleted.id))}?.textContent.includes('Nội dung đã được tác giả xoá') && document.activeElement?.id === 'saved-heading'`,
      '#389 revoked Back anchor falls back to the verified heading');
    const projected = await Promise.all([projection(page, withdrawn.id), projection(page, deleted.id)]);
    check(projected[0]?.kind === 'hidden' && projected[1]?.kind === 'author_deleted' &&
      projected.every(item => item.keys.join() === 'generation,kind,work_id') &&
      await page.eval(`[${q(row(withdrawn.id))}, ${q(row(deleted.id))}].every(el => !el.querySelector('a,time,.vk-public-row__byline,.vk-public-row__title'))`),
      '#389 both neutral projections have only their permitted fields and render no stale metadata or Reader link');

    restore(withdrawn.id);
    await page.reload();
    await page.waitFor(q(open(withdrawn.id)), '#389 public restoration brings back the work');
    check((await projection(page, withdrawn.id))?.savedAt === original?.savedAt,
      '#389 restored publication keeps the original save time');
    await page.click(remove(deleted.id));
    await page.waitFor(`!${q(row(deleted.id))} && ${q('#saved-snack')}?.textContent.trim() === 'Đã bỏ lưu.'`,
      '#389 deleted row can be cleared with a neutral announcement');

    const pair = list + '/' + latest.id;
    const current = await page.eval(`fetch('${pair}').then(r => r.json()).then(pair => pair.generation)`);
    const advanced = await page.eval(`fetch('${pair}?expected_generation=${current}', {
      method:'PUT', headers:{'X-Vink-Expected-Principal':'${principal}'}
    }).then(r => r.status)`);
    check(advanced === 200, '#389 another intent advances the pair generation via the real API');
    const readback = captureRequest(page, 'GET', pair);
    const conflict = await holdAnswer(page, 'DELETE', pair);
    await page.click(remove(latest.id));
    check(await conflict.held() === 409, '#389 obsolete generation gets a real 409 and leaves the row in place');
    await conflict.finish(false, true);
    await page.waitFor(`${q(row(latest.id))}?.textContent.includes('Chưa bỏ lưu được.')`, '#389 checks the same pair after conflict');
    check(readback.count() === 1, '#389 409 triggers one authoritative read of the same pair');
    readback.close();
    await page.click(row(latest.id) + ' .vk-saved-row__error button');
    await page.waitFor(`!${q(row(latest.id))}`, '#389 an explicit retry uses the newly confirmed generation');
  } finally {
    // Memberships are private test data. The standalone runner drops the scratch DB and fixtures.
    sql(`DELETE FROM saved_works WHERE principal_id='${principal}' AND work_id IN (${records.map(({id}) => "'" + id + "'").join(',')});`);
  }
}
