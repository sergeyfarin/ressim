//! Opt-in, test-only wall-clock attribution for the small-direct IMPES replay.
//! Timings are inclusive; do not add nested phases together.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

#[derive(Default)]
struct Profile {
    enabled: bool,
    phases: BTreeMap<&'static str, (usize, Duration)>,
}

thread_local! {
    static PROFILE: RefCell<Profile> = RefCell::new(Profile::default());
}

pub(crate) fn reset(enabled: bool) {
    PROFILE.with(|profile| {
        *profile.borrow_mut() = Profile {
            enabled,
            ..Profile::default()
        };
    });
}

pub(crate) struct Span {
    name: &'static str,
    started: Option<Instant>,
}

pub(crate) fn span(name: &'static str) -> Span {
    Span {
        name,
        started: PROFILE.with(|profile| profile.borrow().enabled.then(Instant::now)),
    }
}

impl Drop for Span {
    fn drop(&mut self) {
        if let Some(started) = self.started {
            let elapsed = started.elapsed();
            PROFILE.with(|profile| {
                let mut profile = profile.borrow_mut();
                let entry = profile.phases.entry(self.name).or_default();
                entry.0 += 1;
                entry.1 += elapsed;
            });
        }
    }
}

pub(crate) fn report() {
    PROFILE.with(|profile| {
        for (name, (calls, duration)) in &profile.borrow().phases {
            println!(
                "IMPES profile {name}: calls={calls} ms={:.3}",
                duration.as_secs_f64() * 1e3
            );
        }
    });
}
