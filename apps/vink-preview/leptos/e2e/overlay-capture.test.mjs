import test from 'node:test';
import assert from 'node:assert/strict';
import { contains, insideInner, openingInvariants } from './overlay-capture.mjs';

test('opening invariants reject the measured F-01 scroll and clipped title', () => {
  const measured = {viewport:{x:0,y:0,width:320,height:700}, panel:{x:32,y:32,width:256,height:636},
    title:{x:80,y:-578,width:172.7,height:280},documentWidth:320,scrollWidth:256,clientWidth:256};
  assert.deepEqual(openingInvariants(measured), {'INV-1':false,'INV-2':true,'INV-4':true,'INV-5':false});
  assert.equal(openingInvariants({...measured,title:{...measured.title,y:27}})['INV-1'],false);
  assert.equal(openingInvariants({...measured,title:{...measured.title,y:80}})['INV-1'],true);
});

test('missing/invalid bounds, cropped viewport and horizontal overflow cannot pass', () => {
  const box={x:0,y:0,width:320,height:700};
  for (const bad of [undefined,{...box,width:0},{...box,x:NaN},{...box,height:701}]) {
    assert.equal(contains(box,bad),false);
  }
  assert.equal(contains({...box,height:640},box),false);
  assert.equal(openingInvariants({viewport:box,panel:box,title:box,documentWidth:321,
    scrollWidth:320,clientWidth:320})['INV-4'],false);
});

test('F-02: text that crosses the panel padding fails INV-5; a column that holds it passes', () => {
  // The measured case: dialog x=32, client width 226, padding 48/48 -> inner x=80, width=130;
  // the title was 172.7 wide, so it ended 42.7 CSS px past the inner edge.
  const inner = {x:80,width:130};
  const title = {x:80,y:80,width:172.7,height:400};
  const base = {viewport:{x:0,y:0,width:320,height:700}, panel:{x:32,y:32,width:241,height:636},
    title, actions:{x:80,y:989,width:130,height:289}, inner, documentWidth:305, scrollWidth:226, clientWidth:226};
  assert.equal(openingInvariants(base)['INV-5'], false);
  assert.equal(openingInvariants({...base, title:{...title,width:130}})['INV-5'], true);
  // Sub-pixel rounding is tolerated, a pixel and a half is not.
  assert.equal(insideInner(inner,{x:80,width:130.9}), true);
  assert.equal(insideInner(inner,{x:80,width:131.6}), false);
  assert.equal(insideInner(inner,{x:78.4,width:130}), false);
  // Actions are held to the same edges as the title.
  assert.equal(openingInvariants({...base, title:{...title,width:130}, actions:{...base.actions,width:173}})['INV-5'], false);
  // Missing, empty or non-finite inner boxes never pass.
  for (const bad of [undefined,{x:80},{x:NaN,width:130},{x:80,width:0}]) {
    assert.equal(insideInner(bad,{x:80,width:10}), false);
  }
});
