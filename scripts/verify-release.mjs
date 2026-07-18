import { readFileSync } from 'node:fs';
import { readChangelogSection } from './changelog-section.mjs';

const readJson = (path) => JSON.parse(readFileSync(path, 'utf8'));
const matchVersion = (path) => {
  const match = readFileSync(path, 'utf8').match(/^version\s*=\s*"([^"]+)"/m);
  if (!match) throw new Error(`Unable to read version from ${path}`);
  return match[1];
};

const packageJson = readJson('package.json');
const packageLock = readJson('package-lock.json');
const tauri = readJson('src-tauri/tauri.conf.json');
const tauriMacOS = readJson('src-tauri/tauri.macos.conf.json');
const tauriWindows = readJson('src-tauri/tauri.windows.conf.json');
const workerVersion = readFileSync('sidecar/worker.py', 'utf8').match(
  /^APP_VERSION\s*=\s*"([^"]+)"/m
)?.[1];
const uvProjectVersion = readFileSync('sidecar/uv.lock', 'utf8').match(
  /\[\[package\]\]\s*name\s*=\s*"ai-job-app-sidecar"\s*version\s*=\s*"([^"]+)"/m
)?.[1];
const versions = new Map([
  ['package.json', packageJson.version],
  ['package-lock.json', packageLock.version],
  ['package-lock root', packageLock.packages[''].version],
  ['tauri.conf.json', tauri.version],
  ['Cargo.toml', matchVersion('src-tauri/Cargo.toml')],
  [
    'Cargo.lock',
    readFileSync('src-tauri/Cargo.lock', 'utf8').match(
      /\[\[package\]\]\s*name\s*=\s*"ai-job-app"\s*version\s*=\s*"([^"]+)"/m
    )?.[1]
  ],
  ['sidecar/pyproject.toml', matchVersion('sidecar/pyproject.toml')],
  ['sidecar/uv.lock', uvProjectVersion],
  ['sidecar/worker.py', workerVersion]
]);

const expected = packageJson.version;
const mismatches = [...versions].filter(([, value]) => value !== expected);
if (mismatches.length) {
  throw new Error(
    `Version mismatch: ${[...versions].map(([name, value]) => `${name}=${value}`).join(', ')}`
  );
}

const githubRef = process.env.GITHUB_REF;
const tag =
  process.argv[2] ||
  (githubRef?.startsWith('refs/tags/')
    ? process.env.GITHUB_REF_NAME || githubRef.slice('refs/tags/'.length)
    : undefined);
if (tag && tag !== `v${expected}`) throw new Error(`Tag ${tag} does not match v${expected}`);
readChangelogSection(expected);

const updater = tauri.plugins?.updater;
if (
  !Array.isArray(updater?.endpoints) ||
  updater.endpoints.length === 0 ||
  !updater.endpoints.every((value) => value.startsWith('https://'))
) {
  throw new Error('Updater endpoints must use HTTPS');
}
if (
  updater.dangerousInsecureTransportProtocol ||
  updater.dangerousAcceptInvalidCerts ||
  updater.dangerousAcceptInvalidHostnames
) {
  throw new Error('Production updater configuration enables an insecure option');
}
if (process.env.REQUIRE_UPDATER_KEY === '1') {
  if (updater.pubkey === 'UPDATER_PUBLIC_KEY_NOT_CONFIGURED') {
    throw new Error('Production updater public key is not configured');
  }
  const decoded = Buffer.from(updater.pubkey, 'base64').toString('utf8');
  if (!decoded.includes('minisign public key'))
    throw new Error('Updater public key is not a valid minisign public key');
}

if (tauri.bundle?.createUpdaterArtifacts !== true) {
  throw new Error('Production bundles must create v2 updater artifacts');
}
const requireTargets = (label, config, required) => {
  const targets = config.bundle?.targets;
  if (!Array.isArray(targets)) throw new Error(`${label} bundle targets must be an explicit array`);
  const missing = required.filter((target) => !targets.includes(target));
  if (missing.length) throw new Error(`${label} bundle targets are missing: ${missing.join(', ')}`);
};
requireTargets('macOS', tauriMacOS, ['app', 'dmg']);
requireTargets('Windows', tauriWindows, ['nsis']);

console.log(`Release configuration verified for v${expected}`);
