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
const packages = normalPackages("splinekit", ["--no-default-features"]);
for (const name of ["splinekit-http", "splinekit-http-contract", "axum", "reqwest", "hyper", "http"]) {
  assert.equal(packages.has(name), false, `${name} leaked into the neutral dependency graph`);
}
assert.ok(packages.has("splinekit-runtime"));
assert.ok(packages.has("tower"));
console.log("Neutral dependency graph: typed Tower runtime, no HTTP binding/runtime libraries");

// The HTTP binding splits by side: a client-only build (including wasm32) has
// no server framework, and a server-only build has no HTTP client.
const client = normalPackages("splinekit", ["--no-default-features", "--features", "http-client"]);
for (const name of ["axum"]) {
  assert.equal(client.has(name), false, `${name} leaked into the client-only dependency graph`);
}
assert.ok(client.has("reqwest"));
const server = normalPackages("splinekit", ["--no-default-features", "--features", "http-server"]);
assert.equal(server.has("reqwest"), false, "reqwest leaked into the server-only dependency graph");
assert.ok(server.has("axum"));
console.log("HTTP client and server dependency graphs stay separate");

const exporter = normalPackages("splinekit-openapi");
for (const name of ["splinekit", "splinekit-http", "splinekit-runtime", "splinekit-typescript", "tower", "axum", "reqwest", "hyper", "http"]) {
  assert.equal(exporter.has(name), false, `${name} leaked into the OpenAPI exporter dependency graph`);
}
assert.ok(exporter.has("splinekit-http-contract"));
console.log("OpenAPI exporter dependency graph: HTTP metadata, no execution/runtime libraries");
