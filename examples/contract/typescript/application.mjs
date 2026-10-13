import assert from "node:assert/strict";
import { createApiClient } from "./application.generated.ts";

const base = process.argv[2];
const anonymous = createApiClient(base);
for (const result of [await anonymous.users.get("alice"), await anonymous.posts.get("post_1")]) {
  assert.equal(result.ok, false);
  assert.equal(result.error.kind, "framework");
  assert.equal(result.error.status, 401);
}

const seen = [];
const alice = createApiClient(base, {
  headers: { authorization: "Bearer alice" },
  fetch: async (url, init) => {
    assert.equal(init.headers.get("authorization"), "Bearer alice");
    assert.ok(url.pathname.startsWith("/api/"));
    seen.push(url.pathname);
    return fetch(url, init);
  },
});
const bob = createApiClient(base, { headers: { authorization: "Bearer bob" } });
const unwrap = result => { assert.equal(result.ok, true, JSON.stringify(result)); return result.value; };
const error = (result, kind) => {
  assert.equal(result.ok, false);
  assert.equal(result.error.kind, "application");
  assert.equal(result.error.value.kind, kind);
};

const renamed = unwrap(await alice.users.rename("TypeScript Alice"));
const post = unwrap(await alice.posts.create({ title: "Composed TypeScript client" }));
assert.deepEqual(post.author, renamed);
assert.deepEqual(unwrap(await bob.posts.get(post.id)), post);
error(await bob.posts.publish(post.id), "forbidden");
assert.equal(unwrap(await alice.posts.publish(post.id)).published, true);
error(await alice.users.get("missing"), "not_found");
error(await alice.posts.create({ title: " " }), "invalid_title");
assert.equal(unwrap(await alice.posts.delete(post.id)), undefined);
error(await alice.posts.get(post.id), "not_found");
assert.ok(seen.some(path => path.includes("/users/")));
assert.ok(seen.some(path => path.includes("/posts")));

// The grouped client uses one response policy and injected transport for all groups.
const user = { id: "alice", displayName: "Alice", added: true };
const enrichedPost = { id: "x", author: user, title: "Post", published: false, added: true };
const fetchEnriched = async url => Response.json(url.pathname.includes("/users/") ? user : enrichedPost);
const compatible = createApiClient(base, { fetch: fetchEnriched });
assert.deepEqual(unwrap(await compatible.users.get("alice")), { id: "alice", displayName: "Alice" });
assert.equal(Object.hasOwn(unwrap(await compatible.posts.get("x")), "added"), false);
const strict = createApiClient(base, { fetch: fetchEnriched, responsePolicy: "strict" });
for (const result of [await strict.users.get("alice"), await strict.posts.get("x")]) {
  assert.equal(result.ok, false);
  assert.equal(result.error.kind, "protocol");
}

console.log("Grouped TypeScript client: shared auth/state, errors, policy, and 204 passed");
