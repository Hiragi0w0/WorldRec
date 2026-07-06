<script lang="ts">
  import { ChevronLeft, ChevronRight } from "lucide-svelte";
  import type { DateRailItem } from "../../data/worldrec";

  export let dates: DateRailItem[] = [];
  export let selectedIndex = 0;
  export let onSelectDate = (index: number) => {};

  function moveDate(delta: number) {
    if (!dates.length) {
      return;
    }

    const nextIndex = (selectedIndex + delta + dates.length) % dates.length;
    onSelectDate(nextIndex);
  }
</script>

<section class="date-rail" aria-label="日付レール">
  <button class="rail-nav" type="button" aria-label="前の日付" on:click={() => moveDate(-1)}>
    <ChevronLeft size={18} />
  </button>

  <div class="rail-scroll">
    {#each dates as date, index}
      <button
        class:selected={index === selectedIndex}
        class="rail-chip"
        type="button"
        on:click={() => onSelectDate(index)}
      >
        <span class="rail-chip__label">{date.label}</span>
        <span class:visible={date.hasActivity} class="rail-chip__dot"></span>
      </button>
    {/each}
  </div>

  <button class="rail-nav" type="button" aria-label="次の日付" on:click={() => moveDate(1)}>
    <ChevronRight size={18} />
  </button>
</section>

