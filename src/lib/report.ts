import type {
  Job,
  JobDataReport,
  ReportBatchComparison,
  ReportBatchSkillChange,
  ReportBucket,
  ReportKeyword,
  ReportSampleQuality,
  SalaryByExperience,
  ScrapeRun,
  ScrapeSampleSummary,
  SearchSpec
} from '$lib/types';

const fallback = (value: string) => value.trim() || '未注明';
const cityOf = (value: string) => value.split('·')[0]?.trim() || '未注明';
const percentage = (count: number, total: number) =>
  total ? Math.round((count / total) * 1000) / 10 : 0;

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

export function parseMonthlySalary(
  value: string
): { low: number; mid: number; high: number; months?: number } | null {
  const upper = value.toUpperCase();
  const kIndex = upper.indexOf('K');
  if (kIndex < 0) return null;
  const range = numbersIn(upper.slice(0, kIndex));
  if (range.length < 2 || range[0] <= 0 || range[1] < range[0]) return null;
  const months = numbersIn(upper.slice(kIndex + 1))[0];
  return {
    low: range[0],
    mid: (range[0] + range[1]) / 2,
    high: range[1],
    months: months >= 12 && months <= 24 ? months : undefined
  };
}

function median(values: number[]): number | null {
  if (!values.length) return null;
  const sorted = [...values].sort((a, b) => a - b);
  const middle = Math.floor(sorted.length / 2);
  return sorted.length % 2 ? sorted[middle] : (sorted[middle - 1] + sorted[middle]) / 2;
}

const shanghaiDateFormatter = new Intl.DateTimeFormat('en-CA', {
  timeZone: 'Asia/Shanghai',
  year: 'numeric',
  month: '2-digit',
  day: '2-digit'
});

function shanghaiDateKey(value: string | Date): string | null {
  const date = value instanceof Date ? value : new Date(value);
  if (Number.isNaN(date.getTime())) return null;
  const parts = Object.fromEntries(
    shanghaiDateFormatter.formatToParts(date).map((part) => [part.type, part.value])
  );
  return `${parts.year}-${parts.month}-${parts.day}`;
}

const rounded = (value: number) => Math.round(value * 10) / 10;

const normalizeKeyword = (value: string) =>
  value.trim().split(/\s+/).filter(Boolean).join(' ').toLocaleLowerCase();
const normalizeOptional = (value?: string) => value?.trim().toLocaleLowerCase() ?? '';

function sameSearchScope(left: SearchSpec, right: SearchSpec): boolean {
  return (
    normalizeKeyword(left.keyword) === normalizeKeyword(right.keyword) &&
    left.city.trim() === right.city.trim() &&
    left.pages === right.pages &&
    normalizeOptional(left.salary) === normalizeOptional(right.salary) &&
    normalizeOptional(left.experience) === normalizeOptional(right.experience) &&
    normalizeOptional(left.degree) === normalizeOptional(right.degree) &&
    normalizeOptional(left.companyScale) === normalizeOptional(right.companyScale)
  );
}

export function buildScrapeSampleSummary(jobs: Job[]): ScrapeSampleSummary {
  jobs = [...new Map(jobs.map((job) => [job.id, job])).values()];
  const totalJobs = jobs.length;
  const detailJobs = jobs.filter((job) => job.description.trim()).length;
  const salaryValues = jobs.flatMap((job) => {
    const parsed = parseMonthlySalary(job.salary);
    return parsed ? [parsed.mid] : [];
  });
  const skillCounter = new Map<string, number>();
  let skillSampleCount = 0;
  for (const job of jobs) {
    const skills = [...new Set(job.skills.map((skill) => skill.trim()).filter(Boolean))];
    if (skills.length) skillSampleCount += 1;
    skills.forEach((skill) => increment(skillCounter, skill));
  }
  return {
    jobIds: [...new Set(jobs.map((job) => job.id))].sort(),
    totalJobs,
    detailJobs,
    detailCoverage: percentage(detailJobs, totalJobs),
    salarySampleCount: salaryValues.length,
    medianSalaryK: median(salaryValues),
    skillSampleCount,
    skillCoverage: percentage(skillSampleCount, totalJobs),
    skills: buckets(skillCounter, totalJobs, Number.MAX_SAFE_INTEGER)
  };
}

const unavailableComparison = (reason: ReportBatchComparison['reason']): ReportBatchComparison => ({
  status: 'unavailable',
  reason,
  current: null,
  previous: null,
  jobCountChangePercentage: null,
  newlyObservedJobs: 0,
  notObservedJobs: 0,
  salaryMedianDeltaK: null,
  skillChanges: []
});

