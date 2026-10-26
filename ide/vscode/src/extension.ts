import * as vscode from "vscode";
import * as path from "path";
import { execFile } from "child_process";
import {
    LanguageClient,
    LanguageClientOptions,
    ServerOptions,
    State,
    TransportKind,
} from "vscode-languageclient/node";
import { restorePatchedIconThemes } from "./iconInjector";
import { clearToolCache, findTool, getPlenoPath, Tool, toolVersion } from "./utils";
import {
    exposeToolchainToTerminals,
    installedVersion,
    installToolchain,
    uninstallToolchain,
} from "./toolchain";
import { PlenoProjectProvider, Project, ProjectItem } from "./projectExplorer";
import { PlenoFormattingProvider } from "./formatter";
import { PlenoDiagnosticsProvider } from "./diagnostics";
import {
    PlenoDebugAdapterFactory,
    PlenoDebugConfigurationProvider,
} from "./debugAdapter";
import { PlenoTestController } from "./testController";

const DOCS_URL = "https://pleno.mohirlab.uz/";
const INSTALL_URL = "https://pleno.mohirlab.uz/guide/installation";

/** The folder to run a file in: its workspace folder, or its own folder. */
function folderOf(file: string): string {
    return vscode.workspace.getWorkspaceFolder(vscode.Uri.file(file))?.uri.fsPath ?? path.dirname(file);
}

function presentation(task: vscode.Task): vscode.Task {
    task.presentationOptions = {
        reveal: vscode.TaskRevealKind.Always,
        panel: vscode.TaskPanelKind.Shared,
        clear: true,
        showReuseMessage: false,
    };
    return task;
}

/**
 * `pleno run <file>` or `pleno check <file>`. The executable is started
 * directly rather than through a shell, so paths with spaces work in every
 * shell.
 */
function plenoFileTask(
    context: vscode.ExtensionContext,
    definition: vscode.TaskDefinition,
    scope?: vscode.WorkspaceFolder | vscode.TaskScope,
): vscode.Task {
    const file: string = definition.file || "${file}";
    const folder = scope && typeof scope === "object" ? scope : undefined;
    return presentation(new vscode.Task(
        definition,
        scope ?? vscode.TaskScope.Workspace,
        `${definition.command} ${definition.file ? path.basename(definition.file) : "current file"}`,
        "pleno",
        new vscode.ProcessExecution(getPlenoPath(context), [definition.command, file], {
            cwd: definition.file ? folderOf(definition.file) : folder?.uri.fsPath,
        }),
    ));
}

/** `plm <command>` in a project folder, run as `pleno plm ...` so only one binary is needed. */
function plmTask(
    context: vscode.ExtensionContext,
    command: string,
    folder: vscode.WorkspaceFolder,
    group?: vscode.TaskGroup,
): vscode.Task {
    const task = presentation(new vscode.Task(
        { type: "plm", command },
        folder,
        `plm ${command}`,
        "plm",
        new vscode.ProcessExecution(getPlenoPath(context), ["plm", ...command.split(" ")], {
            cwd: folder.uri.fsPath,
        }),
    ));
    task.group = group;
    return task;
}

/** Tasks for each project: build, test, install, update, and its scripts. */
class PlmTaskProvider implements vscode.TaskProvider {
    constructor(
        private readonly context: vscode.ExtensionContext,
        private readonly projects: PlenoProjectProvider,
    ) { }

    provideTasks(): vscode.Task[] {
        return this.projects.getProjects().flatMap((project) => [
            plmTask(this.context, "build", project.folder, vscode.TaskGroup.Build),
            plmTask(this.context, "build --release", project.folder, vscode.TaskGroup.Build),
            plmTask(this.context, "test", project.folder, vscode.TaskGroup.Test),
            plmTask(this.context, "install", project.folder),
            plmTask(this.context, "update", project.folder),
            ...Object.keys(project.scripts).map((script) =>
                plmTask(this.context, `run ${script}`, project.folder)),
        ]);
    }

    resolveTask(task: vscode.Task): vscode.Task | undefined {
        const command: string | undefined = task.definition.command;
        const folder = typeof task.scope === "object" ? task.scope : vscode.workspace.workspaceFolders?.[0];
        if (!command || !folder) {
            return undefined;
        }
        const resolved = plmTask(this.context, command, folder);
        // Keep the definition from tasks.json so VS Code matches the two.
        return new vscode.Task(task.definition, folder, resolved.name, "plm", resolved.execution);
    }
}

