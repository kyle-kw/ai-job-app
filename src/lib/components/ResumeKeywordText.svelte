<script lang="ts">
  export let text = '';
  export let highlightKeywords: string[] = [];

  const keywords = [
    'PaddleOCR-VL-1.6',
    'PP-StructureV3',
    'Docker Compose',
    'Prometheus',
    'PostgreSQL',
    'PP-OCRv6',
    'FastAPI',
    'Grafana',
    'llama.cpp',
    'OpenAI',
    'SGLang',
    'Docker',
    'Linux',
    'Milvus',
    'MinerU',
    'Triton',
    'Dify',
    'vLLM'
  ];
  const escape = (value: string) => value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const keywordSet = new Set(keywords.map((keyword) => keyword.toLocaleLowerCase()));
  $: highlightSet = new Set(
    highlightKeywords.map((keyword) => keyword.trim().toLocaleLowerCase()).filter(Boolean)
  );
  $: allKeywords = [
    ...new Set([...keywords, ...highlightKeywords.map((item) => item.trim()).filter(Boolean)])
  ].sort((a, b) => b.length - a.length);
  $: pattern = allKeywords.length
    ? new RegExp(`(${allKeywords.map(escape).join('|')})`, 'gi')
    : null;
  $: parts = (pattern ? text.split(pattern) : [text]).filter(Boolean).map((value) => ({
    value,
    bold: keywordSet.has(value.toLocaleLowerCase()),
    highlighted: highlightSet.has(value.toLocaleLowerCase())
  }));
</script>

{#each parts as part}
  {#if part.highlighted}<mark class:font-semibold={part.bold}>{part.value}</mark
    >{:else if part.bold}<strong>{part.value}</strong>{:else}{part.value}{/if}
{/each}

<style>
  mark {
    border-radius: 2px;
    background: color-mix(in srgb, var(--resume-accent) 14%, transparent);
    color: inherit;
    box-shadow: inset 0 -1px 0 color-mix(in srgb, var(--resume-accent) 48%, transparent);
  }
</style>
