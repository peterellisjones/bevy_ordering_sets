# bevy_ordering_sets

Declarative data-flow system ordering for [Bevy](https://bevyengine.org/).

Instead of hand-writing `.before()` / `.after()` chains, systems declare *what
data they read and write*. Ordering is derived automatically per `(type,
schedule)` pair: every system in `Writes::<T>` runs before every system in
`Reads::<T>`.

- A system that **produces** data of type `T` joins `Writes::<T>::set()`.
- A system that **consumes** data of type `T` joins `Reads::<T>::set()`.
- `register_data_flow::<T>(app, schedule)` wires `Writes::<T>` before
  `Reads::<T>` in that schedule.

`T` names the data flowing from producers to consumers — **use the actual
resource / component / message type itself** (`Writes::<Orders>`,
`Reads::<SensorDetections>`). It is never constructed, so `Writes<T>` /
`Reads<T>` impose no trait bounds on `T` beyond `Send + Sync + 'static`.

## Quick start

```rust
use bevy::prelude::*;
use bevy_ordering_sets::{Reads, Writes, register_data_flow};

// The data that flows from producer to consumer. It is its own marker.
#[derive(Resource, Default)]
struct Targets(Vec<u32>);

fn produce(mut targets: ResMut<Targets>) {
    targets.0.push(42);
}

fn consume(targets: Res<Targets>) {
    // `produce` is guaranteed to have run first this frame.
    assert_eq!(targets.0, vec![42]);
}

fn main() {
    let mut app = App::new();
    app.init_resource::<Targets>();

    // Order Writes::<Targets> before Reads::<Targets> in `Update`.
    register_data_flow::<Targets>(&mut app, Update);

    app.add_systems(Update, (
        produce.in_set(Writes::<Targets>::set()),
        consume.in_set(Reads::<Targets>::set()),
    ));

    app.update();
}
```

A system can join several flows — read one type, write another — and the
ordering composes. For example, a `tasking` system that reads `Detections` and
writes `Orders` runs after detection and before anything that reads orders:

```rust
app.add_systems(Update, tasking
    .in_set(Reads::<Detections>::set())
    .in_set(Writes::<Orders>::set()));
```

## Why this resolves ambiguities

When two systems access the same data and Bevy can't prove an order, it reports
an *ambiguity*. Placing the producer in `Writes<T>` and the consumer in
`Reads<T>` gives Bevy a definite set-level ordering edge, so the pair is no
longer ambiguous — same-frame, deterministically.

## Gotcha: only register flows you actually need same-frame

`register_data_flow` adds two empty system sets and an ordering edge between
them. **Empty sets still participate in the schedule's topological sort.**
Registering a flow whose two halves never need to run in the same frame adds
graph nodes and an ordering constraint for no benefit, and can perturb the
tie-break ordering Bevy chooses for *unrelated* systems.

Reach for `register_data_flow` only when a consumer genuinely needs the
producer's output **within the same frame**. If a one-frame delay is acceptable,
prefer leaving the systems unordered (and, if they conflict, declaring the pair
`ambiguous_with` each other) over forcing an order.

## Feature: `registry` (off by default)

Enable the **`registry`** feature to have every `register_data_flow` call
recorded in a `DataFlowRegistry` resource — one `DataFlowRegistration` per call,
naming the flowing type, its `Writes<T>` / `Reads<T>` set identities, and the
schedule. This lets schedule-introspection tooling enumerate the declared flows
and check that every system touching a flow's data actually joined the right set
— for example, an audit that catches a producer which writes `T` but never
joined `Writes<T>`.

```toml
bevy_ordering_sets = { version = "0.4", features = ["registry"] }
```

The feature is **off by default** so it stays out of builds that don't run such
tooling. It only adds the per-registration bookkeeping — the `Writes<T>` →
`Reads<T>` ordering `register_data_flow` produces is identical with or without
it.

## Bevy compatibility

| `bevy_ordering_sets` | Bevy |
|----------------------|------|
| 0.4                  | 0.20 |
| 0.2, 0.3             | 0.19 |
| 0.1                  | 0.18 |

## AI assistance

This crate was developed with the help of AI coding tools.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
