import { readFileSync, writeFileSync } from 'node:fs';
import { basename } from 'node:path';
import { randomUUID } from 'node:crypto';

const output = process.argv[2];
const platform = process.argv[3] || 'unknown';
if (!output) throw new Error('Usage: node scripts/generate-sbom.mjs <output.json> [platform]');

const packageJson = JSON.parse(readFileSync('package.json', 'utf8'));
const packageLock = JSON.parse(readFileSync('package-lock.json', 'utf8'));
const components = new Map();

function add(type, name, version, purl, ecosystem) {
  if (!name || !version || name === packageJson.name) return;
  const key = `${ecosystem}:${name}@${version}`;
  components.set(key, {
    type,
    'bom-ref': purl,
    name,
    version,
    purl,
    properties: [{ name: '求职舱:dependency-ecosystem', value: ecosystem }]
  });
}

for (const [path, metadata] of Object.entries(packageLock.packages || {})) {
  if (!path || !metadata.version || !path.includes('node_modules/')) continue;
  const name =
    metadata.name || path.slice(path.lastIndexOf('node_modules/') + 13) || basename(path);
  add(
    'library',
    name,
    metadata.version,
    `pkg:npm/${encodeURIComponent(name)}@${metadata.version}`,
    'npm'
  );
}

function lockedPackages(path, ecosystem) {
  const text = readFileSync(path, 'utf8');
  const blocks = text.split(/\[\[package\]\]/).slice(1);
  for (const block of blocks) {
    const name = block.match(/^\s*name\s*=\s*"([^"]+)"/m)?.[1];
    const version = block.match(/^\s*version\s*=\s*"([^"]+)"/m)?.[1];
    if (!name || !version) continue;
    const purlType = ecosystem === 'cargo' ? 'cargo' : 'pypi';
    add(
      'library',
      name,
      version,
      `pkg:${purlType}/${encodeURIComponent(name)}@${version}`,
      ecosystem
    );
  }
}

lockedPackages('src-tauri/Cargo.lock', 'cargo');
lockedPackages('sidecar/uv.lock', 'pypi');

const bom = {
  bomFormat: 'CycloneDX',
  specVersion: '1.6',
  serialNumber: `urn:uuid:${randomUUID()}`,
  version: 1,
  metadata: {
    timestamp: new Date().toISOString(),
    component: {
      type: 'application',
      name: '求职舱',
      version: packageJson.version,
      properties: [{ name: '求职舱:release-platform', value: platform }]
    }
  },
  components: [...components.values()].sort((left, right) => left.purl.localeCompare(right.purl))
};

writeFileSync(output, `${JSON.stringify(bom, null, 2)}\n`, 'utf8');
console.log(`Wrote ${bom.components.length} locked components to ${output}`);
