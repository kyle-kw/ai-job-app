<script lang="ts">
  import { recordJobsBrowseScrollTop } from '$lib/jobs-browse-cache';

  function trackListScroll(node: HTMLElement) {
    recordJobsBrowseScrollTop(node.scrollTop);
    const onScroll = () => recordJobsBrowseScrollTop(node.scrollTop);
    node.addEventListener('scroll', onScroll);
    return {
      destroy() {
        node.removeEventListener('scroll', onScroll);
      }
    };
  }
</script>

<section
  class="job-list scrollbar-thin w-[382px] shrink-0 overflow-y-auto border-r bg-panel"
  style="border-color: var(--line);"
  use:trackListScroll
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
