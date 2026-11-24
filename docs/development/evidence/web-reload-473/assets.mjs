#!/usr/bin/env node
// #473: raw/gzip/Brotli bytes (the #234 harness parameters) and WebAssembly section sizes for
// release files downloaded from the live namespace. Usage: node assets.mjs <file>...
import { readFileSync } from 'node:fs';
import { basename } from 'node:path';
import { COMPRESSION_PARAMETERS, measureCompressedBytes } from '../../../../apps/vink-web/e2e/web-perf-baseline.mjs';

function wasmSections(buffer) {
  const names = { 0: 'custom', 1: 'type', 2: 'import', 3: 'function', 4: 'table', 5: 'memory', 6: 'global',
    7: 'export', 8: 'start', 9: 'element', 10: 'code', 11: 'data', 12: 'datacount' };
  let offset = 8; const sections = {};
  const leb = () => { let value = 0, shift = 0, byte; do { byte = buffer[offset++]; value |= (byte & 0x7f) << shift; shift += 7; } while (byte & 0x80); return value >>> 0; };
  while (offset < buffer.length) {
    const id = buffer[offset++]; const size = leb(); const start = offset;
    let name = names[id] ?? 'id' + id;
    if (id === 0) { const length = leb(); name = 'custom:' + buffer.subarray(offset, offset + length).toString(); }
    sections[name] = (sections[name] || 0) + size; offset = start + size;
  }
  return sections;
}
const files = {};
for (const path of process.argv.slice(2)) {
  const buffer = readFileSync(path);
  files[basename(path)] = { ...measureCompressedBytes(buffer), ...(path.endsWith('.wasm') ? { sections: wasmSections(buffer) } : {}) };
}
console.log(JSON.stringify({ compression: COMPRESSION_PARAMETERS, files }, null, 1));
