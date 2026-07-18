<script lang="ts">
  export let score = 0;
  export let size: 'sm' | 'md' | 'lg' = 'md';
  const tone = (value: number) =>
    value >= 75
      ? 'var(--success)'
      : value >= 60
        ? 'var(--blue)'
        : value >= 45
          ? 'var(--warning)'
          : 'var(--danger)';
  $: diameter = size === 'lg' ? 96 : size === 'sm' ? 42 : 58;
  $: stroke = size === 'lg' ? 8 : size === 'sm' ? 4 : 5;
  $: radius = (diameter - stroke) / 2;
  $: circumference = Math.PI * 2 * radius;
  $: offset = circumference * (1 - score / 100);
</script>

<div
  class="relative grid shrink-0 place-items-center"
  style={`width:${diameter}px;height:${diameter}px;color:${tone(score)}`}
  aria-label={`匹配度 ${score} 分`}
>
  <svg class="absolute inset-0 -rotate-90" width={diameter} height={diameter} aria-hidden="true">
    <circle
      cx={diameter / 2}
      cy={diameter / 2}
      r={radius}
      fill="none"
      stroke="var(--panel-soft)"
      stroke-width={stroke}
    />
    <circle
      cx={diameter / 2}
      cy={diameter / 2}
      r={radius}
      fill="none"
      stroke="currentColor"
      stroke-width={stroke}
      stroke-linecap="round"
      stroke-dasharray={circumference}
      stroke-dashoffset={offset}
    />
  </svg>
  <span
    class:large={size === 'lg'}
    class:small={size === 'sm'}
    class="font-semibold tracking-[-0.04em]">{score}<span class="unit">%</span></span
  >
</div>

<style>
  span {
    font-size: 16px;
  }
  span.large {
    font-size: 27px;
  }
  span.small {
    font-size: 12px;
  }
  .unit {
    font-size: 0.58em;
    margin-left: 1px;
  }
</style>
