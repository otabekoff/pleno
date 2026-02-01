import * as vscode from "vscode";
import * as path from "path";
import { exec } from "child_process";
import { getPlenoPath } from "./utils";

/**
 * Pleno Test Controller
 * Provides test discovery and execution for the VS Code Test Explorer
 */
export class PlenoTestController {
    private testController: vscode.TestController;
    private context: vscode.ExtensionContext;
    private outputChannel: vscode.OutputChannel;
    private testItemMap: Map<string, vscode.TestItem> = new Map();
    private runningTests: Map<string, vscode.CancellationTokenSource> =
        new Map();

    constructor(
        context: vscode.ExtensionContext,
        outputChannel: vscode.OutputChannel,
    ) {
        this.context = context;
        this.outputChannel = outputChannel;

        // Create test controller
        this.testController = vscode.tests.createTestController(
            "plenoTests",
            "Pleno Tests",
        );
        context.subscriptions.push(this.testController);

        // Set up resolve handler for lazy loading
        this.testController.resolveHandler = async (item) => {
            if (!item) {
                await this.discoverAllTests();
            } else {
                await this.resolveTestItem(item);
            }
        };

        // Create run profile for running tests
        this.testController.createRunProfile(
            "Run Tests",
            vscode.TestRunProfileKind.Run,
            (request, token) => this.runTests(request, token),
            true, // isDefault
        );

        // Create run profile for debugging tests
        this.testController.createRunProfile(
            "Debug Tests",
            vscode.TestRunProfileKind.Debug,
            (request, token) => this.runTests(request, token, true),
            false,
        );

        // Create run profile for coverage
        this.testController.createRunProfile(
            "Run with Coverage",
            vscode.TestRunProfileKind.Coverage,
            (request, token) => this.runTests(request, token, false, true),
            false,
        );

        // Watch for file changes
        this.setupFileWatcher();
    }

    /**
     * Set up file watcher to detect test file changes
     */
    private setupFileWatcher(): void {
        const watcher = vscode.workspace.createFileSystemWatcher("**/*.pln");

        watcher.onDidCreate(async (uri) => {
            await this.addTestFile(uri);
        });

        watcher.onDidChange(async (uri) => {
            await this.updateTestFile(uri);
        });

        watcher.onDidDelete((uri) => {
            this.removeTestFile(uri);
        });

        this.context.subscriptions.push(watcher);
    }

    /**
     * Discover all tests in the workspace
     */
    public async discoverAllTests(): Promise<void> {
        this.outputChannel.appendLine("[Test] Discovering tests...");

        const workspaceFolders = vscode.workspace.workspaceFolders;
        if (!workspaceFolders) {
            return;
        }

        for (const folder of workspaceFolders) {
            // Find all .pln files
            const files = await vscode.workspace.findFiles(
                new vscode.RelativePattern(folder, "**/*.pln"),
                "**/node_modules/**",
            );

            for (const file of files) {
                await this.addTestFile(file);
            }
        }

        this.outputChannel.appendLine(
            `[Test] Found ${this.testItemMap.size} test files`,
        );
    }

    /**
     * Add a test file and discover tests within it
     */
    private async addTestFile(uri: vscode.Uri): Promise<void> {
        const content = await this.readFile(uri);
        if (!content) {
            return;
        }

        // Check if file contains tests
        const tests = this.parseTestsFromContent(content);
        if (tests.length === 0) {
            return;
        }

        // Create or update test item for the file
        const fileItem = this.getOrCreateFileItem(uri);

        // Add test items for each test function
        for (const test of tests) {
            const testId = `${uri.toString()}#${test.name}`;
            let testItem = this.testItemMap.get(testId);

            if (!testItem) {
                testItem = this.testController.createTestItem(
                    testId,
                    test.name,
                    uri,
                );
                testItem.range = new vscode.Range(
                    new vscode.Position(test.line, 0),
                    new vscode.Position(test.line, 1000),
                );
                fileItem.children.add(testItem);
                this.testItemMap.set(testId, testItem);
            } else {
                // Update range
                testItem.range = new vscode.Range(
                    new vscode.Position(test.line, 0),
                    new vscode.Position(test.line, 1000),
                );
            }

            // Add description/tags
            if (test.description) {
                testItem.description = test.description;
            }
            if (test.tags && test.tags.length > 0) {
                testItem.tags = test.tags.map((tag) => new vscode.TestTag(tag));
            }
        }
    }

