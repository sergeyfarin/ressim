import { readFileSync, writeFileSync } from 'node:fs';

let lib = readFileSync('src/lib/ressim/src/lib.rs', 'utf8');

// Rust ReservoirSimulator struct replacement
lib = lib.replace(/grid_cells: Vec<GridCell>,/, 
`porosity: Vec<f64>,
    perm_x: Vec<f64>,
    perm_y: Vec<f64>,
    perm_z: Vec<f64>,
    pressure: Vec<f64>,
    sat_water: Vec<f64>,
    sat_oil: Vec<f64>,`);

lib = lib.replace(/let grid_cells = vec!\[GridCell::default_cell\(\); n\];/, `let porosity = vec![0.2; n];
        let perm_x = vec![100.0; n];
        let perm_y = vec![100.0; n];
        let perm_z = vec![10.0; n];
        let pressure = vec![300.0; n];
        let sat_water = vec![0.3; n];
        let sat_oil = vec![0.7; n];`);

lib = lib.replace(/grid_cells,/, `porosity,
            perm_x,
            perm_y,
            perm_z,
            pressure,
            sat_water,
            sat_oil,`);

// Replacements for common accesses in lib.rs
lib = lib.replace(/self\.grid_cells\[(.*?)\]\.pressure/g, 'self.pressure[$1]');
lib = lib.replace(/self\.grid_cells\[(.*?)\]\.sat_water/g, 'self.sat_water[$1]');
lib = lib.replace(/self\.grid_cells\[(.*?)\]\.sat_oil/g, 'self.sat_oil[$1]');
lib = lib.replace(/self\.grid_cells\[(.*?)\]\.perm_x/g, 'self.perm_x[$1]');
lib = lib.replace(/self\.grid_cells\[(.*?)\]\.perm_y/g, 'self.perm_y[$1]');
lib = lib.replace(/self\.grid_cells\[(.*?)\]\.perm_z/g, 'self.perm_z[$1]');

lib = lib.replace(/cell\.perm_x /g, 'self.perm_x[id] ');
lib = lib.replace(/cell\.perm_y /g, 'self.perm_y[id] ');
lib = lib.replace(/cell\.perm_z /g, 'self.perm_z[id] ');

lib = lib.replace(/sim\.grid_cells\[(.*?)\]\.pressure/g, 'sim.pressure[$1]');
lib = lib.replace(/sim\.grid_cells\[(.*?)\]\.sat_water/g, 'sim.sat_water[$1]');
lib = lib.replace(/sim\.grid_cells\[(.*?)\]\.sat_oil/g, 'sim.sat_oil[$1]');
lib = lib.replace(/for cell in \w*\.?grid_cells\.iter_mut\(\)/g, 'for i in 0..self.nx*self.ny*self.nz');
lib = lib.replace(/cell\.perm_x = /g, 'self.perm_x[i] = ');
lib = lib.replace(/cell\.perm_y = /g, 'self.perm_y[i] = ');
lib = lib.replace(/cell\.perm_z = /g, 'self.perm_z[i] = ');

writeFileSync('src/lib/ressim/src/lib.rs', lib);

// same script for grid.rs to drop the struct later
