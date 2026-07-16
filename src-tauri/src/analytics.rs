use crate::models::{
    Job, JobDataReport, ReportBatchComparison, ReportBatchSkillChange, ReportBatchSnapshot,
    ReportBucket, ReportCompetitivenessAnalysis, ReportKeyword, ReportSampleMetric,
    ReportSampleQuality, SalaryByExperience, SalarySummary, ScrapeRun, ScrapeSampleSummary,
    SearchSpec,
};
use crate::time;
use chrono::{DateTime, FixedOffset, NaiveDate};
use std::collections::{BTreeSet, HashMap, HashSet};

const SKILL_TAXONOMY: &[(&str, &[&str])] = &[
    ("RAG / 检索增强", &["rag", "检索增强"]),
    (
        "Prompt 工程",
        &["prompt engineering", "prompt", "提示词", "提示工程"],
    ),
    (
        "工作流 / 编排",
        &["workflow", "工作流", "流程编排", "智能体编排"],
    ),
    (
        "工具 / 函数调用",
        &["tool calling", "function calling", "工具调用", "函数调用"],
    ),
    ("知识库", &["knowledge base", "知识库"]),
    (
        "向量数据库",
        &["vector database", "vector db", "向量数据库"],
    ),
    ("模型微调", &["lora", "qlora", "sft", "微调"]),
    (
        "多 Agent",
        &[
            "multi-agent",
            "multi agent",
            "多agent",
            "多 agent",
            "多智能体",
        ],
    ),
    ("Embedding", &["embedding", "嵌入模型", "向量化"]),
    (
        "模型推理 / 优化",
        &["vllm", "模型推理", "推理优化", "量化", "蒸馏"],
    ),
    ("效果评估", &["效果评估", "模型评估", "评测体系", "准确率"]),
    ("多模态", &["多模态", "vlm", "视觉语言模型"]),
    ("MCP", &["mcp"]),
    ("LangChain", &["langchain"]),
    ("LangGraph", &["langgraph"]),
    ("LlamaIndex", &["llamaindex", "llama index"]),
    ("Dify", &["dify"]),
    ("AutoGen", &["autogen"]),
    ("CrewAI", &["crewai"]),
    ("Python", &["python"]),
    ("Java", &["java"]),
    ("Go / Golang", &["golang"]),
    ("JavaScript / TypeScript", &["javascript", "typescript"]),
    ("FastAPI", &["fastapi"]),
    ("Django", &["django"]),
    ("Flask", &["flask"]),
    ("React / Next.js", &["react", "next.js", "nextjs"]),
    ("Vue", &["vue.js", "vue"]),
    ("Docker", &["docker", "容器化"]),
    ("Kubernetes / K8s", &["kubernetes", "k8s"]),
    ("Linux", &["linux"]),
    (
        "API / 系统集成",
        &["api", "接口集成", "系统集成", "系统对接"],
    ),
    ("部署 / 上线", &["部署", "上线", "生产环境"]),
    ("监控 / 可观测", &["监控", "可观测", "告警"]),
    (
        "性能 / 稳定性",
        &["性能优化", "高可用", "高并发", "稳定性", "可靠性", "低延迟"],
    ),
    ("MySQL", &["mysql"]),
    ("PostgreSQL", &["postgresql", "postgres"]),
    ("Redis", &["redis"]),
    ("MongoDB", &["mongodb"]),
    (
        "消息队列 / Kafka",
        &["kafka", "rabbitmq", "rocketmq", "消息队列"],
    ),
];

pub fn build_report_for_keywords(
    jobs: &[Job],
    selected_keywords: Vec<ReportKeyword>,
) -> JobDataReport {
    build_report_for_keywords_with_runs(jobs, selected_keywords, &[])
}

pub fn build_report_for_keywords_with_runs(
    jobs: &[Job],
    selected_keywords: Vec<ReportKeyword>,
    scrape_runs: &[ScrapeRun],
) -> JobDataReport {
    build_report_for_keywords_at(jobs, selected_keywords, scrape_runs, time::shanghai_now())
}

