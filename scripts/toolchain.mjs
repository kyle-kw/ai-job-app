import { existsSync } from 'node:fs';
import { homedir } from 'node:os';
import { delimiter, dirname, join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';

function cargoExecutable(directory) {
  return join(directory, process.platform === 'win32' ? 'cargo.exe' : 'cargo');
}

export function rustEnvironment() {
  const env = { ...process.env };
  const cargoHome = env.CARGO_HOME || join(homedir(), '.cargo');
  const cargoBin = join(cargoHome, 'bin');
  const pathKey = Object.keys(env).find((key) => key.toLowerCase() === 'path');
  const pathEntries = (pathKey ? env[pathKey] : '').split(delimiter).filter(Boolean);
  const requiredEntries = [dirname(process.execPath)];

  if (existsSync(cargoExecutable(cargoBin))) {
    requiredEntries.unshift(cargoBin);
  }

  for (const key of Object.keys(env)) {
    if (key.toLowerCase() === 'path') delete env[key];
  }

  const normalized = (value) => (process.platform === 'win32' ? value.toLowerCase() : value);
  const uniqueEntries = [...requiredEntries, ...pathEntries].filter(
    (entry, index, entries) =>
      entries.findIndex((candidate) => normalized(candidate) === normalized(entry)) === index
  );
  env.PATH = uniqueEntries.join(delimiter);

  return env;
}

export function runTauri(args, options = {}) {
  const env = rustEnvironment();
  const cargo = spawnSync('cargo', ['--version'], {
    env,
    encoding: 'utf8',
    windowsHide: true
  });

  if (cargo.error || cargo.status !== 0) {
    console.error('\n未找到 Rust/Cargo。请先安装 Rust：https://rustup.rs/');
    console.error('安装完成后重新打开终端，再执行当前 npm 命令。\n');
    return { status: 1 };
  }

  return spawnSync(process.execPath, [resolve('node_modules/@tauri-apps/cli/tauri.js'), ...args], {
    stdio: 'inherit',
    env,
    windowsHide: false,
    ...options
  });
}
