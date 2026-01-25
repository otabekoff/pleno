import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';

export async function injectPlenoIcons(context: vscode.ExtensionContext) {
    try {
        const config = vscode.workspace.getConfiguration('workbench');
        const themeId = config.get<string>('iconTheme');

        if (!themeId) {
            console.log('No icon theme active.');
            return;
        }

        console.log('Active icon theme:', themeId);

        // Skip if it's our own theme
        if (themeId === 'pleno' || themeId === 'pleno-material') {
             console.log('Pleno theme active, no injection needed.');
             return;
        }

        const ext = vscode.extensions.getExtension(themeId);
        if (!ext) {
            console.log('Could not find extension for theme:', themeId);
            return;
        }

        const themePath = ext.extensionPath;
        const pkgPath = path.join(themePath, 'package.json');
        
        if (!fs.existsSync(pkgPath)) {
            console.error('Theme package.json not found at:', pkgPath);
            return;
        }

        const pkg = JSON.parse(fs.readFileSync(pkgPath, 'utf8'));
        const contributes = pkg.contributes;
        if (!contributes || !contributes.iconThemes) {
             console.log('Extension does not contribute iconThemes.');
             return;
        }

        const themeEntry = contributes.iconThemes.find((t: any) => t.id === themeId);
        if (!themeEntry) {
             console.log('Theme entry not found in package.json.');
             return;
        }

        const iconJsonPath = path.join(themePath, themeEntry.path);
        if (!fs.existsSync(iconJsonPath)) {
             console.error('Icon theme JSON not found at:', iconJsonPath);
             return;
        }

        // Backup
        const backupPath = iconJsonPath + '.pleno-backup';
        if (!fs.existsSync(backupPath)) {
            fs.copyFileSync(iconJsonPath, backupPath);
        }

        const theme = JSON.parse(fs.readFileSync(iconJsonPath, 'utf8'));

        // Idempotency check
        if (theme.fileExtensions && theme.fileExtensions['pleno'] === 'pleno') {
             console.log('Pleno icons already injected.');
             // Check if we need to update the SVG file? ignoring for now, assuming standard injection.
             return;
        }

        // Patch
        console.log('Patching theme JSON...');
        theme.fileExtensions = {
            ...(theme.fileExtensions || {}),
            "pleno": "pleno",
            "pln": "pleno"
        };
        theme.folderNames = {
             ...(theme.folderNames || {}),
             "pleno": "pleno",
             ".pleno": "pleno"
        };
        
        // Add icon definition
        theme.iconDefinitions = {
            ...(theme.iconDefinitions || {}),
            "pleno": {
                "iconPath": "pleno-icons/pleno.svg"
            }
        };

        fs.writeFileSync(iconJsonPath, JSON.stringify(theme, null, 2));

        // Copy SVG
        const targetIconDir = path.join(path.dirname(iconJsonPath), 'pleno-icons');
        if (!fs.existsSync(targetIconDir)) {
             fs.mkdirSync(targetIconDir, { recursive: true });
        }

        const sourceIconPath = path.join(context.extensionPath, 'resources', 'file-icon.svg');
        const targetIconPath = path.join(targetIconDir, 'pleno.svg');
        
        fs.copyFileSync(sourceIconPath, targetIconPath);
        
        console.log('Pleno icons injected successfully.');
        
        // Notify user
        const reload = 'Reload Window';
        const selection = await vscode.window.showInformationMessage(
            'Pleno icons have been injected into your active theme! Reload to see changes.',
            reload
        );
        if (selection === reload) {
            vscode.commands.executeCommand('workbench.action.reloadWindow');
        }

    } catch (e) {
        console.error('Failed to inject Pleno icons:', e);
    }
}
