#!/usr/bin/env node

const { execSync, spawn } = require("child_process");
const AdmZip = require("adm-zip");
const path = require("path");
const fs = require("fs");
const { ensureBinary, BINARY_TAG, CACHE_DIR, LOCAL_DEV_MODE, LOCAL_DIST_DIR, R2_BASE_URL, getLatestVersion } = require("./download");

const CLI_VERSION = require("../package.json").version;

// GOST version and download URLs (v3+)
const GOST_VERSION = "3.2.6";
const GOST_RELEASES_BASE = "https://github.com/go-gost/gost/releases/download";

// GOST download URLs for each platform
function getGostUrl(platform) {
  const urls = {
    "linux-x64": `${GOST_RELEASES_BASE}/v${GOST_VERSION}/gost_${GOST_VERSION}_linux_amd64.tar.gz`,
    "linux-arm64": `${GOST_RELEASES_BASE}/v${GOST_VERSION}/gost_${GOST_VERSION}_linux_arm64.tar.gz`,
    "macos-x64": `${GOST_RELEASES_BASE}/v${GOST_VERSION}/gost_${GOST_VERSION}_darwin_amd64.tar.gz`,
    "macos-arm64": `${GOST_RELEASES_BASE}/v${GOST_VERSION}/gost_${GOST_VERSION}_darwin_arm64.tar.gz`,
    "windows-x64": `${GOST_RELEASES_BASE}/v${GOST_VERSION}/gost_${GOST_VERSION}_windows_amd64.zip`,
    "windows-arm64": `${GOST_RELEASES_BASE}/v${GOST_VERSION}/gost_${GOST_VERSION}_windows_arm64.zip`,
  };
  return urls[platform];
}

// Resolve effective arch for our published 64-bit binaries only.
// Any ARM → arm64; anything else → x64. On macOS, handle Rosetta.
function getEffectiveArch() {
  const platform = process.platform;
  const nodeArch = process.arch;

  if (platform === "darwin") {
    // If Node itself is arm64, we're natively on Apple silicon
    if (nodeArch === "arm64") return "arm64";

    // Otherwise check for Rosetta translation
    try {
      const translated = execSync("sysctl -in sysctl.proc_translated", {
        encoding: "utf8",
      }).trim();
      if (translated === "1") return "arm64";
    } catch {
      // sysctl key not present → assume true Intel
    }
    return "x64";
  }

  // Non-macOS: coerce to broad families we support
  if (/arm/i.test(nodeArch)) return "arm64";

  // On Windows with 32-bit Node (ia32), detect OS arch via env
  if (platform === "win32") {
    const pa = process.env.PROCESSOR_ARCHITECTURE || "";
    const paw = process.env.PROCESSOR_ARCHITEW6432 || "";
    if (/arm/i.test(pa) || /arm/i.test(paw)) return "arm64";
  }

  return "x64";
}

const platform = process.platform;
const arch = getEffectiveArch();

// Map to our build target names
function getPlatformDir() {
  if (platform === "linux" && arch === "x64") return "linux-x64";
  if (platform === "linux" && arch === "arm64") return "linux-arm64";
  if (platform === "win32" && arch === "x64") return "windows-x64";
  if (platform === "win32" && arch === "arm64") return "windows-arm64";
  if (platform === "darwin" && arch === "x64") return "macos-x64";
  if (platform === "darwin" && arch === "arm64") return "macos-arm64";

  console.error(`Unsupported platform: ${platform}-${arch}`);
  console.error("Supported platforms:");
  console.error("  - Linux x64");
  console.error("  - Linux ARM64");
  console.error("  - Windows x64");
  console.error("  - Windows ARM64");
  console.error("  - macOS x64 (Intel)");
  console.error("  - macOS ARM64 (Apple Silicon)");
  process.exit(1);
}

function getBinaryName(base) {
  return platform === "win32" ? `${base}.exe` : base;
}

const platformDir = getPlatformDir();
// In local dev mode, extract directly to dist directory; otherwise use global cache
const versionCacheDir = LOCAL_DEV_MODE
  ? path.join(LOCAL_DIST_DIR, platformDir)
  : path.join(CACHE_DIR, BINARY_TAG, platformDir);

function showProgress(downloaded, total) {
  const percent = total ? Math.round((downloaded / total) * 100) : 0;
  const mb = (downloaded / (1024 * 1024)).toFixed(1);
  const totalMb = total ? (total / (1024 * 1024)).toFixed(1) : "?";
  process.stderr.write(`\r   Downloading: ${mb}MB / ${totalMb}MB (${percent}%)`);
}

