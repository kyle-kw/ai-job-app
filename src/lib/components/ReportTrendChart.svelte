<script lang="ts">
  import type { ReportTrendPoint } from '$lib/types';

  export let points: ReportTrendPoint[] = [];

  const plotTop = 8;
  const plotBottom = 92;
  const plotHeight = plotBottom - plotTop;
  let activeIndex: number | null = null;

  function niceStep(value: number): number {
    const rough = Math.max(1, value) / 3;
    const magnitude = 10 ** Math.floor(Math.log10(rough));
    const normalized = rough / magnitude;
    const factor = normalized <= 1 ? 1 : normalized <= 2 ? 2 : normalized <= 5 ? 5 : 10;
    return factor * magnitude;
  }

  function shortDate(value: string): string {
    const match = value.match(/^\d{4}-(\d{2})-(\d{2})$/);
    return match ? `${Number(match[1])}月${Number(match[2])}日` : value;
  }

  function labelIndexes(length: number): Set<number> {
    if (length <= 7) return new Set(Array.from({ length }, (_, index) => index));
    return new Set([0, Math.round((length - 1) / 4), Math.round((length - 1) / 2), Math.round((length - 1) * 3 / 4), length - 1]);
  }

  function tooltipShift(x: number): string {
    if (x < 14) return '0%';
    if (x > 86) return '-100%';
    return '-50%';
  }

  $: maximum = Math.max(0, ...points.map((point) => point.count));
  $: step = niceStep(maximum);
  $: chartMaximum = maximum > 0 ? Math.max(step * 3, Math.ceil(maximum / step) * step) : 1;
  $: total = points.reduce((sum, point) => sum + point.count, 0);
  $: average = points.length ? total / points.length : 0;
  $: peakIndex = maximum > 0 ? points.findIndex((point) => point.count === maximum) : -1;
  $: coordinates = points.map((point, index) => ({
    ...point,
    x: points.length <= 1 ? 50 : 1.5 + index / (points.length - 1) * 97,
    y: plotBottom - point.count / chartMaximum * plotHeight
  }));
  $: polyline = coordinates.map((point) => `${point.x},${point.y}`).join(' ');
  $: area = coordinates.length ? `1.5,${plotBottom} ${polyline} 98.5,${plotBottom}` : '';
  $: gridLines = Array.from({ length: 4 }, (_, index) => ({
    y: plotTop + index / 3 * plotHeight,
    value: Math.round(chartMaximum * (3 - index) / 3)
  }));
  $: visibleLabels = labelIndexes(points.length);
  $: activePoint = activeIndex == null ? null : coordinates[activeIndex];
</script>

