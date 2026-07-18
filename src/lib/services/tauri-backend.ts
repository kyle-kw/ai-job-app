import { createBackendAdapter } from './backend-adapter';
import type { Backend } from './backend-contract';

export const tauriBackend = createBackendAdapter(false) satisfies Backend;
