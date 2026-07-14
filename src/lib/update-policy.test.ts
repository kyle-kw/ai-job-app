import { describe, expect, it } from 'vitest';
import { shouldStartAutomaticUpdateCheck } from '$lib/update-policy';
import type { AppSettings } from '$lib/types';

const settings: AppSettings = {
  advancedMode: false,
  automaticUpdateChecks: true,
  privacyAcknowledgedVersion: '2026-07-14',
  lastUpdateCheckAt: null
};

describe('automatic update policy', () => {
  it('only starts after privacy acknowledgement while enabled', () => {
    expect(shouldStartAutomaticUpdateCheck(settings, '2026-07-14', '2026-07-14')).toBe(true);
    expect(shouldStartAutomaticUpdateCheck({ ...settings, automaticUpdateChecks: false }, '2026-07-14', '2026-07-14')).toBe(false);
    expect(shouldStartAutomaticUpdateCheck(settings, '', '2026-07-14')).toBe(false);
  });
});
