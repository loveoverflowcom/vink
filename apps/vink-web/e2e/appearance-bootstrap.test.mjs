import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import test from 'node:test';
import {runInNewContext} from 'node:vm';

const source = readFileSync(new URL('../src/appearance.rs', import.meta.url), 'utf8');
function inlineScript(name) {
  const result = source.match(new RegExp('const ' + name + ': &str = r#"([\\s\\S]*?)"#;'));
  assert.ok(result, name + ' script is present in the Rust owner');
  return result[1];
}

const bootstrap = readFileSync(new URL('../appearance.js', import.meta.url), 'utf8');
const choiceBootstrap = inlineScript('CHOICE_BOOTSTRAP');

function beforeHydration(storedText, {denyRead = false, denyWrite = false} = {}) {
  const storage = new Map([['vink.ui.theme', 'dark'], ['vink.ui.rail', 'compact']]);
  if (storedText !== null) storage.set('vink.ui.text', storedText);
  const dataset = {};
  const links = new Map([
    ['vink-favicon', '/pkg/brand/vpen-r1-favicon.svg'],
    ['vink-manifest', '/pkg/brand/vpen-r1-manifest-light.webmanifest'],
  ]);
  const radios = [{value: '100', checked: true}, {value: '150', checked: false}];
  const document = {
    documentElement: {
      dataset,
      setAttribute(name, value) { dataset[name.replace('data-', '')] = value; },
    },
    getElementById(id) {
      return {
        getAttribute() { return links.get(id); },
        setAttribute(_, value) { links.set(id, value); },
      };
    },
    currentScript: {
      previousElementSibling: {
        parentElement: {dataset: {preference: 'text'}},
        querySelectorAll() { return radios; },
      },
    },
  };
  const sandbox = {
    document,
    localStorage: {
      getItem(key) {
        if (denyRead) throw new Error('storage reads denied');
        return storage.get(key) ?? null;
      },
      setItem(key, value) {
        if (denyWrite) throw new Error('storage writes denied');
        storage.set(key, value);
      },
    },
    matchMedia() { return {matches: false, addEventListener() {}}; },
    MutationObserver: class { observe() {} },
  };
  runInNewContext(bootstrap, sandbox);
  runInNewContext(choiceBootstrap, sandbox);
  return {dataset, checked: radios.filter(radio => radio.checked).map(radio => radio.value), storage, links};
}

for (const [name, input, options, size, stored] of [
  ['legacy choice', '200', {}, '150', '150'],
  ['current choice', '150', {}, '150', '150'],
  ['unset choice', null, {}, '100', undefined],
  ['invalid choice', '125', {}, '100', '125'],
  ['storage denies migration write', '200', {denyWrite: true}, '150', '200'],
  ['storage denies reads', '200', {denyRead: true}, '100', '200'],
]) {
  test('appearance bootstrap: ' + name, () => {
    const {dataset, checked, storage, links} = beforeHydration(input, options);
    assert.deepEqual(checked, [size]);
    assert.equal(dataset.text, size === '100' ? undefined : size);
    assert.equal(storage.get('vink.ui.text'), stored);
    assert.equal(dataset.theme, options.denyRead ? undefined : 'dark');
    assert.equal(dataset.rail, options.denyRead ? undefined : 'compact');
    assert.match(links.get('vink-favicon'), options.denyRead ? /light\.svg$/ : /dark\.svg$/);
  });
}