function buildBatchComparison(
  selectedKeywords: ReportKeyword[],
  runs: ScrapeRun[]
): ReportBatchComparison {
  if (selectedKeywords.length !== 1) return unavailableComparison('multi_keyword');
  const selectedKeys = new Set([
    normalizeKeyword(selectedKeywords[0].key),
    normalizeKeyword(selectedKeywords[0].label)
  ]);
  const candidates = runs
    .filter(
      (run) =>
        run.completedAt &&
        run.searchSpec &&
        run.sample &&
        selectedKeys.has(normalizeKeyword(run.keyword))
    )
    .sort((left, right) => Date.parse(right.completedAt!) - Date.parse(left.completedAt!));
  const current = candidates[0];
  if (!current?.searchSpec || !current.sample || !current.completedAt)
    return unavailableComparison('no_captured_run');
  const currentDay = shanghaiDateKey(current.completedAt);
  const previous = candidates
    .slice(1)
    .find(
      (run) =>
        run.searchSpec &&
        run.sample &&
        run.completedAt &&
        sameSearchScope(current.searchSpec!, run.searchSpec) &&
        shanghaiDateKey(run.completedAt) !== currentDay
    );
  if (!previous?.searchSpec || !previous.sample || !previous.completedAt)
    return unavailableComparison('no_comparable_run');

  const currentIds = new Set(current.sample.jobIds);
  const previousIds = new Set(previous.sample.jobIds);
  const currentSkills = new Map(current.sample.skills.map((item) => [item.label, item]));
  const previousSkills = new Map(previous.sample.skills.map((item) => [item.label, item]));
  const skillChanges: ReportBatchSkillChange[] = [
    ...new Set([...currentSkills.keys(), ...previousSkills.keys()])
  ]
    .map((label) => {
      const currentItem = currentSkills.get(label);
      const previousItem = previousSkills.get(label);
      return {
        label,
        currentCount: currentItem?.count ?? 0,
        currentPercentage: currentItem?.percentage ?? 0,
        previousCount: previousItem?.count ?? 0,
        previousPercentage: previousItem?.percentage ?? 0,
        deltaPercentagePoints: rounded(
          (currentItem?.percentage ?? 0) - (previousItem?.percentage ?? 0)
        )
      };
    })
    .sort(
      (left, right) =>
        Math.abs(right.deltaPercentagePoints) - Math.abs(left.deltaPercentagePoints) ||
        right.currentCount - left.currentCount ||
        left.label.localeCompare(right.label, 'zh-CN')
    )
    .slice(0, 8);
  const snapshot = (run: ScrapeRun) => ({
    runId: run.id,
    completedAt: run.completedAt!,
    searchSpec: run.searchSpec!,
    totalJobs: run.sample!.totalJobs,
    detailCoverage: run.sample!.detailCoverage,
    salarySampleCount: run.sample!.salarySampleCount,
    medianSalaryK: run.sample!.medianSalaryK
  });
  return {
    status: 'available',
    reason: null,
    current: snapshot(current),
    previous: snapshot(previous),
    jobCountChangePercentage: previous.sample.totalJobs
      ? rounded(
          ((current.sample.totalJobs - previous.sample.totalJobs) / previous.sample.totalJobs) * 100
        )
      : null,
    newlyObservedJobs: [...currentIds].filter((id) => !previousIds.has(id)).length,
    notObservedJobs: [...previousIds].filter((id) => !currentIds.has(id)).length,
    salaryMedianDeltaK:
      current.sample.medianSalaryK != null && previous.sample.medianSalaryK != null
        ? rounded(current.sample.medianSalaryK - previous.sample.medianSalaryK)
        : null,
    skillChanges
  };
}

