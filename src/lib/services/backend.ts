import { browserBackend, resetBrowserMockState } from './browser-backend';
import type { Backend } from './backend-contract';
import { tauriBackend } from './tauri-backend';

const browserMode = () => typeof window === 'undefined' || !window.__TAURI_INTERNALS__;

export const backend: Backend = browserMode() ? browserBackend : tauriBackend;

export { resetBrowserMockState };
export type { Backend };
