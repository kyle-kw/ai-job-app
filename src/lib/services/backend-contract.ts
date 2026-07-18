import type { createBackendAdapter } from './backend-adapter';

export type Backend = ReturnType<typeof createBackendAdapter>;
