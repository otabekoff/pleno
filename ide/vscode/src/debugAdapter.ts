import * as vscode from "vscode";
import { ChildProcess, spawn } from "child_process";
import * as path from "path";
import { getPlenoDapPath, getPlenoPath } from "./utils";

/**
 * Pleno Debug Adapter Factory
 * Creates debug adapter descriptors for Pleno debugging sessions
 */
export class PlenoDebugAdapterFactory
    implements vscode.DebugAdapterDescriptorFactory {
    private context: vscode.ExtensionContext;
    private outputChannel: vscode.OutputChannel;

    constructor(
        context: vscode.ExtensionContext,
        outputChannel: vscode.OutputChannel,
    ) {
        this.context = context;
        this.outputChannel = outputChannel;
    }

    createDebugAdapterDescriptor(
        _session: vscode.DebugSession,
        _executable: vscode.DebugAdapterExecutable | undefined,
    ): vscode.ProviderResult<vscode.DebugAdapterDescriptor> {
        // Debug with the real `pleno-dap` adapter (breakpoints, stepping,
        // variables) when it is installed.
        const dapPath = getPlenoDapPath(this.context);
        if (dapPath) {
            return new vscode.DebugAdapterExecutable(dapPath, []);
        }

        // Without it the program still runs, but stepping is only simulated.
        vscode.window.showWarningMessage(
            "pleno-dap was not found, so debugging is simulated. Install Pleno or set 'pleno.dap.path'.",
        );
        return new vscode.DebugAdapterInlineImplementation(
            new PlenoDebugSession(this.context, this.outputChannel),
        );
    }
}

/**
 * Pleno Debug Configuration Provider
 * Provides debug configurations for Pleno files
 */
