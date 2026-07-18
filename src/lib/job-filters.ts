import type { JobSort, ReportSalaryBand } from '$lib/types';

export type SalaryFilterCode = '' | '402' | '403' | '404' | '405' | '406' | '407';
export type CompanyScaleFilterCode = '' | '301' | '302' | '303' | '304' | '305' | '306';

export interface FilterOption<T extends string> {
  value: T;
  label: string;
}

export const SALARY_FILTER_OPTIONS: FilterOption<SalaryFilterCode>[] = [
  { value: '', label: '不限' },
  { value: '402', label: '3K 以下' },
  { value: '403', label: '3–5K' },
  { value: '404', label: '5–10K' },
  { value: '405', label: '10–20K' },
  { value: '406', label: '20–50K' },
  { value: '407', label: '50K 以上' }
];

export const COMPANY_SCALE_FILTER_OPTIONS: FilterOption<CompanyScaleFilterCode>[] = [
  { value: '', label: '不限' },
  { value: '301', label: '0–20 人' },
  { value: '302', label: '20–99 人' },
  { value: '303', label: '100–499 人' },
  { value: '304', label: '500–999 人' },
  { value: '305', label: '1000–9999 人' },
  { value: '306', label: '10000 人以上' }
];

interface NumericRange {
  min: number;
  max: number;
}

const SALARY_RANGES: Record<Exclude<SalaryFilterCode, ''>, NumericRange> = {
  '402': { min: 0, max: 3 },
  '403': { min: 3, max: 5 },
  '404': { min: 5, max: 10 },
  '405': { min: 10, max: 20 },
  '406': { min: 20, max: 50 },
  '407': { min: 50, max: Number.POSITIVE_INFINITY }
};

export interface LocalJobFilters {
  query: string;
  minScore: number;
  onlyNew: boolean;
  salary: SalaryFilterCode;
  companyScale: CompanyScaleFilterCode;
  city: string;
  missingDescription: boolean;
  skills?: string[];
  experience?: string;
  salaryBand?: ReportSalaryBand;
}

export interface FilterableJob {
  title: string;
  company: string;
  skills: string[];
  salary: string;
  companyScale: string;
  location: string;
  experience?: string;
  description: string;
  isNew: boolean;
  fit?: { overallScore: number } | null;
}

export function jobCity(location: string): string {
  return location.split('·', 1)[0]?.trim() ?? '';
}

export function parseSalaryRange(value: string): NumericRange | null {
  const normalized = value.trim().replace(/,/g, '').replace(/－|—|–|~|～|至/g, '-');
  if (!normalized || /面议|保密|待定|negotiable/i.test(normalized)) return null;

  const range = normalized.match(/(\d+(?:\.\d+)?)\s*(?:K|千)?\s*-\s*(\d+(?:\.\d+)?)\s*(?:K|千)/i);
  if (range) {
    const left = Number(range[1]);
    const right = Number(range[2]);
    return { min: Math.min(left, right), max: Math.max(left, right) };
  }

  const upperBound = normalized.match(/(\d+(?:\.\d+)?)\s*(?:K|千)\s*(?:以下|以内)/i);
  if (upperBound) return { min: 0, max: Number(upperBound[1]) };

  const lowerBound = normalized.match(/(\d+(?:\.\d+)?)\s*(?:K|千)\s*(?:以上|起|\+)/i);
  if (lowerBound) return { min: Number(lowerBound[1]), max: Number.POSITIVE_INFINITY };

  const exact = normalized.match(/(\d+(?:\.\d+)?)\s*(?:K|千)(?!\s*薪)/i);
  if (exact) return { min: Number(exact[1]), max: Number(exact[1]) };

  return null;
}

export function matchesSalaryFilter(value: string, code: SalaryFilterCode): boolean {
  if (!code) return true;
  const salary = parseSalaryRange(value);
  if (!salary) return false;
  const filter = SALARY_RANGES[code];
  return salary.min <= filter.max && salary.max >= filter.min;
}

export function matchesReportSalaryBand(value: string, band: ReportSalaryBand = ''): boolean {
  if (!band) return true;
  const salary = parseSalaryRange(value);
  if (!salary) return false;
  if (!Number.isFinite(salary.max)) return band === '50-plus' && salary.min >= 50;
  const midpoint = (salary.min + salary.max) / 2;
  if (band === 'under-15') return midpoint < 15;
  if (band === '15-25') return midpoint >= 15 && midpoint < 25;
  if (band === '25-35') return midpoint >= 25 && midpoint < 35;
  if (band === '35-50') return midpoint >= 35 && midpoint < 50;
  return midpoint >= 50;
}

