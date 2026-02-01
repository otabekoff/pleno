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
exports.PlenoDashboardPanel = void 0;
const vscode = __importStar(require("vscode"));
class PlenoDashboardPanel {
    static currentPanel;
    static viewType = 'plenoDashboard';
    _panel;
    _extensionUri;
    _disposables = [];
    static createOrShow(extensionUri) {
        const column = vscode.window.activeTextEditor
            ? vscode.window.activeTextEditor.viewColumn
            : undefined;
        if (PlenoDashboardPanel.currentPanel) {
            PlenoDashboardPanel.currentPanel._panel.reveal(column);
            return;
        }
        const panel = vscode.window.createWebviewPanel(PlenoDashboardPanel.viewType, 'Pleno Dashboard', column || vscode.ViewColumn.One, {
            enableScripts: true,
            localResourceRoots: [vscode.Uri.joinPath(extensionUri, 'resources')]
        });
        PlenoDashboardPanel.currentPanel = new PlenoDashboardPanel(panel, extensionUri);
    }
    constructor(panel, extensionUri) {
        this._panel = panel;
        this._extensionUri = extensionUri;
        this._update();
        this._panel.onDidDispose(() => this.dispose(), null, this._disposables);
        this._panel.webview.onDidReceiveMessage(message => {
            switch (message.command) {
                case 'runCommand':
                    vscode.commands.executeCommand(message.value);
                    return;
                case 'openUrl':
                    vscode.env.openExternal(vscode.Uri.parse(message.value));
                    return;
            }
        }, null, this._disposables);
    }
    dispose() {
        PlenoDashboardPanel.currentPanel = undefined;
        this._panel.dispose();
        while (this._disposables.length) {
            const x = this._disposables.pop();
            if (x) {
                x.dispose();
            }
        }
    }
    _update() {
        const webview = this._panel.webview;
        this._panel.webview.html = this._getHtmlForWebview(webview);
    }
    _getHtmlForWebview(webview) {
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
exports.PlenoDashboardPanel = PlenoDashboardPanel;
//# sourceMappingURL=dashboard.js.map