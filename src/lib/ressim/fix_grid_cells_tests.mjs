import { readFileSync, writeFileSync } from 'node:fs';

let lib = readFileSync('src/lib/ressim/src/lib.rs', 'utf8');

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

writeFileSync('src/lib/ressim/src/lib.rs', lib);

let step = readFileSync('src/lib/ressim/src/step.rs', 'utf8');
step = step.replace(/x0\[i\] = self\.grid_cells\[i\]\.pressure;/g, 'x0[i] = self.pressure[i];');
step = step.replace(/let pc_i = self\.get_capillary_pressure\(self\.grid_cells\[id\]\.sat_water\);/g, 'let pc_i = self.get_capillary_pressure(self.sat_water[id]);');
step = step.replace(/let pc_j = self\.get_capillary_pressure\(self\.grid_cells\[nid\]\.sat_water\);/g, 'let pc_j = self.get_capillary_pressure(self.sat_water[nid]);');
step = step.replace(/let vp_m3 = self\.grid_cells\[idx\]\.pore_volume_m3\(self\.dx, self\.dy, self\.dz\);/g, 'let vp_m3 = self.pore_volume_m3(idx);');
step = step.replace(/let dp = \(p_new\[idx\] - self\.grid_cells\[idx\]\.pressure\)\.abs\(\);/g, 'let dp = (p_new[idx] - self.pressure[idx]).abs();');
step = step.replace(/let q_old = self\.well_rate_m3_day\(w, self\.grid_cells\[id\]\.pressure\)\.unwrap_or\(0\.0\);/g, 'let q_old = self.well_rate_m3_day(w, self.pressure[id]).unwrap_or(0.0);');
step = step.replace(/let sw_old = self\.grid_cells\[idx\]\.sat_water;/g, 'let sw_old = self.sat_water[idx];');
step = step.replace(/self\.grid_cells\[idx\]\.sat_water = sw_new;/g, 'self.sat_water[idx] = sw_new;');
step = step.replace(/self\.grid_cells\[idx\]\.sat_oil = so_new;/g, 'self.sat_oil[idx] = so_new;');
step = step.replace(/self\.grid_cells\[idx\]\.pressure = p_new\[idx\];/g, 'self.pressure[idx] = p_new[idx];');
step = step.replace(/for cell in self\.grid_cells\.iter\(\) {/g, 'for i in 0..self.nx*self.ny*self.nz {');
// pore volume replace in step.rs 
step = step.replace(/let vp_m3 = cell\.pore_volume_m3\(self\.dx, self\.dy, self\.dz\);/g, 'let vp_m3 = self.pore_volume_m3(i);');

writeFileSync('src/lib/ressim/src/step.rs', step);
