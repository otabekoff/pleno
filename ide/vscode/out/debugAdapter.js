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
exports.PlenoDebugConfigurationProvider = exports.PlenoDebugAdapterFactory = void 0;
const vscode = __importStar(require("vscode"));
const child_process_1 = require("child_process");
const path = __importStar(require("path"));
const utils_1 = require("./utils");
/**
 * Pleno Debug Adapter Factory
 * Creates debug adapter descriptors for Pleno debugging sessions
 */
class PlenoDebugAdapterFactory {
    context;
    outputChannel;
    constructor(context, outputChannel) {
        this.context = context;
        this.outputChannel = outputChannel;
    }
    createDebugAdapterDescriptor(_session, _executable) {
        // For now, we use an inline implementation
        // In the future, this can be replaced with an external debug adapter
        return new vscode.DebugAdapterInlineImplementation(new PlenoDebugSession(this.context, this.outputChannel));
    }
}
exports.PlenoDebugAdapterFactory = PlenoDebugAdapterFactory;
/**
 * Pleno Debug Configuration Provider
 * Provides debug configurations for Pleno files
 */
class PlenoDebugConfigurationProvider {
    context;
    constructor(context) {
        this.context = context;
    }
    resolveDebugConfiguration(folder, config, _token) {
        // If no program is specified, use the current file
        if (!config.program) {
            const editor = vscode.window.activeTextEditor;
            if (editor && editor.document.languageId === "pleno") {
                config.program = editor.document.uri.fsPath;
            }
        }
        // Set default values
        if (!config.type) {
            config.type = "pleno";
        }
        if (!config.name) {
            config.name = "Launch Pleno Program";
        }
        if (!config.request) {
            config.request = "launch";
        }
        if (!config.cwd) {
            config.cwd = folder?.uri.fsPath || "${workspaceFolder}";
        }
        if (config.stopOnEntry === undefined) {
            config.stopOnEntry = false;
        }
        return config;
    }
    provideDebugConfigurations(_folder, _token) {
        return [
            {
                type: "pleno",
                request: "launch",
                name: "Launch Pleno Program",
                program: "${file}",
                stopOnEntry: false,
                cwd: "${workspaceFolder}",
            },
            {
                type: "pleno",
                request: "launch",
                name: "Launch with Stop on Entry",
                program: "${file}",
                stopOnEntry: true,
                cwd: "${workspaceFolder}",
            },
        ];
    }
}
exports.PlenoDebugConfigurationProvider = PlenoDebugConfigurationProvider;
/**
 * Pleno Debug Session
 * Implements the Debug Adapter Protocol for Pleno
 */
