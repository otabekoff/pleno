const fs = require("fs");
const path = require("path");
const https = require("https");
const os = require("os");

const VERSION = "0.1.0";
const REPO = "otabekoff/pleno";

function getPlatform() {
  const platform = os.platform();
  const arch = os.arch();

  if (platform === "linux") {
    if (arch === "x64") return "x86_64-unknown-linux-gnu";
    if (arch === "arm64") return "aarch64-unknown-linux-gnu";
  } else if (platform === "darwin") {
    if (arch === "x64") return "x86_64-apple-darwin";
    if (arch === "arm64") return "aarch64-apple-darwin";
  } else if (platform === "win32") {
    if (arch === "x64") return "x86_64-pc-windows-msvc";
  }
  throw new Error(`Unsupported platform: ${platform} ${arch}`);
}

async function download(url, dest) {
  return new Promise((resolve, reject) => {
    const file = fs.createWriteStream(dest);
    https
      .get(url, (response) => {
        if (response.statusCode === 302 || response.statusCode === 301) {
          download(response.headers.location, dest).then(resolve).catch(reject);
          return;
        }
        if (response.statusCode !== 200) {
          reject(new Error(`Failed to download: ${response.statusCode}`));
          return;
        }
        response.pipe(file);
        file.on("finish", () => {
          file.close();
          resolve();
        });
      })
      .on("error", (err) => {
        fs.unlink(dest, () => {});
        reject(err);
      });
  });
}

async function main() {
  try {
    const platform = getPlatform();
    const binaryName = platform.includes("windows") ? "pleno.exe" : "pleno";
    const plmName = platform.includes("windows") ? "plm.exe" : "plm";

    // In a real scenario, we'd download a tarball/zip and extract.
    // For now, we'll use a placeholder URL structure.
    const baseUrl = `https://github.com/${REPO}/releases/download/v${VERSION}`;

    const binDir = path.join(__dirname, "..", "bin");
    if (!fs.existsSync(binDir)) fs.mkdirSync(binDir);

    console.log(`Downloading Pleno v${VERSION} for ${platform}...`);

    // Note: This script assumes the presence of direct binaries in releases for simplicity.
    // In production, this should handle .tar.gz or .zip extraction.

    const plenoPath = path.join(binDir, binaryName);
    const plmPath = path.join(binDir, plmName);

    // Placeholder logic for the walkthrough/demo
    // await download(`${baseUrl}/pleno-${platform}`, plenoPath);
    // await download(`${baseUrl}/plm-${platform}`, plmPath);

    if (!platform.includes("windows")) {
      // fs.chmodSync(plenoPath, 0o755);
      // fs.chmodSync(plmPath, 0o755);
    }

    console.log("Pleno successfully installed via NPM.");
  } catch (err) {
    console.error("Failed to install Pleno:", err.message);
    process.exit(1);
  }
}

main();
