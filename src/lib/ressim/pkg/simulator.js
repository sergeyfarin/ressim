/* @ts-self-types="./simulator.d.ts" */

import * as wasm from "./simulator_bg.wasm";
import { __wbg_set_wasm } from "./simulator_bg.js";
__wbg_set_wasm(wasm);
wasm.__wbindgen_start();
export {
    ReservoirSimulator, set_panic_hook
} from "./simulator_bg.js";
