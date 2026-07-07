pub(crate) mod ad;
pub(crate) mod assembly;
pub(crate) mod assembly_ad;
pub(crate) mod flash;
pub(crate) mod flash_ad;
pub(crate) mod flux;
pub(crate) mod linear;
pub(crate) mod newton;
#[cfg(test)]
pub(crate) mod numjac;
pub(crate) mod properties;
pub(crate) mod scaling;
pub(crate) mod state;
pub(crate) mod timestep;
pub(crate) mod wells;
pub(crate) mod wells_ad;

#[cfg(test)]
mod tests;
