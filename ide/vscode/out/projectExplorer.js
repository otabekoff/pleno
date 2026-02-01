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
exports.PlenoOutlineProvider = exports.PlenoProjectProvider = void 0;
const vscode = __importStar(require("vscode"));
const fs = __importStar(require("fs"));
const path = __importStar(require("path"));
const child_process_1 = require("child_process");
const utils_1 = require("./utils");
/**
 * Pleno Project Provider
 * Provides tree view data for the Pleno Project Overview sidebar
 */
class PlenoProjectProvider {
    context;
    workspaceRoot;
    _onDidChangeTreeData = new vscode.EventEmitter();
    onDidChangeTreeData = this._onDidChangeTreeData.event;
    projectInfo = null;
    hasProject = false;
    constructor(context, workspaceRoot) {
        this.context = context;
        this.workspaceRoot = workspaceRoot;
        this.detectProject();
    }
    /**
     * Detect if a Pleno project exists in the workspace
     */
    detectProject() {
        if (!this.workspaceRoot) {
            this.hasProject = false;
            return;
        }
        const manifestPath = path.join(this.workspaceRoot, "pleno.toml");
        this.hasProject = fs.existsSync(manifestPath);
        // Set context for welcome view conditions
        vscode.commands.executeCommand("setContext", "pleno.hasProject", this.hasProject);
    }
    /**
     * Refresh the tree view
     */
    refresh() {
        this.projectInfo = null; // Clear cache
        this.detectProject();
        this._onDidChangeTreeData.fire();
    }
    getTreeItem(element) {
        return element;
    }
    getChildren(element) {
        if (!this.workspaceRoot) {
            return [];
        }
        if (!element) {
            return this.getRootItems();
        }
        return this.getChildItems(element);
    }
    /**
     * Get root level items
     */
    getRootItems() {
        const items = [];
        // Project Info section
        const projectInfo = this.getProjectInfo();
        if (projectInfo) {
            // Project name and version
            const nameItem = new ProjectItem(projectInfo.name || "Pleno Project", vscode.TreeItemCollapsibleState.None, "projectName");
            nameItem.description = projectInfo.version || "";
            nameItem.iconPath = new vscode.ThemeIcon("project");
            nameItem.tooltip = `${projectInfo.name || "Project"} v${projectInfo.version || "0.0.0"}`;
            items.push(nameItem);
        }
        // Quick Actions section
        const actionsItem = new ProjectItem("Quick Actions", vscode.TreeItemCollapsibleState.Expanded, "quickActions");
        actionsItem.iconPath = new vscode.ThemeIcon("zap");
        items.push(actionsItem);
        // Source Files section
        const sourceItem = new ProjectItem("Source Files", vscode.TreeItemCollapsibleState.Collapsed, "sourceFiles");
        sourceItem.iconPath = new vscode.ThemeIcon("file-code");
        items.push(sourceItem);
        // Dependencies section
        const depsItem = new ProjectItem("Dependencies", vscode.TreeItemCollapsibleState.Collapsed, "dependencies");
        depsItem.iconPath = new vscode.ThemeIcon("package");
        items.push(depsItem);
        // Scripts section
        const scriptsItem = new ProjectItem("Scripts", vscode.TreeItemCollapsibleState.Collapsed, "scripts");
        scriptsItem.iconPath = new vscode.ThemeIcon("terminal");
        items.push(scriptsItem);
        // Build Targets section
        const targetsItem = new ProjectItem("Build Targets", vscode.TreeItemCollapsibleState.Collapsed, "buildTargets");
        targetsItem.iconPath = new vscode.ThemeIcon("tools");
        items.push(targetsItem);
        return items;
    }
    /**
     * Get children for a specific parent item
     */
    getChildItems(element) {
        const projectInfo = this.getProjectInfo();
        switch (element.contextValue) {
            case "quickActions":
                return this.getQuickActionItems();
            case "sourceFiles":
                return this.getSourceFileItems();
            case "dependencies":
                return this.getDependencyItems(projectInfo);
            case "scripts":
                return this.getScriptItems(projectInfo);
            case "buildTargets":
                return this.getBuildTargetItems(projectInfo);
            default:
                return [];
        }
    }
    /**
     * Get quick action items
     */
    getQuickActionItems() {
        const items = [];
        // Run
        const runItem = new ProjectItem("Run Project", vscode.TreeItemCollapsibleState.None, "action");
        runItem.iconPath = new vscode.ThemeIcon("play");
        runItem.command = { command: "pleno.runFile", title: "Run" };
        runItem.tooltip = "Run the current file or project";
        items.push(runItem);
        // Build
        const buildItem = new ProjectItem("Build Project", vscode.TreeItemCollapsibleState.None, "action");
        buildItem.iconPath = new vscode.ThemeIcon("tools");
        buildItem.command = {
            command: "workbench.action.tasks.runTask",
            title: "Build",
            arguments: ["plm: plm build"],
        };
        buildItem.tooltip = "Build the project";
        items.push(buildItem);
        // Test
        const testItem = new ProjectItem("Run Tests", vscode.TreeItemCollapsibleState.None, "action");
        testItem.iconPath = new vscode.ThemeIcon("beaker");
        testItem.command = {
            command: "workbench.action.tasks.runTask",
            title: "Test",
            arguments: ["plm: plm run test"],
        };
        testItem.tooltip =
            "Run test script (requires 'test' script in pleno.toml)";
        items.push(testItem);
        // Check
        const checkItem = new ProjectItem("Check Code", vscode.TreeItemCollapsibleState.None, "action");
        checkItem.iconPath = new vscode.ThemeIcon("check");
        checkItem.command = { command: "pleno.checkFile", title: "Check" };
        checkItem.tooltip = "Check the current file for errors";
        items.push(checkItem);
        // Open REPL
        const replItem = new ProjectItem("Open REPL", vscode.TreeItemCollapsibleState.None, "action");
        replItem.iconPath = new vscode.ThemeIcon("terminal");
        replItem.command = { command: "pleno.openRepl", title: "REPL" };
        replItem.tooltip = "Open the Pleno REPL";
        items.push(replItem);
        // Dashboard
        const dashboardItem = new ProjectItem("Open Dashboard", vscode.TreeItemCollapsibleState.None, "action");
        dashboardItem.iconPath = new vscode.ThemeIcon("dashboard");
        dashboardItem.command = {
            command: "pleno.showDashboard",
            title: "Dashboard",
        };
        dashboardItem.tooltip = "Open the Pleno Dashboard";
        items.push(dashboardItem);
        return items;
    }
    /**
     * Get source file items
     */
    getSourceFileItems() {
        if (!this.workspaceRoot) {
            return [];
        }
        const items = [];
        const srcDir = path.join(this.workspaceRoot, "src");
        // Check for main.pln or src directory
        const mainFile = path.join(this.workspaceRoot, "main.pln");
        if (fs.existsSync(mainFile)) {
            const item = new ProjectItem("main.pln", vscode.TreeItemCollapsibleState.None, "sourceFile");
            item.iconPath = new vscode.ThemeIcon("file");
            item.resourceUri = vscode.Uri.file(mainFile);
            item.command = {
                command: "vscode.open",
                title: "Open",
                arguments: [vscode.Uri.file(mainFile)],
            };
            items.push(item);
        }
        // List .pln files in workspace root
        try {
            const files = fs.readdirSync(this.workspaceRoot);
            for (const file of files) {
                if (file.endsWith(".pln") && file !== "main.pln") {
                    const filePath = path.join(this.workspaceRoot, file);
                    const item = new ProjectItem(file, vscode.TreeItemCollapsibleState.None, "sourceFile");
                    item.iconPath = new vscode.ThemeIcon("file");
                    item.resourceUri = vscode.Uri.file(filePath);
                    item.command = {
                        command: "vscode.open",
                        title: "Open",
                        arguments: [vscode.Uri.file(filePath)],
                    };
                    items.push(item);
                }
            }
        }
        catch {
            // Ignore errors
        }
        // List files in src directory if it exists
        if (fs.existsSync(srcDir)) {
            try {
                const srcFiles = this.findPlenoFiles(srcDir, "src");
                items.push(...srcFiles);
            }
            catch {
                // Ignore errors
            }
        }
        if (items.length === 0) {
            const noFilesItem = new ProjectItem("No source files found", vscode.TreeItemCollapsibleState.None, "info");
            noFilesItem.iconPath = new vscode.ThemeIcon("info");
            items.push(noFilesItem);
        }
        return items;
    }
    /**
     * Find all .pln files in a directory
     */
    findPlenoFiles(dir, prefix) {
        const items = [];
        try {
            const entries = fs.readdirSync(dir, { withFileTypes: true });
            for (const entry of entries) {
                const fullPath = path.join(dir, entry.name);
                const displayName = `${prefix}/${entry.name}`;
                if (entry.isFile() && entry.name.endsWith(".pln")) {
                    const item = new ProjectItem(displayName, vscode.TreeItemCollapsibleState.None, "sourceFile");
                    item.iconPath = new vscode.ThemeIcon("file");
                    item.resourceUri = vscode.Uri.file(fullPath);
                    item.command = {
                        command: "vscode.open",
                        title: "Open",
                        arguments: [vscode.Uri.file(fullPath)],
                    };
                    items.push(item);
                }
                else if (entry.isDirectory() && !entry.name.startsWith(".")) {
                    items.push(...this.findPlenoFiles(fullPath, displayName));
                }
            }
        }
        catch {
            // Ignore errors
        }
        return items;
    }
    /**
     * Get dependency items
     */
    getDependencyItems(projectInfo) {
        const items = [];
        if (!projectInfo) {
            const noInfoItem = new ProjectItem("No project manifest found", vscode.TreeItemCollapsibleState.None, "info");
            noInfoItem.iconPath = new vscode.ThemeIcon("info");
            return [noInfoItem];
        }
        const deps = projectInfo.dependencies || {};
        const devDeps = projectInfo["dev-dependencies"] || {};
        // Regular dependencies
        for (const [name, version] of Object.entries(deps)) {
            const item = new ProjectItem(name, vscode.TreeItemCollapsibleState.None, "dependency");
            item.description = typeof version === "string"
                ? version
                : JSON.stringify(version);
            item.iconPath = new vscode.ThemeIcon("package");
            item.tooltip = `${name}: ${item.description}`;
            items.push(item);
        }
        // Dev dependencies
        for (const [name, version] of Object.entries(devDeps)) {
            const item = new ProjectItem(`${name} (dev)`, vscode.TreeItemCollapsibleState.None, "dependency");
            item.description = typeof version === "string"
                ? version
                : JSON.stringify(version);
            item.iconPath = new vscode.ThemeIcon("package");
            item.tooltip = `${name}: ${item.description} (dev dependency)`;
            items.push(item);
        }
        if (items.length === 0) {
            const noDepItem = new ProjectItem("No dependencies", vscode.TreeItemCollapsibleState.None, "info");
            noDepItem.iconPath = new vscode.ThemeIcon("info");
            items.push(noDepItem);
        }
        return items;
    }
    /**
     * Get script items
     */
    getScriptItems(projectInfo) {
        const items = [];
        if (!projectInfo) {
            return items;
        }
        const scripts = projectInfo.scripts || {};
        for (const [name, command] of Object.entries(scripts)) {
            const item = new ProjectItem(name, vscode.TreeItemCollapsibleState.None, "script");
            item.description = typeof command === "string" ? command : "";
            item.iconPath = new vscode.ThemeIcon("play");
            item.tooltip = `Run: ${command}`;
            item.command = {
                command: "pleno.runScript",
                title: "Run Script",
                arguments: [name],
            };
            items.push(item);
        }
        if (items.length === 0) {
            const noScriptsItem = new ProjectItem("No scripts defined", vscode.TreeItemCollapsibleState.None, "info");
            noScriptsItem.iconPath = new vscode.ThemeIcon("info");
            items.push(noScriptsItem);
        }
        return items;
    }
    /**
     * Get build target items
     */
    getBuildTargetItems(_projectInfo) {
        const items = [];
        // Default targets with their task names
        const targets = [
            { name: "debug", task: "plm: plm build --debug", icon: "bug" },
            {
                name: "release",
                task: "plm: plm build --release",
                icon: "package",
            },
            { name: "test", task: "plm: plm build --test", icon: "beaker" },
        ];
        for (const target of targets) {
            const item = new ProjectItem(target.name, vscode.TreeItemCollapsibleState.None, "buildTarget");
            item.iconPath = new vscode.ThemeIcon(target.icon);
            item.tooltip = `Build with --${target.name} flag`;
            item.command = {
                command: "workbench.action.tasks.runTask",
                title: "Build",
                arguments: [target.task],
            };
            items.push(item);
        }
        return items;
    }
    /**
     * Get project info from pleno.toml or plm info command
     */
    getProjectInfo() {
        if (this.projectInfo) {
            return this.projectInfo;
        }
        if (!this.workspaceRoot) {
            return null;
        }
        // Try to read pleno.toml directly first
        try {
            const manifestPath = path.join(this.workspaceRoot, "pleno.toml");
            if (fs.existsSync(manifestPath)) {
                const content = fs.readFileSync(manifestPath, "utf-8");
                // Simple TOML parsing for common fields
                const projectInfo = {};
                // Parse [package] section
                const packageMatch = content.match(/\[package\]([\s\S]*?)(?=\[|$)/);
                if (packageMatch) {
                    const packageSection = packageMatch[1];
                    const nameMatch = packageSection.match(/name\s*=\s*"([^"]+)"/);
                    const versionMatch = packageSection.match(/version\s*=\s*"([^"]+)"/);
                    if (nameMatch)
                        projectInfo.name = nameMatch[1];
                    if (versionMatch)
                        projectInfo.version = versionMatch[1];
                }
                // Parse [dependencies] section
                const depsMatch = content.match(/\[dependencies\]([\s\S]*?)(?=\[|$)/);
                if (depsMatch) {
                    projectInfo.dependencies = this.parseTOMLSection(depsMatch[1]);
                }
                // Parse [dev-dependencies] section
                const devDepsMatch = content.match(/\[dev-dependencies\]([\s\S]*?)(?=\[|$)/);
                if (devDepsMatch) {
                    projectInfo["dev-dependencies"] = this.parseTOMLSection(devDepsMatch[1]);
                }
                // Parse [scripts] section
                const scriptsMatch = content.match(/\[scripts\]([\s\S]*?)(?=\[|$)/);
                if (scriptsMatch) {
                    projectInfo.scripts = this.parseTOMLSection(scriptsMatch[1]);
                }
                this.projectInfo = projectInfo;
                return projectInfo;
            }
        }
        catch {
            // Fall through to try plm info
        }
        // Try to get info from plm command
        try {
            const pleno = (0, utils_1.getPlenoPath)(this.context);
            const output = (0, child_process_1.execSync)(`"${pleno}" plm info --json`, {
                cwd: this.workspaceRoot,
                timeout: 5000,
                encoding: "utf-8",
            });
            this.projectInfo = JSON.parse(output);
            return this.projectInfo;
        }
        catch {
            return null;
        }
    }
    /**
     * Parse a simple TOML section (key = "value" pairs)
     */
    parseTOMLSection(section) {
        const result = {};
        const lines = section.split("\n");
        for (const line of lines) {
            const match = line.match(/^\s*(\w+)\s*=\s*"([^"]+)"/);
            if (match) {
                result[match[1]] = match[2];
            }
        }
        return result;
    }
}
exports.PlenoProjectProvider = PlenoProjectProvider;
/**
 * Project Item for tree view
 */
class ProjectItem extends vscode.TreeItem {
    label;
    collapsibleState;
    contextValue;
    constructor(label, collapsibleState, contextValue) {
        super(label, collapsibleState);
        this.label = label;
        this.collapsibleState = collapsibleState;
        this.contextValue = contextValue;
    }
}
/**
 * Pleno Outline Provider
 * Provides document symbols/outline for Pleno files
 */
class PlenoOutlineProvider {
    _onDidChangeTreeData = new vscode.EventEmitter();
    onDidChangeTreeData = this._onDidChangeTreeData.event;
    document;
    constructor() {
        // Update outline when active editor changes
        vscode.window.onDidChangeActiveTextEditor((editor) => {
            if (editor && editor.document.languageId === "pleno") {
                this.document = editor.document;
                this._onDidChangeTreeData.fire();
            }
        });
        // Update outline when document changes
        vscode.workspace.onDidChangeTextDocument((event) => {
            if (event.document === this.document) {
                this._onDidChangeTreeData.fire();
            }
        });
        // Initialize with current editor
        if (vscode.window.activeTextEditor?.document.languageId === "pleno") {
            this.document = vscode.window.activeTextEditor.document;
        }
    }
    refresh() {
        this._onDidChangeTreeData.fire();
    }
    getTreeItem(element) {
        return element;
    }
    getChildren(_element) {
        if (!this.document) {
            return [];
        }
        return this.parseDocument(this.document);
    }
    /**
     * Parse document for symbols
     */
    parseDocument(document) {
        const items = [];
        const text = document.getText();
        const lines = text.split("\n");
        for (let i = 0; i < lines.length; i++) {
            const line = lines[i];
            // Function declarations: fn name(...) or func name(...)
            let match = line.match(/^\s*(pub\s+)?(fn|func)\s+(\w+)\s*\(/);
            if (match) {
                const item = new OutlineItem(match[3], vscode.TreeItemCollapsibleState.None, "function");
                item.iconPath = new vscode.ThemeIcon("symbol-function");
                item.description = match[1] ? "pub" : "";
                item.command = {
                    command: "revealLine",
                    title: "Go to",
                    arguments: [{ lineNumber: i, at: "center" }],
                };
                items.push(item);
                continue;
            }
            // Struct declarations: struct Name { or type Name struct {
            match = line.match(/^\s*(pub\s+)?(struct|type)\s+(\w+)/);
            if (match) {
                const item = new OutlineItem(match[3], vscode.TreeItemCollapsibleState.None, "struct");
                item.iconPath = new vscode.ThemeIcon("symbol-struct");
                item.description = match[1] ? "pub" : "";
                item.command = {
                    command: "revealLine",
                    title: "Go to",
                    arguments: [{ lineNumber: i, at: "center" }],
                };
                items.push(item);
                continue;
            }
            // Enum declarations: enum Name {
            match = line.match(/^\s*(pub\s+)?enum\s+(\w+)/);
            if (match) {
                const item = new OutlineItem(match[2], vscode.TreeItemCollapsibleState.None, "enum");
                item.iconPath = new vscode.ThemeIcon("symbol-enum");
                item.description = match[1] ? "pub" : "";
                item.command = {
                    command: "revealLine",
                    title: "Go to",
                    arguments: [{ lineNumber: i, at: "center" }],
                };
                items.push(item);
                continue;
            }
            // Trait/Interface declarations: trait Name { or interface Name {
            match = line.match(/^\s*(pub\s+)?(trait|interface)\s+(\w+)/);
            if (match) {
                const item = new OutlineItem(match[3], vscode.TreeItemCollapsibleState.None, "trait");
                item.iconPath = new vscode.ThemeIcon("symbol-interface");
                item.description = match[1] ? "pub" : "";
                item.command = {
                    command: "revealLine",
                    title: "Go to",
                    arguments: [{ lineNumber: i, at: "center" }],
                };
                items.push(item);
                continue;
            }
            // Constant declarations: const NAME = or let NAME =
            match = line.match(/^\s*(pub\s+)?(const|let)\s+(\w+)\s*[:=]/);
            if (match && match[3] === match[3].toUpperCase()) {
                const item = new OutlineItem(match[3], vscode.TreeItemCollapsibleState.None, "constant");
                item.iconPath = new vscode.ThemeIcon("symbol-constant");
                item.description = match[1] ? "pub" : "";
                item.command = {
                    command: "revealLine",
                    title: "Go to",
                    arguments: [{ lineNumber: i, at: "center" }],
                };
                items.push(item);
                continue;
            }
            // Import statements: import "module" or use module
            match = line.match(/^\s*(import|use)\s+["']?([^"'\s]+)/);
            if (match) {
                const item = new OutlineItem(match[2], vscode.TreeItemCollapsibleState.None, "import");
                item.iconPath = new vscode.ThemeIcon("symbol-namespace");
                item.command = {
                    command: "revealLine",
                    title: "Go to",
                    arguments: [{ lineNumber: i, at: "center" }],
                };
                items.push(item);
            }
        }
        return items;
    }
}
exports.PlenoOutlineProvider = PlenoOutlineProvider;
/**
 * Outline Item for tree view
 */
class OutlineItem extends vscode.TreeItem {
    label;
    collapsibleState;
    contextValue;
    constructor(label, collapsibleState, contextValue) {
        super(label, collapsibleState);
        this.label = label;
        this.collapsibleState = collapsibleState;
        this.contextValue = contextValue;
    }
}
//# sourceMappingURL=projectExplorer.js.map