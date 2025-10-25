/* tslint:disable */
/* eslint-disable */
export function set_panic_hook(): void;
export class ReservoirSimulator {
  free(): void;
  [Symbol.dispose](): void;
  constructor(nx: number, ny: number, nz: number);
  add_well(i: number, j: number, k: number, bhp: number, pi: number, injector: boolean): void;
  step(delta_t_days: number): void;
  get_time(): number;
  getGridState(): any;
  getWellState(): any;
  getDimensions(): any;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
  readonly memory: WebAssembly.Memory;
  readonly set_panic_hook: () => void;
  readonly __wbg_reservoirsimulator_free: (a: number, b: number) => void;
  readonly reservoirsimulator_new: (a: number, b: number, c: number) => number;
  readonly reservoirsimulator_add_well: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => void;
  readonly reservoirsimulator_step: (a: number, b: number) => void;
  readonly reservoirsimulator_get_time: (a: number) => number;
  readonly reservoirsimulator_getGridState: (a: number) => any;
  readonly reservoirsimulator_getWellState: (a: number) => any;
  readonly reservoirsimulator_getDimensions: (a: number) => any;
  readonly __wbindgen_malloc: (a: number, b: number) => number;
  readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
  readonly __wbindgen_export_2: WebAssembly.Table;
  readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;
/**
* Instantiates the given `module`, which can either be bytes or
* a precompiled `WebAssembly.Module`.
*
* @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
*
* @returns {InitOutput}
*/
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
* If `module_or_path` is {RequestInfo} or {URL}, makes a request and
* for everything else, calls `WebAssembly.instantiate` directly.
*
* @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
*
* @returns {Promise<InitOutput>}
*/
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
