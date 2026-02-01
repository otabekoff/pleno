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
exports.PlenoCodeActionProvider = exports.PlenoDiagnosticsProvider = void 0;
const vscode = __importStar(require("vscode"));
const child_process_1 = require("child_process");
const utils_1 = require("./utils");
/**
 * Pleno Diagnostics Provider
 * Provides linting and error diagnostics for Pleno files
 */
class PlenoDiagnosticsProvider {
    diagnosticCollection;
    context;
    outputChannel;
    pendingDiagnostics = new Map();
    debounceTime = 500; // ms
    constructor(context, outputChannel) {
        this.context = context;
        this.outputChannel = outputChannel;
        this.diagnosticCollection = vscode.languages.createDiagnosticCollection("pleno");
        context.subscriptions.push(this.diagnosticCollection);
    }
    /**
     * Register all diagnostic-related event handlers
     */
    register() {
        const disposables = [];
        // Check on document open
        disposables.push(vscode.workspace.onDidOpenTextDocument((doc) => {
            if (doc.languageId === "pleno") {
                this.checkDocument(doc);
            }
        }));
        // Check on document change (debounced)
        disposables.push(vscode.workspace.onDidChangeTextDocument((event) => {
            if (event.document.languageId === "pleno") {
                this.scheduleCheck(event.document);
            }
        }));
        // Check on document save
        disposables.push(vscode.workspace.onDidSaveTextDocument((doc) => {
            if (doc.languageId === "pleno") {
                this.checkDocument(doc);
            }
        }));
        // Clear diagnostics when document is closed
        disposables.push(vscode.workspace.onDidCloseTextDocument((doc) => {
            this.diagnosticCollection.delete(doc.uri);
            this.cancelPendingCheck(doc);
        }));
        // Check all open Pleno documents on activation
        vscode.workspace.textDocuments.forEach((doc) => {
            if (doc.languageId === "pleno") {
                this.checkDocument(doc);
            }
        });
        return disposables;
    }
    /**
     * Schedule a debounced document check
     */
    scheduleCheck(document) {
        this.cancelPendingCheck(document);
        const timeout = setTimeout(() => {
            this.checkDocument(document);
            this.pendingDiagnostics.delete(document.uri.toString());
        }, this.debounceTime);
        this.pendingDiagnostics.set(document.uri.toString(), timeout);
    }
    /**
     * Cancel any pending check for a document
     */
    cancelPendingCheck(document) {
        const pending = this.pendingDiagnostics.get(document.uri.toString());
        if (pending) {
            clearTimeout(pending);
            this.pendingDiagnostics.delete(document.uri.toString());
        }
    }
    /**
     * Check a document for errors using pleno check command
     */
    checkDocument(document) {
        const filePath = document.uri.fsPath;
        const pleno = (0, utils_1.getPlenoPath)(this.context);
        // Run pleno check command
        (0, child_process_1.exec)(`"${pleno}" check "${filePath}" --json`, { timeout: 30000 }, (error, stdout, stderr) => {
            const diagnostics = [];
            try {
                // Try to parse JSON output
                const output = stdout || stderr;
                if (output) {
                    const results = JSON.parse(output);
                    if (Array.isArray(results)) {
                        for (const err of results) {
                            diagnostics.push(this.createDiagnostic(err.line || 1, err.column || 1, err.endLine || err.line || 1, err.endColumn || err.column + 10 || 11, err.message || "Unknown error", this.getSeverity(err.severity || "error"), err.code));
                        }
                    }
                    else if (results.errors) {
                        for (const err of results.errors) {
                            diagnostics.push(this.createDiagnostic(err.line || 1, err.column || 1, err.endLine || err.line || 1, err.endColumn || err.column + 10 || 11, err.message || "Unknown error", vscode.DiagnosticSeverity.Warning, err.code));
                        }
                    }
                }
            }
            catch {
                // If JSON parsing fails, try to parse plain text error output
                const parsedDiagnostics = this.parseTextOutput(stderr || stdout || "");
                diagnostics.push(...parsedDiagnostics);
            }
            // Also do basic syntax checking in the extension
            const syntaxDiagnostics = this.checkSyntax(document);
            diagnostics.push(...syntaxDiagnostics);
            this.diagnosticCollection.set(document.uri, diagnostics);
        });
    }
    /**
     * Parse text error output from pleno check
     * Expected formats:
     * - error: file.pln:10:5: message
     * - Error at line 10, column 5: message
     * - [Error] (10:5) message
     */
    parseTextOutput(output) {
        const diagnostics = [];
        const lines = output.split("\n");
        for (const line of lines) {
            // Format: file:line:column: severity: message
            let match = line.match(/^(?:.*?):(\d+):(\d+):\s*(error|warning|info|hint):\s*(.+)$/i);
            if (match) {
                const lineNum = parseInt(match[1]) - 1;
                const colNum = parseInt(match[2]) - 1;
                const severity = this.getSeverity(match[3]);
                const message = match[4];
                diagnostics.push(this.createDiagnostic(lineNum + 1, colNum + 1, lineNum + 1, colNum + 100, message, severity));
                continue;
            }
            // Format: Error at line X, column Y: message
            match = line.match(/^(error|warning|info|hint)\s+at\s+line\s+(\d+)(?:,\s*column\s+(\d+))?:\s*(.+)$/i);
            if (match) {
                const severity = this.getSeverity(match[1]);
                const lineNum = parseInt(match[2]);
                const colNum = match[3] ? parseInt(match[3]) : 1;
                const message = match[4];
                diagnostics.push(this.createDiagnostic(lineNum, colNum, lineNum, colNum + 100, message, severity));
                continue;
            }
            // Format: [Error] (line:col) message
            match = line.match(/^\[(error|warning|info|hint)\]\s*\((\d+):(\d+)\)\s*(.+)$/i);
            if (match) {
                const severity = this.getSeverity(match[1]);
                const lineNum = parseInt(match[2]);
                const colNum = parseInt(match[3]);
                const message = match[4];
                diagnostics.push(this.createDiagnostic(lineNum, colNum, lineNum, colNum + 100, message, severity));
                continue;
            }
            // Simple error format: error: message
            match = line.match(/^(error|warning):\s*(.+)$/i);
            if (match) {
                const severity = this.getSeverity(match[1]);
                const message = match[2];
                diagnostics.push(this.createDiagnostic(1, 1, 1, 100, message, severity));
            }
        }
        return diagnostics;
    }
    /**
     * Basic syntax checking done in the extension
     */
    checkSyntax(document) {
        const diagnostics = [];
        const text = document.getText();
        const lines = text.split("\n");
        let braceCount = 0;
        let parenCount = 0;
        let bracketCount = 0;
        let inString = false;
        let stringChar = "";
        let inMultilineComment = false;
        for (let lineIndex = 0; lineIndex < lines.length; lineIndex++) {
            const line = lines[lineIndex];
            for (let i = 0; i < line.length; i++) {
                const char = line[i];
                const nextChar = line[i + 1] || "";
                // Handle comments
                if (!inString) {
                    if (!inMultilineComment &&
                        char === "/" &&
                        nextChar === "/") {
                        break; // Rest of line is comment
                    }
                    if (!inMultilineComment &&
                        char === "/" &&
                        nextChar === "*") {
                        inMultilineComment = true;
                        i++;
                        continue;
                    }
                    if (inMultilineComment &&
                        char === "*" &&
                        nextChar === "/") {
                        inMultilineComment = false;
                        i++;
                        continue;
                    }
                    if (inMultilineComment) {
                        continue;
                    }
                }
                // Handle strings
                if (!inString && (char === '"' || char === "'")) {
                    inString = true;
                    stringChar = char;
                    continue;
                }
                if (inString && char === stringChar && line[i - 1] !== "\\") {
                    inString = false;
                    continue;
                }
                if (inString) {
                    continue;
                }
                // Count brackets
                if (char === "{")
                    braceCount++;
                if (char === "}")
                    braceCount--;
                if (char === "(")
                    parenCount++;
                if (char === ")")
                    parenCount--;
                if (char === "[")
                    bracketCount++;
                if (char === "]")
                    bracketCount--;
                // Check for immediate mismatches
                if (braceCount < 0) {
                    diagnostics.push(this.createDiagnostic(lineIndex + 1, i + 1, lineIndex + 1, i + 2, "Unexpected closing brace", vscode.DiagnosticSeverity.Error, "E001"));
                    braceCount = 0;
                }
                if (parenCount < 0) {
                    diagnostics.push(this.createDiagnostic(lineIndex + 1, i + 1, lineIndex + 1, i + 2, "Unexpected closing parenthesis", vscode.DiagnosticSeverity.Error, "E002"));
                    parenCount = 0;
                }
                if (bracketCount < 0) {
                    diagnostics.push(this.createDiagnostic(lineIndex + 1, i + 1, lineIndex + 1, i + 2, "Unexpected closing bracket", vscode.DiagnosticSeverity.Error, "E003"));
                    bracketCount = 0;
                }
            }
            // Check for unclosed string at end of line (unless it's a multiline string)
            if (inString) {
                diagnostics.push(this.createDiagnostic(lineIndex + 1, 1, lineIndex + 1, line.length + 1, "Unclosed string literal", vscode.DiagnosticSeverity.Error, "E004"));
                inString = false;
            }
        }
        // Check for unclosed brackets at end of file
        if (braceCount > 0) {
            diagnostics.push(this.createDiagnostic(lines.length, 1, lines.length, 100, `Missing ${braceCount} closing brace(s)`, vscode.DiagnosticSeverity.Error, "E005"));
        }
        if (parenCount > 0) {
            diagnostics.push(this.createDiagnostic(lines.length, 1, lines.length, 100, `Missing ${parenCount} closing parenthesis(es)`, vscode.DiagnosticSeverity.Error, "E006"));
        }
        if (bracketCount > 0) {
            diagnostics.push(this.createDiagnostic(lines.length, 1, lines.length, 100, `Missing ${bracketCount} closing bracket(s)`, vscode.DiagnosticSeverity.Error, "E007"));
        }
        if (inMultilineComment) {
            diagnostics.push(this.createDiagnostic(lines.length, 1, lines.length, 100, "Unclosed multiline comment", vscode.DiagnosticSeverity.Error, "E008"));
        }
        return diagnostics;
    }
    /**
     * Create a diagnostic object
     */
    createDiagnostic(startLine, startCol, endLine, endCol, message, severity, code) {
        const range = new vscode.Range(startLine - 1, startCol - 1, endLine - 1, endCol - 1);
        const diagnostic = new vscode.Diagnostic(range, message, severity);
        diagnostic.source = "pleno";
        if (code) {
            diagnostic.code = code;
        }
        return diagnostic;
    }
    /**
     * Convert severity string to DiagnosticSeverity
     */
    getSeverity(severity) {
        switch (severity.toLowerCase()) {
            case "error":
                return vscode.DiagnosticSeverity.Error;
            case "warning":
                return vscode.DiagnosticSeverity.Warning;
            case "info":
                return vscode.DiagnosticSeverity.Information;
            case "hint":
                return vscode.DiagnosticSeverity.Hint;
            default:
                return vscode.DiagnosticSeverity.Error;
        }
    }
    /**
     * Clear all diagnostics
     */
    clearAll() {
        this.diagnosticCollection.clear();
    }
    /**
     * Dispose of all resources
     */
    dispose() {
        this.diagnosticCollection.dispose();
        for (const timeout of this.pendingDiagnostics.values()) {
            clearTimeout(timeout);
        }
    }
}
exports.PlenoDiagnosticsProvider = PlenoDiagnosticsProvider;
/**
 * Pleno Code Action Provider
 * Provides quick fixes for diagnostics
 */
