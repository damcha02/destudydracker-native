#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 1 ]; then
  echo "Usage: ./update-version.sh <version>"
  echo "Example: ./update-version.sh 0.1.4"
  exit 1
fi

VERSION="$1"

if [[ ! "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+([+-][0-9A-Za-z.-]+)?$ ]]; then
  echo "Error: version must be a semver value like 0.1.4"
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

node - "$SCRIPT_DIR" "$VERSION" <<'NODE'
const fs = require('fs');
const path = require('path');

const root = process.argv[2];
const version = process.argv[3];

const files = {
  packageJson: path.join(root, 'desktop/package.json'),
  packageLock: path.join(root, 'desktop/package-lock.json'),
  tauriConfig: path.join(root, 'desktop/src-tauri/tauri.conf.json'),
  cargoToml: path.join(root, 'desktop/src-tauri/Cargo.toml'),
  cargoLock: path.join(root, 'desktop/src-tauri/Cargo.lock'),
};

function replaceRequired(content, pattern, replacement, file) {
  if (!pattern.test(content)) {
    throw new Error(`Could not update ${path.relative(root, file)}`);
  }
  return content.replace(pattern, replacement);
}

let packageJson = fs.readFileSync(files.packageJson, 'utf8');
packageJson = replaceRequired(packageJson, /^(\s*"version"\s*:\s*)"[^"]+"/m, `$1"${version}"`, files.packageJson);
JSON.parse(packageJson);
fs.writeFileSync(files.packageJson, packageJson);

let packageLock = fs.readFileSync(files.packageLock, 'utf8');
packageLock = replaceRequired(packageLock, /^(\s*"version"\s*:\s*)"[^"]+"/m, `$1"${version}"`, files.packageLock);
packageLock = replaceRequired(
  packageLock,
  /(""\s*:\s*\{\n\s*"name"\s*:\s*"[^"]+",\n\s*"version"\s*:\s*)"[^"]+"/,
  `$1"${version}"`,
  files.packageLock,
);
JSON.parse(packageLock);
fs.writeFileSync(files.packageLock, packageLock);

let tauriConfig = fs.readFileSync(files.tauriConfig, 'utf8');
tauriConfig = replaceRequired(tauriConfig, /^(\s*"version"\s*:\s*)"[^"]+"/m, `$1"${version}"`, files.tauriConfig);
JSON.parse(tauriConfig);
fs.writeFileSync(files.tauriConfig, tauriConfig);

let cargoToml = fs.readFileSync(files.cargoToml, 'utf8');
const packageName = cargoToml.match(/^name\s*=\s*"([^"]+)"/m)?.[1];
if (!packageName) {
  throw new Error('Could not determine Cargo package name');
}
cargoToml = replaceRequired(
  cargoToml,
  /(\[package\][\s\S]*?^version\s*=\s*)"[^"]+"/m,
  `$1"${version}"`,
  files.cargoToml,
);
fs.writeFileSync(files.cargoToml, cargoToml);

let cargoLock = fs.readFileSync(files.cargoLock, 'utf8');
const escapedPackageName = packageName.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
const cargoLockPattern = new RegExp(`(\\[\\[package\\]\\]\\nname = "${escapedPackageName}"\\nversion = )"[^"]+"`);
cargoLock = replaceRequired(cargoLock, cargoLockPattern, `$1"${version}"`, files.cargoLock);
fs.writeFileSync(files.cargoLock, cargoLock);

for (const file of Object.values(files)) {
  console.log(`Updated ${path.relative(root, file)} to ${version}`);
}
NODE

BRANCH="$(git -C "$SCRIPT_DIR" branch --show-current 2>/dev/null || true)"
if [ -z "$BRANCH" ]; then
  BRANCH="main"
fi

echo
echo "Version updated to $VERSION."
echo
echo "Next commands to commit, tag, push, and trigger the GitHub release build:"
echo "  git status"
echo "  git add update-version.sh desktop/package.json desktop/package-lock.json desktop/src-tauri/Cargo.toml desktop/src-tauri/Cargo.lock desktop/src-tauri/tauri.conf.json"
echo "  git commit -m \"Release v$VERSION\""
echo "  git tag v$VERSION"
echo "  git push origin $BRANCH"
echo "  git push origin v$VERSION"
echo
echo "Your GitHub workflow creates the release as a draft."
echo "After the action finishes, review and publish the draft release on GitHub."

if [ ! -d "$SCRIPT_DIR/.github/workflows" ] || ! ls "$SCRIPT_DIR/.github/workflows"/* >/dev/null 2>&1; then
  echo
  echo "Note: no GitHub Actions workflow was found in .github/workflows."
  echo "A tag push will only build/release automatically after a release workflow exists."
fi
