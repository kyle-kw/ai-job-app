import type { ClearDataResult } from '$lib/types';

export function shouldReloadAfterClear(result: ClearDataResult): boolean {
  return !result.restartRequired;
}