fn build_report_for_keywords_at(
    jobs: &[Job],
    selected_keywords: Vec<ReportKeyword>,
    scrape_runs: &[ScrapeRun],
    now: DateTime<FixedOffset>,
) -> JobDataReport {
    let total = jobs.len() as i64;
    let mut companies = BTreeSet::new();
    let mut city_counter = HashMap::new();
    let mut experience_counter = HashMap::new();
    let mut degree_counter = HashMap::new();
    let mut role_counter = HashMap::new();
    let mut industry_counter = HashMap::new();
    let mut scale_counter = HashMap::new();
    let mut skill_counter = HashMap::new();
    let mut pair_counter = HashMap::new();
    let mut welfare_counter = HashMap::new();
    let mut salary_bands = HashMap::new();
    let mut salary_lows = vec![];
    let mut salary_mids = vec![];
    let mut salary_highs = vec![];
    let mut salary_by_experience: HashMap<String, Vec<f64>> = HashMap::new();
    let mut extra_months_count = 0_i64;
    let mut detail_jobs = 0_i64;
    let mut skill_jobs = 0_i64;
    let mut experience_jobs = 0_i64;
    let mut degree_jobs = 0_i64;
    let mut first_dates = vec![];
    let mut last_dates = vec![];

    for job in jobs {
        if !job.company.trim().is_empty() {
            companies.insert(job.company.trim().to_string());
        }
        increment(&mut city_counter, first_segment(&job.location));
        if !job.experience.trim().is_empty() {
            experience_jobs += 1;
        }
        if !job.degree.trim().is_empty() {
            degree_jobs += 1;
        }
        increment(&mut experience_counter, fallback(&job.experience));
        increment(&mut degree_counter, fallback(&job.degree));
        increment(&mut role_counter, classify_role(&job.title));
        increment(&mut industry_counter, fallback(&job.industry));
        increment(&mut scale_counter, fallback(&job.company_scale));
        if !job.description.trim().is_empty() {
            detail_jobs += 1;
        }
        if !job.first_seen.is_empty() {
            first_dates.push(job.first_seen.clone());
        }
        if !job.last_seen.is_empty() {
            last_dates.push(job.last_seen.clone());
        }

        for item in &job.welfare {
            increment(&mut welfare_counter, item.trim().to_string());
        }

        let detected = detected_skills(job);
        if !detected.is_empty() {
            skill_jobs += 1;
        }
        for skill in &detected {
            increment(&mut skill_counter, skill.clone());
        }
        let skill_list: Vec<_> = detected.into_iter().collect();
        for left in 0..skill_list.len() {
            for right in left + 1..skill_list.len() {
                increment(
                    &mut pair_counter,
                    format!("{} × {}", skill_list[left], skill_list[right]),
                );
            }
        }

        if let Some((low, mid, high, months)) = parse_salary(&job.salary) {
            salary_lows.push(low);
            salary_mids.push(mid);
            salary_highs.push(high);
            if months.unwrap_or(12) > 12 {
                extra_months_count += 1;
            }
            increment(&mut salary_bands, salary_band(mid));
            salary_by_experience
                .entry(fallback(&job.experience))
                .or_default()
                .push(mid);
        }
    }

    let top_skills = buckets(skill_counter, total, 18);
    let experience = buckets(experience_counter, total, 12);
    let degree = buckets(degree_counter, total, 10);
    let roles = buckets(role_counter, total, 10);
    let mut salary_by_experience: Vec<_> = salary_by_experience
        .into_iter()
        .filter_map(|(label, values)| {
            median(&values).map(|median_k| SalaryByExperience {
                label,
                count: values.len() as i64,
                median_k,
            })
        })
        .collect();
    salary_by_experience.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.label.cmp(&right.label))
    });

    let mut insights = vec![];
    if total == 0 {
        insights.push("当前关键词范围暂无岗位数据，请调整筛选或先完成抓取。".to_string());
    } else {
        insights.push(format!(
            "当前 {total} 个本地去重岗位样本覆盖 {} 家公司、{} 个城市。",
            companies.len(),
            city_counter.len()
        ));
        if let Some(value) = median(&salary_mids) {
            insights.push(format!(
                "当前样本中可解析薪资的岗位有 {} 个，月薪区间中点中位数为 {:.1}K。",
                salary_mids.len(),
                value
            ));
        }
        if !top_skills.is_empty() {
            insights.push(format!(
                "当前样本中反复出现的技能包括 {}。",
                top_skills
                    .iter()
                    .take(5)
                    .map(|item| format!("{}（{:.1}%）", item.label, item.percentage))
                    .collect::<Vec<_>>()
                    .join("、")
            ));
        }
        if let Some(item) = experience.first() {
            insights.push(format!(
                "当前样本的经验要求以“{}”为主，占全部岗位的 {:.1}%。",
                item.label, item.percentage
            ));
        }
    }

    first_dates.sort();
    last_dates.sort();
    let salary_sample_count = salary_mids.len() as i64;
    let sample_quality = build_sample_quality(
        total,
        detail_jobs,
        salary_sample_count,
        skill_jobs,
        experience_jobs,
        degree_jobs,
    );
    let batch_comparison = build_batch_comparison(&selected_keywords, scrape_runs);
    JobDataReport {
        generated_at: now.to_rfc3339(),
        selected_keywords,
        data_from: first_dates.first().map(|value| date_part(value)),
        data_to: last_dates.last().map(|value| date_part(value)),
        total_jobs: total,
        total_companies: companies.len() as i64,
        total_cities: city_counter.len() as i64,
        detail_jobs,
        detail_coverage: percentage(detail_jobs, total),
        salary: SalarySummary {
            sample_count: salary_sample_count,
            median_low_k: median(&salary_lows),
            median_mid_k: median(&salary_mids),
            median_high_k: median(&salary_highs),
            extra_months_count,
            bands: buckets(salary_bands, salary_mids.len() as i64, 10),
        },
        experience,
        degree,
        roles,
        cities: buckets(city_counter, total, 12),
        industries: buckets(industry_counter, total, 12),
        company_scales: buckets(scale_counter, total, 10),
        top_skills,
        skill_pairs: buckets(pair_counter, total, 10),
        welfare: buckets(welfare_counter, total, 12),
        salary_by_experience,
        insights,
        sample_quality,
        batch_comparison,
    }
}

pub fn build_scrape_sample(jobs: &[Job]) -> ScrapeSampleSummary {
    let mut seen = HashSet::new();
    let jobs = jobs
        .iter()
        .filter(|job| seen.insert(job.id.as_str()))
        .collect::<Vec<_>>();
    let total = jobs.len() as i64;
    let mut job_ids = jobs.iter().map(|job| job.id.clone()).collect::<Vec<_>>();
    job_ids.sort();
    let detail_jobs = jobs
        .iter()
        .filter(|job| !job.description.trim().is_empty())
        .count() as i64;
    let salary_mids = jobs
        .iter()
        .filter_map(|job| parse_salary(&job.salary).map(|(_, middle, _, _)| middle))
        .collect::<Vec<_>>();
    let mut skill_counter = HashMap::new();
    let mut skill_sample_count = 0_i64;
    for job in &jobs {
        let skills = detected_skills(job);
        if !skills.is_empty() {
            skill_sample_count += 1;
        }
        for skill in skills {
            increment(&mut skill_counter, skill);
        }
    }
    ScrapeSampleSummary {
        job_ids,
        total_jobs: total,
        detail_jobs,
        detail_coverage: percentage(detail_jobs, total),
        salary_sample_count: salary_mids.len() as i64,
        median_salary_k: median(&salary_mids),
        skill_sample_count,
        skill_coverage: percentage(skill_sample_count, total),
        skills: buckets(skill_counter, total, usize::MAX),
    }
}

