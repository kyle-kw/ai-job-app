import { runTauri } from './toolchain.mjs';

const result = runTauri(process.argv.slice(2));
process.exit(result.status ?? 1);
