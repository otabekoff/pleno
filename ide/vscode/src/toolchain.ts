import * as vscode from 'vscode';
import * as crypto from 'crypto';
import * as fs from 'fs';
import * as https from 'https';
import * as os from 'os';
import * as path from 'path';
import { execFile } from 'child_process';
import { clearToolCache, toolchainDir } from './utils';

const REPOSITORY = 'otabekoff/pleno';
const VERSION_KEY = 'pleno.toolchainVersion';

interface ReleaseAsset {
    name: string;
    browser_download_url: string;
    size: number;
}

interface Release {
    tag_name: string;
    assets: ReleaseAsset[];
}

/** The release target for this machine, as named in the release archives. */
function releaseTarget(): string | undefined {
    const targets: Record<string, string> = {
        'win32-x64': 'x86_64-pc-windows-msvc',
        'darwin-x64': 'x86_64-apple-darwin',
        'darwin-arm64': 'aarch64-apple-darwin',
        'linux-x64': 'x86_64-unknown-linux-gnu',
        'linux-arm64': 'aarch64-unknown-linux-gnu',
        'linux-ia32': 'i686-unknown-linux-gnu',
    };
    return targets[`${process.platform}-${process.arch}`];
}

/** GETs `url`, following redirects, and reports the downloaded fraction. */
function download(url: string, onProgress?: (fraction: number) => void, redirects = 5): Promise<Buffer> {
    return new Promise((resolve, reject) => {
        const request = https.get(url, {
            headers: { 'User-Agent': 'pleno-vscode', Accept: 'application/octet-stream, application/json' },
        }, (response) => {
            const status = response.statusCode ?? 0;
            if (status >= 300 && status < 400 && response.headers.location) {
                response.resume();
                if (redirects === 0) {
                    reject(new Error(`too many redirects for ${url}`));
                    return;
                }
                resolve(download(new URL(response.headers.location, url).toString(), onProgress, redirects - 1));
                return;
            }
            if (status !== 200) {
                response.resume();
                reject(new Error(`${url} returned HTTP ${status}`));
                return;
            }
            const total = Number(response.headers['content-length'] ?? 0);
            const chunks: Buffer[] = [];
            let received = 0;
            response.on('data', (chunk: Buffer) => {
                chunks.push(chunk);
                received += chunk.length;
                if (total > 0 && onProgress) {
                    onProgress(received / total);
                }
            });
            response.on('end', () => resolve(Buffer.concat(chunks)));
            response.on('error', reject);
        });
        request.on('error', reject);
        request.setTimeout(60_000, () => request.destroy(new Error(`timed out downloading ${url}`)));
    });
}

function run(command: string, args: string[]): Promise<void> {
    return new Promise((resolve, reject) => {
        execFile(command, args, (error, _stdout, stderr) => {
            if (error) {
                reject(new Error(`${command} failed: ${stderr || error.message}`));
            } else {
                resolve();
            }
        });
    });
}

/** Unpacks a `.zip` or `.tar.gz` archive with the system `tar`. */
async function extract(archive: string, into: string): Promise<void> {
    try {
        // bsdtar ships with Windows 10 and later and also reads zip files.
        await run('tar', ['-xf', archive, '-C', into]);
    } catch (error) {
        if (process.platform !== 'win32') {
            throw error;
        }
        await run('powershell.exe', [
            '-NoProfile', '-NonInteractive', '-Command',
            `Expand-Archive -LiteralPath '${archive.replace(/'/g, "''")}' -DestinationPath '${into.replace(/'/g, "''")}' -Force`,
        ]);
    }
}

/** The installed toolchain version, if this extension installed one. */
export function installedVersion(context: vscode.ExtensionContext): string | undefined {
    const version = context.globalState.get<string>(VERSION_KEY);
    return version && fs.existsSync(toolchainDir(context)) ? version : undefined;
}

/** Puts the installed toolchain on PATH in VS Code's integrated terminals. */
export function exposeToolchainToTerminals(context: vscode.ExtensionContext): void {
    const collection = context.environmentVariableCollection;
    if (installedVersion(context)) {
        collection.prepend('PATH', toolchainDir(context) + path.delimiter);
    } else {
        collection.delete('PATH');
    }
}

/**
 * Downloads the latest Pleno release for this machine from GitHub, checks its
 * SHA-256 checksum, and installs it into the extension's storage. Returns the
 * installed version, or undefined when nothing was installed.
 */