fn build_sample_quality(
    total: i64,
    detail: i64,
    salary: i64,
    skill: i64,
    experience: i64,
    degree: i64,
) -> ReportSampleQuality {
    let metric = |count| ReportSampleMetric {
        count,
        coverage: percentage(count, total),
    };
    let mut limitations =
        vec!["本报告仅反映本机保存的有限页 BOSS 岗位样本，不代表完整招聘市场。".to_string()];
    if total < 20 {
        limitations.push("当前少于 20 个岗位，比例和排序仅适合作为方向提示。".into());
    }
    for (count, threshold, message) in [
        (
            detail,
            60.0,
            "岗位详情覆盖不足 60%，职责和要求统计可能不完整。",
        ),
        (
            salary,
            50.0,
            "可解析薪资覆盖不足 50%，薪资统计可能偏离当前样本。",
        ),
        (
            skill,
            60.0,
            "技能信息覆盖不足 60%，高频技能排序可能受缺失字段影响。",
        ),
        (experience, 60.0, "经验要求覆盖不足 60%，经验分布仅供参考。"),
        (degree, 60.0, "学历要求覆盖不足 60%，学历分布仅供参考。"),
    ] {
        if percentage(count, total) < threshold {
            limitations.push(message.into());
        }
    }
    ReportSampleQuality {
        detail: metric(detail),
        salary: metric(salary),
        skill: metric(skill),
        experience: metric(experience),
        degree: metric(degree),
        limitations,
    }
}

fn unavailable_batch_comparison(reason: &str) -> ReportBatchComparison {
    ReportBatchComparison {
        status: "unavailable".into(),
        reason: Some(reason.into()),
        current: None,
        previous: None,
        job_count_change_percentage: None,
        newly_observed_jobs: 0,
        not_observed_jobs: 0,
        salary_median_delta_k: None,
        skill_changes: vec![],
    }
}

fn build_batch_comparison(
    selected_keywords: &[ReportKeyword],
    scrape_runs: &[ScrapeRun],
) -> ReportBatchComparison {
    if selected_keywords.len() != 1 {
        return unavailable_batch_comparison("multi_keyword");
    }
    let selected_keys = [
        normalize_keyword(&selected_keywords[0].key),
        normalize_keyword(&selected_keywords[0].label),
    ];
    let mut candidates = scrape_runs
        .iter()
        .filter(|run| {
            run.completed_at.is_some()
                && run.search_spec.is_some()
                && run.sample.is_some()
                && selected_keys.contains(&normalize_keyword(&run.keyword))
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        run_timestamp(right)
            .cmp(&run_timestamp(left))
            .then_with(|| right.id.cmp(&left.id))
    });
    let Some(current) = candidates.first().copied() else {
        return unavailable_batch_comparison("no_captured_run");
    };
    let current_spec = current.search_spec.as_ref().expect("filtered search spec");
    let current_date = current.completed_at.as_deref().and_then(shanghai_date);
    let previous = candidates.into_iter().skip(1).find(|run| {
        let Some(spec) = run.search_spec.as_ref() else {
            return false;
        };
        same_search_scope(current_spec, spec)
            && current_date.is_some()
            && run.completed_at.as_deref().and_then(shanghai_date) != current_date
    });
    let Some(previous) = previous else {
        return unavailable_batch_comparison("no_comparable_run");
    };
    let current_sample = current.sample.as_ref().expect("filtered sample");
    let previous_sample = previous.sample.as_ref().expect("filtered sample");
    let current_ids = current_sample
        .job_ids
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let previous_ids = previous_sample
        .job_ids
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let current_skills = current_sample
        .skills
        .iter()
        .map(|item| (item.label.as_str(), item))
        .collect::<HashMap<_, _>>();
    let previous_skills = previous_sample
        .skills
        .iter()
        .map(|item| (item.label.as_str(), item))
        .collect::<HashMap<_, _>>();
    let mut labels = BTreeSet::new();
    labels.extend(current_skills.keys().copied());
    labels.extend(previous_skills.keys().copied());
    let mut skill_changes = labels
        .into_iter()
        .map(|label| {
            let current = current_skills.get(label).copied();
            let previous = previous_skills.get(label).copied();
            let current_count = current.map_or(0, |item| item.count);
            let previous_count = previous.map_or(0, |item| item.count);
            let current_percentage = current.map_or(0.0, |item| item.percentage);
            let previous_percentage = previous.map_or(0.0, |item| item.percentage);
            ReportBatchSkillChange {
                label: label.to_string(),
                current_count,
                current_percentage,
                previous_count,
                previous_percentage,
                delta_percentage_points: round_one(current_percentage - previous_percentage),
            }
        })
        .collect::<Vec<_>>();
    skill_changes.sort_by(|left, right| {
        right
            .delta_percentage_points
            .abs()
            .total_cmp(&left.delta_percentage_points.abs())
            .then_with(|| right.current_count.cmp(&left.current_count))
            .then_with(|| left.label.cmp(&right.label))
    });
    skill_changes.truncate(8);
    ReportBatchComparison {
        status: "available".into(),
        reason: None,
        current: Some(batch_snapshot(current)),
        previous: Some(batch_snapshot(previous)),
        job_count_change_percentage: if previous_sample.total_jobs == 0 {
            None
        } else {
            Some(round_one(
                (current_sample.total_jobs - previous_sample.total_jobs) as f64
                    / previous_sample.total_jobs as f64
                    * 100.0,
            ))
        },
        newly_observed_jobs: current_ids.difference(&previous_ids).count() as i64,
        not_observed_jobs: previous_ids.difference(&current_ids).count() as i64,
        salary_median_delta_k: current_sample
            .median_salary_k
            .zip(previous_sample.median_salary_k)
            .map(|(current, previous)| round_one(current - previous)),
        skill_changes,
    }
}

