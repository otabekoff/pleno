import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';
import { execSync } from 'child_process';

export function getPlenoPath(context: vscode.ExtensionContext): string {
    const command = process.platform === 'win32' ? 'where' : 'which';
    try {
        execSync(`${command} pleno`);
        return 'pleno';
    } catch {
        const localCli = context.asAbsolutePath(
            path.join('..', '..', 'target', 'release', 'pleno')
        );
        if (fs.existsSync(localCli)) {
            return localCli;
        }
    }
    return 'pleno'; // Fallback to name and let it fail later
}

export function getPlenoLspPath(context: vscode.ExtensionContext): string {
    const config = vscode.workspace.getConfiguration('pleno');
    const customLspPath = config.get<string>('lsp.path');
    if (customLspPath) {
        return customLspPath;
    }

    const command = process.platform === 'win32' ? 'where' : 'which';
    try {
        execSync(`${command} pleno-lsp`);
        return 'pleno-lsp';
    } catch {
        const localLsp = context.asAbsolutePath(
            path.join('..', '..', 'target', 'release', 'pleno-lsp')
        );
        if (fs.existsSync(localLsp)) {
            return localLsp;
        }
    }
    return 'pleno-lsp';
}
