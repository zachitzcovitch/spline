import assert from "node:assert/strict";
import { createEvolutionV1Client } from "./evolution-v1.generated.ts";

const [modernBase, originalBase, requiredBase] = process.argv.slice(2);
const client = createEvolutionV1Client(modernBase);
const strict = createEvolutionV1Client(modernBase, { responsePolicy: "strict" });

const raw = await fetch(`${modernBase}/evolution/users/existing`).then(r => r.json());
assert.equal(raw.revision, 2);
assert.equal(raw.profile.locale, "en");

const response = await client.get("existing");
assert.equal(response.ok, true);
assert.equal(response.value.id, "user_1");
assert.equal(response.value.state, "active");
assert.deepEqual(response.value.profile, { displayName: "New" });
assert.deepEqual(response.value.history, [{ displayName: "New" }]);
assert.deepEqual(response.value.profiles.added_key, { displayName: "New" });
assert.equal(response.value.counters.added_key, 9);
assert.equal(response.value.counters.audit, 0);
assert.equal(Object.hasOwn(response.value, "revision"), false);
assert.equal((await strict.get("existing")).error.kind, "protocol");

const created = await client.create({ displayName: "Created by v1", state: "inactive" });
assert.equal(created.ok, true);
assert.equal(created.value.profile.displayName, "Created by v1");
assert.equal(created.value.counters.tags, 0);
assert.equal(created.value.state, "inactive");

assert.deepEqual(await client.get("existing-error"), {
  ok: false, error: { kind: "application", value: { kind: "missing", details: { message: "missing" } } },
});
assert.equal((await strict.get("existing-error")).error.kind, "protocol");
assert.deepEqual(await client.event("known"), { ok: true, value: { item: { id: "known" } } });
assert.equal((await strict.event("known")).error.kind, "protocol");

for (const id of ["new-state", "new-error"]) {
  assert.equal((await client.get(id)).error.kind, "protocol");
}
for (const id of ["new-variant", "ambiguous"]) {
  assert.equal((await client.event(id)).error.kind, "protocol");
}

for (const cause of ["missing", "type", "null", "array", "map", "range"]) {
  const fixture = await fetch(`${modernBase}/evolution/users/bad/${cause}`).then(r => r.json());
  const bad = createEvolutionV1Client(modernBase, {
    fetch: async () => new Response(JSON.stringify(fixture), { headers: { "content-type": "application/json" } }),
  });
  assert.equal((await bad.get("test")).error.kind, "protocol", cause);
}

// A dictionary entry is data, not an unknown DTO field. Keep its key while
// projecting the nested profile. fromEntries must create __proto__ as data.
const special = JSON.parse(JSON.stringify(raw));
special.profiles = JSON.parse('{"__proto__":{"displayName":"safe","locale":"new"}}');
const specialClient = createEvolutionV1Client(modernBase, {
  fetch: async () => new Response(JSON.stringify(special), { headers: { "content-type": "application/json" } }),
});
const projected = await specialClient.get("special");
assert.equal(projected.ok, true);
assert.equal(Object.hasOwn(projected.value.profiles, "__proto__"), true);
assert.deepEqual(projected.value.profiles["__proto__"], { displayName: "safe" });
assert.equal(Object.getPrototypeOf(projected.value.profiles), Object.prototype);

const original = createEvolutionV1Client(originalBase, { responsePolicy: "strict" });
assert.equal((await original.get("existing")).ok, true);
const required = createEvolutionV1Client(requiredBase);
const rejected = await required.create({ displayName: "v1", state: "active" });
assert.equal(rejected.error.kind, "framework");
assert.equal(rejected.error.status, 400);

let requests = 0;
const guarded = createEvolutionV1Client(modernBase, {
  fetch: async (...args) => { requests++; return fetch(...args); },
});
assert.equal((await guarded.create({ displayName: "x", state: "active", typo: true })).error.kind, "encode");
assert.equal((await guarded.create({ displayName: "x", state: "suspended" })).error.kind, "encode");
assert.equal(requests, 0);
const unknownRequest = await fetch(`${modernBase}/evolution/users`, {
  method: "POST", headers: { "content-type": "application/json" },
  body: JSON.stringify({ displayName: "x", state: "active", typo: true }),
});
assert.equal(unknownRequest.status, 400);
assert.throws(() => createEvolutionV1Client(modernBase, { responsePolicy: "anything" }), TypeError);
console.log("Old-contract TypeScript client passed evolution checks against newer and original servers");
