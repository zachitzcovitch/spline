// A repository-level smoke test exercises the three crates as real consumers.
// Cargo's JSON artifacts locate the binaries even with a custom CARGO_TARGET_DIR.
import assert from "node:assert/strict";
import { execFile, spawn } from "node:child_process";
import { once } from "node:events";
import { createInterface } from "node:readline";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";

const execute = promisify(execFile);
const root = fileURLToPath(new URL("../../../", import.meta.url));
const typescript = fileURLToPath(new URL("../typescript/run.mjs", import.meta.url));

test("the blog server and both README clients work across package boundaries", { timeout: 600_000 }, async t => {
  const build = await execute(process.env.CARGO ?? "cargo", [
    "build", "--locked", "--message-format=json",
    "-p", "spline-example-blog-server",
    "-p", "spline-example-blog-client",
  ], { cwd: root, signal: t.signal, maxBuffer: 8 * 1024 * 1024 });
  const binaries = new Map();
  for (const line of build.stdout.split("\n").filter(Boolean)) {
    const artifact = JSON.parse(line);
    if (artifact.reason === "compiler-artifact" && artifact.target.kind.includes("bin") && artifact.executable) {
      binaries.set(artifact.target.name, artifact.executable);
    }
  }
  assert.ok(binaries.has("blog-server"), "Cargo must build the server binary");
  assert.ok(binaries.has("blog-client"), "Cargo must build the Rust client binary");

  const server = spawn(binaries.get("blog-server"), ["127.0.0.1:0"], {
    cwd: root, stdio: ["ignore", "pipe", "pipe"], signal: t.signal, killSignal: "SIGKILL",
  });
  let stderr = "";
  server.stderr.setEncoding("utf8").on("data", chunk => { stderr += chunk; });
  // Keep the error listener installed throughout the test, including cancellation.
  const stopped = new Promise(resolve => {
    server.once("error", resolve);
    server.once("close", (code, signal) => resolve({ code, signal }));
  });
  const lines = createInterface({ input: server.stdout });
  try {
    const [ready] = await Promise.race([
      once(lines, "line", { signal: AbortSignal.any([t.signal, AbortSignal.timeout(10_000)]) }),
      stopped.then(status => { throw new Error(`Server stopped before becoming ready: ${JSON.stringify(status)}\n${stderr}`); }),
    ]);
    assert.ok(ready.startsWith("Blog API: "), ready);
    const base = ready.slice("Blog API: ".length);
    const request = (path, options = {}) => fetch(`${base}${path}`, {
      ...options, signal: AbortSignal.any([t.signal, AbortSignal.timeout(10_000)]),
    });

    // The shared authentication factory protects both resource groups.
    for (const path of ["/users/alice", "/posts/post_1"]) {
      const response = await request(path);
      assert.equal(response.status, 401);
      const error = await response.json();
      assert.equal(error.kind, "framework");
      assert.equal(error.code, "unauthorized");
    }

    const options = { cwd: root, signal: t.signal, timeout: 30_000 };
    const rust = await execute(binaries.get("blog-client"), [base], options);
    const node = await execute(process.execPath, [typescript, base], options);
    for (const [output, id] of [[rust.stdout, "post_1"], [node.stdout, "post_2"]]) {
      assert.ok(output.includes("User: Alice"), output);
      assert.ok(output.includes("Bob cannot publish Alice's post (typed Forbidden)"), output);
      assert.ok(output.includes(`Published: ${id} (published=true)`), output);

      // Verify persisted effects independently of the clients' logs.
      const response = await request(`/posts/${id}`, { headers: { authorization: "Bearer bob-token" } });
      assert.equal(response.status, 200);
      const post = await response.json();
      assert.equal(post.author.id, "alice");
      assert.equal(post.title, "Hello from Spline");
      assert.equal(post.body, "One Rust contract, two typed clients.");
      assert.equal(post.published, true);
    }

    const response = await request("/posts", {
      method: "POST",
      headers: { authorization: "Bearer alice-token", "content-type": "application/json" },
      body: JSON.stringify({ title: "  ", body: "A title is required" }),
    });
    assert.equal(response.status, 422);
    assert.equal(response.headers.get("x-spline-error"), "application");
    assert.deepEqual(await response.json(), { kind: "application", value: { kind: "invalid_title" } });

    // The derived defaults preserve the same status/envelope across both resources.
    for (const [path, method, status, kind] of [
      ["/users/missing", "GET", 404, "not_found"],
      ["/posts/missing", "GET", 404, "not_found"],
      ["/posts/post_1/publish", "POST", 403, "forbidden"],
    ]) {
      const response = await request(path, { method, headers: { authorization: "Bearer bob-token" } });
      assert.equal(response.status, status);
      assert.equal(response.headers.get("x-spline-error"), "application");
      assert.deepEqual(await response.json(), { kind: "application", value: { kind } });
    }
  } finally {
    lines.close();
    server.kill("SIGKILL");
    await stopped;
  }
});
