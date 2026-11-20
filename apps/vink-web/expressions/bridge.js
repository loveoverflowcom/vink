// Tiny wasm-bindgen glue; the pilot controller is imported only by its mounted route.
let modulePromise;
export function loadPilotModule() {
  modulePromise ??= import(new URL('../../../expressions/pilot.js', import.meta.url).href).catch(error => { modulePromise = undefined; throw error; });
  return modulePromise;
}
export function createPilot(module, host, decide) { return module.mountPilot(host, decide); }
export function destroyPilot(handle) { handle.destroy(); }
