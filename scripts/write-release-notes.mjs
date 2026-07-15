import { writeFileSync } from 'node:fs';
import { readChangelogSection } from './changelog-section.mjs';

const [version, output] = process.argv.slice(2);
if (!version || !output) {
  throw new Error('Usage: node scripts/write-release-notes.mjs <version> <output>');
}

writeFileSync(output, `${readChangelogSection(version)}\n`, 'utf8');
