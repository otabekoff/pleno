import * as vscode from "vscode";
import * as path from "path";
import * as fs from "fs";
import {
    LanguageClient,
    LanguageClientOptions,
    ServerOptions,
    TransportKind,
} from "vscode-languageclient/node";
import { injectPlenoIcons } from "./iconInjector";
import { getPlenoLspPath, getPlenoPath } from "./utils";
import { PlenoOutlineProvider, PlenoProjectProvider } from "./projectExplorer";
import { PlenoDashboardPanel } from "./dashboard";
import { PlenoFormattingProvider } from "./formatter";
import {
    PlenoCodeActionProvider,
    PlenoDiagnosticsProvider,
} from "./diagnostics";
import {
    PlenoDebugAdapterFactory,
    PlenoDebugConfigurationProvider,
} from "./debugAdapter";
import { PlenoTestController } from "./testController";

class PlmTaskProvider implements vscode.TaskProvider {
    static PlmType = "plm";

    provideTasks(): vscode.ProviderResult<vscode.Task[]> {
        const workspaceFolders = vscode.workspace.workspaceFolders;
        if (!workspaceFolders || workspaceFolders.length === 0) {
            return [];
        }

        const tasks: vscode.Task[] = [];
        for (const folder of workspaceFolders) {
            const manifestPath = path.join(folder.uri.fsPath, "pleno.toml");
            if (fs.existsSync(manifestPath)) {
                // Basic plm commands
                tasks.push(
                    this.createTask(
                        "build",
                        "plm build",
                        folder,
                        vscode.TaskGroup.Build,
                    ),
                );
                tasks.push(this.createTask("install", "plm install", folder));
                tasks.push(this.createTask("update", "plm update", folder));

                // Build variants
                tasks.push(
                    this.createTask(
                        "build --debug",
                        "plm build --debug",
                        folder,
                        vscode.TaskGroup.Build,
                    ),
                );
                tasks.push(
                    this.createTask(
                        "build --release",
                        "plm build --release",
                        folder,
                        vscode.TaskGroup.Build,
                    ),
                );
                tasks.push(
                    this.createTask(
                        "build --test",
                        "plm build --test",
                        folder,
                        vscode.TaskGroup.Build,
                    ),
                );

                tasks.push(
                    this.createTask(
                        "test",
                        "plm test",
                        folder,
                        vscode.TaskGroup.Test,
                    ),
                );

                // Script-based test (runs 'test' script from pleno.toml)
                tasks.push(
                    this.createTask(
                        "run test",
                        "plm run test",
                        folder,
                        vscode.TaskGroup.Test,
                    ),
                );
            }
        }
        return tasks;
    }

    resolveTask(task: vscode.Task): vscode.Task | undefined {
        const command = task.definition.command;
        if (command) {
            return new vscode.Task(
                task.definition,
                task.scope ?? vscode.TaskScope.Workspace,
                command,
                "plm",
                new vscode.ShellExecution(`plm ${command}`),
            );
        }
        return undefined;
    }

    private createTask(
        command: string,
        shellCommand: string,
        folder: vscode.WorkspaceFolder,
        group?: vscode.TaskGroup,
    ): vscode.Task {
        const definition = { type: PlmTaskProvider.PlmType, command };
        const task = new vscode.Task(
            definition,
            folder,
            `plm ${command}`,
            "plm",
            new vscode.ShellExecution(shellCommand),
        );
        if (group) {
            task.group = group;
        }
        return task;
    }
}

class PlenoTaskProvider implements vscode.TaskProvider {
    static PlenoType = "pleno";

    provideTasks(): vscode.ProviderResult<vscode.Task[]> {
        const editor = vscode.window.activeTextEditor;
        if (!editor || editor.document.languageId !== "pleno") {
            return [];
        }

        const tasks: vscode.Task[] = [];
        const file = editor.document.fileName;
        tasks.push(
            new vscode.Task(
                { type: PlenoTaskProvider.PlenoType, command: "run", file },
                vscode.TaskScope.Workspace,
                "pleno run",
                "pleno",
                new vscode.ShellExecution(`pleno run "${file}"`),
            ),
        );
        tasks.push(
            new vscode.Task(
                { type: PlenoTaskProvider.PlenoType, command: "check", file },
                vscode.TaskScope.Workspace,
                "pleno check",
                "pleno",
                new vscode.ShellExecution(`pleno check "${file}"`),
            ),
        );
        return tasks;
    }

