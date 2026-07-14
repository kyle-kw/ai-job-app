import type { AppSettings } from '$lib/types';

export function shouldStartAutomaticUpdateCheck(
  settings: AppSettings,
  acknowledgedPrivacyVersion: string,
  requiredPrivacyVersion: string
): boolean {
  return settings.automaticUpdateChecks
    && acknowledgedPrivacyVersion === requiredPrivacyVersion;
}
