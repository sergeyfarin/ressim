pub(crate) mod ad;
pub(crate) mod assembly;
pub(crate) mod assembly_ad;
pub(crate) mod flash;
pub(crate) mod flash_ad;
pub(crate) mod flow_resv;
pub(crate) mod flux;
pub(crate) mod linear;
pub(crate) mod newton;
#[cfg(test)]
pub(crate) mod numjac;
pub(crate) mod properties;
pub(crate) mod scaling;
pub(crate) mod state;
pub(crate) mod timestep;
// Native-only: every call site is already `#[cfg(not(target_arch = "wasm32"))]`, and the sink
// writes to the filesystem, so compiling it for wasm only produced dead `std::fs` code.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod trace_sink;
pub(crate) mod wells;
pub(crate) mod wells_ad;
pub(crate) mod wells_inner;

#[cfg(test)]
mod tests;