export function normalizeCompanyScale(value: string): CompanyScaleFilterCode {
  const normalized = value
    .trim()
    .replace(/\s+/g, '')
    .replace(/－|—|–|~|～|至/g, '-')
    .replace(/，/g, ',');
  if (!normalized || /不限|未知|未标注|面议/.test(normalized)) return '';

  if (/^(?:0|1)?-?20人(?:以下|以内)?$|^(?:少于|小于|不满)20人$|^20人以下$/.test(normalized)) return '301';
  if (/^20-99人$|^20-100人$/.test(normalized)) return '302';
  if (/^100-499人$|^100-500人$/.test(normalized)) return '303';
  if (/^500-999人$|^500-1000人$/.test(normalized)) return '304';
  if (/^1000-9999人$|^1000-10000人$/.test(normalized)) return '305';
  if (/^(?:10000人|1万人|万人)(?:以上|及以上|起)?$|^10000\+人?$/.test(normalized)) return '306';

  return '';
}

export function matchesCompanyScaleFilter(value: string, code: CompanyScaleFilterCode): boolean {
  return !code || normalizeCompanyScale(value) === code;
}

export function filterJobs<T extends FilterableJob>(jobs: T[], filters: LocalJobFilters): T[] {
  const query = filters.query.trim().toLocaleLowerCase();
  const requiredSkills = (filters.skills ?? []).map((skill) => skill.trim().toLocaleLowerCase()).filter(Boolean);
  return jobs.filter((job) => {
    const searchable = `${job.title} ${job.company} ${job.skills.join(' ')}`.toLocaleLowerCase();
    const skills = new Set(job.skills.map((skill) => skill.trim().toLocaleLowerCase()));
    return (!query || searchable.includes(query))
      && (job.fit?.overallScore ?? 0) >= filters.minScore
      && (!filters.onlyNew || job.isNew)
      && matchesSalaryFilter(job.salary, filters.salary)
      && matchesCompanyScaleFilter(job.companyScale, filters.companyScale)
      && (!filters.city || jobCity(job.location) === filters.city)
      && (!filters.experience || (job.experience ?? '').trim() === filters.experience.trim())
      && matchesReportSalaryBand(job.salary, filters.salaryBand)
      && requiredSkills.every((skill) => skills.has(skill))
      && (!filters.missingDescription || !job.description.trim());
  });
}

export interface SortableJob extends FilterableJob {
  id: string;
  lastSeen: string;
}

const compareScore = (left: SortableJob, right: SortableJob) =>
  (right.fit?.overallScore ?? 0) - (left.fit?.overallScore ?? 0);

const compareLastSeen = (left: SortableJob, right: SortableJob) =>
  right.lastSeen.localeCompare(left.lastSeen);

const compareId = (left: SortableJob, right: SortableJob) => left.id.localeCompare(right.id);

export function sortJobs<T extends SortableJob>(jobs: T[], sort: JobSort = 'recommended'): T[] {
  return [...jobs].sort((left, right) => {
    if (sort === 'recent') {
      return compareLastSeen(left, right) || compareScore(left, right) || compareId(left, right);
    }
    if (sort === 'salary-desc') {
      const leftSalary = parseSalaryRange(left.salary);
      const rightSalary = parseSalaryRange(right.salary);
      if (!leftSalary && rightSalary) return 1;
      if (leftSalary && !rightSalary) return -1;
      if (leftSalary && rightSalary) {
        const leftMidpoint = Number.isFinite(leftSalary.max) ? (leftSalary.min + leftSalary.max) / 2 : leftSalary.min;
        const rightMidpoint = Number.isFinite(rightSalary.max) ? (rightSalary.min + rightSalary.max) / 2 : rightSalary.min;
        if (leftMidpoint !== rightMidpoint) return rightMidpoint - leftMidpoint;
      }
      return compareScore(left, right) || compareLastSeen(left, right) || compareId(left, right);
    }
    return compareScore(left, right) || compareLastSeen(left, right) || compareId(left, right);
  });
}
