import * as assert from "assert";
import * as vscode from "vscode";
import { findTests, parseTestOutput } from "../../testParser";

const EXTENSION_ID = "pleno.pleno-language";

function extension(): vscode.Extension<unknown> {
    const found = vscode.extensions.getExtension(EXTENSION_ID);
    assert.ok(found, `${EXTENSION_ID} is installed`);
    return found;
}

suite("Pleno extension", () => {
    suiteSetup(async () => {
        await extension().activate();
    });

    test("activates", () => {
        assert.strictEqual(extension().isActive, true);
    });

    test("registers the pleno language", async () => {
        assert.ok((await vscode.languages.getLanguages()).includes("pleno"));
    });

    test("registers every contributed command", async () => {
        const registered = new Set(await vscode.commands.getCommands(true));
        const contributed: { command: string }[] = extension().packageJSON.contributes.commands;
        assert.ok(contributed.length > 0);
        for (const { command } of contributed) {
            assert.ok(registered.has(command), `${command} is registered`);
        }
    });

    test("menus and walkthrough steps only use contributed or built-in commands", async () => {
        const json = extension().packageJSON.contributes;
        const contributed = new Set<string>(json.commands.map((c: { command: string }) => c.command));
        const registered = new Set(await vscode.commands.getCommands(true));
        for (const [menu, entries] of Object.entries(json.menus) as [string, { command: string }[]][]) {
            for (const entry of entries) {
                assert.ok(contributed.has(entry.command), `${menu} uses ${entry.command}, which is contributed`);
            }
        }
        for (const walkthrough of json.walkthroughs) {
            for (const step of walkthrough.steps) {
                for (const match of (step.description as string).matchAll(/\(command:([\w.]+)\)/g)) {
                    assert.ok(registered.has(match[1]), `step ${step.id} links to ${match[1]}`);
                }
            }
        }
    });

    test("contributes no command without a category", () => {
        for (const command of extension().packageJSON.contributes.commands) {
            assert.strictEqual(command.category, "Pleno", command.command);
        }
    });
});

suite("std:test discovery", () => {
    const source = [
        "import { describe, test, assert } from \"std:test\";",
        "// test(\"commented out\", fn() {});",
        "describe(\"math\", fn() {",
        "    let s = \"${f(\"test(\")}\";",
        "    test(\"adds\", fn() { assert.equal(1 + 1, 2); });",
        "    describe(\"nested\", fn() {",
        "        test(`with \"quotes\"`, fn() {});",
        "    });",
        "});",
        "test(\"top\", fn() {});",
        "test(`dynamic ${1}`, fn() {});",
    ].join("\n");

    test("finds nested describe and test calls with their lines", () => {
        const found = findTests(source);
        assert.deepStrictEqual(
            found.map((t) => [t.name, t.line, t.children.map((c) => c.name)]),
            [["math", 2, ["adds", "nested"]], ["top", 9, []]],
        );
        assert.strictEqual(found[0].children[1].children[0].name, "with \"quotes\"");
    });

    test("reads results, failure messages and the file summary", () => {
        const stdout = [
            "math",
            "  ✓ adds",
            "  nested",
            "math",
            "    ✗ with \"quotes\"",
            "      expected 1, got 2",
            "✓ top",
            "{",
            "  \"passed\": 0,",
            "  \"failed\": 1,",
            "  \"total\": 1,",
            "  \"results\": [{ \"file\": \"a.pln\", \"status\": \"failed\", \"error_type\": \"runtime\", \"message\": \"1 test(s) failed\" }]",
            "}",
        ].join("\n");
        const suites = new Set(["math", "math/nested"]);
        const output = parseTestOutput(stdout, (names) => suites.has(names.join("/")));
        assert.deepStrictEqual(output.results, [
            { names: ["math", "adds"], passed: true, message: undefined },
            { names: ["math", "nested", "with \"quotes\""], passed: false, message: "expected 1, got 2" },
            { names: ["top"], passed: true, message: undefined },
        ]);
        assert.strictEqual(output.onlyTestsFailed, true);
        assert.ok(!output.printed.includes("\"passed\""));
    });
});
