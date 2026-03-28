cd /home/reken/Repos/ressim
node --input-type=module <<'EOF'
import { readFile } from 'node:fs/promises';
import init, { ReservoirSimulator } from './src/lib/ressim/pkg/simulator.js';

const wasmBytes = await readFile(new URL('./src/lib/ressim/pkg/simulator_bg.wasm', import.meta.url));
await init({ module_or_path: wasmBytes });

const nx = 36;
const sim = new ReservoirSimulator(nx, 1, 1, 0.2);
sim.setFimEnabled(true);
sim.setCellDimensions(10, 10, 1);
sim.setRelPermProps(0.1, 0.1, 2, 2, 1, 1);
sim.setInitialPressure(300);
sim.setInitialSaturation(0.1);
sim.setFluidProperties(1.0, 0.5);
sim.setFluidCompressibilities(1e-5, 3e-6);
sim.setRockProperties(1e-6, 0, 1, 1);
sim.setFluidDensities(800, 1000);
sim.setCapillaryParams(0, 2);
sim.setGravityEnabled(false);
sim.setPermeabilityPerLayer(new Float64Array([2000]), new Float64Array([2000]), new Float64Array([200]));
sim.setStabilityParams(0.05, 75, 0.75);
sim.setWellControlModes('pressure', 'pressure');
sim.setTargetWellRates(0, 0);
sim.setWellBhpLimits(100, 500);
sim.add_well(0, 0, 0, 500, 0.1, 0, true);
sim.add_well(nx - 1, 0, 0, 100, 0.1, 0, false);

const t0 = performance.now();
sim.step(0.25);
const ms = performance.now() - t0;
const hist = sim.getRateHistory();
const last = hist.at(-1);

console.log(JSON.stringify({
  nx,
  ms,
  time: last?.time,
  warning: sim.getLastSolverWarning(),
  history: hist.length
}));
EOF