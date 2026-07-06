<script lang="ts">
    import { Bookmark, Grid, List, Settings } from "lucide-svelte";
    import type { PaperStyle, TapeStyle, ViewFormat, VisitRecord } from "../../data/visitTypes";
    import { paperClass } from "../../utils/history";
    import VisitCard from "./VisitCard.svelte";

    export let selectedDateLabel: string;
    export let timelineRecords: VisitRecord[];
    export let paperStyle: PaperStyle;
    export let tapeStyle: TapeStyle;
    export let viewFormat: ViewFormat;
    export let emptyTitle = "まだ訪問履歴がありません。";
    export let emptyBody = "VRChatを起動してワールドに入ると、ここに記録が表示されます。";
    export let onViewFormatChange: (format: ViewFormat) => void;
    export let onOpenSettings: () => void;
    export let onOpenSync: () => void;
    export let showAllVisitsAction = false;
    export let onShowAllVisits: () => void;
    export let onOpenWorldDetail: (record: VisitRecord) => void;
    export let thumbnailForVisit: (record: VisitRecord) => string | null;
    export let onDeleteRecord: (record: VisitRecord) => void;
</script>

<div class="flex flex-col md:flex-row md:items-center justify-between gap-4 bg-white/50 p-4 rounded-xl border border-[#E7E4D9]">
    <div>
        <span class="text-[10px] text-[#0f4743] font-bold tracking-widest uppercase">VISIT TIMELINE</span>
        <div class="flex items-baseline gap-3 mt-0.5">
            <h2 class="text-2xl font-black text-zinc-800 tracking-tight">{selectedDateLabel}</h2>
            <span class="text-sm font-semibold text-zinc-400">{timelineRecords.length} 件の滞在記録</span>
        </div>
    </div>
    <div class="bg-white border border-[#E1DEC9] p-1 rounded-xl flex items-center">
        <button onclick={() => onViewFormatChange("list")} class={`p-1.5 rounded-lg ${viewFormat === "list" ? "bg-zinc-100 text-zinc-900" : "text-zinc-400"}`} title="リスト表示"><List class="w-4 h-4" /></button>
        <button onclick={() => onViewFormatChange("grid")} class={`p-1.5 rounded-lg ${viewFormat === "grid" ? "bg-zinc-100 text-zinc-900" : "text-zinc-400"}`} title="グリッド表示"><Grid class="w-4 h-4" /></button>
        <div class="w-px h-4 bg-zinc-200 mx-1"></div>
        <button onclick={onOpenSettings} class="p-1.5 rounded-lg text-zinc-400 hover:text-zinc-700 hover:bg-zinc-50 transition-colors" title="手帳・デザイン設定"><Settings class="w-4 h-4" /></button>
    </div>
</div>

{#if timelineRecords.length === 0}
    <div class="bg-white border border-[#E3DFC9] rounded-2xl p-16 text-center shadow-sm">
        <div class="w-16 h-16 rounded-full bg-amber-50 text-[#7E6941] flex items-center justify-center mx-auto mb-4 border border-dashed border-[#DFCFA9]"><Bookmark class="w-8 h-8" /></div>
        <h3 class="text-lg font-bold text-zinc-800">{emptyTitle}</h3>
        <p class="text-sm text-zinc-500 mt-2 max-w-sm mx-auto">{emptyBody}</p>
        <div class="mt-6 flex flex-wrap justify-center gap-3">
            {#if showAllVisitsAction}
                <button onclick={onShowAllVisits} class="bg-white hover:bg-[#FAF9F5] text-zinc-700 border border-[#C5BFAB] font-bold text-xs py-2 px-5 rounded-xl shadow-sm">全件表示に切り替える</button>
            {/if}
            <button onclick={onOpenSync} class="bg-[#1e5854] hover:bg-[#133c39] text-white font-bold text-xs py-2 px-5 rounded-xl shadow-sm">VRCログ同期を実行</button>
        </div>
    </div>
{:else}
    <div class={`relative px-4 py-8 rounded-3xl border border-[#DEDAC4] shadow-sm ${paperClass(paperStyle)}`}>
        <div class="absolute top-0 bottom-0 left-6 flex flex-col justify-around pointer-events-none opacity-20">
            {#each Array(8) as _}
                <div class="w-3.5 h-3.5 rounded-full bg-zinc-400/50 shadow-inner border border-zinc-500/20"></div>
            {/each}
        </div>
        {#if viewFormat === "list"}
            <div class="absolute left-16 md:left-[92px] top-6 bottom-6 w-0.5 bg-gradient-to-b from-[#1e5854]/40 via-[#A68F63]/30 to-[#D2C8B5]/20 pointer-events-none"></div>
        {/if}
        <div class={viewFormat === "grid" ? "grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-8 pl-12 pr-6" : "space-y-12 pl-12 pr-6"}>
            {#each timelineRecords as record, index}
                <VisitCard
                    {record}
                    {tapeStyle}
                    {viewFormat}
                    eagerLoad={index < 4}
                    {onOpenWorldDetail}
                    thumbnail={thumbnailForVisit(record)}
                    {onDeleteRecord}
                />
            {/each}
        </div>
    </div>
{/if}
