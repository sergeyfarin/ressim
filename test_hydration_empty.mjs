import { readFileSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const rootDir = resolve(__dirname, '.');

const caseData = JSON.parse(readFileSync(resolve(rootDir, 'public/cases/depletion_center_producer.json')));
const wasmPath = resolve(rootDir, 'src/lib/ressim/pkg/simulator_bg.wasm');
const wasmBytes = readFileSync(wasmPath);

const simulatorModule = await import(resolve(rootDir, 'src/lib/ressim/pkg/simulator.js'));
const { initSync, ReservoirSimulator, set_panic_hook } = simulatorModule;

initSync({ module: wasmBytes });
set_panic_hook();

const sim = new ReservoirSimulator(49, 49, 1);
const finalHistoryGrid = caseData.history[caseData.history.length-1].grid;
const finalWells = caseData.finalWells;
const simTime = caseData.simTime;

try {
  // Pass [] for rateHistory just like App.svelte does due to undefined activeCaseData
  sim.loadState(simTime, finalHistoryGrid, finalWells, []);
  console.log("Hydration SUCCESS with []");
} catch(e) {
  console.error("Hydration ERROR with []:", e);
}
