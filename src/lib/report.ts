import type { Job, JobDataReport, ReportBucket, ReportKeyword, ReportSkillChange, ReportTrendWindow, SalaryByExperience } from '$lib/types';

const fallback = (value: string) => value.trim() || '未注明';
const cityOf = (value: string) => value.split('·')[0]?.trim() || '未注明';
const percentage = (count: number, total: number) => total ? Math.round(count / total * 1000) / 10 : 0;

function increment(counter: Map<string, number>, label: string) {
  if (label.trim()) counter.set(label, (counter.get(label) ?? 0) + 1);
}

function buckets(counter: Map<string, number>, total: number, limit = 12): ReportBucket[] {
  return [...counter]
    .map(([label, count]) => ({ label, count, percentage: percentage(count, total) }))
    .sort((a, b) => b.count - a.count || a.label.localeCompare(b.label, 'zh-CN'))
    .slice(0, limit);
}

function numbersIn(value: string): number[] {
  return value.match(/\d+(?:\.\d+)?/g)?.map(Number) ?? [];
}

export function parseMonthlySalary(value: string): { low: number; mid: number; high: number; months?: number } | null {
  const upper = value.toUpperCase();
  const kIndex = upper.indexOf('K');
  if (kIndex < 0) return null;
  const range = numbersIn(upper.slice(0, kIndex));
  if (range.length < 2 || range[0] <= 0 || range[1] < range[0]) return null;
  const months = numbersIn(upper.slice(kIndex + 1))[0];
  return { low: range[0], mid: (range[0] + range[1]) / 2, high: range[1], months: months >= 12 && months <= 24 ? months : undefined };
}

function median(values: number[]): number | null {
  if (!values.length) return null;
  const sorted = [...values].sort((a, b) => a - b);
  const middle = Math.floor(sorted.length / 2);
  return sorted.length % 2 ? sorted[middle] : (sorted[middle - 1] + sorted[middle]) / 2;
}

const shanghaiDateFormatter = new Intl.DateTimeFormat('en-CA', {
  timeZone: 'Asia/Shanghai', year: 'numeric', month: '2-digit', day: '2-digit'
});

function shanghaiDateKey(value: string | Date): string | null {
  const date = value instanceof Date ? value : new Date(value);
  if (Number.isNaN(date.getTime())) return null;
  const parts = Object.fromEntries(shanghaiDateFormatter.formatToParts(date).map((part) => [part.type, part.value]));
  return `${parts.year}-${parts.month}-${parts.day}`;
}

function dateOrdinal(value: string | Date): number | null {
  const key = shanghaiDateKey(value);
  if (!key) return null;
  const [year, month, day] = key.split('-').map(Number);
  return Math.floor(Date.UTC(year, month - 1, day) / 86_400_000);
}

const ordinalDate = (ordinal: number) => new Date(ordinal * 86_400_000).toISOString().slice(0, 10);
const rounded = (value: number) => Math.round(value * 10) / 10;