class PlenoDebugSession {
    sendMessageEmitter = new vscode.EventEmitter();
    onDidSendMessage = this.sendMessageEmitter.event;
    context;
    outputChannel;
    process = null;
    seq = 1;
    // Debug state
    breakpoints = new Map();
    currentLine = 1;
    currentFile = "";
    paused = false;
    variables = new Map();
    programPath = "";
    stopOnEntry = false;
    constructor(context, outputChannel) {
        this.context = context;
        this.outputChannel = outputChannel;
    }
    handleMessage(message) {
        const request = message;
        this.outputChannel.appendLine(`Debug request: ${request.command} - ${JSON.stringify(request.arguments)}`);
        switch (request.command) {
            case "initialize":
                this.handleInitialize(request);
                break;
            case "launch":
                this.handleLaunch(request);
                break;
            case "setBreakpoints":
                this.handleSetBreakpoints(request);
                break;
            case "configurationDone":
                this.handleConfigurationDone(request);
                break;
            case "threads":
                this.handleThreads(request);
                break;
            case "stackTrace":
                this.handleStackTrace(request);
                break;
            case "scopes":
                this.handleScopes(request);
                break;
            case "variables":
                this.handleVariables(request);
                break;
            case "continue":
                this.handleContinue(request);
                break;
            case "next":
                this.handleNext(request);
                break;
            case "stepIn":
                this.handleStepIn(request);
                break;
            case "stepOut":
                this.handleStepOut(request);
                break;
            case "pause":
                this.handlePause(request);
                break;
            case "disconnect":
                this.handleDisconnect(request);
                break;
            case "evaluate":
                this.handleEvaluate(request);
                break;
            case "setExceptionBreakpoints":
                this.handleSetExceptionBreakpoints(request);
                break;
            default:
                this.sendErrorResponse(request, `Unknown command: ${request.command}`);
        }
    }
    handleInitialize(request) {
        this.sendResponse(request, {
            supportsConfigurationDoneRequest: true,
            supportsEvaluateForHovers: true,
            supportsStepBack: false,
            supportsSetVariable: false,
            supportsRestartFrame: false,
            supportsGotoTargetsRequest: false,
            supportsStepInTargetsRequest: false,
            supportsCompletionsRequest: false,
            supportsModulesRequest: false,
            supportsExceptionInfoRequest: false,
            supportsBreakpointLocationsRequest: false,
            supportTerminateDebuggee: true,
            supportsDelayedStackTraceLoading: false,
        });
        // Send initialized event
        this.sendEvent("initialized", {});
    }
    handleLaunch(request) {
        const args = request.arguments;
        this.programPath = args.program || "";
        this.currentFile = this.programPath;
        this.stopOnEntry = args.stopOnEntry || false;
        const cwd = args.cwd || path.dirname(this.programPath);
        const programArgs = args.args || [];
        this.outputChannel.appendLine(`Launching: ${this.programPath}`);
        this.outputChannel.appendLine(`CWD: ${cwd}`);
        const pleno = (0, utils_1.getPlenoPath)(this.context);
        // Start the Pleno process
        this.process = (0, child_process_1.spawn)(pleno, ["run", this.programPath, "--debug", ...programArgs], {
            cwd,
            stdio: ["pipe", "pipe", "pipe"],
        });
        this.process.stdout?.on("data", (data) => {
            const output = data.toString();
            this.outputChannel.appendLine(`[stdout] ${output}`);
            this.sendEvent("output", {
                category: "stdout",
                output: output,
            });
            // Parse debug output for variable updates
            this.parseDebugOutput(output);
        });
        this.process.stderr?.on("data", (data) => {
            const output = data.toString();
            this.outputChannel.appendLine(`[stderr] ${output}`);
            this.sendEvent("output", {
                category: "stderr",
                output: output,
            });
        });
        this.process.on("close", (code) => {
            this.outputChannel.appendLine(`Process exited with code ${code}`);
            this.sendEvent("terminated", {});
        });
        this.process.on("error", (err) => {
            this.outputChannel.appendLine(`Process error: ${err.message}`);
            this.sendEvent("output", {
                category: "stderr",
                output: `Error: ${err.message}\n`,
            });
            this.sendEvent("terminated", {});
        });
        this.sendResponse(request, {});
        // If stopOnEntry, pause at first line
        if (this.stopOnEntry) {
            this.currentLine = 1;
            this.paused = true;
            this.sendEvent("stopped", {
                reason: "entry",
                threadId: 1,
                allThreadsStopped: true,
            });
        }
    }
    handleSetBreakpoints(request) {
        const args = request.arguments;
        const sourcePath = args.source?.path || "";
        const requestedBreakpoints = args.breakpoints || [];
        const breakpoints = requestedBreakpoints.map((bp, index) => ({
            id: index + 1,
            line: bp.line,
            verified: true,
            source: { path: sourcePath },
        }));
        this.breakpoints.set(sourcePath, breakpoints);
        this.sendResponse(request, { breakpoints });
    }
    handleSetExceptionBreakpoints(request) {
        // Accept exception breakpoint configuration
        this.sendResponse(request, {});
    }
    handleConfigurationDone(request) {
        this.sendResponse(request, {});
        // If not stopping on entry, simulate running until breakpoint
        if (!this.stopOnEntry) {
            this.simulateExecution();
        }
    }
    handleThreads(request) {
        this.sendResponse(request, {
            threads: [{ id: 1, name: "Main Thread" }],
        });
    }
    handleStackTrace(request) {
        const args = request.arguments;
        const startFrame = args.startFrame || 0;
        const levels = args.levels || 20;
        const stackFrames = [
            {
                id: 1,
                name: "main",
                source: {
                    name: path.basename(this.currentFile),
                    path: this.currentFile,
                },
                line: this.currentLine,
                column: 1,
            },
        ];
        this.sendResponse(request, {
            stackFrames: stackFrames.slice(startFrame, startFrame + levels),
            totalFrames: stackFrames.length,
        });
    }
    handleScopes(request) {
        // frameId is available in request.arguments.frameId for future use
        void request.arguments;
        this.sendResponse(request, {
            scopes: [
                {
                    name: "Local",
                    variablesReference: 1,
                    expensive: false,
                },
                {
                    name: "Global",
                    variablesReference: 2,
                    expensive: true,
                },
            ],
        });
    }
    handleVariables(request) {
        // variablesReference is available in request.arguments for filtering
        void request.arguments;
        // Return sample variables - in real implementation, this would come from the debugger
        const variables = Array.from(this.variables.entries()).map(([name, value]) => ({
            name,
            value: String(value),
            type: typeof value,
            variablesReference: 0,
        }));
        // Add some default variables if empty
        if (variables.length === 0) {
            variables.push({
                name: "x",
                value: "0",
                type: "number",
                variablesReference: 0,
            }, {
                name: "y",
                value: "0",
                type: "number",
                variablesReference: 0,
            });
        }
        this.sendResponse(request, { variables });
    }
    handleContinue(request) {
        this.paused = false;
        this.sendResponse(request, { allThreadsContinued: true });
        // Simulate running until next breakpoint or end
        this.simulateExecution();
    }
    handleNext(request) {
        this.currentLine++;
        this.sendResponse(request, {});
        this.sendEvent("stopped", {
            reason: "step",
            threadId: 1,
            allThreadsStopped: true,
        });
    }
    handleStepIn(request) {
        this.currentLine++;
        this.sendResponse(request, {});
        this.sendEvent("stopped", {
            reason: "step",
            threadId: 1,
            allThreadsStopped: true,
        });
    }
    handleStepOut(request) {
        this.currentLine++;
        this.sendResponse(request, {});
        this.sendEvent("stopped", {
            reason: "step",
            threadId: 1,
            allThreadsStopped: true,
        });
    }
    handlePause(request) {
        this.paused = true;
        this.sendResponse(request, {});
        this.sendEvent("stopped", {
            reason: "pause",
            threadId: 1,
            allThreadsStopped: true,
        });
    }
    handleDisconnect(request) {
        if (this.process) {
            this.process.kill();
            this.process = null;
        }
        this.sendResponse(request, {});
    }
    handleEvaluate(request) {
        const args = request.arguments;
        const expression = args.expression || "";
        // context (args.context) can be 'watch', 'repl', 'hover' - for future use
        // Simple expression evaluation
        let result = `<${expression}>`;
        const variablesReference = 0;
        // Check if it's a known variable
        if (this.variables.has(expression)) {
            result = String(this.variables.get(expression));
        }
        this.sendResponse(request, {
            result,
            variablesReference,
        });
    }
    simulateExecution() {
        // Check for breakpoints
        const fileBreakpoints = this.breakpoints.get(this.currentFile) || [];
        const nextBreakpoint = fileBreakpoints.find((bp) => bp.line > this.currentLine);
        if (nextBreakpoint) {
            this.currentLine = nextBreakpoint.line;
            this.paused = true;
            this.sendEvent("stopped", {
                reason: "breakpoint",
                threadId: 1,
                hitBreakpointIds: [nextBreakpoint.id],
                allThreadsStopped: true,
            });
        }
    }
    parseDebugOutput(output) {
        // Parse variable assignments from debug output
        // Format: [DEBUG] var_name = value
        const varMatch = output.match(/\[DEBUG\]\s+(\w+)\s*=\s*(.+)/);
        if (varMatch) {
            const name = varMatch[1];
            let value = varMatch[2].trim();
            // Try to parse as number
            const numValue = parseFloat(value);
            if (!isNaN(numValue)) {
                value = numValue;
            }
            this.variables.set(name, value);
        }
        // Parse line number updates
        // Format: [DEBUG] line: 10
        const lineMatch = output.match(/\[DEBUG\]\s+line:\s*(\d+)/);
        if (lineMatch) {
            this.currentLine = parseInt(lineMatch[1]);
        }
    }
    sendResponse(request, body) {
        const response = {
            seq: this.seq++,
            type: "response",
            request_seq: request.seq,
            success: true,
            command: request.command,
            body,
        };
        this.sendMessageEmitter.fire(response);
    }
    sendErrorResponse(request, message) {
        const response = {
            seq: this.seq++,
            type: "response",
            request_seq: request.seq,
            success: false,
            command: request.command,
            message,
        };
        this.sendMessageEmitter.fire(response);
    }
    sendEvent(event, body) {
        const evt = {
            seq: this.seq++,
            type: "event",
            event,
            body,
        };
        this.sendMessageEmitter.fire(evt);
    }
    dispose() {
        if (this.process) {
            this.process.kill();
            this.process = null;
        }
        this.sendMessageEmitter.dispose();
    }
}
//# sourceMappingURL=debugAdapter.js.map