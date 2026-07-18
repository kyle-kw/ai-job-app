import { readFileSync, writeFileSync } from 'node:fs';

const output = process.argv[2];
if (!output) throw new Error('Usage: node scripts/generate-third-party-list.mjs <output.txt>');

const lock = JSON.parse(readFileSync('package-lock.json', 'utf8'));
const rows = [];
for (const [path, value] of Object.entries(lock.packages || {})) {
  if (!path.includes('node_modules/') || !value.version) continue;
  const name = value.name || path.slice(path.lastIndexOf('node_modules/') + 13);
  rows.push(`npm\t${name}\t${value.version}`);
}
for (const [path, ecosystem] of [
  ['src-tauri/Cargo.lock', 'cargo'],
  ['sidecar/uv.lock', 'pypi']
]) {
  for (const block of readFileSync(path, 'utf8')
    .split(/\[\[package\]\]/)
    .slice(1)) {
    const name = block.match(/^\s*name\s*=\s*"([^"]+)"/m)?.[1];
    const version = block.match(/^\s*version\s*=\s*"([^"]+)"/m)?.[1];
    if (name && version) rows.push(`${ecosystem}\t${name}\t${version}`);
  }
}
rows.sort((left, right) => left.localeCompare(right));
writeFileSync(
  output,
  `Generated from committed lockfiles; review with THIRD_PARTY_NOTICES.md.\n\n${rows.join('\n')}\n`,
  'utf8'
);
