import * as vscode from 'vscode';
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';
import { execFile } from 'child_process';
import { getPlenoPath } from './utils';

/**
 * Formatting with `pleno fmt`, used only when the language server is not
 * available (the server formats documents itself). The editor's current text,
 * including unsaved changes, is formatted through a temporary file.
 */
export class PlenoFormattingProvider implements vscode.DocumentFormattingEditProvider {
    constructor(private context: vscode.ExtensionContext) { }

    public provideDocumentFormattingEdits(
        document: vscode.TextDocument,
    ): vscode.ProviderResult<vscode.TextEdit[]> {
        const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'pleno-fmt-'));
        const file = path.join(dir, path.basename(document.fileName) || 'document.pln');
        fs.writeFileSync(file, document.getText());

        return new Promise((resolve) => {
            execFile(getPlenoPath(this.context), ['fmt', '--stdout', file], (error, stdout, stderr) => {
                fs.rmSync(dir, { recursive: true, force: true });
                if (error) {
                    vscode.window.setStatusBarMessage(`Pleno: formatting failed: ${stderr.trim() || error.message}`, 5000);
                    resolve([]);
                    return;
                }
                if (stdout === document.getText()) {
                    resolve([]);
                    return;
                }
                const whole = new vscode.Range(
                    document.positionAt(0),
                    document.positionAt(document.getText().length),
                );
                resolve([vscode.TextEdit.replace(whole, stdout)]);
            });
        });
    }
}