/**
 * Ensure GOST binary is available.
 * Returns the path to the GOST binary.
 */
async function ensureGost() {
  // Respect GOST_BINARY_PATH environment variable
  if (process.env.GOST_BINARY_PATH) {
    return process.env.GOST_BINARY_PATH;
  }

  const gostCacheDir = path.join(CACHE_DIR, "gost", GOST_VERSION);
  const gostBin = getBinaryName("gost");
  const gostPath = path.join(gostCacheDir, gostBin);

  // Return cached binary if exists
  if (fs.existsSync(gostPath)) {
    return gostPath;
  }

  // Download GOST
  const gostUrl = getGostUrl(platformDir);
  if (!gostUrl) {
    console.warn(`GOST not available for ${platformDir}, tunnel features will be disabled.`);
    return null;
  }

  fs.mkdirSync(gostCacheDir, { recursive: true });
  console.error(`Downloading GOST v${GOST_VERSION}...`);

  const downloadPath = gostPath + ".download";

  try {
    await downloadFile(gostUrl, downloadPath, null, showProgress);
    console.error(""); // newline after progress

    // Extract based on file type
    if (gostUrl.endsWith(".tar.gz")) {
      // .tar.gz files (GOST v3+ on Linux/macOS) - use system tar
      const extractDir = path.join(gostCacheDir, "extract");
      fs.mkdirSync(extractDir, { recursive: true });

      await new Promise((resolve, reject) => {
        const { exec } = require("child_process");
        exec(`tar -xzf "${downloadPath}" -C "${extractDir}"`, (error, stdout, stderr) => {
          if (error) {
            reject(new Error(`tar extraction failed: ${error.message}`));
          } else {
            resolve();
          }
        });
      });

      // Find and move the gost binary
      const extractedBin = path.join(extractDir, "gost");
      if (fs.existsSync(extractedBin)) {
        fs.renameSync(extractedBin, gostPath);
      } else {
        // Try to find any executable in extract dir
        const files = fs.readdirSync(extractDir);
        const exeFile = files.find(f => f.startsWith("gost"));
        if (exeFile) {
          fs.renameSync(path.join(extractDir, exeFile), gostPath);
        } else {
          throw new Error("gost binary not found in archive");
        }
      }

      // Cleanup extract dir
      try {
        fs.rmdirSync(extractDir, { recursive: true });
      } catch {}
    } else if (gostUrl.endsWith(".zip")) {
      // Windows .zip contains gost.exe
      const zip = new AdmZip(downloadPath);
      zip.extractAllTo(gostCacheDir, true);
      // Move gost.exe if needed
      const extractedPath = path.join(gostCacheDir, "gost.exe");
      if (fs.existsSync(extractedPath) && extractedPath !== gostPath) {
        fs.renameSync(extractedPath, gostPath);
      }
    }

    // Cleanup download file
    try {
      fs.unlinkSync(downloadPath);
    } catch {}

    // Set executable permission (non-Windows)
    if (platform !== "win32") {
      try {
        fs.chmodSync(gostPath, 0o755);
      } catch {}
    }

    console.error(`GOST installed to: ${gostPath}`);
    return gostPath;
  } catch (err) {
    console.error(`Failed to download GOST: ${err.message}`);
    console.warn("Tunnel features will be disabled.");
    return null;
  }
}

/**
 * Download a file from URL to destination path.
 */
function downloadFile(url, destPath, expectedSha256, onProgress) {
  return new Promise((resolve, reject) => {
    const https = require("https");
    const crypto = require("crypto");

    const file = fs.createWriteStream(destPath);
    const hash = crypto.createHash("sha256");

    const cleanup = () => {
      try {
        fs.unlinkSync(destPath);
      } catch {}
    };

    https.get(url, (res) => {
      // Follow redirects
      if (res.statusCode === 301 || res.statusCode === 302) {
        file.close();
        cleanup();
        return downloadFile(res.headers.location, destPath, expectedSha256, onProgress)
          .then(resolve)
          .catch(reject);
      }

      if (res.statusCode !== 200) {
        file.close();
        cleanup();
        return reject(new Error(`HTTP ${res.statusCode} downloading ${url}`));
      }

      const totalSize = parseInt(res.headers["content-length"], 10);
      let downloadedSize = 0;

      res.on("data", (chunk) => {
        downloadedSize += chunk.length;
        hash.update(chunk);
        if (onProgress) onProgress(downloadedSize, totalSize);
      });
      res.pipe(file);

      file.on("finish", () => {
        file.close();
        const actualSha256 = hash.digest("hex");
        if (expectedSha256 && actualSha256 !== expectedSha256) {
          cleanup();
          reject(new Error(`Checksum mismatch: expected ${expectedSha256}, got ${actualSha256}`));
        } else {
          resolve(destPath);
        }
      });
    }).on("error", (err) => {
      file.close();
      cleanup();
      reject(err);
    });
  });
}

