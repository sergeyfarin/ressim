import { ReservoirSimulator } from '../src/lib/ressim-wasm/simulator.js';
import { spe1_gas_injection } from '../src/lib/catalog/scenarios/spe1_gas_injection';
import { buildCreatePayloadFromState } from '../src/lib/buildCreatePayload';

import fs from 'fs';

async function main() {
    
    const initialState = spe1_gas_injection.params;
    initialState.nx = 2;
    initialState.ny = 2;
    initialState.nz = 1;
    initialState.injectorI = 0;
    initialState.injectorJ = 0;
    initialState.injectorKLayers = [0];
    initialState.producerI = 1;
    initialState.producerJ = 1;
    initialState.producerKLayers = [0];
    
    // Convert control to rate so we know we inject
    initialState.injectorControlMode = 'rate';
    initialState.targetInjectorSurfaceRate = 1e6; // very high
    
    const payload = buildCreatePayloadFromState(initialState);
    const sim = new ReservoirSimulator(payload.nx, payload.ny, payload.nz, Number(payload.porosity));
    sim.setFimEnabled(true);
    sim.setCellDimensions(payload.cellDx, payload.cellDy, payload.cellDz);
    
    // We only want a single layer
    
    sim.setPvtTable(payload.pvtTable);
    sim.setGasFluidProperties(payload.mu_g, payload.c_g, payload.rho_g ?? 1.22);
    sim.setThreePhaseModeEnabled(true);
    sim.setGasRedissolutionEnabled(false);
    sim.setInjectedFluid('gas');
    sim.setInitialPressure(payload.initialPressure);
    sim.setInitialSaturation(payload.initialSaturation);
    sim.setInitialGasSaturation(0.0);
    sim.setInitialRs(payload.initialRs);
    
    // Injector
    sim.addWellWithId(0, 0, 0, 621, 0.1, 0.0);
    sim.setWellSchedule("0", "rate", NaN, payload.targetInjectorSurfaceRate, 621, true);
    // Producer
    sim.addWellWithId(1, 1, 0, 100, 0.1, 0.0);
    sim.setWellSchedule("1", "pressure", NaN, NaN, 100, true);
    
    console.log("Before dt:", {
        sg: sim.getGasSaturation()[0],
        rs: sim.getRs()[0],
        pressure: sim.getPressure()[0]
    });
    
    sim.stepFim(1.0);
    
    console.log("After dt 1:", {
        sg: sim.getGasSaturation()[0],
        rs: sim.getRs()[0],
        pressure: sim.getPressure()[0],
        inj_rate: sim.getWellRatesM3Day()[0]
    });
}
main().catch(console.error);