export function classifyJobRole(title: string): string {
  const value = title.toLocaleLowerCase();
  if (/架构|专家|负责人|lead/.test(value)) return '架构 / 专家';
  if (/产品|product/.test(value)) return '产品';
  if (/测试|质量|qa/.test(value)) return '测试 / 质量';
  if (/全栈|前端|frontend|full.?stack/.test(value)) return '前端 / 全栈';
  if (/agent|智能体|大模型|llm|rag|人工智能/.test(value) || /(^|[^a-z])ai([^a-z]|$)/i.test(value))
    return 'AI / Agent 开发';
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

export function buildClientJobDataReport(
  jobs: Job[],
  selectedKeywords: ReportKeyword[] = [],
  scrapeRuns: ScrapeRun[] = [],
  now = new Date()
): JobDataReport {
  const total = jobs.length;
  const companies = new Set<string>();
  const counters = {
    experience: new Map<string, number>(),
    degree: new Map<string, number>(),
    roles: new Map<string, number>(),
    cities: new Map<string, number>(),
    industries: new Map<string, number>(),
    scales: new Map<string, number>(),
    skills: new Map<string, number>(),
    pairs: new Map<string, number>(),
    welfare: new Map<string, number>(),
    bands: new Map<string, number>()
  };
  const lows: number[] = [],
    mids: number[] = [],
    highs: number[] = [];
  const salaryByExperience = new Map<string, number[]>();
  let extraMonthsCount = 0;
  let detailJobs = 0;
  let skillJobs = 0;
  let experienceJobs = 0;
  let degreeJobs = 0;

  for (const job of jobs) {
    if (job.company.trim()) companies.add(job.company.trim());
    if (job.experience.trim()) experienceJobs += 1;
    if (job.degree.trim()) degreeJobs += 1;
    increment(counters.experience, fallback(job.experience));
    increment(counters.degree, fallback(job.degree));
    increment(counters.roles, classifyJobRole(job.title));
    increment(counters.cities, cityOf(job.location));
    increment(counters.industries, fallback(job.industry));
    increment(counters.scales, fallback(job.companyScale));
    if (job.description.trim()) detailJobs += 1;
    job.welfare.forEach((item) => increment(counters.welfare, item));
    const skills = [...new Set(job.skills.filter(Boolean))].sort();
    if (skills.length) skillJobs += 1;
    skills.forEach((skill) => increment(counters.skills, skill));
    for (let left = 0; left < skills.length; left += 1) {
      for (let right = left + 1; right < skills.length; right += 1)
        increment(counters.pairs, `${skills[left]} × ${skills[right]}`);
    }
    const salary = parseMonthlySalary(job.salary);
    if (salary) {
      lows.push(salary.low);
      mids.push(salary.mid);
      highs.push(salary.high);
      if ((salary.months ?? 12) > 12) extraMonthsCount += 1;
      increment(counters.bands, salaryBand(salary.mid));
      const experience = fallback(job.experience);
      salaryByExperience.set(experience, [
        ...(salaryByExperience.get(experience) ?? []),
        salary.mid
      ]);
    }
  }

  const topSkills = buckets(counters.skills, total, 18);
  const experience = buckets(counters.experience, total);
  const salaryByExperienceRows: SalaryByExperience[] = [...salaryByExperience]
    .map(([label, values]) => ({ label, count: values.length, medianK: median(values) ?? 0 }))
    .sort((a, b) => b.count - a.count);
  const insights = total
    ? [
        `当前 ${total} 个本地去重岗位样本覆盖 ${companies.size} 家公司、${counters.cities.size} 个城市。`,
        mids.length
          ? `当前样本中可解析薪资的岗位有 ${mids.length} 个，月薪区间中点中位数为 ${(median(mids) ?? 0).toFixed(1)}K。`
          : '当前样本暂缺可解析的月薪区间。',
        topSkills.length
          ? `当前样本中反复出现的技能包括 ${topSkills
              .slice(0, 5)
              .map((item) => `${item.label}（${item.percentage.toFixed(1)}%）`)
              .join('、')}。`
          : '当前样本暂缺结构化技能信息。',
        experience[0]
          ? `当前样本的经验要求以“${experience[0].label}”为主，占全部岗位的 ${experience[0].percentage.toFixed(1)}%。`
          : '当前样本暂缺经验要求。'
      ]
    : ['岗位库暂无数据，完成至少一轮抓取后即可生成全量报告。'];
  const dates = jobs
    .flatMap((job) => [job.firstSeen, job.lastSeen])
    .filter(Boolean)
    .sort();
  const metric = (count: number) => ({ count, coverage: percentage(count, total) });
  const limitations = ['本报告仅反映本机保存的有限页 BOSS 岗位样本，不代表完整招聘市场。'];
  if (total < 20) limitations.push('当前少于 20 个岗位，比例和排序仅适合作为方向提示。');
  for (const [count, threshold, message] of [
    [detailJobs, 60, '岗位详情覆盖不足 60%，职责和要求统计可能不完整。'],
    [mids.length, 50, '可解析薪资覆盖不足 50%，薪资统计可能偏离当前样本。'],
    [skillJobs, 60, '技能信息覆盖不足 60%，高频技能排序可能受缺失字段影响。'],
    [experienceJobs, 60, '经验要求覆盖不足 60%，经验分布仅供参考。'],
    [degreeJobs, 60, '学历要求覆盖不足 60%，学历分布仅供参考。']
  ] as const)
    if (percentage(count, total) < threshold) limitations.push(message);
  const sampleQuality: ReportSampleQuality = {
    detail: metric(detailJobs),
    salary: metric(mids.length),
    skill: metric(skillJobs),
    experience: metric(experienceJobs),
    degree: metric(degreeJobs),
    limitations
  };

  return {
    generatedAt: now.toISOString(),
    selectedKeywords,
    dataFrom: dates[0]?.slice(0, 10),
    dataTo: dates.at(-1)?.slice(0, 10),
    totalJobs: total,
    totalCompanies: companies.size,
    totalCities: counters.cities.size,
    detailJobs,
    detailCoverage: percentage(detailJobs, total),
    salary: {
      sampleCount: mids.length,
      medianLowK: median(lows),
      medianMidK: median(mids),
      medianHighK: median(highs),
      extraMonthsCount,
      bands: buckets(counters.bands, mids.length)
    },
    experience,
    degree: buckets(counters.degree, total),
    roles: buckets(counters.roles, total),
    cities: buckets(counters.cities, total),
    industries: buckets(counters.industries, total),
    companyScales: buckets(counters.scales, total),
    topSkills,
    skillPairs: buckets(counters.pairs, total, 10),
    welfare: buckets(counters.welfare, total),
    salaryByExperience: salaryByExperienceRows,
    insights,
    sampleQuality,
    batchComparison: buildBatchComparison(selectedKeywords, scrapeRuns)
  };
}
