import assert from 'node:assert/strict';
import test from 'node:test';
import { extractChangelogSection } from './changelog-section.mjs';

const changelog = `# Changelog

## [0.2.1] - 2026-07-15

### Changed

- current change

## [0.2.0] - 2026-07-14

### Added

- older change
`;

test('extracts only the requested changelog version body', () => {
  assert.equal(extractChangelogSection(changelog, '0.2.1'), '### Changed\n\n- current change');
  assert.equal(extractChangelogSection(changelog, '0.2.0'), '### Added\n\n- older change');
});

test('rejects missing and empty changelog sections', () => {
  assert.throws(() => extractChangelogSection(changelog, '9.9.9'), /has no/);
  assert.throws(
    () => extractChangelogSection('## [1.0.0]\n\n## [0.9.0]\n\n- older', '1.0.0'),
    /is empty/
  );
});