    /**
     * Update test file when content changes
     */
    private async updateTestFile(uri: vscode.Uri): Promise<void> {
        // Remove existing tests for this file
        this.removeTestFile(uri);
        // Re-add with updated content
        await this.addTestFile(uri);
    }

    /**
     * Remove test file and its tests
     */
    private removeTestFile(uri: vscode.Uri): void {
        const fileId = uri.toString();
        const fileItem = this.testItemMap.get(fileId);

        if (fileItem) {
            this.testController.items.delete(fileId);
            this.testItemMap.delete(fileId);

            // Remove child test items
            fileItem.children.forEach((child) => {
                this.testItemMap.delete(child.id);
            });
        }
    }

    /**
     * Get or create a test item for a file
     */
    private getOrCreateFileItem(uri: vscode.Uri): vscode.TestItem {
        const fileId = uri.toString();
        let fileItem = this.testItemMap.get(fileId);

        if (!fileItem) {
            const fileName = path.basename(uri.fsPath);
            fileItem = this.testController.createTestItem(
                fileId,
                fileName,
                uri,
            );
            this.testController.items.add(fileItem);
            this.testItemMap.set(fileId, fileItem);
        }

        return fileItem;
    }

    /**
     * Parse test functions from file content
     */
    private parseTestsFromContent(content: string): TestInfo[] {
        const tests: TestInfo[] = [];
        const lines = content.split("\n");

        for (let i = 0; i < lines.length; i++) {
            const line = lines[i];

            // Match test function patterns:
            // #[test] fn test_name()
            // test fn test_name()
            // fn test_name() // with test_ prefix
            // @test fn name()

            // Check for #[test] or @test attribute on previous line
            const prevLine = i > 0 ? lines[i - 1].trim() : "";
            const hasTestAttr = prevLine.match(/^(#\[test\]|@test)/);

            // Match function declaration
            const fnMatch = line.match(/^\s*(pub\s+)?fn\s+(\w+)\s*\(/);
            if (fnMatch) {
                const fnName = fnMatch[2];

                // Check if it's a test function
                const isTest = hasTestAttr ||
                    fnName.startsWith("test_") ||
                    fnName.startsWith("test") ||
                    fnName.endsWith("_test") ||
                    fnName.includes("_test_");

                if (isTest) {
                    // Extract description from comment
                    let description = "";
                    if (i > 0) {
                        const commentMatch = lines[i - 1].match(
                            /^\s*\/\/\s*(.+)/,
                        );
                        if (commentMatch && !hasTestAttr) {
                            description = commentMatch[1];
                        }
                    }

                    // Extract tags from attributes
                    const tags: string[] = [];
                    if (hasTestAttr) {
                        const tagMatch = prevLine.match(
                            /\[test(?:\s*,\s*(\w+))*\]/,
                        );
                        if (tagMatch) {
                            // Parse additional tags
                        }
                    }

                    tests.push({
                        name: fnName,
                        line: i,
                        description,
                        tags,
                    });
                }
            }
        }

        return tests;
    }

    /**
     * Resolve a test item (for lazy loading)
     */
    private async resolveTestItem(item: vscode.TestItem): Promise<void> {
        if (item.uri) {
            await this.addTestFile(item.uri);
        }
    }

    /**
     * Run tests
     */
    private async runTests(
        request: vscode.TestRunRequest,
        token: vscode.CancellationToken,
        debug: boolean = false,
        coverage: boolean = false,
    ): Promise<void> {
        const run = this.testController.createTestRun(request);
        const queue: vscode.TestItem[] = [];

        // Collect tests to run
        if (request.include) {
            request.include.forEach((item) => this.collectTests(item, queue));
        } else {
            this.testController.items.forEach((item) =>
                this.collectTests(item, queue)
            );
        }

        // Filter excluded tests
        const excludedIds = new Set(
            request.exclude?.map((item) => item.id) || [],
        );
        const testsToRun = queue.filter((item) => !excludedIds.has(item.id));

        this.outputChannel.appendLine(
            `[Test] Running ${testsToRun.length} tests...`,
        );

        // Group tests by file
        const testsByFile = new Map<string, vscode.TestItem[]>();
        for (const test of testsToRun) {
            const fileUri = test.uri?.toString() || "";
            const fileTests = testsByFile.get(fileUri) || [];
            fileTests.push(test);
            testsByFile.set(fileUri, fileTests);
        }

        // Run tests for each file
        for (const [fileUri, tests] of testsByFile) {
            if (token.isCancellationRequested) {
                tests.forEach((test) => run.skipped(test));
                continue;
            }

            await this.runFileTests(
                run,
                fileUri,
                tests,
                token,
                debug,
                coverage,
            );
        }

        run.end();
    }

    /**
     * Run tests for a specific file
     */
    private async runFileTests(
        run: vscode.TestRun,
        fileUri: string,
        tests: vscode.TestItem[],
        token: vscode.CancellationToken,
        debug: boolean,
        coverage: boolean,
    ): Promise<void> {
        const filePath = vscode.Uri.parse(fileUri).fsPath;
        const plenoPath = getPlenoPath(this.context);

        // Mark tests as started
        tests.forEach((test) => run.started(test));

        // Build command
        let command = `"${plenoPath}" test "${filePath}" --json`;
        if (coverage) {
            command += " --coverage";
        }

        // Extract test names for filtering
        const testNames = tests
            .filter((t) => !t.children.size) // Only leaf tests
            .map((t) => t.label);

        if (testNames.length > 0) {
            command += ` --filter "${testNames.join("|")}"`;
        }

        this.outputChannel.appendLine(`[Test] Running: ${command}`);

        return new Promise((resolve) => {
            const startTime = Date.now();

            const process = exec(
                command,
                { timeout: 60000 },
                (error, stdout, stderr) => {
                    const duration = Date.now() - startTime;

                    if (token.isCancellationRequested) {
                        tests.forEach((test) => run.skipped(test));
                        resolve();
                        return;
                    }

                    try {
                        // Try to parse JSON output
                        const result = JSON.parse(stdout);
                        this.processTestResults(run, tests, result, duration);
                    } catch {
                        // If JSON parsing fails, try to parse text output
                        this.processTextOutput(
                            run,
                            tests,
                            stdout,
                            stderr,
                            duration,
                            error,
                        );
                    }

                    resolve();
                },
            );

            // Handle cancellation
            token.onCancellationRequested(() => {
                process.kill();
            });
        });
    }

    /**
     * Process JSON test results
     */
    private processTestResults(
        run: vscode.TestRun,
        tests: vscode.TestItem[],
        result: any,
        defaultDuration: number,
    ): void {
        const testResults = result.results || result.tests || [];
        const testMap = new Map(tests.map((t) => [t.label, t]));

        for (const testResult of testResults) {
            const testItem = testMap.get(testResult.name);
            if (!testItem) {
                continue;
            }

            const duration = testResult.duration || defaultDuration;

            switch (testResult.status || testResult.result) {
                case "passed":
                case "pass":
                case "ok":
                    run.passed(testItem, duration);
                    break;

                case "failed":
                case "fail":
                case "error":
                    const message = new vscode.TestMessage(
                        testResult.message || testResult.error || "Test failed",
                    );
                    if (testResult.location) {
                        message.location = new vscode.Location(
                            testItem.uri!,
                            new vscode.Position(
                                testResult.location.line - 1,
                                testResult.location.column - 1,
                            ),
                        );
                    }
                    run.failed(testItem, message, duration);
                    break;

                case "skipped":
                case "skip":
                case "ignored":
                    run.skipped(testItem);
                    break;

                default:
                    run.passed(testItem, duration);
            }

            testMap.delete(testResult.name);
        }

        // Mark any remaining tests as failed (not in results)
        for (const item of testMap.values()) {
            if (item.children.size === 0) {
                run.errored(
                    item,
                    new vscode.TestMessage("Test not found in results"),
                );
            }
        }
    }

    /**
     * Process text output when JSON parsing fails
     */
    private processTextOutput(
        run: vscode.TestRun,
        tests: vscode.TestItem[],
        stdout: string,
        stderr: string,
        duration: number,
        error: Error | null,
    ): void {
        const output = stdout + "\n" + stderr;

        // Try to parse individual test results from text
        // Common formats:
        // test test_name ... ok
        // test test_name ... FAILED
        // ✓ test_name
        // ✗ test_name

        const testMap = new Map(tests.map((t) => [t.label, t]));

        // Pattern: test test_name ... ok/FAILED
        const testPattern =
            /test\s+(\w+)\s*\.\.\.\s*(ok|FAILED|passed|failed)/gi;
        let match;

        while ((match = testPattern.exec(output)) !== null) {
            const testName = match[1];
            const status = match[2].toLowerCase();
            const testItem = testMap.get(testName);

            if (testItem) {
                if (status === "ok" || status === "passed") {
                    run.passed(testItem, duration / tests.length);
                } else {
                    run.failed(
                        testItem,
                        new vscode.TestMessage("Test failed"),
                        duration / tests.length,
                    );
                }
                testMap.delete(testName);
            }
        }

        // Pattern: ✓/✗ test_name or [PASS]/[FAIL] test_name
        const symbolPattern = /(✓|✗|\[PASS\]|\[FAIL\])\s+(\w+)/g;
        while ((match = symbolPattern.exec(output)) !== null) {
            const symbol = match[1];
            const testName = match[2];
            const testItem = testMap.get(testName);

            if (testItem) {
                if (symbol === "✓" || symbol === "[PASS]") {
                    run.passed(testItem, duration / tests.length);
                } else {
                    run.failed(
                        testItem,
                        new vscode.TestMessage("Test failed"),
                        duration / tests.length,
                    );
                }
                testMap.delete(testName);
            }
        }

        // Handle remaining tests
        for (const item of testMap.values()) {
            if (item.children.size > 0) {
                // File item - skip
                continue;
            }

            if (error) {
                run.errored(
                    item,
                    new vscode.TestMessage(
                        `Error: ${error.message}\n${output}`,
                    ),
                );
            } else if (output.includes("passed") || output.includes("ok")) {
                // Assume passed if no specific failure
                run.passed(item, duration / tests.length);
            } else {
                run.errored(
                    item,
                    new vscode.TestMessage(
                        `Could not determine test result\n${output}`,
                    ),
                );
            }
        }
    }

    /**
     * Collect all test items recursively
     */
    private collectTests(
        item: vscode.TestItem,
        queue: vscode.TestItem[],
    ): void {
        if (item.children.size === 0) {
            queue.push(item);
        } else {
            item.children.forEach((child) => this.collectTests(child, queue));
        }
    }

    /**
     * Read file content
     */
    private async readFile(uri: vscode.Uri): Promise<string | null> {
        try {
            const content = await vscode.workspace.fs.readFile(uri);
            return Buffer.from(content).toString("utf-8");
        } catch {
            return null;
        }
    }

    /**
     * Refresh all tests
     */
    public refresh(): void {
        this.testController.items.forEach((item) => {
            this.testItemMap.delete(item.id);
        });
        this.testController.items.replace([]);
        this.testItemMap.clear();
        this.discoverAllTests();
    }

    /**
     * Dispose resources
     */
    public dispose(): void {
        this.testController.dispose();
        this.testItemMap.clear();
        this.runningTests.forEach((cts) => cts.cancel());
        this.runningTests.clear();
    }
}

/**
 * Test information structure
 */
interface TestInfo {
    name: string;
    line: number;
    description?: string;
    tags?: string[];
}
