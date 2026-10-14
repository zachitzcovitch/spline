// Generated from real Serde values. Do not edit.
import type { UserOutput, ApiErrorOutput, NodeOfString, Tree } from "./generated";

export const user = {
  "displayName": "Zach",
  "id": "18446744073709551615",
  "nickname": null,
  "tags": []
} satisfies UserOutput;

export const notFound = {
  "kind": "not_found"
} satisfies ApiErrorOutput;

export const validation = {
  "details": {
    "field": "title"
  },
  "kind": "validation"
} satisfies ApiErrorOutput;

export const node = {
  "children": [
    {
      "children": [],
      "value": "leaf"
    }
  ],
  "value": "root"
} satisfies NodeOfString;

export const tree = [
  []
] satisfies Tree;
