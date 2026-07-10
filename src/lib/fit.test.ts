import { describe, expect, it } from 'vitest';
import { normalizedOverall, verdictFor } from './fit';

describe('fit scoring', () => {
  it('renormalizes around unknown dimensions', () => {
    const result = normalizedOverall([
      { key: 'technical', label: '技能', score: 80, weight: 30, note: '', evidence: [] },
      { key: 'experience', label: '经验', score: 60, weight: 25, note: '', evidence: [] },
      { key: 'behavior', label: '行为', score: null, weight: 15, note: '', evidence: [] },
      { key: 'career', label: '方向', score: null, weight: 30, note: '', evidence: [] }
    ]);
    expect(result.score).toBe(71);
    expect(result.confidence).toBe(55);
  });

  it('uses stable verdict thresholds', () => {
    expect(verdictFor(75)).toBe('strong');
    expect(verdictFor(60)).toBe('good');
    expect(verdictFor(45)).toBe('moderate');
    expect(verdictFor(30)).toBe('weak');
    expect(verdictFor(29)).toBe('poor');
  });
});
