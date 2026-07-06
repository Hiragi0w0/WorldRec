<script lang="ts">
    import { RotateCw, Terminal, Wifi } from "lucide-svelte";

    export let syncLogs: string[];
    export let isSyncing: boolean;
    export let onStartSync: () => void;
    export let onClose: () => void;
</script>

<div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/65 backdrop-blur-xs animate-fadeIn">
    <div class="bg-[#18181B] border-2 border-[#2E2E33] w-full max-w-2xl rounded-2xl shadow-2xl overflow-hidden text-neutral-200">
        <div class="bg-[#27272A] px-4 py-3 border-b border-[#3F3F46] flex items-center justify-between">
            <div class="flex items-center gap-2">
                <div class="w-3 h-3 rounded-full bg-[#EF4444]"></div>
                <div class="w-3 h-3 rounded-full bg-[#F59E0B]"></div>
                <div class="w-3 h-3 rounded-full bg-[#10B981]"></div>
                <span class="text-xs font-bold text-neutral-400 font-mono ml-2">VRChat Output Log synchronization terminal</span>
            </div>
            <button onclick={onClose} class="text-neutral-400 hover:text-white font-mono text-sm">ESC</button>
        </div>
        <div class="p-6 space-y-4">
            <div class="flex gap-2 items-center text-xs text-amber-400 font-bold"><Terminal class="w-4 h-4" /><span>Rust 側のログ監視 command を開始し、SQLite DB から訪問履歴を再読み込みします。</span></div>
            {#if syncLogs.length > 0}
                <div class="bg-[#09090B] border border-zinc-800 rounded-xl p-4 h-48 overflow-y-auto font-mono text-[11px] text-emerald-400 space-y-1.5">
                    {#each syncLogs as log}
                        <div class="leading-relaxed whitespace-pre-wrap">{log}</div>
                    {/each}
                    {#if isSyncing}
                        <div class="flex items-center gap-1.5 text-zinc-400 mt-2 select-none">
                            <div class="w-1.5 h-1.5 bg-sky-400 rounded-full animate-bounce"></div>
                            <div class="w-1.5 h-1.5 bg-sky-400 rounded-full animate-bounce"></div>
                            <div class="w-1.5 h-1.5 bg-sky-400 rounded-full animate-bounce"></div>
                            <span>WORLDREC_DB_BRIDGE (active)...</span>
                        </div>
                    {/if}
                </div>
            {/if}
            <div class="flex items-center justify-between pt-4 border-t border-[#27272A]">
                <span class="text-[10px] text-zinc-500 font-mono">SOURCE: Tauri commands / LOCALAPPDATA/WorldRec/worldrec.db</span>
                <div class="flex gap-3">
                    <button onclick={onClose} class="bg-zinc-800 hover:bg-zinc-700 text-zinc-300 font-bold text-xs py-2 px-4 rounded-xl transition-colors">閉じる</button>
                    <button onclick={onStartSync} disabled={isSyncing} class="bg-[#1e5854] hover:bg-[#133c39] text-white font-extrabold text-xs py-2 px-5 rounded-xl flex items-center gap-1.5 shadow-md disabled:bg-zinc-700 transition-all">
                        {#if isSyncing}
                            <RotateCw class="w-3.5 h-3.5 animate-spin" />同期中...
                        {:else}
                            <Wifi class="w-3.5 h-3.5" />同期を開始する
                        {/if}
                    </button>
                </div>
            </div>
        </div>
    </div>
</div>
