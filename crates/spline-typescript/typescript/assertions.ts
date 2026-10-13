import type {
  api_User_Input,
  api_User_Output,
  api_UserId_Output,
  api_ApiError_Input,
  api_ApiError_Output,
  api_Node_Of_core_String_End_Output,
  api_Page_Of_api_User_End_Output,
  api_Page_Of_api_Post_End_Output,
  api_Left_Output,
  api_Tree_Output,
  api_OpenRecord_Input,
  api_OpenRecord_Output,
  api_Empty_Output,
  api_Unit_Output,
  api_Unusual_Output,
  OptionalIdsOutput,
  LabelsOutput,
  PairOutput,
  UserListInput,
  Array as ShadowArray,
  Record as ShadowRecord,
} from "./generated";
import { user as serializedUser, node as serializedNode } from "./wire-samples";

// This name occurs in a Rust doc comment, not as an exported declaration.
// @ts-expect-error escaped documentation must not inject a TypeScript export
import type { Injected } from "./generated";

type Equal<A, B> = (<T>() => T extends A ? 1 : 2) extends
  (<T>() => T extends B ? 1 : 2) ? true : false;
type Expect<T extends true> = T;
type Optional<T, K extends keyof T> = {} extends Pick<T, K> ? true : false;

type ShapeChecks = [
  Expect<Equal<api_UserId_Output, string>>,
  Expect<Equal<api_User_Input["nickname"], string | null | undefined>>,
  Expect<Equal<api_User_Output["nickname"], string | null>>,
  Expect<Optional<api_User_Input, "tags">>,
  Expect<Equal<Optional<api_User_Output, "tags">, false>>,
  Expect<Equal<api_User_Output["bio"], string | undefined>>,
  Expect<Equal<OptionalIdsOutput, (string | null)[]>>,
  Expect<Equal<PairOutput, [string, boolean]>>,
  Expect<Equal<ShadowArray, string>>,
  Expect<Equal<ShadowRecord, boolean>>,
];

const input: api_User_Input = { id: "7", displayName: "Zach" };
const nullableInput: api_User_Input = { ...input, nickname: null, bio: null };
const output: api_User_Output = { id: "7", displayName: "Zach", nickname: null, tags: [] };
const withBio: api_User_Output = { ...output, bio: "Hello" };
const wireUser: api_User_Output = serializedUser;

// @ts-expect-error nickname is present on serialized output, even if null
const missingNickname: api_User_Output = { id: "7", displayName: "Zach", tags: [] };
// @ts-expect-error defaulted input does not imply omitted output
const missingTags: api_User_Output = { id: "7", displayName: "Zach", nickname: null };
// @ts-expect-error the precisely omitted Option emits a string when present
const nullBio: api_User_Output = { ...output, bio: null };
// @ts-expect-error exactOptionalPropertyTypes distinguishes omission from undefined
const undefinedBio: api_User_Output = { ...output, bio: undefined };
// @ts-expect-error required IDs use the string wire codec, not numeric JSON
const numericId: api_User_Input = { id: 7, displayName: "Zach" };
// @ts-expect-error deny_unknown_fields also rejects excess properties in fresh literals
const extraInput: api_User_Input = { ...input, extra: true };

const ids: OptionalIdsOutput = ["1", null, "2"];
// @ts-expect-error nullable elements do not make the array itself nullable
const nullIds: OptionalIdsOutput = null;
// @ts-expect-error large IDs have a string codec
const numericIds: OptionalIdsOutput = [1];
const users: UserListInput = [input, nullableInput];
const labels: LabelsOutput = { primary: [1, null, 2] };
// @ts-expect-error maps preserve their value shape
const badLabels: LabelsOutput = { primary: "one" };
declare const dictionary: LabelsOutput;
const lookup: (number | null)[] | undefined = dictionary["missing"];
// @ts-expect-error noUncheckedIndexedAccess does not promise that a key exists
const guaranteedLookup: (number | null)[] = dictionary["missing"];
const pair: PairOutput = ["hello", true];
// @ts-expect-error tuples have fixed length and ordered element types
const shortPair: PairOutput = ["hello"];

const notFoundInput: api_ApiError_Input = { kind: "not_found", details: null };
const notFound: api_ApiError_Output = { kind: "not_found" };
const validation: api_ApiError_Output = { kind: "validation", details: { field: "title" } };
// @ts-expect-error named payload is required for this discriminator
const missingDetails: api_ApiError_Output = { kind: "validation" };
// @ts-expect-error unit variant omits content in the serialization view
const outputDetails: api_ApiError_Output = { kind: "not_found", details: null };
// @ts-expect-error unknown variants are not silently accepted
const unknownVariant: api_ApiError_Output = { kind: "unknown" };
function message(error: api_ApiError_Output): string {
  if (error.kind === "validation") return error.details.field;
  // @ts-expect-error discrimination excludes the payload from the unit variant
  error.details;
  return "not found";
}

const tree: api_Tree_Output = [[], [[], [[]]]];
const node: api_Node_Of_core_String_End_Output = serializedNode;
const mutual: api_Left_Output = { right: { left: null } };
// @ts-expect-error recursive element types remain constrained
const badTree: api_Tree_Output = ["leaf"];
// @ts-expect-error generic element type survives recursive references
const badNode: api_Node_Of_core_String_End_Output = { value: 7, children: [] };
// @ts-expect-error recursive output Option fields are required, though nullable
const missingRecursiveField: api_Left_Output = {};

const userPage: api_Page_Of_api_User_End_Output = { items: [output] };
const postPage: api_Page_Of_api_Post_End_Output = { items: [{ title: "A post" }] };
// @ts-expect-error concrete generic instantiations do not collapse together
const wrongPage: api_Page_Of_api_User_End_Output = postPage;

const openInput: api_OpenRecord_Input = { label: "hello", extra: { arbitrary: true } };
// @ts-expect-error the index signature does not weaken known properties
const badOpen: api_OpenRecord_Input = { label: 3 };
// @ts-expect-error output does not promise to emit unknown input fields
const extraOutput: api_OpenRecord_Output = { label: "hello", extra: true };
// Structural TypeScript permits extras on non-fresh values. It is not exact-object validation.
const structuralOutput: api_OpenRecord_Output = openInput;
const empty: api_Empty_Output = {};
// @ts-expect-error TS {} would allow this primitive; the exporter avoids that spelling
const primitiveEmpty: api_Empty_Output = 3;
// @ts-expect-error a closed empty object has no properties
const extraEmpty: api_Empty_Output = { extra: true };
const unit: api_Unit_Output = null;
// @ts-expect-error Rust unit's JSON representation is null, not undefined
const undefinedUnit: api_Unit_Output = undefined;

const unusual: api_Unusual_Output = {
  "quote\"line\n\\end\u2028": "safe",
  "default": true,
  ["__proto__"]: "a property, not a prototype mutation",
  "kind": "literal\"\\\n\u2029",
};
type UnusualChecks = [
  Expect<Equal<api_Unusual_Output["quote\"line\n\\end\u2028"], string>>,
  Expect<Equal<api_Unusual_Output["kind"], "literal\"\\\n\u2029">>,
];