fn batch_snapshot(run: &ScrapeRun) -> ReportBatchSnapshot {
    let sample = run.sample.as_ref().expect("batch snapshot requires sample");
    ReportBatchSnapshot {
        run_id: run.id.clone(),
        completed_at: run.completed_at.clone().unwrap_or_default(),
        search_spec: run
            .search_spec
            .clone()
            .expect("batch snapshot requires spec"),
        total_jobs: sample.total_jobs,
        detail_coverage: sample.detail_coverage,
        salary_sample_count: sample.salary_sample_count,
        median_salary_k: sample.median_salary_k,
    }
}

fn normalize_keyword(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn normalized_option(value: &Option<String>) -> String {
    value.as_deref().unwrap_or_default().trim().to_lowercase()
}

fn same_search_scope(left: &SearchSpec, right: &SearchSpec) -> bool {
    normalize_keyword(&left.keyword) == normalize_keyword(&right.keyword)
        && left.city.trim() == right.city.trim()
        && left.pages == right.pages
        && normalized_option(&left.salary) == normalized_option(&right.salary)
        && normalized_option(&left.experience) == normalized_option(&right.experience)
        && normalized_option(&left.degree) == normalized_option(&right.degree)
        && normalized_option(&left.company_scale) == normalized_option(&right.company_scale)
}

fn run_timestamp(run: &ScrapeRun) -> i64 {
    run.completed_at
        .as_deref()
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map_or(i64::MIN, |value| value.timestamp_millis())
}

fn shanghai_date(value: &str) -> Option<NaiveDate> {
    if let Ok(value) = DateTime::parse_from_rfc3339(value) {
        let shanghai = FixedOffset::east_opt(8 * 60 * 60)?;
        return Some(value.with_timezone(&shanghai).date_naive());
    }
    value
        .get(..10)
        .and_then(|value| NaiveDate::parse_from_str(value, "%Y-%m-%d").ok())
}

fn round_one(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

pub fn render_html(report: &JobDataReport) -> String {
    let insights = report
        .insights
        .iter()
        .map(|item| format!("<li>{}</li>", escape_html(item)))
        .collect::<Vec<_>>()
        .join("");
    let quality = [
        ("岗位详情", &report.sample_quality.detail),
        ("可解析薪资", &report.sample_quality.salary),
        ("技能信息", &report.sample_quality.skill),
        ("经验要求", &report.sample_quality.experience),
        ("学历要求", &report.sample_quality.degree),
    ]
    .iter()
    .map(|(label, metric)| {
        format!(
            "<div><strong>{}</strong><p>{} 个 · 覆盖 {:.1}%</p></div>",
            escape_html(label),
            metric.count,
            metric.coverage
        )
    })
    .collect::<Vec<_>>()
    .join("");
    let limitations = report
        .sample_quality
        .limitations
        .iter()
        .map(|item| format!("<li>{}</li>", escape_html(item)))
        .collect::<Vec<_>>()
        .join("");
    let period = match (&report.data_from, &report.data_to) {
        (Some(from), Some(to)) => format!("{} 至 {}", escape_html(from), escape_html(to)),
        _ => "暂无时间范围".to_string(),
    };
    let keyword_scope = if report.selected_keywords.is_empty() {
        "未指定关键词".to_string()
    } else {
        report
            .selected_keywords
            .iter()
            .map(|keyword| keyword.label.as_str())
            .collect::<Vec<_>>()
            .join("、")
    };
    let title = format!("岗位数据报告 · {keyword_scope}");
    format!(
        r#"<!doctype html><html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>{title}</title><style>
:root{{font-family:Inter,"PingFang SC","Microsoft YaHei",sans-serif;color:#18302a;background:#f4f6f2}}*{{box-sizing:border-box}}body{{margin:0}}main{{max-width:1200px;margin:auto;padding:40px 24px 80px}}header{{padding:34px;border-radius:24px;background:#176b57;color:#fff}}h1{{margin:0 0 10px;font-size:34px}}header p{{margin:0;opacity:.82}}.kpis{{display:grid;grid-template-columns:repeat(4,1fr);gap:14px;margin:20px 0}}.card,.section{{background:#fff;border:1px solid #dfe4de;border-radius:18px}}.card{{padding:18px}}.card strong{{display:block;font-size:27px}}.card span{{color:#68736e;font-size:13px}}.section{{padding:24px;margin-top:18px}}h2{{font-size:20px;margin:0 0 18px}}.grid{{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:20px}}.bars{{display:grid;gap:10px}}.bar{{display:grid;grid-template-columns:150px 1fr 88px;gap:10px;align-items:center;font-size:13px}}.track{{height:9px;background:#edf1ed;border-radius:99px;overflow:hidden}}.fill{{height:100%;background:#2d8b70;border-radius:99px}}.value{{text-align:right;color:#68736e}}ul{{margin:0;padding-left:20px;line-height:1.8}}.meta{{margin-top:10px;font-size:12px;color:#68736e}}@media(max-width:760px){{.kpis,.grid{{grid-template-columns:1fr 1fr}}.bar{{grid-template-columns:110px 1fr 74px}}}}@media(max-width:520px){{.kpis,.grid{{grid-template-columns:1fr}}}}
</style></head><body><main><header><h1>{title}</h1><p>关键词范围：{scope} · 本机保存的有限页 BOSS 样本 · {period}</p></header><section class="kpis"><div class="card"><strong>{jobs}</strong><span>本地去重岗位样本</span></div><div class="card"><strong>{companies}</strong><span>样本内招聘公司</span></div><div class="card"><strong>{salary}</strong><span>样本月薪中点中位数</span></div><div class="card"><strong>{coverage:.1}%</strong><span>岗位详情覆盖率</span></div></section><section class="section"><h2>先看本地样本结论</h2><ul>{insights}</ul></section><section class="section"><h2>样本范围与可用性</h2><div class="grid">{quality}</div><h3>使用限制</h3><ul>{limitations}</ul></section><div class="grid"><section class="section"><h2>高频技能</h2>{skills}</section><section class="section"><h2>技能共现组合</h2>{pairs}</section><section class="section"><h2>经验要求</h2>{experience}</section><section class="section"><h2>学历要求</h2>{degree}</section><section class="section"><h2>薪资分布</h2>{salary_bands}</section><section class="section"><h2>岗位方向</h2>{roles}</section><section class="section"><h2>城市分布</h2>{cities}</section><section class="section"><h2>行业分布</h2>{industries}</section><section class="section"><h2>公司规模</h2>{scales}</section><section class="section"><h2>常见福利</h2>{welfare}</section></div><p class="meta">生成时间：{generated}（Asia/Shanghai） · 按本地岗位 ID 去重 · 文件编码 UTF-8</p></main></body></html>"#,
        title = escape_html(&title),
        scope = escape_html(&keyword_scope),
        quality = quality,
        limitations = limitations,
        jobs = report.total_jobs,
        companies = report.total_companies,
        salary = report
            .salary
            .median_mid_k
            .map(|value| format!("{value:.1}K"))
            .unwrap_or_else(|| "—".to_string()),
        coverage = report.detail_coverage,
        skills = render_bars(&report.top_skills),
        pairs = render_bars(&report.skill_pairs),
        experience = render_bars(&report.experience),
        degree = render_bars(&report.degree),
        salary_bands = render_bars(&report.salary.bands),
        roles = render_bars(&report.roles),
        cities = render_bars(&report.cities),
        industries = render_bars(&report.industries),
        scales = render_bars(&report.company_scales),
        welfare = render_bars(&report.welfare),
        generated = escape_html(&report.generated_at),
    )
}

pub fn append_decision_sections(
    html: String,
    report: &JobDataReport,
    competitiveness: Option<&ReportCompetitivenessAnalysis>,
) -> String {
    let comparison_section = if report.batch_comparison.status == "available" {
        let comparison = &report.batch_comparison;
        let current = comparison
            .current
            .as_ref()
            .expect("available current batch");
        let previous = comparison
            .previous
            .as_ref()
            .expect("available previous batch");
        let total_change = comparison
            .job_count_change_percentage
            .map(|value| format!("{value:+.1}%"))
            .unwrap_or_else(|| "暂无可比比例".into());
        let salary_change = comparison
            .salary_median_delta_k
            .map(|value| format!("{value:+.1}K"))
            .unwrap_or_else(|| "暂无可比薪资".into());
        let skill_changes = if comparison.skill_changes.is_empty() {
            "<p>两个批次暂无可展示的技能占比变化。</p>".to_string()
        } else {
            format!(
                "<ul>{}</ul>",
                comparison
                    .skill_changes
                    .iter()
                    .map(|item| format!(
                        "<li><strong>{}</strong>：{:+.1} 个百分点（本次 {} 个，上次 {} 个）</li>",
                        escape_html(&item.label),
                        item.delta_percentage_points,
                        item.current_count,
                        item.previous_count
                    ))
                    .collect::<Vec<_>>()
                    .join("")
            )
        };
        format!(
            "<section class=\"section\"><h2>最近两次同条件样本对比</h2><p>{previous_time} 与 {current_time}，关键词、城市、页数和筛选条件完全一致。</p><div class=\"grid\"><div><strong>{current_jobs}</strong><p>本次岗位 · 较上次 {total_change}</p></div><div><strong>{new_jobs}</strong><p>本次新出现</p></div><div><strong>{missing_jobs}</strong><p>本次有限结果未再次出现</p></div><div><strong>{salary_change}</strong><p>薪资中点中位数变化</p></div></div><p><small>“未再次出现”仅表示不在本次有限页结果中，不能据此判断岗位下架。</small></p><h3>技能占比变化</h3>{skill_changes}</section>",
            previous_time = escape_html(&previous.completed_at),
            current_time = escape_html(&current.completed_at),
            current_jobs = current.total_jobs,
            new_jobs = comparison.newly_observed_jobs,
            missing_jobs = comparison.not_observed_jobs,
        )
    } else {
        let reason = match report.batch_comparison.reason.as_deref() {
            Some("multi_keyword") => "当前选择了多个关键词，合并样本不能进行同条件批次比较。",
            Some("no_captured_run") => {
                "历史抓取记录没有完整的搜索条件和样本摘要；完成一次新抓取后开始积累。"
            }
            _ => "暂时没有跨日、且搜索条件完全相同的两个成功批次。",
        };
        format!(
            "<section class=\"section\"><h2>同条件样本对比</h2><p>{}</p><p><small>应用不会为了生成批次对比而自动或重复访问 BOSS。</small></p></section>",
            escape_html(reason)
        )
    };
    let competitiveness_section = competitiveness.map(|analysis| {
        let rows = analysis
            .items
            .iter()
            .map(|item| {
                let status = match item.status.as_str() {
                    "covered" => "已覆盖",
                    "strengthenable" => "可强化",
                    "gap" => "真实缺口",
                    _ => "待判断",
                };
                format!(
                    "<li><strong>{}</strong>（{} 个岗位，{:.1}%）· {}<br>{}</li>",
                    escape_html(&item.label),
                    item.job_count,
                    item.percentage,
                    status,
                    escape_html(&item.rationale)
                )
            })
            .collect::<Vec<_>>()
            .join("");
        format!(
            "<section class=\"section\"><h2>个人竞争力矩阵</h2><p>分析来源：{}</p><ul>{rows}</ul></section>",
            if analysis.source == "ai" { "AI 语义复核" } else { "本地精确匹配" }
        )
    }).unwrap_or_default();
    html.replacen(
        "</main>",
        &format!("{comparison_section}{competitiveness_section}</main>"),
        1,
    )
}

pub fn append_interview_preparation(
    html: String,
    preparation: &crate::models::InterviewPreparation,
) -> String {
    let skills = preparation
        .skills
        .iter()
        .map(|skill| {
            format!(
                "<li><strong>{}</strong>{}<br>{}</li>",
                escape_html(&skill.name),
                skill
                    .job_count
                    .map(|count| format!("（{count} 个岗位）"))
                    .unwrap_or_default(),
                escape_html(&skill.action)
            )
        })
        .collect::<Vec<_>>()
        .join("");
    let projects = preparation
        .project_ideas
        .iter()
        .map(|item| format!("<li>{}</li>", escape_html(item)))
        .collect::<Vec<_>>()
        .join("");
    let questions = preparation
        .practice_questions
        .iter()
        .map(|item| format!("<li>{}</li>", escape_html(item)))
        .collect::<Vec<_>>()
        .join("");
    let section = format!(
        "<section class=\"section\"><h2>AI 面试准备</h2><p>{}</p><h3>优先技能</h3><ul>{skills}</ul><h3>项目案例</h3><ul>{projects}</ul><h3>练习问题</h3><ul>{questions}</ul></section>",
        escape_html(&preparation.summary)
    );
    html.replacen("</main>", &format!("{section}</main>"), 1)
}

fn detected_skills(job: &Job) -> BTreeSet<String> {
    let mut result = BTreeSet::new();
    for skill in &job.skills {
        let skill = skill.trim();
        if !skill.is_empty() && skill.chars().count() <= 36 {
            result.insert(skill.to_string());
        }
    }
    let text = format!("{} {} {}", job.title, job.description, job.skills.join(" ")).to_lowercase();
    for (label, aliases) in SKILL_TAXONOMY {
        if aliases.iter().any(|alias| matches_alias(&text, alias)) {
            result.insert((*label).to_string());
        }
    }
    result
}

pub fn job_has_skill(job: &Job, label: &str) -> bool {
    detected_skills(job)
        .iter()
        .any(|skill| skill.eq_ignore_ascii_case(label.trim()))
}

fn matches_alias(text: &str, alias: &str) -> bool {
    if alias.chars().all(|value| value.is_ascii_alphanumeric()) && alias.len() <= 5 {
        text.split(|value: char| !value.is_ascii_alphanumeric())
            .any(|token| token == alias)
    } else {
        text.contains(alias)
    }
}

fn parse_salary(value: &str) -> Option<(f64, f64, f64, Option<i64>)> {
    let upper = value.to_uppercase();
    let k_index = upper.find('K')?;
    let numbers = numbers_in(&upper[..k_index]);
    if numbers.len() < 2 {
        return None;
    }
    let low = numbers[0];
    let high = numbers[1];
    if low <= 0.0 || high < low {
        return None;
    }
    let months = numbers_in(&upper[k_index + 1..])
        .first()
        .map(|value| *value as i64)
        .filter(|value| (12..=24).contains(value));
    Some((low, (low + high) / 2.0, high, months))
}

fn numbers_in(value: &str) -> Vec<f64> {
    let mut numbers = vec![];
    let mut current = String::new();
    for character in value.chars() {
        if character.is_ascii_digit() || character == '.' {
            current.push(character);
        } else if !current.is_empty() {
            if let Ok(number) = current.parse() {
                numbers.push(number);
            }
            current.clear();
        }
    }
    if !current.is_empty() {
        if let Ok(number) = current.parse() {
            numbers.push(number);
        }
    }
    numbers
}

fn classify_role(title: &str) -> String {
    let value = title.to_lowercase();
    if contains_any(&value, &["架构", "专家", "负责人", "技术总监", "lead"]) {
        "架构 / 专家".to_string()
    } else if contains_any(&value, &["产品经理", "产品负责人", "product"]) {
        "产品".to_string()
    } else if contains_any(&value, &["测试", "质量", "qa"]) {
        "测试 / 质量".to_string()
    } else if contains_any(
        &value,
        &["全栈", "前端", "frontend", "full stack", "fullstack"],
    ) {
        "前端 / 全栈".to_string()
    } else if contains_any(
        &value,
        &["agent", "智能体", "大模型", "llm", "rag", "人工智能"],
    ) || has_ascii_ai_marker(&value)
    {
        "AI / Agent 开发".to_string()
    } else if contains_any(&value, &["算法", "数据科学", "机器学习", "nlp", "数据分析"])
    {
        "算法 / 数据".to_string()
    } else if contains_any(
        &value,
        &["后端", "java", "golang", "go开发", "rust", "服务端"],
    ) {
        "后端开发".to_string()
    } else {
        "其他岗位".to_string()
    }
}

fn has_ascii_ai_marker(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.windows(2).enumerate().any(|(index, pair)| {
        pair.eq_ignore_ascii_case(b"ai")
            && (index == 0 || !bytes[index - 1].is_ascii_alphabetic())
            && (index + 2 == bytes.len() || !bytes[index + 2].is_ascii_alphabetic())
    })
}

fn contains_any(value: &str, items: &[&str]) -> bool {
    items.iter().any(|item| value.contains(item))
}

fn salary_band(value: f64) -> String {
    match value {
        value if value < 15.0 => "15K 以下",
        value if value < 25.0 => "15–25K",
        value if value < 35.0 => "25–35K",
        value if value < 50.0 => "35–50K",
        _ => "50K 以上",
    }
    .to_string()
}

fn increment(counter: &mut HashMap<String, i64>, label: String) {
    if !label.trim().is_empty() {
        *counter.entry(label).or_insert(0) += 1;
    }
}

fn buckets(counter: HashMap<String, i64>, total: i64, limit: usize) -> Vec<ReportBucket> {
    let mut rows: Vec<_> = counter
        .into_iter()
        .map(|(label, count)| ReportBucket {
            label,
            count,
            percentage: percentage(count, total),
        })
        .collect();
    rows.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.label.cmp(&right.label))
    });
    rows.truncate(limit);
    rows
}

fn percentage(count: i64, total: i64) -> f64 {
    if total == 0 {
        0.0
    } else {
        ((count as f64 / total as f64) * 1000.0).round() / 10.0
    }
}

fn median(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut values = values.to_vec();
    values.sort_by(f64::total_cmp);
    let middle = values.len() / 2;
    Some(if values.len().is_multiple_of(2) {
        (values[middle - 1] + values[middle]) / 2.0
    } else {
        values[middle]
    })
}

fn first_segment(value: &str) -> String {
    value
        .split('·')
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("未注明")
        .to_string()
}

fn fallback(value: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        "未注明".to_string()
    } else {
        value.to_string()
    }
}

