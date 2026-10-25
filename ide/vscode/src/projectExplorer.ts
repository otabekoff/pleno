import * as vscode from "vscode";
import * as fs from "fs";
import { parse } from "smol-toml";

/** A dependency as written in `pleno.toml`: a version or a table. */
type Dependency = string | {
    version?: string;
    git?: string;
    path?: string;
    branch?: string;
    tag?: string;
    rev?: string;
};

/** The parts of `pleno.toml` the view shows (see `crates/pleno-package`). */
export interface Project {
    folder: vscode.WorkspaceFolder;
    manifest: vscode.Uri;
    name?: string;
    version?: string;
    description?: string;
    main?: string;
    scripts: Record<string, string>;
    dependencies: Record<string, Dependency>;
    devDependencies: Record<string, Dependency>;
    /** Why the manifest could not be read, if it could not. */
    error?: string;
    /** The manifest text, to find the line a script is on. */
    text: string;
}

function table<T>(value: unknown): Record<string, T> {
    return value && typeof value === "object" && !Array.isArray(value)
        ? value as Record<string, T>
        : {};
}

function readProject(folder: vscode.WorkspaceFolder): Project | undefined {
    const manifest = vscode.Uri.joinPath(folder.uri, "pleno.toml");
    let text: string;
    try {
        text = fs.readFileSync(manifest.fsPath, "utf-8");
    } catch {
        return undefined;
    }
    const project: Project = {
        folder,
        manifest,
        scripts: {},
        dependencies: {},
        devDependencies: {},
        text,
    };
    try {
        const data = parse(text) as Record<string, unknown>;
        const pkg = table<unknown>(data.package);
        project.name = typeof pkg.name === "string" ? pkg.name : undefined;
        project.version = typeof pkg.version === "string" ? pkg.version : undefined;
        project.description = typeof pkg.description === "string" ? pkg.description : undefined;
        // `plm` uses src/main.pln when `main` is not set.
        project.main = typeof pkg.main === "string" ? pkg.main : "src/main.pln";
        project.scripts = table<string>(data.scripts);
        project.dependencies = table<Dependency>(data.dependencies);
        project.devDependencies = table<Dependency>(data["dev-dependencies"]);
    } catch (error) {
        project.error = error instanceof Error ? error.message.split("\n")[0] : String(error);
    }
    return project;
}

function describeDependency(dependency: Dependency): string {
    if (typeof dependency === "string") {
        return dependency;
    }
    if (dependency.git) {
        const at = dependency.tag ?? dependency.branch ?? dependency.rev;
        return at ? `${dependency.git} @ ${at}` : dependency.git;
    }
    return dependency.path ?? dependency.version ?? "";
}

/** Opens `pleno.toml` with the cursor on `line` (0-based). */
function openManifestAt(project: Project, line = 0): vscode.Command {
    return {
        command: "vscode.open",
        title: "Open pleno.toml",
        arguments: [project.manifest, { selection: new vscode.Range(line, 0, line, 0) }],
    };
}

export class ProjectItem extends vscode.TreeItem {
    children?: ProjectItem[];
    project?: Project;
    script?: string;
}

function item(
    label: string,
    icon: string,
    options: Partial<ProjectItem> = {},
): ProjectItem {
    const node = new ProjectItem(label, options.children
        ? vscode.TreeItemCollapsibleState.Expanded
        : vscode.TreeItemCollapsibleState.None);
    node.iconPath = new vscode.ThemeIcon(icon);
    Object.assign(node, options);
    return node;
}

/**
 * The "Pleno Project" view in the Explorer: the package, entry file, scripts
 * and dependencies from each workspace folder's `pleno.toml`. It updates when
 * a manifest changes.
 */
export class PlenoProjectProvider implements vscode.TreeDataProvider<ProjectItem>, vscode.Disposable {
    private readonly changed = new vscode.EventEmitter<void>();
    readonly onDidChangeTreeData = this.changed.event;
    private projects: Project[] = [];
    private readonly disposables: vscode.Disposable[] = [];

