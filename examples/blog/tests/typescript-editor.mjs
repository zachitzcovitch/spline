// Exercise the TypeScript language service used by editors, not just tsc.
import assert from "node:assert/strict";
import test from "node:test";
import { fileURLToPath } from "node:url";
import ts from "typescript";

test("generated blog types provide readable hovers, signatures, and completions", () => {
  const directory = fileURLToPath(new URL("../typescript/", import.meta.url));
  const file = `${directory}editor-fixture.ts`;
  const source = `
    import { createBlogClient, type User, type Post, type NewPost, type UserError } from "./generated.ts";
    const client = createBlogClient("https://example.test/api");
    async function inspect() {
      client.posts.create(/*create-signature*/ { title: "Post", body: "Text" });
      client.users.get(/*get-signature*/ "alice");
      client.posts.create({ /*post-fields*/ });
      const result = await client.users.get("alice");
      if (result.ok) {
        const /*user-hover*/user = result.value;
        user./*user-fields*/
      } else if (result.error.kind === "application") {
        const /*error-hover*/error = result.error.value;
      }
    }
  `;
  const config = ts.readConfigFile(`${directory}tsconfig.json`, ts.sys.readFile);
  const parsed = ts.parseJsonConfigFileContent(config.config, ts.sys, directory);
  const host = {
    getCompilationSettings: () => parsed.options,
    getScriptFileNames: () => [file],
    getScriptVersion: () => "0",
    getScriptSnapshot: path => {
      const text = path === file ? source : ts.sys.readFile(path);
      return text === undefined ? undefined : ts.ScriptSnapshot.fromString(text);
    },
    getCurrentDirectory: () => directory,
    getDefaultLibFileName: options => ts.getDefaultLibFilePath(options),
    fileExists: path => path === file || ts.sys.fileExists(path),
    readFile: path => path === file ? source : ts.sys.readFile(path),
    readDirectory: ts.sys.readDirectory,
    directoryExists: ts.sys.directoryExists,
  };
  const service = ts.createLanguageService(host);
  const position = marker => source.indexOf(`/*${marker}*/`) + marker.length + 4;
  const signature = marker => {
    const help = service.getSignatureHelpItems(file, position(marker), undefined);
    assert.ok(help, marker);
    const item = help.items[help.selectedItemIndex];
    return ts.displayPartsToString(item.prefixDisplayParts) +
      item.parameters.map(p => ts.displayPartsToString(p.displayParts)).join(ts.displayPartsToString(item.separatorDisplayParts)) +
      ts.displayPartsToString(item.suffixDisplayParts);
  };
  try {
    assert.equal(signature("create-signature"), "create(input: NewPost): Promise<BlogResult<Post, PostError, BlogHttpFailure>>");
    assert.equal(signature("get-signature"), "get(id: string): Promise<BlogResult<User, UserError, BlogHttpFailure>>");
    for (const [marker, expected] of [["user-hover", "const user: User"], ["error-hover", "const error: UserError"]]) {
      const info = service.getQuickInfoAtPosition(file, position(marker));
      assert.ok(info, marker);
      assert.equal(ts.displayPartsToString(info.displayParts), expected);
    }
    for (const [marker, expected] of [["post-fields", ["body", "title"]], ["user-fields", ["displayName", "id"]]]) {
      const completion = service.getCompletionsAtPosition(file, position(marker), {});
      assert.ok(completion, marker);
      assert.deepEqual(completion.entries.map(entry => entry.name).sort(), expected);
    }
  } finally {
    service.dispose();
  }
});
