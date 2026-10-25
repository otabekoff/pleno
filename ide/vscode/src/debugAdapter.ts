import * as vscode from "vscode";
import * as path from "path";
import { findTool } from "./utils";

/**
 * Starts the `pleno-dap` debug adapter, which talks the Debug Adapter Protocol
 * over stdin and stdout, or `pleno dap` (the same adapter built into the CLI)
 * when `pleno-dap` is not installed. The adapter runs the program in its own
 * process, so the launch configuration's `cwd` and `env` are applied to it.
 */
export class PlenoDebugAdapterFactory implements vscode.DebugAdapterDescriptorFactory {
    constructor(private readonly context: vscode.ExtensionContext) { }

    createDebugAdapterDescriptor(
        session: vscode.DebugSession,
    ): vscode.ProviderResult<vscode.DebugAdapterDescriptor> {
        const config = session.configuration;
        const options: vscode.DebugAdapterExecutableOptions = {
            cwd: config.cwd,
            env: config.env,
        };
        const dap = findTool(this.context, "pleno-dap");
        if (dap.found) {
            return new vscode.DebugAdapterExecutable(dap.path, [], options);
        }
        const pleno = findTool(this.context, "pleno");
        if (pleno.found) {
            return new vscode.DebugAdapterExecutable(pleno.path, ["dap"], options);
        }
        throw new Error(
            "Neither pleno-dap nor pleno was found. Run 'Pleno: Install or Update Toolchain' or set 'pleno.dap.path'.",
        );
    }
}

/**
 * Fills in a launch configuration: F5 without a launch.json debugs the
 * active file, and a relative `program` is resolved against the folder.
 */
export class PlenoDebugConfigurationProvider implements vscode.DebugConfigurationProvider {
    resolveDebugConfiguration(
        folder: vscode.WorkspaceFolder | undefined,
        config: vscode.DebugConfiguration,
    ): vscode.ProviderResult<vscode.DebugConfiguration> {
        if (!config.type && !config.request && !config.name) {
            const editor = vscode.window.activeTextEditor;
            if (editor?.document.languageId !== "pleno") {
                return undefined;
            }
            config.type = "pleno";
            config.request = "launch";
            config.name = "Debug current file";
            config.program = "${file}";
        }
        if (!config.program) {
            return vscode.window.showErrorMessage(
                "Set \"program\" in the launch configuration to the .pln file to debug.",
            ).then(() => undefined);
        }
        config.cwd ??= folder?.uri.fsPath;
        return config;
    }

    resolveDebugConfigurationWithSubstitutedVariables(
        folder: vscode.WorkspaceFolder | undefined,
        config: vscode.DebugConfiguration,
    ): vscode.ProviderResult<vscode.DebugConfiguration> {
        const base = config.cwd ?? folder?.uri.fsPath;
        if (base && !path.isAbsolute(config.program)) {
            config.program = path.resolve(base, config.program);
        }
        config.cwd ??= path.dirname(config.program);
        return config;
    }
}