/** `pleno run` and `pleno check` for the active file. */
class PlenoTaskProvider implements vscode.TaskProvider {
    constructor(private readonly context: vscode.ExtensionContext) { }

    provideTasks(): vscode.Task[] {
        if (vscode.window.activeTextEditor?.document.languageId !== "pleno") {
            return [];
        }
        return ["run", "check"].map((command) =>
            plenoFileTask(this.context, { type: "pleno", command }));
    }

    resolveTask(task: vscode.Task): vscode.Task | undefined {
        if (!task.definition.command) {
            return undefined;
        }
        return plenoFileTask(this.context, task.definition, task.scope);
    }
}

let client: LanguageClient | undefined;
let statusBarItem: vscode.StatusBarItem;
let outputChannel: vscode.LogOutputChannel;
let diagnosticsProvider: PlenoDiagnosticsProvider | undefined;
let formatterRegistration: vscode.Disposable | undefined;
let testController: PlenoTestController | undefined;
let serverState: State | undefined;

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
    outputChannel = vscode.window.createOutputChannel("Pleno", { log: true });
    context.subscriptions.push(outputChannel);

    exposeToolchainToTerminals(context);

    const projects = new PlenoProjectProvider();
    context.subscriptions.push(
        projects,
        vscode.window.createTreeView("plenoProject", { treeDataProvider: projects }),
    );

    context.subscriptions.push(
        vscode.debug.registerDebugConfigurationProvider(
            "pleno",
            new PlenoDebugConfigurationProvider(),
        ),
        vscode.debug.registerDebugAdapterDescriptorFactory(
            "pleno",
            new PlenoDebugAdapterFactory(context),
        ),
    );

    testController = new PlenoTestController(context, outputChannel);
    context.subscriptions.push(testController);

    context.subscriptions.push(
        vscode.tasks.registerTaskProvider("plm", new PlmTaskProvider(context, projects)),
        vscode.tasks.registerTaskProvider("pleno", new PlenoTaskProvider(context)),
    );

    registerCommands(context, projects);

    restorePatchedIconThemes(context);

    statusBarItem = vscode.window.createStatusBarItem(
        vscode.StatusBarAlignment.Right,
        100,
    );
    statusBarItem.name = "Pleno";
    statusBarItem.command = "pleno.showStatus";
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
            void offerInstall([
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
        await startLanguageServer(context, lsp.path);
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
    const lsp = findTool(context, "pleno-lsp");
    const cli = findTool(context, "pleno");
    const version = installedVersion(context);
    void vscode.commands.executeCommand("setContext", "pleno.toolchainFound", cli.found && lsp.found);
    void vscode.commands.executeCommand("setContext", "pleno.toolchainInstalled", version !== undefined);
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

async function offerInstall(missing: string[]): Promise<void> {
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
        await vscode.env.openExternal(vscode.Uri.parse(INSTALL_URL));
    }
}

/**
 * The Pleno file a command acts on: the one it was given (from the explorer,
 * an editor menu or a code lens) or the active editor's. Unsaved changes are
 * saved first so the CLI sees them.
 */
async function targetFile(resource?: unknown): Promise<string | undefined> {
    let uri: vscode.Uri | undefined;
    if (resource instanceof vscode.Uri) {
        uri = resource;
    } else if (typeof resource === "string") {
        uri = vscode.Uri.parse(resource);
    } else {
        const editor = vscode.window.activeTextEditor;
        if (editor?.document.languageId === "pleno") {
            uri = editor.document.uri;
        }
    }
    if (!uri) {
        vscode.window.showWarningMessage("Open a Pleno file first.");
        return undefined;
    }
    const document = vscode.workspace.textDocuments.find((doc) => doc.uri.toString() === uri!.toString());
    if (document?.isUntitled || uri.scheme !== "file") {
        vscode.window.showWarningMessage("Save the file before running it.");
        return undefined;
    }
    if (document?.isDirty) {
        await document.save();
    }
    return uri.fsPath;
}

/** Asks which project to use when there is more than one. */
async function pickProject(projects: Project[]): Promise<Project | undefined> {
    if (projects.length <= 1) {
        return projects[0];
    }
    const picked = await vscode.window.showQuickPick(
        projects.map((project) => ({ label: project.name ?? project.folder.name, description: project.folder.uri.fsPath, project })),
        { placeHolder: "Pleno project" },
    );
    return picked?.project;
}

function registerCommands(
    context: vscode.ExtensionContext,
    projects: PlenoProjectProvider,
): void {
    const register = (command: string, callback: (...args: any[]) => any) =>
        context.subscriptions.push(
            vscode.commands.registerCommand(command, callback),
        );

    register("pleno.runFile", async (resource?: unknown) => {
        const file = await targetFile(resource);
        if (file) {
            await vscode.tasks.executeTask(plenoFileTask(context, { type: "pleno", command: "run", file }));
        }
    });

    register("pleno.checkFile", async (resource?: unknown) => {
        const file = await targetFile(resource);
        if (file) {
            await vscode.tasks.executeTask(plenoFileTask(context, { type: "pleno", command: "check", file }));
        }
    });

    register("pleno.debugFile", async (resource?: unknown) => {
        const file = await targetFile(resource);
        if (file) {
            await vscode.debug.startDebugging(
                vscode.workspace.getWorkspaceFolder(vscode.Uri.file(file)),
                { type: "pleno", request: "launch", name: `Debug ${path.basename(file)}`, program: file },
            );
        }
    });

    register("pleno.runScript", async (target?: ProjectItem | string) => {
        let project = target instanceof ProjectItem ? target.project : undefined;
        let script = target instanceof ProjectItem ? target.script : target;
        if (!project) {
            const choices = projects.getProjects().flatMap((candidate) =>
                Object.entries(candidate.scripts)
                    .filter(([name]) => script === undefined || name === script)
                    .map(([name, command]) => ({
                        label: name,
                        description: command,
                        detail: projects.getProjects().length > 1 ? candidate.folder.name : undefined,
                        project: candidate,
                    })));
            if (choices.length === 0) {
                vscode.window.showInformationMessage("There are no scripts in pleno.toml.");
                return;
            }
            const picked = choices.length === 1 && script
                ? choices[0]
                : await vscode.window.showQuickPick(choices, { placeHolder: "Script to run with plm run" });
            project = picked?.project;
            script = picked?.label;
        }
        if (project && script) {
            await vscode.tasks.executeTask(plmTask(context, `run ${script}`, project.folder));
        }
    });

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

    register("pleno.createProject", () => createProject(context, projects));

    register("pleno.openManifest", async () => {
        const project = await pickProject(projects.getProjects());
        if (project) {
            await vscode.window.showTextDocument(project.manifest);
        } else {
            vscode.window.showInformationMessage("There is no pleno.toml in the workspace.");
        }
    });

    register("pleno.restartServer", async () => {
        await vscode.window.withProgress({
            location: vscode.ProgressLocation.Window,
            title: "Restarting the Pleno language server",
        }, () => {
            clearToolCache();
            return startTooling(context);
        });
        projects.refresh();
        void testController?.refresh();
        if (!client) {
            void offerInstall(["pleno-lsp"]);
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

    register("pleno.showStatus", () => showStatus(context));

    register("pleno.openDocs", () =>
        vscode.env.openExternal(vscode.Uri.parse(DOCS_URL))
    );

    register("pleno.openWalkthrough", () =>
        vscode.commands.executeCommand(
            "workbench.action.openWalkthrough",
            `${context.extension.id}#pleno-welcome`,
            false,
        )
    );
}

type StatusPick = vscode.QuickPickItem & { run?: () => unknown };

/**
 * The status bar menu: where each tool was found and its version, whether
 * the language server runs, and the commands to fix or change that.
 */
async function showStatus(context: vscode.ExtensionContext): Promise<void> {
    const picker = vscode.window.createQuickPick<StatusPick>();
    picker.title = "Pleno";
    picker.placeholder = "Toolchain status and actions";
    picker.matchOnDescription = true;

    const settings: Partial<Record<Tool, string>> = {
        "pleno": "pleno.cli.path",
        "pleno-lsp": "pleno.lsp.path",
        "pleno-dap": "pleno.dap.path",
    };
    const sources: Record<string, string> = {
        setting: "from settings",
        toolchain: "installed by the extension",
        path: "on PATH",
        home: "in ~/.pleno/bin",
        repository: "built in this repository",
    };
    const tools: Tool[] = ["pleno", "plm", "pleno-lsp", "pleno-dap", "pleno-repl"];
    const toolItem = (tool: Tool, version?: string): StatusPick => {
        const location = findTool(context, tool);
        const state = tool === "pleno-lsp" && location.found
            ? (client && serverState === State.Running ? "running" : "not running")
            : undefined;
        const setting = settings[tool];
        return {
            label: `${location.found ? "$(check)" : "$(error)"} ${tool}`,
            description: location.found
                ? [version, state, sources[location.source]].filter(Boolean).join(" · ")
                : "not found",
            detail: location.found ? location.path : undefined,
            run: setting
                ? () => vscode.commands.executeCommand("workbench.action.openSettings", setting)
                : undefined,
        };
    };
    const actions: StatusPick[] = [
        { label: "Actions", kind: vscode.QuickPickItemKind.Separator },
        { label: "$(cloud-download) Install or Update Toolchain", run: () => vscode.commands.executeCommand("pleno.installToolchain") },
        { label: "$(debug-restart) Restart Language Server", run: () => vscode.commands.executeCommand("pleno.restartServer") },
        { label: "$(output) Show Output", run: () => outputChannel.show() },
        { label: "$(book) Open Documentation", run: () => vscode.commands.executeCommand("pleno.openDocs") },
    ];
    const render = (versions: (string | undefined)[]) => {
        picker.items = [
            { label: "Toolchain", kind: vscode.QuickPickItemKind.Separator },
            ...tools.map((tool, index) => toolItem(tool, versions[index])),
            ...actions,
        ];
    };

    render([]);
    picker.busy = true;
    picker.onDidAccept(() => {
        const picked = picker.selectedItems[0];
        picker.hide();
        void picked?.run?.();
    });
    picker.onDidHide(() => picker.dispose());
    picker.show();

    // `pleno-dap` and `pleno-repl` have no --version flag.
    const versions = await Promise.all(tools.map((tool) =>
        tool === "pleno-dap" || tool === "pleno-repl"
            ? Promise.resolve(undefined)
            : toolVersion(findTool(context, tool))));
    render(versions);
    picker.busy = false;
}

/**
 * Runs `plm init` in a folder: the open workspace folder when it has no
 * project yet, or one the user picks, which is then opened.
 */
async function createProject(context: vscode.ExtensionContext, projects: PlenoProjectProvider): Promise<void> {
    const taken = new Set(projects.getProjects().map((project) => project.folder.uri.toString()));
    let folder = vscode.workspace.workspaceFolders?.find((candidate) => !taken.has(candidate.uri.toString()))?.uri;
    if (!folder) {
        const picked = await vscode.window.showOpenDialog({
            canSelectFolders: true,
            canSelectFiles: false,
            openLabel: "Create Project Here",
            title: "Folder for the new Pleno project",
        });
        folder = picked?.[0];
    }
    if (!folder) {
        return;
    }
    const name = await vscode.window.showInputBox({
        title: "New Pleno project",
        prompt: `Package name. plm init writes pleno.toml and src/main.pln in ${folder.fsPath}.`,
        value: path.basename(folder.fsPath).replace(/[^A-Za-z0-9_-]/g, "-"),
        validateInput: (value) =>
            /^[A-Za-z0-9_-]+$/.test(value) ? undefined : "Use letters, digits, '-' and '_'.",
    });
    if (!name) {
        return;
    }

    const failed = await vscode.window.withProgress({
        location: vscode.ProgressLocation.Notification,
        title: `plm init ${name}`,
    }, () => new Promise<string | undefined>((resolve) => {
        execFile(getPlenoPath(context), ["plm", "init", name], { cwd: folder!.fsPath, timeout: 60_000 }, (error, stdout, stderr) => {
            outputChannel.appendLine(stdout + stderr);
            resolve(error ? (stderr.trim() || error.message) : undefined);
        });
    }));
    if (failed) {
        vscode.window.showErrorMessage(`plm init failed: ${failed}`);
        return;
    }

    const main = vscode.Uri.joinPath(folder, "src", "main.pln");
    if (vscode.workspace.getWorkspaceFolder(folder)) {
        projects.refresh();
        await vscode.window.showTextDocument(main);
    } else {
        await vscode.commands.executeCommand("vscode.openFolder", folder);
    }
}

async function startLanguageServer(context: vscode.ExtensionContext, serverPath: string): Promise<void> {
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
        },
        outputChannel,
    };

    const languageClient = new LanguageClient(
        // The id is the settings prefix the client reads: `pleno.trace.server`.
        "pleno",
        "Pleno Language Server",
        serverOptions,
        clientOptions,
    );
    // Keep the status bar in step when the server stops or restarts later.
    languageClient.onDidChangeState((event) => {
        serverState = event.newState;
        updateStatus(context);
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
    return client?.stop();
}
