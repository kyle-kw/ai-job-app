import { readFileSync } from 'node:fs';

const escapeRegExp = (value) => value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

export function extractChangelogSection(changelog, version) {
  const heading = new RegExp(`^## \\[${escapeRegExp(version)}\\](?:[^\\r\\n]*)$`, 'm');
  const match = heading.exec(changelog);
  if (!match) throw new Error(`CHANGELOG.md has no ## [${version}] section`);

  const remainder = changelog.slice(match.index + match[0].length).replace(/^\r?\n/, '');
  const nextHeading = remainder.search(/^## \[/m);
  const notes = (nextHeading >= 0 ? remainder.slice(0, nextHeading) : remainder).trim();
  if (!notes) throw new Error(`CHANGELOG.md section ${version} is empty`);
  return notes;
}

export function readChangelogSection(version, path = 'CHANGELOG.md') {
  return extractChangelogSection(readFileSync(path, 'utf8'), version);
}
