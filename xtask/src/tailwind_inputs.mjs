// Run via node --eval in the pinned Tailwind package, so resolution matches its CLI.
// Ask the compiler which files it opened without generating a stylesheet.
import {readFileSync} from 'node:fs';
import {basename, dirname, resolve} from 'node:path';
import {compile} from '@tailwindcss/node';
import {Scanner} from '@tailwindcss/oxide';

const [entry, ...allowed] = process.argv.slice(1);
const css = new Set([entry]);
const compiler = await compile(readFileSync(entry, 'utf8'), {
  from: entry,
  base: dirname(entry),
  onDependency: file => css.add(file),
  customJsResolver() { throw new Error('Unbounded Tailwind plugin or config'); },
});
const roots = compiler.sources.map(s => s.negated ? null : resolve(s.base, s.pattern)).sort();
if (compiler.root !== 'none' || JSON.stringify(roots) !== JSON.stringify(allowed.sort())) {
  throw new Error('Tailwind source roots outside the product allowlist');
}
const scanner = new Scanner({sources: [
  ...compiler.sources,
  {base: dirname(process.execPath), pattern: basename(process.execPath), negated: true},
  {base: dirname(entry), pattern: basename(entry), negated: false},
]});
console.log(JSON.stringify({css: [...css], scanned: scanner.files.sort()}));
