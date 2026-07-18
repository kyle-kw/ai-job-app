import type { FitDimension, FitReport, Job, ResumeProfile } from './types';
import { flattenProfessionalSkills } from './resume-templates';

export const FIT_WEIGHTS = {
  technical: 30,
  experience: 25,
  behavior: 15,
  career: 30
} as const;

export function normalizedOverall(dimensions: FitDimension[]): {
  score: number;
  confidence: number;
} {
  const known = dimensions.filter((dimension) => dimension.score !== null);
  const knownWeight = known.reduce((sum, dimension) => sum + dimension.weight, 0);
  if (knownWeight === 0) return { score: 0, confidence: 0 };
  const weighted = known.reduce(
    (sum, dimension) => sum + (dimension.score ?? 0) * dimension.weight,
    0
  );
  return {
    score: Math.round(weighted / knownWeight),
    confidence: Math.round(knownWeight)
  };
}

export function verdictFor(score: number): FitReport['verdict'] {
  if (score >= 75) return 'strong';
  if (score >= 60) return 'good';
  if (score >= 45) return 'moderate';
  if (score >= 30) return 'weak';
  return 'poor';
}

export function deterministicFit(job: Job, resume: ResumeProfile): FitReport {
  const resumeSkills = new Set(
    flattenProfessionalSkills(resume).map((skill) => skill.toLocaleLowerCase())
  );
  const matched = job.skills.filter((skill) => resumeSkills.has(skill.toLocaleLowerCase()));
  const technical =
    job.skills.length === 0 ? 60 : Math.round((matched.length / job.skills.length) * 70 + 25);
  const hasRelatedRole = resume.experiences.some((experience) =>
    `${experience.position} ${experience.highlights.join(' ')}`
      .toLocaleLowerCase()
      .includes(job.title.split(/[（(]/)[0].toLocaleLowerCase())
  );
  const experienceScore = hasRelatedRole ? 82 : Math.min(76, 48 + matched.length * 6);
  const targetText = resume.preferences.targetRoles.join(' ').toLocaleLowerCase();
  const careerScore = targetText
    ? targetText.split(/\s+/).some((word) => word && job.title.toLocaleLowerCase().includes(word))
      ? 88
      : 62
    : null;
  const behaviorScore = resume.preferences.energizingTasks.length > 0 ? 72 : null;
  const dimensions: FitDimension[] = [
    {
      key: 'technical',
      label: '技能匹配',
      score: Math.min(100, technical),
      weight: FIT_WEIGHTS.technical,
      note: matched.length ? `命中 ${matched.length} 项核心技能` : '暂未发现直接技能命中',
      evidence: matched
    },
    {
      key: 'experience',
      label: '经验匹配',
      score: experienceScore,
      weight: FIT_WEIGHTS.experience,
      note: hasRelatedRole ? '存在直接相关岗位经历' : '可迁移经验较多，需在材料中建立关联',
      evidence: resume.experiences.slice(0, 2).map((item) => `${item.company} · ${item.position}`)
    },
    {
      key: 'behavior',
      label: '行为与文化',
      score: behaviorScore,
      weight: FIT_WEIGHTS.behavior,
      note: behaviorScore === null ? '完善偏好后可评估' : '根据偏好与岗位任务推断',
      evidence: resume.preferences.energizingTasks
    },
    {
      key: 'career',
      label: '职业方向',
      score: careerScore,
      weight: FIT_WEIGHTS.career,
      note: careerScore === null ? '尚未设置目标岗位' : '岗位方向与目标角色有交集',
      evidence: resume.preferences.targetRoles
    }
  ];
  const normalized = normalizedOverall(dimensions);
  const cityKnown = resume.preferences.cities.length > 0;
  const cityMatched =
    !cityKnown || resume.preferences.cities.some((city) => job.location.includes(city));
  const overallScore =
    cityKnown && !cityMatched ? Math.min(normalized.score, 44) : normalized.score;
  return {
    overallScore,
    confidence: normalized.confidence,
    verdict: verdictFor(overallScore),
    recommendation:
      overallScore >= 60
        ? '建议申请，并围绕已命中的技能定制简历。'
        : '建议先核对关键缺口，再决定是否投入申请。',
    summary: matched.length
      ? `你的 ${matched.slice(0, 3).join('、')} 与岗位要求直接对应。`
      : '当前更依赖可迁移经验，需要用项目成果证明匹配度。',
    dimensions,
    hardConstraints: [
      {
        label: '工作地点',
        status: cityKnown ? (cityMatched ? 'pass' : 'fail') : 'unknown',
        note: cityKnown ? (cityMatched ? '符合地点偏好' : '不在目标城市范围') : '尚未设置目标城市'
      }
    ],
    strengths: matched.slice(0, 4).map((skill) => `${skill} 与 JD 明确匹配`),
    gaps: job.skills.filter((skill) => !resumeSkills.has(skill.toLocaleLowerCase())).slice(0, 4),
    evidence: resume.experiences.flatMap((item) => item.highlights.slice(0, 1)).slice(0, 3),
    generatedAt: new Date().toISOString(),
    skillVersion: 'job-fit@1.0.0'
  };
}
