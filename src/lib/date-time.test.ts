import { describe, expect, it } from 'vitest';
import { formatLocalDateTime } from './date-time';

describe('formatLocalDateTime', () => {
  it('uses local date parts with a stable seconds-level format', () => {
    const local = new Date(2026, 6, 15, 11, 26, 46);
    expect(formatLocalDateTime(local.toISOString())).toBe('2026-07-15 11:26:46');
  });

  it('returns null for missing or invalid values', () => {
    expect(formatLocalDateTime(null)).toBeNull();
    expect(formatLocalDateTime('invalid')).toBeNull();
  });
});
