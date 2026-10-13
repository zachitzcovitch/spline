# Schema and registry prototype — promoted

The implementation and its 20 integration tests now live in
[`crates/spline-schema`](../../crates/spline-schema).
The [research design note](../schema-registry.md) preserves the experiment's
findings and API evolution.

From the repository root:

```sh
cargo test --locked -p spline-schema
cargo tree --locked -p spline-schema --edges normal
```

See the [workspace guide](../../docs/workspace-contracts.md) for the current crate
boundaries and the complete verification commands.
