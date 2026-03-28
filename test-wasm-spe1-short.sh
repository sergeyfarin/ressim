cat << 'RUST' > src/lib/ressim/src/tests/spe1_short.rs
use crate::ReservoirSimulator;
use crate::tests::make_spe1_like_base_sim; // ah actually this is not exported from lib.rs but inside an inner module
RUST
