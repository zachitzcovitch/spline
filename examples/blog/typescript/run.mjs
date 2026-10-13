import { run } from "./client.ts";

if (process.argv.length > 3) throw new Error("usage: node run.mjs [base-url]");
await run(process.argv[2] ?? "http://127.0.0.1:3000/api");
