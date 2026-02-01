import * as vscode from 'vscode';
import * as path from 'path';
import {
    LanguageClient,
    LanguageClientOptions,
    ServerOptions,
    TransportKind
} from 'vscode-languageclient/node';
import { injectPlenoIcons } from './iconInjector';

let client: LanguageClient;
let myStatusBarItem: vscode.StatusBarItem;

export function activate(context: vscode.ExtensionContext) {
    // Try to inject icon theme patches
    injectPlenoIcons(context);

    const config = vscode.workspace.getConfiguration('pleno');
    const customLspPath = config.get<string>('lsp.path');

    const outputChannel = vscode.window.createOutputChannel('Pleno');
    outputChannel.appendLine('Pleno extension activating...');

    // Tool check setup
    let lspPath = customLspPath || 'pleno-lsp';
    let lspFound = false;
    let cliFound = false;

    const command = process.platform === 'win32' ? 'where' : 'which';

    // Check for LSP
    try {
        require('child_process').execSync(`${command} ${lspPath}`);
        lspFound = true;
    } catch {
        if (!customLspPath) {
            // Try local fallback for LSP
            const localLsp = context.asAbsolutePath(
                path.join('..', '..', 'target', 'release', 'pleno-lsp')
            );
            if (require('fs').existsSync(localLsp)) {
                lspPath = localLsp;
                lspFound = true;
            }
        }
    }

    // Check for CLI
    try {
        require('child_process').execSync(`${command} pleno`);
        cliFound = true;
    } catch {
        // Try local fallback for CLI
        const localCli = context.asAbsolutePath(
            path.join('..', '..', 'target', 'release', 'pleno')
        );
        if (require('fs').existsSync(localCli)) {
            cliFound = true;
        }
    }

    // Create status bar item
    myStatusBarItem = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Right, 100);
    myStatusBarItem.command = 'pleno.showOutput';
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
                const { exec } = require('child_process');
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

    // Register commands
    context.subscriptions.push(
        vscode.commands.registerCommand('pleno.runFile', () => {
            const editor = vscode.window.activeTextEditor;
            if (editor) {
                const terminal = vscode.window.activeTerminal || vscode.window.createTerminal('Pleno');
                terminal.show();
                terminal.sendText(`pleno run "${editor.document.fileName}"`);
            }
        }),
        vscode.commands.registerCommand('pleno.checkFile', () => {
            const editor = vscode.window.activeTextEditor;
            if (editor) {
                const terminal = vscode.window.activeTerminal || vscode.window.createTerminal('Pleno');
                terminal.show();
                terminal.sendText(`pleno check "${editor.document.fileName}"`);
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
        vscode.commands.registerCommand('pleno.showOutput', () => {
            outputChannel.show();
        })
    );

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
