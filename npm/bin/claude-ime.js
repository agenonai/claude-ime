#!/usr/bin/env node

const os = require('os');
const path = require('path');
const fs = require('fs');
const { execFileSync } = require('child_process');
const https = require('https');
const { createWriteStream } = require('fs');
const { pipeline } = require('stream');
const zlib = require('zlib');
const { Extract } = require('tar');

// Read version from package.json
const packageJson = JSON.parse(fs.readFileSync(path.join(__dirname, '..', 'package.json'), 'utf-8'));
const version = packageJson.version;

// Detect platform and architecture
const platform = os.platform();
const arch = os.arch();

// Map to release binary filename
const binaryMap = {
  'darwin-arm64': 'claude-ime-aarch64-apple-darwin.tar.gz',
  'darwin-x64': 'claude-ime-x86_64-apple-darwin.tar.gz',
  'linux-x64': 'claude-ime-x86_64-unknown-linux-gnu.tar.gz',
  'linux-arm64': 'claude-ime-aarch64-unknown-linux-gnu.tar.gz',
  'win32-x64': 'claude-ime-x86_64-pc-windows-msvc.zip',
};

const key = `${platform}-${arch}`;
const filename = binaryMap[key];

if (!filename) {
  console.error(`Error: Unsupported platform-arch combination: ${key}`);
  process.exit(1);
}

// Cache directory
const cacheDir = path.join(os.homedir(), '.cache', 'claude-ime', 'bin');
const binaryPath = path.join(cacheDir, platform === 'win32' ? 'claude-ime.exe' : 'claude-ime');
const versionFile = path.join(cacheDir, 'VERSION');

// Ensure cache directory exists
if (!fs.existsSync(cacheDir)) {
  fs.mkdirSync(cacheDir, { recursive: true });
}

// Check if binary exists and version matches
let needsDownload = true;
if (fs.existsSync(binaryPath) && fs.existsSync(versionFile)) {
  const cachedVersion = fs.readFileSync(versionFile, 'utf-8').trim();
  if (cachedVersion === version) {
    needsDownload = false;
  }
}

function downloadAndExtract() {
  return new Promise((resolve, reject) => {
    const url = `https://github.com/agenon/claude-ime/releases/download/v${version}/${filename}`;
    const archivePath = path.join(cacheDir, filename);

    console.error(`Downloading claude-ime v${version}...`);

    https.get(url, (res) => {
      if (res.statusCode !== 200) {
        reject(new Error(`Failed to download: HTTP ${res.statusCode}`));
        return;
      }

      const file = createWriteStream(archivePath);
      res.pipe(file);

      file.on('finish', () => {
        file.close();
        console.error('Download complete. Extracting...');

        if (filename.endsWith('.tar.gz')) {
          const extractStream = Extract({ cwd: cacheDir });
          const readStream = fs.createReadStream(archivePath);

          pipeline(readStream, zlib.createGunzip(), extractStream, (err) => {
            if (err) {
              reject(err);
            } else {
              // Make binary executable
              fs.chmodSync(binaryPath, 0o755);
              // Write version file
              fs.writeFileSync(versionFile, version);
              console.error('Installation complete.');
              resolve();
            }
          });
        } else if (filename.endsWith('.zip')) {
          const AdmZip = require('adm-zip');
          const zip = new AdmZip(archivePath);
          zip.extractAllTo(cacheDir, true);
          fs.chmodSync(binaryPath, 0o755);
          fs.writeFileSync(versionFile, version);
          console.error('Installation complete.');
          resolve();
        }
      });

      file.on('error', reject);
    }).on('error', reject);
  });
}

async function run() {
  try {
    if (needsDownload) {
      await downloadAndExtract();
    }

    // Execute binary with pass-through args and stdio
    execFileSync(binaryPath, process.argv.slice(2), {
      stdio: 'inherit',
    });
  } catch (err) {
    if (err.status !== undefined) {
      process.exit(err.status);
    }
    console.error(err.message);
    process.exit(1);
  }
}

run();
