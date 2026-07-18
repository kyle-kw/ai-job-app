import { exactMatcher, resumeSearchFields } from '$lib/resume-coverage';
import type {
  ReportBucket,
  ReportCompetitivenessAnalysis,
  ReportCompetitivenessItem,
  ResumeProfile
} from '$lib/types';

export function buildLocalReportCompetitiveness(
  skills: ReportBucket[],
  resume: ResumeProfile,
  generatedAt = new Date().toISOString()
): ReportCompetitivenessAnalysis {
  const fields = resumeSearchFields(resume);
  const facts = resume.facts
    .filter((fact) => fact.confirmed)
    .map((fact) => ({ id: fact.id, text: fact.value }));
  const items: ReportCompetitivenessItem[] = skills.slice(0, 12).map((skill, index) => {
    const matches = exactMatcher(skill.label);
    const resumePaths = fields.filter((field) => matches(field.text)).map((field) => field.path);
    const evidenceFactIds = facts.filter((fact) => matches(fact.text)).map((fact) => fact.id);
    const status = resumePaths.length
      ? 'covered'
      : evidenceFactIds.length
        ? 'strengthenable'
        : 'gap';
    const rationale =
      status === 'covered'
        ? '主简历正文中已有明确表达。'
        : status === 'strengthenable'
          ? '已确认事实中存在证据，但主简历正文尚未明确表达。'
          : '主简历正文和已确认事实中均未找到可靠证据。';
    return {
      id: `report-skill-${index + 1}`,
      label: skill.label,
      jobCount: skill.count,
      percentage: skill.percentage,
      status,
      resumePaths,
      evidenceFactIds,
      rationale
    };
  });
  return {
    source: 'local',
    resumeId: resume.id,
    resumeVersion: resume.version,
    generatedAt,
    items
  };
}
