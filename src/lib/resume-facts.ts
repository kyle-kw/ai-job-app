import type { ResumeFact, ResumeFactCategory, ResumeProfile, ResumeTemplateId } from '$lib/types';
import { displayDegree, formatDateRange } from '$lib/resume-format';

export const RESUME_FACT_CATEGORIES: ReadonlyArray<{ value: ResumeFactCategory; label: string }> = [
  { value: 'identity', label: '基本信息' },
  { value: 'experience', label: '工作经历' },
  { value: 'education', label: '教育经历' },
  { value: 'skill', label: '专业技能' },
  { value: 'project', label: '项目经历' },
  { value: 'certification', label: '证书资质' },
  { value: 'other', label: '其他' }
];

const TEMPLATE_GUIDANCE: Record<ResumeTemplateId, { title: string; examples: string[] }> = {
  'ai-engineering': {
    title: '优先确认可验证的工程能力与交付结果',
    examples: ['技术栈与实际使用场景', '系统规模、性能或质量指标', '本人承担的职责与上线结果']
  },
  'data-analysis': {
    title: '优先确认分析方法与业务结果之间的证据链',
    examples: [
      '指标口径、数据规模与数据来源',
      'SQL、Python、BI 工具及实际场景',
      '分析方法、业务动作与量化结果'
    ]
  },
  'finance-accounting': {
    title: '优先确认核算范围、合规责任与流程结果',
    examples: [
      '核算主体、月结时效与对账范围',
      '税务申报、预算差异与审计配合',
      '金蝶、用友、Excel 等系统的实际使用'
    ]
  },
  general: {
    title: '优先确认与目标岗位直接相关的事实',
    examples: [
      '任职公司、岗位与时间',
      '真实使用过的工具和专业能力',
      '可以追溯的项目、证书与量化成果'
    ]
  }
};

export function resumeFactCategoryLabel(category: ResumeFactCategory): string {
  return RESUME_FACT_CATEGORIES.find((item) => item.value === category)?.label ?? '其他';
}

export function resumeFactGuidance(templateId: ResumeTemplateId) {
  return TEMPLATE_GUIDANCE[templateId] ?? TEMPLATE_GUIDANCE.general;
}

export function normalizeResumeFactValue(value: string): string {
  return value.trim().replace(/\s+/g, ' ').toLocaleLowerCase();
}

function factKey(fact: Pick<ResumeFact, 'category' | 'value'>): string {
  return `${fact.category}\u0000${normalizeResumeFactValue(fact.value)}`;
}

function generatedFact(
  category: ResumeFactCategory,
  value: string,
  source: string,
  idFactory: () => string
): ResumeFact | null {
  const normalized = value.trim().replace(/\s+/g, ' ');
  if (!normalized) return null;
  return { id: idFactory(), category, value: normalized, source, confidence: 1, confirmed: false };
}

export function factsFromResumeContent(
  resume: Pick<
    ResumeProfile,
    'professionalSkills' | 'experiences' | 'education' | 'projects' | 'certifications'
  >,
  idFactory: () => string = () => crypto.randomUUID()
): ResumeFact[] {
  const candidates: ResumeFact[] = [];
  const add = (fact: ResumeFact | null) => {
    if (fact) candidates.push(fact);
  };

  for (const group of resume.professionalSkills) {
    for (const skill of group.items) {
      add(
        generatedFact(
          'skill',
          skill,
          `当前主简历 · 专业技能 · ${group.label || '未分组'}`,
          idFactory
        )
      );
    }
  }

  for (const experience of resume.experiences) {
    const role = [experience.company.trim(), experience.position.trim()]
      .filter(Boolean)
      .join(' · ');
    const dates = formatDateRange(experience.startDate, experience.endDate);
    const employment = [role, dates ? `（${dates}）` : ''].join('');
    const source = `当前主简历 · 工作经历 · ${experience.company || experience.position || '未命名经历'}`;
    add(generatedFact('experience', employment, source, idFactory));
    for (const highlight of experience.highlights)
      add(generatedFact('experience', highlight, source, idFactory));
  }

  for (const project of resume.projects) {
    const source = `当前主简历 · 项目经历 · ${project.name || '未命名项目'}`;
    add(generatedFact('project', project.summary || project.name, source, idFactory));
    for (const highlight of project.highlights)
      add(generatedFact('project', highlight, source, idFactory));
  }

  for (const education of resume.education) {
    const dates = formatDateRange(education.startDate, education.endDate);
    const value = [education.institution, education.area, displayDegree(education), dates]
      .map((item) => item.trim())
      .filter(Boolean)
      .join(' · ');
    add(
      generatedFact(
        'education',
        value,
        `当前主简历 · 教育经历 · ${education.institution || '未命名教育经历'}`,
        idFactory
      )
    );
  }

  for (const certification of resume.certifications) {
    const value = [certification.name, certification.issuer, certification.date]
      .map((item) => item.trim())
      .filter(Boolean)
      .join(' · ');
    add(
      generatedFact(
        'certification',
        value,
        `当前主简历 · 证书资质 · ${certification.name || '未命名证书'}`,
        idFactory
      )
    );
  }

  const seen = new Set<string>();
  return candidates.filter((fact) => {
    const key = factKey(fact);
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}

export function mergeResumeFacts(
  existing: readonly ResumeFact[],
  candidates: readonly ResumeFact[]
) {
  const seen = new Set(existing.map(factKey));
  const additions = candidates.filter((fact) => {
    const key = factKey(fact);
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
  return { facts: [...existing, ...additions], added: additions.length };
}
