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
exports.PlenoFormattingProvider = void 0;
const vscode = __importStar(require("vscode"));
const child_process_1 = require("child_process");
const utils_1 = require("./utils");
class PlenoFormattingProvider {
    context;
    constructor(context) {
        this.context = context;
    }
    provideDocumentFormattingEdits(document, _options, _token) {
        return new Promise((resolve, _reject) => {
            const plenoPath = (0, utils_1.getPlenoPath)(this.context);
            const filePath = document.fileName;
            // Use 'pleno fmt' to format the code.
            // We use a temporary file or pipe to avoid modifying the file on disk directly.
            // For simplicity in V1, we just run 'pleno fmt' on the file if it's saved,
            // or we could use stdin if supported.
            // Assuming 'pleno fmt <file>' formats in place, but we want to return edits.
            // Better: 'pleno fmt --stdout <file>'
            (0, child_process_1.exec)(`${plenoPath} fmt "${filePath}" --stdout`, (error, stdout, stderr) => {
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
exports.PlenoFormattingProvider = PlenoFormattingProvider;
//# sourceMappingURL=formatter.js.map