import type { UsersCalls, UsersOperations, UserId, NewUser, UserPatch } from "./generated";
import { createUsersClient } from "./generated";

type Equal<A, B> = (<T>() => T extends A ? 1 : 2) extends
  (<T>() => T extends B ? 1 : 2) ? true : false;
type Expect<T extends true> = T;
type KindChecks = [
  Expect<Equal<UsersOperations["users.get"]["kind"], "query">>,
  Expect<Equal<UsersOperations["users.create"]["kind"], "mutation">>,
  Expect<Equal<Parameters<UsersCalls["get"]>, [UserId]>>,
  Expect<Equal<Parameters<UsersCalls["create"]>, [NewUser]>>,
  Expect<Equal<Parameters<UsersCalls["update"]>, [UserId, UserPatch]>>,
  Expect<Equal<UsersOperations["users.delete"]["output"], void>>,
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

http.replace("user_1", { displayName: "Replacement", bio: "new" });
http.update("user_1", {});
http.update("user_1", { displayName: "Partial" });
http.update("user_1", { bio: null });
http.update("user_1", { bio: "new" });
http.delete("user_1");
// @ts-expect-error a PUT replacement requires the nonnullable name
http.replace("user_1", {});
// @ts-expect-error only nullable fields can be explicitly cleared
http.update("user_1", { displayName: null });
// @ts-expect-error exact optional properties distinguish omission from undefined
http.update("user_1", { bio: undefined });
// @ts-expect-error delete has no JSON body argument
http.delete("user_1", {});
// @ts-expect-error no content is not JSON null
const jsonNullIsNotNoContent: UsersOperations["users.delete"]["output"] = null;
async function deleted() {
  const result = await http.delete("user_1");
  if (result.ok) {
    const absent: void = result.value;
    // @ts-expect-error 204 success has no user payload
    result.value.id;
    return absent;
  }
}
