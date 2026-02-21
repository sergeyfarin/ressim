import { readFileSync, writeFileSync } from 'node:fs';

let lib = readFileSync('src/lib.rs', 'utf8');

lib = lib.replace(/impl ReservoirSimulator {/, `impl ReservoirSimulator {
    pub fn pore_volume_m3(&self, id: usize) -> f64 {
        self.dx * self.dy * self.dz * self.porosity[id]
    }
`);

writeFileSync('src/lib.rs', lib);

let step = readFileSync('src/step.rs', 'utf8');
step = step.replace(/self\.grid_cells\[(.*?)\]\.sat_water/g, 'self.sat_water[$1]');
step = step.replace(/let vp_m3 = cell\.pore_volume_m3\(self\.dx, self\.dy, self\.dz\);/g, 'let vp_m3 = self.pore_volume_m3(i);');

step = step.replace(/let total_mobility = self\.total_mobility\(cell\);/g, 'let total_mobility = self.total_mobility(id);');

writeFileSync('src/step.rs', step);
