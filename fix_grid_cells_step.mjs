import { readFileSync, writeFileSync } from 'node:fs';

let step = readFileSync('src/lib/ressim/src/step.rs', 'utf8');

// Replace function signatures that take `&GridCell`
step = step.replace(/fn total_mobility\(&self, cell: &GridCell\)/g, 'fn total_mobility(&self, id: usize)');
step = step.replace(/fn phase_mobilities\(&self, cell: &GridCell\)/g, 'fn phase_mobilities(&self, id: usize)');
step = step.replace(/fn frac_flow_water\(&self, cell: &GridCell\)/g, 'fn frac_flow_water(&self, id: usize)');
step = step.replace(/fn calculate_well_productivity_index\(\s*&self,\s*cell: &GridCell,/g, 'fn calculate_well_productivity_index(&self, id: usize,');
step = step.replace(/fn total_density_face\(&self, c_i: &GridCell, c_j: &GridCell\)/g, 'fn total_density_face(&self, i: usize, j: usize)');
step = step.replace(/fn geometric_transmissibility\(&self, c1: &GridCell, c2: &GridCell, dim: char\)/g, 'fn geometric_transmissibility(&self, id1: usize, id2: usize, dim: char)');
step = step.replace(/fn transmissibility_upstream\(\s*&self,\s*c1: &GridCell,\s*c2: &GridCell,\s*dim: char,/g, 'fn transmissibility_upstream(&self, id1: usize, id2: usize, dim: char,');
step = step.replace(/fn transmissibility_with_prev_pressure\(\s*&self,\s*c1: &GridCell,\s*c2: &GridCell,\s*dim: char,\s*grav_head_bar: f64,\s*\) -> f64/g, 'fn transmissibility_with_prev_pressure(&self, id1: usize, id2: usize, dim: char, grav_head_bar: f64) -> f64');

// Replace function body accesses inside step.rs
step = step.replace(/cell\.sat_water/g, 'self.sat_water[id]');
step = step.replace(/cell\.sat_oil/g, 'self.sat_oil[id]');
step = step.replace(/cell\.pressure/g, 'self.pressure[id]');
step = step.replace(/cell\.perm_x/g, 'self.perm_x[id]');
step = step.replace(/cell\.perm_y/g, 'self.perm_y[id]');
step = step.replace(/cell\.perm_z/g, 'self.perm_z[id]');

// Replace function calls passing `&self.grid_cells[id]`
step = step.replace(/&self\.grid_cells\[(.*?)\]/g, '$1');
step = step.replace(/c1\.perm_x/g, 'self.perm_x[id1]');
step = step.replace(/c2\.perm_x/g, 'self.perm_x[id2]');
step = step.replace(/c1\.perm_y/g, 'self.perm_y[id1]');
step = step.replace(/c2\.perm_y/g, 'self.perm_y[id2]');
step = step.replace(/c1\.perm_z/g, 'self.perm_z[id1]');
step = step.replace(/c2\.perm_z/g, 'self.perm_z[id2]');

step = step.replace(/c1\.pressure/g, 'self.pressure[id1]');
step = step.replace(/c2\.pressure/g, 'self.pressure[id2]');
step = step.replace(/&c_i/g, 'i');
step = step.replace(/&c_j/g, 'j');

step = step.replace(/self\.transmissibility_upstream\(c1, c2, dim, c1\.pressure, c2\.pressure, grav_head_bar\)/g, 'self.transmissibility_upstream(id1, id2, dim, self.pressure[id1], self.pressure[id2], grav_head_bar)');

step = step.replace(/c1/g, 'id1');
step = step.replace(/c2/g, 'id2');

step = step.replace(/let cell = self\.grid_cells\[id\];/g, '');
step = step.replace(/calculate_well_productivity_index\(&cell,/g, 'calculate_well_productivity_index(id,');

// Replace loop in update_saturations_and_pressure
// `for (i, cell) in self.grid_cells.iter_mut().enumerate()` -> `for id in 0..self.nx*self.ny*self.nz`
step = step.replace(/for \(i, cell\) in self\.grid_cells\.iter_mut\(\)\.enumerate\(\)/g, "for i in 0..self.nx*self.ny*self.nz");
step = step.replace(/cell\.pressure\s*=\s*p_new\[i\];/g, "self.pressure[i] = p_new[i];");
step = step.replace(/cell\.sat_water\s*\+=\s*ds;/g, "self.sat_water[i] += ds;");
step = step.replace(/cell\.sat_water\s*=\s*cell\.sat_water\.clamp\(s_wc, 1\.0 - s_or\);/g, "self.sat_water[i] = self.sat_water[i].clamp(s_wc, 1.0 - s_or);");
step = step.replace(/cell\.sat_oil\s*=\s*1\.0 - cell\.sat_water;/g, "self.sat_oil[i] = 1.0 - self.sat_water[i];");

writeFileSync('src/lib/ressim/src/step.rs', step);
