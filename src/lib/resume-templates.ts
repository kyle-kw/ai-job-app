import type { ProfessionalSkillGroup, ResumeProfile, ResumeTemplateId } from '$lib/types';

export type ResumeSectionKey = 'summary' | 'professionalSkills' | 'experiences' | 'projects' | 'certifications' | 'education';

export interface ResumeTemplateSample {
  readonly name: string;
  readonly headline: string;
  readonly email: string;
  readonly phone: string;
  readonly location: string;
  readonly website: string;
  readonly summary: string;
  readonly templateId: ResumeTemplateId;
  readonly professionalSkills: readonly {
    readonly label: string;
    readonly items: readonly string[];
  }[];
  readonly experiences: readonly {
    readonly company: string;
    readonly position: string;
    readonly location: string;
    readonly startDate: string;
    readonly endDate: string;
    readonly highlights: readonly string[];
  }[];
  readonly projects: readonly {
    readonly name: string;
    readonly summary: string;
    readonly startDate: string;
    readonly endDate: string;
    readonly highlights: readonly string[];
  }[];
  readonly certifications: readonly {
    readonly name: string;
    readonly issuer: string;
    readonly date: string;
  }[];
  readonly education: readonly {
    readonly institution: string;
    readonly area: string;
    readonly degree: string;
    readonly degreeDetail?: string;
    readonly startDate: string;
    readonly endDate: string;
    readonly highlights: readonly string[];
  }[];
}

export interface ResumeTemplateDefinition {
  id: ResumeTemplateId;
  label: string;
  description: string;
  sectionOrder: ResumeSectionKey[];
  suggestedSkillGroups: string[];
  sample?: ResumeTemplateSample;
}

