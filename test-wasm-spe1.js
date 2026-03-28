import { readFile } from 'node:fs/promises';
import init, { ReservoirSimulator } from './src/lib/ressim/pkg/simulator.js';

const wasmBytes = await readFile(new URL('./src/lib/ressim/pkg/simulator_bg.wasm', import.meta.url));
await init({ module_or_path: wasmBytes });

import { buildSimulatorFromConfig } from './src/lib/benchmarkRunModel.js';
import { spe1_gas_injection } from './src/lib/catalog/scenarios/spe1_gas_injection.js';
// Actually it is complicated. Better off looking at test-native and rewriting SPE1 in rust.