class PlenoCodeActionProvider {
    static providedCodeActionKinds = [
        vscode.CodeActionKind.QuickFix,
    ];
    provideCodeActions(document, _range, context) {
        const actions = [];
        for (const diagnostic of context.diagnostics) {
            if (diagnostic.source !== "pleno") {
                continue;
            }
            switch (diagnostic.code) {
                case "E001": // Unexpected closing brace
                    actions.push(this.createRemoveCharAction(document, diagnostic, "Remove unexpected closing brace"));
                    break;
                case "E002": // Unexpected closing parenthesis
                    actions.push(this.createRemoveCharAction(document, diagnostic, "Remove unexpected closing parenthesis"));
                    break;
                case "E003": // Unexpected closing bracket
                    actions.push(this.createRemoveCharAction(document, diagnostic, "Remove unexpected closing bracket"));
                    break;
                case "E005": // Missing closing brace
                    actions.push(this.createInsertCharAction(document, diagnostic, "Add missing closing brace", "}"));
                    break;
                case "E006": // Missing closing parenthesis
                    actions.push(this.createInsertCharAction(document, diagnostic, "Add missing closing parenthesis", ")"));
                    break;
                case "E007": // Missing closing bracket
                    actions.push(this.createInsertCharAction(document, diagnostic, "Add missing closing bracket", "]"));
                    break;
            }
        }
        return actions;
    }
    createRemoveCharAction(document, diagnostic, title) {
        const action = new vscode.CodeAction(title, vscode.CodeActionKind.QuickFix);
        action.diagnostics = [diagnostic];
        action.isPreferred = true;
        action.edit = new vscode.WorkspaceEdit();
        action.edit.delete(document.uri, diagnostic.range);
        return action;
    }
    createInsertCharAction(document, diagnostic, title, char) {
        const action = new vscode.CodeAction(title, vscode.CodeActionKind.QuickFix);
        action.diagnostics = [diagnostic];
        action.isPreferred = true;
        action.edit = new vscode.WorkspaceEdit();
        const position = new vscode.Position(document.lineCount - 1, document.lineAt(document.lineCount - 1).text.length);
        action.edit.insert(document.uri, position, char);
        return action;
    }
}
exports.PlenoCodeActionProvider = PlenoCodeActionProvider;
//# sourceMappingURL=diagnostics.js.map