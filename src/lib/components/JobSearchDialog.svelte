<script lang="ts">
  import { ArrowLeft, Info, RefreshCw, Search, ShieldCheck, X } from 'lucide-svelte';
  import { POPULAR_BOSS_CITIES, isBossCityName, matchingBossCities } from '$lib/boss-cities';
  import { COMPANY_SCALE_FILTER_OPTIONS, SALARY_FILTER_OPTIONS } from '$lib/job-filters';
  import { modalFocus } from '$lib/modal-focus';
  import type { SearchSpec } from '$lib/types';

  export let open = false;
  export let searchSpec: SearchSpec;
  export let scraping = false;
  export let scrapeTaskRunning = false;
  export let onStart: () => void | Promise<void>;

  let confirming = false;
  let cityQuery = searchSpec.city;
  let cityMenuOpen = false;
  let cityFiltering = false;
  let citySelectionConfirmed = true;
  let activeCityIndex = -1;
  let synchronizedSearchSpec = searchSpec;
  const pageOptions = [1, 2, 3, 4, 5] as const;

  $: cityOptions = cityFiltering ? matchingBossCities(cityQuery) : POPULAR_BOSS_CITIES;
  $: selectedCityValid =
    citySelectionConfirmed &&
    cityQuery.trim() === searchSpec.city &&
    isBossCityName(searchSpec.city);
  $: cityMessage = citySelectionConfirmed
    ? ''
    : cityOptions.length
      ? '请从筛选结果中选择城市。'
      : '未找到匹配城市，请修改关键词。';
  $: keywordMissing = !searchSpec.keyword.trim();
  $: scrapeDuration =
    searchSpec.pages === 3 ? '60 分钟（约 1 小时）' : `${searchSpec.pages * 20} 分钟`;
  $: if (searchSpec !== synchronizedSearchSpec) {
    synchronizedSearchSpec = searchSpec;
    cityQuery = searchSpec.city;
    cityMenuOpen = false;
    cityFiltering = false;
    citySelectionConfirmed = true;
    activeCityIndex = -1;
  }
  $: if (!open) {
    confirming = false;
    cityQuery = searchSpec.city;
    cityMenuOpen = false;
    cityFiltering = false;
    citySelectionConfirmed = true;
    activeCityIndex = -1;
  }
  const close = () => {
    if (scraping) return;
    confirming = false;
    open = false;
  };
  const review = () => {
    if (!keywordMissing && !scrapeTaskRunning && selectedCityValid) confirming = true;
  };
  const openCityMenu = () => {
    cityMenuOpen = true;
    cityFiltering = false;
    activeCityIndex = -1;
  };
  const updateCityQuery = (event: Event) => {
    cityQuery = (event.currentTarget as HTMLInputElement).value;
    cityFiltering = true;
    cityMenuOpen = true;
    citySelectionConfirmed = false;
    const matches = matchingBossCities(cityQuery);
    activeCityIndex = matches.length ? 0 : -1;
  };
  const selectCity = (city: string) => {
    cityQuery = city;
    searchSpec.city = city;
    citySelectionConfirmed = true;
    cityMenuOpen = false;
    cityFiltering = false;
    activeCityIndex = -1;
  };
  const handleCityKeydown = (event: KeyboardEvent) => {
    if (event.key === 'Escape') {
      cityMenuOpen = false;
      activeCityIndex = -1;
      return;
    }
    if (event.key === 'Tab') {
      cityMenuOpen = false;
      activeCityIndex = -1;
      return;
    }
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      cityMenuOpen = true;
      if (!cityOptions.length) return;
      const direction = event.key === 'ArrowDown' ? 1 : -1;
      activeCityIndex =
        activeCityIndex < 0
          ? direction > 0
            ? 0
            : cityOptions.length - 1
          : (activeCityIndex + direction + cityOptions.length) % cityOptions.length;
      return;
    }
    if (event.key === 'Enter' && cityMenuOpen && activeCityIndex >= 0) {
      event.preventDefault();
      selectCity(cityOptions[activeCityIndex]);
    }
  };
</script>

