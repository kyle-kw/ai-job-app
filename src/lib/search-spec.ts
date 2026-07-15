import type { SearchSpec } from '$lib/types';

export const DEFAULT_SEARCH_SPEC: Readonly<SearchSpec> = Object.freeze({
  keyword: '',
  city: '上海',
  pages: 1,
  salary: '',
  experience: '',
  degree: '',
  companyScale: ''
});

export function createSearchSpec(saved?: SearchSpec | null): SearchSpec {
  return { ...DEFAULT_SEARCH_SPEC, ...(saved ?? {}) };
}
