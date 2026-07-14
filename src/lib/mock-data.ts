import { deterministicFit } from './fit';
import type { AiProviderConfig, BootstrapSnapshot, Job, ResumeProfile } from './types';

const now = new Date().toISOString();

export const mockResume: ResumeProfile = {
  id: 'resume-master',
  name: '林知远',
  headline: 'AI 应用研发工程师',
  email: 'lin@example.com',
  phone: '138 0000 0000',
  location: '上海',
  website: 'github.com/linzhiyuan',
  summary: '3 年 AI 应用与后端工程经验，专注于大模型应用、RAG、Agent 工作流和生产级服务交付。',
  templateId: 'ai-engineering',
  professionalSkills: [
    { id: 'skills-core', label: '核心方向', items: ['LangChain', 'RAG'] },
    { id: 'skills-backend', label: '后端与数据', items: ['Python', 'FastAPI', 'PostgreSQL', 'Redis', 'TypeScript'] },
    { id: 'skills-ops', label: '工程运维', items: ['Docker'] }
  ],
  experiences: [
    {
      id: 'experience-cloud',
      company: '云帆科技',
      position: 'AI 应用研发工程师',
      location: '上海',
      startDate: '2023-06',
      endDate: '至今',
      highlights: [
        '负责企业知识库 RAG 平台，检索命中率提升 23%，覆盖 6 个业务团队。',
        '设计多 Agent 审核工作流，将人工处理时长从 20 分钟降低至 6 分钟。',
        '使用 FastAPI、PostgreSQL、Redis 和 Docker 构建生产服务。'
      ]
    },
    {
      id: 'experience-star-ring',
      company: '星环数据',
      position: '后端开发工程师',
      location: '杭州',
      startDate: '2021-07',
      endDate: '2023-05',
      highlights: ['维护 Python 数据服务与异步任务系统，接口 P95 延迟降低 35%。']
    }
  ],
  education: [
    {
      id: 'education-zjut',
      institution: '浙江工业大学',
      area: '计算机科学与技术',
      degree: '本科',
      degreeDetail: '',
      startDate: '2017-09',
      endDate: '2021-06',
      highlights: []
    }
  ],
  projects: [],
  certifications: [],
  facts: [
    { id: 'fact-rag', category: 'experience', value: 'RAG 检索命中率提升 23%', source: '云帆科技经历第 1 条', confidence: 0.99, confirmed: true },
    { id: 'fact-agent', category: 'experience', value: '多 Agent 流程将处理时长从 20 分钟降到 6 分钟', source: '云帆科技经历第 2 条', confidence: 0.99, confirmed: true },
    { id: 'fact-stack', category: 'skill', value: 'Python、FastAPI、PostgreSQL、Redis、Docker', source: '云帆科技经历第 3 条', confidence: 0.98, confirmed: true }
  ],
  preferences: {
    targetRoles: ['AI Agent', '大模型应用', 'AI 应用研发'],
    cities: ['上海', '杭州'],
    remotePreference: 'hybrid',
    energizingTasks: ['从 0 到 1 构建产品', 'Agent 工作流', '工程化落地'],
    drainingTasks: ['长期纯维护', '高频出差'],
    hardConstraints: ['不接受长期出差']
  },
  sourceFileName: '林知远_AI工程师_简历.pdf',
  updatedAt: now,
  version: 3
};

