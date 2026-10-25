import * as vscode from "vscode";
import * as path from "path";
import * as fs from "fs";
import {
    LanguageClient,
    LanguageClientOptions,
    ServerOptions,
    State,
    TransportKind,
} from "vscode-languageclient/node";
import { restorePatchedIconThemes } from "./iconInjector";
import { clearToolCache, findTool, getPlenoPath, Tool } from "./utils";
import {
    exposeToolchainToTerminals,
    installedVersion,
    installToolchain,
    uninstallToolchain,
} from "./toolchain";
import { PlenoOutlineProvider, PlenoProjectProvider } from "./projectExplorer";
import { PlenoDashboardPanel } from "./dashboard";
import { PlenoFormattingProvider } from "./formatter";
import { PlenoDiagnosticsProvider } from "./diagnostics";
import {
    PlenoDebugAdapterFactory,
    PlenoDebugConfigurationProvider,
} from "./debugAdapter";
import { PlenoTestController } from "./testController";

const DOCS_URL = "https://pleno.mohirlab.uz/";
const SETUP_URL = "https://github.com/otabekoff/pleno#installation";

/**
 * Runs a Pleno tool as a task in the shared terminal panel. The executable is
 * started directly rather than through a shell, so paths with spaces work in
 * every shell.
 */
function runToolTask(
    context: vscode.ExtensionContext,
    tool: Tool,
    args: string[],
    name: string,
    cwd?: string,
): Thenable<vscode.TaskExecution> {
    const task = new vscode.Task(
        { type: "pleno", command: args.join(" ") },
        vscode.workspace.workspaceFolders?.[0] ?? vscode.TaskScope.Workspace,
        name,
        "pleno",
        new vscode.ProcessExecution(findTool(context, tool).path, args, { cwd }),
    );
    task.presentationOptions = {
        reveal: vscode.TaskRevealKind.Always,
        panel: vscode.TaskPanelKind.Shared,
        clear: true,
        showReuseMessage: false,
    };
    return vscode.tasks.executeTask(task);
}

class PlmTaskProvider implements vscode.TaskProvider {
    static PlmType = "plm";

    constructor(private context: vscode.ExtensionContext) { }

    provideTasks(): vscode.ProviderResult<vscode.Task[]> {
        const tasks: vscode.Task[] = [];
        for (const folder of vscode.workspace.workspaceFolders ?? []) {
            if (!fs.existsSync(path.join(folder.uri.fsPath, "pleno.toml"))) {
                continue;
            }
            tasks.push(this.createTask("build", folder, vscode.TaskGroup.Build));
            tasks.push(this.createTask("build --release", folder, vscode.TaskGroup.Build));
            tasks.push(this.createTask("install", folder));
            tasks.push(this.createTask("update", folder));
            tasks.push(this.createTask("test", folder, vscode.TaskGroup.Test));
            tasks.push(this.createTask("run test", folder, vscode.TaskGroup.Test));
        }
        return tasks;
    }

    resolveTask(task: vscode.Task): vscode.Task | undefined {
        const command: string | undefined = task.definition.command;
        if (!command) {
            return undefined;
        }
        return new vscode.Task(
            task.definition,
            task.scope ?? vscode.TaskScope.Workspace,
            `plm ${command}`,
            "plm",
            this.execution(command),
        );
    }

    private execution(command: string, cwd?: string): vscode.ProcessExecution {
        // `pleno plm ...` is the same as `plm ...` and needs only one binary.
        return new vscode.ProcessExecution(
            getPlenoPath(this.context),
            ["plm", ...command.split(" ")],
            { cwd },
        );
    }

    private createTask(
        command: string,
        folder: vscode.WorkspaceFolder,
        group?: vscode.TaskGroup,
    ): vscode.Task {
        const task = new vscode.Task(
            { type: PlmTaskProvider.PlmType, command },
            folder,
            `plm ${command}`,
            "plm",
            this.execution(command, folder.uri.fsPath),
        );
        if (group) {
            task.group = group;
        }
        return task;
    }
}

class PlenoTaskProvider implements vscode.TaskProvider {
    static PlenoType = "pleno";

    constructor(private context: vscode.ExtensionContext) { }

    provideTasks(): vscode.ProviderResult<vscode.Task[]> {
        const editor = vscode.window.activeTextEditor;
        if (!editor || editor.document.languageId !== "pleno") {
            return [];
        }
        const file = editor.document.fileName;
        return ["run", "check"].map((command) =>
            this.createTask({ type: PlenoTaskProvider.PlenoType, command, file })
        );
    }

