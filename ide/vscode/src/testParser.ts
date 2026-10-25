/** A `describe(...)` or `test(...)` call found in a file. */
export interface FoundTest {
    name: string;
    kind: "describe" | "test";
    line: number;
    children: FoundTest[];
}

/**
 * Finds the `describe("name", ...)` and `test("name", ...)` calls of
 * `std:test` in a file, nested as in the source. Strings and comments are
 * skipped; names must be plain string literals.
 */
export function findTests(text: string): FoundTest[] {
    const roots: FoundTest[] = [];
    // Open calls, each with the parenthesis depth inside its argument list.
    const open: { test: FoundTest; depth: number }[] = [];
    let depth = 0;
    let line = 0;
    let i = 0;
    // Leaves `i` on the closing quote. `${...}` may hold strings of its own.
    const skipString = (quote: string): void => {
        for (i++; i < text.length && text[i] !== quote; i++) {
            if (text[i] === "\\") {
                i++;
            } else if (text[i] === "\n") {
                line++;
            } else if (quote !== "'" && text[i] === "$" && text[i + 1] === "{") {
                let braces = 0;
                for (i++; i < text.length; i++) {
                    if (text[i] === "{") {
                        braces++;
                    } else if (text[i] === "}" && --braces === 0) {
                        break;
                    } else if (text[i] === "\"" || text[i] === "`") {
                        skipString(text[i]);
                    } else if (text[i] === "\n") {
                        line++;
                    }
                }
            }
        }
    };
    while (i < text.length) {
        const ch = text[i];
        if (ch === "\n") {
            line++;
        } else if (ch === "/" && text[i + 1] === "/") {
            while (i < text.length && text[i] !== "\n") {
                i++;
            }
            continue;
        } else if (ch === "/" && text[i + 1] === "*") {
            const end = text.indexOf("*/", i + 2);
            const stop = end < 0 ? text.length : end + 2;
            line += (text.slice(i, stop).match(/\n/g) ?? []).length;
            i = stop;
            continue;
        } else if (ch === "\"" || ch === "`" || ch === "'") {
            skipString(ch);
        } else if (ch === "(") {
            depth++;
        } else if (ch === ")") {
            depth--;
            while (open.length && open[open.length - 1].depth > depth) {
                open.pop();
            }
        } else if (/[A-Za-z_]/.test(ch) && !/[\w.]/.test(text[i - 1] ?? "")) {
            const call = /^(describe|test)\s*\(\s*(?:"((?:[^"\\\n$]|\\.)*)"|`((?:[^`\\$]|\\.)*)`)/
                .exec(text.slice(i, i + 300));
            if (call) {
                const found: FoundTest = {
                    name: (call[2] ?? call[3]).replace(/\\(.)/g, "$1"),
                    kind: call[1] as "describe" | "test",
                    line,
                    children: [],
                };
                (open.length ? open[open.length - 1].test.children : roots).push(found);
                depth++;
                open.push({ test: found, depth });
                i += call[0].length;
                continue;
            }
            while (i < text.length && /\w/.test(text[i])) {
                i++;
            }
            continue;
        }
        i++;
    }
    return roots;
}

/** The file result in the JSON summary of `pleno test --json`. */
export interface FileResult {
    status: "passed" | "failed" | "skipped" | "error";
    error_type?: string;
    message?: string;
}

/** One `✓ name` or `✗ name` line, with the suites it is in. */
export interface TestResult {
    names: string[];
    passed: boolean;
    message?: string;
}

export interface TestOutput {
    /** What the file printed, without the JSON summary. */
    printed: string;
    results: TestResult[];
    file?: FileResult;
    /** True when the file failed only because some of its tests failed. */
    onlyTestsFailed: boolean;
}

/**
 * Reads the output of `pleno test <file> --json`: `std:test` prints each
 * `describe` name, then `✓ name` or `✗ name` and the failure message, two
 * spaces deeper per level, and the JSON summary comes last. A line that is
 * not a result is a suite only if `isSuite` knows it, as the program's own
 * output can look the same.
 */
export function parseTestOutput(stdout: string, isSuite: (names: string[]) => boolean): TestOutput {
    const summaryAt = stdout.search(/^\{\s*"passed"(?![\s\S]*^\{\s*"passed")/m);
    const printed = summaryAt >= 0 ? stdout.slice(0, summaryAt) : stdout;
    let file: FileResult | undefined;
    try {
        file = summaryAt >= 0 ? JSON.parse(stdout.slice(summaryAt)).results?.[0] : undefined;
    } catch {
        file = undefined;
    }

    const suites: string[] = [];
    const results: TestResult[] = [];
    const lines = printed.split(/\r?\n/);
    for (let i = 0; i < lines.length; i++) {
        const indent = /^ */.exec(lines[i])![0].length;
        const level = Math.floor(indent / 2);
        const text = lines[i].slice(indent);
        const result = /^(✓|✗) (.*)$/.exec(text);
        if (!result) {
            // Deeper names stay: results only use the names above them, and a
            // test printing a suite's name must not lose the inner suites.
            if (text && level <= suites.length && isSuite([...suites.slice(0, level), text])) {
                suites[level] = text;
            }
            continue;
        }
        const passed = result[1] === "✓";
        const next = lines[i + 1] ?? "";
        const message = !passed && next.startsWith(" ".repeat(indent + 2)) ? next.trim() : undefined;
        results.push({ names: [...suites.slice(0, level), result[2]], passed, message });
    }

    const onlyTestsFailed = file?.status === "failed"
        && file.error_type === "runtime"
        && /^\d+ test\(s\) failed$/.test(file.message ?? "");
    return { printed, results, file, onlyTestsFailed };
}
