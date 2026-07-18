<aside
  class="filter-sidebar scrollbar-thin w-[250px] shrink-0 overflow-y-auto border-r p-4"
  style="border-color: var(--line); background: color-mix(in srgb, var(--canvas) 72%, var(--panel));"
>
  <slot />
</aside>

<style>
  @media (max-width: 1280px) {
    .filter-sidebar {
      width: 210px;
    }
  }
</style>
