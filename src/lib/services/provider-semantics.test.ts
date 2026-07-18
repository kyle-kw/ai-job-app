import { describe, expect, it } from 'vitest';
import { backend } from './backend';

describe('provider test and save semantics', () => {
  it('keeps a connection test read-only even when it fails', async () => {
    const before = await backend.bootstrap();
    const provider = structuredClone(before.providers[0]);
    const result = await backend.testProvider({
      ...provider,
      apiKey: undefined,
      apiKeyRef: undefined
    });
    const after = await backend.bootstrap();

    expect(result.ok).toBe(false);
    expect(after.providers).toEqual(before.providers);
    expect(after.readiness.ai).toBe(before.readiness.ai);
  });

  it('does not replace the saved provider when validation before save fails', async () => {
    const before = await backend.bootstrap();
    const provider = structuredClone(before.providers[0]);
    await expect(
      backend.saveProvider({ ...provider, baseUrl: '', apiKey: undefined, apiKeyRef: undefined })
    ).rejects.toThrow();
    const after = await backend.bootstrap();
    expect(after.providers).toEqual(before.providers);
  });
});