function trendWindow(jobs: Job[], generatedAt: Date, windowDays: 7 | 30): ReportTrendWindow {
  const today = dateOrdinal(generatedAt) ?? Math.floor(generatedAt.getTime() / 86_400_000);
  const recentStart = today - windowDays + 1;
  const previousStart = recentStart - windowDays;
  const recentJobs: Job[] = [];
  const previousJobs: Job[] = [];
  const dailyCounts = new Map<number, number>();
  let dateSampleCount = 0;
  let recentlySeenExistingJobs = 0;

  for (const job of jobs) {
    const first = dateOrdinal(job.firstSeen);
    const last = dateOrdinal(job.lastSeen);
    if (first != null) {
      dateSampleCount += 1;
      if (first >= recentStart && first <= today) {
        recentJobs.push(job);
        dailyCounts.set(first, (dailyCounts.get(first) ?? 0) + 1);
      } else if (first >= previousStart && first < recentStart) {
        previousJobs.push(job);
      }
    }
    if (first != null && first < recentStart && last != null && last >= recentStart && last <= today) {
      recentlySeenExistingJobs += 1;
    }
  }

  const skillCounts = (items: Job[]) => {
    const counter = new Map<string, number>();
    for (const job of items) {
      for (const skill of new Set(job.skills.map((item) => item.trim()).filter(Boolean))) increment(counter, skill);
    }
    return counter;
  };
  const recentSkills = skillCounts(recentJobs);
  const previousSkills = skillCounts(previousJobs);
  const skillChanges: ReportSkillChange[] = [...new Set([...recentSkills.keys(), ...previousSkills.keys()])]
    .map((label) => {
      const recentCount = recentSkills.get(label) ?? 0;
      const previousCount = previousSkills.get(label) ?? 0;
      const recentPercentage = percentage(recentCount, recentJobs.length);
      const previousPercentage = percentage(previousCount, previousJobs.length);
      return {
        label, recentCount, recentPercentage, previousCount, previousPercentage,
        deltaPercentagePoints: rounded(recentPercentage - previousPercentage)
      };
    })
    .sort((left, right) => Math.abs(right.deltaPercentagePoints) - Math.abs(left.deltaPercentagePoints)
      || right.recentCount - left.recentCount || left.label.localeCompare(right.label, 'zh-CN'))
    .slice(0, 8);
  const salaryMids = (items: Job[]) => items.flatMap((job) => {
    const parsed = parseMonthlySalary(job.salary);
    return parsed ? [parsed.mid] : [];
  });
  const recentSalaryMedianK = median(salaryMids(recentJobs));
  const previousSalaryMedianK = median(salaryMids(previousJobs));

  return {
    windowDays,
    recentNewJobs: recentJobs.length,
    previousNewJobs: previousJobs.length,
    newJobsChangePercentage: previousJobs.length ? rounded((recentJobs.length - previousJobs.length) / previousJobs.length * 100) : null,
    recentlySeenExistingJobs,
    recentSalaryMedianK,
    previousSalaryMedianK,
    salaryMedianDeltaK: recentSalaryMedianK != null && previousSalaryMedianK != null ? rounded(recentSalaryMedianK - previousSalaryMedianK) : null,
    dateSampleCount,
    dateCoverage: percentage(dateSampleCount, jobs.length),
    dailyNewJobs: Array.from({ length: windowDays }, (_, index) => {
      const ordinal = recentStart + index;
      return { date: ordinalDate(ordinal), count: dailyCounts.get(ordinal) ?? 0 };
    }),
    skillChanges
  };
}

export function classifyJobRole(title: string): string {
  const value = title.toLocaleLowerCase();
  if (/架构|专家|负责人|lead/.test(value)) return '架构 / 专家';
  if (/产品|product/.test(value)) return '产品';
  if (/测试|质量|qa/.test(value)) return '测试 / 质量';
  if (/全栈|前端|frontend|full.?stack/.test(value)) return '前端 / 全栈';
  if (/agent|智能体|大模型|llm|rag|人工智能/.test(value) || /(^|[^a-z])ai([^a-z]|$)/i.test(value)) return 'AI / Agent 开发';
  if (/算法|数据科学|机器学习|nlp|数据分析/.test(value)) return '算法 / 数据';
  if (/后端|java|golang|rust|服务端/.test(value)) return '后端开发';
  return '其他岗位';
}

function salaryBand(value: number): string {
  if (value < 15) return '15K 以下';
  if (value < 25) return '15–25K';
  if (value < 35) return '25–35K';
  if (value < 50) return '35–50K';
  return '50K 以上';
}