<div class="trend-chart" aria-label={`最近 ${points.length} 天每日新增岗位趋势`}>
  <div class="chart-layout">
    <div class="y-axis" aria-hidden="true">
      {#each gridLines as line}<span style={`top:${line.y}%`}>{line.value}</span>{/each}
    </div>

    <div class="plot" role="group" aria-label="每日新增岗位数据点" on:mouseleave={() => activeIndex = null}>
      <svg viewBox="0 0 100 100" preserveAspectRatio="none" aria-hidden="true">
        {#each gridLines as line}
          <line x1="0" y1={line.y} x2="100" y2={line.y} class="grid-line" vector-effect="non-scaling-stroke" />
        {/each}
        {#if coordinates.length > 0}
          <polygon points={area} class="area" />
          <polyline points={polyline} class="line" vector-effect="non-scaling-stroke" />
        {/if}
      </svg>

      {#if total === 0}
        <div class="empty-state"><strong>当前窗口暂无新增岗位</strong><span>有新样本后会在这里显示每日变化</span></div>
      {:else}
        {#each coordinates as point, index}
          <button
            type="button"
            class:active={activeIndex === index}
            class:peak={index === peakIndex}
            class="point"
            style={`left:${point.x}%; top:${point.y}%`}
            aria-label={`${shortDate(point.date)}新增 ${point.count} 个岗位`}
            on:mouseenter={() => activeIndex = index}
            on:focus={() => activeIndex = index}
            on:blur={() => activeIndex = null}
            on:click={() => activeIndex = activeIndex === index ? null : index}
          ><span></span></button>
        {/each}
      {/if}

      {#if activePoint}
        <div
          class="tooltip"
          role="status"
          style={`left:${activePoint.x}%; top:${Math.max(2, activePoint.y - 5)}%; --tooltip-shift:${tooltipShift(activePoint.x)}`}
        >
          <span>{shortDate(activePoint.date)}</span>
          <strong>{activePoint.count} 个新增岗位</strong>
        </div>
      {/if}
    </div>
  </div>

  <div class="x-axis" style={`grid-template-columns:repeat(${Math.max(1, points.length)}, minmax(0, 1fr))`} aria-hidden="true">
    {#each points as point, index}<span>{visibleLabels.has(index) ? shortDate(point.date) : ''}</span>{/each}
  </div>

  <div class="summary">
    <span>窗口合计 <strong>{total}</strong> 个</span>
    <span class="divider"></span>
    <span>日均 <strong>{average.toFixed(1)}</strong> 个</span>
    {#if peakIndex >= 0}
      <span class="divider"></span>
      <span>峰值 <strong>{maximum}</strong> 个 · {shortDate(points[peakIndex].date)}</span>
    {/if}
  </div>
</div>

<style>
  .trend-chart {
    display: flex;
    flex: 1 1 auto;
    min-height: 0;
    flex-direction: column;
    border: 1px solid color-mix(in srgb, var(--line) 72%, transparent);
    border-radius: 0.9rem;
    background: linear-gradient(180deg, color-mix(in srgb, var(--panel) 76%, var(--brand-faint)) 0%, var(--panel) 100%);
    padding: 0.9rem 1rem 0.75rem;
  }

  .chart-layout {
    display: grid;
    flex: 1 1 auto;
    grid-template-columns: 2rem minmax(0, 1fr);
    gap: 0.55rem;
    min-height: 10.75rem;
  }

  .y-axis,
  .plot {
    position: relative;
    min-height: 10.75rem;
  }

  .y-axis span {
    position: absolute;
    right: 0;
    color: var(--muted);
    font-size: 0.62rem;
    font-variant-numeric: tabular-nums;
    line-height: 1;
    transform: translateY(-50%);
  }

  .plot {
    min-width: 0;
  }

  svg {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    overflow: visible;
  }

  .grid-line {
    stroke: color-mix(in srgb, var(--line) 76%, transparent);
    stroke-dasharray: 3 5;
    stroke-width: 1;
  }

  .area {
    fill: var(--brand);
    opacity: 0.1;
  }

  .line {
    fill: none;
    stroke: var(--brand);
    stroke-linecap: round;
    stroke-linejoin: round;
    stroke-width: 2.5;
  }

  .point {
    position: absolute;
    z-index: 2;
    width: 1.5rem;
    height: 1.5rem;
    border: 0;
    border-radius: 999px;
    background: transparent;
    cursor: pointer;
    transform: translate(-50%, -50%);
  }

  .point span {
    position: absolute;
    inset: 50% auto auto 50%;
    width: 0.46rem;
    height: 0.46rem;
    border: 2px solid var(--panel);
    border-radius: 999px;
    background: var(--brand);
    box-shadow: 0 0 0 1px var(--brand);
    transform: translate(-50%, -50%);
    transition: width 120ms ease, height 120ms ease, box-shadow 120ms ease;
  }

  .point:hover span,
  .point:focus-visible span,
  .point.active span,
  .point.peak span {
    width: 0.62rem;
    height: 0.62rem;
    box-shadow: 0 0 0 1px var(--brand), 0 0 0 5px color-mix(in srgb, var(--brand) 13%, transparent);
  }

  .point:focus-visible {
    outline: 2px solid var(--brand);
    outline-offset: 1px;
  }

  .tooltip {
    --tooltip-shift: -50%;
    position: absolute;
    z-index: 4;
    display: grid;
    min-width: 7.5rem;
    gap: 0.15rem;
    border: 1px solid color-mix(in srgb, var(--brand) 24%, var(--line));
    border-radius: 0.65rem;
    background: var(--panel);
    box-shadow: 0 10px 28px rgba(20, 54, 45, 0.13);
    padding: 0.48rem 0.62rem;
    pointer-events: none;
    transform: translate(var(--tooltip-shift), -100%);
  }

  .tooltip span {
    color: var(--muted);
    font-size: 0.62rem;
  }

  .tooltip strong {
    color: var(--ink);
    font-size: 0.72rem;
  }

  .empty-state {
    position: absolute;
    inset: 0;
    display: grid;
    place-content: center;
    gap: 0.25rem;
    text-align: center;
  }

  .empty-state strong { color: var(--ink); font-size: 0.76rem; }
  .empty-state span { color: var(--muted); font-size: 0.65rem; }

  .x-axis {
    display: grid;
    margin-left: 2.55rem;
    margin-top: 0.15rem;
  }

  .x-axis span {
    min-width: 0;
    color: var(--muted);
    font-size: 0.6rem;
    text-align: center;
    white-space: nowrap;
  }

  .x-axis span:first-child { text-align: left; }
  .x-axis span:last-child { text-align: right; }

  .summary {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.45rem;
    border-top: 1px solid color-mix(in srgb, var(--line) 70%, transparent);
    margin-top: 0.65rem;
    padding-top: 0.65rem;
    color: var(--muted);
    font-size: 0.64rem;
  }

  .summary strong { color: var(--ink); font-weight: 650; }
  .divider { width: 1px; height: 0.7rem; background: var(--line); }

  @media (prefers-reduced-motion: reduce) {
    .point span { transition: none; }
  }
</style>
