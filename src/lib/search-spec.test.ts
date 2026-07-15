import { describe, expect, it } from 'vitest';
import { createSearchSpec } from './search-spec';

describe('createSearchSpec', () => {
  it('provides first-run defaults and clones every saved search field', () => {
    expect(createSearchSpec()).toEqual({
      keyword: '', city: '上海', pages: 1, salary: '', experience: '', degree: '', companyScale: ''
    });
    const saved = {
      keyword: 'AI Agent', city: '杭州', pages: 4, salary: '405', experience: '105', degree: '203', companyScale: '303'
    };
    const restored = createSearchSpec(saved);
    expect(restored).toEqual(saved);
    expect(restored).not.toBe(saved);
  });
});
