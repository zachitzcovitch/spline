// Keep the default facade and execution path independent of HTTP dependencies.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";

function normalPackages(name, extra = []) {
  const output = execFileSync(process.env.CARGO ?? "cargo", [
    "tree", "--locked", "-p", name, ...extra,
    "--edges", "normal", "--prefix", "none", "--format", "{p}",
  ], { encoding: "utf8" });
  return new Set(output.split("\n").map(line => line.split(" ")[0]));
}
const packages = normalPackages("spline", ["--no-default-features"]);
for (const name of ["spline-http", "spline-http-contract", "axum", "reqwest", "hyper", "http"]) {
  assert.equal(packages.has(name), false, `${name} leaked into the neutral dependency graph`);
}
assert.ok(packages.has("spline-runtime"));
assert.ok(packages.has("tower"));
console.log("Neutral dependency graph: typed Tower runtime, no HTTP binding/runtime libraries");

const exporter = normalPackages("spline-openapi");
for (const name of ["spline", "spline-http", "spline-runtime", "spline-typescript", "tower", "axum", "reqwest", "hyper", "http"]) {
  assert.equal(exporter.has(name), false, `${name} leaked into the OpenAPI exporter dependency graph`);
}
assert.ok(exporter.has("spline-http-contract"));
console.log("OpenAPI exporter dependency graph: HTTP metadata, no execution/runtime libraries");
