import * as vscode from 'vscode';

export class PlenoDashboardPanel {
    public static currentPanel: PlenoDashboardPanel | undefined;
    public static readonly viewType = 'plenoDashboard';

    private readonly _panel: vscode.WebviewPanel;
    private readonly _extensionUri: vscode.Uri;
    private _disposables: vscode.Disposable[] = [];

    public static createOrShow(extensionUri: vscode.Uri) {
        const column = vscode.window.activeTextEditor
            ? vscode.window.activeTextEditor.viewColumn
            : undefined;

        if (PlenoDashboardPanel.currentPanel) {
            PlenoDashboardPanel.currentPanel._panel.reveal(column);
            return;
        }

        const panel = vscode.window.createWebviewPanel(
            PlenoDashboardPanel.viewType,
            'Pleno Dashboard',
            column || vscode.ViewColumn.One,
            {
                enableScripts: true,
                localResourceRoots: [vscode.Uri.joinPath(extensionUri, 'resources')]
            }
        );

        PlenoDashboardPanel.currentPanel = new PlenoDashboardPanel(panel, extensionUri);
    }

    private constructor(panel: vscode.WebviewPanel, extensionUri: vscode.Uri) {
        this._panel = panel;
        this._extensionUri = extensionUri;

        this._update();

        this._panel.onDidDispose(() => this.dispose(), null, this._disposables);

        this._panel.webview.onDidReceiveMessage(
            message => {
                switch (message.command) {
                    case 'runCommand':
                        vscode.commands.executeCommand(message.value);
                        return;
                    case 'openUrl':
                        vscode.env.openExternal(vscode.Uri.parse(message.value));
                        return;
                }
            },
            null,
            this._disposables
        );
    }

    public dispose() {
        PlenoDashboardPanel.currentPanel = undefined;
        this._panel.dispose();
        while (this._disposables.length) {
            const x = this._disposables.pop();
            if (x) {
                x.dispose();
            }
        }
    }

    private _update() {
        const webview = this._panel.webview;
        this._panel.webview.html = this._getHtmlForWebview(webview);
    }

    private _getHtmlForWebview(webview: vscode.Webview) {
        const scriptUri = webview.asWebviewUri(vscode.Uri.joinPath(this._extensionUri, 'resources', 'dashboard', 'main.js'));
        const styleUri = webview.asWebviewUri(vscode.Uri.joinPath(this._extensionUri, 'resources', 'dashboard', 'style.css'));
        const logoUri = webview.asWebviewUri(vscode.Uri.joinPath(this._extensionUri, 'resources', 'icon.svg'));

        return `<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <link href="${styleUri}" rel="stylesheet">
    <title>Pleno Dashboard</title>
</head>
<body>
    <div class="container">
        <header>
            <img src="${logoUri}" alt="Pleno Logo" class="logo">
            <h1>Pleno Dashboard</h1>
            <p class="subtitle">Empowering your development with Pleno</p>
        </header>

        <div class="main-grid">
            <section class="card quick-actions">
                <h2>Quick Actions</h2>
                <div class="button-group">
                    <button onclick="runCommand('pleno.openRepl')">
                        <span class="icon">⚡</span> Open REPL
                    </button>
                    <button onclick="runCommand('pleno.openWalkthrough')">
                        <span class="icon">📖</span> Getting Started
                    </button>
                    <button onclick="runCommand('pleno.restartServer')">
                        <span class="icon">🔄</span> Restart LSP
                    </button>
                </div>
            </section>

            <section class="card resources">
                <h2>Resources</h2>
                <ul>
                    <li><a href="#" onclick="openUrl('https://pleno.run/docs')">Official Documentation</a></li>
                    <li><a href="#" onclick="openUrl('https://github.com/otabekoff/pleno')">GitHub Repository</a></li>
                    <li><a href="#" onclick="openUrl('https://pleno.run/community')">Community Discord</a></li>
                </ul>
            </section>

            <section class="card project-status">
                <h2>Project Status</h2>
                <div id="project-info">
                    <p>Detecting project...</p>
                </div>
            </section>
        </div>

        <footer>
            <p>Pleno v0.1.0 - Made with ❤️ for developers</p>
        </footer>
    </div>
    <script src="${scriptUri}"></script>
</body>
</html>`;
    }
}
