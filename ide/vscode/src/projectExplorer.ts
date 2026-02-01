import * as vscode from 'vscode';
import { execSync } from 'child_process';
import { getPlenoPath } from './utils';

export class PlenoProjectProvider implements vscode.TreeDataProvider<ProjectItem> {
    private _onDidChangeTreeData: vscode.EventEmitter<ProjectItem | undefined | void> = new vscode.EventEmitter<ProjectItem | undefined | void>();
    readonly onDidChangeTreeData: vscode.Event<ProjectItem | undefined | void> = this._onDidChangeTreeData.event;

    constructor(
        private readonly context: vscode.ExtensionContext,
        private workspaceRoot: string | undefined
    ) { }

    refresh(): void {
        this._onDidChangeTreeData.fire();
    }

    getTreeItem(element: ProjectItem): vscode.TreeItem {
        return element;
    }

    getChildren(element?: ProjectItem): vscode.ProviderResult<ProjectItem[]> {
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

    private getProjectInfo(): any {
        try {
            const pleno = getPlenoPath(this.context);
            const output = execSync(`${pleno} plm info --json`, { cwd: this.workspaceRoot });
            return JSON.parse(output.toString());
        } catch {
            return null;
        }
    }
}

class ProjectItem extends vscode.TreeItem {
    constructor(
        public readonly label: string,
        public readonly collapsibleState: vscode.TreeItemCollapsibleState,
        public readonly contextValue: string
    ) {
        super(label, collapsibleState);
    }
}
