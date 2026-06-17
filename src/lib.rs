//! Declarative data-flow system ordering for [Bevy](https://bevyengine.org/).
//!
//! Instead of hand-writing `.before()` / `.after()` chains, systems declare
//! *what data they read and write*. Ordering is then derived automatically per
//! `(type, schedule)` pair: every system in [`Writes<T>`] runs before every
//! system in [`Reads<T>`].
//!
//! - A system that produces data of type `T` joins `Writes::<T>::set()`.
//! - A system that consumes data of type `T` joins `Reads::<T>::set()`.
//! - [`register_data_flow::<T>`](register_data_flow) wires
//!   `Writes::<T>` before `Reads::<T>` in a given schedule.
//!
//! `T` names the data that flows from producers to consumers — **use the actual
//! resource / component / message type itself** (`Writes::<Orders>`,
//! `Reads::<SensorDetections>`). It is never constructed, so `Writes<T>` /
//! `Reads<T>` impose no trait bounds on `T` beyond `Send + Sync + 'static`.
//!
//! # Quick start
//!
//! ```
//! use bevy::prelude::*;
//! use bevy_ordering_sets::{Reads, Writes, register_data_flow};
//!
//! // The data that flows from producer to consumer. It is its own marker.
//! #[derive(Resource, Default)]
//! struct Targets(Vec<u32>);
//!
//! fn produce(mut targets: ResMut<Targets>) {
//!     targets.0.push(42);
//! }
//!
//! fn consume(targets: Res<Targets>) {
//!     // `produce` is guaranteed to have run first this frame.
//!     assert_eq!(targets.0, vec![42]);
//! }
//!
//! let mut app = App::new();
//! app.init_resource::<Targets>();
//!
//! // Order Writes::<Targets> before Reads::<Targets> in `Update`.
//! register_data_flow::<Targets>(&mut app, Update);
//!
//! app.add_systems(Update, (
//!     produce.in_set(Writes::<Targets>::set()),
//!     consume.in_set(Reads::<Targets>::set()),
//! ));
//!
//! app.update();
//! ```
//!
//! # Why this resolves ambiguities
//!
//! When two systems access the same data and Bevy can't prove an order, it
//! reports an *ambiguity*. By placing the producer in `Writes<T>` and the
//! consumer in `Reads<T>`, the set-level ordering edge gives Bevy a definite
//! order, so the pair is no longer ambiguous — same-frame, deterministically.
//!
//! # Gotcha: only register flows you actually need same-frame
//!
//! [`register_data_flow`] adds two empty system sets and an ordering edge
//! between them. **Empty sets still participate in the schedule's topological
//! sort.** Registering a flow whose two halves never need to run in the same
//! frame adds graph nodes and an ordering constraint for no benefit, and can
//! perturb the tie-break ordering Bevy chooses for *unrelated* systems.
//!
//! Reach for `register_data_flow` only when a consumer genuinely needs the
//! producer's output **within the same frame**. If a one-frame delay is
//! acceptable, prefer leaving the systems unordered (and, if they conflict,
//! declaring the pair `ambiguous_with` each other) instead of forcing an order.
//!
//! # Bevy compatibility
//!
//! | `bevy_ordering_sets` | Bevy |
//! |----------------------|------|
//! | 0.1                  | 0.18 |

use std::{
    fmt,
    hash::{Hash, Hasher},
    marker::PhantomData,
};

use bevy::{ecs::schedule::ScheduleLabel, prelude::*};

/// System set for systems that **write** data of type `T`.
///
/// Ordering: `Writes::<T>` always runs before `Reads::<T>` in any schedule
/// where both are registered via [`register_data_flow`].
pub struct Writes<T: Send + Sync + 'static>(PhantomData<T>);

/// System set for systems that **read** data of type `T`.
///
/// Ordering: `Reads::<T>` always runs after `Writes::<T>` in any schedule
/// where both are registered via [`register_data_flow`].
pub struct Reads<T: Send + Sync + 'static>(PhantomData<T>);

// --- Manual trait impls to avoid derive-imposed bounds on T ---

impl<T: Send + Sync + 'static> Clone for Writes<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: Send + Sync + 'static> Copy for Writes<T> {}

impl<T: Send + Sync + 'static> fmt::Debug for Writes<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Writes")
            .field("type", &std::any::type_name::<T>())
            .finish()
    }
}

impl<T: Send + Sync + 'static> PartialEq for Writes<T> {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}
impl<T: Send + Sync + 'static> Eq for Writes<T> {}

impl<T: Send + Sync + 'static> Hash for Writes<T> {
    fn hash<H: Hasher>(&self, _state: &mut H) {}
}

impl<T: Send + Sync + 'static> SystemSet for Writes<T> {
    fn dyn_clone(&self) -> Box<dyn SystemSet> {
        Box::new(*self)
    }
}

