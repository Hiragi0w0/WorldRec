<script lang="ts">
  import { Filter, LayoutGrid, LayoutList } from "lucide-svelte";
  import VisitCard from "./VisitCard.svelte";
  import type { VisitRecord } from "../../data/worldrec";

  export let selectedDateLabel = "";
  export let selectedDateVisitCount = 0;
  export let visits: VisitRecord[] = [];
  export let emptyTitle = "この日の訪問記録はありません";
  export let emptyBody = "別の日付を選ぶと、旅の記録が表示されます。";

  let viewMode: "list" | "grid" = "list";
</script>

<section class="timeline-card">
  <div class="timeline-header">
    <div>
      <p class="section-label">Visit Timeline</p>
      <h2>{selectedDateLabel}</h2>
      <p class="timeline-meta">{selectedDateVisitCount}件の訪問</p>
    </div>

    <div class="timeline-actions">
      <button class="pill-button" type="button">
        <Filter size={16} />
        <span>フィルター</span>
      </button>
      <div class="toggle-group" role="group" aria-label="表示切替">
        <button
          class:view-active={viewMode === "list"}
          class="toggle-button"
          type="button"
          on:click={() => (viewMode = "list")}
        >
          <LayoutList size={16} />
        </button>
        <button
          class:view-active={viewMode === "grid"}
          class="toggle-button"
          type="button"
          on:click={() => (viewMode = "grid")}
        >
          <LayoutGrid size={16} />
        </button>
      </div>
    </div>
  </div>

  <div class:compact-grid={viewMode === "grid"} class="visit-timeline">
    {#if visits.length > 0}
      {#each visits as visit, index (visit.id)}
        <VisitCard {visit} {viewMode} featured={index === 0} />
      {/each}
    {:else}
      <div class="empty-state">
        <p class="empty-state__title">{emptyTitle}</p>
        <p class="empty-state__body">{emptyBody}</p>
      </div>
    {/if}
  </div>
</section>