    resolveTask(task: vscode.Task): vscode.Task | undefined {
        const command = task.definition.command;
        const file = task.definition.file || "${file}";
        if (command) {
            return new vscode.Task(
                task.definition,
                task.scope ?? vscode.TaskScope.Workspace,
                command,
                "pleno",
                new vscode.ShellExecution(`pleno ${command} "${file}"`),
            );
        }
        return undefined;
    }
}

let client: LanguageClient;
let myStatusBarItem: vscode.StatusBarItem;
let outputChannel: vscode.OutputChannel;
let diagnosticsProvider: PlenoDiagnosticsProvider;
let testController: PlenoTestController;

export function activate(context: vscode.ExtensionContext) {
    console.log("[Pleno] activate() called");
    try {
        return _activate(context);
    } catch (e) {
        console.error("[Pleno] Activation failed:", e);
        vscode.window.showErrorMessage(
            `Failed to activate Pleno extension: ${e}`,
        );
    }
}

function _activate(context: vscode.ExtensionContext) {
    console.log("[Pleno] Entering _activate");

    // 0. Create output channel FIRST
    outputChannel = vscode.window.createOutputChannel("Pleno");
    outputChannel.appendLine("Pleno extension activating...");
    context.subscriptions.push(outputChannel);

    // Tool check setup
    const lspPath = getPlenoLspPath(context);
    const cliPath = getPlenoPath(context);
    const lspFound = lspPath.includes("pleno-lsp");
    const cliFound = cliPath.includes("pleno");

    outputChannel.appendLine(`LSP path: ${lspPath}`);
    outputChannel.appendLine(`CLI path: ${cliPath}`);

    // 1. Register formatter
    context.subscriptions.push(
        vscode.languages.registerDocumentFormattingEditProvider(
            "pleno",
            new PlenoFormattingProvider(context),
        ),
    );

    // 2. Register diagnostics provider (linting)
    diagnosticsProvider = new PlenoDiagnosticsProvider(context, outputChannel);
    context.subscriptions.push(...diagnosticsProvider.register());

    // 3. Register code action provider (quick fixes)
    context.subscriptions.push(
        vscode.languages.registerCodeActionsProvider(
            "pleno",
            new PlenoCodeActionProvider(),
            {
                providedCodeActionKinds:
                    PlenoCodeActionProvider.providedCodeActionKinds,
            },
        ),
    );

    // 4. Register tree data providers
    const rootPath = (vscode.workspace.workspaceFolders &&
        (vscode.workspace.workspaceFolders.length > 0))
        ? vscode.workspace.workspaceFolders[0].uri.fsPath
        : undefined;

    const projectProvider = new PlenoProjectProvider(context, rootPath);
    const outlineProvider = new PlenoOutlineProvider();

    // Use createTreeView for more control
    const projectTreeView = vscode.window.createTreeView("plenoProject", {
        treeDataProvider: projectProvider,
        showCollapseAll: true,
    });
    context.subscriptions.push(projectTreeView);

    const outlineTreeView = vscode.window.createTreeView("plenoOutline", {
        treeDataProvider: outlineProvider,
        showCollapseAll: true,
    });
    context.subscriptions.push(outlineTreeView);

    // 5. Register debug adapter
    context.subscriptions.push(
        vscode.debug.registerDebugConfigurationProvider(
            "pleno",
            new PlenoDebugConfigurationProvider(context),
        ),
    );
    context.subscriptions.push(
        vscode.debug.registerDebugAdapterDescriptorFactory(
            "pleno",
            new PlenoDebugAdapterFactory(context, outputChannel),
        ),
    );

    // 6. Register test controller
    testController = new PlenoTestController(context, outputChannel);

    // 7. Register task providers
    context.subscriptions.push(
        vscode.tasks.registerTaskProvider(
            PlmTaskProvider.PlmType,
            new PlmTaskProvider(),
        ),
    );
    context.subscriptions.push(
        vscode.tasks.registerTaskProvider(
            PlenoTaskProvider.PlenoType,
            new PlenoTaskProvider(),
        ),
    );

    // 8. Register ALL commands
    registerCommands(
        context,
        outputChannel,
        projectProvider,
        outlineProvider,
        cliFound,
    );

    // Try to inject icon theme patches in background
    injectPlenoIcons(context).catch((e) =>
        outputChannel.appendLine(`Icon injection failed: ${e}`)
    );

    // Create status bar item
    myStatusBarItem = vscode.window.createStatusBarItem(
        vscode.StatusBarAlignment.Right,
        100,
    );
    myStatusBarItem.text = `$(zap) Pleno`;
    myStatusBarItem.tooltip = "Pleno Extension Control (Click for options)";
    myStatusBarItem.command = "pleno.showControlPanel";
    myStatusBarItem.show();
    context.subscriptions.push(myStatusBarItem);

    const updateStatus = () => {
        if (lspFound && cliFound) {
            myStatusBarItem.text = `$(check) Pleno`;
            myStatusBarItem.tooltip = "Pleno toolchain (LSP & CLI) is ready.";
            myStatusBarItem.backgroundColor = undefined;
        } else if (lspFound) {
            myStatusBarItem.text = `$(warning) Pleno CLI`;
            myStatusBarItem.tooltip =
                "Pleno CLI not found. Some features may not work.";
            myStatusBarItem.backgroundColor = new vscode.ThemeColor(
                "statusBarItem.warningBackground",
            );
        } else if (cliFound) {
            myStatusBarItem.text = `$(error) Pleno LSP`;
            myStatusBarItem.tooltip =
                "Pleno LSP not found. Advanced features disabled.";
            myStatusBarItem.backgroundColor = new vscode.ThemeColor(
                "statusBarItem.errorBackground",
            );
        } else {
            myStatusBarItem.text = `$(error) Pleno`;
            myStatusBarItem.tooltip = "Pleno toolchain (CLI & LSP) not found.";
            myStatusBarItem.backgroundColor = new vscode.ThemeColor(
                "statusBarItem.errorBackground",
            );
        }
        myStatusBarItem.show();
    };

    updateStatus();

    if (!lspFound || !cliFound) {
        let missing = [];
        if (!lspFound) missing.push("LSP");
        if (!cliFound) missing.push("CLI");

        const msg = `Pleno ${missing.join(" & ")} not found.`;
        vscode.window.showWarningMessage(msg, "Help", "Retry").then(
            (selection) => {
                if (selection === "Help") {
                    vscode.commands.executeCommand(
                        "vscode.open",
                        vscode.Uri.parse(
                            "https://github.com/otabekoff/pleno/blob/main/ide/vscode/README.md",
                        ),
                    );
                } else if (selection === "Retry") {
                    vscode.commands.executeCommand("pleno.restartServer");
                }
            },
        );
    }

    if (lspFound && cliFound) {
        outputChannel.appendLine(
            `Pleno extension activated. LSP found at: ${lspPath}`,
        );
    } else if (lspFound) {
        outputChannel.appendLine(
            "Pleno CLI not found. Running and checking files may not work.",
        );
    } else if (cliFound) {
        outputChannel.appendLine(
            "Pleno LSP server not found. Advanced features will be disabled.",
        );
    } else {
        outputChannel.appendLine("Pleno toolchain (CLI & LSP) not found.");
    }

    // Start Language Server if found
    if (lspFound) {
        startLanguageServer(context, lspPath, outputChannel);
    }

    outputChannel.appendLine("Pleno extension activated successfully!");

    return {
        extendMarkdownIt(md: any) {
            const defaultRender = md.renderer.rules.fence ||
                function (
                    tokens: any[],
                    idx: number,
                    options: any,
                    env: any,
                    self: any,
                ) {
                    return self.renderToken(tokens, idx, options);
                };

            md.renderer.rules.fence = (
                tokens: any[],
                idx: number,
                options: any,
                env: any,
                self: any,
            ) => {
                const token = tokens[idx];
                const info = token.info ? token.info.trim() : "";
                const langName = info.split(/\s+/g)[0];

                if (langName === "pleno" || langName === "pln") {
                    const highlighted = escapeHtml(token.content);
                    return `<pre class="pleno-highlight"><code class="language-pleno">${highlighted}</code></pre>`;
                }

                return defaultRender(tokens, idx, options, env, self);
            };
            return md;
        },
    };
}

