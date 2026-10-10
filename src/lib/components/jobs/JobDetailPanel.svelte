<script lang="ts">
  import { recordJobsDetailScrollTop } from '$lib/jobs-browse-cache';

  function trackDetailScroll(node: HTMLElement) {
    recordJobsDetailScrollTop(node.scrollTop);
    const onScroll = () => recordJobsDetailScrollTop(node.scrollTop);
    node.addEventListener('scroll', onScroll);
    return {
      destroy() {
        node.removeEventListener('scroll', onScroll);
      }
    };
  }
</script>

<section class="job-detail scrollbar-thin min-w-0 flex-1 overflow-y-auto" use:trackDetailScroll>
  <slot />
</section>
