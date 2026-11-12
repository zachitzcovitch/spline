export type __PREFIX__HttpFailure =
  | { kind: "transport" | "protocol" | "encode"; message: string }
  | { kind: "http"; status: number }
  /** A problem-details fault from the service: branch on `code`; `detail` is developer text, not for end users. */
  | { kind: "framework"; status: number; code: string; detail?: string };

export type __PREFIX__HttpOptions = {
  fetch?: typeof globalThis.fetch;
  headers?: { [name: string]: string };
  /** Compatible projects closed response objects onto known fields. Requests remain schema-checked. */
  responsePolicy?: "compatible" | "strict";
};

/** What an application error carries besides its value: the HTTP status it was answered with. */
export type __PREFIX__HttpErrorContext = { status: number };

export type __PREFIX__Client = __PREFIX__Calls<__PREFIX__HttpFailure, __PREFIX__HttpErrorContext>;

/** Positional calls; JSON codecs and HTTP routing come from the contract binding. */
export function create__PREFIX__Client(
  baseUrl: string,
  options: __PREFIX__HttpOptions = {},
): __PREFIX__Client {
  const base = new globalThis.URL(baseUrl);
  if (!["http:", "https:"].includes(base.protocol) || base.href.includes("?") || base.href.includes("#")
      || base.username || base.password) {
    throw new globalThis.TypeError("base URL must be HTTP(S), without credentials, query, or fragment");
  }
  const fetcher = options.fetch ?? globalThis.fetch.bind(globalThis);
  const defaultHeaders = new globalThis.Headers(options.headers);
  const responsePolicy = options.responsePolicy ?? "compatible";
  if (responsePolicy !== "compatible" && responsePolicy !== "strict") {
    throw new globalThis.TypeError("responsePolicy must be compatible or strict");
  }

  type __SplinekitWire =
    | ["null" | "string" | "boolean"]
    | ["string", "uuid" | "date-time" | "date" | "uri" | "decimal"]
    | ["string", "integer", boolean, number]
    | ["integer", boolean, number]
    | ["number", number]
    | ["literal" | "ref", string]
    | ["array" | "map", __SplinekitWire]
    | ["tuple" | "union", __SplinekitWire[]]
    | ["object", boolean, [string, string, __SplinekitWire, boolean][]];
  const definitions: { [name: string]: __SplinekitWire } = /*__DEFINITIONS__*/;
  const object = (value: unknown): value is { [name: string]: unknown } =>
    value !== null && typeof value === "object" && !globalThis.Array.isArray(value);
  const owns = (value: object, name: string): boolean => globalThis.Object.prototype.hasOwnProperty.call(value, name);
  // Integer strings compare digits against the type's limit, so no BigInt is needed.
  const limits: { [key: string]: string } = {
    u64: "18446744073709551615", i64: "9223372036854775807", n64: "9223372036854775808",
    u128: "340282366920938463463374607431768211455",
    i128: "170141183460469231731687303715884105727",
    n128: "170141183460469231731687303715884105728",
  };
  // Calendar check without Date, whose parsers differ between engines and
  // accept impossible dates such as February 30.
  const day = (year: string, month: string, date: string): boolean => {
    const y = +year, m = +month, d = +date;
    const leap = y % 4 === 0 && (y % 100 !== 0 || y % 400 === 0);
    const days = [31, leap ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31][m - 1];
    return days !== undefined && d >= 1 && d <= days;
  };
  const formatted = (
    value: string,
    schema: ["string", "uuid" | "date-time" | "date" | "uri" | "decimal"] | ["string", "integer", boolean, number],
  ): boolean => {
    switch (schema[1]) {
      case "uuid": return /^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$/.test(value);
      case "date-time": {
        const m = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})(\.\d+)?(Z|[+-](\d{2}):(\d{2}))$/.exec(value);
        return m !== null && day(m[1]!, m[2]!, m[3]!) && +m[4]! <= 23 && +m[5]! <= 59 && +m[6]! <= 60
          && (m[8] === "Z" || (+m[9]! <= 23 && +m[10]! <= 59));
      }
      case "date": {
        const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value);
        return m !== null && day(m[1]!, m[2]!, m[3]!);
      }
      case "uri": try { new globalThis.URL(value); return true; } catch { return false; }
      case "decimal": return /^-?(0|[1-9][0-9]*)(\.[0-9]+)?$/.test(value);
      case "integer": {
        if (!(schema[2] ? /^(0|-?[1-9][0-9]*)$/ : /^(0|[1-9][0-9]*)$/).test(value)) return false;
        const negative = value.startsWith("-");
        const digits = negative ? value.slice(1) : value;
        const limit = limits[`${schema[2] ? (negative ? "n" : "i") : "u"}${schema[3]}`];
        return limit !== undefined
          && (digits.length < limit.length || (digits.length === limit.length && digits <= limit));
      }
    }
  };
  const valid = (value: unknown, schema: __SplinekitWire, depth = 0): boolean => {
    if (depth > 256) return false;
    const child = (value: unknown, schema: __SplinekitWire): boolean => valid(value, schema, depth + 1);
    switch (schema[0]) {
      case "null": return value === null;
      case "string": return typeof value === "string" && (schema.length === 1 || formatted(value, schema));
      case "boolean": return typeof value === "boolean";
      case "number": return typeof value === "number" && globalThis.Number.isFinite(value)
        && (schema[1] === 64 || globalThis.Math.abs(value) <= 3.4028234663852886e38);
      case "integer": {
        if (schema[2] === 53) {
          return typeof value === "number" && globalThis.Number.isSafeInteger(value) && (schema[1] || value >= 0);
        }
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
        && schema[2].every(([name, , type, optional]) => owns(value, name) ? child(value[name], type) : optional)
        && (schema[1] || globalThis.Object.keys(value).every(name => schema[2].some(f => f[0] === name)));
    }
  };
  const same = (a: unknown, b: unknown): boolean => {
    if (a === b) return true;
    if (globalThis.Array.isArray(a) && globalThis.Array.isArray(b)) {
      return a.length === b.length && a.every((v, i) => same(v, b[i]));
    }
    return object(a) && object(b) && globalThis.Object.keys(a).length === globalThis.Object.keys(b).length
      && globalThis.Object.keys(a).every(key => owns(b, key) && same(a[key], b[key]));
  };
  // Undefined is a failure sentinel: parsed JSON cannot contain it. Decoding
  // projects wire names onto TypeScript property names and keeps extra fields
  // from leaking into returned values or strict Rust DTOs; encoding maps
  // property names back to the wire and refuses unknown properties.
  const walk = (value: unknown, schema: __SplinekitWire, encoding: boolean, depth = 0): unknown => {
    if (depth >= 256) return undefined;
    const child = (value: unknown, schema: __SplinekitWire): unknown => walk(value, schema, encoding, depth + 1);
    switch (schema[0]) {
      case "ref": {
        const target = owns(definitions, schema[1]) ? definitions[schema[1]] : undefined;
        return target === undefined ? undefined : child(value, target);
      }
      case "object": {
        if (!object(value)) return undefined;
        const [from, to] = encoding ? [1, 0] as const : [0, 1] as const;
        const known = new globalThis.Set(schema[2].map(f => f[from]));
        const written = new globalThis.Set(schema[2].map(f => f[to]));
        if ((encoding || responsePolicy === "strict") && !schema[1]
            && globalThis.Object.keys(value).some(name => !known.has(name))) return undefined;
        const entries: [string, unknown][] = schema[1]
          ? globalThis.Object.entries(value).filter(([name]) => !known.has(name) && !written.has(name)) : [];
        for (const field of schema[2]) {
          if (!owns(value, field[from])) {
            if (!field[3]) return undefined;
            continue;
          }
          const result = child(value[field[from]], field[2]);
          if (result === undefined) return undefined;
          entries.push([field[to], result]);
        }
        // fromEntries creates own data properties even for names like __proto__.
        return globalThis.Object.fromEntries(entries);
      }
      case "map": {
        if (!object(value)) return undefined;
        const entries: [string, unknown][] = [];
        for (const [name, entry] of globalThis.Object.entries(value)) {
          const decoded = child(entry, schema[1]);
          if (decoded === undefined) return undefined;
          entries.push([name, decoded]);
        }
        return globalThis.Object.fromEntries(entries);
      }
      case "array": {
        if (!globalThis.Array.isArray(value)) return undefined;
        const decoded = value.map(v => child(v, schema[1]));
        return decoded.some(v => v === undefined) ? undefined : decoded;
      }
      case "tuple": {
        if (!globalThis.Array.isArray(value) || value.length !== schema[1].length) return undefined;
        const decoded = schema[1].map((s, i) => child(value[i], s));
        return decoded.some(v => v === undefined) ? undefined : decoded;
      }
      case "union": {
        let result: unknown;
        for (const alternative of schema[1]) {
          const candidate = child(value, alternative);
          if (candidate !== undefined) {
            if (result !== undefined && !same(result, candidate)) return undefined;
            result = candidate;
          }
        }
        return result;
      }
      default: return valid(value, schema, depth) ? value : undefined;
    }
  };
  const project = (value: unknown, schema: __SplinekitWire): unknown => walk(value, schema, false);
  type __SplinekitPlan = {
    method: "GET" | "POST" | "PUT" | "PATCH" | "DELETE";
    path: string; body: string | null; status: number;
    paths: string[]; queries: [string, boolean][];
    input: __SplinekitWire; output: __SplinekitWire;
    /** Declared application errors: [status (0 = any), codes, wire shape]. */
    errors: [number, string[], __SplinekitWire][];
  };
  const message = (error: unknown): string => error instanceof globalThis.Error ? error.message : globalThis.String(error);
  const jsonType = (response: globalThis.Response): boolean => {
    const type = response.headers.get("content-type")?.split(";")[0]?.trim().toLowerCase();
    return type === "application/json" || (type?.startsWith("application/") === true && type.endsWith("+json"));
  };
  const call = async <O, E>(plan: __SplinekitPlan, argumentsObject: unknown): Promise<__PREFIX__Result<O, E, __PREFIX__HttpFailure, __PREFIX__HttpErrorContext>> => {
    let url: globalThis.URL;
    let init: globalThis.RequestInit;
    try {
      // Encode the JSON representation, including omitted undefined properties,
      // onto wire names while validating it.
      const normalized: unknown = globalThis.JSON.parse(globalThis.JSON.stringify(argumentsObject));
      const input = walk(normalized, plan.input, true);
      if (!object(input)) {
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
    const success = response.status === plan.status;
    if (success && response.status === 204) {
      if (response.headers.has("content-length") || response.headers.has("transfer-encoding")) {
        return { ok: false, error: { kind: "protocol", message: "invalid 204 response headers" } };
      }
      let text: string;
      try { text = await response.text(); }
      catch (error) { return { ok: false, error: { kind: "transport", message: message(error) } }; }
      return text.length === 0
        ? { ok: true, value: undefined as O }
        : { ok: false, error: { kind: "protocol", message: "204 response must have an empty body" } };
    }
    if (success && !jsonType(response)) {
      return { ok: false, error: { kind: "protocol", message: "invalid success response headers" } };
    }
    if (!success && response.status < 400) {
      return { ok: false, error: { kind: "protocol", message: `unexpected success/redirect status ${response.status}` } };
    }
    if (!success && !jsonType(response)) {
      return { ok: false, error: { kind: "http", status: response.status } };
    }
    let text: string;
    try { text = await response.text(); }
    catch (error) { return { ok: false, error: { kind: "transport", message: message(error) } }; }
    let value: unknown;
    try { value = globalThis.JSON.parse(text); }
    catch (error) {
      return success
        ? { ok: false, error: { kind: "protocol", message: message(error) } }
        : { ok: false, error: { kind: "http", status: response.status } };
    }
    if (success) {
      const decoded = project(value, plan.output);
      return decoded !== undefined
        ? { ok: true, value: decoded as O }
        : { ok: false, error: { kind: "protocol", message: "response does not match the output schema" } };
    }
    // Problem details carrying this status and a code; anything else (a
    // proxy's page, a gateway error) is an unrecognized HTTP failure.
    if (!object(value) || value.status !== response.status || typeof value.code !== "string") {
      return { ok: false, error: { kind: "http", status: response.status } };
    }
    const code = value.code;
    // A code the operation declares at this status is an application error
    // and must decode as one; any other code is a framework fault.
    const declared = plan.errors.find(([status, codes]) => (status === 0 || status === response.status) && codes.includes(code));
    if (declared !== undefined) {
      // fromEntries defines properties, so a `__proto__` key stays data.
      const fields = globalThis.Object.fromEntries(globalThis.Object.entries(value)
        .filter(([name]) => !["type", "title", "status", "detail", "instance"].includes(name)));
      const decoded = project(fields, declared[2]);
      return decoded !== undefined
        ? { ok: false, error: { kind: "application", status: response.status, value: decoded as E } }
        : { ok: false, error: { kind: "protocol", message: "application error does not match its declared schema" } };
    }
    return {
      ok: false,
      error: typeof value.detail === "string"
        ? { kind: "framework", status: response.status, code, detail: value.detail }
        : { kind: "framework", status: response.status, code },
    };
  };
  return {
/*__METHODS__*/
  };
}
