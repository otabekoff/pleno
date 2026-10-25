import * as vscode from "vscode";
import * as path from "path";
import { execFile } from "child_process";
import { getPlenoPath } from "./utils";
import { FileResult, findTests, FoundTest, parseTestOutput } from "./testParser";

/** Whether a file uses `std:test` and so may contain tests. */
function usesStdTest(text: string): boolean {
    return /from\s+["']std:test["']/.test(text);
}

/**
 * The Testing view for `std:test` tests. Files are run with
 * `pleno test <file> --json`: the ✓ and ✗ lines `std:test` prints give each
 * test's result, and the JSON summary reports files that did not compile or
 * stopped with an error.
 */
export class PlenoTestController implements vscode.Disposable {
    private readonly controller: vscode.TestController;
    private readonly disposables: vscode.Disposable[] = [];

    constructor(
        private readonly context: vscode.ExtensionContext,
        private readonly output: vscode.OutputChannel,
    ) {
        this.controller = vscode.tests.createTestController("pleno", "Pleno");
        this.controller.resolveHandler = async (item) => {
            if (!item) {
                await this.discoverAll();
            }
        };
        this.controller.refreshHandler = () => this.refresh();
        this.controller.createRunProfile(
            "Run",
            vscode.TestRunProfileKind.Run,
            (request, token) => this.run(request, token),
            true,
        );
        this.controller.createRunProfile(
            "Debug",
            vscode.TestRunProfileKind.Debug,
            (request) => this.debug(request),
            true,
        );

        const watcher = vscode.workspace.createFileSystemWatcher("**/*.pln");
        this.disposables.push(
            this.controller,
            watcher,
            watcher.onDidCreate((uri) => this.updateFromDisk(uri)),
            watcher.onDidChange((uri) => this.updateFromDisk(uri)),
            watcher.onDidDelete((uri) => this.controller.items.delete(uri.toString())),
            vscode.workspace.onDidChangeTextDocument((event) => {
                if (event.document.languageId === "pleno" && event.document.uri.scheme === "file") {
                    this.update(event.document.uri, event.document.getText());
                }
            }),
        );
    }

    /** Forgets every test and finds them again. */
    async refresh(): Promise<void> {
        this.controller.items.replace([]);
        await this.discoverAll();
    }

    private async discoverAll(): Promise<void> {
        const files = await vscode.workspace.findFiles(
            "**/*.pln",
            "{**/node_modules/**,**/packages/**,**/target/**}",
        );
        await Promise.all(files.map((uri) => this.updateFromDisk(uri)));
    }

    private async updateFromDisk(uri: vscode.Uri): Promise<void> {
        try {
            const bytes = await vscode.workspace.fs.readFile(uri);
            this.update(uri, Buffer.from(bytes).toString("utf-8"));
        } catch {
            this.controller.items.delete(uri.toString());
        }
    }

    /** Replaces a file's tests with those found in `text`. */
    private update(uri: vscode.Uri, text: string): void {
        const found = usesStdTest(text) ? findTests(text) : [];
        if (found.length === 0) {
            this.controller.items.delete(uri.toString());
            return;
        }
        let file = this.controller.items.get(uri.toString());
        if (!file) {
            file = this.controller.createTestItem(uri.toString(), path.basename(uri.fsPath), uri);
            file.description = vscode.workspace.asRelativePath(path.dirname(uri.fsPath));
            this.controller.items.add(file);
        }
        file.children.replace(this.createItems(uri, file.id, found));
    }

    private createItems(uri: vscode.Uri, parentId: string, tests: FoundTest[]): vscode.TestItem[] {
        const seen = new Set<string>();
        return tests.map((test) => {
            let id = `${parentId}/${test.name}`;
            for (let n = 2; seen.has(id); n++) {
                id = `${parentId}/${test.name} (${n})`;
            }
            seen.add(id);
            const item = this.controller.createTestItem(id, test.name, uri);
            item.range = new vscode.Range(test.line, 0, test.line, 0);
            item.children.replace(this.createItems(uri, id, test.children));
            return item;
        });
    }

    private async run(request: vscode.TestRunRequest, token: vscode.CancellationToken): Promise<void> {
        const run = this.controller.createTestRun(request);
        const excluded = new Set(request.exclude?.map((item) => item.id));
        // Tests to report, by file: a file runs as a whole.
        const byFile = new Map<vscode.TestItem, Set<vscode.TestItem>>();
        const collect = (item: vscode.TestItem, into: Set<vscode.TestItem>) => {
            if (excluded.has(item.id)) {
                return;
            }
            into.add(item);
            item.children.forEach((child) => collect(child, into));
        };
        const roots: vscode.TestItem[] = [];
        if (request.include) {
            roots.push(...request.include);
        } else {
            this.controller.items.forEach((item) => roots.push(item));
        }
        for (const root of roots) {
            let file = root;
            while (file.parent) {
                file = file.parent;
            }
            const items = byFile.get(file) ?? new Set<vscode.TestItem>();
            collect(root, items);
            byFile.set(file, items);
        }

        for (const [file, items] of byFile) {
            if (token.isCancellationRequested) {
                break;
            }
            items.forEach((item) => run.started(item));
            await this.runFile(run, file, items, token);
        }
        run.end();
    }

    private runFile(
        run: vscode.TestRun,
        file: vscode.TestItem,
        items: Set<vscode.TestItem>,
        token: vscode.CancellationToken,
    ): Promise<void> {
        const filePath = file.uri!.fsPath;
        const cwd = vscode.workspace.getWorkspaceFolder(file.uri!)?.uri.fsPath ?? path.dirname(filePath);
        const started = Date.now();
        return new Promise((resolve) => {
            const child = execFile(
                getPlenoPath(this.context),
                ["test", filePath, "--json"],
                { cwd, timeout: 120_000, env: { ...process.env, NO_COLOR: "1" }, maxBuffer: 16 * 1024 * 1024 },
                (error, stdout, stderr) => {
                    if (token.isCancellationRequested) {
                        items.forEach((item) => run.skipped(item));
                    } else if (error && !stdout) {
                        const message = `Could not run pleno test: ${stderr || error.message}`;
                        this.output.appendLine(message);
                        items.forEach((item) => run.errored(item, new vscode.TestMessage(message)));
                    } else {
                        this.report(run, file, items, stdout, stderr, Date.now() - started);
                    }
                    resolve();
                },
            );
            token.onCancellationRequested(() => child.kill());
        });
    }

    /** Reports what `pleno test --json` printed for the tests in `items`. */
    private report(
        run: vscode.TestRun,
        file: vscode.TestItem,
        items: Set<vscode.TestItem>,
        stdout: string,
        stderr: string,
        duration: number,
    ): void {
        const output = parseTestOutput(stdout, (names) => this.child(file, names) !== undefined);
        run.appendOutput((output.printed + stderr).replace(/\r?\n/g, "\r\n"), undefined, file);

        const reported = new Set<vscode.TestItem>();
        const failed = new Set<vscode.TestItem>();
        for (const result of output.results) {
            const item = this.child(file, result.names)
                ?? this.onlyMatch(file, result.names)
                ?? this.addDynamic(file, result.names);
            if (!item || !this.isWanted(item, items)) {
                continue;
            }
            reported.add(item);
            if (result.passed) {
                run.passed(item);
            } else {
                failed.add(item);
                const message = new vscode.TestMessage(result.message ?? "Test failed");
                if (item.uri && item.range) {
                    message.location = new vscode.Location(item.uri, item.range);
                }
                run.failed(item, message);
            }
        }

        // Suites pass or fail with their tests. A file that did not compile or
        // stopped with an error fails, and so do the tests that did not run.
        const stopped = output.file?.status === "failed" && !output.onlyTestsFailed;
        for (const item of items) {
            if (reported.has(item)) {
                continue;
            }
            if (stopped && item === file) {
                run.failed(item, this.errorMessage(file, output.file!), duration);
            } else if (item.children.size > 0 || item === file) {
                if (stopped || [...failed].some((test) => this.isInside(test, item))) {
                    run.failed(item, new vscode.TestMessage("Some tests failed"));
                } else {
                    run.passed(item, item === file ? duration : undefined);
                }
            } else if (stopped) {
                run.errored(item, this.errorMessage(file, output.file!));
            } else {
                run.skipped(item);
            }
        }
    }

    private errorMessage(file: vscode.TestItem, result: FileResult): vscode.TestMessage {
        const kind = result.error_type ? `${result.error_type} error: ` : "";
        const message = new vscode.TestMessage(`${kind}${result.message ?? "the file failed"}`);
        message.location = new vscode.Location(file.uri!, new vscode.Position(0, 0));
        return message;
    }

    private isWanted(item: vscode.TestItem, items: Set<vscode.TestItem>): boolean {
        for (let node: vscode.TestItem | undefined = item; node; node = node.parent) {
            if (items.has(node)) {
                return true;
            }
        }
        return false;
    }

    private isInside(item: vscode.TestItem, ancestor: vscode.TestItem): boolean {
        for (let node = item.parent; node; node = node.parent) {
            if (node === ancestor) {
                return true;
            }
        }
        return false;
    }

    /** The test under `file` with the suite and test `names`, if found in the source. */
    private child(file: vscode.TestItem, names: string[]): vscode.TestItem | undefined {
        let node: vscode.TestItem | undefined = file;
        for (const name of names) {
            let next: vscode.TestItem | undefined;
            node.children.forEach((child) => {
                if (!next && child.label === name) {
                    next = child;
                }
            });
            if (!next) {
                return undefined;
            }
            node = next;
        }
        return node;
    }

    /**
     * The one test with this name at this depth, for when the program's own
     * output got in the way of reading the suite names.
     */
    private onlyMatch(file: vscode.TestItem, names: string[]): vscode.TestItem | undefined {
        const matches: vscode.TestItem[] = [];
        const visit = (node: vscode.TestItem, depth: number) => node.children.forEach((child) => {
            if (depth === names.length - 1) {
                if (child.label === names[depth]) {
                    matches.push(child);
                }
            } else {
                visit(child, depth + 1);
            }
        });
        visit(file, 0);
        return matches.length === 1 ? matches[0] : undefined;
    }

    /** Adds a test whose name is computed at run time, under its suite. */
    private addDynamic(file: vscode.TestItem, names: string[]): vscode.TestItem | undefined {
        const parent = this.child(file, names.slice(0, -1));
        if (!parent) {
            return undefined;
        }
        const name = names[names.length - 1];
        const item = this.controller.createTestItem(`${parent.id}/${name}`, name, file.uri);
        parent.children.add(item);
        return item;
    }

    /** Debugs the files of the selected tests; the debugger shows the output. */
    private async debug(request: vscode.TestRunRequest): Promise<void> {
        const files = new Set<string>();
        const roots = [...(request.include ?? [])];
        if (roots.length === 0) {
            this.controller.items.forEach((item) => roots.push(item));
        }
        for (const item of roots) {
            if (item.uri) {
                files.add(item.uri.fsPath);
            }
        }
        for (const program of files) {
            await vscode.debug.startDebugging(
                vscode.workspace.getWorkspaceFolder(vscode.Uri.file(program)),
                { type: "pleno", request: "launch", name: `Debug ${path.basename(program)}`, program },
            );
        }
    }

    dispose(): void {
        this.disposables.forEach((disposable) => disposable.dispose());
    }
}