export async function installToolchain(
    context: vscode.ExtensionContext,
    output: vscode.OutputChannel,
): Promise<string | undefined> {
    const target = releaseTarget();
    if (!target) {
        vscode.window.showErrorMessage(
            `There is no prebuilt Pleno toolchain for ${process.platform}-${process.arch}. Build it from source: https://pleno.mohirlab.uz/guide/installation#from-source`,
        );
        return undefined;
    }

    return vscode.window.withProgress({
        location: vscode.ProgressLocation.Notification,
        title: 'Pleno toolchain',
        cancellable: false,
    }, async (progress) => {
        try {
            progress.report({ message: 'Finding the latest release...' });
            const release: Release = JSON.parse(
                (await download(`https://api.github.com/repos/${REPOSITORY}/releases/latest`)).toString('utf8'),
            );
            const version = release.tag_name;
            const current = installedVersion(context);
            if (current === version) {
                const choice = await vscode.window.showInformationMessage(
                    `Pleno ${version} is already installed and is the latest release.`,
                    'Reinstall',
                );
                if (choice !== 'Reinstall') {
                    return undefined;
                }
            }

            const extension = process.platform === 'win32' ? 'zip' : 'tar.gz';
            const archiveName = `pleno-${version}-${target}.${extension}`;
            const archiveAsset = release.assets.find((asset) => asset.name === archiveName);
            const checksumAsset = release.assets.find((asset) => asset.name === `${archiveName}.sha256`);
            if (!archiveAsset || !checksumAsset) {
                throw new Error(`release ${version} has no ${archiveName} with a checksum`);
            }

            output.appendLine(`Downloading ${archiveAsset.browser_download_url}`);
            let reported = 0;
            const archive = await download(archiveAsset.browser_download_url, (fraction) => {
                const percent = Math.floor(fraction * 100);
                if (percent > reported) {
                    progress.report({ message: `Downloading ${version} (${percent}%)`, increment: percent - reported });
                    reported = percent;
                }
            });

            progress.report({ message: 'Verifying the checksum...' });
            const expected = (await download(checksumAsset.browser_download_url))
                .toString('utf8').trim().split(/\s+/)[0].toLowerCase();
            const actual = crypto.createHash('sha256').update(archive).digest('hex');
            if (actual !== expected) {
                throw new Error(`checksum mismatch for ${archiveName}: expected ${expected}, got ${actual}`);
            }

            progress.report({ message: 'Installing...' });
            const staging = fs.mkdtempSync(path.join(os.tmpdir(), 'pleno-toolchain-'));
            const archivePath = path.join(staging, archiveName);
            const unpacked = path.join(staging, 'unpacked');
            fs.writeFileSync(archivePath, archive);
            fs.mkdirSync(unpacked);
            await extract(archivePath, unpacked);
            if (process.platform !== 'win32') {
                for (const file of fs.readdirSync(unpacked)) {
                    fs.chmodSync(path.join(unpacked, file), 0o755);
                }
            }

            const destination = toolchainDir(context);
            fs.rmSync(destination, { recursive: true, force: true });
            fs.mkdirSync(path.dirname(destination), { recursive: true });
            fs.cpSync(unpacked, destination, { recursive: true });
            fs.rmSync(staging, { recursive: true, force: true });

            await context.globalState.update(VERSION_KEY, version);
            clearToolCache();
            exposeToolchainToTerminals(context);
            output.appendLine(`Installed Pleno ${version} into ${destination}`);
            return version;
        } catch (error) {
            const message = error instanceof Error ? error.message : String(error);
            output.appendLine(`Toolchain install failed: ${message}`);
            vscode.window.showErrorMessage(`Installing the Pleno toolchain failed: ${message}`);
            return undefined;
        }
    });
}

/** Removes the toolchain installed by this extension. */
export async function uninstallToolchain(context: vscode.ExtensionContext): Promise<boolean> {
    if (!installedVersion(context)) {
        vscode.window.showInformationMessage('No Pleno toolchain was installed by this extension.');
        return false;
    }
    fs.rmSync(toolchainDir(context), { recursive: true, force: true });
    await context.globalState.update(VERSION_KEY, undefined);
    clearToolCache();
    exposeToolchainToTerminals(context);
    return true;
}
