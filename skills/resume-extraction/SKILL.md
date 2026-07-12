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
7. Treat `rawText` as the source whether it came from a PDF text layer, DOCX, or page-by-page visual transcription. Page markers such as `--- Page 2 ---` are source identifiers, not resume content.
8. Keep every distinct job, project, and education record as a separate array item. Never merge employers, schools, degrees, or date ranges.
9. Split every date range into `startDate` and `endDate`. For `2024.12 - 至今`, return `startDate: "2024.12"` and `endDate: "至今"`. Never place a full range in one field and never add a leading dash.
10. Normalize degrees to `本科`, `硕士`, `博士`, or `其他`. For any other degree, set `degree` to `其他` and preserve the exact source wording in `degreeDetail`.
11. Every fact created during import must use `confirmed: false`; only the user can confirm imported facts.

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
  "templateId": "ai-engineering",
  "professionalSkills": [{
    "id": "",
    "label": "核心方向",
    "items": [""]
  }],
  "experiences": [{
    "id": "",
    "company": "",
    "position": "",
    "location": "",
    "startDate": "",
    "endDate": "",
    "highlights": [""]
  }],
  "education": [{
    "id": "",
    "institution": "",
    "area": "",
    "degree": "",
    "degreeDetail": "",
    "startDate": "",
    "endDate": "",
    "highlights": []
  }],
  "projects": [{
    "id": "",
    "name": "",
    "summary": "",
    "startDate": "",
    "endDate": "",
    "highlights": []
  }],
  "certifications": [{
    "id": "",
    "name": "",
    "issuer": "",
    "date": ""
  }],
  "facts": [{
    "id": "fact-stable-id",
    "category": "identity|experience|education|skill|project|certification|other",
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
