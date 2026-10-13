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
const id = created.value.id;
const replaced = await client.replace(id, { displayName: "Replacement", bio: "original bio" });
assert.equal(replaced.ok, true);
assert.deepEqual(await client.replace(id, { displayName: "Replacement", bio: "original bio" }), replaced);
assert.deepEqual(await client.update(id, {}), replaced);
const renamed = await client.update(id, { displayName: "Partial" });
assert.equal(renamed.value.bio, "original bio");
const updatedBio = await client.update(id, { bio: "new bio" });
assert.equal(updatedBio.value.displayName, "Partial");
assert.equal(updatedBio.value.bio, "new bio");
const clearedBio = await client.update(id, { bio: null });
assert.equal(clearedBio.value.displayName, "Partial");
assert.equal(clearedBio.value.bio, null);
assert.equal((await client.update(id, { displayName: null })).error.kind, "encode");
assert.equal((await client.update(id, { typo: true })).error.kind, "encode");
assert.deepEqual(await client.delete(id), { ok: true, value: undefined });
assert.equal((await client.get(id)).error.kind, "application");
assert.equal((await client.delete(id)).error.kind, "application");
assert.deepEqual(await client.get("missing"), {
  ok: false, error: { kind: "application", value: { kind: "not_found" } },
});
const invalid = await client.create({ displayName: " " });
assert.equal(invalid.error.kind, "application");
assert.equal(invalid.error.value.kind, "invalid_name");

const echo = createEchoClient(base);
assert.deepEqual(await echo.health(), { ok: true, value: null });
assert.deepEqual(await echo.no_content(), { ok: true, value: undefined });
assert.deepEqual(await echo.command(false), { ok: true, value: undefined });
assert.deepEqual(await echo.command(true), { ok: false, error: { kind: "application", value: null } });
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

// WHATWG Response forbids constructing a real 204 with a body. A fetch test
// double exercises an observable malformed body; framing errors use real headers.
const empty = createUsersClient(base, { fetch: async () => new Response(null, { status: 204 }) });
assert.deepEqual(await empty.delete("x"), { ok: true, value: undefined });
const changedUnitStatus = createEchoClient(base, {
  fetch: async () => new Response(null, { status: 204 }),
});
assert.equal((await changedUnitStatus.health()).error.kind, "protocol");
const emptyWithMetadata = createUsersClient(base, {
  fetch: async () => new Response(null, { status: 204, headers: { "content-type": "application/json" } }),
});
assert.deepEqual(await emptyWithMetadata.delete("x"), { ok: true, value: undefined });
for (const headers of [{ "content-length": "0" }, { "transfer-encoding": "chunked" }, { "x-spline-error": "application" }]) {
  const bad204 = createUsersClient(base, { fetch: async () => new Response(null, { status: 204, headers }) });
  assert.equal((await bad204.delete("x")).error.kind, "protocol");
}
const observableBody = createUsersClient(base, {
  fetch: async () => ({ status: 204, headers: new Headers(), text: async () => "null" }),
});
assert.equal((await observableBody.delete("x")).error.kind, "protocol");
const wrongDeleteStatus = createUsersClient(base, {
  fetch: async () => new Response("null", { status: 200, headers: { "content-type": "application/json" } }),
});
assert.equal((await wrongDeleteStatus.delete("x")).error.kind, "protocol");

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