    resolveTask(task: vscode.Task): vscode.Task | undefined {
        if (!task.definition.command) {
            return undefined;
        }
        return this.createTask(task.definition, task.scope);
    }

    private createTask(
        definition: vscode.TaskDefinition,
        scope?: vscode.WorkspaceFolder | vscode.TaskScope,
    ): vscode.Task {
        const file: string = definition.file || "${file}";
        return new vscode.Task(
            definition,
            scope ?? vscode.TaskScope.Workspace,
            `pleno ${definition.command}`,
            "pleno",
            new vscode.ProcessExecution(getPlenoPath(this.context), [
                definition.command,
                file,
            ]),
        );
    }
}

let client: LanguageClient | undefined;
let statusBarItem: vscode.StatusBarItem;
let outputChannel: vscode.OutputChannel;
let diagnosticsProvider: PlenoDiagnosticsProvider | undefined;
let formatterRegistration: vscode.Disposable | undefined;
let testController: PlenoTestController;
let serverState: State | undefined;
let statusContext: vscode.ExtensionContext | undefined;

export function activate(context: vscode.ExtensionContext) {
    try {
        return activateExtension(context);
    } catch (e) {
        console.error("[Pleno] Activation failed:", e);
        vscode.window.showErrorMessage(
            `Failed to activate Pleno extension: ${e}`,
        );
    }
}

function activateExtension(context: vscode.ExtensionContext) {
    outputChannel = vscode.window.createOutputChannel("Pleno");
    outputChannel.appendLine("Pleno extension activating...");
    context.subscriptions.push(outputChannel);

    exposeToolchainToTerminals(context);

    // Tree views
    const rootPath = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
    const projectProvider = new PlenoProjectProvider(context, rootPath);
    const outlineProvider = new PlenoOutlineProvider();
    context.subscriptions.push(
        vscode.window.createTreeView("plenoProject", {
            treeDataProvider: projectProvider,
            showCollapseAll: true,
        }),
        vscode.window.createTreeView("plenoOutline", {
            treeDataProvider: outlineProvider,
            showCollapseAll: true,
        }),
    );

    // Debugging
    context.subscriptions.push(
        vscode.debug.registerDebugConfigurationProvider(
            "pleno",
            new PlenoDebugConfigurationProvider(context),
        ),
        vscode.debug.registerDebugAdapterDescriptorFactory(
            "pleno",
            new PlenoDebugAdapterFactory(context, outputChannel),
        ),
    );

    testController = new PlenoTestController(context, outputChannel);

    context.subscriptions.push(
        vscode.tasks.registerTaskProvider(
            PlmTaskProvider.PlmType,
            new PlmTaskProvider(context),
        ),
        vscode.tasks.registerTaskProvider(
            PlenoTaskProvider.PlenoType,
            new PlenoTaskProvider(context),
        ),
    );

    registerCommands(context, projectProvider);

    restorePatchedIconThemes(context);

    statusBarItem = vscode.window.createStatusBarItem(
        vscode.StatusBarAlignment.Right,
        100,
    );
    statusBarItem.command = "pleno.showControlPanel";
    context.subscriptions.push(statusBarItem);

    // Changing a tool path takes effect without reloading the window.
    context.subscriptions.push(
        vscode.workspace.onDidChangeConfiguration((event) => {
            if (
                event.affectsConfiguration("pleno.lsp.path") ||
                event.affectsConfiguration("pleno.cli.path") ||
                event.affectsConfiguration("pleno.dap.path")
            ) {
                clearToolCache();
                void startTooling(context);
            }
        }),
    );

    void startTooling(context).then(() => {
        const lsp = findTool(context, "pleno-lsp");
        const cli = findTool(context, "pleno");
        if (!lsp.found || !cli.found) {
            void offerInstall(context, [
                ...(cli.found ? [] : ["pleno"]),
                ...(lsp.found ? [] : ["pleno-lsp"]),
            ]);
        }
    });
}

/**
 * Starts (or restarts) the language server when it is installed. Without it,
 * diagnostics and formatting fall back to the `pleno` command line tool.
 */
