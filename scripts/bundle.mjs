import { spawnSync } from 'node:child_process';
import { resolve } from 'node:path';
import { runTauri } from './toolchain.mjs';

const python = process.platform === 'win32' ? 'python' : 'python3';
const sidecar = spawnSync(python, [resolve('scripts/build_sidecar.py')], {
  stdio: 'inherit'
});
if (sidecar.status !== 0) process.exit(sidecar.status ?? 1);

const tauri = runTauri(['build', '--config', 'src-tauri/tauri.bundle.conf.json']);
if (tauri.status !== 0) process.exit(tauri.status ?? 1);
