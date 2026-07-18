<section
  class="job-list scrollbar-thin w-[382px] shrink-0 overflow-y-auto border-r bg-panel"
  style="border-color: var(--line);"
>
  <slot />
</section>

<style>
  @media (max-width: 1280px) {
    .job-list {
      width: 330px;
    }
  }
</style>