async function startTooling(context: vscode.ExtensionContext): Promise<void> {
    const lsp = findTool(context, "pleno-lsp");
    const cli = findTool(context, "pleno");
    outputChannel.appendLine(
        `Language server: ${lsp.found ? lsp.path : "not found"}`,
    );
    outputChannel.appendLine(`CLI: ${cli.found ? cli.path : "not found"}`);

    if (client) {
        await client.stop().catch(() => undefined);
        client = undefined;
    }
    if (lsp.found) {
        await startLanguageServer(lsp.path);
    }

    const useFallback = !client && cli.found;
    if (useFallback && !diagnosticsProvider) {
        diagnosticsProvider = new PlenoDiagnosticsProvider(context, outputChannel);
        diagnosticsProvider.register();
        formatterRegistration = vscode.languages.registerDocumentFormattingEditProvider(
            "pleno",
            new PlenoFormattingProvider(context),
        );
    } else if (!useFallback && diagnosticsProvider) {
        diagnosticsProvider.dispose();
        diagnosticsProvider = undefined;
        formatterRegistration?.dispose();
        formatterRegistration = undefined;
    }

    updateStatus(context);
}

function updateStatus(context: vscode.ExtensionContext): void {
    statusContext = context;
    const lsp = findTool(context, "pleno-lsp");
    const cli = findTool(context, "pleno");
    const version = installedVersion(context);
    if (client && serverState === State.Starting) {
        statusBarItem.text = "$(sync~spin) Pleno";
        statusBarItem.tooltip = "The Pleno language server is starting...";
        statusBarItem.backgroundColor = undefined;
    } else if (client && serverState === State.Stopped) {
        statusBarItem.text = "$(debug-disconnect) Pleno";
        statusBarItem.tooltip = "The Pleno language server stopped. Click for options, including restarting it.";
        statusBarItem.backgroundColor = new vscode.ThemeColor(
            "statusBarItem.warningBackground",
        );
    } else if (client && cli.found) {
        statusBarItem.text = "$(check) Pleno";
        statusBarItem.tooltip = `Pleno is ready${version ? ` (${version})` : ""}: the language server is running. Click for options.`;
        statusBarItem.backgroundColor = undefined;
    } else if (cli.found) {
        statusBarItem.text = "$(warning) Pleno";
        statusBarItem.tooltip = lsp.found
            ? "The Pleno language server did not start. See the Pleno output."
            : "pleno-lsp was not found: diagnostics and formatting use the CLI. Click for options.";
        statusBarItem.backgroundColor = new vscode.ThemeColor(
            "statusBarItem.warningBackground",
        );
    } else {
        statusBarItem.text = "$(error) Pleno";
        statusBarItem.tooltip = "The Pleno toolchain was not found. Click to install it.";
        statusBarItem.backgroundColor = new vscode.ThemeColor(
            "statusBarItem.errorBackground",
        );
    }
    statusBarItem.show();
}

async function offerInstall(
    context: vscode.ExtensionContext,
    missing: string[],
): Promise<void> {
    const choice = await vscode.window.showWarningMessage(
        `${missing.join(" and ")} ${missing.length > 1 ? "were" : "was"} not found. Install the Pleno toolchain?`,
        "Install",
        "Set Path",
        "Instructions",
    );
    if (choice === "Install") {
        await vscode.commands.executeCommand("pleno.installToolchain");
    } else if (choice === "Set Path") {
        await vscode.commands.executeCommand(
            "workbench.action.openSettings",
            "pleno path",
        );
    } else if (choice === "Instructions") {
        await vscode.env.openExternal(vscode.Uri.parse(SETUP_URL));
    }
}

/** The active Pleno file, saved first so the CLI sees the current text. */
async function activePlenoFile(): Promise<string | undefined> {
    const editor = vscode.window.activeTextEditor;
    if (!editor || editor.document.languageId !== "pleno") {
        vscode.window.showWarningMessage("Open a Pleno file first.");
        return undefined;
    }
    if (editor.document.isUntitled) {
        vscode.window.showWarningMessage("Save the file before running it.");
        return undefined;
    }
    if (editor.document.isDirty) {
        await editor.document.save();
    }
    return editor.document.fileName;
}

