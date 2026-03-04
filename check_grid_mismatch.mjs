import zlib from 'zlib';
import fs from 'fs';
const file = 'public/cases/prerun/mode-sim_geo-2dxy_well-corner_rock-uni_grav-off_cap-off_fluid-std_grid-def_dt-def.json.gz';
if(fs.existsSync(file)) {
    const data = JSON.parse(zlib.gunzipSync(fs.readFileSync(file)).toString());
    const expectedCellCount = 21 * 21 * 1;
    let mismatches = 0;
    for (let i = 0; i < data.history.length; i++) {
        const entry = data.history[i];
        if (entry.grid && entry.grid.pressure) {
            if (entry.grid.pressure.length !== expectedCellCount) {
                 console.log(`Mismatch at index ${i}: len=${entry.grid.pressure.length}`);
                 mismatches++;
            }
        }
    }
    console.log(`Expected size ${expectedCellCount}. Total mismatches: ${mismatches} out of ${data.history.length}`);
}
