<script lang="ts">
    import { BarChart2, BookOpen, CalendarCheck, Settings, Sparkles, Wifi } from "lucide-svelte";
    import type { Screen } from "../../data/visitTypes";

    export let activeScreen: Screen;
    export let visitCount: number;
    export let onNavigate: (screen: Screen) => void;
    export let onOpenSync: () => void;
</script>

<aside class="w-64 bg-[#FAF9F5] border-r border-[#EBE8DF] flex flex-col shrink-0 justify-between sticky top-0 h-screen">
    <div class="flex flex-col">
        <div class="p-6 border-b border-[#EBE8DF] flex items-center justify-between">
            <div class="flex items-center gap-3">
                <div class="w-10 h-10 rounded-full bg-[#1e5854] text-white flex items-center justify-center font-bold tracking-tight shadow-md">
                    <span class="text-sm font-semibold tracking-wider font-travel">WR</span>
                </div>
                <div class="flex flex-col">
                    <span class="text-base font-bold tracking-wide font-travel">WorldRec</span>
                    <span class="text-xs text-zinc-400 font-medium tracking-widest">TRAVEL MEMORIES</span>
                </div>
            </div>
            <div class="hidden lg:block w-2.5 h-6 border-l border-dashed border-[#D2C8B5] opacity-60"></div>
        </div>

        <nav class="px-3 mt-4 space-y-1">
            <button onclick={() => onNavigate("record")} class={`nav-button ${activeScreen === "record" ? "nav-active" : ""}`} id="nav-travel-records">
                <CalendarCheck class="w-4 h-4" />
                <span class="tracking-wide">旅の記録</span>
                {#if visitCount > 0}
                    <span class="ml-auto bg-[#1e5854]/10 text-[#0f4743] text-xs px-2 py-0.5 rounded-full font-bold">{visitCount}</span>
                {/if}
            </button>
            <button onclick={() => onNavigate("library")} class={`nav-button ${activeScreen === "library" ? "nav-active" : ""}`} id="nav-library">
                <BookOpen class="w-4 h-4" />
                <span>ライブラリ</span>
            </button>
            <button onclick={() => onNavigate("ai_guide")} class={`nav-button ${activeScreen === "ai_guide" ? "nav-active" : ""}`} id="nav-ai-explore">
                <Sparkles class="w-4 h-4 text-amber-500 fill-amber-50" />
                <span>AI 探索</span>
                <span class="ml-auto bg-amber-100 text-amber-800 text-[9px] px-1.5 rounded-sm font-bold tracking-wider">NEW</span>
            </button>
            <button onclick={() => onNavigate("stats")} class={`nav-button ${activeScreen === "stats" ? "nav-active" : ""}`} id="nav-stats">
                <BarChart2 class="w-4 h-4" />
                <span>統 計</span>
            </button>
            <button onclick={() => onNavigate("settings")} class={`nav-button ${activeScreen === "settings" ? "nav-active" : ""}`} id="nav-settings">
                <Settings class="w-4 h-4" />
                <span>設 定</span>
            </button>
        </nav>
    </div>

    <div class="p-4 border-t border-[#EBE8DF]">
        <div class="bg-white/60 p-3 rounded-xl border border-[#ECE9DD] mb-3">
            <div class="flex items-center gap-1.5 mb-1">
                <div class="w-2 h-2 rounded-full bg-emerald-500 animate-pulse"></div>
                <span class="text-[10px] font-bold text-zinc-500">Sync Status:</span>
                <span class="text-[10px] font-bold text-[#1e5854]" id="sync-active-badge">Active</span>
            </div>
            <p class="text-[10px] text-zinc-400">VRChatのログファイルを直接取り込んでチェキアルバムを自動生成できます。</p>
        </div>
        <button
            onclick={onOpenSync}
            class="w-full bg-[#1e5854] hover:bg-[#133c39] border border-[#164340] text-white py-3 px-4 rounded-xl flex items-center justify-center gap-2 text-xs font-bold shadow-md hover:shadow-lg transition-all"
            id="btn-sync-vrchat"
        >
            <Wifi class="w-3.5 h-3.5 animate-bounce" />
            VRChat ログを同期
        </button>
    </div>
</aside>