function registerCommands(
    context: vscode.ExtensionContext,
    projectProvider: PlenoProjectProvider,
): void {
    const register = (command: string, callback: (...args: any[]) => any) =>
        context.subscriptions.push(
            vscode.commands.registerCommand(command, callback),
        );

    register("pleno.showControlPanel", async () => {
        const items: (vscode.QuickPickItem & { command: string })[] = [
            { label: "$(play) Run Current File", description: "pleno run", command: "pleno.runFile" },
            { label: "$(debug-alt) Start Debugging", description: "Debug the current file", command: "pleno.startDebugging" },
            { label: "$(terminal) Open Interactive REPL", description: "pleno repl", command: "pleno.openRepl" },
            { label: "$(cloud-download) Install or Update Toolchain", description: "Download the latest release", command: "pleno.installToolchain" },
            { label: "$(refresh) Restart Language Server", description: "pleno-lsp", command: "pleno.restartServer" },
            { label: "$(info) Show Status", description: "Toolchain paths and versions", command: "pleno.showStatus" },
            { label: "$(output) Show Output", description: "Extension logs", command: "pleno.showOutput" },
            { label: "$(rocket) Getting Started", description: "Walkthrough", command: "pleno.openWalkthrough" },
            { label: "$(book) Open Documentation", description: DOCS_URL, command: "pleno.openDocs" },
            { label: "$(dashboard) Open Dashboard", description: "Project dashboard", command: "pleno.showDashboard" },
        ];
        const selected = await vscode.window.showQuickPick(items, {
            placeHolder: "Pleno",
            matchOnDescription: true,
        });
        if (selected) {
            await vscode.commands.executeCommand(selected.command);
        }
    });

    register("pleno.showOutput", () => outputChannel.show());

    register("pleno.runFile", async () => {
        const file = await activePlenoFile();
        if (file) {
            await runToolTask(context, "pleno", ["run", file], `run ${path.basename(file)}`, path.dirname(file));
        }
    });

    register("pleno.checkFile", async () => {
        const file = await activePlenoFile();
        if (file) {
            await runToolTask(context, "pleno", ["check", file], `check ${path.basename(file)}`, path.dirname(file));
        }
    });

    register("pleno.restartServer", async () => {
        await vscode.window.withProgress({
            location: vscode.ProgressLocation.Notification,
            title: "Restarting the Pleno language server...",
        }, () => {
            clearToolCache();
            return startTooling(context);
        });
        if (client) {
            vscode.window.showInformationMessage("Pleno language server restarted.");
        } else {
            void offerInstall(context, ["pleno-lsp"]);
        }
    });

    register("pleno.installToolchain", async () => {
        const version = await installToolchain(context, outputChannel);
        if (version) {
            await startTooling(context);
            vscode.window.showInformationMessage(
                `Pleno ${version} is installed. New terminals have pleno, plm and pleno-repl on PATH.`,
            );
        }
    });

    register("pleno.uninstallToolchain", async () => {
        const confirm = await vscode.window.showWarningMessage(
            "Remove the Pleno toolchain installed by this extension?",
            { modal: true },
            "Remove",
        );
        if (confirm === "Remove") {
            if (client) {
                await client.stop().catch(() => undefined);
                client = undefined;
            }
            if (await uninstallToolchain(context)) {
                await startTooling(context);
                vscode.window.showInformationMessage("The Pleno toolchain was removed.");
            }
        }
    });

    register("pleno.openDocs", () =>
        vscode.env.openExternal(vscode.Uri.parse(DOCS_URL))
    );

    register("pleno.openRepl", () => {
        const repl = findTool(context, "pleno-repl");
        const terminal = repl.found
            ? vscode.window.createTerminal({ name: "Pleno REPL", shellPath: repl.path })
            : vscode.window.createTerminal({
                name: "Pleno REPL",
                shellPath: getPlenoPath(context),
                shellArgs: ["repl"],
            });
        terminal.show();
    });

    register("pleno.runScript", async (scriptName?: string) => {
        const name = scriptName ?? await vscode.window.showInputBox({
            prompt: "Script from pleno.toml to run",
            placeHolder: "build, test, ...",
        });
        if (name) {
            await runToolTask(
                context,
                "pleno",
                ["plm", "run", name],
                `plm run ${name}`,
                vscode.workspace.workspaceFolders?.[0]?.uri.fsPath,
            );
        }
    });

    register("pleno.showDashboard", () =>
        PlenoDashboardPanel.createOrShow(context.extensionUri)
    );

    register("pleno.openWalkthrough", () =>
        vscode.commands.executeCommand(
            "workbench.action.openWalkthrough",
            `${context.extension.id}#pleno-welcome`,
            false,
        )
    );

    register("pleno.openManifest", async () => {
        const folder = vscode.workspace.workspaceFolders?.[0];
        if (!folder) {
            vscode.window.showErrorMessage("No workspace folder is open.");
            return;
        }
        const manifest = vscode.Uri.joinPath(folder.uri, "pleno.toml");
        try {
            await vscode.workspace.fs.stat(manifest);
            await vscode.window.showTextDocument(manifest);
        } catch {
            vscode.window.showErrorMessage("There is no pleno.toml in the workspace root.");
        }
    });

    register("pleno.reloadWorkspace", async () => {
        clearToolCache();
        await startTooling(context);
        projectProvider.refresh();
        testController?.refresh();
        vscode.window.showInformationMessage("Pleno workspace reloaded.");
    });

    register("pleno.showStatus", () => {
        const describe = (tool: Tool) => {
            const location = findTool(context, tool);
            return location.found
                ? `${location.path} (${location.source})`
                : "not found";
        };
        outputChannel.show();
        outputChannel.appendLine("");
        outputChannel.appendLine("=== Pleno Status ===");
        outputChannel.appendLine(`Extension:        ${context.extension.packageJSON.version}`);
        outputChannel.appendLine(`Toolchain:        ${installedVersion(context) ?? "not installed by the extension"}`);
        outputChannel.appendLine(`pleno:            ${describe("pleno")}`);
        outputChannel.appendLine(`pleno-lsp:        ${describe("pleno-lsp")}`);
        outputChannel.appendLine(`pleno-dap:        ${describe("pleno-dap")}`);
        outputChannel.appendLine(`pleno-repl:       ${describe("pleno-repl")}`);
        outputChannel.appendLine(`Language server:  ${client ? "running" : "not running"}`);
        outputChannel.appendLine(`Workspace:        ${vscode.workspace.workspaceFolders?.[0]?.uri.fsPath ?? "none"}`);
        outputChannel.appendLine("====================");
    });

    register("pleno.refreshProject", () => projectProvider.refresh());

    register("pleno.startDebugging", () => {
        const editor = vscode.window.activeTextEditor;
        if (editor && editor.document.languageId === "pleno") {
            vscode.debug.startDebugging(undefined, {
                type: "pleno",
                name: "Debug Pleno",
                request: "launch",
                program: editor.document.fileName,
                stopOnEntry: true,
            });
        } else {
            vscode.window.showWarningMessage(
                "Open a Pleno file to start debugging.",
            );
        }
    });

    register("pleno.createProject", async () => {
        const folder = vscode.workspace.workspaceFolders?.[0];
        if (!folder) {
            vscode.window.showErrorMessage("Open a folder to create the project in.");
            return;
        }
        const name = await vscode.window.showInputBox({
            prompt: "Project name",
            placeHolder: "my-pleno-project",
            validateInput: (value) =>
                /^[A-Za-z0-9_-]+$/.test(value) ? undefined : "Use letters, digits, '-' and '_'.",
        });
        if (name) {
            await runToolTask(context, "pleno", ["plm", "init", name], `plm init ${name}`, folder.uri.fsPath);
        }
    });
}

