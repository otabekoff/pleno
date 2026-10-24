import * as vscode from "vscode";
import { execFile } from "child_process";
import { getPlenoPath } from "./utils";

/** One problem as printed by `pleno check --json` (1-based positions). */
interface CheckProblem {
    line: number;
    column: number;
    endLine: number;
    endColumn: number;
    severity: string;
    message: string;
}

/**
 * Diagnostics from `pleno check --json`, used only when the language server
 * is not available (the server reports diagnostics as you type). Files are
 * checked as saved on disk, when they are opened and saved, so imports
 * resolve relative to the real file.
 */
export class PlenoDiagnosticsProvider {
    private diagnosticCollection: vscode.DiagnosticCollection;
    private listeners: vscode.Disposable[] = [];

    constructor(
        private context: vscode.ExtensionContext,
        private outputChannel: vscode.OutputChannel,
    ) {
        this.diagnosticCollection = vscode.languages.createDiagnosticCollection(
            "pleno",
        );
    }

    public register(): void {
        const check = (doc: vscode.TextDocument) => {
            if (doc.languageId === "pleno" && doc.uri.scheme === "file") {
                this.checkDocument(doc);
            }
        };
        vscode.workspace.textDocuments.forEach(check);
        this.listeners = [
            vscode.workspace.onDidOpenTextDocument(check),
            vscode.workspace.onDidSaveTextDocument(check),
            vscode.workspace.onDidCloseTextDocument((doc) =>
                this.diagnosticCollection.delete(doc.uri)
            ),
        ];
    }

    public checkDocument(document: vscode.TextDocument): void {
        execFile(
            getPlenoPath(this.context),
            ["check", document.uri.fsPath, "--json"],
            { timeout: 30000 },
            (error, stdout, stderr) => {
                let problems: CheckProblem[];
                try {
                    problems = JSON.parse(stdout);
                } catch {
                    // Not JSON: the CLI is missing or too old for `--json`.
                    this.outputChannel.appendLine(
                        `pleno check failed: ${stderr || error?.message || stdout}`,
                    );
                    return;
                }
                this.diagnosticCollection.set(
                    document.uri,
                    problems.map((problem) => this.toDiagnostic(problem)),
                );
            },
        );
    }

    private toDiagnostic(problem: CheckProblem): vscode.Diagnostic {
        const range = new vscode.Range(
            Math.max(problem.line - 1, 0),
            Math.max(problem.column - 1, 0),
            Math.max(problem.endLine - 1, 0),
            Math.max(problem.endColumn - 1, 0),
        );
        const severity = problem.severity === "warning"
            ? vscode.DiagnosticSeverity.Warning
            : vscode.DiagnosticSeverity.Error;
        const diagnostic = new vscode.Diagnostic(range, problem.message, severity);
        diagnostic.source = "pleno";
        return diagnostic;
    }

    public dispose(): void {
        this.listeners.forEach((listener) => listener.dispose());
        this.listeners = [];
        this.diagnosticCollection.dispose();
    }
}
