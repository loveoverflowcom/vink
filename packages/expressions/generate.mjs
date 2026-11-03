// Offline-only ingestion validation and deterministic Rust/CMP projections of the manifest.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join, resolve } from 'node:path';

const directory = dirname(fileURLToPath(import.meta.url));
export const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');
const cp = text => [...text].map(c => c.codePointAt(0).toString(16)).join('-');

export function validate(manifest, readAsset) {
  assert.equal(manifest.schema_version, 1);
  assert.match(manifest.catalog_version, /^noto-pilot-\d+$/);
  assert.match(manifest.static_source.revision, /^[a-f0-9]{40}$/);
  assert.equal(manifest.animation_source.revision, 'google-cdn-snapshot-2026-09-16');
  assert(manifest.assets.length >= 12 && manifest.assets.length <= 20);
  const ids = new Set(), sequences = new Set(), files = new Set();
  for (const asset of manifest.assets) {
    assert.match(asset.id, /^noto-[a-z0-9-]+$/);
    assert(!ids.has(asset.id), 'duplicate ID'); ids.add(asset.id);
    assert(asset.label_vi.length > 0 && asset.aliases_vi.length > 0);
    assert(['cam-xuc', 'cu-chi', 'bieu-tuong'].includes(asset.category));
    assert(asset.lookup_sequences.includes(asset.unicode));
    assert(asset.width > 0 && asset.height > 0 && asset.width <= 2048 && asset.height <= 2048);
    for (const sequence of asset.lookup_sequences) {
      assert(sequence.length > 0 && !sequences.has(sequence), 'duplicate/empty Unicode sequence');
      assert(!/[\ud800-\udfff]/u.test(sequence), 'unpaired surrogate');
      sequences.add(sequence);
    }
    for (const kind of ['poster', 'animation']) {
      const resource = asset[kind];
      assert(resource, `${asset.id}: ${kind} required in this bounded pilot`);
      assert.match(resource.file, new RegExp(`^${asset.id}-[a-f0-9]{16}\\.${kind === 'poster' ? 'svg' : 'json'}$`));
      assert(!files.has(resource.file)); files.add(resource.file);
      assert.equal(resource.license, kind === 'poster' ? 'Apache-2.0' : 'CC-BY-4.0');
      assert(resource.attribution.includes('Google') && resource.changes.length > 0);
      assert.equal(resource.source_revision, kind === 'poster' ? manifest.static_source.revision : manifest.animation_source.revision);
      const url = new URL(resource.source_url);
      assert.equal(url.protocol, 'https:');
      if (kind === 'poster') assert(resource.source_url.startsWith(`https://raw.githubusercontent.com/googlefonts/noto-emoji/${manifest.static_source.revision}/2D/svg/`));
      else assert.equal(url.hostname, 'fonts.gstatic.com');
      const bytes = readAsset(resource.file);
      assert.equal(bytes.length, resource.bytes, 'size differs');
      assert.equal(sha256(bytes), resource.sha256, 'SHA-256 differs');
      assert(resource.file.includes(resource.sha256.slice(0, 16)), 'unversioned resource');
      assert(bytes.length <= (kind === 'poster' ? 64_000 : 512_000), 'ingest size limit');
      const text = bytes.toString('utf8');
      if (kind === 'poster') {
        assert(/<svg[\s>]/i.test(text));
        assert(!/<(?:script|foreignObject|image|iframe|animate|set)\b|<!DOCTYPE|<!ENTITY|\son[a-z]+\s*=|(?:href|src)\s*=\s*['"](?!#)|@import|(?:https?:|data:|javascript:|\/\/)/i.test(text.replace(/xmlns(?:\:[a-z]+)?="[^"]*"/gi, '')), 'active/external SVG');
        assert(!/url\(\s*['"]?(?!#)/i.test(text), 'external SVG URL');
      } else {
        const animation = JSON.parse(text);
        assert.equal(animation.v, resource.lottie_version);
        assert.equal(animation.w, asset.width); assert.equal(animation.h, asset.height);
        assert(animation.fr > 0 && animation.fr <= 120 && animation.op > animation.ip && animation.op - animation.ip <= 3600);
        assert(!animation.fonts && !animation.chars, 'no fonts in this vector-only subset');
        assert(Array.isArray(animation.assets) && animation.assets.every(a => a.layers && !a.p && !a.u), 'embedded/external images forbidden');
        const visit = value => {
          if (!value || typeof value !== 'object') return;
          for (const [key, child] of Object.entries(value)) {
            assert(!(key === 'x' && typeof child === 'string' && child.length), 'expressions forbidden');
            visit(child);
          }
        };
        visit(animation);
        assert.equal(animation.markers.find(m => m.cm === 'rest')?.tm ?? null, resource.rest_frame);
      }
    }
  }
  assert.equal(sha256(manifest.assets.map(a => `${a.animation.file}:${a.animation.sha256}`).join('\n')), manifest.animation_source.snapshot_sha256);
  return manifest;
}

// Explicit exact-sequence fixtures; nothing strips ZWJ, selectors, modifiers or flag/keycap tails.
function fixtures(manifest) {
  const sources = [...manifest.assets.flatMap(a => a.lookup_sequences), '👍🏻', '👩‍👩‍👧‍👦', '🇻🇳', '1️⃣', '1⃣', '☃️', '😀😀', '😀\u200d', 'á — thơ\n  vẫn còn', ''];
  return { format: 'vink-expression-unicode-v1', cases: sources.map(source => ({ source, codepoints: cp(source), asset_id: manifest.assets.find(a => a.lookup_sequences.includes(source))?.id ?? null })) };
}

export function generate(check = true) {
  const manifest = validate(JSON.parse(readFileSync(join(directory, 'shared/catalog.json'), 'utf8')), name => readFileSync(join(directory, 'assets', name)));
  const q = JSON.stringify;
  const assets = manifest.assets;
  const vectors = fixtures(manifest);
  const rs = `// Generated by packages/expressions/generate.mjs. Do not edit.\n\n#[derive(Clone, Copy, Debug)]\npub struct Asset {\n    pub id: &'static str,\n    pub unicode: &'static str,\n    pub sequences: &'static [&'static str],\n    pub label_vi: &'static str,\n    pub category: &'static str,\n    pub poster: &'static str,\n    pub animation: Option<&'static str>,\n}\n\npub const VERSION: &str = ${q(manifest.catalog_version)};\npub const COUNT: usize = ${assets.length};\n#[rustfmt::skip]\npub const ASSETS: [Asset; COUNT] = [\n${assets.map(a => `    Asset { id: ${q(a.id)}, unicode: ${q(a.unicode)}, sequences: &[${a.lookup_sequences.map(q).join(', ')}], label_vi: ${q(a.label_vi)}, category: ${q(a.category)}, poster: ${q(a.poster.file)}, animation: Some(${q(a.animation.file)}) },`).join('\n')}\n];\n\n/// Exact whole-sequence lookup. This immutable borrow cannot rewrite source text.\npub fn lookup(source: &str) -> Option<&'static Asset> {\n    ASSETS.iter().find(|asset| asset.sequences.contains(&source))\n}\n\npub fn by_id(id: &str) -> Option<&'static Asset> {\n    ASSETS.iter().find(|asset| asset.id == id)\n}\n\n#[derive(Debug, PartialEq, Eq)]\npub enum Fallback { Poster(&'static str), Unicode }\n\npub fn fallback(asset: Option<&Asset>, poster_failed: bool) -> Fallback {\n    match asset {\n        Some(asset) if !poster_failed => Fallback::Poster(asset.poster),\n        _ => Fallback::Unicode,\n    }\n}\n`;
  const kt = `// Generated by packages/expressions/generate.mjs. Do not edit.\npackage vink.ui\n\n// Contract only: the native fallback is original Unicode. No dotLottie Web runtime/FFI.\ndata class NotoExpression(val id: String, val unicode: String, val label: String, val posterFile: String, val animated: Boolean, val sequences: List<String>)\n\nobject NotoExpressions {\n    const val version = ${q(manifest.catalog_version)}\n    val assets = listOf(\n${assets.map(a => `        NotoExpression(${q(a.id)}, ${q(a.unicode)}, ${q(a.label_vi)}, ${q(a.poster.file)}, true, listOf(${a.lookup_sequences.map(q).join(', ')})),`).join('\n')}\n    )\n    fun lookup(source: String): NotoExpression? = assets.firstOrNull { source in it.sequences }\n}\n`;
  const ktt = `// Generated from the shared Unicode conformance fixture. Do not edit.\npackage vink.ui\n\nimport kotlin.test.Test\nimport kotlin.test.assertEquals\n\nclass NotoExpressionTest {\n    @Test fun exactSequenceLookupAndUnicodeFallback() {\n${vectors.cases.map(v => `        assertEquals(${v.asset_id === null ? 'null' : q(v.asset_id)}, NotoExpressions.lookup(${q(v.source)})?.id)`).join('\n')}\n        assertEquals(${assets.length}, NotoExpressions.assets.size)\n    }\n}\n`;
  const outputs = new Map([
    ['src/catalog.rs', rs], ['shared/unicode-fixtures.json', JSON.stringify(vectors, null, 2) + '\n'],
    ['../ui-system/cmp/ui/src/commonMain/kotlin/vink/ui/NotoExpressions.kt', kt],
    ['../ui-system/cmp/ui/src/commonTest/kotlin/vink/ui/NotoExpressionTest.kt', ktt],
  ]);
  for (const [relative, content] of outputs) {
    const file = join(directory, relative);
    if (check) assert.equal(readFileSync(file, 'utf8'), content, `${relative}: generated drift; run node packages/expressions/generate.mjs --write`);
    else writeFileSync(file, content);
  }
  console.log(`PASS Noto manifest: ${assets.length} assets, source hashes, safety, Unicode, Rust/CMP drift`);
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  assert(process.argv.slice(2).every(a => a === '--write' || a === '--check'));
  generate(!process.argv.includes('--write'));
}
