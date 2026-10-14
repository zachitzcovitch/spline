// Keep the default facade and execution path independent of HTTP dependencies.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";

const output = execFileSync(process.env.CARGO ?? "cargo", [
  "tree", "--locked", "-p", "spline", "--no-default-features",
  "--edges", "normal", "--prefix", "none", "--format", "{p}",
], { encoding: "utf8" });
const packages = new Set(output.split("\n").map(line => line.split(" ")[0]));
for (const name of ["spline-http", "spline-http-contract", "axum", "reqwest", "hyper", "http"]) {
  assert.equal(packages.has(name), false, `${name} leaked into the neutral dependency graph`);
}
assert.ok(packages.has("spline-runtime"));
assert.ok(packages.has("tower"));
console.log("Neutral dependency graph: typed Tower runtime, no HTTP binding/runtime libraries");
