import assert from "node:assert/strict";
import { pathToFileURL } from "node:url";
import { createUsersClient } from "./generated.ts";

const base = process.argv[2];
const { createEchoClient } = await import(pathToFileURL(process.argv[3]).href);
const client = createUsersClient(base);
assert.equal((await client.get("user_1")).value.displayName, "Zach");
const created = await client.create({ displayName: "TypeScript" });
assert.equal(created.ok, true);
assert.deepEqual(await client.get(created.value.id), created);
assert.deepEqual(await client.get("missing"), {
  ok: false, error: { kind: "application", value: { kind: "not_found" } },
});
const invalid = await client.create({ displayName: " " });
assert.equal(invalid.error.kind, "application");
assert.equal(invalid.error.value.kind, "invalid_name");

const echo = createEchoClient(base);
assert.deepEqual(await echo.health(), { ok: true, value: null });
const value = "slash/space ?#%ü";
const note = "query &+?/ü";
assert.deepEqual(await echo.inspect(value, -7, true, note), { ok: true, value: { value, count: -7, flag: true, note } });
assert.equal((await echo.inspect("plain", 0, false, null)).value.note, null);
assert.equal((await echo.inspect("..", 0, false, null)).error.kind, "encode");
assert.equal((await echo.inspect("plain", 2147483648, false, null)).error.kind, "encode");

let requests = 0;
const checked = createUsersClient(base, { fetch: async (...args) => { requests++; return fetch(...args); } });
assert.equal((await checked.get({ id: "wrong wrapper" })).error.kind, "encode");
assert.equal((await checked.create({ displayName: "bad", tags: [1n] })).error.kind, "encode");
assert.equal(requests, 0);

const mock = (body, status = 200, headers = {}) => createUsersClient(base, {
  fetch: async () => new Response(body, { status, headers: { "content-type": "application/json", ...headers } }),
});
for (const bad of [
  mock("{}"),
  mock("{"),
  mock(JSON.stringify({ id: "x", displayName: "x" }), 201),
  mock(JSON.stringify({ kind: "application", value: 3 }), 404, { "x-spline-error": "application" }),
  mock(JSON.stringify({ kind: "application" }), 404, { "x-spline-error": "application" }),
  mock(JSON.stringify({ kind: "framework", code: "bad" }), 400, { "x-spline-error": "framework" }),
]) {
  assert.equal((await bad.get("x")).error.kind, "protocol");
}
const unavailable = mock("proxy error", 503, { "content-type": "text/html" });
assert.deepEqual(await unavailable.get("x"), { ok: false, error: { kind: "http", status: 503 } });
const framework = mock(JSON.stringify({ kind: "framework", code: "unauthorized", message: "token required" }),
  401, { "x-spline-error": "framework" });
assert.deepEqual(await framework.get("x"), {
  ok: false, error: { kind: "framework", status: 401, code: "unauthorized", message: "token required" },
});
const transport = createUsersClient(base, { fetch: async () => { throw new TypeError("network unavailable"); } });
assert.equal((await transport.get("x")).error.kind, "transport");

let body;
const configured = createUsersClient(base, {
  headers: { "x-example": "configured" },
  fetch: async (url, init) => {
    assert.equal(init.headers.get("x-example"), "configured");
    body = JSON.parse(init.body);
    return fetch(url, init);
  },
});
assert.equal((await configured.create({ displayName: "Body mapping" })).ok, true);
assert.deepEqual(body, { displayName: "Body mapping" });
for (const invalid of ["relative", "ftp://example.test", "http://example.test?", "http://example.test#", "http://user:password@example.test"]) {
  assert.throws(() => createUsersClient(invalid), TypeError);
}
console.log("Generated TypeScript HTTP conformance passed");
