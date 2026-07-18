import { createBackendAdapter, resetBrowserMockState } from './backend-adapter';
import type { Backend } from './backend-contract';

export const browserBackend = createBackendAdapter(true) satisfies Backend;

export { resetBrowserMockState };