export class PlenoDebugConfigurationProvider
    implements vscode.DebugConfigurationProvider {
    private context: vscode.ExtensionContext;

    constructor(context: vscode.ExtensionContext) {
        this.context = context;
    }

    resolveDebugConfiguration(
        folder: vscode.WorkspaceFolder | undefined,
        config: vscode.DebugConfiguration,
        _token?: vscode.CancellationToken,
    ): vscode.ProviderResult<vscode.DebugConfiguration> {
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

    provideDebugConfigurations(
        _folder: vscode.WorkspaceFolder | undefined,
        _token?: vscode.CancellationToken,
    ): vscode.ProviderResult<vscode.DebugConfiguration[]> {
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

// Debug protocol interfaces
interface DebugRequest {
    seq: number;
    type: string;
    command: string;
    arguments: Record<string, unknown>;
}

interface DebugResponse {
    seq: number;
    type: string;
    request_seq: number;
    success: boolean;
    command: string;
    message?: string;
    body?: unknown;
}

interface DebugEvent {
    seq: number;
    type: string;
    event: string;
    body?: unknown;
}

// Custom interface for breakpoints with line property
interface DebugBreakpoint {
    id: number;
    line: number;
    verified: boolean;
    source?: {
        path?: string;
    };
}

/**
 * Pleno Debug Session
 * Implements the Debug Adapter Protocol for Pleno
 */
class PlenoDebugSession implements vscode.DebugAdapter {
    private sendMessageEmitter = new vscode.EventEmitter<
        vscode.DebugProtocolMessage
    >();
    readonly onDidSendMessage: vscode.Event<vscode.DebugProtocolMessage> =
        this.sendMessageEmitter.event;

    private context: vscode.ExtensionContext;
    private outputChannel: vscode.OutputChannel;
    private process: ChildProcess | null = null;
    private seq: number = 1;

    // Debug state
    private breakpoints: Map<string, DebugBreakpoint[]> = new Map();
    private currentLine: number = 1;
    private currentFile: string = "";
    private paused: boolean = false;
    private variables: Map<string, unknown> = new Map();
    private programPath: string = "";
    private stopOnEntry: boolean = false;

    constructor(
        context: vscode.ExtensionContext,
        outputChannel: vscode.OutputChannel,
    ) {
        this.context = context;
        this.outputChannel = outputChannel;
    }

    handleMessage(message: vscode.DebugProtocolMessage): void {
        const request = message as DebugRequest;
        this.outputChannel.appendLine(
            `Debug request: ${request.command} - ${
                JSON.stringify(request.arguments)
            }`,
        );

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
                this.sendErrorResponse(
                    request,
                    `Unknown command: ${request.command}`,
                );
        }
    }

    private handleInitialize(request: DebugRequest): void {
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

    private handleLaunch(request: DebugRequest): void {
        const args = request.arguments as {
            program?: string;
            cwd?: string;
            args?: string[];
            stopOnEntry?: boolean;
        };

        this.programPath = args.program || "";
        this.currentFile = this.programPath;
        this.stopOnEntry = args.stopOnEntry || false;
        const cwd = args.cwd || path.dirname(this.programPath);
        const programArgs = args.args || [];

        this.outputChannel.appendLine(`Launching: ${this.programPath}`);
        this.outputChannel.appendLine(`CWD: ${cwd}`);

        const pleno = getPlenoPath(this.context);

        // Start the Pleno process
        this.process = spawn(
            pleno,
            ["run", this.programPath, "--debug", ...programArgs],
            {
                cwd,
                stdio: ["pipe", "pipe", "pipe"],
            },
        );

        this.process.stdout?.on("data", (data: Buffer) => {
            const output = data.toString();
            this.outputChannel.appendLine(`[stdout] ${output}`);
            this.sendEvent("output", {
                category: "stdout",
                output: output,
            });

            // Parse debug output for variable updates
            this.parseDebugOutput(output);
        });

        this.process.stderr?.on("data", (data: Buffer) => {
            const output = data.toString();
            this.outputChannel.appendLine(`[stderr] ${output}`);
            this.sendEvent("output", {
                category: "stderr",
                output: output,
            });
        });

        this.process.on("close", (code: number | null) => {
            this.outputChannel.appendLine(`Process exited with code ${code}`);
            this.sendEvent("terminated", {});
        });

        this.process.on("error", (err: Error) => {
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

    private handleSetBreakpoints(request: DebugRequest): void {
        const args = request.arguments as {
            source?: { path?: string };
            breakpoints?: { line: number }[];
        };

        const sourcePath = args.source?.path || "";
        const requestedBreakpoints = args.breakpoints || [];

        const breakpoints: DebugBreakpoint[] = requestedBreakpoints.map(
            (bp, index) => ({
                id: index + 1,
                line: bp.line,
                verified: true,
                source: { path: sourcePath },
            }),
        );

        this.breakpoints.set(sourcePath, breakpoints);

        this.sendResponse(request, { breakpoints });
    }

    private handleSetExceptionBreakpoints(request: DebugRequest): void {
        // Accept exception breakpoint configuration
        this.sendResponse(request, {});
    }

    private handleConfigurationDone(request: DebugRequest): void {
        this.sendResponse(request, {});

        // If not stopping on entry, simulate running until breakpoint
        if (!this.stopOnEntry) {
            this.simulateExecution();
        }
    }

    private handleThreads(request: DebugRequest): void {
        this.sendResponse(request, {
            threads: [{ id: 1, name: "Main Thread" }],
        });
    }

    private handleStackTrace(request: DebugRequest): void {
        const args = request.arguments as {
            startFrame?: number;
            levels?: number;
        };

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

    private handleScopes(request: DebugRequest): void {
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

    private handleVariables(request: DebugRequest): void {
        // variablesReference is available in request.arguments for filtering
        void request.arguments;

        // Return sample variables - in real implementation, this would come from the debugger
        const variables = Array.from(this.variables.entries()).map(
            ([name, value]) => ({
                name,
                value: String(value),
                type: typeof value,
                variablesReference: 0,
            }),
        );

        // Add some default variables if empty
        if (variables.length === 0) {
            variables.push(
                {
                    name: "x",
                    value: "0",
                    type: "number",
                    variablesReference: 0,
                },
                {
                    name: "y",
                    value: "0",
                    type: "number",
                    variablesReference: 0,
                },
            );
        }

        this.sendResponse(request, { variables });
    }

    private handleContinue(request: DebugRequest): void {
        this.paused = false;
        this.sendResponse(request, { allThreadsContinued: true });

        // Simulate running until next breakpoint or end
        this.simulateExecution();
    }

    private handleNext(request: DebugRequest): void {
        this.currentLine++;
        this.sendResponse(request, {});
        this.sendEvent("stopped", {
            reason: "step",
            threadId: 1,
            allThreadsStopped: true,
        });
    }

    private handleStepIn(request: DebugRequest): void {
        this.currentLine++;
        this.sendResponse(request, {});
        this.sendEvent("stopped", {
            reason: "step",
            threadId: 1,
            allThreadsStopped: true,
        });
    }

    private handleStepOut(request: DebugRequest): void {
        this.currentLine++;
        this.sendResponse(request, {});
        this.sendEvent("stopped", {
            reason: "step",
            threadId: 1,
            allThreadsStopped: true,
        });
    }

    private handlePause(request: DebugRequest): void {
        this.paused = true;
        this.sendResponse(request, {});
        this.sendEvent("stopped", {
            reason: "pause",
            threadId: 1,
            allThreadsStopped: true,
        });
    }

    private handleDisconnect(request: DebugRequest): void {
        if (this.process) {
            this.process.kill();
            this.process = null;
        }

        this.sendResponse(request, {});
    }

    private handleEvaluate(request: DebugRequest): void {
        const args = request.arguments as {
            expression?: string;
            context?: string;
        };
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

    private simulateExecution(): void {
        // Check for breakpoints
        const fileBreakpoints = this.breakpoints.get(this.currentFile) || [];
        const nextBreakpoint = fileBreakpoints.find(
            (bp) => bp.line > this.currentLine,
        );

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

    private parseDebugOutput(output: string): void {
        // Parse variable assignments from debug output
        // Format: [DEBUG] var_name = value
        const varMatch = output.match(/\[DEBUG\]\s+(\w+)\s*=\s*(.+)/);
        if (varMatch) {
            const name = varMatch[1];
            let value: unknown = varMatch[2].trim();

            // Try to parse as number
            const numValue = parseFloat(value as string);
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

    private sendResponse(request: DebugRequest, body?: unknown): void {
        const response: DebugResponse = {
            seq: this.seq++,
            type: "response",
            request_seq: request.seq,
            success: true,
            command: request.command,
            body,
        };

        this.sendMessageEmitter.fire(response as vscode.DebugProtocolMessage);
    }

    private sendErrorResponse(request: DebugRequest, message: string): void {
        const response: DebugResponse = {
            seq: this.seq++,
            type: "response",
            request_seq: request.seq,
            success: false,
            command: request.command,
            message,
        };

        this.sendMessageEmitter.fire(response as vscode.DebugProtocolMessage);
    }

    private sendEvent(event: string, body?: unknown): void {
        const evt: DebugEvent = {
            seq: this.seq++,
            type: "event",
            event,
            body,
        };

        this.sendMessageEmitter.fire(evt as vscode.DebugProtocolMessage);
    }

    dispose(): void {
        if (this.process) {
            this.process.kill();
            this.process = null;
        }
        this.sendMessageEmitter.dispose();
    }
}
