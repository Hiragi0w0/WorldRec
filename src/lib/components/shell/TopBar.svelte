<script lang="ts">
    import { Bell, RotateCw, Search } from "lucide-svelte";

    export let searchQuery: string;
    export let isSyncing: boolean;
    export let onSearchChange: (query: string) => void;
    export let onQuickSync: () => void;

    function handleSearchInput(event: Event) {
        onSearchChange((event.currentTarget as HTMLInputElement).value);
    }
</script>

<header class="bg-[#FAF9F5]/80 backdrop-blur-md border-b border-[#EBE8DF] px-8 py-4 flex items-center justify-between sticky top-0 z-30">
    <div class="flex flex-col">
        <span class="text-[10px] text-[#A68F63] font-bold tracking-widest uppercase font-travel">TRAVEL MEMORIES</span>
        <h1 class="text-xl font-extrabold text-zinc-800 tracking-tight flex items-center gap-2">
            旅の記憶
            <span class="text-xs font-medium px-2 py-0.5 rounded-full bg-[#F3EFE3] text-[#7E6941] border border-[#E4DCBF] font-serif">Traveler's Notebook</span>
        </h1>
    </div>

    <div class="relative w-96 max-w-lg">
        <input
            type="text"
            placeholder="ワールド名やタグ、メモで検索..."
            value={searchQuery}
            oninput={handleSearchInput}
            class="w-full bg-white border border-[#E1DEC9] text-sm text-zinc-700 py-2.5 pl-10 pr-12 rounded-xl focus:outline-none focus:ring-2 focus:ring-[#1e5854] focus:border-transparent transition-all shadow-inner"
            id="top-search-input"
        />
        <Search class="w-4 h-4 text-zinc-400 absolute left-3.5 top-1/2 -translate-y-1/2" />
        <span class="absolute right-3.5 top-1/2 -translate-y-1/2 text-[9px] bg-zinc-100 text-zinc-400 px-1.5 py-0.5 rounded border border-zinc-200 font-mono font-bold tracking-widest">Ctrl K</span>
    </div>

    <div class="flex items-center gap-3">
        <button
            onclick={onQuickSync}
            disabled={isSyncing}
            class="p-2.5 text-zinc-600 bg-white border border-[#E1DEC9] rounded-xl hover:bg-[#FAF9F5] transition-colors shadow-sm disabled:opacity-50 disabled:cursor-not-allowed"
            title="データを再読み込み・同期"
        >
            <RotateCw class="w-4 h-4" />
        </button>
        <div class="relative">
            <button class="p-2.5 text-zinc-600 bg-white border border-[#E1DEC9] rounded-xl hover:bg-[#FAF9F5] transition-colors shadow-sm" title="通知">
                <Bell class="w-4 h-4" />
            </button>
            <span class="absolute top-1 right-1 w-2.5 h-2.5 bg-rose-500 rounded-full border-2 border-[#FAF9F5]"></span>
        </div>
    </div>
</header>
