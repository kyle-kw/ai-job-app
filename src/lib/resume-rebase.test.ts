import { describe, expect, it } from 'vitest';
import { mockResume } from '$lib/mock-data';
import { applyResumeRebase, buildResumeRebasePreview } from '$lib/resume-rebase';

describe('resume variant rebase', () => {
  it('automatically takes untouched master fields and asks for true conflicts', () => {
    const base = structuredClone(mockResume);
    const master = {
      ...structuredClone(base),
      headline: '高级 AI 工程师',
      summary: '主简历新简介',
      version: base.version + 1
    };
    const variant = {
      ...structuredClone(base),
      id: 'variant',
      summary: '岗位定制简介',
      version: 2
    };
    const preview = buildResumeRebasePreview('variant', 2, base.version, base, master, variant);

    expect(preview.autoChanges.map((item) => item.path)).toContain('/headline');
    expect(preview.conflicts.map((item) => item.path)).toContain('/summary');
    const applied = applyResumeRebase(variant, master, preview, [
      { path: '/summary', choice: 'variant' }
    ]);
    expect(applied.headline).toBe('高级 AI 工程师');
    expect(applied.summary).toBe('岗位定制简介');
  });
});
