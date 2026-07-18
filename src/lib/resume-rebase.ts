import type {
  ResumeProfile,
  ResumeRebaseChange,
  ResumeRebasePreview,
  ResumeRebaseResolution
} from '$lib/types';

const fields: ReadonlyArray<[keyof ResumeProfile, string]> = [
  ['name', '姓名'],
  ['headline', '职业标题'],
  ['email', '邮箱'],
  ['phone', '电话'],
  ['location', '所在地'],
  ['website', '个人主页'],
  ['summary', '个人简介'],
  ['templateId', '简历结构模板'],
  ['professionalSkills', '专业技能'],
  ['experiences', '工作经历'],
  ['education', '教育经历'],
  ['projects', '项目经历'],
  ['certifications', '证书 / 专业资质']
];

const equal = (left: unknown, right: unknown) => JSON.stringify(left) === JSON.stringify(right);

export function buildResumeRebasePreview(
  variantId: string,
  variantVersion: number,
  baseResumeVersion: number,
  base: ResumeProfile,
  master: ResumeProfile,
  variant: ResumeProfile
): ResumeRebasePreview {
  const autoChanges: ResumeRebaseChange[] = [];
  const conflicts: ResumeRebaseChange[] = [];
  for (const [key, label] of fields) {
    const change: ResumeRebaseChange = {
      path: `/${key}`,
      label,
      base: structuredClone(base[key]),
      master: structuredClone(master[key]),
      variant: structuredClone(variant[key])
    };
    if (equal(variant[key], base[key]) && !equal(master[key], base[key])) autoChanges.push(change);
    else if (
      !equal(variant[key], base[key]) &&
      !equal(master[key], base[key]) &&
      !equal(variant[key], master[key])
    )
      conflicts.push(change);
  }
  return {
    variantId,
    variantVersion,
    baseResumeVersion,
    masterVersion: master.version,
    autoChanges,
    conflicts
  };
}

export function applyResumeRebase(
  variant: ResumeProfile,
  master: ResumeProfile,
  preview: ResumeRebasePreview,
  resolutions: ResumeRebaseResolution[]
): ResumeProfile {
  const next = structuredClone(variant) as ResumeProfile & Record<string, unknown>;
  for (const change of preview.autoChanges)
    next[change.path.slice(1)] = structuredClone(change.master);
  const choices = new Map(resolutions.map((item) => [item.path, item.choice]));
  for (const conflict of preview.conflicts) {
    const choice = choices.get(conflict.path);
    if (!choice) throw new Error(`请处理字段“${conflict.label}”的同步冲突`);
    if (choice === 'master') next[conflict.path.slice(1)] = structuredClone(conflict.master);
  }
  next.facts = structuredClone(master.facts);
  next.preferences = structuredClone(master.preferences);
  return next;
}
