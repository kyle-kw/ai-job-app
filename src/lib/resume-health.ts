import type { ResumeHealthIssue, ResumeHealthReport, ResumeProfile } from '$lib/types';

const severityOrder = { error: 0, warning: 1, suggestion: 2 } as const;
const numberWithUnitPattern =
  /\d+(?:\.\d+)?\s*(?:%|％|倍|个|人|天|小时|分钟|秒|万|千|元|k|m|gb|mb)/gi;

function normalize(value: string): string {
  return value
    .trim()
    .toLocaleLowerCase()
    .replace(/[\s，,。；;：:、·|/\\()[\]（）【】_-]+/g, '');
}

function issue(
  code: string,
  severity: ResumeHealthIssue['severity'],
  path: string,
  label: string,
  message: string
): ResumeHealthIssue {
  return { id: `${code}:${path}`, code, severity, path, label, message };
}

function parseResumeDate(value: string): number | null {
  const raw = value.trim();
  if (!raw) return null;
  if (/^(至今|present)$/i.test(raw)) return Number.POSITIVE_INFINITY;
  const match = raw.match(/^(\d{4})(?:[-./](\d{1,2}))?$/);
  if (!match) return Number.NaN;
  const month = match[2] ? Number(match[2]) : 1;
  if (month < 1 || month > 12) return Number.NaN;
  return Number(match[1]) * 12 + month;
}

function checkDateRange(
  issues: ResumeHealthIssue[],
  startDate: string,
  endDate: string,
  path: string,
  label: string
) {
  const start = parseResumeDate(startDate);
  const end = parseResumeDate(endDate);
  if (startDate.trim() && Number.isNaN(start)) {
    issues.push(
      issue(
        'invalid-date',
        'warning',
        `${path}/startDate`,
        label,
        '开始时间格式无法识别，请使用 YYYY、YYYY-MM、YYYY.MM 或 YYYY/MM。'
      )
    );
  }
  if (endDate.trim() && Number.isNaN(end)) {
    issues.push(
      issue(
        'invalid-date',
        'warning',
        `${path}/endDate`,
        label,
        '结束时间格式无法识别，请使用日期、至今或 Present。'
      )
    );
  }
  if (start !== null && end !== null && !Number.isNaN(start) && !Number.isNaN(end) && start > end) {
    issues.push(issue('reversed-date', 'error', path, label, '开始时间晚于结束时间。'));
  }
}

function duplicateIssues(
  values: Array<{ value: string; path: string; label: string }>,
  code: string
): ResumeHealthIssue[] {
  const seen = new Map<string, string>();
  const issues: ResumeHealthIssue[] = [];
  for (const item of values) {
    const key = normalize(item.value);
    if (!key) continue;
    if (seen.has(key)) {
      issues.push(issue(code, 'warning', item.path, item.label, `与“${seen.get(key)}”重复。`));
    } else {
      seen.set(key, item.value.trim());
    }
  }
  return issues;
}

function checkHighlight(
  issues: ResumeHealthIssue[],
  value: string,
  path: string,
  label: string,
  confirmedClaims: Set<string>
) {
  const length = value.trim().length;
  if (!length) {
    issues.push(
      issue('empty-highlight', 'warning', path, label, '存在空成果描述，保存和导出时会忽略。')
    );
    return;
  }
  if (length < 12 || length > 160) {
    issues.push(
      issue('highlight-length', 'suggestion', path, label, '成果建议控制在 12–160 个字符。')
    );
  }
  const claims = value.match(numberWithUnitPattern) ?? [];
  for (const claim of claims) {
    if (!confirmedClaims.has(normalize(claim))) {
      issues.push(
        issue(
          'unconfirmed-claim',
          'warning',
          path,
          label,
          `量化表述“${claim}”未在已确认事实中找到相同数字与单位。`
        )
      );
      break;
    }
  }
}