    constructor() {
        const watcher = vscode.workspace.createFileSystemWatcher("**/pleno.toml");
        this.disposables.push(
            watcher,
            watcher.onDidCreate(() => this.refresh()),
            watcher.onDidChange(() => this.refresh()),
            watcher.onDidDelete(() => this.refresh()),
            vscode.workspace.onDidChangeWorkspaceFolders(() => this.refresh()),
            this.changed,
        );
        this.refresh();
    }

    /** The workspace folders that have a `pleno.toml`. */
    getProjects(): Project[] {
        return this.projects;
    }

    refresh(): void {
        this.projects = (vscode.workspace.workspaceFolders ?? [])
            .map(readProject)
            .filter((project): project is Project => project !== undefined);
        void vscode.commands.executeCommand("setContext", "pleno.hasProject", this.projects.length > 0);
        this.changed.fire();
    }

    getTreeItem(element: ProjectItem): vscode.TreeItem {
        return element;
    }

    getChildren(element?: ProjectItem): ProjectItem[] {
        if (element) {
            return element.children ?? [];
        }
        if (this.projects.length === 1) {
            return this.projectItems(this.projects[0]);
        }
        return this.projects.map((project) => item(project.folder.name, "root-folder", {
            children: this.projectItems(project),
            description: project.name,
        }));
    }

    private projectItems(project: Project): ProjectItem[] {
        if (project.error) {
            return [item("pleno.toml has an error", "error", {
                description: project.error,
                tooltip: project.error,
                command: openManifestAt(project),
            })];
        }

        const items: ProjectItem[] = [item(project.name ?? project.folder.name, "package", {
            description: project.version,
            tooltip: project.description ?? project.manifest.fsPath,
            command: openManifestAt(project),
            contextValue: "package",
        })];

        if (project.main) {
            const main = vscode.Uri.joinPath(project.folder.uri, project.main);
            items.push(item(project.main, "file-code", {
                description: "entry",
                resourceUri: main,
                command: { command: "vscode.open", title: "Open", arguments: [main] },
                contextValue: "main",
            }));
        }

        // The line of `name = ...` in the `[section]` table, for "go to".
        const lines = project.text.split(/\r?\n/);
        const lineOf = (section: string, name: string) => {
            const start = lines.findIndex((line) => line.trim() === `[${section}]`);
            const found = lines.findIndex((line, index) => {
                const key = /^\s*"?([^"=\s]+)"?\s*=/.exec(line);
                return index > start && key?.[1] === name;
            });
            return Math.max(found, start, 0);
        };

        const scripts = Object.entries(project.scripts).map(([name, command]) =>
            item(name, "terminal", {
                description: command,
                tooltip: `plm run ${name}: ${command}`,
                command: openManifestAt(project, lineOf("scripts", name)),
                contextValue: "script",
                project,
                script: name,
            }));
        items.push(item("Scripts", "list-unordered", {
            children: scripts.length ? scripts : [item("No scripts in pleno.toml", "info")],
        }));

        const dependencies = (section: string, entries: Record<string, Dependency>) =>
            Object.entries(entries).map(([name, dependency]) => item(name, "library", {
                description: describeDependency(dependency),
                command: openManifestAt(project, lineOf(section, name)),
                contextValue: "dependency",
            }));
        const deps = dependencies("dependencies", project.dependencies);
        items.push(item("Dependencies", "references", {
            children: deps.length ? deps : [item("No dependencies", "info")],
            collapsibleState: deps.length
                ? vscode.TreeItemCollapsibleState.Expanded
                : vscode.TreeItemCollapsibleState.Collapsed,
        }));
        const devDeps = dependencies("dev-dependencies", project.devDependencies);
        if (devDeps.length) {
            items.push(item("Dev dependencies", "references", { children: devDeps }));
        }
        return items;
    }

    dispose(): void {
        this.disposables.forEach((disposable) => disposable.dispose());
    }
}