export const RESUME_TEMPLATES: ResumeTemplateDefinition[] = [
  {
    id: 'ai-engineering',
    label: 'IT 技术类',
    description: '突出专业技能、工程项目与生产交付能力。',
    sectionOrder: ['summary', 'professionalSkills', 'projects', 'experiences', 'certifications', 'education'],
    suggestedSkillGroups: ['核心方向', '后端与数据', '模型与文档', '工程运维', '扩展实践']
  },
  {
    id: 'data-analysis',
    label: '数据分析',
    description: '突出数据工具、分析方法、报表与业务洞察。',
    sectionOrder: ['summary', 'professionalSkills', 'experiences', 'projects', 'certifications', 'education'],
    suggestedSkillGroups: ['数据工具', '数据处理', '分析方法', '可视化与报表', '业务分析'],
    sample: {
      name: '林晓（示例候选人）',
      headline: '数据分析师｜SQL / Python / Tableau',
      email: 'data-analyst@example.invalid',
      phone: '138-0000-0000（示例）',
      location: '上海（示例）',
      website: 'https://example.invalid/data-portfolio',
      summary: '4 年电商与零售数据分析经验，擅长从指标体系、用户漏斗和实验分析中定位业务机会。能够使用 SQL、Python 与 Tableau 完成从数据清洗、分析建模到可视化落地的完整流程，并推动分析结论转化为可衡量的业务动作。',
      templateId: 'data-analysis',
      professionalSkills: [
        { label: '数据工具', items: ['SQL', 'Python', 'Excel'] },
        { label: '数据处理', items: ['Pandas', 'ETL', '数据质量检查'] },
        { label: '分析方法', items: ['指标体系', '漏斗分析', 'A/B 测试'] },
        { label: '可视化与报表', items: ['Tableau', 'Power BI', '自动化看板'] },
        { label: '业务分析', items: ['用户增长', '留存分析', '商品分析'] }
      ],
      experiences: [
        {
          company: '星河零售（示例公司）',
          position: '高级数据分析师（示例岗位）',
          location: '上海（示例）',
          startDate: '2023.07',
          endDate: '至今',
          highlights: [
            '重构经营指标口径并搭建 Tableau 自动化看板，将周报制作周期缩短 60%，覆盖商品、渠道与用户增长三类核心场景。',
            '通过新客激活漏斗与分群分析定位关键流失节点，协同运营完成两轮 A/B 测试，次月留存率提升 8%。',
            '建立数据质量巡检规则和异常告警流程，使重复取数与人工核对工作量降低 70%。'
          ]
        },
        {
          company: '云帆电商（示例公司）',
          position: '数据分析师（示例岗位）',
          location: '杭州（示例）',
          startDate: '2021.07',
          endDate: '2023.06',
          highlights: [
            '使用 SQL 与 Python 分析活动转化、复购和客单价，形成月度经营复盘并支持渠道预算调整。',
            '搭建商品生命周期分析模型，帮助业务识别滞销库存与高潜品类，示例项目库存周转天数下降 12%。'
          ]
        }
      ],
      projects: [
        {
          name: '用户增长归因与实验分析（示例项目）',
          summary: '整合广告、站内行为和订单数据，建立从获客到复购的增长分析框架。',
          startDate: '2024.03',
          endDate: '2024.08',
          highlights: [
            '统一渠道归因规则并沉淀可复用 SQL 数据集，使投放复盘从 2 天缩短至半天。',
            '设计实验分组与显著性检验口径，推动预算向高增量渠道倾斜。'
          ]
        }
      ],
      certifications: [
        { name: 'Tableau Desktop Specialist（示例）', issuer: 'Tableau（示例信息）', date: '2023.05' }
      ],
      education: [
        {
          institution: '示例财经大学',
          area: '统计学',
          degree: '本科（示例）',
          degreeDetail: '',
          startDate: '2017.09',
          endDate: '2021.06',
          highlights: ['主修统计推断、数据库与商业分析（示例信息）。']
        }
      ]
    }
  },
  {
    id: 'finance-accounting',
    label: '财务会计',
    description: '优先展示财务经历、专业资质与合规能力。',
    sectionOrder: ['summary', 'experiences', 'certifications', 'professionalSkills', 'education', 'projects'],
    suggestedSkillGroups: ['会计核算', '税务与合规', '预算与财务分析', '财务系统与办公工具'],
    sample: {
      name: '周宁（示例候选人）',
      headline: '财务会计｜总账 / 税务 / 预算',
      email: 'finance-accounting@example.invalid',
      phone: '139-0000-0000（示例）',
      location: '苏州（示例）',
      website: 'https://example.invalid/finance-profile',
      summary: '4 年制造与零售企业财务会计经验，熟悉总账核算、月末结账、纳税申报和预算执行分析。能够使用金蝶、用友和 Excel 建立标准化核对流程，重视凭证依据、数据准确性与合规边界，并持续推动财务流程提效。',
      templateId: 'finance-accounting',
      professionalSkills: [
        { label: '会计核算', items: ['总账', '应收应付', '成本结转', '月末结账'] },
        { label: '税务与合规', items: ['增值税申报', '企业所得税', '发票管理', '审计配合'] },
        { label: '预算与财务分析', items: ['预算跟踪', '差异分析', '现金流分析'] },
        { label: '财务系统与办公工具', items: ['金蝶', '用友', 'Excel', 'Power Query'] }
      ],
      experiences: [
        {
          company: '远航制造（示例公司）',
          position: '总账会计（示例岗位）',
          location: '苏州（示例）',
          startDate: '2023.04',
          endDate: '至今',
          highlights: [
            '负责总账、成本结转和月末关账，梳理结账清单与责任节点，将月结周期从 7 天缩短至 4 天。',
            '建立往来款龄与科目余额自动核对表，使跨期和重复入账等核对差错下降 50%。',
            '按月输出预算执行差异分析，推动重点费用科目偏差稳定控制在 5% 以内。'
          ]
        },
        {
          company: '嘉禾零售（示例公司）',
          position: '财务会计（示例岗位）',
          location: '上海（示例）',
          startDate: '2021.07',
          endDate: '2023.03',
          highlights: [
            '完成应收应付、费用报销、发票认证和纳税申报基础工作，按期支持年度审计资料准备。',
            '使用 Power Query 整合门店流水与收款渠道数据，将每日对账时间由 3 小时缩短至 1 小时。'
          ]
        }
      ],
      projects: [
        {
          name: '月结与对账流程优化（示例项目）',
          summary: '围绕结账依赖、科目核对和资料归档建立标准化月结流程。',
          startDate: '2024.01',
          endDate: '2024.05',
          highlights: [
            '设计月结任务清单、截止时间与复核责任人，减少跨部门等待和重复确认。',
            '沉淀银行、往来和存货三类核对模板，提升异常定位与审计追溯效率。'
          ]
        }
      ],
      certifications: [
        { name: '初级会计专业技术资格（示例）', issuer: '示例资格信息', date: '2022.09' }
      ],
      education: [
        {
          institution: '示例财经大学',
          area: '会计学',
          degree: '本科（示例）',
          degreeDetail: '',
          startDate: '2017.09',
          endDate: '2021.06',
          highlights: ['主修财务会计、成本会计、税法与审计（示例信息）。']
        }
      ]
    }
  },
  {
    id: 'general',
    label: '通用 / 空白',
    description: '适合尚未确定岗位方向时建立可信主简历。',
    sectionOrder: ['summary', 'experiences', 'professionalSkills', 'projects', 'certifications', 'education'],
    suggestedSkillGroups: ['专业能力', '工具与系统']
  }
];

export const resumeTemplate = (id?: string) => RESUME_TEMPLATES.find((item) => item.id === id) ?? RESUME_TEMPLATES[0];

export function flattenProfessionalSkills(resume: Pick<ResumeProfile, 'professionalSkills'>): string[] {
  const seen = new Set<string>();
  return resume.professionalSkills.flatMap((group) => group.items).map((item) => item.trim()).filter((item) => {
    const key = item.toLocaleLowerCase();
    if (!key || seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}

export function suggestedProfessionalSkillGroups(templateId: ResumeTemplateId): ProfessionalSkillGroup[] {
  return resumeTemplate(templateId).suggestedSkillGroups.map((label) => ({
    id: crypto.randomUUID(),
    label,
    items: []
  }));
}
