import { describe, expect, it } from 'vitest';
import { defaultProviders } from '$lib/mock-data';
import { availableProviderConfigs } from '$lib/provider-policy';

describe('provider distribution policy', () => {
  it('keeps Xiaomi and OpenAI-compatible presets in development', () => {
    const providers = availableProviderConfigs(defaultProviders, false);
    expect(providers.map((provider) => provider.kind)).toEqual(['xiaomi', 'custom']);
    expect(providers.find((provider) => provider.kind === 'xiaomi')?.isDefault).toBe(true);
  });

  it('only exposes an effective OpenAI-compatible default in production', () => {
    const providers = availableProviderConfigs(defaultProviders, true);
    expect(providers).toHaveLength(1);
    expect(providers[0]).toMatchObject({ id: 'provider-custom', kind: 'custom', isDefault: true });
    expect(defaultProviders.find((provider) => provider.kind === 'xiaomi')?.isDefault).toBe(true);
  });
});
