import { readFileSync, readdirSync, statSync, writeFileSync } from 'node:fs';
import { basename, join } from 'node:path';
import { readChangelogSection } from './changelog-section.mjs';

const [directory, repository, tag] = process.argv.slice(2);
if (!directory || !repository || !tag) {
  throw new Error('Usage: node scripts/generate-latest-json.mjs <asset-dir> <owner/repo> <v-tag>');
}
const packageJson = JSON.parse(readFileSync('package.json', 'utf8'));
if (tag !== `v${packageJson.version}`)
  throw new Error(`Tag ${tag} does not match v${packageJson.version}`);

const files = readdirSync(directory);
const requireOne = (pattern, label) => {
  const matches = files.filter((file) => pattern.test(file));
  if (matches.length !== 1)
    throw new Error(`Expected exactly one ${label}; found ${matches.join(', ') || 'none'}`);
  return matches[0];
};
const windows = requireOne(/windows-x86_64-unsigned-setup\.exe$/, 'Windows NSIS updater');
const macIntel = requireOne(/macos-x86_64-unsigned\.app\.tar\.gz$/, 'Intel macOS updater');
const macArm = requireOne(/macos-aarch64-unsigned\.app\.tar\.gz$/, 'Apple Silicon updater');

const notes = readChangelogSection(packageJson.version);
const asset = (file) => {
  const signaturePath = join(directory, `${file}.sig`);
  if (!statSync(signaturePath).size) throw new Error(`Empty updater signature for ${file}`);
  return {
    signature: readFileSync(signaturePath, 'utf8').trim(),
    url: `https://github.com/${repository}/releases/download/${tag}/${encodeURIComponent(basename(file))}`
  };
};

const manifest = {
  version: packageJson.version,
  notes,
  pub_date: new Date().toISOString(),
  platforms: {
    'windows-x86_64': asset(windows),
    'darwin-x86_64': asset(macIntel),
    'darwin-aarch64': asset(macArm)
  }
};
writeFileSync(join(directory, 'latest.json'), `${JSON.stringify(manifest, null, 2)}\n`, 'utf8');
