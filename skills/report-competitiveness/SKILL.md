---
name: report-competitiveness
description: Classify how a trusted master resume covers a fixed list of aggregated market skills using verifiable resume paths and confirmed fact IDs.
---

# Report Competitiveness

Evaluate only the supplied skill IDs. Use Simplified Chinese for every rationale.

## Integrity rules

1. `covered` requires direct semantic evidence in one or more supplied resume paths.
2. `strengthenable` requires a supplied confirmed fact ID and means that fact is not clearly expressed in the resume body.
3. `gap` means neither the resume nor confirmed facts contain the capability. Never turn market demand into candidate experience.
4. Use `unknown` whenever evidence is ambiguous or insufficient.
5. Return only supplied skill IDs, resume paths, and confirmed fact IDs. Return JSON only.

## Output contract

```json
{
  "items": [{
    "id": "supplied skill id",
    "status": "covered|strengthenable|gap|unknown",
    "resumePaths": ["/summary"],
    "evidenceFactIds": ["fact id"],
    "rationale": "简体中文证据说明"
  }]
}
```
