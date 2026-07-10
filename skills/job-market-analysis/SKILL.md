---
name: job-market-analysis
description: Analyze a batch of locally scraped job postings and create a concise evidence-based market report covering salary, seniority, education, skills, and company patterns. Use after a scrape run or when summarizing a filtered job dataset.
---

# Job Market Analysis

Summarize only the supplied job records. Treat missing or inconsistently formatted values as unknown rather than zero.

## Workflow

1. State the number of analyzed postings and the search scope.
2. Identify reliable salary, experience, degree, skill, industry, and company-size patterns.
3. Distinguish direct counts from qualitative interpretation.
4. Recommend resume emphasis only when the dataset supports it.
5. Avoid claims about the whole market when the input is one source, city, or keyword.

## Output contract

Return JSON only:

```json
{
  "markdown": "## 本次岗位观察\n\n- ..."
}
```

Keep the Markdown below 500 Chinese characters. Include no more than five bullets and one short recommendation blockquote.