fn date_part(value: &str) -> String {
    value.chars().take(10).collect()
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn render_bars(rows: &[ReportBucket]) -> String {
    if rows.is_empty() {
        return "<p>暂无可统计数据</p>".to_string();
    }
    format!(
        "<div class=\"bars\">{}</div>",
        rows.iter()
            .map(|row| format!(
                "<div class=\"bar\"><span>{}</span><div class=\"track\"><div class=\"fill\" style=\"width:{:.1}%\"></div></div><span class=\"value\">{} · {:.1}%</span></div>",
                escape_html(&row.label), row.percentage.clamp(0.0, 100.0), row.count, row.percentage
            ))
            .collect::<Vec<_>>()
            .join("")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ai_role_marker_supports_chinese_titles_without_matching_substrings() {
        assert_eq!(classify_role("AI工程师"), "AI / Agent 开发");
        assert_eq!(classify_role("AI 应用开发"), "AI / Agent 开发");
        assert_eq!(classify_role("Paid Media Specialist"), "其他岗位");
        assert_eq!(classify_role("Rail Platform Engineer"), "其他岗位");
    }

    fn sample_job(id: &str, salary: &str, skills: &[&str]) -> Job {
        Job {
            id: id.into(),
            source: "boss".into(),
            external_id: id.into(),
            title: "AI Agent 工程师".into(),
            company: format!("公司{id}"),
            salary: salary.into(),
            location: "上海·浦东新区".into(),
            experience: "3-5年".into(),
            degree: "本科".into(),
            company_scale: "100-499人".into(),
            company_stage: "B轮".into(),
            industry: "人工智能".into(),
            skills: skills.iter().map(|value| (*value).to_string()).collect(),
            welfare: vec!["五险一金".into()],
            description: "负责 RAG、工具调用与 Docker 部署".into(),
            source_url: String::new(),
            boss_name: None,
            boss_title: None,
            first_seen: "2026-07-01T09:00:00+08:00".into(),
            last_seen: "2026-07-11T09:00:00+08:00".into(),
            is_new: true,
            fit: None,
            greeting: None,
            patches: vec![],
            structured_details: None,
        }
    }

    #[test]
    fn parses_monthly_salary_and_bonus_months() {
        assert_eq!(
            parse_salary("20-30K·15薪"),
            Some((20.0, 25.0, 30.0, Some(15)))
        );
        assert_eq!(parse_salary("薪资面议"), None);
    }

    #[test]
    fn aggregates_all_jobs_and_skill_pairs() {
        let report = build_report_for_keywords(
            &[
                sample_job("1", "20-30K·15薪", &["Python", "RAG"]),
                sample_job("2", "30-40K", &["Python", "LangChain"]),
            ],
            vec![],
        );
        assert_eq!(report.total_jobs, 2);
        assert_eq!(report.total_companies, 2);
        assert_eq!(report.salary.median_mid_k, Some(30.0));
        assert_eq!(report.salary.extra_months_count, 1);
        assert_eq!(report.top_skills[0].label, "Docker");
        assert_eq!(report.top_skills[0].count, 2);
        assert!(report
            .skill_pairs
            .iter()
            .any(|item| item.label.contains('×')));
    }

    fn sample_run(id: &str, completed_at: &str, jobs: &[Job], spec: &SearchSpec) -> ScrapeRun {
        ScrapeRun {
            id: id.into(),
            keyword: spec.keyword.clone(),
            city: spec.city.clone(),
            total_seen: jobs.len() as i64,
            inserted: jobs.len() as i64,
            updated: 0,
            started_at: completed_at.into(),
            completed_at: Some(completed_at.into()),
            report_markdown: None,
            search_spec: Some(spec.clone()),
            resolved_city: Some(spec.city.clone()),
            detail_summary: None,
            sample: Some(build_scrape_sample(jobs)),
        }
    }

    #[test]
    fn compares_latest_identical_cross_day_batches() {
        let spec = SearchSpec {
            keyword: "AI Agent".into(),
            city: "上海".into(),
            pages: 3,
            salary: Some("20-40K".into()),
            experience: Some("3-5年".into()),
            degree: Some("本科".into()),
            company_scale: None,
        };
        let previous = vec![
            sample_job("shared", "20-30K", &["Python"]),
            sample_job("previous-only", "10-20K", &["Python"]),
        ];
        let current = vec![
            sample_job("shared", "30-40K", &["Python", "RAG"]),
            sample_job("current-new-1", "40-50K", &["RAG"]),
            sample_job("current-new-2", "50-60K", &["RAG"]),
        ];
        let runs = vec![
            sample_run("current", "2026-07-16T10:00:00+08:00", &current, &spec),
            sample_run("previous", "2026-07-15T10:00:00+08:00", &previous, &spec),
        ];
        let report = build_report_for_keywords_at(
            &current,
            vec![ReportKeyword {
                key: "ai agent".into(),
                label: "AI Agent".into(),
                job_count: 3,
                last_seen: "2026-07-16T10:00:00+08:00".into(),
            }],
            &runs,
            DateTime::parse_from_rfc3339("2026-07-16T12:00:00+08:00").unwrap(),
        );
        let comparison = report.batch_comparison;
        assert_eq!(comparison.status, "available");
        assert_eq!(comparison.job_count_change_percentage, Some(50.0));
        assert_eq!(comparison.newly_observed_jobs, 2);
        assert_eq!(comparison.not_observed_jobs, 1);
        assert_eq!(comparison.salary_median_delta_k, Some(25.0));
        assert!(comparison
            .skill_changes
            .iter()
            .any(|item| item.label == "RAG / 检索增强"));
    }

    #[test]
    fn reports_quality_limits_and_non_comparable_scopes() {
        let mut job = sample_job("sparse", "面议", &[]);
        job.description.clear();
        job.experience.clear();
        job.degree.clear();
        let report = build_report_for_keywords(
            &[job],
            vec![
                ReportKeyword {
                    key: "ai-agent".into(),
                    label: "AI Agent".into(),
                    job_count: 1,
                    last_seen: "2026-07-16".into(),
                },
                ReportKeyword {
                    key: "data".into(),
                    label: "数据".into(),
                    job_count: 1,
                    last_seen: "2026-07-16".into(),
                },
            ],
        );
        assert_eq!(
            report.batch_comparison.reason.as_deref(),
            Some("multi_keyword")
        );
        assert_eq!(report.sample_quality.detail.coverage, 0.0);
        assert!(report
            .sample_quality
            .limitations
            .iter()
            .any(|item| item.contains("有限页 BOSS")));
        assert!(report
            .sample_quality
            .limitations
            .iter()
            .any(|item| item.contains("少于 20")));
    }

    #[test]
    fn batch_comparison_rejects_same_day_changed_scope_and_legacy_runs() {
        let keywords = vec![ReportKeyword {
            key: "ai agent".into(),
            label: "AI Agent".into(),
            job_count: 1,
            last_seen: "2026-07-16".into(),
        }];
        assert_eq!(
            build_batch_comparison(&keywords, &[]).reason.as_deref(),
            Some("no_captured_run")
        );
        let spec = SearchSpec {
            keyword: "AI Agent".into(),
            city: "上海".into(),
            pages: 2,
            salary: None,
            experience: None,
            degree: None,
            company_scale: None,
        };
        let jobs = vec![sample_job("one", "20-30K", &["Python"])];
        let current = sample_run("current", "2026-07-16T12:00:00+08:00", &jobs, &spec);
        let same_day = sample_run("same-day", "2026-07-16T08:00:00+08:00", &jobs, &spec);
        assert_eq!(
            build_batch_comparison(&keywords, &[current.clone(), same_day])
                .reason
                .as_deref(),
            Some("no_comparable_run")
        );

        let mut changed_spec = spec.clone();
        changed_spec.pages = 3;
        let changed = sample_run("changed", "2026-07-15T08:00:00+08:00", &jobs, &changed_spec);
        assert_eq!(
            build_batch_comparison(&keywords, &[current.clone(), changed])
                .reason
                .as_deref(),
            Some("no_comparable_run")
        );

        let mut legacy = current;
        legacy.id = "legacy".into();
        legacy.sample = None;
        assert_eq!(
            build_batch_comparison(&keywords, &[legacy])
                .reason
                .as_deref(),
            Some("no_captured_run")
        );
    }

    #[test]
    fn exported_report_is_utf8_and_self_contained() {
        let report = build_report_for_keywords(&[sample_job("1", "20-30K", &["Python"])], vec![]);
        let html = append_decision_sections(render_html(&report), &report, None);
        assert!(html.contains("<meta charset=\"utf-8\">"));
        assert!(html.contains("岗位数据报告"));
        assert!(html.contains("上海"));
        assert!(html.contains("本机保存的有限页 BOSS 样本"));
        assert!(html.contains("同条件样本对比"));
        assert!(html.contains("不会为了生成批次对比而自动或重复访问 BOSS"));
        assert!(!html.contains("本次岗位 · 较上次"));
    }

    #[test]
    fn java_does_not_match_javascript() {
        assert!(!matches_alias("javascript typescript", "java"));
        assert!(matches_alias("java spring boot", "java"));
    }
}
