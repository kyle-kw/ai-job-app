import { spawnSync } from 'node:child_process';
import { resolve } from 'node:path';

const commands = [
  [resolve('node_modules/@sveltejs/kit/svelte-kit.js'), ['sync']],
  [resolve('node_modules/svelte-check/bin/svelte-check'), ['--tsconfig', './tsconfig.json']]
];

for (const [script, args] of commands) {
  const result = spawnSync(process.execPath, [script, ...args], { stdio: 'inherit' });
  if (result.status !== 0) process.exit(result.status ?? 1);
}
