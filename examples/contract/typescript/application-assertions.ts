import { createApiClient, type ApiCalls, type ApiHttpFailure, type ApiOperations } from "./application.generated";

const client: ApiCalls<ApiHttpFailure> = createApiClient("https://example.test/api", {
  headers: { authorization: "Bearer alice" },
  responsePolicy: "strict",
});
client.users.get("alice");
client.users.rename("Alice");
client.posts.create({ title: "Post" });
client.posts.get("post_1");
client.posts.publish("post_1");
const query: ApiOperations["app.users.get"]["kind"] = "query";
const mutation: ApiOperations["app.posts.publish"]["kind"] = "mutation";
void query; void mutation;

// @ts-expect-error grouped clients do not flatten colliding operation names
client.get("alice");
// @ts-expect-error users do not expose post operations
client.users.publish("post_1");
// @ts-expect-error positional arguments retain their types
client.posts.get(1);
// @ts-expect-error post creation requires its DTO
client.posts.create("Post");
// @ts-expect-error required DTO fields remain required
client.posts.create({});
// @ts-expect-error renamed client groups do not change logical operation IDs
type WrongIdentity = ApiOperations["users.get"];

async function results() {
  const user = await client.users.get("alice");
  if (user.ok) {
    const name: string = user.value.displayName;
    void name;
    // @ts-expect-error user and post result types are distinct
    user.value.published;
  } else if (user.error.kind === "application") {
    const kind: "not_found" = user.error.value.kind;
    void kind;
    // @ts-expect-error user errors do not include post-only variants
    const postOnly: "forbidden" = user.error.value.kind;
    void postOnly;
  }
  const post = await client.posts.publish("post_1");
  if (post.ok) {
    const author: ApiOperations["app.users.get"]["output"] = post.value.author;
    void author;
  } else if (post.error.kind === "application") {
    const kind: "not_found" | "forbidden" | "invalid_title" = post.error.value.kind;
    void kind;
  }
  const deleted = await client.posts.delete("post_1");
  if (deleted.ok) {
    const empty: void = deleted.value;
    void empty;
    // @ts-expect-error a grouped 204 operation still has no JSON payload
    const payload: null = deleted.value;
    void payload;
  }
}
void results;
