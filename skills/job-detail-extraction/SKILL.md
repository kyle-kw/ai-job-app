---
name: job-detail-extraction
description: Extract clean, source-grounded job, company, and business-registration fields from a noisy BOSS job-detail page captured as plain text.
---

# 岗位详情结构化提取

你会收到一条 BOSS 岗位的基础元数据和 `rawDetailText`。原文可能同时包含职位正文、招聘者姓名与活跃状态、竞争力分析、安全提示、公司介绍、工商信息、工作地址、推荐职位、城市导航等页面噪声。

## 提取目标

只依据 `rawDetailText`，提取以下五类信息：

1. `jobDescription`：职位概述或未归入职责、要求的岗位正文。不要复制已经放入 `responsibilities` 或 `requirements` 的列表；若原文没有独立概述则返回空字符串。
2. `responsibilities`：岗位职责、主要职责、工作内容、What you will do 等段落，按原文中的独立要点拆为字符串数组。
3. `requirements`：任职要求、职位要求、任职资格、Requirements、Qualifications、加分项等段落，按独立要点拆为字符串数组；加分项需保留“加分项：”前缀。
4. `companyIntroduction`：仅提取明确位于“公司介绍/公司简介/About the company”等标题下的内容。
5. `businessInformation`：仅提取明确位于“工商信息”段落、且紧跟对应标签出现的值。

## 实际页面切分规则

- 标题可能使用中文括号、方括号、冒号、中英文或不同写法，例如“【岗位职责】”“主要职责”“职位要求”“Requirements / 任职要求”。按语义识别，不依赖单一固定标题。
- “职位描述”通常只是整个职位区的总标题，不等于所有后续页面文本。
- 遇到“公司介绍”后，后续内容不得再归入职位描述、岗位职责或任职要求。
- 遇到“工商信息”后，只读取工商字段，直到“查看全部”“工作地址”“点击查看地图”“更多职位”等边界。
- 删除“微信扫码分享”“举报”、技能标签堆叠、招聘者姓名/职位/活跃状态、竞争力分析、BOSS 安全提示、工作地址、地图、推荐职位、精选职位、搜索和城市导航等噪声。
- 保留原文事实、数字、技术名词、学历、年限和限定条件；只做去噪、断句与最小必要的格式整理，不扩写、不评价。
- 去除重复要点，但不要把两个不同条件合并成一个模糊总结。

## 事实完整性规则

1. 严禁根据公司名、行业常识、基础元数据或模型知识补全原文没有的信息。
2. 缺失字段必须返回空字符串或空数组，不能返回“未知”“暂无”“未提供”等占位文字。
3. 工商字段标签存在但值为空、为 `-`、被截断或无法和标签可靠配对时，返回空字符串。
4. 不要把招聘者姓名误识别为法定代表人，不要把岗位发布日期误识别为成立日期。
5. 不要把公司规模、融资阶段、行业、工作地址写入工商信息，除非输出合约中存在对应字段；当前合约不存在这些字段。
6. 输出必须是一个 JSON 对象，不得包含 Markdown、注释或额外说明。

## 输出合约

```json
{
  "jobDescription": "",
  "responsibilities": [""],
  "requirements": [""],
  "companyIntroduction": "",
  "businessInformation": {
    "companyName": "",
    "legalRepresentative": "",
    "establishedDate": "",
    "companyType": "",
    "operatingStatus": "",
    "registeredCapital": ""
  },
  "extractedAt": "",
  "extractorVersion": "job-detail-extraction@1.0.0"
}
```

`extractedAt` 可返回空字符串，应用会在保存时写入本地时间；`extractorVersion` 必须使用合约中的固定值。
