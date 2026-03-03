use simulator::*;
use simulator::ReservoirSimulator;

fn check() {
        let mut sim = ReservoirSimulator::new(3, 1, 1, 0.2);
        sim.set_permeability_random(100_000.0, 100_000.0).unwrap();
        sim.set_stability_params(0.01, 75.0, 0.75);
        sim.add_well(0, 0, 0, 700.0, 0.1, 0.0, true).unwrap();
        sim.add_well(2, 0, 0, 50.0, 0.1, 0.0, false).unwrap();

        sim.step(30.0);
        println!("History Length: {}", sim.rate_history.len());
        println!("Time Days: {}", sim.time_days);
}
fn main() { check(); }
