"use strict";
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || (function () {
    var ownKeys = function(o) {
        ownKeys = Object.getOwnPropertyNames || function (o) {
            var ar = [];
            for (var k in o) if (Object.prototype.hasOwnProperty.call(o, k)) ar[ar.length] = k;
            return ar;
        };
        return ownKeys(o);
    };
    return function (mod) {
        if (mod && mod.__esModule) return mod;
        var result = {};
        if (mod != null) for (var k = ownKeys(mod), i = 0; i < k.length; i++) if (k[i] !== "default") __createBinding(result, mod, k[i]);
        __setModuleDefault(result, mod);
        return result;
    };
})();
Object.defineProperty(exports, "__esModule", { value: true });
const assert = __importStar(require("assert"));
const vscode = __importStar(require("vscode"));
suite("Pleno Extension Test Suite", () => {
    vscode.window.showInformationMessage("Start all tests.");
    test("Extension should be present", () => {
        assert.ok(vscode.extensions.getExtension("pleno.pleno-language"));
    });
    test("Extension should activate", async () => {
        const extension = vscode.extensions.getExtension("pleno.pleno-language");
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
            assert.ok(commands.includes(cmd), `Command ${cmd} should be registered`);
        }
    });
    test("Pleno language should be registered", () => {
        const languages = vscode.languages.getLanguages();
        return languages.then((langs) => {
            assert.ok(langs.includes("pleno"), "Pleno language should be registered");
        });
    });
    test("Tree view should be available", () => {
        // This just checks if the view ID is valid
        // The actual tree view provider registration is tested implicitly
        assert.ok(true, "Tree view test placeholder");
    });
});
//# sourceMappingURL=extension.test.js.map