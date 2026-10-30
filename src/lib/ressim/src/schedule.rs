//! Declarative well-event schedules, shared by the black-oil and compositional engines (#50, #17).
//!
//! The envelope is deliberately engine-neutral: **a time, a well, and a change**. What a change
//! may contain is the engine's own type — compositional controls and injection belong to
//! [`crate::compositional::api`], black-oil controls to the black-oil boundary — but both engines
//! consume the same event shape, order rules and cursor semantics. That is what keeps the
//! scenario payload one contract rather than two (#17).
//!
//! Events are **absolute** times in days, not offsets. An event applies at the end of the
//! accepted step that reaches its time, so a well's source over `[t, t_next)` is governed by the
//! last event at or before `t`. The engine caps a requested step at the next event rather than
//! applying a change part-way through a step; a report step may therefore be split, and the
//! caller's run loop has to continue on the remainder (the compositional session already does).
//!
//! Validation rejects an unsorted or ambiguous schedule instead of sorting it. A schedule whose
//! two entries for one well at one instant can be read in either order does not have a defined
//! answer, and silently choosing one is how a run means something other than what it says.

use serde::{Deserialize, Serialize};

/// One scheduled change to one well, at an absolute simulated time.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WellEvent<C> {
    /// Simulated time [day] at which the change takes effect.
    pub time_days: f64,
    /// Stable well identifier, matching a configured well.
    pub well: String,
    /// What changes. The engine's own type, flattened into this event.
    #[serde(flatten)]
    pub change: C,
}

/// Check the parts of a schedule every engine shares: finite, ordered times and unique slots.
///
/// The change payload is not inspected here; its owner validates the fields it understands and
/// refuses the ones it does not.
pub fn validate_event_times<C>(events: &[WellEvent<C>]) -> Result<(), String> {
    // Every well at one instant must appear at most once, not merely not twice in a row. A
    // schedule may legitimately interleave wells at the same time (`INJ`, `PROD`, `INJ` is the
    // failure case): the second `INJ` would otherwise silently overwrite the first.
    let mut same_time_wells: Vec<&str> = Vec::new();
    for (index, event) in events.iter().enumerate() {
        if event.well.trim().is_empty() {
            return Err(format!("schedule event {index} has an empty well id"));
        }
        if !event.time_days.is_finite() {
            return Err(format!(
                "schedule event {index} for well {:?} has a non-finite time {}",
                event.well, event.time_days
            ));
        }
        if event.time_days < 0.0 {
            return Err(format!(
                "schedule event {index} for well {:?} is at {} day, before the start of the run",
                event.well, event.time_days
            ));
        }
        if let Some(previous) = index.checked_sub(1).map(|i| &events[i]) {
            if event.time_days < previous.time_days {
                return Err(format!(
                    "schedule events must be in nondecreasing time order; event {index} for well \
                     {:?} at {} day follows event {} for well {:?} at {} day",
                    event.well,
                    event.time_days,
                    index - 1,
                    previous.well,
                    previous.time_days
                ));
            }
            if event.time_days != previous.time_days {
                same_time_wells.clear();
            }
        }
        if same_time_wells.contains(&event.well.as_str()) {
            return Err(format!(
                "schedule has two changes for well {:?} at {} day; their order would be \
                 ambiguous",
                event.well, event.time_days
            ));
        }
        same_time_wells.push(&event.well);
    }
    Ok(())
}

/// True when `time_days` has reached an event at `event_time_days` within the schedule's clock
/// tolerance.
///
/// The tolerance is scaled with the clock so a long run is not held to an absolute day fraction
/// that its floating-point representation cannot carry. It is used only to decide whether an
/// event boundary has been reached, never to move the clock.
pub fn event_reached(time_days: f64, event_time_days: f64) -> bool {
    time_days + event_tolerance(time_days) >= event_time_days
}

fn event_tolerance(time_days: f64) -> f64 {
    1e-9 * time_days.abs().max(1.0)
}

/// The largest `dt` that ends no later than the next event.
pub fn cap_dt_at_next_event<C>(
    events: &[WellEvent<C>],
    position: usize,
    time_days: f64,
    dt_days: f64,
) -> f64 {
    match events.get(position) {
        Some(event) => dt_days.min((event.time_days - time_days).max(0.0)),
        None => dt_days,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    struct Change {
        value: f64,
    }

    fn event(time_days: f64, well: &str, value: f64) -> WellEvent<Change> {
        WellEvent {
            time_days,
            well: well.to_string(),
            change: Change { value },
        }
    }

    #[test]
    fn schedule_envelope_round_trips_as_plain_data() {
        let events = vec![event(1.0, "INJ", 2.0)];
        let json = serde_json::to_string(&events).unwrap();
        assert!(json.contains("\"value\""));
        let parsed: Vec<WellEvent<Change>> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, events);
    }

    #[test]
    fn schedule_validation_rejects_unsorted_and_ambiguous_events() {
        assert!(validate_event_times(&[event(1.0, "A", 0.0), event(2.0, "B", 0.0)]).is_ok());
        assert!(validate_event_times(&[event(2.0, "A", 0.0), event(1.0, "B", 0.0)]).is_err());
        assert!(validate_event_times(&[event(1.0, "A", 0.0), event(1.0, "A", 1.0)]).is_err());
        assert!(validate_event_times(&[event(1.0, "A", 0.0), event(1.0, "B", 1.0)]).is_ok());
        // Ambiguity is per instant, not per adjacent pair: a second change for A after B must
        // still be refused even though A is not the immediately preceding entry.
        assert!(
            validate_event_times(&[
                event(1.0, "A", 0.0),
                event(1.0, "B", 1.0),
                event(1.0, "A", 2.0),
            ])
            .is_err()
        );
        assert!(
            validate_event_times(&[
                event(1.0, "A", 0.0),
                event(1.0, "B", 1.0),
                event(2.0, "A", 2.0),
            ])
            .is_ok()
        );
        assert!(validate_event_times(&[event(-1.0, "A", 0.0)]).is_err());
        assert!(validate_event_times(&[event(f64::NAN, "A", 0.0)]).is_err());
        assert!(validate_event_times(&[event(1.0, " ", 0.0)]).is_err());
    }

    #[test]
    fn schedule_caps_a_step_at_the_next_event_and_not_before_it() {
        let events = vec![event(1.0, "A", 0.0), event(2.5, "A", 0.0)];
        assert_eq!(cap_dt_at_next_event(&events, 0, 0.0, 5.0), 1.0);
        assert_eq!(cap_dt_at_next_event(&events, 0, 0.0, 0.25), 0.25);
        assert_eq!(cap_dt_at_next_event(&events, 1, 1.0, 5.0), 1.5);
        assert_eq!(cap_dt_at_next_event(&events, 2, 2.5, 5.0), 5.0);
    }

    #[test]
    fn schedule_reaching_an_event_is_tolerant_of_clock_roundoff_only() {
        assert!(event_reached(20.0 - 1e-12, 20.0));
        assert!(!event_reached(19.999, 20.0));
        assert!(event_reached(0.0, 0.0));
    }
}