async function extractAndRun(baseName, launch) {
  const binName = getBinaryName(baseName);
  const binPath = path.join(versionCacheDir, binName);
  const zipPath = path.join(versionCacheDir, `${baseName}.zip`);

  // Clean old binary if exists
  try {
    if (fs.existsSync(binPath)) {
      fs.unlinkSync(binPath);
    }
  } catch (err) {
    if (process.env.VIBE_KANBAN_DEBUG) {
      console.warn(`Warning: Could not delete existing binary: ${err.message}`);
    }
  }

  // Download if not cached
  if (!fs.existsSync(zipPath)) {
    console.error(`Downloading ${baseName}...`);
    try {
      await ensureBinary(platformDir, baseName, showProgress);
      console.error(""); // newline after progress
    } catch (err) {
      console.error(`\nDownload failed: ${err.message}`);
      process.exit(1);
    }
  }

  // Extract
  if (!fs.existsSync(binPath)) {
    try {
      const zip = new AdmZip(zipPath);
      zip.extractAllTo(versionCacheDir, true);
    } catch (err) {
      console.error("Extraction failed:", err.message);
      try {
        fs.unlinkSync(zipPath);
      } catch {}
      process.exit(1);
    }
  }

  if (!fs.existsSync(binPath)) {
    console.error(`Extracted binary not found at: ${binPath}`);
    console.error("This usually indicates a corrupt download. Please try again.");
    process.exit(1);
  }

  // Set permissions (non-Windows)
  if (platform !== "win32") {
    try {
      fs.chmodSync(binPath, 0o755);
    } catch {}
  }

  return launch(binPath);
}

async function main() {
  fs.mkdirSync(versionCacheDir, { recursive: true });

  const args = process.argv.slice(2);
  const isMcpMode = args.includes("--mcp");
  const isReviewMode = args[0] === "review";

  // Ensure GOST is available (only for main mode, not MCP/Review)
  // Skip if GOST_BINARY_PATH is already set
  if (!isMcpMode && !isReviewMode && !process.env.GOST_BINARY_PATH) {
    const gostPath = await ensureGost();
    if (gostPath) {
      process.env.GOST_BINARY_PATH = gostPath;
    }
  }

  // Non-blocking update check (skip in MCP mode, local dev mode, and when R2 URL not configured)
  const hasValidR2Url = !R2_BASE_URL.startsWith("__");
  if (!isMcpMode && !LOCAL_DEV_MODE && hasValidR2Url) {
    getLatestVersion()
      .then((latest) => {
        if (latest && latest !== CLI_VERSION) {
          setTimeout(() => {
            console.log(`\nUpdate available: ${CLI_VERSION} -> ${latest}`);
            console.log(`Run: npx vibe-kanban@latest`);
          }, 2000);
        }
      })
      .catch(() => {});
  }

  if (isMcpMode) {
    await extractAndRun("vibe-kanban-mcp", (bin) => {
      const proc = spawn(bin, [], { stdio: "inherit" });
      proc.on("exit", (c) => process.exit(c || 0));
      proc.on("error", (e) => {
        console.error("MCP server error:", e.message);
        process.exit(1);
      });
      process.on("SIGINT", () => {
        proc.kill("SIGINT");
      });
      process.on("SIGTERM", () => proc.kill("SIGTERM"));
    });
  } else if (isReviewMode) {
    await extractAndRun("vibe-kanban-review", (bin) => {
      const reviewArgs = args.slice(1);
      const proc = spawn(bin, reviewArgs, { stdio: "inherit" });
      proc.on("exit", (c) => process.exit(c || 0));
      proc.on("error", (e) => {
        console.error("Review CLI error:", e.message);
        process.exit(1);
      });
    });
  } else {
    const modeLabel = LOCAL_DEV_MODE ? " (local dev)" : "";
    console.log(`Starting vibe-kanban v${CLI_VERSION}${modeLabel}...`);
    await extractAndRun("vibe-kanban", (bin) => {
      if (platform === "win32") {
        execSync(`"${bin}"`, { stdio: "inherit" });
      } else {
        execSync(`"${bin}"`, { stdio: "inherit" });
      }
    });
  }
}

main().catch((err) => {
  console.error("Fatal error:", err.message);
  if (process.env.VIBE_KANBAN_DEBUG) {
    console.error(err.stack);
  }
  process.exit(1);
});
