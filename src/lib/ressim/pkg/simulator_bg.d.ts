/* Ambient declarations for the wasm-bindgen background JS module.
 * wasm-pack does not generate this file — it is hand-maintained.
 * Import simulator_bg.js for Node.js WASM bootstrap (bypasses the static .wasm import in simulator.js).
 * After calling __wbg_set_wasm, ReservoirSimulator etc. are fully usable.
 */
export * from './simulator.js';

/** Set the live WASM exports after manual instantiation in Node.js. */
export function __wbg_set_wasm(val: WebAssembly.Exports): void;
