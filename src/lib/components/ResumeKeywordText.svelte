<script lang="ts">
  export let text = '';

  const keywords = [
    'PaddleOCR-VL-1.6', 'PP-StructureV3', 'Docker Compose', 'Prometheus', 'PostgreSQL',
    'PP-OCRv6', 'FastAPI', 'Grafana', 'llama.cpp', 'OpenAI', 'SGLang', 'Docker',
    'Linux', 'Milvus', 'MinerU', 'Triton', 'Dify', 'vLLM'
  ];
  const keywordSet = new Set(keywords.map((keyword) => keyword.toLocaleLowerCase()));
  const pattern = new RegExp(`(${keywords.map((keyword) => keyword.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')).join('|')})`, 'gi');

  $: parts = text.split(pattern).filter(Boolean).map((value) => ({
    value,
    bold: keywordSet.has(value.toLocaleLowerCase())
  }));
</script>

{#each parts as part}
  {#if part.bold}<strong>{part.value}</strong>{:else}{part.value}{/if}
{/each}