export function analyzeResumeHealth(resume: ResumeProfile): ResumeHealthReport {
  const issues: ResumeHealthIssue[] = [];
  const confirmedClaims = new Set(
    resume.facts
      .filter((fact) => fact.confirmed)
      .flatMap((fact) => fact.value.match(numberWithUnitPattern) ?? [])
      .map(normalize)
  );

  if (!resume.name.trim())
    issues.push(issue('missing-name', 'error', '/name', '姓名', '请填写姓名。'));
  if (!resume.headline.trim())
    issues.push(issue('missing-headline', 'error', '/headline', '职业标题', '请填写职业标题。'));
  if (!resume.email.trim() && !resume.phone.trim()) {
    issues.push(
      issue('missing-contact', 'error', '/email', '联系方式', '邮箱和电话至少填写一项。')
    );
  }
  if (resume.email.trim() && !/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(resume.email.trim())) {
    issues.push(issue('invalid-email', 'warning', '/email', '邮箱', '邮箱格式可能不正确。'));
  }
  if (resume.website.trim()) {
    try {
      const url = new URL(
        /^https?:\/\//i.test(resume.website.trim())
          ? resume.website.trim()
          : `https://${resume.website.trim()}`
      );
      if (!url.hostname.includes('.')) throw new Error('invalid host');
    } catch {
      issues.push(
        issue('invalid-website', 'warning', '/website', '个人主页', '个人主页网址格式可能不正确。')
      );
    }
  }

  const summaryLength = resume.summary.trim().length;
  if (summaryLength < 40 || summaryLength > 220) {
    issues.push(
      issue(
        'summary-length',
        'suggestion',
        '/summary',
        '个人简介',
        '个人简介建议控制在 40–220 个字符。'
      )
    );
  }

  issues.push(
    ...duplicateIssues(
      resume.professionalSkills.map((group, index) => ({
        value: group.label,
        path: `/professionalSkills/${index}/label`,
        label: `技能分组 ${index + 1}`
      })),
      'duplicate-skill-group'
    )
  );
  resume.professionalSkills.forEach((group, index) => {
    if (!group.label.trim() && !group.items.some((item) => item.trim())) {
      issues.push(
        issue(
          'empty-record',
          'warning',
          `/professionalSkills/${index}`,
          `技能分组 ${index + 1}`,
          '这个技能分组没有内容。'
        )
      );
    }
  });
  const skills = resume.professionalSkills.flatMap((group, groupIndex) =>
    group.items.map((value, skillIndex) => ({
      value,
      path: `/professionalSkills/${groupIndex}/items/${skillIndex}`,
      label: `技能“${value || skillIndex + 1}”`
    }))
  );
  issues.push(...duplicateIssues(skills, 'duplicate-skill'));
  for (const skill of skills.filter((item) => !item.value.trim())) {
    issues.push(
      issue('empty-skill', 'warning', skill.path, skill.label, '存在空技能，保存和导出时会忽略。')
    );
  }

  const allHighlights: Array<{ value: string; path: string; label: string }> = [];
  resume.experiences.forEach((experience, index) => {
    const path = `/experiences/${index}`;
    if (
      ![
        experience.company,
        experience.position,
        experience.location,
        experience.startDate,
        experience.endDate,
        ...experience.highlights
      ].some((value) => value.trim())
    ) {
      issues.push(
        issue('empty-record', 'warning', path, `工作经历 ${index + 1}`, '这条工作经历没有内容。')
      );
    }
    checkDateRange(issues, experience.startDate, experience.endDate, path, `工作经历 ${index + 1}`);
    experience.highlights.forEach((value, highlightIndex) => {
      const item = {
        value,
        path: `${path}/highlights/${highlightIndex}`,
        label: `工作成果 ${index + 1}.${highlightIndex + 1}`
      };
      allHighlights.push(item);
      checkHighlight(issues, value, item.path, item.label, confirmedClaims);
    });
  });
  resume.projects.forEach((project, index) => {
    const path = `/projects/${index}`;
    if (
      ![
        project.name,
        project.summary,
        project.startDate,
        project.endDate,
        ...project.highlights
      ].some((value) => value.trim())
    ) {
      issues.push(
        issue('empty-record', 'warning', path, `项目 ${index + 1}`, '这条项目经历没有内容。')
      );
    }
    checkDateRange(issues, project.startDate, project.endDate, path, `项目 ${index + 1}`);
    project.highlights.forEach((value, highlightIndex) => {
      const item = {
        value,
        path: `${path}/highlights/${highlightIndex}`,
        label: `项目成果 ${index + 1}.${highlightIndex + 1}`
      };
      allHighlights.push(item);
      checkHighlight(issues, value, item.path, item.label, confirmedClaims);
    });
  });
  resume.education.forEach((education, index) => {
    const path = `/education/${index}`;
    if (
      ![
        education.institution,
        education.area,
        education.degree,
        education.degreeDetail,
        education.startDate,
        education.endDate,
        ...education.highlights
      ].some((value) => value.trim())
    ) {
      issues.push(
        issue('empty-record', 'warning', path, `教育经历 ${index + 1}`, '这条教育经历没有内容。')
      );
    }
    checkDateRange(issues, education.startDate, education.endDate, path, `教育经历 ${index + 1}`);
    education.highlights.forEach((value, highlightIndex) => {
      const item = {
        value,
        path: `${path}/highlights/${highlightIndex}`,
        label: `教育成果 ${index + 1}.${highlightIndex + 1}`
      };
      allHighlights.push(item);
      checkHighlight(issues, value, item.path, item.label, confirmedClaims);
    });
  });
  issues.push(...duplicateIssues(allHighlights, 'duplicate-highlight'));

  resume.certifications.forEach((certification, index) => {
    if (
      ![certification.name, certification.issuer, certification.date].some((value) => value.trim())
    ) {
      issues.push(
        issue(
          'empty-record',
          'warning',
          `/certifications/${index}`,
          `证书 ${index + 1}`,
          '这条证书记录没有内容。'
        )
      );
    }
    if (certification.date.trim() && Number.isNaN(parseResumeDate(certification.date))) {
      issues.push(
        issue(
          'invalid-date',
          'warning',
          `/certifications/${index}/date`,
          `证书 ${index + 1}`,
          '证书日期格式无法识别，请使用 YYYY、YYYY-MM、YYYY.MM 或 YYYY/MM。'
        )
      );
    }
  });

  issues.sort((left, right) => severityOrder[left.severity] - severityOrder[right.severity]);
  return {
    issues,
    errorCount: issues.filter((item) => item.severity === 'error').length,
    warningCount: issues.filter((item) => item.severity === 'warning').length,
    suggestionCount: issues.filter((item) => item.severity === 'suggestion').length
  };
}
