<script lang="ts">
  import DateRail from "../dashboard/DateRail.svelte";
  import SummaryCard from "../dashboard/SummaryCard.svelte";
  import VisitTimeline from "../dashboard/VisitTimeline.svelte";
  import type { DailyVisitGroup, DateRailItem } from "../../data/worldrec";

  let selectedDateIndex = 0;
  let dailyVisitGroups: DailyVisitGroup[] = [];
  let dateRailItems: DateRailItem[] = [];

  function selectDate(index: number) {
    selectedDateIndex = index;
  }

  $: selectedGroup = dailyVisitGroups[selectedDateIndex] ?? {
    dateKey: "recent",
    label: "最近",
    visits: [],
  };

  $: emptyTitle = "まだ訪問記録はありません";

  $: emptyBody = "記録が追加されるとここに表示されます。";
</script>

<section class="main-content">
  <SummaryCard />

  <DateRail
    dates={dateRailItems}
    selectedIndex={selectedDateIndex}
    onSelectDate={selectDate}
  />

  <VisitTimeline
    selectedDateLabel={selectedGroup.label}
    selectedDateVisitCount={selectedGroup.visits.length}
    visits={selectedGroup.visits}
    {emptyTitle}
    {emptyBody}
  />
</section>

