import { readFileSync, writeFileSync } from 'node:fs';

const version = process.argv[2];
if (!/^\d+\.\d+\.\d+$/.test(version ?? '')) {
  console.error('Usage: npm run version:set -- <major.minor.patch>');
  process.exit(2);
}

function updateJson(path, mutate) {
  const value = JSON.parse(readFileSync(path, 'utf8'));
  mutate(value);
  writeFileSync(path, `${JSON.stringify(value, null, 2)}\n`, 'utf8');
}

updateJson('package.json', (value) => {
  value.version = version;
});
updateJson('package-lock.json', (value) => {
  value.version = version;
  value.packages[''].version = version;
});
updateJson('src-tauri/tauri.conf.json', (value) => {
  value.version = version;
});

for (const path of ['src-tauri/Cargo.toml', 'sidecar/pyproject.toml']) {
  const source = readFileSync(path, 'utf8');
  const updated = source.replace(/^version\s*=\s*"[^"]+"/m, `version = "${version}"`);
  if (updated === source) throw new Error(`Unable to update version in ${path}`);
  writeFileSync(path, updated, 'utf8');
}

{
  const path = 'src-tauri/Cargo.lock';
  const source = readFileSync(path, 'utf8');
  const updated = source.replace(
    /(\[\[package\]\]\s*name\s*=\s*"ai-job-app"\s*version\s*=\s*)"[^"]+"/m,
    `$1"${version}"`
  );
  if (updated === source) throw new Error(`Unable to update version in ${path}`);
  writeFileSync(path, updated, 'utf8');
}

for (const [path, pattern, replacement] of [
  [
    'sidecar/uv.lock',
    /(\[\[package\]\]\s*name\s*=\s*"ai-job-app-sidecar"\s*version\s*=\s*)"[^"]+"/m,
    `$1"${version}"`
  ],
  ['sidecar/worker.py', /^APP_VERSION\s*=\s*"[^"]+"/m, `APP_VERSION = "${version}"`]
]) {
  const source = readFileSync(path, 'utf8');
  const updated = source.replace(pattern, replacement);
  if (updated === source) throw new Error(`Unable to update version in ${path}`);
  writeFileSync(path, updated, 'utf8');
}

console.log(`Updated application version to ${version}`);
