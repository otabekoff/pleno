import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';
import * as os from 'os';
import { execFile } from 'child_process';

/** The executables of the Pleno toolchain. */
export type Tool = 'pleno' | 'pleno-lsp' | 'pleno-dap' | 'pleno-repl' | 'plm';

/** Where a tool was found, or `missing`. */
export type ToolSource = 'setting' | 'toolchain' | 'path' | 'home' | 'repository' | 'missing';

export interface ToolLocation {
    /** The executable to run; the bare tool name when it was not found. */
    path: string;
    found: boolean;
    source: ToolSource;
}

/** The setting that overrides each tool's location, if there is one. */
const SETTINGS: Partial<Record<Tool, string>> = {
    'pleno': 'cli.path',
    'pleno-lsp': 'lsp.path',
    'pleno-dap': 'dap.path',
};

const cache = new Map<Tool, ToolLocation>();

/** Forgets found tools, after an install or a settings change. */
export function clearToolCache(): void {
    cache.clear();
}

/** The folder the "Install Pleno Toolchain" command installs into. */
export function toolchainDir(context: vscode.ExtensionContext): string {
    return path.join(context.globalStorageUri.fsPath, 'toolchain');
}

function executableName(tool: string): string {
    return process.platform === 'win32' ? `${tool}.exe` : tool;
}

function isFile(file: string): boolean {
    try {
        return fs.statSync(file).isFile();
    } catch {
        return false;
    }
}

/** Looks `name` up on PATH, as the shell would. */
function findOnPath(name: string): string | undefined {
    const dirs = (process.env.PATH ?? '').split(path.delimiter).filter(Boolean);
    const extensions = process.platform === 'win32'
        ? (process.env.PATHEXT ?? '.EXE;.CMD;.BAT').split(';').filter(Boolean)
        : [''];
    for (const dir of dirs) {
        for (const ext of extensions) {
            const candidate = path.join(dir, name + (name.toLowerCase().endsWith(ext.toLowerCase()) ? '' : ext));
            if (isFile(candidate)) {
                return candidate;
            }
        }
    }
    return undefined;
}

/**
 * Finds a Pleno tool: its setting, then the toolchain installed by this
 * extension, then PATH, then `$PLENO_HOME/bin` or `~/.pleno/bin` (where the install scripts put
 * it, for when VS Code was started before PATH was updated), then a build in
 * the Pleno repository (when the extension runs from `ide/vscode`).
 */
export function findTool(context: vscode.ExtensionContext, tool: Tool): ToolLocation {
    const cached = cache.get(tool);
    if (cached) {
        return cached;
    }
    const location = locate(context, tool);
    cache.set(tool, location);
    return location;
}

function locate(context: vscode.ExtensionContext, tool: Tool): ToolLocation {
    const setting = SETTINGS[tool];
    const configured = setting
        ? vscode.workspace.getConfiguration('pleno').get<string>(setting)?.trim()
        : undefined;
    // A bare name such as the default `pleno-lsp` means "look it up".
    if (configured && configured !== tool) {
        if (configured.includes('/') || configured.includes('\\')) {
            return { path: configured, found: isFile(configured), source: 'setting' };
        }
        const onPath = findOnPath(configured);
        return { path: onPath ?? configured, found: onPath !== undefined, source: 'setting' };
    }

    const installed = path.join(toolchainDir(context), executableName(tool));
    if (isFile(installed)) {
        return { path: installed, found: true, source: 'toolchain' };
    }

    const onPath = findOnPath(tool);
    if (onPath) {
        return { path: onPath, found: true, source: 'path' };
    }

    // The install scripts use PLENO_HOME when it is set.
    const plenoHome = process.env.PLENO_HOME?.trim() || path.join(os.homedir(), '.pleno');
    const home = path.join(plenoHome, 'bin', executableName(tool));
    if (isFile(home)) {
        return { path: home, found: true, source: 'home' };
    }

    for (const profile of ['release', 'debug']) {
        const built = context.asAbsolutePath(
            path.join('..', '..', 'target', profile, executableName(tool)),
        );
        if (isFile(built)) {
            return { path: built, found: true, source: 'repository' };
        }
    }

    return { path: tool, found: false, source: 'missing' };
}

export function getPlenoPath(context: vscode.ExtensionContext): string {
    return findTool(context, 'pleno').path;
}

/** The `pleno-dap` debug adapter, or undefined when it is not installed. */
export function getPlenoDapPath(context: vscode.ExtensionContext): string | undefined {
    const location = findTool(context, 'pleno-dap');
    return location.found ? location.path : undefined;
}

/**
 * The version a tool reports with `--version`, such as `1.2.3`, or undefined
 * when it cannot be run. `pleno-dap` and `pleno-repl` have no such flag.
 */
export function toolVersion(location: ToolLocation): Promise<string | undefined> {
    if (!location.found) {
        return Promise.resolve(undefined);
    }
    return new Promise((resolve) => {
        execFile(location.path, ['--version'], { timeout: 5000, env: { ...process.env, NO_COLOR: '1' } }, (error, stdout) => {
            // `pleno 1.2.3` -> `1.2.3`; color codes, if any, have no dotted numbers.
            const match = /\d+\.\d+\.\d+[\w.+-]*/.exec(stdout);
            resolve(error || !match ? undefined : match[0]);
        });
    });
}
