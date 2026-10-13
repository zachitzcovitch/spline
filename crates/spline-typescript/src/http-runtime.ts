export type __PREFIX__HttpFailure =
  | { kind: "transport" | "protocol" | "encode"; message: string }
  | { kind: "http"; status: number }
  | { kind: "framework"; status: number; code: string; message: string };

export type __PREFIX__HttpOptions = {
  fetch?: typeof globalThis.fetch;
  headers?: { [name: string]: string };
};

/** Positional calls; JSON codecs and HTTP routing come from the contract binding. */
export function create__PREFIX__Client(
  baseUrl: string,
  options: __PREFIX__HttpOptions = {},
): __PREFIX__Calls<__PREFIX__HttpFailure> {
  const base = new globalThis.URL(baseUrl);
  if (!["http:", "https:"].includes(base.protocol) || base.href.includes("?") || base.href.includes("#")
      || base.username || base.password) {
    throw new globalThis.TypeError("base URL must be HTTP(S), without credentials, query, or fragment");
  }
  const fetcher = options.fetch ?? globalThis.fetch.bind(globalThis);
  const defaultHeaders = new globalThis.Headers(options.headers);

  type Wire =
    | ["null" | "string" | "boolean"]
    | ["integer", boolean, number]
    | ["literal" | "ref", string]
    | ["array" | "map", Wire]
    | ["tuple" | "union", Wire[]]
    | ["object", boolean, [string, Wire, boolean][]];
  const definitions: { [name: string]: Wire } = /*__DEFINITIONS__*/;
  const object = (value: unknown): value is { [name: string]: unknown } =>
    value !== null && typeof value === "object" && !globalThis.Array.isArray(value);
  const owns = (value: object, name: string): boolean => globalThis.Object.prototype.hasOwnProperty.call(value, name);
  const valid = (value: unknown, schema: Wire, depth = 0): boolean => {
    if (depth > 256) return false;
    const child = (value: unknown, schema: Wire): boolean => valid(value, schema, depth + 1);
    switch (schema[0]) {
      case "null": return value === null;
      case "string": return typeof value === "string";
      case "boolean": return typeof value === "boolean";
      case "integer": {
        const bound = 2 ** (schema[2] - (schema[1] ? 1 : 0));
        return typeof value === "number" && globalThis.Number.isInteger(value)
          && value >= (schema[1] ? -bound : 0) && value < bound;
      }
      case "literal": return value === schema[1];
      case "ref": {
        const target = owns(definitions, schema[1]) ? definitions[schema[1]] : undefined;
        return target !== undefined && child(value, target);
      }
      case "array": return globalThis.Array.isArray(value) && value.every(v => child(v, schema[1]));
      case "map": return object(value) && globalThis.Object.values(value).every(v => child(v, schema[1]));
      case "tuple": return globalThis.Array.isArray(value) && value.length === schema[1].length
        && schema[1].every((s, i) => child(value[i], s));
      case "union": return schema[1].some(s => child(value, s));
      case "object": return object(value)
        && schema[2].every(([name, type, optional]) => owns(value, name) ? child(value[name], type) : optional)
        && (schema[1] || globalThis.Object.keys(value).every(name => schema[2].some(f => f[0] === name)));
    }
  };
  type Plan = {
    method: "GET" | "POST"; path: string; body: string | null; status: number;
    paths: string[]; queries: [string, boolean][];
    input: Wire; output: Wire; error: Wire;
  };
  type Result<O, E> =
    | { ok: true; value: O }
    | { ok: false; error: { kind: "application"; value: E } | __PREFIX__HttpFailure };
  const message = (error: unknown): string => error instanceof globalThis.Error ? error.message : globalThis.String(error);
  const jsonType = (response: globalThis.Response): boolean => {
    const type = response.headers.get("content-type")?.split(";")[0]?.trim().toLowerCase();
    return type === "application/json" || (type?.startsWith("application/") === true && type.endsWith("+json"));
  };
  const call = async <O, E>(plan: Plan, argumentsObject: unknown): Promise<Result<O, E>> => {
    let url: globalThis.URL;
    let init: globalThis.RequestInit;
    try {
      // Validate the JSON representation, including omitted undefined properties.
      const encoded = globalThis.JSON.stringify(argumentsObject);
      const input: unknown = globalThis.JSON.parse(encoded);
      if (!object(input) || !valid(input, plan.input)) {
        return { ok: false, error: { kind: "encode", message: "arguments do not match the input schema" } };
      }
      let path = plan.path;
      for (const name of plan.paths) {
        const value = globalThis.String(input[name]);
        if (!value || value === "." || value === "..") throw new globalThis.TypeError("invalid path argument");
        path = path.replace(`{${name}}`, globalThis.encodeURIComponent(value));
      }
      url = new globalThis.URL(base.toString());
      url.pathname = base.pathname.replace(/\/+$/, "") + path;
      for (const [name, optional] of plan.queries) {
        const value = input[name];
        if ((value === null || value === undefined) && optional) continue;
        url.searchParams.append(name, globalThis.String(value));
      }
      const headers = new globalThis.Headers(defaultHeaders);
      headers.set("accept", "application/json");
      init = { method: plan.method, headers, redirect: "manual" };
      if (plan.body !== null) {
        const body = globalThis.JSON.stringify(input[plan.body]);
        if (body === undefined) throw new globalThis.TypeError("missing body argument");
        headers.set("content-type", "application/json");
        init.body = body;
      }
    } catch (error) {
      return { ok: false, error: { kind: "encode", message: message(error) } };
    }
    let response: globalThis.Response;
    try { response = await fetcher(url, init); }
    catch (error) { return { ok: false, error: { kind: "transport", message: message(error) } }; }
    const marker = response.headers.get("x-spline-error");
    const success = response.status === plan.status;
    if (success && (!jsonType(response) || marker !== null)) {
      return { ok: false, error: { kind: "protocol", message: "invalid success response headers" } };
    }
    if (!success && response.status < 400) {
      return { ok: false, error: { kind: "protocol", message: `unexpected success/redirect status ${response.status}` } };
    }
    if (!success && (!jsonType(response) || marker === null)) {
      return { ok: false, error: { kind: "http", status: response.status } };
    }
    let text: string;
    try { text = await response.text(); }
    catch (error) { return { ok: false, error: { kind: "transport", message: message(error) } }; }
    let value: unknown;
    try { value = globalThis.JSON.parse(text); }
    catch (error) { return { ok: false, error: { kind: "protocol", message: message(error) } }; }
    if (success) {
      return valid(value, plan.output)
        ? { ok: true, value: value as O }
        : { ok: false, error: { kind: "protocol", message: "response does not match the output schema" } };
    }
    if (object(value) && marker === "application" && value.kind === "application"
        && owns(value, "value") && valid(value.value, plan.error)) {
      return { ok: false, error: { kind: "application", value: value.value as E } };
    }
    if (object(value) && marker === "framework" && value.kind === "framework"
        && typeof value.code === "string" && typeof value.message === "string") {
      return { ok: false, error: { kind: "framework", status: response.status, code: value.code, message: value.message } };
    }
    return { ok: false, error: { kind: "protocol", message: "unrecognized error envelope" } };
  };
  return {
/*__METHODS__*/
  };
}
