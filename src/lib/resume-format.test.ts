import { describe, expect, it } from 'vitest';
import { displayDegree, formatDateRange, normalizeDegree, safeResumeFileName } from './resume-format';

describe('resume formatting', () => {
  it('never prefixes a single date with a range separator', () => {
    expect(formatDateRange('', '2024.12')).toBe('2024.12');
    expect(formatDateRange('', '—2024.12')).toBe('2024.12');
    expect(formatDateRange('2024.12', '至今')).toBe('2024.12—至今');
    expect(formatDateRange('', '')).toBe('');
  });

  it('preserves non-standard degrees as detail', () => {
    expect(normalizeDegree('学士')).toEqual({ degree: '本科', degreeDetail: '' });
    expect(normalizeDegree('Bachelor of Science')).toEqual({ degree: '其他', degreeDetail: 'Bachelor of Science' });
    expect(displayDegree({ degree: '其他', degreeDetail: '大专' })).toBe('大专');
  });

  it('builds a safe timestamped PDF filename', () => {
    expect(safeResumeFileName('张:三', new Date(2026, 6, 12, 9, 8, 7))).toBe('张_三-20260712-090807.pdf');
  });
});
