import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';
import { exec } from 'child_process';
import {
    LanguageClient,
    LanguageClientOptions,
    ServerOptions,
    TransportKind
} from 'vscode-languageclient/node';
import { injectPlenoIcons } from './iconInjector';
import { getPlenoPath, getPlenoLspPath } from './utils';
import { PlenoProjectProvider } from './projectExplorer';
import { PlenoDashboardPanel } from './dashboard';


class PlmTaskProvider implements vscode.TaskProvider {
    static PlmType = 'plm';

    provideTasks(): vscode.ProviderResult<vscode.Task[]> {
        const workspaceFolders = vscode.workspace.workspaceFolders;
        if (!workspaceFolders || workspaceFolders.length === 0) {
            return [];
        }

        const tasks: vscode.Task[] = [];
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

    resolveTask(task: vscode.Task): vscode.Task | undefined {
        const command = task.definition.command;
        if (command) {
            return new vscode.Task(
                task.definition,
                task.scope ?? vscode.TaskScope.Workspace,
                command,
                'plm',
                new vscode.ShellExecution(`plm ${command}`)
            );
        }
        return undefined;
    }

    private createTask(command: string, folder: vscode.WorkspaceFolder): vscode.Task {
        const definition = { type: PlmTaskProvider.PlmType, command };
        const task = new vscode.Task(
            definition,
            folder,
            `plm ${command}`,
            'plm',
            new vscode.ShellExecution(`plm ${command}`)
        );
        if (command === 'build') {
            task.group = vscode.TaskGroup.Build;
        } else if (command === 'test') {
            task.group = vscode.TaskGroup.Test;
        }
        return task;
    }
}

class PlenoTaskProvider implements vscode.TaskProvider {
    static PlenoType = 'pleno';

    provideTasks(): vscode.ProviderResult<vscode.Task[]> {
        const editor = vscode.window.activeTextEditor;
        if (!editor || editor.document.languageId !== 'pleno') {
            return [];
        }

        const tasks: vscode.Task[] = [];
        const file = editor.document.fileName;
        tasks.push(new vscode.Task(
            { type: PlenoTaskProvider.PlenoType, command: 'run', file },
            vscode.TaskScope.Workspace,
            'pleno run',
            'pleno',
            new vscode.ShellExecution(`pleno run "${file}"`)
        ));
        tasks.push(new vscode.Task(
            { type: PlenoTaskProvider.PlenoType, command: 'check', file },
            vscode.TaskScope.Workspace,
            'pleno check',
            'pleno',
            new vscode.ShellExecution(`pleno check "${file}"`)
        ));
        return tasks;
    }

