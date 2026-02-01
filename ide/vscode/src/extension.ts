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

export function activate(context: vscode.ExtensionContext) {
    // Try to inject icon theme patches
    injectPlenoIcons(context);

    const config = vscode.workspace.getConfiguration('pleno');
    const customLspPath = config.get<string>('lsp.path');

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

    if (lspFound && cliFound) {
        vscode.window.showInformationMessage(`Pleno extension activated. LSP and CLI found.`);
    } else if (lspFound) {
        vscode.window.showWarningMessage('Pleno CLI not found. Running and checking files may not work. Please install the Pleno toolchain.');
    } else if (cliFound) {
        vscode.window.showWarningMessage('Pleno LSP server not found. Advanced features like hover and go-to-definition will be disabled.');
    } else {
        vscode.window.showErrorMessage('Pleno toolchain (CLI & LSP) not found. Please check your installation and settings.');
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
                await client.stop();
                client.start();
                vscode.window.showInformationMessage('Pleno Language Server restarted.');
            }
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
