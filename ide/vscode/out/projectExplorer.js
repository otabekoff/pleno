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
exports.PlenoProjectProvider = void 0;
const vscode = __importStar(require("vscode"));
const child_process_1 = require("child_process");
const utils_1 = require("./utils");
class PlenoProjectProvider {
    context;
    workspaceRoot;
    _onDidChangeTreeData = new vscode.EventEmitter();
    onDidChangeTreeData = this._onDidChangeTreeData.event;
    constructor(context, workspaceRoot) {
        this.context = context;
        this.workspaceRoot = workspaceRoot;
    }
    refresh() {
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
            // Root items: Dependencies, Scripts
            return [
                new ProjectItem('Dependencies', vscode.TreeItemCollapsibleState.Collapsed, 'dependencies'),
                new ProjectItem('Scripts', vscode.TreeItemCollapsibleState.Collapsed, 'scripts')
            ];
        }
        const projectInfo = this.getProjectInfo();
        if (!projectInfo) {
            return [];
        }
        if (element.contextValue === 'dependencies') {
            const deps = projectInfo.dependencies || {};
            const devDeps = projectInfo['dev-dependencies'] || {};
            const items = [
                ...Object.keys(deps).map(name => {
                    const item = new ProjectItem(`${name}`, vscode.TreeItemCollapsibleState.None, 'dependency');
                    item.description = typeof deps[name] === 'string' ? deps[name] : JSON.stringify(deps[name]);
                    item.iconPath = new vscode.ThemeIcon('package');
                    return item;
                }),
                ...Object.keys(devDeps).map(name => {
                    const item = new ProjectItem(`${name} (dev)`, vscode.TreeItemCollapsibleState.None, 'dependency');
                    item.description = typeof devDeps[name] === 'string' ? devDeps[name] : JSON.stringify(devDeps[name]);
                    item.iconPath = new vscode.ThemeIcon('package');
                    return item;
                })
            ];
            return items;
        }
        if (element.contextValue === 'scripts') {
            const scripts = projectInfo.scripts || {};
            return Object.keys(scripts).map(name => {
                const item = new ProjectItem(name, vscode.TreeItemCollapsibleState.None, 'script');
                item.description = scripts[name];
                item.iconPath = new vscode.ThemeIcon('play');
                item.command = {
                    command: 'pleno.runScript',
                    title: 'Run Script',
                    arguments: [name]
                };
                return item;
            });
        }
        return [];
    }
    getProjectInfo() {
        try {
            const pleno = (0, utils_1.getPlenoPath)(this.context);
            const output = (0, child_process_1.execSync)(`${pleno} plm info --json`, { cwd: this.workspaceRoot });
            return JSON.parse(output.toString());
        }
        catch {
            return null;
        }
    }
}
exports.PlenoProjectProvider = PlenoProjectProvider;
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
//# sourceMappingURL=projectExplorer.js.map