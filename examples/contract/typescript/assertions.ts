import type { UsersCalls, UsersOperations, example_UserId_Input, example_NewUser_Input } from "./generated";
import { createUsersClient } from "./generated";

type Equal<A, B> = (<T>() => T extends A ? 1 : 2) extends
  (<T>() => T extends B ? 1 : 2) ? true : false;
type Expect<T extends true> = T;
type KindChecks = [
  Expect<Equal<UsersOperations["users.get"]["kind"], "query">>,
  Expect<Equal<UsersOperations["users.create"]["kind"], "mutation">>,
  Expect<Equal<Parameters<UsersCalls["get"]>, [example_UserId_Input]>>,
  Expect<Equal<Parameters<UsersCalls["create"]>, [example_NewUser_Input]>>,
];

declare const calls: UsersCalls<{ kind: "transport"; message: string }>;
calls.get("user_1");
calls.create({ displayName: "New user" });
calls.create({ displayName: "New user", tags: ["author"] });
// @ts-expect-error query argument type follows the contract's UserId codec
calls.get(1);
// @ts-expect-error the authored positional argument is required
calls.get();
// @ts-expect-error create accepts the DTO directly, not a generated argument wrapper
calls.create({ input: { displayName: "New user" } });
// @ts-expect-error DTO fields remain typed
calls.create({ displayName: "New user", tags: "author" });
// @ts-expect-error operation names are closed
calls.remove({ id: "user_1" });
// @ts-expect-error stable operation IDs include their namespace
type WrongIdentity = UsersOperations["get"];

async function consume() {
  const result = await calls.get("user_1");
  if (result.ok) {
    const displayName: string = result.value.displayName;
    // @ts-expect-error successful output is typed rather than any
    const invalid: number = result.value.id;
    return displayName;
  }
  if (result.error.kind === "transport") return result.error.message;
  const application = result.error.value;
  if (application.kind === "invalid_name") return application.details.message;
  // @ts-expect-error declared unit error has no details payload
  application.details;
  return "missing";
}

declare const domainOnly: UsersCalls;
async function domainFailure() {
  const result = await domainOnly.create({ displayName: "Name" });
  if (!result.ok) {
    const category: "application" = result.error.kind;
    // @ts-expect-error transport errors only exist when supplied in the Failure slot
    result.error.message;
    return category;
  }
}

const http = createUsersClient("http://localhost:3000");
http.get("user_1");
http.create({ displayName: "HTTP user" });
// @ts-expect-error generated HTTP methods preserve the authored positional signature
http.get({ id: "user_1" });
