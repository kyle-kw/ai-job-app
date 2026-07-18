import { describe, expect, it } from 'vitest';
import { displayDegree, formatDateRange, safeResumeFileName } from './resume-format';

describe('resume formatting', () => {
  it('never prefixes a single date with a range separator', () => {
    expect(formatDateRange('', '2024.12')).toBe('2024.12');
    expect(formatDateRange('', '—2024.12')).toBe('2024.12');
    expect(formatDateRange('2024.12', '至今')).toBe('2024.12—至今');
    expect(formatDateRange('', '')).toBe('');
  });

  it('displays a non-standard degree from its detail', () => {
    expect(displayDegree({ degree: '其他', degreeDetail: '大专' })).toBe('大专');
  });

  it('builds a safe timestamped PDF filename', () => {
    expect(safeResumeFileName('张:三', new Date(2026, 6, 12, 9, 8, 7))).toBe(
      '张_三-20260712-090807.pdf'
    );
  });
});
