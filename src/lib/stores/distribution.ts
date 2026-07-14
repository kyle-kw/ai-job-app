import { writable } from 'svelte/store';
import { backend } from '$lib/services/backend';
import type { AppUpdateInfo } from '$lib/types';

export const availableUpdate = writable<AppUpdateInfo | null>(null);
export const updateChecking = writable(false);
export const updateCheckError = writable<string | null>(null);

export async function checkForUpdate(manual: boolean) {
  updateChecking.set(true);
  if (manual) updateCheckError.set(null);
  try {
    const update = await backend.checkForUpdate(manual);
    availableUpdate.set(update);
    return update;
  } catch (error) {
    if (manual) updateCheckError.set(error instanceof Error ? error.message : String(error));
    return null;
  } finally {
    updateChecking.set(false);
  }
}
