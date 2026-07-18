import { describe, expect, it } from 'vitest';
import packageMetadata from '../../../package.json';
import { backend } from './backend';

describe('browser backend metadata', () => {
  it('uses the application package version without claiming a desktop schema', async () => {
    const appInfo = await backend.getAppInfo();

    expect(appInfo.version).toBe(packageMetadata.version);
    expect(appInfo.schemaVersion).toBeNull();
  });
});
