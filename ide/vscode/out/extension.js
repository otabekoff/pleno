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
exports.activate = activate;
exports.deactivate = deactivate;
const vscode = __importStar(require("vscode"));
const path = __importStar(require("path"));
const fs = __importStar(require("fs"));
const child_process_1 = require("child_process");
const node_1 = require("vscode-languageclient/node");
const iconInjector_1 = require("./iconInjector");
const utils_1 = require("./utils");
const projectExplorer_1 = require("./projectExplorer");
const dashboard_1 = require("./dashboard");
class PlmTaskProvider {
    static PlmType = 'plm';
    provideTasks() {
        const workspaceFolders = vscode.workspace.workspaceFolders;
        if (!workspaceFolders || workspaceFolders.length === 0) {
            return [];
        }
        const tasks = [];
        for (const folder of workspaceFolders) {
            const manifestPath = path.join(folder.uri.fsPath, 'pleno.toml');
            if (fs.existsSync(manifestPath)) {
                tasks.push(this.createTask('build', folder));
                tasks.push(this.createTask('test', folder));
                tasks.push(this.createTask('install', folder));
                tasks.push(this.createTask('run', folder));
            }
        }
        return tasks;
    }
    resolveTask(task) {
        const command = task.definition.command;
        if (command) {
            return new vscode.Task(task.definition, task.scope ?? vscode.TaskScope.Workspace, command, 'plm', new vscode.ShellExecution(`plm ${command}`));
        }
        return undefined;
    }
    createTask(command, folder) {
        const definition = { type: PlmTaskProvider.PlmType, command };
        const task = new vscode.Task(definition, folder, `plm ${command}`, 'plm', new vscode.ShellExecution(`plm ${command}`));
        if (command === 'build') {
            task.group = vscode.TaskGroup.Build;
        }
        else if (command === 'test') {
            task.group = vscode.TaskGroup.Test;
        }
        return task;
    }
}
class PlenoTaskProvider {
    static PlenoType = 'pleno';
    provideTasks() {
        const editor = vscode.window.activeTextEditor;
        if (!editor || editor.document.languageId !== 'pleno') {
            return [];
        }
        const tasks = [];
        const file = editor.document.fileName;
        tasks.push(new vscode.Task({ type: PlenoTaskProvider.PlenoType, command: 'run', file }, vscode.TaskScope.Workspace, 'pleno run', 'pleno', new vscode.ShellExecution(`pleno run "${file}"`)));
        tasks.push(new vscode.Task({ type: PlenoTaskProvider.PlenoType, command: 'check', file }, vscode.TaskScope.Workspace, 'pleno check', 'pleno', new vscode.ShellExecution(`pleno check "${file}"`)));
        return tasks;
    }
    resolveTask(task) {
        const command = task.definition.command;
        const file = task.definition.file || '${file}';
        if (command) {
            return new vscode.Task(task.definition, task.scope ?? vscode.TaskScope.Workspace, command, 'pleno', new vscode.ShellExecution(`pleno ${command} "${file}"`));
        }
        return undefined;
    }
}
let client;
let myStatusBarItem;
function activate(context) {
    // Try to inject icon theme patches
    (0, iconInjector_1.injectPlenoIcons)(context);
    // Register task providers
    context.subscriptions.push(vscode.tasks.registerTaskProvider(PlmTaskProvider.PlmType, new PlmTaskProvider()));
    context.subscriptions.push(vscode.tasks.registerTaskProvider(PlenoTaskProvider.PlenoType, new PlenoTaskProvider()));
    // Register project explorer
    const rootPath = (vscode.workspace.workspaceFolders && (vscode.workspace.workspaceFolders.length > 0))
        ? vscode.workspace.workspaceFolders[0].uri.fsPath : undefined;
    const projectProvider = new projectExplorer_1.PlenoProjectProvider(context, rootPath);
    vscode.window.registerTreeDataProvider('plenoProject', projectProvider);
    const outputChannel = vscode.window.createOutputChannel('Pleno');
    outputChannel.appendLine('Pleno extension activating...');
    // Tool check setup
    const lspPath = (0, utils_1.getPlenoLspPath)(context);
    const cliPath = (0, utils_1.getPlenoPath)(context);
    outputChannel.appendLine(`LSP path: ${lspPath}`);
    outputChannel.appendLine(`CLI path: ${cliPath}`);
    const lspFound = lspPath.includes('pleno-lsp'); // Simplified check for logs
    const cliFound = cliPath.includes('pleno');
    // Create status bar item
    myStatusBarItem = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Right, 100);
    myStatusBarItem.text = `$(zap) Pleno`;
    myStatusBarItem.tooltip = 'Pleno Extension Status (Click to show output)';
    myStatusBarItem.command = 'pleno.showOutput';
    myStatusBarItem.show();
    context.subscriptions.push(myStatusBarItem);
    const updateStatus = () => {
        if (lspFound && cliFound) {
            myStatusBarItem.text = `$(check) Pleno`;
            myStatusBarItem.tooltip = 'Pleno toolchain (LSP & CLI) is ready.';
            myStatusBarItem.backgroundColor = undefined;
        }
        else if (lspFound) {
            myStatusBarItem.text = `$(warning) Pleno CLI`;
            myStatusBarItem.tooltip = 'Pleno CLI not found. Some features may not work.';
            myStatusBarItem.backgroundColor = new vscode.ThemeColor('statusBarItem.warningBackground');
        }
        else if (cliFound) {
            myStatusBarItem.text = `$(error) Pleno LSP`;
            myStatusBarItem.tooltip = 'Pleno LSP not found. Advanced features disabled.';
            myStatusBarItem.backgroundColor = new vscode.ThemeColor('statusBarItem.errorBackground');
        }
        else {
            myStatusBarItem.text = `$(error) Pleno`;
            myStatusBarItem.tooltip = 'Pleno toolchain (CLI & LSP) not found.';
            myStatusBarItem.backgroundColor = new vscode.ThemeColor('statusBarItem.errorBackground');
        }
        myStatusBarItem.show();
    };
    updateStatus();
    if (!lspFound || !cliFound) {
        let missing = [];
        if (!lspFound)
            missing.push('LSP');
        if (!cliFound)
            missing.push('CLI');
        const msg = `Pleno ${missing.join(' & ')} not found.`;
        vscode.window.showWarningMessage(msg, 'Help', 'Retry').then(selection => {
            if (selection === 'Help') {
                vscode.commands.executeCommand('vscode.open', vscode.Uri.parse('https://github.com/otabekoff/pleno/blob/main/ide/vscode/README.md'));
            }
            else if (selection === 'Retry') {
                vscode.commands.executeCommand('pleno.restartServer'); // Re-trigger activation logic or just restart
            }
        });
    }
    if (lspFound && cliFound) {
        outputChannel.appendLine(`Pleno extension activated. LSP found at: ${lspPath}`);
    }
    else if (lspFound) {
        outputChannel.appendLine('Pleno CLI not found. Running and checking files may not work.');
    }
    else if (cliFound) {
        outputChannel.appendLine('Pleno LSP server not found. Advanced features will be disabled.');
    }
    else {
        outputChannel.appendLine('Pleno toolchain (CLI & LSP) not found.');
    }
    let serverPath = lspPath;
    // If the extension is launched in debug mode then the debug server options are used
    // Otherwise the run options are used
    let serverOptions = {
        run: { command: serverPath, transport: node_1.TransportKind.stdio },
        debug: { command: serverPath, transport: node_1.TransportKind.stdio }
    };
    // Options to control the language client
    let clientOptions = {
        // Register the server for pleno documents
        documentSelector: [{ scheme: 'file', language: 'pleno' }],
        synchronize: {
            // Notify the server about file changes to '.pln files contained in the workspace
            fileEvents: vscode.workspace.createFileSystemWatcher('**/*.pln')
        }
    };
    // Create the language client and start the client.
    client = new node_1.LanguageClient('plenoLanguageServer', 'Pleno Language Server', serverOptions, clientOptions);
    // Start the client. This will also launch the server
    client.start();
    // Test Explorer Setup
    const testController = vscode.tests.createTestController('plenoTests', 'Pleno Tests');
    context.subscriptions.push(testController);
    const testItemMap = new Map();
    const getOrCreateFile = (uri) => {
        const existing = testController.items.get(uri.toString());
        if (existing) {
            return existing;
        }
        const file = testController.createTestItem(uri.toString(), uri.path.split('/').pop(), uri);
        testController.items.add(file);
        testItemMap.set(uri.toString(), file);
        return file;
    };
    const discoverTests = async () => {
        const files = await vscode.workspace.findFiles('**/*.pln', '**/node_modules/**');
        for (const file of files) {
            getOrCreateFile(file);
        }
    };
    discoverTests();
    const runHandler = async (request, token) => {
        const run = testController.createTestRun(request);
        const queue = [];
        if (request.include) {
            request.include.forEach(test => queue.push(test));
        }
        else {
            testController.items.forEach(test => queue.push(test));
        }
        for (const test of queue) {
            if (token.isCancellationRequested) {
                run.skipped(test);
                continue;
            }
            run.started(test);
            const start = Date.now();
            try {
                const filePath = vscode.Uri.parse(test.id).fsPath;
                const plenoPath = cliFound ? 'pleno' : context.asAbsolutePath(path.join('..', '..', 'target', 'release', 'pleno'));
                (0, child_process_1.exec)(`${plenoPath} test "${filePath}" --json`, (error, stdout, stderr) => {
                    const duration = Date.now() - start;
                    try {
                        const output = JSON.parse(stdout);
                        if (output.failed === 0) {
                            run.passed(test, duration);
                        }
                        else {
                            const result = output.results[0];
                            const message = result ? result.message : 'Test failed';
                            run.failed(test, new vscode.TestMessage(message), duration);
                        }
                    }
                    catch {
                        run.errored(test, new vscode.TestMessage(`Parse error: ${stdout}\n${stderr}`), duration);
                    }
                    if (test === queue[queue.length - 1]) {
                        run.end();
                    }
                });
            }
            catch (e) {
                run.errored(test, new vscode.TestMessage(e.message));
                if (test === queue[queue.length - 1]) {
                    run.end();
                }
            }
        }
    };
    testController.createRunProfile('Run', vscode.TestRunProfileKind.Run, runHandler, true);
    const watcher = vscode.workspace.createFileSystemWatcher('**/*.pln');
    watcher.onDidCreate(uri => getOrCreateFile(uri));
    watcher.onDidChange(uri => getOrCreateFile(uri));
    watcher.onDidDelete(uri => testController.items.delete(uri.toString()));
    context.subscriptions.push(watcher);
    // Register commands
    context.subscriptions.push(vscode.commands.registerCommand('pleno.runFile', () => {
        const editor = vscode.window.activeTextEditor;
        if (editor) {
            const terminal = vscode.window.activeTerminal || vscode.window.createTerminal('Pleno');
            terminal.show();
            terminal.sendText(`pleno run "${editor.document.fileName}"`);
        }
    }), vscode.commands.registerCommand('pleno.checkFile', () => {
        const editor = vscode.window.activeTextEditor;
        if (editor) {
            const terminal = vscode.window.activeTerminal || vscode.window.createTerminal('Pleno');
            terminal.show();
            terminal.sendText(`pleno check "${editor.document.fileName}"`);
        }
    }), vscode.commands.registerCommand('pleno.restartServer', async () => {
        if (client) {
            await vscode.window.withProgress({
                location: vscode.ProgressLocation.Notification,
                title: "Restarting Pleno Language Server...",
                cancellable: false
            }, async (progress) => {
                progress.report({ increment: 0 });
                await client.stop();
                progress.report({ increment: 50, message: "Starting server..." });
                client.start();
                progress.report({ increment: 100 });
            });
            vscode.window.showInformationMessage('Pleno Language Server restarted.');
        }
    }), vscode.commands.registerCommand('pleno.showOutput', () => {
        outputChannel.show();
    }), vscode.commands.registerCommand('pleno.openDocs', () => {
        vscode.env.openExternal(vscode.Uri.parse('https://pleno.run/docs'));
    }), vscode.commands.registerCommand('pleno.openRepl', () => {
        const terminal = vscode.window.activeTerminal || vscode.window.createTerminal('Pleno REPL');
        terminal.show();
        // Assuming 'plm repl' or 'pleno-repl'
        terminal.sendText(`${(0, utils_1.getPlenoPath)(context)}-repl`);
    }), vscode.commands.registerCommand('pleno.runScript', (scriptName) => {
        const terminal = vscode.window.activeTerminal || vscode.window.createTerminal('Pleno');
        terminal.show();
        terminal.sendText(`${(0, utils_1.getPlenoPath)(context)} plm run ${scriptName}`);
    }), vscode.commands.registerCommand('pleno.showDashboard', () => {
        dashboard_1.PlenoDashboardPanel.createOrShow(context.extensionUri);
    }), vscode.commands.registerCommand('pleno.openWalkthrough', () => {
        vscode.commands.executeCommand('workbench.action.openWalkthrough', 'pleno.pleno-language#pleno-welcome', false);
    }));
    return {
        extendMarkdownIt(md) {
            const defaultRender = md.renderer.rules.fence || function (tokens, idx, options, env, self) {
                return self.renderToken(tokens, idx, options);
            };
            md.renderer.rules.fence = (tokens, idx, options, env, self) => {
                const token = tokens[idx];
                const info = token.info ? token.info.trim() : '';
                const langName = info.split(/\s+/g)[0];
                if (langName === 'pleno' || langName === 'pln') {
                    const highlighted = escapeHtml(token.content);
                    return `<pre class="pleno-highlight"><code class="language-pleno">${highlighted}</code></pre>`;
                }
                return defaultRender(tokens, idx, options, env, self);
            };
            return md;
        }
    };
}
function escapeHtml(unsafe) {
    return unsafe
        .replace(/&/g, "&amp;")
        .replace(/</g, "&lt;")
        .replace(/>/g, "&gt;")
        .replace(/"/g, "&quot;")
        .replace(/'/g, "&#039;");
}
function deactivate() {
    if (!client) {
        return undefined;
    }
    return client.stop();
}
//# sourceMappingURL=extension.js.map