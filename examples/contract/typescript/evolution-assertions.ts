import { createEvolutionV1Client } from "./evolution-v1.generated";
import type { State, User, EvolutionV1HttpOptions } from "./evolution-v1.generated";

const compatible = createEvolutionV1Client("http://localhost:3000");
const strict = createEvolutionV1Client("http://localhost:3000", { responsePolicy: "strict" });
const options: EvolutionV1HttpOptions = { responsePolicy: "compatible" };
// @ts-expect-error invalid policy values must not silently disable validation
const invalid: EvolutionV1HttpOptions = { responsePolicy: "unchecked" };

compatible.get("user_1");
strict.create({ displayName: "v1", state: "active" });
// @ts-expect-error the old contract's positional signature has no newly added query argument
compatible.get("user_1", "new audit value");
// @ts-expect-error accepting additive response fields does not relax authored inputs
compatible.create({ displayName: "v1", state: "active", typo: true });
// @ts-expect-error old input enums remain closed
compatible.create({ displayName: "v1", state: "suspended" });
// @ts-expect-error response projection does not widen the declared enum
const unknownState: State = "suspended";

declare const user: User;
// @ts-expect-error projected extra response fields do not become part of the old type
user.revision;
// @ts-expect-error nested added fields are projected as well
user.profile.locale;
const name: string = user.profile.displayName;
const addedCounter: number | undefined = user.counters["added_key"];

async function errors() {
  const result = await compatible.get("existing-error");
  if (!result.ok && result.error.kind === "application" && result.error.value.kind === "missing") {
    const message: string = result.error.value.details.message;
    // @ts-expect-error fields added to a known error are absent from the old type
    result.error.value.details.retryable;
    return message;
  }
}
