<script lang="ts">
    import { tick } from "svelte";
    import { ChevronLeft, ChevronRight } from "lucide-svelte";
    import type { DateListItem, VisitRecord } from "../../data/visitTypes";

    export let dateList: DateListItem[];
    export let selectedDate: string;
    export let visitRecords: VisitRecord[];
    export let onSelectDate: (dateKey: string) => void;

    let railElement: HTMLDivElement;

    function countForDate(dateKey: string) {
        return visitRecords.filter((record) => record.dateKey === dateKey).length;
    }

    function scrollRail(delta: number) {
        if (!railElement) return;

        const distance = Math.max(railElement.clientWidth - 96, 120);
        railElement.scrollBy({
            left: delta * distance,
            behavior: "smooth",
        });
    }

    async function scrollSelectedDateIntoView(behavior: ScrollBehavior = "smooth") {
        if (!railElement || !selectedDate) return;

        await tick();
        const selectedButton = Array.from(railElement.querySelectorAll<HTMLButtonElement>("button")).find(
            (button) => button.dataset.dateKey === selectedDate,
        );
        selectedButton?.scrollIntoView({
            behavior,
            block: "nearest",
            inline: "nearest",
        });
    }

    async function selectDate(dateKey: string, element: HTMLButtonElement) {
        onSelectDate(dateKey);
        await tick();
        element.scrollIntoView({
            behavior: "smooth",
            block: "nearest",
            inline: "nearest",
        });
    }

    $: if (railElement && selectedDate) {
        void scrollSelectedDateIntoView("smooth");
    }
</script>

<div class="bg-white border border-[#E2DFD3] p-3 sm:p-4 rounded-2xl shadow-sm">
    <div class="grid grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-2">
        <button
            type="button"
            aria-label="前の日付を表示"
            onclick={() => scrollRail(-1)}
            class="shrink-0 bg-white/95 p-2 rounded-full shadow-sm border border-[#E7E4D9] hover:bg-[#FAF9F5] focus:outline-none focus:ring-2 focus:ring-[#1e5854]/30"
        >
            <ChevronLeft class="w-4 h-4 text-zinc-500" />
        </button>
        <div
            bind:this={railElement}
            class="min-w-0 flex items-center gap-2 overflow-x-auto py-1.5 no-scrollbar scroll-smooth"
        >
            {#each dateList as day}
                {@const count = countForDate(day.key)}
                <button
                    data-date-key={day.key}
                    aria-current={selectedDate === day.key ? "date" : undefined}
                    aria-pressed={selectedDate === day.key}
                    onclick={(event) => selectDate(day.key, event.currentTarget as HTMLButtonElement)}
                    class={`px-4 py-2 rounded-xl text-sm font-semibold tracking-wide border shrink-0 transition-all duration-200 flex flex-col items-center gap-1 min-w-[76px] relative group ${selectedDate === day.key ? "bg-[#1e5854] text-white border-[#1e5854] shadow-md -translate-y-0.5" : "bg-white text-zinc-700 border-[#E5E2D2] hover:bg-[#F9F7EF] hover:border-zinc-400"}`}
                >
                    <span>{day.display}</span>
                    <span class={`w-1.5 h-1.5 rounded-full ${count > 0 ? (selectedDate === day.key ? "bg-amber-300" : "bg-[#A68F63]") : "bg-transparent"}`}></span>
                    {#if count > 0}
                        <span class={`absolute -top-1.5 -right-1 text-[9px] px-1.5 rounded-full border ${selectedDate === day.key ? "bg-amber-400 text-zinc-900 border-amber-500" : "bg-neutral-100 text-neutral-600 border-neutral-300"}`}>{count}</span>
                    {/if}
                </button>
            {/each}
        </div>
        <button
            type="button"
            aria-label="次の日付を表示"
            onclick={() => scrollRail(1)}
            class="shrink-0 bg-white/95 p-2 rounded-full shadow-sm border border-[#E7E4D9] hover:bg-[#FAF9F5] focus:outline-none focus:ring-2 focus:ring-[#1e5854]/30"
        >
            <ChevronRight class="w-4 h-4 text-zinc-500" />
        </button>
    </div>
</div>
