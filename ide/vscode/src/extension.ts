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

    // LSP Server setup
    // 1. Try to use 'pleno-lsp' from PATH
    // 2. Fallback to local 'target/release/pleno-lsp' (development)

    const command = process.platform === 'win32' ? 'where' : 'which';
    let serverPath = 'pleno-lsp';
    
    try {
        require('child_process').execSync(`${command} pleno-lsp`);
    } catch {
       // Not in PATH, use local fallback
       serverPath = context.asAbsolutePath(
         path.join('..', '..', 'target', 'release', 'pleno-lsp')
       );
    }
    
    let serverModule = serverPath;

    // If the extension is launched in debug mode then the debug server options are used
    // Otherwise the run options are used
    let serverOptions: ServerOptions = {
        run: { command: serverModule, transport: TransportKind.stdio },
        debug: { command: serverModule, transport: TransportKind.stdio }
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
