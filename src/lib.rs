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
//! # Registry (feature `registry`, off by default)
//!
//! With the **`registry`** cargo feature enabled, every `register_data_flow`
//! call is recorded in a `DataFlowRegistry` resource, so schedule-introspection
//! tooling can enumerate the declared flows — each entry names the flowing type,
//! its `Writes<T>` / `Reads<T>` set identities, and the schedule — and check
//! that every system participating in a flow's data actually joined the right
//! set. The feature is **off by default**: without it, `register_data_flow` only
//! wires the ordering edge (the crate's original behaviour) and adds no
//! per-registration bookkeeping. Enable it in the consumer that needs the audit
//! (`bevy_ordering_sets = { version = "0.4", features = ["registry"] }`).
//!
//! # Bevy compatibility
//!
//! | `bevy_ordering_sets` | Bevy |
//! |----------------------|------|
//! | 0.1                  | 0.18 |
//! | 0.2, 0.3             | 0.19 |
//! | 0.4                  | 0.20 |

use std::{
    fmt,
    hash::{Hash, Hasher},
    marker::PhantomData,
};

#[cfg(feature = "registry")]
use std::any::{TypeId, type_name};

#[cfg(feature = "registry")]
use bevy::ecs::schedule::{InternedScheduleLabel, InternedSystemSet};
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
/// With the `registry` feature enabled, also records the registration in the
/// app's `DataFlowRegistry` (creating it on first use), so tooling can enumerate
/// every declared flow. Without the feature (the default) this only wires the
/// ordering edge — the recording is the sole thing the feature adds; the
/// `Writes<T>` → `Reads<T>` ordering is identical either way.
///
/// See the [crate-level gotcha](crate#gotcha-only-register-flows-you-actually-need-same-frame):
/// this adds empty sets that participate in toposort, so only register flows
/// whose consumer genuinely needs the producer's output the same frame.
pub fn register_data_flow<T: Send + Sync + 'static>(app: &mut App, schedule: impl ScheduleLabel) {
    #[cfg(feature = "registry")]
    let schedule = schedule.intern();
    app.configure_sets(schedule, Writes::<T>::set().before(Reads::<T>::set()));
    #[cfg(feature = "registry")]
    app.world_mut()
        .get_resource_or_init::<DataFlowRegistry>()
        .registrations
        .push(DataFlowRegistration {
            data_type_name: type_name::<T>(),
            data_type_id: TypeId::of::<T>(),
            writes: Writes::<T>::set().intern(),
            reads: Reads::<T>::set().intern(),
            schedule,
        });
}

/// Enumerable registry of every [`register_data_flow`] registration in an
/// [`App`].
///
/// One [`DataFlowRegistration`] is recorded per `register_data_flow::<T>` call.
/// The resource is created on the first registration and is otherwise inert —
/// it holds no schedule state, only a description of the declared flows for
/// introspection tooling (e.g. an audit that every writer of `T` joined
/// `Writes<T>` and every reader joined `Reads<T>`).
///
/// Present only with the `registry` feature enabled.
#[cfg(feature = "registry")]
#[derive(Resource, Default)]
pub struct DataFlowRegistry {
    registrations: Vec<DataFlowRegistration>,
}

#[cfg(feature = "registry")]
impl DataFlowRegistry {
    /// Iterates every recorded registration, in the order they were registered.
    pub fn iter(&self) -> impl Iterator<Item = &DataFlowRegistration> {
        self.registrations.iter()
    }
}

/// A single [`register_data_flow`] registration: the flowing data type together
/// with the set identities and schedule the flow was wired into.
///
/// The `writes` / `reads` [`InternedSystemSet`]s are the exact `Writes<T>` /
/// `Reads<T>` set instances participating systems join, so tooling can look up
/// each set's members in a built schedule (e.g. via
/// `Schedule::systems_in_set`). `data_type_id` is `TypeId::of::<T>()`, which
/// resolves the flow's underlying resource / component id in a world.
///
/// Present only with the `registry` feature enabled.
#[cfg(feature = "registry")]
#[derive(Debug, Clone)]
pub struct DataFlowRegistration {
    /// `core::any::type_name::<T>()` of the flowing data type — for diagnostics
    /// and, for message flows, resolving the `Messages<T>` storage by name.
    pub data_type_name: &'static str,
    /// `TypeId::of::<T>()` of the flowing data type.
    pub data_type_id: TypeId,
    /// The `Writes<T>` set that producers of the flow join.
    pub writes: InternedSystemSet,
    /// The `Reads<T>` set that consumers of the flow join.
    pub reads: InternedSystemSet,
    /// The schedule the flow was registered in.
    pub schedule: InternedScheduleLabel,
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

    // Marker-only resources: distinct flow types whose values are never read.
    #[cfg(feature = "registry")]
    #[derive(Resource, Default)]
    struct Alpha;
    #[cfg(feature = "registry")]
    #[derive(Resource, Default)]
    struct Beta;

    /// The registry resource does not exist until the first `register_data_flow`
    /// call creates it.
    #[cfg(feature = "registry")]
    #[test]
    fn registry_absent_until_first_registration() {
        let mut app = App::new();
        assert!(app.world().get_resource::<DataFlowRegistry>().is_none());

        register_data_flow::<Alpha>(&mut app, Update);

        assert!(app.world().get_resource::<DataFlowRegistry>().is_some());
    }

    /// Each `register_data_flow::<T>` records one entry naming `T`, the exact
    /// `Writes<T>` / `Reads<T>` set identities, and the schedule.
    #[cfg(feature = "registry")]
    #[test]
    fn registrations_are_recorded_with_set_and_schedule_identity() {
        let mut app = App::new();
        register_data_flow::<Alpha>(&mut app, Update);
        register_data_flow::<Beta>(&mut app, PostUpdate);

        let registry = app.world().resource::<DataFlowRegistry>();
        let entries: Vec<&DataFlowRegistration> = registry.iter().collect();
        assert_eq!(entries.len(), 2, "one entry per register_data_flow call");

        let alpha = entries[0];
        assert_eq!(alpha.data_type_id, TypeId::of::<Alpha>());
        assert_eq!(alpha.data_type_name, type_name::<Alpha>());
        assert_eq!(alpha.writes, Writes::<Alpha>::set().intern());
        assert_eq!(alpha.reads, Reads::<Alpha>::set().intern());
        assert_eq!(alpha.schedule, Update.intern());

        let beta = entries[1];
        assert_eq!(beta.data_type_id, TypeId::of::<Beta>());
        assert_eq!(beta.writes, Writes::<Beta>::set().intern());
        assert_eq!(beta.schedule, PostUpdate.intern());

        // Distinct flows carry distinct set identities.
        assert_ne!(alpha.writes, beta.writes);
        assert_ne!(alpha.writes, alpha.reads);
    }
}
