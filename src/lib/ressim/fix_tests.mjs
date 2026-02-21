import { readFileSync, writeFileSync } from 'node:fs';

let lib = readFileSync('src/lib.rs', 'utf8');

lib = lib.replace(/let cell = self\.grid_cells\[cell_id\];/g, '');
lib = lib.replace(/let pi = self\.calculate_well_productivity_index\(&cell, well_radius, skin\)\?;/g, 'let pi = self.calculate_well_productivity_index(cell_id, well_radius, skin)?;');
lib = lib.replace(/serde_wasm_bindgen::to_value\(&self\.grid_cells\)\.unwrap\(\)/g, "JsValue::NULL"); // We'll fix grid state getter properly later

lib = lib.replace(/let grid_cells: Vec<GridCell> = serde_wasm_bindgen::from_value\(grid_state\)\?;/, '');
lib = lib.replace(/if grid_cells\.len\(\) != expected_cells {/, 'if false {');
lib = lib.replace(/grid_cells\.len\(\)/, '0');
lib = lib.replace(/self\.grid_cells = grid_cells;/, '// TODO impl restore');

// tests
lib = lib.replace(/sim\.grid_cells\n\s*\.iter\(\)\n\s*\.map\(\|cell\| cell\.sat_water \* cell\.pore_volume_m3\(sim\.dx, sim\.dy, sim\.dz\)\)/g, 
  '(0..sim.nx*sim.ny*sim.nz).map(|i| sim.sat_water[i] * sim.pore_volume_m3(i))');
lib = lib.replace(/sim\n\s*\.grid_cells\n\s*\.iter\(\)\n\s*\.map\(\|cell\| cell\.pore_volume_m3\(sim\.dx, sim\.dy, sim\.dz\)\)/g, 
  '(0..sim.nx*sim.ny*sim.nz).map(|i| sim.pore_volume_m3(i))');

lib = lib.replace(/for cell in &sim\.grid_cells {/g, 'for i in 0..sim.nx*sim.ny*sim.nz {');
lib = lib.replace(/cell\.pressure/g, 'sim.pressure[i]');
lib = lib.replace(/cell\.sat_water/g, 'sim.sat_water[i]');
lib = lib.replace(/cell\.sat_oil/g, 'sim.sat_oil[i]');

// The indexer for sim_g.grid_cells[top_id_g].pressure etc
lib = lib.replace(/sim_no_g\.grid_cells\[(.*?)\]\.pressure/g, 'sim_no_g.pressure[$1]');
lib = lib.replace(/sim_g\.grid_cells\[(.*?)\]\.pressure/g, 'sim_g.pressure[$1]');
lib = lib.replace(/sim_g\.grid_cells\[(.*?)\]\.sat_water/g, 'sim_g.sat_water[$1]');
lib = lib.replace(/sim_no_g\.grid_cells\[(.*?)\]\.sat_water/g, 'sim_no_g.sat_water[$1]');

writeFileSync('src/lib.rs', lib);
