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

/**
 * Finds the `pleno-dap` debug adapter: the `pleno.dap.path` setting, then
 * `PATH`, then a local release build. Returns undefined when it is not installed.
 */
export function getPlenoDapPath(context: vscode.ExtensionContext): string | undefined {
    const config = vscode.workspace.getConfiguration('pleno');
    const customDapPath = config.get<string>('dap.path');
    if (customDapPath) {
        return customDapPath;
    }

    const command = process.platform === 'win32' ? 'where' : 'which';
    try {
        execSync(`${command} pleno-dap`);
        return 'pleno-dap';
    } catch {
        const exe = process.platform === 'win32' ? 'pleno-dap.exe' : 'pleno-dap';
        const localDap = context.asAbsolutePath(
            path.join('..', '..', 'target', 'release', exe)
        );
        if (fs.existsSync(localDap)) {
            return localDap;
        }
    }
    return undefined;
}
