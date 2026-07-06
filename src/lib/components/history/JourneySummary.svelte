<script lang="ts">
    import { Clock, Compass, MapPin, Plane } from "lucide-svelte";

    export let summary: {
        selectedDateCount: number;
        totalCount: number;
        totalDifferentWorlds: number;
        timeFormatted: string;
    };
</script>

<div class="bg-white border border-[#E2DFD3] rounded-2xl p-6 shadow-sm relative overflow-hidden" id="summary-panel-container">
    <div class="absolute top-0 right-0 w-32 h-full opacity-5 pointer-events-none dot-deco"></div>
    <div class="flex items-center justify-between mb-4">
        <h3 class="text-xs text-[#0f4743] font-bold tracking-widest uppercase font-travel flex items-center gap-1.5">
            <Plane class="w-3.5 h-3.5" />
            JOURNEY SUMMARY
        </h3>
        <div class="h-px flex-1 bg-gradient-to-r from-[#DFDAC8] to-transparent ml-4"></div>
    </div>

    <div class="grid grid-cols-1 xl:grid-cols-3 gap-6">
        <div class="summary-tile border-l-[#1e5854]">
            <div class="p-3 bg-[#EBF3F2] rounded-xl text-[#0F4743]"><MapPin class="w-5 h-5" /></div>
            <div class="min-w-0">
                <span class="text-xs text-zinc-400 font-bold tracking-wider">選択日の訪問ワールド</span>
                <div class="flex items-baseline gap-1 mt-0.5">
                    <span class="text-2xl font-black text-zinc-800" id="stat-selected-date-count">{summary.selectedDateCount}</span>
                    <span class="text-xs text-zinc-500 font-semibold">件</span>
                </div>
            </div>
        </div>
        <div class="summary-tile border-l-[#A68F63]">
            <div class="p-3 bg-amber-50 rounded-xl text-[#A68F63]"><Compass class="w-5 h-5" /></div>
            <div class="min-w-0">
                <span class="text-xs text-zinc-400 font-bold tracking-wider">総訪問ワールド数</span>
                <div class="flex items-baseline gap-1 mt-0.5">
                    <span class="text-2xl font-black text-zinc-800" id="stat-total-worlds">{summary.totalDifferentWorlds}</span>
                    <span class="text-xs text-zinc-500 font-semibold">ワールド (計 {summary.totalCount}回)</span>
                </div>
            </div>
        </div>
        <div class="summary-tile border-l-[#4c5f95]">
            <div class="p-3 bg-indigo-50 rounded-xl text-indigo-700"><Clock class="w-5 h-5" /></div>
            <div class="min-w-0">
                <span class="text-xs text-zinc-400 font-bold tracking-wider">総滞在時間</span>
                <div class="flex items-baseline gap-1 mt-0.5">
                    <span class="text-2xl font-black text-zinc-800" id="stat-total-stay">{summary.timeFormatted}</span>
                </div>
            </div>
        </div>
    </div>
</div>
