import type { AiProviderConfig } from '$lib/types';

export function availableProviderConfigs(
  providers: AiProviderConfig[],
  production = import.meta.env.PROD
): AiProviderConfig[] {
  const available = providers
    .filter((provider) => provider.kind !== ('openrouter' as string))
    .filter((provider) => !production || provider.kind === 'custom')
    .map((provider) => ({ ...provider }));
  if (available.length && !available.some((provider) => provider.isDefault)) {
    available[0].isDefault = true;
  }
  return available;
}
