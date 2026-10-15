import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { instanceValidator, responseAssertions, validateDocument } from "./openapi-validation.mjs";

if (process.argv[2] === "--stdin") {
  let input = "";
  for await (const chunk of process.stdin) input += chunk;
  const { document, cases = [], responses = [] } = JSON.parse(input);
  await validateDocument(document);
  const validate = instanceValidator(document);
  for (const { pointer, value, valid } of cases) {
    const result = validate(pointer, value);
    assert.equal(result.valid, valid, `${pointer}: ${JSON.stringify(value)}; ${JSON.stringify(result.errors)}`);
  }
  const assertResponse = responseAssertions(document);
  for (const { path, method, status, headers, body } of responses) {
    await assertResponse(path, method, new Response(status === 204 ? null : body, { status, headers }));
  }
  console.log(`OpenAPI document, ${cases.length} schema cases, and ${responses.length} live responses passed`);
} else {
  for (const file of ["../examples/blog/openapi.json", "../examples/contract/openapi.json"]) {
    const document = JSON.parse(await readFile(new URL(file, import.meta.url), "utf8"));
    await validateDocument(document);
    assert.ok(!JSON.stringify(document).toLowerCase().includes("spline"), `${file} must be customer-facing without implementation branding`);
    console.log(`OpenAPI 3.1 document and local references validated: ${file}`);
  }
}
