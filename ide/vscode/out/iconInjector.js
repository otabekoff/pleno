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
exports.injectPlenoIcons = injectPlenoIcons;
const vscode = __importStar(require("vscode"));
const path = __importStar(require("path"));
const fs = __importStar(require("fs"));
async function injectPlenoIcons(context) {
    try {
        const config = vscode.workspace.getConfiguration('workbench');
        const themeId = config.get('iconTheme');
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
        const themeEntry = contributes.iconThemes.find((t) => t.id === themeId);
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
        const selection = await vscode.window.showInformationMessage('Pleno icons have been injected into your active theme! Reload to see changes.', reload);
        if (selection === reload) {
            vscode.commands.executeCommand('workbench.action.reloadWindow');
        }
    }
    catch (e) {
        console.error('Failed to inject Pleno icons:', e);
    }
}
//# sourceMappingURL=iconInjector.js.map