import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import test from 'node:test';
import {runInNewContext} from 'node:vm';

const bootstrap = readFileSync(new URL('../appearance.js', import.meta.url), 'utf8');

test('appearance bootstrap follows the OS without touching storage', () => {
  const root = {dataset: {}};
  const links = new Map([
    ['vink-favicon', '/pkg/brand/vpen-r1-favicon.svg'],
    ['vink-manifest', '/pkg/brand/vpen-r1-manifest-light.webmanifest'],
  ]);
  let osChanged, themeChanged;
  const media = {matches: false, addEventListener(_, listener) { osChanged = listener; }};
  const sandbox = {
    document: {
      documentElement: root,
      getElementById(id) {
        return {
          getAttribute() { return links.get(id); },
          setAttribute(_, value) { links.set(id, value); },
        };
      },
    },
    get localStorage() { assert.fail('head bootstrap must never access storage'); },
    matchMedia() { return media; },
    MutationObserver: class {
      constructor(listener) { themeChanged = listener; }
      observe(target, {attributeFilter}) {
        assert.equal(target, root);
        assert.equal(attributeFilter.join(','), 'data-theme');
      }
    },
  };
  runInNewContext(bootstrap, sandbox);
  const expectLinks = theme => {
    assert.equal(links.get('vink-favicon'), `/pkg/brand/vpen-r1-mark-${theme}.svg`);
    assert.equal(links.get('vink-manifest'), `/pkg/brand/vpen-r1-manifest-${theme}.webmanifest`);
  };
  expectLinks('light');
  media.matches = true;
  osChanged();
  expectLinks('dark');
  assert.deepEqual(root.dataset, {});

  root.dataset.theme = 'light';
  themeChanged();
  expectLinks('light');
  delete root.dataset.theme;
  themeChanged();
  expectLinks('dark');
});
