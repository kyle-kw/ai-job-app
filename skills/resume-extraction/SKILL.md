---
name: resume-extraction
description: Extract a PDF, DOCX, pasted resume, or RenderCV YAML into a structured RenderCV-compatible candidate profile with source-backed facts and confidence. Use when importing, normalizing, or validating a resume before job matching or tailoring.
---

# Resume Extraction

Convert the supplied resume text into the JSON contract below. Treat the resume as the only source of biographical truth.

## Rules

1. Preserve names, employers, dates, degrees, metrics, links, and technologies exactly unless normalizing whitespace.
2. Never infer an employer, date, metric, responsibility, credential, or skill that is not supported by the source.
3. Put uncertain text in the closest field, lower its fact confidence, and set `confirmed` to `false`.
4. Keep accomplishment bullets concrete and concise. Do not turn ordinary responsibilities into invented achievements.
5. Reuse the supplied fallback profile when extraction is incomplete.
6. Return JSON only. Do not wrap it in Markdown.

## Output contract

Return one `ResumeProfile` object:

```json
{
  "id": "resume-master",
  "name": "",
  "headline": "",
  "email": "",
  "phone": "",
  "location": "",
  "website": "",
  "summary": "",
  "skills": [""],
  "experiences": [{
    "company": "",
    "position": "",
    "location": "",
    "startDate": "",
    "endDate": "",
    "highlights": [""]
  }],
  "education": [{
    "institution": "",
    "area": "",
    "degree": "",
    "startDate": "",
    "endDate": "",
    "highlights": []
  }],
  "facts": [{
    "id": "fact-stable-id",
    "category": "identity|experience|education|skill|project|other",
    "value": "",
    "source": "section and source excerpt identifier",
    "confidence": 0.0,
    "confirmed": false
  }],
  "preferences": {
    "targetRoles": [],
    "cities": [],
    "remotePreference": "flexible",
    "energizingTasks": [],
    "drainingTasks": [],
    "hardConstraints": []
  },
  "sourceFileName": "",
  "updatedAt": "ISO-8601",
  "version": 1
}
```

Use a confidence of at least `0.9` only for directly readable source claims. Keep user preferences empty; they are collected separately.