    resolveTask(task: vscode.Task): vscode.Task | undefined {
        const command = task.definition.command;
        const file = task.definition.file || '${file}';
        if (command) {
            return new vscode.Task(
                task.definition,
                task.scope ?? vscode.TaskScope.Workspace,
                command,
                'pleno',
                new vscode.ShellExecution(`pleno ${command} "${file}"`)
            );
        }
        return undefined;
    }
}

let client: LanguageClient;
let myStatusBarItem: vscode.StatusBarItem;

export function activate(context: vscode.ExtensionContext) {
    try {
        return _activate(context);
    } catch (e) {
        vscode.window.showErrorMessage(`Failed to activate Pleno extension: ${e}`);
    }
}

function _activate(context: vscode.ExtensionContext) {
    const outputChannel = vscode.window.createOutputChannel('Pleno');
    outputChannel.appendLine('Pleno extension activating...');

    // 1. Register commands first (so they exist even if later steps fail)
    context.subscriptions.push(
        vscode.commands.registerCommand('pleno.showOutput', () => {
            outputChannel.show();
        }),
        vscode.commands.registerCommand('pleno.runFile', () => {
            const editor = vscode.window.activeTextEditor;
            if (editor) {
                const terminal = vscode.window.activeTerminal || vscode.window.createTerminal('Pleno');
                terminal.show();
                terminal.sendText(`${getPlenoPath(context)} run "${editor.document.fileName}"`);
            }
        }),
        vscode.commands.registerCommand('pleno.checkFile', () => {
            const editor = vscode.window.activeTextEditor;
            if (editor) {
                const terminal = vscode.window.activeTerminal || vscode.window.createTerminal('Pleno');
                terminal.show();
                terminal.sendText(`${getPlenoPath(context)} check "${editor.document.fileName}"`);
            }
        }),
        vscode.commands.registerCommand('pleno.restartServer', async () => {
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
        }),
        vscode.commands.registerCommand('pleno.openDocs', () => {
            vscode.env.openExternal(vscode.Uri.parse('https://pleno.run/docs'));
        }),
        vscode.commands.registerCommand('pleno.openRepl', () => {
            const terminal = vscode.window.activeTerminal || vscode.window.createTerminal('Pleno REPL');
            terminal.show();
            const plenoPath = getPlenoPath(context);
            // If the path is just 'pleno', we assume 'pleno-repl' exists.
            // If it's an absolute path, we replace 'pleno' with 'pleno-repl' at the end.
            const replPath = plenoPath.endsWith('pleno') ? plenoPath.replace(/pleno$/, 'pleno-repl') : `${plenoPath}-repl`;
            terminal.sendText(replPath);
        }),
        vscode.commands.registerCommand('pleno.runScript', (scriptName: string) => {
            const terminal = vscode.window.activeTerminal || vscode.window.createTerminal('Pleno');
            terminal.show();
            terminal.sendText(`${getPlenoPath(context)} plm run ${scriptName}`);
        }),
        vscode.commands.registerCommand('pleno.showDashboard', () => {
            PlenoDashboardPanel.createOrShow(context.extensionUri);
        }),
        vscode.commands.registerCommand('pleno.openWalkthrough', () => {
            vscode.commands.executeCommand('workbench.action.openWalkthrough', 'pleno.pleno-language#pleno-welcome', false);
        })
    );

    // 2. Register tree data provider
    const rootPath = (vscode.workspace.workspaceFolders && (vscode.workspace.workspaceFolders.length > 0))
        ? vscode.workspace.workspaceFolders[0].uri.fsPath : undefined;
    const projectProvider = new PlenoProjectProvider(context, rootPath);
    vscode.window.registerTreeDataProvider('plenoProject', projectProvider);

    // 3. Register task providers
    context.subscriptions.push(vscode.tasks.registerTaskProvider(PlmTaskProvider.PlmType, new PlmTaskProvider()));
    context.subscriptions.push(vscode.tasks.registerTaskProvider(PlenoTaskProvider.PlenoType, new PlenoTaskProvider()));

    // Try to inject icon theme patches in background
    injectPlenoIcons(context).catch(e => outputChannel.appendLine(`Icon injection failed: ${e}`));

    // Tool check setup
    const lspPath = getPlenoLspPath(context);
    const cliPath = getPlenoPath(context);

    outputChannel.appendLine(`LSP path: ${lspPath}`);
    outputChannel.appendLine(`CLI path: ${cliPath}`);

    const lspFound = lspPath.includes('pleno-lsp');
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
        } else if (lspFound) {
            myStatusBarItem.text = `$(warning) Pleno CLI`;
            myStatusBarItem.tooltip = 'Pleno CLI not found. Some features may not work.';
            myStatusBarItem.backgroundColor = new vscode.ThemeColor('statusBarItem.warningBackground');
        } else if (cliFound) {
            myStatusBarItem.text = `$(error) Pleno LSP`;
            myStatusBarItem.tooltip = 'Pleno LSP not found. Advanced features disabled.';
            myStatusBarItem.backgroundColor = new vscode.ThemeColor('statusBarItem.errorBackground');
        } else {
            myStatusBarItem.text = `$(error) Pleno`;
            myStatusBarItem.tooltip = 'Pleno toolchain (CLI & LSP) not found.';
            myStatusBarItem.backgroundColor = new vscode.ThemeColor('statusBarItem.errorBackground');
        }
        myStatusBarItem.show();
    };

    updateStatus();

    if (!lspFound || !cliFound) {
        let missing = [];
        if (!lspFound) missing.push('LSP');
        if (!cliFound) missing.push('CLI');

        const msg = `Pleno ${missing.join(' & ')} not found.`;
        vscode.window.showWarningMessage(msg, 'Help', 'Retry').then(selection => {
            if (selection === 'Help') {
                vscode.commands.executeCommand('vscode.open', vscode.Uri.parse('https://github.com/otabekoff/pleno/blob/main/ide/vscode/README.md'));
            } else if (selection === 'Retry') {
                vscode.commands.executeCommand('pleno.restartServer'); // Re-trigger activation logic or just restart
            }
        });
    }

    if (lspFound && cliFound) {
        outputChannel.appendLine(`Pleno extension activated. LSP found at: ${lspPath}`);
    } else if (lspFound) {
        outputChannel.appendLine('Pleno CLI not found. Running and checking files may not work.');
    } else if (cliFound) {
        outputChannel.appendLine('Pleno LSP server not found. Advanced features will be disabled.');
    } else {
        outputChannel.appendLine('Pleno toolchain (CLI & LSP) not found.');
    }

    let serverPath = lspPath;

    // If the extension is launched in debug mode then the debug server options are used
    // Otherwise the run options are used
    let serverOptions: ServerOptions = {
        run: { command: serverPath, transport: TransportKind.stdio },
        debug: { command: serverPath, transport: TransportKind.stdio }
    };

    // Options to control the language client
    let clientOptions: LanguageClientOptions = {
        // Register the server for pleno documents
        documentSelector: [{ scheme: 'file', language: 'pleno' }],
        synchronize: {
            // Notify the server about file changes to '.pln files contained in the workspace
            fileEvents: vscode.workspace.createFileSystemWatcher('**/*.pln')
        }
    };

    // Create the language client and start the client.
    client = new LanguageClient(
        'plenoLanguageServer',
        'Pleno Language Server',
        serverOptions,
        clientOptions
    );

    // Start the client. This will also launch the server
    client.start();

    // Test Explorer Setup
    const testController = vscode.tests.createTestController('plenoTests', 'Pleno Tests');
    context.subscriptions.push(testController);

    const testItemMap = new Map<string, vscode.TestItem>();

    const getOrCreateFile = (uri: vscode.Uri) => {
        const existing = testController.items.get(uri.toString());
        if (existing) {
            return existing;
        }
        const file = testController.createTestItem(uri.toString(), uri.path.split('/').pop()!, uri);
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

    const runHandler = async (request: vscode.TestRunRequest, token: vscode.CancellationToken) => {
        const run = testController.createTestRun(request);
        const queue: vscode.TestItem[] = [];

        if (request.include) {
            request.include.forEach(test => queue.push(test));
        } else {
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

                exec(`${plenoPath} test "${filePath}" --json`, (error: any, stdout: string, stderr: string) => {
                    const duration = Date.now() - start;
                    try {
                        const output = JSON.parse(stdout);
                        if (output.failed === 0) {
                            run.passed(test, duration);
                        } else {
                            const result = output.results[0];
                            const message = result ? result.message : 'Test failed';
                            run.failed(test, new vscode.TestMessage(message), duration);
                        }
                    } catch {
                        run.errored(test, new vscode.TestMessage(`Parse error: ${stdout}\n${stderr}`), duration);
                    }
                    if (test === queue[queue.length - 1]) {
                        run.end();
                    }
                });
            } catch (e: any) {
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

    // Watcher and other setup remains...

    return {
        extendMarkdownIt(md: any) {
            const defaultRender = md.renderer.rules.fence || function (tokens: any[], idx: number, options: any, env: any, self: any) {
                return self.renderToken(tokens, idx, options);
            };

            md.renderer.rules.fence = (tokens: any[], idx: number, options: any, env: any, self: any) => {
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

function escapeHtml(unsafe: string) {
    return unsafe
        .replace(/&/g, "&amp;")
        .replace(/</g, "&lt;")
        .replace(/>/g, "&gt;")
        .replace(/"/g, "&quot;")
        .replace(/'/g, "&#039;");
}

export function deactivate(): Thenable<void> | undefined {
    if (!client) {
        return undefined;
    }
    return client.stop();
}
