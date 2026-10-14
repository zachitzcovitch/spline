import type {
  UserInput,
  UserOutput,
  UserId,
  ApiErrorInput,
  ApiErrorOutput,
  NodeOfString,
  PageOfUserOutput,
  PageOfPost,
  LeftOutput,
  Tree,
  OpenRecordInput,
  OpenRecordOutput,
  Empty,
  Unit,
  Unusual,
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
  Expect<Equal<UserId, string>>,
  Expect<Equal<UserInput["nickname"], string | null | undefined>>,
  Expect<Equal<UserOutput["nickname"], string | null>>,
  Expect<Optional<UserInput, "tags">>,
  Expect<Equal<Optional<UserOutput, "tags">, false>>,
  Expect<Equal<UserOutput["bio"], string | undefined>>,
  Expect<Equal<OptionalIdsOutput, (string | null)[]>>,
  Expect<Equal<PairOutput, [string, boolean]>>,
  Expect<Equal<ShadowArray, string>>,
  Expect<Equal<ShadowRecord, boolean>>,
];

const input: UserInput = { id: "7", displayName: "Zach" };
const nullableInput: UserInput = { ...input, nickname: null, bio: null };
const output: UserOutput = { id: "7", displayName: "Zach", nickname: null, tags: [] };
const withBio: UserOutput = { ...output, bio: "Hello" };
const wireUser: UserOutput = serializedUser;

// @ts-expect-error nickname is present on serialized output, even if null
const missingNickname: UserOutput = { id: "7", displayName: "Zach", tags: [] };
// @ts-expect-error defaulted input does not imply omitted output
const missingTags: UserOutput = { id: "7", displayName: "Zach", nickname: null };
// @ts-expect-error the precisely omitted Option emits a string when present
const nullBio: UserOutput = { ...output, bio: null };
// @ts-expect-error exactOptionalPropertyTypes distinguishes omission from undefined
const undefinedBio: UserOutput = { ...output, bio: undefined };
// @ts-expect-error required IDs use the string wire codec, not numeric JSON
const numericId: UserInput = { id: 7, displayName: "Zach" };
// @ts-expect-error deny_unknown_fields also rejects excess properties in fresh literals
const extraInput: UserInput = { ...input, extra: true };

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

const notFoundInput: ApiErrorInput = { kind: "not_found", details: null };
const notFound: ApiErrorOutput = { kind: "not_found" };
const validation: ApiErrorOutput = { kind: "validation", details: { field: "title" } };
// @ts-expect-error named payload is required for this discriminator
const missingDetails: ApiErrorOutput = { kind: "validation" };
// @ts-expect-error unit variant omits content in the serialization view
const outputDetails: ApiErrorOutput = { kind: "not_found", details: null };
// @ts-expect-error unknown variants are not silently accepted
const unknownVariant: ApiErrorOutput = { kind: "unknown" };
function message(error: ApiErrorOutput): string {
  if (error.kind === "validation") return error.details.field;
  // @ts-expect-error discrimination excludes the payload from the unit variant
  error.details;
  return "not found";
}

const tree: Tree = [[], [[], [[]]]];
const node: NodeOfString = serializedNode;
const mutual: LeftOutput = { right: { left: null } };
// @ts-expect-error recursive element types remain constrained
const badTree: Tree = ["leaf"];
// @ts-expect-error generic element type survives recursive references
const badNode: NodeOfString = { value: 7, children: [] };
// @ts-expect-error recursive output Option fields are required, though nullable
const missingRecursiveField: LeftOutput = {};

const userPage: PageOfUserOutput = { items: [output] };
const postPage: PageOfPost = { items: [{ title: "A post" }] };
// @ts-expect-error concrete generic instantiations do not collapse together
const wrongPage: PageOfUserOutput = postPage;

const openInput: OpenRecordInput = { label: "hello", extra: { arbitrary: true } };
// @ts-expect-error the index signature does not weaken known properties
const badOpen: OpenRecordInput = { label: 3 };
// @ts-expect-error output does not promise to emit unknown input fields
const extraOutput: OpenRecordOutput = { label: "hello", extra: true };
// Structural TypeScript permits extras on non-fresh values. It is not exact-object validation.
const structuralOutput: OpenRecordOutput = openInput;
const empty: Empty = {};
// @ts-expect-error TS {} would allow this primitive; the exporter avoids that spelling
const primitiveEmpty: Empty = 3;
// @ts-expect-error a closed empty object has no properties
const extraEmpty: Empty = { extra: true };
const unit: Unit = null;
// @ts-expect-error Rust unit's JSON representation is null, not undefined
const undefinedUnit: Unit = undefined;

const unusual: Unusual = {
  "quote\"line\n\\end\u2028": "safe",
  "default": true,
  ["__proto__"]: "a property, not a prototype mutation",
  "kind": "literal\"\\\n\u2029",
};
type UnusualChecks = [
  Expect<Equal<Unusual["quote\"line\n\\end\u2028"], string>>,
  Expect<Equal<Unusual["kind"], "literal\"\\\n\u2029">>,
];
