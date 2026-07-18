<script lang="ts">
  import { tick } from 'svelte';
  import { Check, ChevronDown, Search, X } from 'lucide-svelte';

  export let options: Array<{ label: string; count: number }> = [];
  export let selected: string[] = [];

  let open = false;
  let query = '';
  let activeIndex = -1;
  let trigger: HTMLButtonElement;
  let searchInput: HTMLInputElement;

  $: selectedKeys = new Set(selected.map((item) => item.toLocaleLowerCase()));
  $: filteredOptions = options.filter((option) => option.label.toLocaleLowerCase().includes(query.trim().toLocaleLowerCase()));
  $: if (open && filteredOptions.length && (activeIndex < 0 || activeIndex >= filteredOptions.length)) activeIndex = 0;
  $: if (!filteredOptions.length) activeIndex = -1;

  function outside(node: HTMLElement) {
    const handlePointerDown = (event: MouseEvent) => {
      if (!node.contains(event.target as Node)) close(false);
    };
    document.addEventListener('mousedown', handlePointerDown);
    return { destroy: () => document.removeEventListener('mousedown', handlePointerDown) };
  }

  async function toggleMenu() {
    open = !open;
    if (open) {
      query = '';
      activeIndex = filteredOptions.length ? 0 : -1;
      await tick();
      searchInput?.focus();
    }
  }

  function close(restoreFocus = true) {
    if (!open) return;
    open = false;
    query = '';
    activeIndex = -1;
    if (restoreFocus) void tick().then(() => trigger?.focus());
  }

  function toggleSkill(label: string) {
    const key = label.toLocaleLowerCase();
    selected = selectedKeys.has(key)
      ? selected.filter((item) => item.toLocaleLowerCase() !== key)
      : [...selected, label];
  }

  function handleSearchKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      event.preventDefault();
      close();
    } else if (event.key === 'ArrowDown' && filteredOptions.length) {
      event.preventDefault();
      activeIndex = (activeIndex + 1) % filteredOptions.length;
    } else if (event.key === 'ArrowUp' && filteredOptions.length) {
      event.preventDefault();
      activeIndex = (activeIndex - 1 + filteredOptions.length) % filteredOptions.length;
    } else if (event.key === 'Enter' && activeIndex >= 0) {
      event.preventDefault();
      toggleSkill(filteredOptions[activeIndex].label);
    }
  }
</script>

<div class="relative" use:outside>
  <button
    bind:this={trigger}
    type="button"
    class="select flex w-full items-center justify-between text-left"
    aria-haspopup="listbox"
    aria-label="技能筛选"
    aria-expanded={open}
    aria-controls="job-skill-options"
    on:click={toggleMenu}
    on:keydown={(event) => { if (event.key === 'ArrowDown') { event.preventDefault(); if (!open) void toggleMenu(); } }}
  >
    <span class={selected.length ? 'text-ink' : 'body-muted'}>{selected.length ? `已选 ${selected.length} 项` : '不限'}</span>
    <ChevronDown size={14} class={open ? 'rotate-180' : ''} />
  </button>

  {#if open}
    <div class="absolute left-0 right-0 top-full z-30 mt-1 rounded-xl border bg-panel p-2 shadow-xl" style="border-color: var(--line);">
      <label class="relative block">
        <span class="sr-only">搜索技能</span>
        <Search size={13} class="pointer-events-none absolute left-2.5 top-2.5 body-muted" />
        <input
          bind:this={searchInput}
          class="input h-8 pl-8 text-xs"
          value={query}
          placeholder="搜索技能"
          on:input={(event) => { query = event.currentTarget.value; activeIndex = 0; }}
          on:keydown={handleSearchKeydown}
        />
      </label>
      <div id="job-skill-options" class="scrollbar-thin mt-2 max-h-52 overflow-y-auto" role="listbox" aria-label="技能选项" aria-multiselectable="true">
        {#if filteredOptions.length}
          {#each filteredOptions as option, index}
            <button
              type="button"
              role="option"
              aria-selected={selectedKeys.has(option.label.toLocaleLowerCase())}
              class="flex w-full items-center justify-between rounded-lg px-2.5 py-2 text-left text-xs hover:bg-brand-soft"
              class:bg-brand-soft={index === activeIndex}
              on:mousedown|preventDefault={() => {}}
              on:click={() => toggleSkill(option.label)}
            >
              <span class="flex min-w-0 items-center gap-2"><span class="grid h-4 w-4 shrink-0 place-items-center rounded border" style="border-color: var(--line);">{#if selectedKeys.has(option.label.toLocaleLowerCase())}<Check size={11} class="text-brand" />{/if}</span><span class="truncate">{option.label}</span></span>
              <span class="body-muted">{option.count}</span>
            </button>
          {/each}
        {:else}
          <p class="px-2 py-3 text-center text-xs body-muted">未找到匹配技能</p>
        {/if}
      </div>
    </div>
  {/if}

  {#if selected.length}
    <div class="mt-2 flex flex-wrap gap-1.5" aria-label="已选技能">
      {#each selected as skill}
        <button type="button" class="chip gap-1 px-2 py-1" aria-label={`移除技能 ${skill}`} on:click={() => toggleSkill(skill)}>{skill}<X size={11} /></button>
      {/each}
    </div>
  {/if}
</div>
