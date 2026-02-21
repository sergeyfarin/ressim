import { readFileSync, writeFileSync } from 'node:fs';

let typesFile = 'src/lib/simulator-types.ts';
let types = readFileSync(typesFile, 'utf8');

types = types.replace(/grid\?: GridCell\[\];/g, 'grid?: GridState;');
types = types.replace(/grid: GridCell\[\];/g, 'grid: GridState;');
types = types.replace(/export interface GridCell \{[\s\S]*?\}\n/g, `export interface GridState {
  pressure: Float64Array;
  sat_water: Float64Array;
  sat_oil: Float64Array;
  porosity: Float64Array;
  perm_x: Float64Array;
  perm_y: Float64Array;
  perm_z: Float64Array;
}
`);
writeFileSync(typesFile, types);

let viewFile = 'src/lib/3dview.svelte';
let view = readFileSync(viewFile, 'utf8');
view = view.replace(/GridCell\[\]/g, 'GridState');
view = view.replace(/GridCell\[\]\[\]/g, 'GridState[]');
view = view.replace(/GridCell/g, 'GridState'); // for imports and single cell types?? Wait, single cell types!
writeFileSync(viewFile, view);

let appFile = 'src/App.svelte';
let app = readFileSync(appFile, 'utf8');
app = app.replace(/GridCell\[\]/g, 'GridState');
app = app.replace(/GridCell/g, 'GridState');
writeFileSync(appFile, app);

let chartFile = 'src/lib/SwProfileChart.svelte';
let chart = readFileSync(chartFile, 'utf8');
chart = chart.replace(/GridCell\[\]/g, 'GridState');
chart = chart.replace(/GridCell/g, 'GridState');
writeFileSync(chartFile, chart);