{#if open}
  <button
    class="fixed inset-0 z-40 bg-black/25 backdrop-blur-sm"
    tabindex="-1"
    on:click={close}
    aria-label="关闭抓取新岗位"
  ></button>
  <div
    class="fixed left-1/2 top-1/2 z-50 w-[640px] max-w-[calc(100vw-32px)] -translate-x-1/2 -translate-y-1/2 panel p-6"
    role="dialog"
    aria-modal="true"
    aria-labelledby="job-search-title"
    tabindex="-1"
    use:modalFocus={{ close, canClose: !scraping, initialFocus: '[aria-required="true"]' }}
  >
    <div class="mb-5 flex items-start justify-between">
      <div>
        <p class="eyebrow">BOSS 直聘</p>
        <h3 id="job-search-title" class="mt-1 text-xl font-semibold">
          {confirming ? '抓取前确认' : '抓取新岗位'}
        </h3>
        <p class="mt-1 text-xs body-muted">
          {confirming
            ? '确认登录检查方式与本次抓取范围。'
            : '自动连接专用 Chrome、确认登录、抓详情并去重保存。'}
        </p>
      </div>
      <button class="btn-icon" aria-label="关闭" disabled={scraping} on:click={close}
        ><X size={17} /></button
      >
    </div>
    {#if confirming}
      <div
        class="rounded-2xl border p-5"
        style="border-color: var(--line); background: var(--panel-soft);"
      >
        <div class="flex items-start gap-3">
          <span
            class="grid h-10 w-10 shrink-0 place-items-center rounded-xl bg-brand-soft text-brand"
            ><ShieldCheck size={19} /></span
          >
          <div>
            <p class="text-sm font-semibold">将打开 BOSS 专用 Chrome 检查登录</p>
            <p class="mt-1 text-xs leading-5 body-muted">
              如果登录状态仍有效，验证后会自动开始抓取；如果出现登录界面，请在 5
              分钟内完成登录。应用不会读取或关闭你的日常 Chrome。
            </p>
          </div>
        </div>
        <dl class="mt-5 grid grid-cols-3 gap-3 text-xs">
          <div class="rounded-xl bg-panel p-3">
            <dt class="body-muted">关键词</dt>
            <dd class="mt-1 truncate font-semibold">{searchSpec.keyword.trim()}</dd>
          </div>
          <div class="rounded-xl bg-panel p-3">
            <dt class="body-muted">城市</dt>
            <dd class="mt-1 font-semibold">{searchSpec.city}</dd>
          </div>
          <div class="rounded-xl bg-panel p-3">
            <dt class="body-muted">范围</dt>
            <dd class="mt-1 font-semibold">{searchSpec.pages} 页 · 约 {scrapeDuration}</dd>
          </div>
        </dl>
      </div>
      <div
        class="mt-5 rounded-xl border px-4 py-3"
        style="border-color: color-mix(in srgb, #b7791f 35%, var(--line)); background: var(--warning-soft);"
      >
        <div class="flex items-start gap-2">
          <Info size={16} class="mt-0.5 shrink-0 text-warning" />
          <p class="text-xs leading-5 body-muted">
            验证成功后会自动抓取岗位列表与详情。抓取期间请勿关闭应用；可以切换页面，任务不会中断。
          </p>
        </div>
      </div>
      <div class="mt-5 flex justify-end gap-2">
        <button class="btn" disabled={scraping} on:click={() => (confirming = false)}
          ><ArrowLeft size={15} />返回修改</button
        ><button
          class="btn-primary"
          data-confirm-scrape
          disabled={scraping || scrapeTaskRunning}
          on:click={onStart}
          >{#if scraping}<RefreshCw size={15} class="animate-spin" />正在检查登录{:else}<ShieldCheck
              size={15}
            />检查登录并开始抓取{/if}</button
        >
      </div>
    {:else}
      <div class="grid grid-cols-2 gap-4">
        <label
          ><span class="label">关键词</span><input
            class="input"
            bind:value={searchSpec.keyword}
            placeholder="例如：数据分析、财务会计"
            required
            aria-required="true"
          /><span class="mt-1 block min-h-[16px] text-[11px] text-warning"
            >{keywordMissing ? '请输入岗位关键词后开始抓取。' : ''}</span
          ></label
        >
        <label
          ><span class="label">城市</span>
          <div class="relative">
            <input
              class="input"
              role="combobox"
              aria-autocomplete="list"
              aria-expanded={cityMenuOpen}
              aria-controls="boss-city-options"
              aria-activedescendant={cityMenuOpen && activeCityIndex >= 0
                ? `boss-city-option-${activeCityIndex}`
                : undefined}
              aria-invalid={!selectedCityValid}
              aria-describedby="boss-city-message"
              value={cityQuery}
              on:focus={openCityMenu}
              on:input={updateCityQuery}
              on:keydown={handleCityKeydown}
              on:blur={() => {
                cityMenuOpen = false;
                activeCityIndex = -1;
              }}
            />{#if cityMenuOpen}<div
                id="boss-city-options"
                class="absolute left-0 right-0 top-full z-20 mt-1 max-h-52 overflow-y-auto rounded-xl border bg-panel p-1 shadow-xl"
                style="border-color: var(--line);"
                role="listbox"
                aria-label="城市选项"
              >
                {#if cityOptions.length}{#each cityOptions as city, index}<button
                      id={`boss-city-option-${index}`}
                      type="button"
                      role="option"
                      aria-selected={index === activeCityIndex}
                      class="block w-full rounded-lg px-3 py-2 text-left text-sm hover:bg-brand-soft"
                      class:bg-brand-soft={index === activeCityIndex}
                      tabindex="-1"
                      on:mousedown|preventDefault={() => selectCity(city)}
                      on:click={() => selectCity(city)}>{city}</button
                    >{/each}{:else}<p class="px-3 py-2 text-xs body-muted">未找到匹配城市</p>{/if}
              </div>{/if}
          </div>
          <span id="boss-city-message" class="mt-1 block min-h-[16px] text-[11px] text-warning"
            >{cityMessage}</span
          ></label
        >
        <label
          ><span class="label">抓取页数</span><select class="select" bind:value={searchSpec.pages}
            >{#each pageOptions as pages}<option value={pages}
                >{pages} 页{pages === 1 ? '（推荐）' : ''}</option
              >{/each}</select
          ></label
        >
        <label
          ><span class="label">经验要求</span><select
            class="select"
            bind:value={searchSpec.experience}
            ><option value="">不限</option><option value="104">1–3 年</option><option value="105"
              >3–5 年</option
            ><option value="106">5–10 年</option></select
          ></label
        >
        <label
          ><span class="label">薪资范围</span><select class="select" bind:value={searchSpec.salary}
            >{#each SALARY_FILTER_OPTIONS as option}<option value={option.value}
                >{option.label}</option
              >{/each}</select
          ></label
        >
        <label
          ><span class="label">公司规模</span><select
            class="select"
            bind:value={searchSpec.companyScale}
            >{#each COMPANY_SCALE_FILTER_OPTIONS as option}<option value={option.value}
                >{option.label}</option
              >{/each}</select
          ></label
        >
      </div>
      <div
        class="mt-5 rounded-xl border px-4 py-3"
        style="border-color: color-mix(in srgb, #b7791f 35%, var(--line)); background: var(--warning-soft);"
      >
        <div class="flex items-start gap-2">
          <Info size={16} class="mt-0.5 shrink-0 text-warning" />
          <div>
            <p class="text-sm font-semibold">预计耗时：{scrapeDuration}</p>
            <p class="mt-1 text-xs leading-5 body-muted">
              抓取期间请勿关闭应用；可以切换页面，任务不会中断。实际耗时受网络和岗位数量影响。
            </p>
          </div>
        </div>
      </div>
      <div class="mt-5 flex items-center justify-between gap-4">
        <p class="flex items-center gap-2 text-xs body-muted">
          <Info size={14} />下一步会说明登录检查方式，确认后才会启动。
        </p>
        <button
          class="btn-primary shrink-0"
          disabled={scrapeTaskRunning || keywordMissing || !selectedCityValid}
          on:click={review}
          >{#if scrapeTaskRunning}<RefreshCw
              size={15}
              class="animate-spin"
            />已有任务运行{:else}<Search size={15} />开始抓取{/if}</button
        >
      </div>
    {/if}
  </div>
{/if}
