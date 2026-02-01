import * as vscode from 'vscode';
import { exec } from 'child_process';
import { getPlenoPath } from './utils';

export class PlenoFormattingProvider implements vscode.DocumentFormattingEditProvider {
    constructor(private context: vscode.ExtensionContext) { }

    public provideDocumentFormattingEdits(
        document: vscode.TextDocument,
        _options: vscode.FormattingOptions,
        _token: vscode.CancellationToken
    ): vscode.ProviderResult<vscode.TextEdit[]> {
        return new Promise((resolve, _reject) => {
            const plenoPath = getPlenoPath(this.context);
            const filePath = document.fileName;

            // Use 'pleno fmt' to format the code.
            // We use a temporary file or pipe to avoid modifying the file on disk directly.
            // For simplicity in V1, we just run 'pleno fmt' on the file if it's saved,
            // or we could use stdin if supported.

            // Assuming 'pleno fmt <file>' formats in place, but we want to return edits.
            // Better: 'pleno fmt --stdout <file>'

            exec(`${plenoPath} fmt "${filePath}" --stdout`, (error, stdout, stderr) => {
                if (error) {
                    console.error(`Formatting failed: ${stderr}`);
                    return resolve([]);
                }

                if (stdout) {
                    const lastLine = document.lineCount - 1;
                    const lastChar = document.lineAt(lastLine).text.length;
                    const range = new vscode.Range(0, 0, lastLine, lastChar);
                    return resolve([vscode.TextEdit.replace(range, stdout)]);
                }

                resolve([]);
            });
        });
    }
}
