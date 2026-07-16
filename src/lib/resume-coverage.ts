import type { Job, ResumeCoverageItem, ResumeCoverageReport, ResumeProfile, ResumeTargetRef } from '$lib/types';

const MAX_DESCRIPTION_REQUIREMENTS = 20;
const FNV_64_OFFSET = 0xcbf29ce484222325n;
const FNV_64_PRIME = 0x100000001b3n;

function normalize(value: string): string {
  return value.trim().toLowerCase().replace(/[\s，,。；;：:、·|/\\()[\]（）【】_-]+/g, '');
}

function exactMatcher(label: string): (text: string) => boolean {
  const needle = label.trim();
  if (!needle) return () => false;
  if (/^[\x00-\x7F]+$/.test(needle) && /[a-z0-9]/i.test(needle)) {
    const escaped = needle.replace(/[.*+?^${}()|[\]\\]/g, '\\$&').replace(/\s+/g, '\\s+');
    const pattern = new RegExp(`(^|[^a-z0-9])${escaped}([^a-z0-9]|$)`, 'i');
    return (text) => pattern.test(text);
  }
  const normalizedNeedle = normalize(needle);
  return (text) => normalize(text).includes(normalizedNeedle);
}

function coverageRequirementId(normalized: string): string {
  let hash = FNV_64_OFFSET;
  for (const byte of new TextEncoder().encode(normalized)) {
    hash ^= BigInt(byte);
    hash = BigInt.asUintN(64, hash * FNV_64_PRIME);
  }
  return `requirement-${hash.toString(16).padStart(16, '0')}`;
}

function resumeSearchFields(resume: ResumeProfile): Array<{ path: string; text: string }> {
  return [
    { path: '/headline', text: resume.headline }, { path: '/summary', text: resume.summary },
    ...resume.professionalSkills.map((group, index) => ({ path: `/professionalSkills/${index}`, text: `${group.label} ${group.items.join(' ')}` })),
    ...resume.experiences.map((item, index) => ({ path: `/experiences/${index}`, text: `${item.company} ${item.position} ${item.highlights.join(' ')}` })),
    ...resume.projects.map((item, index) => ({ path: `/projects/${index}`, text: `${item.name} ${item.summary} ${item.highlights.join(' ')}` })),
    ...resume.education.map((item, index) => ({ path: `/education/${index}`, text: `${item.institution} ${item.area} ${item.degree} ${item.degreeDetail} ${item.highlights.join(' ')}` })),
    ...resume.certifications.map((item, index) => ({ path: `/certifications/${index}`, text: `${item.name} ${item.issuer}` }))
  ];
}

export function coverageRequirements(job: Job): Array<{ id: string; label: string; kind: ResumeCoverageItem['kind'] }> {
  const values: Array<{ label: string; kind: ResumeCoverageItem['kind'] }> = [];
  for (const requirement of job.structuredDetails?.requirements ?? []) values.push({ label: requirement, kind: 'requirement' });
  for (const skill of job.skills) values.push({ label: skill, kind: 'skill' });
  if (!job.structuredDetails?.requirements?.length) {
    for (const sentence of job.description
      .split(/[。；;\n]/)
      .map((item) => item.trim())
      .filter((item) => Array.from(item).length >= 6 && Array.from(item).length <= 140)
      .slice(0, MAX_DESCRIPTION_REQUIREMENTS)) {
      values.push({ label: sentence, kind: 'requirement' });
    }
  }
  const seen = new Set<string>();
  return values.flatMap((item) => {
    const key = normalize(item.label);
    if (!key || seen.has(key)) return [];
    seen.add(key);
    return [{ ...item, id: coverageRequirementId(key) }];
  });
}

export function buildLocalResumeCoverage(
  job: Job,
  target: ResumeTargetRef,
  resume: ResumeProfile
): ResumeCoverageReport {
  const fields = resumeSearchFields(resume);
  const facts = resume.facts.filter((fact) => fact.confirmed).map((fact) => ({ id: fact.id, text: fact.value }));
  const items = coverageRequirements(job).map((source): ResumeCoverageItem => {
    const matches = exactMatcher(source.label);
    const resumePaths = fields.filter((field) => matches(field.text)).map((field) => field.path);
    const evidenceFactIds = facts.filter((fact) => matches(fact.text)).map((fact) => fact.id);
    const status = resumePaths.length ? 'covered'
      : evidenceFactIds.length ? 'strengthenable'
        : source.kind === 'skill' ? 'gap' : 'unknown';
    const rationale = status === 'covered' ? '简历正文存在精确匹配。'
      : status === 'strengthenable' ? '已确认事实中存在证据，但简历正文尚未表达。'
        : status === 'gap' ? '简历和已确认事实均未发现此技能。'
          : '长句要求需要主动运行 AI 语义分析。';
    return { id: source.id, label: source.label, kind: source.kind, status, resumePaths, evidenceFactIds, rationale };
  });
  return summarizeCoverage({ jobId: job.id, target, targetVersion: resume.version, source: 'local', generatedAt: new Date().toISOString(), items });
}

export function summarizeCoverage(report: Omit<ResumeCoverageReport, 'coveredCount' | 'strengthenableCount' | 'gapCount' | 'unknownCount'>): ResumeCoverageReport {
  return {
    ...report,
    coveredCount: report.items.filter((item) => item.status === 'covered').length,
    strengthenableCount: report.items.filter((item) => item.status === 'strengthenable').length,
    gapCount: report.items.filter((item) => item.status === 'gap').length,
    unknownCount: report.items.filter((item) => item.status === 'unknown').length
  };
}

export function coverageHighlightKeywords(report: ResumeCoverageReport | null): string[] {
  if (!report) return [];
  return report.items
    .filter((item) => item.kind === 'skill' && (item.status === 'covered' || item.status === 'strengthenable'))
    .map((item) => item.label);
}
