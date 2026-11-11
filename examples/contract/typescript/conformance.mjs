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
assert.deepEqual(await echo.noContent(), { ok: true, value: undefined });
assert.deepEqual(await echo.command(false), { ok: true, value: undefined });
assert.deepEqual(await echo.command(true), { ok: false, error: { kind: "application", value: null } });
const value = "slash/space ?#%ü";
const note = "query &+?/ü";
assert.deepEqual(await echo.inspect(value, -7, true, note), { ok: true, value: { value, count: -7, flag: true, note } });
assert.equal((await echo.inspect("plain", 0, false, null)).value.note, null);
assert.equal((await echo.inspect("..", 0, false, null)).error.kind, "encode");
assert.equal((await echo.inspect("plain", 2147483648, false, null)).error.kind, "encode");

const formats = {
  id: "0191f1b2-7c4e-7a3b-9f1e-2a3b4c5d6e7f", at: "2026-11-11T05:06:53.123Z", day: "2026-11-11",
  link: "https://example.test/a?b=c", amount: "-12.50", big: "18446744073709551615",
  safe: -9007199254740991, ratio: 0.25, total: "18446744073709551615", limit: null, status: "past_due",
};
assert.deepEqual(await echo.formats(formats), { ok: true, value: formats });
for (const [field, value] of [
  ["at", "2026-11-11T05:06:53.123456789Z"], ["at", "2016-12-31T23:59:60+05:30"], ["day", "2024-02-29"],
  ["total", "0"], ["limit", 9007199254740991],
  // An open enum sends and receives values this client doesn't know.
  ["status", "paused"],
]) {
  assert.equal((await echo.formats({ ...formats, [field]: value })).ok, true, `${field}=${value}`);
}
for (const [field, value] of [
  ["id", "0191f1b2"], ["at", "2026-11-11 05:06"], ["at", "2026-02-30T00:00:00Z"], ["at", "2026-11-11T24:00:00Z"],
  ["at", "2026-11-11T05:06:53+24:00"], ["day", "2026-02-30"], ["day", "2026-02-29"], ["day", "2026-13-01"],
  ["link", "relative"],
  ["amount", "1e5"], ["amount", "01.5"], ["big", "18446744073709551616"], ["big", "-1"],
  ["safe", 9007199254740992], ["ratio", Infinity],
  ["total", "007"], ["total", "+7"], ["total", 7], ["total", "18446744073709551616"], ["limit", 9007199254740992],
  ["status", 7], ["status", null],
]) {
  assert.equal((await echo.formats({ ...formats, [field]: value })).error.kind, "encode", `${field}=${value}`);
}
const answering = (patch) => createEchoClient(base, {
  fetch: async () => new Response(JSON.stringify({ ...formats, ...patch }), {
    status: 200, headers: { "content-type": "application/json" },
  }),
});
for (const patch of [
  { id: "x" }, { big: "99999999999999999999" }, { safe: 2 ** 53 }, { ratio: "0.25" }, { total: "-1" }, { limit: "7" },
  { status: 7 },
]) {
  assert.equal((await answering(patch).formats(formats)).error.kind, "protocol", JSON.stringify(patch));
}
assert.equal((await answering({ status: "suspended" }).formats(formats)).value.status, "suspended");

// TypeScript uses camelCase names of the Rust fields; the wire keeps Serde's.
const named = {
  displayName: "Ada", kind: "admin", nestedItems: [{ itemCount: 1 }],
  byKey: { snake_key: { itemCount: 2 } }, maybeValue: null,
};
let sentBody;
const spying = createEchoClient(base, {
  fetch: async (url, init) => { sentBody = JSON.parse(init.body); return fetch(url, init); },
});
assert.deepEqual(await spying.named(named), { ok: true, value: named });
assert.deepEqual(sentBody, {
  display_name: "Ada", type: "admin", nested_items: [{ item_count: 1 }],
  by_key: { snake_key: { item_count: 2 } }, maybe_value: null,
});
for (const wrong of [{ ...named, display_name: "wire name" }, { ...named, type: "wire name" }, { ...named, extra: 1 }]) {
  assert.equal((await echo.named(wrong)).error.kind, "encode", JSON.stringify(wrong));
}
const { maybeValue: _optional, ...withoutOptional } = named;
assert.equal((await echo.named(withoutOptional)).ok, true, "an Option input may be omitted");
const { displayName: _required, ...withoutRequired } = named;
assert.equal((await echo.named(withoutRequired)).error.kind, "encode", "a required property stays required");

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
for (const headers of [{ "content-length": "0" }, { "transfer-encoding": "chunked" }, { "x-splinekit-error": "application" }]) {
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
  mock(JSON.stringify({ kind: "application", value: 3 }), 404, { "x-splinekit-error": "application" }),
  mock(JSON.stringify({ kind: "application" }), 404, { "x-splinekit-error": "application" }),
  mock(JSON.stringify({ kind: "framework", code: "bad" }), 400, { "x-splinekit-error": "framework" }),
]) {
  assert.equal((await bad.get("x")).error.kind, "protocol");
}
const unavailable = mock("proxy error", 503, { "content-type": "text/html" });
assert.deepEqual(await unavailable.get("x"), { ok: false, error: { kind: "http", status: 503 } });
const framework = mock(JSON.stringify({ kind: "framework", code: "unauthorized", message: "token required" }),
  401, { "x-splinekit-error": "framework" });
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