async function startLanguageServer(serverPath: string): Promise<void> {
    const serverOptions: ServerOptions = {
        run: { command: serverPath, transport: TransportKind.stdio },
        debug: { command: serverPath, transport: TransportKind.stdio },
    };

    const config = vscode.workspace.getConfiguration("pleno");
    const clientOptions: LanguageClientOptions = {
        documentSelector: [{ scheme: "file", language: "pleno" }],
        synchronize: {
            fileEvents: vscode.workspace.createFileSystemWatcher("**/*.pln"),
            // Sends `pleno.*` settings changes to the server, which re-lints.
            configurationSection: "pleno",
        },
        initializationOptions: {
            lint: {
                enabled: config.get("lint.enabled", true),
                maxLineLength: config.get("lint.maxLineLength", 100),
                rules: config.get("lint.rules", {}),
            },
            inlayHints: {
                enabled: config.get("inlayHints.enabled", true),
            },
            semanticHighlighting: {
                enabled: config.get("semanticHighlighting.enabled", true),
            },
        },
        outputChannel,
    };

    const languageClient = new LanguageClient(
        "plenoLanguageServer",
        "Pleno Language Server",
        serverOptions,
        clientOptions,
    );
    // Keep the status bar in step when the server stops or restarts later.
    languageClient.onDidChangeState((event) => {
        serverState = event.newState;
        if (statusContext) {
            updateStatus(statusContext);
        }
    });
    try {
        await languageClient.start();
        client = languageClient;
        outputChannel.appendLine(`Language server started: ${serverPath}`);
    } catch (error) {
        outputChannel.appendLine(`Language server failed to start: ${error}`);
    }
}

export function deactivate(): Thenable<void> | undefined {
    diagnosticsProvider?.dispose();
    formatterRegistration?.dispose();
    testController?.dispose();
    return client?.stop();
}
