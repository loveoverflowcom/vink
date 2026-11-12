import test from 'node:test';
import assert from 'node:assert/strict';
import { contains, openingInvariants } from './overlay-capture.mjs';

test('opening invariants reject the measured F-01 scroll and clipped title', () => {
  const measured = {viewport:{x:0,y:0,width:320,height:700}, panel:{x:32,y:32,width:256,height:636},
    title:{x:80,y:-578,width:172.7,height:280},documentWidth:320,scrollWidth:256,clientWidth:256};
  assert.deepEqual(openingInvariants(measured), {'INV-1':false,'INV-2':true,'INV-4':true});
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
