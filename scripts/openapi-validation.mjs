// Independent document/instance validation used by artifacts and live workflows.
import assert from "node:assert/strict";
import SwaggerParser from "@apidevtools/swagger-parser";
import Ajv2020 from "ajv/dist/2020.js";

const escape = value => value.replaceAll("~", "~0").replaceAll("/", "~1");

function at(document, pointer) {
  assert.ok(pointer.startsWith("#/"), `Expected a local document reference: ${pointer}`);
  return pointer.slice(2).split("/").reduce((node, part) => {
    const key = decodeURIComponent(part).replaceAll("~1", "/").replaceAll("~0", "~");
    assert.ok(node && Object.hasOwn(node, key), `Unresolved document reference: ${pointer}`);
    return node[key];
  }, document);
}

function resolved(document, pointer) {
  const visited = new Set();
  let value = at(document, pointer);
  while (typeof value?.$ref === "string") {
    assert.ok(!visited.has(pointer), `Cyclic non-schema reference: ${pointer}`);
    visited.add(pointer);
    pointer = value.$ref;
    value = at(document, pointer);
  }
  return { value, pointer };
}

export async function validateDocument(document) {
  // Generated artifacts are self-contained and do not need external resolution.
  function references(value) {
    if (!value || typeof value !== "object") return;
    if (typeof value.$ref === "string") at(document, value.$ref);
    for (const child of Object.values(value)) references(child);
  }
  references(document);
  await SwaggerParser.validate(structuredClone(document), { resolve: { external: false } });
  const ids = new Set();
  for (const path of Object.values(document.paths)) {
    for (const [method, operation] of Object.entries(path)) {
      if (!["get", "post", "put", "patch", "delete"].includes(method)) continue;
      assert.ok(operation.operationId && !ids.has(operation.operationId), "Operation IDs must be present and unique");
      ids.add(operation.operationId);
    }
  }
}

export function instanceValidator(document) {
  const ajv = new Ajv2020({ strict: false, allErrors: true, validateFormats: false, ownProperties: true });
  const compiled = new Map();
  return (pointer, value) => {
    if (!compiled.has(pointer)) {
      compiled.set(pointer, ajv.compile({
        $schema: "https://json-schema.org/draft/2020-12/schema",
        components: document.components,
        allOf: [at(document, pointer)],
      }));
    }
    const validate = compiled.get(pointer);
    const valid = validate(value);
    return { valid, errors: validate.errors };
  };
}

export function responseFor(document, path, method, status) {
  const pointer = `#/paths/${escape(path)}/${method.toLowerCase()}/responses`;
  const responses = at(document, pointer);
  // OpenAPI's exact-code > range > default precedence is significant for errors.
  const key = [String(status), `${Math.floor(status / 100)}XX`, "default"]
    .find(candidate => Object.hasOwn(responses, candidate));
  assert.ok(key, `Undocumented response ${method} ${path}: ${status}`);
  return resolved(document, `${pointer}/${key}`);
}

export function responseAssertions(document) {
  const validate = instanceValidator(document);
  return async (path, method, response) => {
    const { value: expected, pointer } = responseFor(document, path, method, response.status);
    for (const [name, header] of Object.entries(expected.headers ?? {})) {
      const value = response.headers.get(name);
      assert.ok(value !== null || !header.required, `Missing response header ${name}`);
      if (value !== null && header.schema) {
        const result = validate(`${pointer}/headers/${escape(name)}/schema`, value);
        assert.ok(result.valid, `Invalid response header ${name}: ${JSON.stringify(result.errors)}`);
      }
    }
    const text = await response.clone().text();
    if (!expected.content) {
      assert.equal(text, "", "Bodyless response must be empty");
      assert.equal(response.headers.get("x-spline-error"), null);
      return;
    }
    const media = response.headers.get("content-type")?.split(";")[0].trim().toLowerCase();
    assert.ok(expected.content[media]?.schema, `Undocumented response content type ${media}`);
    const body = JSON.parse(text);
    const result = validate(`${pointer}/content/${escape(media)}/schema`, body);
    assert.ok(result.valid, `Response violates ${method} ${path} (${response.status}): ${JSON.stringify(result.errors)}`);
    const kind = response.headers.get("x-spline-error");
    if (kind !== null) assert.equal(body.kind, kind, "Error marker must match the envelope kind");
  };
}
