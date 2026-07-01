pub(crate) mod ad;
pub(crate) mod assembly;
pub(crate) mod flash;
#[cfg(test)]
pub(crate) mod numjac;
pub(crate) mod linear;
pub(crate) mod newton;
pub(crate) mod properties;
pub(crate) mod scaling;
pub(crate) mod state;
pub(crate) mod timestep;
pub(crate) mod wells;

#[cfg(test)]
mod tests;
