use crate::models::{Job, JobDataReport, ReportBucket, SalaryByExperience, SalarySummary};
use crate::time;
use std::collections::{BTreeSet, HashMap};

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

pub fn build_report(jobs: &[Job]) -> JobDataReport {
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
    let mut first_dates = vec![];
    let mut last_dates = vec![];

    for job in jobs {
        if !job.company.trim().is_empty() {
            companies.insert(job.company.trim().to_string());
        }
        increment(&mut city_counter, first_segment(&job.location));
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
        insights.push("岗位库暂无数据，完成至少一轮抓取后即可生成全量报告。".to_string());
    } else {
        insights.push(format!(
            "当前岗位库按岗位去重后共有 {total} 个岗位，覆盖 {} 家公司、{} 个城市。",
            companies.len(),
            city_counter.len()
        ));
        if let Some(value) = median(&salary_mids) {
            insights.push(format!(
                "可解析薪资的岗位有 {} 个，月薪区间中点中位数为 {:.1}K。",
                salary_mids.len(),
                value
            ));
        }
        if !top_skills.is_empty() {
            insights.push(format!(
                "最常出现的技能是 {}。",
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
                "经验要求以“{}”为主，占全部岗位的 {:.1}%。",
                item.label, item.percentage
            ));
        }
    }

    first_dates.sort();
    last_dates.sort();
    JobDataReport {
        generated_at: time::shanghai_rfc3339(),
        data_from: first_dates.first().map(|value| date_part(value)),
        data_to: last_dates.last().map(|value| date_part(value)),
        total_jobs: total,
        total_companies: companies.len() as i64,
        total_cities: city_counter.len() as i64,
        detail_jobs,
        detail_coverage: percentage(detail_jobs, total),
        salary: SalarySummary {
            sample_count: salary_mids.len() as i64,
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
    }
}

pub fn render_html(report: &JobDataReport) -> String {
    let insights = report
        .insights
        .iter()
        .map(|item| format!("<li>{}</li>", escape_html(item)))
        .collect::<Vec<_>>()
        .join("");
    let period = match (&report.data_from, &report.data_to) {
        (Some(from), Some(to)) => format!("{} 至 {}", escape_html(from), escape_html(to)),
        _ => "暂无时间范围".to_string(),
    };
    format!(
        r#"<!doctype html><html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>全量岗位数据报告</title><style>
:root{{font-family:Inter,"PingFang SC","Microsoft YaHei",sans-serif;color:#18302a;background:#f4f6f2}}*{{box-sizing:border-box}}body{{margin:0}}main{{max-width:1200px;margin:auto;padding:40px 24px 80px}}header{{padding:34px;border-radius:24px;background:#176b57;color:#fff}}h1{{margin:0 0 10px;font-size:34px}}header p{{margin:0;opacity:.82}}.kpis{{display:grid;grid-template-columns:repeat(4,1fr);gap:14px;margin:20px 0}}.card,.section{{background:#fff;border:1px solid #dfe4de;border-radius:18px}}.card{{padding:18px}}.card strong{{display:block;font-size:27px}}.card span{{color:#68736e;font-size:13px}}.section{{padding:24px;margin-top:18px}}h2{{font-size:20px;margin:0 0 18px}}.grid{{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:20px}}.bars{{display:grid;gap:10px}}.bar{{display:grid;grid-template-columns:150px 1fr 88px;gap:10px;align-items:center;font-size:13px}}.track{{height:9px;background:#edf1ed;border-radius:99px;overflow:hidden}}.fill{{height:100%;background:#2d8b70;border-radius:99px}}.value{{text-align:right;color:#68736e}}ul{{margin:0;padding-left:20px;line-height:1.8}}.meta{{margin-top:10px;font-size:12px;color:#68736e}}@media(max-width:760px){{.kpis,.grid{{grid-template-columns:1fr 1fr}}.bar{{grid-template-columns:110px 1fr 74px}}}}@media(max-width:520px){{.kpis,.grid{{grid-template-columns:1fr}}}}
</style></head><body><main><header><h1>全量岗位数据报告</h1><p>基于本地 SQLite 中全部去重岗位 · {period}</p></header><section class="kpis"><div class="card"><strong>{jobs}</strong><span>有效岗位样本</span></div><div class="card"><strong>{companies}</strong><span>招聘公司</span></div><div class="card"><strong>{salary}</strong><span>月薪中点中位数</span></div><div class="card"><strong>{coverage:.1}%</strong><span>岗位详情覆盖率</span></div></section><section class="section"><h2>先看结论</h2><ul>{insights}</ul></section><div class="grid"><section class="section"><h2>高频技能</h2>{skills}</section><section class="section"><h2>技能共现组合</h2>{pairs}</section><section class="section"><h2>经验要求</h2>{experience}</section><section class="section"><h2>学历要求</h2>{degree}</section><section class="section"><h2>薪资分布</h2>{salary_bands}</section><section class="section"><h2>岗位方向</h2>{roles}</section><section class="section"><h2>城市分布</h2>{cities}</section><section class="section"><h2>行业分布</h2>{industries}</section><section class="section"><h2>公司规模</h2>{scales}</section><section class="section"><h2>常见福利</h2>{welfare}</section></div><p class="meta">生成时间：{generated}（Asia/Shanghai） · 按岗位去重计数 · 文件编码 UTF-8</p></main></body></html>"#,
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
        &["agent", "智能体", "大模型", "llm", "rag", "人工智能", "ai "],
    ) {
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
    Some(if values.len() % 2 == 0 {
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
        let report = build_report(&[
            sample_job("1", "20-30K·15薪", &["Python", "RAG"]),
            sample_job("2", "30-40K", &["Python", "LangChain"]),
        ]);
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

    #[test]
    fn exported_report_is_utf8_and_self_contained() {
        let html = render_html(&build_report(&[sample_job("1", "20-30K", &["Python"])]));
        assert!(html.contains("<meta charset=\"utf-8\">"));
        assert!(html.contains("全量岗位数据报告"));
        assert!(html.contains("上海"));
    }

    #[test]
    fn java_does_not_match_javascript() {
        assert!(!matches_alias("javascript typescript", "java"));
        assert!(matches_alias("java spring boot", "java"));
    }
}
