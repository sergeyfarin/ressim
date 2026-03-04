const zlib = require('zlib');
const fs = require('fs');
const file = 'public/cases/prerun/mode-sim_geo-2dxy_well-corner_rock-uni_grav-off_cap-off_fluid-std_grid-def_dt-def.json.gz';
if(fs.existsSync(file)) {
    console.log("File exists, reading...");
    const data = JSON.parse(zlib.gunzipSync(fs.readFileSync(file)).toString());
    console.log('history size:', data.history.length);
    console.log('first grid size:', data.history[0].grid.pressure.length);
    console.log('rate history type:', typeof data.rateHistory);
    console.log('is array?', Array.isArray(data.rateHistory));
    console.log('keys if obj:', !Array.isArray(data.rateHistory) ? Object.keys(data.rateHistory).join(',') : 'n/a');
    if (Array.isArray(data.rateHistory)) console.log('first rate item:', data.rateHistory[0]);
} else {
    console.log('File not found', file);
}
