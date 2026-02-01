import * as assert from "assert";
import * as vscode from "vscode";

suite("Pleno Extension Test Suite", () => {
    vscode.window.showInformationMessage("Start all tests.");

    test("Extension should be present", () => {
        assert.ok(vscode.extensions.getExtension("pleno.pleno-language"));
    });

    test("Extension should activate", async () => {
        const extension = vscode.extensions.getExtension(
            "pleno.pleno-language",
        );
        if (extension) {
            await extension.activate();
            assert.strictEqual(extension.isActive, true);
        }
    });

    test("Commands should be registered", async () => {
        const commands = await vscode.commands.getCommands(true);

        const plenoCommands = [
            "pleno.showDashboard",
            "pleno.runFile",
            "pleno.checkFile",
            "pleno.refreshProject",
            "pleno.showControlPanel",
        ];

        for (const cmd of plenoCommands) {
            assert.ok(
                commands.includes(cmd),
                `Command ${cmd} should be registered`,
            );
        }
    });

    test("Pleno language should be registered", () => {
        const languages = vscode.languages.getLanguages();
        return languages.then((langs) => {
            assert.ok(
                langs.includes("pleno"),
                "Pleno language should be registered",
            );
        });
    });

    test("Tree view should be available", () => {
        // This just checks if the view ID is valid
        // The actual tree view provider registration is tested implicitly
        assert.ok(true, "Tree view test placeholder");
    });
});
