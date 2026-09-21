#!/usr/bin/env node
/**
 * Runs the parity matrix through the *browser* bindings, under Node, and writes the results.
 *
 * This is the reference half of the cross-client check. It loads the same generated wasm bundle
 * the application ships, so agreement between this and the native run means the two bindings
 * expose the same engine — not that two copies of the physics happen to agree.
 *
 * Both runners read `cases.json` verbatim and configure through the same ordered calls, so a
 * disagreement is attributable to the binding rather than to one runner having set something up
 * differently.
 */
import { readFile, writeFile } from 'node:fs/promises'
import init, { ReservoirSimulator } from '../../../src/lib/ressim/pkg/simulator.js'

const here = new URL('.', import.meta.url)
const spec = JSON.parse(await readFile(new URL('cases.json', here), 'utf8'))

await init({
  module_or_path: await readFile(new URL('../../../src/lib/ressim/pkg/simulator_bg.wasm', here)),
})

/** Configure and run one case. Keep the call order identical to compare_native.py. */
function runCase(c) {
    const sim = new ReservoirSimulator(c.nx, c.ny, c.nz, c.porosity)
    sim.setFimEnabled(Boolean(c.fim))
    sim.setRelPermProps(c.s_wc, c.s_or, c.n_w, c.n_o, 1.0, 1.0)
    sim.setInitialSaturation(c.s_wc)
    sim.setPermeabilityRandomSeeded(c.permeability_md, c.permeability_md, BigInt(c.seed))
    sim.setStabilityParams(0.05, 75.0, 0.75)
    sim.setCapillaryParams(0.0, 2.0)
    sim.setFluidProperties(c.mu_o, c.mu_w)
    sim.addWellWithId(0, 0, 0, c.injector_bhp, 0.1, 0.0, true, 'inj')
    sim.addWellWithId(c.nx - 1, 0, 0, c.producer_bhp, 0.1, 0.0, false, 'prod')

    for (let i = 0; i < c.steps; i += 1) sim.step(c.dt_days)

    return {
        pressures: [...sim.getPressures()],
        satWater: [...sim.getSatWater()],
        dimensions: sim.getDimensions(),
        rateHistory: sim.getRateHistory(),
        // The zero-copy path: Float64Array views over engine memory rather than a serde payload.
        // Comparing it against the native `grid_state()` is what keeps that optimization honest.
        gridState: Object.fromEntries(
            Object.entries(sim.getGridState()).map(([k, v]) => [k, Array.from(v)]),
        ),
    }
}

const results = {}
for (const override of spec.cases) {
    const c = { ...spec.base, ...override }
    results[c.id] = runCase(c)
    console.log(
        `  ${c.id.padEnd(22)} ${c.nx} cells, ${c.steps} steps, ${c.fim ? 'FIM' : 'IMPES'}` +
            `${c.strict ? '' : '  (non-strict)'}`,
    )
}

await writeFile(new URL('reference_wasm.json', here), `${JSON.stringify(results, null, 2)}\n`)
console.log(`wrote reference_wasm.json: ${Object.keys(results).length} cases`)
