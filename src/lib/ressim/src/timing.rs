//! Wall-clock timing. The wasm32 arm calls `js_sys::Date::now()`, and `js-sys` is only
//! linked with the `wasm` feature, so both arms are gated on the *pair* rather than on the
//! target alone — otherwise a wasm32 build without the feature would select an arm whose
//! dependency is absent, or no arm at all.

#[cfg(not(all(target_arch = "wasm32", feature = "wasm")))]
use std::time::Instant;

pub(crate) struct PerfTimer {
    #[cfg(not(all(target_arch = "wasm32", feature = "wasm")))]
    start: Instant,
    #[cfg(all(target_arch = "wasm32", feature = "wasm"))]
    start_ms: f64,
}

impl PerfTimer {
    pub(crate) fn start() -> Self {
        Self {
            #[cfg(not(all(target_arch = "wasm32", feature = "wasm")))]
            start: Instant::now(),
            #[cfg(all(target_arch = "wasm32", feature = "wasm"))]
            start_ms: js_sys::Date::now(),
        }
    }

    pub(crate) fn elapsed_ms(&self) -> f64 {
        #[cfg(not(all(target_arch = "wasm32", feature = "wasm")))]
        {
            self.start.elapsed().as_secs_f64() * 1_000.0
        }

        #[cfg(all(target_arch = "wasm32", feature = "wasm"))]
        {
            js_sys::Date::now() - self.start_ms
        }
    }
}
