<script lang="ts">
    export let startDate: string;
    export let endDate: string;
    export let error: string | null = null;
    export let onChange: (start: string, end: string) => void;

    function handleStartChange(e: Event) {
        const value = (e.currentTarget as HTMLInputElement).value;
        if (value > endDate) {
            error = "開始日は終了日より前にしてください";
            return;
        }
        error = null;
        onChange(value, endDate);
    }

    function handleEndChange(e: Event) {
        const value = (e.currentTarget as HTMLInputElement).value;
        if (value < startDate) {
            error = "終了日は開始日より後にしてください";
            return;
        }
        error = null;
        onChange(startDate, value);
    }
</script>

<div class="flex items-center gap-3 flex-wrap">
    <div class="flex items-center gap-2">
        <label for="stats-start-date" class="text-xs font-semibold text-zinc-500">開始日</label>
        <input
            id="stats-start-date"
            type="date"
            value={startDate}
            onchange={handleStartChange}
            class="text-sm border border-zinc-200 rounded-lg px-3 py-1.5 bg-white focus:outline-none focus:ring-2 focus:ring-[#1e5854]/30"
        />
    </div>
    <span class="text-zinc-400">〜</span>
    <div class="flex items-center gap-2">
        <label for="stats-end-date" class="text-xs font-semibold text-zinc-500">終了日</label>
        <input
            id="stats-end-date"
            type="date"
            value={endDate}
            onchange={handleEndChange}
            class="text-sm border border-zinc-200 rounded-lg px-3 py-1.5 bg-white focus:outline-none focus:ring-2 focus:ring-[#1e5854]/30"
        />
    </div>
    {#if error}
        <span class="text-xs text-red-500 font-medium">{error}</span>
    {/if}
</div>
