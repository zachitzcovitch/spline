// Generated from real Serde values. Do not edit.
import type { api_User_Output, api_ApiError_Output, api_Node_Of_core_String_End_Output, api_Tree_Output } from "./generated";

export const user = {
  "displayName": "Zach",
  "id": "18446744073709551615",
  "nickname": null,
  "tags": []
} satisfies api_User_Output;

export const notFound = {
  "kind": "not_found"
} satisfies api_ApiError_Output;

export const validation = {
  "details": {
    "field": "title"
  },
  "kind": "validation"
} satisfies api_ApiError_Output;

export const node = {
  "children": [
    {
      "children": [],
      "value": "leaf"
    }
  ],
  "value": "root"
} satisfies api_Node_Of_core_String_End_Output;

export const tree = [
  []
] satisfies api_Tree_Output;
