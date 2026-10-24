import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';

const RESTORED_KEY = 'pleno.iconThemesRestored';

/**
 * Earlier versions of this extension patched the active icon theme's files to
 * add a Pleno icon, keeping a `.pleno-backup` copy. Pleno files now get their
 * icon from the language contribution instead, so this puts every patched
 * theme back as it was, once.
 */
export function restorePatchedIconThemes(context: vscode.ExtensionContext): void {
    if (context.globalState.get<boolean>(RESTORED_KEY)) {
        return;
    }
    for (const extension of vscode.extensions.all) {
        const themes: { path?: string }[] = extension.packageJSON?.contributes?.iconThemes ?? [];
        for (const theme of themes) {
            if (!theme.path) {
                continue;
            }
            const themeFile = path.join(extension.extensionPath, theme.path);
            const backup = `${themeFile}.pleno-backup`;
            try {
                if (fs.existsSync(backup)) {
                    fs.copyFileSync(backup, themeFile);
                    fs.rmSync(backup);
                    fs.rmSync(path.join(path.dirname(themeFile), 'pleno-icons'), { recursive: true, force: true });
                }
            } catch (error) {
                console.error(`[Pleno] Could not restore ${themeFile}:`, error);
            }
        }
    }
    void context.globalState.update(RESTORED_KEY, true);
}