const baseJobs: Job[] = [
  {
    id: 'job-1', source: 'boss', externalId: 'boss-a1', title: 'AI Agent 开发工程师', company: '森亿智能', salary: '25-40K·15薪', location: '上海·浦东新区', experience: '3-5年', degree: '本科', companyScale: '500-999人', companyStage: 'D轮及以上', industry: '人工智能', skills: ['Python', 'LangChain', 'RAG', 'FastAPI', 'Docker'], welfare: ['五险一金', '带薪年假', '年度体检'], description: '负责企业级 AI Agent 平台研发，构建 RAG、工具调用和多 Agent 编排能力。要求熟练掌握 Python、FastAPI，有 LangChain 或类似框架经验，具备服务工程化与 Docker 部署经验。', sourceUrl: 'https://www.zhipin.com/job_detail/demo-a1.html', bossName: '陈女士', bossTitle: '招聘经理', firstSeen: now, lastSeen: now, isNew: true
  },
  {
    id: 'job-2', source: 'boss', externalId: 'boss-a2', title: '大模型应用研发工程师', company: '声网', salary: '30-60K·15薪', location: '上海·杨浦区', experience: '3-5年', degree: '本科', companyScale: '500-999人', companyStage: '已上市', industry: '互联网', skills: ['Python', 'RAG', 'Redis', 'PostgreSQL', 'Kubernetes'], welfare: ['年终奖', '五险一金', '零食下午茶'], description: '面向实时互动业务构建大模型应用，负责检索增强、评测体系与在线服务。熟悉 Python、向量检索和云原生部署，有复杂系统性能优化经验。', sourceUrl: 'https://www.zhipin.com/job_detail/demo-a2.html', bossName: '周女士', bossTitle: 'HRBP', firstSeen: now, lastSeen: now, isNew: true
  },
  {
    id: 'job-3', source: 'boss', externalId: 'boss-a3', title: 'AI 全栈工程师', company: '趣申请', salary: '25-35K', location: '上海·浦东新区', experience: '3-5年', degree: '本科', companyScale: '20人以下', companyStage: '天使轮', industry: '人工智能', skills: ['TypeScript', 'Svelte', 'Python', 'PostgreSQL', 'LLM'], welfare: ['股票期权', '补充医疗', '弹性工作'], description: '负责 AI 求职产品的全栈交付，从交互原型到模型能力接入。希望候选人能独立推进、快速验证并参与产品决策。', sourceUrl: 'https://www.zhipin.com/job_detail/demo-a3.html', bossName: '田先生', bossTitle: '创始人', firstSeen: now, lastSeen: now, isNew: false
  },
  {
    id: 'job-4', source: 'boss', externalId: 'boss-a4', title: 'LLM 平台后端工程师', company: 'XTransfer', salary: '30-50K·16薪', location: '上海·黄浦区', experience: '5-10年', degree: '本科', companyScale: '1000-9999人', companyStage: 'D轮及以上', industry: '金融科技', skills: ['Java', 'Kubernetes', 'Redis', 'MySQL', '微服务'], welfare: ['五险一金', '补充医疗', '年度奖金'], description: '建设大模型平台服务和网关体系，要求 5 年以上 Java 微服务经验，熟悉 Kubernetes、MySQL 和高并发系统。', sourceUrl: 'https://www.zhipin.com/job_detail/demo-a4.html', bossName: '王女士', bossTitle: '招聘专家', firstSeen: now, lastSeen: now, isNew: false
  },
  {
    id: 'job-5', source: 'boss', externalId: 'boss-a5', title: 'RAG 算法工程师', company: '阶跃星辰', salary: '35-65K', location: '上海·徐汇区', experience: '3-5年', degree: '硕士', companyScale: '100-499人', companyStage: 'B轮', industry: '人工智能', skills: ['Python', 'RAG', 'PyTorch', '向量数据库', 'NLP'], welfare: ['股票期权', '餐补', '补充医疗'], description: '研发企业级 RAG 算法和评测体系，负责召回、排序与数据闭环。要求硕士学历，具备 PyTorch 与 NLP 算法经验。', sourceUrl: 'https://www.zhipin.com/job_detail/demo-a5.html', bossName: '李先生', bossTitle: '技术招聘', firstSeen: now, lastSeen: now, isNew: true
  }
];

export const mockJobs = baseJobs.map((job) => ({ ...job, fit: deterministicFit(job, mockResume) }));

export const defaultProviders: AiProviderConfig[] = [
  { id: 'provider-xiaomi', kind: 'xiaomi', name: '默认模型 · 小米 MiMo', baseUrl: 'https://token-plan-sgp.xiaomimimo.com/v1', model: 'mimo-v2.5', allowInsecureHttp: false, isDefault: true, verified: false, visionVerified: false },
  { id: 'provider-custom', kind: 'custom', name: '自定义 OpenAI 兼容服务', baseUrl: '', model: '', allowInsecureHttp: false, isDefault: false, verified: false, visionVerified: false }
];

export const mockSnapshot: BootstrapSnapshot = {
  readiness: { ai: false, resume: true, boss: false },
  configuration: {
    boss: { state: 'needs_setup', message: '需要配置 BOSS 专用浏览器。' },
    llm: { state: 'needs_setup', message: '填写 API Key 并测试默认模型。' }
  },
  resume: mockResume,
  providers: defaultProviders,
  tasks: [],
  scrapeRuns: [
    {
      id: 'run-demo', keyword: 'AI Agent', city: '上海', totalSeen: 90, inserted: 38, updated: 52, startedAt: now, completedAt: now,
      reportMarkdown: '## 本次岗位观察\n\n- 共整理 **90** 个岗位，其中 38 个为首次出现。\n- 高频技能为 **Python、RAG、LangChain、Docker**。\n- 3–5 年经验岗位占比最高，主流薪资集中在 **25–45K**。\n\n> 建议在简历前半页突出 Agent 工作流、RAG 评测和生产部署经验。'
    }
  ],
  settings: { advancedMode: false, privacyAcknowledgedVersion: null, lastUpdateCheckAt: null }
};
