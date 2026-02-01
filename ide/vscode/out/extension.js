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
const node_1 = require("vscode-languageclient/node");
const iconInjector_1 = require("./iconInjector");
let client;
let myStatusBarItem;
function activate(context) {
    // Try to inject icon theme patches
    (0, iconInjector_1.injectPlenoIcons)(context);
    const config = vscode.workspace.getConfiguration('pleno');
    const customLspPath = config.get('lsp.path');
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
    }
    catch {
        if (!customLspPath) {
            // Try local fallback for LSP
            const localLsp = context.asAbsolutePath(path.join('..', '..', 'target', 'release', 'pleno-lsp'));
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
    }
    catch {
        // Try local fallback for CLI
        const localCli = context.asAbsolutePath(path.join('..', '..', 'target', 'release', 'pleno'));
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