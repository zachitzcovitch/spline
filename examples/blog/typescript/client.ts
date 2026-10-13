import { createBlogClient } from "./generated.ts";

// -------- Blog workflow --------

export async function run(baseUrl: string): Promise<void> {
  // -------- Authenticated clients --------

  const alice = createClient(baseUrl, "alice-token");
  const bob = createClient(baseUrl, "bob-token");

  // -------- Read a profile --------

  const user = valueOrThrow(await alice.users.get("alice"), "Get user");
  console.log(`User: ${user.displayName}`);

  // -------- Create and read a draft --------

  const newPost = {
    title: "Hello from Spline",
    body: "One Rust contract, two typed clients.",
  };
  const draft = valueOrThrow(await alice.posts.create(newPost), "Create draft");
  console.log(`Created draft: ${draft.id}`);

  const fetched = valueOrThrow(await alice.posts.get(draft.id), "Get draft");
  console.log(`Read draft: ${fetched.title}`);

  // -------- Handle Bob's expected Forbidden error --------

  const denied = await bob.posts.publish(draft.id);
  if (
    !denied.ok &&
    denied.error.kind === "application" &&
    denied.error.value.kind === "forbidden"
  ) {
    console.log("Bob cannot publish Alice's post (typed Forbidden)");
  } else {
    throw new Error(`Expected Forbidden, received ${JSON.stringify(denied)}`);
  }

  // -------- Publish as Alice --------

  const published = valueOrThrow(await alice.posts.publish(draft.id), "Publish");
  console.log(`Published: ${published.id} (published=${published.published})`);
}

// -------- Client configuration --------

function createClient(baseUrl: string, token: string) {
  return createBlogClient(baseUrl, {
    headers: { authorization: `Bearer ${token}` },
  });
}

// -------- Unexpected-error handling --------

type Result<T> =
  | { ok: true; value: T }
  | { ok: false; error: unknown };

function valueOrThrow<T>(result: Result<T>, operation: string): T {
  if (result.ok) return result.value;
  throw new Error(`${operation} failed: ${JSON.stringify(result.error)}`);
}
