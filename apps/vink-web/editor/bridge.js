// Bridge between the Leptos hydration bundle and the CodeMirror runtime (src/editor.ts).
//
// wasm-bindgen ships this file with vink.js and imports it eagerly. CodeMirror itself is a
// separate module, fetched only when an editing surface mounts, so public reading pages never load it.
// `cargo xtask build` writes the runtime to target/site/pkg/vink-editor.js.

const RUNTIME_URL = new URL('../../../vink-editor.js', import.meta.url).href;

let runtime;
let loading;

export function loadEditorModule() {
  loading ??= import(RUNTIME_URL)
    .then((module) => {
      runtime = module;
    })
    .catch((error) => {
      loading = undefined;
      throw error;
    });
  return loading;
}

function ready() {
  if (!runtime) throw new Error('The Vink editor runtime has not loaded');
  return runtime;
}

export const createEditor = (host, options) => ready().createEditor(host, options);
export const destroyEditor = (handle) => ready().destroyEditor(handle);
export const getText = (handle) => ready().getText(handle);
export const focusEditor = (handle) => ready().focusEditor(handle);
export const isComposing = (handle) => ready().isComposing(handle);
export const applyConvertedDocument = (handle, format, text) =>
  ready().applyConvertedDocument(handle, format, text);
export const applyHeading = (handle, level) => ready().applyHeading(handle, level);
export const applyInlineFormat = (handle, kind) => ready().applyInlineFormat(handle, kind);
export const jumpToHeading = (handle, from, generation) =>
  ready().jumpToHeading(handle, from, generation);
