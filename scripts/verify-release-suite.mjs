import { spawnSync } from 'node:child_process';

const tests = spawnSync(process.execPath, ['--test', 'scripts/changelog-section.test.mjs'], {
  stdio: 'inherit'
});
if (tests.status !== 0) process.exit(tests.status ?? 1);

await import('./verify-release.mjs');