/**
 * Register all commands
 */
function registerCommands(
    context: vscode.ExtensionContext,
    outputChannel: vscode.OutputChannel,
    projectProvider: PlenoProjectProvider,
    _outlineProvider: PlenoOutlineProvider,
    _cliFound: boolean,
): void {
    // Control Panel
    context.subscriptions.push(
        vscode.commands.registerCommand("pleno.showControlPanel", async () => {
            const items: vscode.QuickPickItem[] = [
                {
                    label: "$(output) Show Output Channel",
                    description: "View Pleno extension logs",
                } as any,
                {
                    label: "$(info) Show Pleno Status",
                    description: "View toolchain status",
                } as any,
                {
                    label: "$(refresh) Reload Workspace",
                    description: "Reload Pleno workspace",
                } as any,
                {
                    label: "$(rocket) Open Getting Started Walkthrough",
                    description: "Learn Pleno basics",
                } as any,
                {
                    label: "$(book) Open Documentation",
                    description: "View online docs",
                } as any,
                {
                    label: "$(terminal) Open Interactive REPL",
                    description: "Start Pleno REPL",
                } as any,
                {
                    label: "$(dashboard) Open Dashboard",
                    description: "Open Pleno Dashboard",
                } as any,
                {
                    label: "$(debug-alt) Start Debugging",
                    description: "Debug current file",
                } as any,
            ];

            const selected = await vscode.window.showQuickPick(items, {
                placeHolder: "Pleno Control Panel",
                matchOnDescription: true,
            });

            if (selected) {
                const label = selected.label;
                if (label.includes("Output")) {
                    vscode.commands.executeCommand("pleno.showOutput");
                } else if (label.includes("Status")) {
                    vscode.commands.executeCommand("pleno.showStatus");
                } else if (label.includes("Reload")) {
                    vscode.commands.executeCommand("pleno.reloadWorkspace");
                } else if (label.includes("Walkthrough")) {
                    vscode.commands.executeCommand("pleno.openWalkthrough");
                } else if (label.includes("Documentation")) {
                    vscode.commands.executeCommand("pleno.openDocs");
                } else if (label.includes("REPL")) {
                    vscode.commands.executeCommand("pleno.openRepl");
                } else if (label.includes("Dashboard")) {
                    vscode.commands.executeCommand("pleno.showDashboard");
                } else if (label.includes("Debugging")) {
                    vscode.commands.executeCommand("pleno.startDebugging");
                }
            }
        }),
    );

    // Show Output Channel
    context.subscriptions.push(
        vscode.commands.registerCommand("pleno.showOutput", () => {
            outputChannel.show();
        }),
    );

    // Run Current File
    context.subscriptions.push(
        vscode.commands.registerCommand("pleno.runFile", () => {
            const editor = vscode.window.activeTextEditor;
            if (editor) {
                const terminal = vscode.window.activeTerminal ||
                    vscode.window.createTerminal("Pleno");
                terminal.show();
                terminal.sendText(
                    `${getPlenoPath(context)
                    } run "${editor.document.fileName}"`,
                );
            } else {
                vscode.window.showWarningMessage(
                    "No active editor. Open a Pleno file first.",
                );
            }
        }),
    );

    // Check Current File
    context.subscriptions.push(
        vscode.commands.registerCommand("pleno.checkFile", () => {
            const editor = vscode.window.activeTextEditor;
            if (editor) {
                // Also trigger diagnostics
                if (diagnosticsProvider) {
                    diagnosticsProvider.checkDocument(editor.document);
                }
                const terminal = vscode.window.activeTerminal ||
                    vscode.window.createTerminal("Pleno");
                terminal.show();
                terminal.sendText(
                    `${getPlenoPath(context)
                    } check "${editor.document.fileName}"`,
                );
            } else {
                vscode.window.showWarningMessage(
                    "No active editor. Open a Pleno file first.",
                );
            }
        }),
    );

    // Restart Language Server
    context.subscriptions.push(
        vscode.commands.registerCommand("pleno.restartServer", async () => {
            if (client) {
                await vscode.window.withProgress({
                    location: vscode.ProgressLocation.Notification,
                    title: "Restarting Pleno Language Server...",
                    cancellable: false,
                }, async (progress) => {
                    progress.report({ increment: 0 });
                    await client.stop();
                    progress.report({
                        increment: 50,
                        message: "Starting server...",
                    });
                    await client.start();
                    progress.report({ increment: 100 });
                });
                vscode.window.showInformationMessage(
                    "Pleno Language Server restarted.",
                );
            } else {
                vscode.window.showWarningMessage(
                    "Language Server is not running.",
                );
            }
        }),
    );

    // Open Documentation
    context.subscriptions.push(
        vscode.commands.registerCommand("pleno.openDocs", () => {
            vscode.env.openExternal(vscode.Uri.parse("https://pleno.run/docs"));
        }),
    );

    // Open REPL
    context.subscriptions.push(
        vscode.commands.registerCommand("pleno.openRepl", () => {
            const terminal = vscode.window.createTerminal("Pleno REPL");
            terminal.show();
            const plenoPath = getPlenoPath(context);
            const replPath = plenoPath.endsWith("pleno")
                ? plenoPath.replace(/pleno$/, "pleno-repl")
                : `${plenoPath}-repl`;
            terminal.sendText(replPath);
        }),
    );

    // Run Script
    context.subscriptions.push(
        vscode.commands.registerCommand(
            "pleno.runScript",
            (scriptName: string) => {
                if (!scriptName) {
                    vscode.window.showInputBox({
                        prompt: "Enter script name to run",
                        placeHolder: "build, test, etc.",
                    }).then((name) => {
                        if (name) {
                            const terminal = vscode.window.createTerminal(
                                "Pleno",
                            );
                            terminal.show();
                            terminal.sendText(
                                `${getPlenoPath(context)} plm run ${name}`,
                            );
                        }
                    });
                } else {
                    const terminal = vscode.window.createTerminal("Pleno");
                    terminal.show();
                    terminal.sendText(
                        `${getPlenoPath(context)} plm run ${scriptName}`,
                    );
                }
            },
        ),
    );

    // Show Dashboard
    context.subscriptions.push(
        vscode.commands.registerCommand("pleno.showDashboard", () => {
            PlenoDashboardPanel.createOrShow(context.extensionUri);
        }),
    );

    // Open Walkthrough
    context.subscriptions.push(
        vscode.commands.registerCommand("pleno.openWalkthrough", () => {
            vscode.commands.executeCommand(
                "workbench.action.openWalkthrough",
                "pleno.pleno-language#pleno-welcome",
                false,
            );
        }),
    );

    // Open Manifest
    context.subscriptions.push(
        vscode.commands.registerCommand("pleno.openManifest", async () => {
            const workspaceFolders = vscode.workspace.workspaceFolders;
            if (!workspaceFolders) {
                vscode.window.showErrorMessage("No workspace folder open");
                return;
            }
            const manifestPath = vscode.Uri.joinPath(
                workspaceFolders[0].uri,
                "pleno.toml",
            );
            try {
                await vscode.workspace.fs.stat(manifestPath);
                const doc = await vscode.workspace.openTextDocument(
                    manifestPath,
                );
                await vscode.window.showTextDocument(doc);
            } catch {
                vscode.window.showErrorMessage(
                    "pleno.toml not found in workspace root",
                );
            }
        }),
    );

    // Reload Workspace
    context.subscriptions.push(
        vscode.commands.registerCommand("pleno.reloadWorkspace", async () => {
            outputChannel.appendLine("Reloading Pleno workspace...");
            if (client) {
                await client.stop();
                await client.start();
            }
            projectProvider.refresh();
            if (testController) {
                testController.refresh();
            }
            vscode.window.showInformationMessage("Pleno workspace reloaded");
        }),
    );

    // Show Status
    context.subscriptions.push(
        vscode.commands.registerCommand("pleno.showStatus", () => {
            outputChannel.show();
            outputChannel.appendLine("");
            outputChannel.appendLine("=== Pleno Status ===");
            outputChannel.appendLine(
                `Extension Version: ${context.extension.packageJSON.version}`,
            );
            outputChannel.appendLine(`LSP Path: ${getPlenoLspPath(context)}`);
            outputChannel.appendLine(`CLI Path: ${getPlenoPath(context)}`);
            outputChannel.appendLine(
                `Workspace: ${vscode.workspace.workspaceFolders?.[0]?.uri.fsPath || "None"
                }`,
            );
            outputChannel.appendLine(
                `Language Server: ${client ? "Running" : "Not running"}`,
            );
            outputChannel.appendLine("====================");
        }),
    );

    // Refresh Project View
    context.subscriptions.push(
        vscode.commands.registerCommand("pleno.refreshProject", () => {
            projectProvider.refresh();
            vscode.window.showInformationMessage("Project view refreshed");
        }),
    );

    // Start Debugging
    context.subscriptions.push(
        vscode.commands.registerCommand("pleno.startDebugging", () => {
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
        }),
    );

    // Create Project (for welcome view)
    context.subscriptions.push(
        vscode.commands.registerCommand("pleno.createProject", async () => {
            const name = await vscode.window.showInputBox({
                prompt: "Enter project name",
                placeHolder: "my-pleno-project",
            });
            if (name) {
                const terminal = vscode.window.createTerminal("Pleno");
                terminal.show();
                terminal.sendText(`${getPlenoPath(context)} plm init ${name}`);
            }
        }),
    );
}

