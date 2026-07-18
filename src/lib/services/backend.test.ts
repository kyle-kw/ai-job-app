import { describe, expect, it } from 'vitest';
import packageMetadata from '../../../package.json';
import { backend } from './backend';
import { browserBackend } from './browser-backend';
import { tauriBackend } from './tauri-backend';

describe('browser backend metadata', () => {
  it('uses the application package version without claiming a desktop schema', async () => {
    const appInfo = await backend.getAppInfo();

    expect(appInfo.version).toBe(packageMetadata.version);
    expect(appInfo.schemaVersion).toBeNull();
  });

  it('keeps the browser and Tauri adapters on the same complete contract', () => {
    expect(Object.keys(browserBackend).sort()).toEqual(Object.keys(tauriBackend).sort());
    expect(Object.keys(browserBackend).sort()).toEqual(Object.keys(backend).sort());
  });
});
