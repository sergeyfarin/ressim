#!/usr/bin/env node
/**
 * Runs the shared case through the *browser* bindings, under Node, and writes the result as JSON.
 *
 * This is the reference half of the native-binding parity check. It loads the same generated
 * wasm bundle the application ships, so agreement between this and the native run means the two
 * bindings expose the same engine — not that two copies of the physics happen to agree.
 */
import { readFile, writeFile } from 'node:fs/promises'
import init, { ReservoirSimulator } from '../../../src/lib/ressim/pkg/simulator.js'

const here = new URL('.', import.meta.url)
const c = JSON.parse(await readFile(new URL('case.json', here), 'utf8'))

await init({ module_or_path: await readFile(new URL('../../../src/lib/ressim/pkg/simulator_bg.wasm', here)) })

const sim = new ReservoirSimulator(c.nx, 1, 1, c.porosity)
sim.setFimEnabled(false)
sim.setRelPermProps(c.s_wc, c.s_or, c.n_w, c.n_o, 1.0, 1.0)
sim.setInitialSaturation(c.s_wc)
sim.setPermeabilityRandomSeeded(c.permeability_md, c.permeability_md, BigInt(c.seed))
sim.setStabilityParams(0.05, 75.0, 0.75)
sim.setCapillaryParams(0.0, 2.0)
sim.setFluidProperties(c.mu_o, c.mu_w)
sim.addWellWithId(0, 0, 0, c.injector_bhp, 0.1, 0.0, true, 'inj')
sim.addWellWithId(c.nx - 1, 0, 0, c.producer_bhp, 0.1, 0.0, false, 'prod')

for (let i = 0; i < c.steps; i += 1) sim.step(c.dt_days)

await writeFile(
  new URL('reference_wasm.json', here),
  `${JSON.stringify({ pressures: [...sim.getPressures()], satWater: [...sim.getSatWater()] }, null, 2)}\n`,
)
console.log(`wrote reference_wasm.json: ${c.nx} cells, ${c.steps} steps`)