export function buildClientJobDataReport(jobs: Job[], selectedKeywords: ReportKeyword[] = [], now = new Date()): JobDataReport {
  const total = jobs.length;
  const companies = new Set<string>();
  const counters = {
    experience: new Map<string, number>(), degree: new Map<string, number>(), roles: new Map<string, number>(),
    cities: new Map<string, number>(), industries: new Map<string, number>(), scales: new Map<string, number>(),
    skills: new Map<string, number>(), pairs: new Map<string, number>(), welfare: new Map<string, number>(), bands: new Map<string, number>()
  };
  const lows: number[] = [], mids: number[] = [], highs: number[] = [];
  const salaryByExperience = new Map<string, number[]>();
  let extraMonthsCount = 0;
  let detailJobs = 0;

  for (const job of jobs) {
    if (job.company.trim()) companies.add(job.company.trim());
    increment(counters.experience, fallback(job.experience));
    increment(counters.degree, fallback(job.degree));
    increment(counters.roles, classifyJobRole(job.title));
    increment(counters.cities, cityOf(job.location));
    increment(counters.industries, fallback(job.industry));
    increment(counters.scales, fallback(job.companyScale));
    if (job.description.trim()) detailJobs += 1;
    job.welfare.forEach((item) => increment(counters.welfare, item));
    const skills = [...new Set(job.skills.filter(Boolean))].sort();
    skills.forEach((skill) => increment(counters.skills, skill));
    for (let left = 0; left < skills.length; left += 1) {
      for (let right = left + 1; right < skills.length; right += 1) increment(counters.pairs, `${skills[left]} × ${skills[right]}`);
    }
    const salary = parseMonthlySalary(job.salary);
    if (salary) {
      lows.push(salary.low); mids.push(salary.mid); highs.push(salary.high);
      if ((salary.months ?? 12) > 12) extraMonthsCount += 1;
      increment(counters.bands, salaryBand(salary.mid));
      const experience = fallback(job.experience);
      salaryByExperience.set(experience, [...(salaryByExperience.get(experience) ?? []), salary.mid]);
    }
  }

  const topSkills = buckets(counters.skills, total, 18);
  const experience = buckets(counters.experience, total);
  const salaryByExperienceRows: SalaryByExperience[] = [...salaryByExperience]
    .map(([label, values]) => ({ label, count: values.length, medianK: median(values) ?? 0 }))
    .sort((a, b) => b.count - a.count);
  const insights = total ? [
    `当前岗位库按岗位去重后共有 ${total} 个岗位，覆盖 ${companies.size} 家公司、${counters.cities.size} 个城市。`,
    mids.length ? `可解析薪资的岗位有 ${mids.length} 个，月薪区间中点中位数为 ${(median(mids) ?? 0).toFixed(1)}K。` : '当前岗位暂缺可解析的月薪区间。',
    topSkills.length ? `最常出现的技能是 ${topSkills.slice(0, 5).map((item) => `${item.label}（${item.percentage.toFixed(1)}%）`).join('、')}。` : '当前岗位暂缺结构化技能信息。',
    experience[0] ? `经验要求以“${experience[0].label}”为主，占全部岗位的 ${experience[0].percentage.toFixed(1)}%。` : '当前岗位暂缺经验要求。'
  ] : ['岗位库暂无数据，完成至少一轮抓取后即可生成全量报告。'];
  const dates = jobs.flatMap((job) => [job.firstSeen, job.lastSeen]).filter(Boolean).sort();

  return {
    generatedAt: now.toISOString(), selectedKeywords, dataFrom: dates[0]?.slice(0, 10), dataTo: dates.at(-1)?.slice(0, 10),
    totalJobs: total, totalCompanies: companies.size, totalCities: counters.cities.size, detailJobs,
    detailCoverage: percentage(detailJobs, total),
    salary: { sampleCount: mids.length, medianLowK: median(lows), medianMidK: median(mids), medianHighK: median(highs), extraMonthsCount, bands: buckets(counters.bands, mids.length) },
    experience, degree: buckets(counters.degree, total), roles: buckets(counters.roles, total), cities: buckets(counters.cities, total),
    industries: buckets(counters.industries, total), companyScales: buckets(counters.scales, total), topSkills,
    skillPairs: buckets(counters.pairs, total, 10), welfare: buckets(counters.welfare, total), salaryByExperience: salaryByExperienceRows, insights,
    trends: { sevenDays: trendWindow(jobs, now, 7), thirtyDays: trendWindow(jobs, now, 30) }
  };
}
