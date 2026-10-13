# Performance benchmarks

These benchmarks establish local baselines for Spline's metadata, exporters, and
clients. Run commands from the repository root with the pinned Rust/Node versions.
Rust measurements use Criterion in Cargo's optimized bench profile. Criterion is
a development dependency of this benchmark package only.

## What is measured

| Benchmark | Timed work | Setup outside timing |
| --- | --- | --- |
| `schema/cold_post` | Register the blog `Post` and shared `User`, finish/drop the graph. | None; a fresh registry is constructed each iteration. |
| `schema/cold_recursive_tree` | Register and finish/drop a recursive definition graph. | None. This measures schema registration, not traversal of tree values. |
| `schema/cached_post` | Register an already known root in a populated registry. | Initial registration. The registry's transactional cloning/checking is still measured. |
| `metadata/blog` | Describe the real blog application: two resources, four operations. | None. |
| `metadata/shared_dtos/{1,10,100}` | Build synthetic applications with increasing operation counts and shared DTOs. | None. This isolates operation-count scaling, not schema-count scaling. |
| `typescript/export_http/{blog,100_resources}` | Render types, plans, and the fetch runtime into one source string. | Contract description and exporter configuration; no filesystem writes. |
| `http/loopback_get_post/{spline,reqwest_serde}/{128,16384}` | One sequential GET, server response serialization, HTTP, and typed response decoding. | Router/client construction, Tokio startup, and initial connection establishment. |
| Node `spline_{compatible,strict}/{128,16384}` | Generated TS request construction, a fresh synthetic fetch response, parsing, validation/projection. | Generated module loading, client construction, fixture serialization. |
| Node `response_json/{128,16384}` | Fresh Response construction and JSON parsing. | Fixture serialization. |

HTTP and Node size labels are ASCII bytes in `Post.body`. Criterion's HTTP byte
throughput uses the complete serialized JSON size. The native HTTP fixture uses
the blog contracts with a fixed read-only service on an ephemeral loopback port,
two Tokio workers, no TLS/authentication, and a reused reqwest connection pool.
The fixture's post cloning and server encoding are part of the measurement.

The reqwest comparison talks to the **same Spline server** and decodes the same
Rust DTO, but does not perform Spline response-schema checks. It provides context
for the additional client work; the difference is not a pure isolated overhead
measurement because network scheduling and allocation also contribute.

The Node fixture injects fetch and performs **no network I/O**. Its timings cannot
be compared directly with native loopback timings. Both generated-client cases
use valid, exact-schema responses; compatible mode does not encounter extra fields
in this fixture. These are sequential latency/processing measurements, not a
concurrent server capacity test.

## Run

```sh
cargo bench --locked -p spline-benchmarks --bench metadata
cargo bench --locked -p spline-benchmarks --bench http
npm run bench:typescript
```

For a quick exploratory run:

```sh
cargo bench --locked -p spline-benchmarks -- --quick
npm run bench:typescript -- --quick
```

Criterion stores measurements under `target/criterion` (or your configured Cargo
target directory). Node prints microseconds per call: the median and p95 of
**batch-mean** timings, not the p95 latency of individual requests. Warm-up is
excluded, results are consumed, and fixtures are checked before timing. Quick
mode reduces sampling and should be treated as a smoke measurement.

## Compare a change

Record the commit, dirty-working-tree changes, CPU/OS, toolchain versions, and
command. Use the same machine, toolchain, fixtures, and build settings. Avoid
concurrent builds or other heavy work while measuring.

```sh
rustc --version
node --version
git rev-parse HEAD
git status --short

cargo bench --locked -p spline-benchmarks -- --save-baseline before
# Apply the change, then compare using the same commands and environment.
cargo bench --locked -p spline-benchmarks -- --baseline before
```

Use the normal sample sizes for comparisons, inspect distributions, and reproduce
an apparent change before optimizing. Node prints its runtime/platform/CPU and
sample configuration alongside results. Report measurements with that context;
these fixtures do not establish performance guarantees for other applications.

## CI and future measurements

CI validates the fixtures without asserting timing thresholds:

```sh
cargo bench --locked -p spline-benchmarks -- --test
npm run bench:typescript -- --check
```

The metadata benchmark checks the real blog artifact for freshness; the HTTP
benchmark verifies decoded response sizes and shuts down its server. Performance
numbers should be collected on a controlled machine rather than used as pass/fail
thresholds on shared CI runners.

Useful future additions include schema-count scaling, compile-time measurements,
allocation profiling, map/union-heavy response projection, TypeScript over real
HTTP, and concurrent throughput. Add them when a concrete performance question
needs an answer, with the timed boundary documented here.