impl<T: Send + Sync + 'static> Clone for Reads<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: Send + Sync + 'static> Copy for Reads<T> {}

impl<T: Send + Sync + 'static> fmt::Debug for Reads<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Reads")
            .field("type", &std::any::type_name::<T>())
            .finish()
    }
}

impl<T: Send + Sync + 'static> PartialEq for Reads<T> {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}
impl<T: Send + Sync + 'static> Eq for Reads<T> {}

impl<T: Send + Sync + 'static> Hash for Reads<T> {
    fn hash<H: Hasher>(&self, _state: &mut H) {}
}

impl<T: Send + Sync + 'static> SystemSet for Reads<T> {
    fn dyn_clone(&self) -> Box<dyn SystemSet> {
        Box::new(*self)
    }
}

// --- Constructors ---

impl<T: Send + Sync + 'static> Writes<T> {
    /// The `Writes<T>` system set. Add a producer of `T` to it via
    /// `.in_set(Writes::<T>::set())`.
    #[must_use]
    pub fn set() -> Self {
        Self(PhantomData)
    }
}

impl<T: Send + Sync + 'static> Reads<T> {
    /// The `Reads<T>` system set. Add a consumer of `T` to it via
    /// `.in_set(Reads::<T>::set())`.
    #[must_use]
    pub fn set() -> Self {
        Self(PhantomData)
    }
}

/// Register `Writes::<T>` → `Reads::<T>` ordering in `schedule`.
///
/// Call once per `(T, schedule)` pair in the plugin that owns the data flow.
///
/// See the [crate-level gotcha](crate#gotcha-only-register-flows-you-actually-need-same-frame):
/// this adds empty sets that participate in toposort, so only register flows
/// whose consumer genuinely needs the producer's output the same frame.
pub fn register_data_flow<T: Send + Sync + 'static>(app: &mut App, schedule: impl ScheduleLabel) {
    app.configure_sets(schedule, Writes::<T>::set().before(Reads::<T>::set()));
}

#[cfg(test)]
mod tests {
    use bevy::ecs::schedule::{LogLevel, ScheduleBuildSettings};

    use super::*;

    // The flowing data is its own marker — the idiomatic usage.
    #[derive(Resource, Default)]
    struct RunLog(Vec<&'static str>);

    fn producer(mut log: ResMut<RunLog>) {
        log.0.push("write");
    }

    fn consumer(mut log: ResMut<RunLog>) {
        log.0.push("read");
    }

    /// A system in `Writes::<T>` runs before a system in `Reads::<T>`.
    #[test]
    fn writer_runs_before_reader() {
        let mut app = App::new();
        app.init_resource::<RunLog>();
        register_data_flow::<RunLog>(&mut app, Update);
        app.add_systems(
            Update,
            (
                producer.in_set(Writes::<RunLog>::set()),
                consumer.in_set(Reads::<RunLog>::set()),
            ),
        );

        app.update();

        assert_eq!(app.world().resource::<RunLog>().0, vec!["write", "read"]);
    }

    #[derive(Resource, Default)]
    struct Shared(u32);

    fn shared_writer(mut s: ResMut<Shared>) {
        s.0 += 1;
    }

    fn shared_reader(s: Res<Shared>) {
        let _ = s.0;
    }

    /// With the pair registered, the shared-resource conflict has a definite
    /// order, so ambiguity detection (set to error) does not fire.
    #[test]
    fn registered_pair_is_not_ambiguous() {
        let mut app = App::new();
        app.init_resource::<Shared>();
        app.edit_schedule(Update, |schedule| {
            schedule.set_build_settings(ScheduleBuildSettings {
                ambiguity_detection: LogLevel::Error,
                ..default()
            });
        });
        register_data_flow::<Shared>(&mut app, Update);
        app.add_systems(
            Update,
            (
                shared_writer.in_set(Writes::<Shared>::set()),
                shared_reader.in_set(Reads::<Shared>::set()),
            ),
        );

        // Must not panic: the Writes→Reads edge resolves the conflict.
        app.update();
    }

    /// Meta-test: the same pair *without* `register_data_flow` is genuinely
    /// ambiguous — proving the previous test passes because of the ordering,
    /// not because the conflict was never there.
    #[test]
    #[should_panic(expected = "conflicting data access have indeterminate execution order")]
    fn unregistered_pair_is_ambiguous() {
        let mut app = App::new();
        app.init_resource::<Shared>();
        app.edit_schedule(Update, |schedule| {
            schedule.set_build_settings(ScheduleBuildSettings {
                ambiguity_detection: LogLevel::Error,
                ..default()
            });
        });
        app.add_systems(Update, (shared_writer, shared_reader));

        app.update();
    }
}
