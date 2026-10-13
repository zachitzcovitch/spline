import { createUsersClient } from "./generated.ts";

const base = process.argv[2] ?? "http://127.0.0.1:3000";
const client = createUsersClient(base);
const existing = await client.get("user_1");
if (!existing.ok) throw new Error(JSON.stringify(existing.error));
console.log(`Found ${existing.value.displayName}`);
const created = await client.create({ displayName: "TypeScript client" });
if (!created.ok) throw new Error(JSON.stringify(created.error));
const fetched = await client.get(created.value.id);
if (!fetched.ok) throw new Error(JSON.stringify(fetched.error));
console.log(`Created and fetched ${fetched.value.id}`);
const missing = await client.get("missing");
if (missing.ok || missing.error.kind !== "application" || missing.error.value.kind !== "not_found") {
  throw new Error("expected a typed NotFound error");
}
console.log("Handled typed NotFound error");