/**
 * Start the Language Server
 */
function startLanguageServer(
    context: vscode.ExtensionContext,
    serverPath: string,
    outputChannel: vscode.OutputChannel,
): void {
    outputChannel.appendLine(`Starting Language Server: ${serverPath}`);

    const serverOptions: ServerOptions = {
        run: { command: serverPath, transport: TransportKind.stdio },
        debug: { command: serverPath, transport: TransportKind.stdio },
    };

    const config = vscode.workspace.getConfiguration("pleno");

    const clientOptions: LanguageClientOptions = {
        documentSelector: [{ scheme: "file", language: "pleno" }],
        synchronize: {
            fileEvents: vscode.workspace.createFileSystemWatcher("**/*.pln"),
        },
        initializationOptions: {
            inlayHints: {
                enabled: config.get("inlayHints.enabled", true),
            },
            semanticHighlighting: {
                enabled: config.get("semanticHighlighting.enabled", true),
            },
        },
        outputChannel: outputChannel,
    };

    client = new LanguageClient(
        "plenoLanguageServer",
        "Pleno Language Server",
        serverOptions,
        clientOptions,
    );

    client.start();
    outputChannel.appendLine("Language Server started.");
}

function escapeHtml(unsafe: string) {
    return unsafe
        .replace(/&/g, "&amp;")
        .replace(/</g, "&lt;")
        .replace(/>/g, "&gt;")
        .replace(/"/g, "&quot;")
        .replace(/'/g, "&#039;");
}

export function deactivate(): Thenable<void> | undefined {
    if (diagnosticsProvider) {
        diagnosticsProvider.dispose();
    }
    if (testController) {
        testController.dispose();
    }
    if (!client) {
        return undefined;
    }
    return client.stop();
}
