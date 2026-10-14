import { createBlogClient, type BlogClient, type BlogResult, type BlogHttpFailure, type User, type Post, type NewPost, type UserError, type PostError } from "./generated.ts";

type Equal<A, B> = (<T>() => T extends A ? 1 : 2) extends
  (<T>() => T extends B ? 1 : 2) ? true : false;
type Expect<T extends true> = T;

const client: BlogClient = createBlogClient("https://example.test/api");
const input: NewPost = { title: "Hello", body: "Text" };
const draft: Promise<BlogResult<Post, PostError, BlogHttpFailure>> = client.posts.create(input);
type Checks = [
  Expect<Equal<Parameters<BlogClient["posts"]["create"]>, [input: NewPost]>>,
  Expect<Equal<Awaited<ReturnType<BlogClient["users"]["get"]>>, BlogResult<User, UserError, BlogHttpFailure>>>,
];
void draft;

// @ts-expect-error generated dispatch argument wrappers are not public DTO exports
import type { blog_posts_create_Args_Input } from "./generated.ts";
// @ts-expect-error an operation-specific alias is not needed to name a public result
import type { blog_users_get_Output } from "./generated.ts";
// @ts-expect-error public DTOs still enforce their required fields
client.posts.create({ title: "No body" });
