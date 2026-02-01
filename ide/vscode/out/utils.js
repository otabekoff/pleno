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
exports.getPlenoPath = getPlenoPath;
exports.getPlenoLspPath = getPlenoLspPath;
const vscode = __importStar(require("vscode"));
const path = __importStar(require("path"));
const fs = __importStar(require("fs"));
const child_process_1 = require("child_process");
function getPlenoPath(context) {
    const command = process.platform === 'win32' ? 'where' : 'which';
    try {
        (0, child_process_1.execSync)(`${command} pleno`);
        return 'pleno';
    }
    catch {
        const localCli = context.asAbsolutePath(path.join('..', '..', 'target', 'release', 'pleno'));
        if (fs.existsSync(localCli)) {
            return localCli;
        }
    }
    return 'pleno'; // Fallback to name and let it fail later
}
function getPlenoLspPath(context) {
    const config = vscode.workspace.getConfiguration('pleno');
    const customLspPath = config.get('lsp.path');
    if (customLspPath) {
        return customLspPath;
    }
    const command = process.platform === 'win32' ? 'where' : 'which';
    try {
        (0, child_process_1.execSync)(`${command} pleno-lsp`);
        return 'pleno-lsp';
    }
    catch {
        const localLsp = context.asAbsolutePath(path.join('..', '..', 'target', 'release', 'pleno-lsp'));
        if (fs.existsSync(localLsp)) {
            return localLsp;
        }
    }
    return 'pleno-lsp';
}
//# sourceMappingURL=utils.js.map