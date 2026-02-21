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
lib = lib.replace(/sim\.grid_cells/g, ''); // wait, this could break. I'll just write sed rules

let step = readFileSync('src/lib/ressim/src/step.rs', 'utf8');
step = step.replace(/x0\[i\] = self\.grid_cells\[i\]\.pressure;/g, 'x0[i] = self.pressure[i];');
step = step.replace(/let vp_m3 = self\.grid_cells\[idx\]\.pore_volume_m3\(self\.dx, self\.dy, self\.dz\);/g, 'let vp_m3 = self.pore_volume_m3(idx);');
step = step.replace(/let dp = \(p_new\[idx\] - self\.grid_cells\[idx\]\.pressure\)\.abs\(\);/g, 'let dp = (p_new[idx] - self.pressure[idx]).abs();');
step = step.replace(/let q_old = self\.well_rate_m3_day\(w, self\.grid_cells\[id\]\.pressure\)\.unwrap_or\(0\.0\);/g, 'let q_old = self.well_rate_m3_day(w, self.pressure[id]).unwrap_or(0.0);');
step = step.replace(/self\.grid_cells\[idx\]\.sat_oil = so_new;/g, 'self.sat_oil[idx] = so_new;');
step = step.replace(/self\.grid_cells\[idx\]\.pressure = p_new\[idx\];/g, 'self.pressure[idx] = p_new[idx];');
step = step.replace(/for cell in self\.grid_cells\.iter\(\) {/g, 'for i in 0..self.nx*self.ny*self.nz {');

writeFileSync('src/lib/ressim/src/step.rs', step);
