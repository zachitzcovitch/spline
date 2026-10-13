// Isolated generated-client processing: fresh Response bodies, no network/server.
import assert from "node:assert/strict";
import { cpus } from "node:os";
import { performance } from "node:perf_hooks";
import { createBlogClient } from "../examples/blog/typescript/generated.ts";

const args = process.argv.slice(2);
if (args.length > 1 || (args.length === 1 && !["--quick", "--check"].includes(args[0]))) {
  throw new Error("usage: node benchmarks/typescript.mjs [--quick|--check]");
}
const check = args[0] === "--check";
const quick = args[0] === "--quick";
const samples = quick ? 5 : 20;
const iterations = quick ? 100 : 1_000;
let sink = 0;

async function measure(name, call) {
  const batch = async () => {
    for (let i = 0; i < iterations; i++) sink += await call();
  };
  await batch(); // Warm-up is excluded from the reported samples.
  const times = [];
  for (let sample = 0; sample < samples; sample++) {
    const start = performance.now();
    await batch();
    times.push((performance.now() - start) * 1_000 / iterations);
  }
  times.sort((a, b) => a - b);
  console.log(`${name}: median=${times[Math.floor(samples / 2)].toFixed(2)} µs/call, ` +
    `p95 batch-mean=${times[Math.ceil(samples * 0.95) - 1].toFixed(2)} µs/call`);
}

console.log(`Node ${process.version}; ${process.platform}/${process.arch}; ${cpus()[0]?.model}`);
console.log(check ? "Checking benchmark fixtures" : `${samples} samples × ${iterations} calls; isolated fetch responses`);

for (const bodyBytes of [128, 16 * 1024]) {
  const post = {
    id: "post_1", author: { id: "alice", displayName: "Alice" },
    title: "Benchmark post", body: "x".repeat(bodyBytes), published: true,
  };
  const body = JSON.stringify(post);
  const fetcher = async () => new Response(body, { headers: { "content-type": "application/json" } });
  const compatible = createBlogClient("http://benchmark.test", { fetch: fetcher });
  const strict = createBlogClient("http://benchmark.test", { fetch: fetcher, responsePolicy: "strict" });
  const calls = [
    ["response_json", async () => (await (await fetcher()).json()).body.length],
    ...[["spline_compatible", compatible], ["spline_strict", strict]].map(([name, client]) => [
      name, async () => {
        const result = await client.posts.get("post_1");
        if (!result.ok) throw new Error(JSON.stringify(result.error));
        return result.value.body.length;
      },
    ]),
  ];
  for (const [name, call] of calls) {
    assert.equal(await call(), bodyBytes);
    if (!check) await measure(`${name}/${bodyBytes}`, call);
  }
}
assert.ok(Number.isSafeInteger(sink));
console.log(check ? "Benchmark fixtures passed" : `Consumed body bytes: ${sink}`);
