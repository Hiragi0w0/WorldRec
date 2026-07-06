<script lang="ts">
    import type { DateListItem, PaperStyle, TapeStyle, ViewFormat, VisitRecord } from "../../data/visitTypes";
    import type { RuntimeStatusDto } from "../../api/commands";
    import DateRail from "../history/DateRail.svelte";
    import JourneySummary from "../history/JourneySummary.svelte";
    import VisitTimeline from "../history/VisitTimeline.svelte";

    export let summary: {
        selectedDateCount: number;
        totalCount: number;
        totalDifferentWorlds: number;
        timeFormatted: string;
    };
    export let dateList: DateListItem[];
    export let selectedDate: string;
    export let selectedDateLabel: string;
    export let visitRecords: VisitRecord[];
    export let timelineRecords: VisitRecord[];
    export let searchQuery: string;
    export let isLoadingVisits: boolean;
    export let visitLoadError: string | null;
    export let paperStyle: PaperStyle;
    export let tapeStyle: TapeStyle;
    export let viewFormat: ViewFormat;
    export let runtimeStatus: RuntimeStatusDto | null;
    export let onSelectDate: (dateKey: string) => void;
    export let onViewFormatChange: (format: ViewFormat) => void;
    export let onOpenSettings: () => void;
    export let onOpenSync: () => void;
    export let onShowAllVisits: () => void;
    export let onOpenWorldDetail: (record: VisitRecord) => void;
    export let thumbnailForVisit: (record: VisitRecord) => string | null;
    export let onDeleteRecord: (record: VisitRecord) => void;

    $: databaseVisitCount = runtimeStatus?.visit_count ?? null;
    $: hasSearchQuery = searchQuery.trim().length > 0;
    $: hasSearchNoResults =
        !isLoadingVisits && !visitLoadError && hasSearchQuery && visitRecords.length > 0 && timelineRecords.length === 0;
    $: showAllVisitsAction = false;

    $: emptyTitle = buildEmptyTitle();
    $: emptyBody = buildEmptyBody();

    function buildEmptyTitle() {
        if (visitLoadError) return "訪問履歴を読み込めません";
        if (isLoadingVisits) return "訪問履歴を読み込んでいます";
        if (hasSearchNoResults) return "検索に一致する滞在記録はありません";
        if (databaseVisitCount === 0) return "まだ訪問履歴がありません";
        return "この日の訪問履歴はまだありません";
    }

    function buildEmptyBody() {
        if (visitLoadError) return visitLoadError;
        if (isLoadingVisits) return "ローカルDBの訪問履歴を確認しています。";
        if (hasSearchNoResults) return "検索語を変更するか、検索欄を空にすると選択日の記録を確認できます。";
        if (databaseVisitCount === 0) return "VRChatを起動してワールドに入ると、ここに記録が表示されます。";
        return "VRChatログ同期後、選択日の履歴がここに表示されます。";
    }
</script>

<section class="space-y-8 animate-fadeIn">
    <JourneySummary {summary} />
    <DateRail {dateList} {selectedDate} {visitRecords} {onSelectDate} />
    <VisitTimeline
        {selectedDateLabel}
        {timelineRecords}
        {paperStyle}
        {tapeStyle}
        {viewFormat}
        {emptyTitle}
        {emptyBody}
        {onViewFormatChange}
        {onOpenSettings}
        {onOpenSync}
        {showAllVisitsAction}
        {onShowAllVisits}
        {onOpenWorldDetail}
        {thumbnailForVisit}
        {onDeleteRecord}
    />
</section>
