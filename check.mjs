import zlib from 'zlib';
import fs from 'fs';
const file = 'public/cases/prerun/mode-sim_geo-2dxy_well-corner_rock-uni_grav-off_cap-off_fluid-std_grid-def_dt-def.json.gz';
if(fs.existsSync(file)) {
    const data = JSON.parse(zlib.gunzipSync(fs.readFileSync(file)).toString());
    const lastHist = data.history[data.history.length - 1];
    console.log('last grid size:', lastHist.grid.pressure.length);
    console.log('rate history type:', typeof data.rateHistory);
    console.log('is array?', Array.isArray(data.rateHistory));
    if (Array.isArray(data.rateHistory)) console.log('first rate item:', data.rateHistory[0]);
}
