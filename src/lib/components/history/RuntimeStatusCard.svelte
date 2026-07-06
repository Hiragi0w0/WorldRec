<script lang="ts">
    import {
        Activity,
        Database,
        FolderOpen,
        Play,
        RefreshCw,
        TriangleAlert,
    } from "lucide-svelte";
    import type { RuntimeStatusDto } from "../../api/commands";

    export let runtimeStatus: RuntimeStatusDto | null;
    export let runtimeStatusLoading: boolean;
    export let isLoading: boolean;
    export let error: string | null;
    export let onReload: () => void;
    export let onStartWatcher: () => void;

    $: watcherLabel = runtimeStatus?.watcher_running ? "監視中" : "停止中";
    $: dbPath = runtimeStatus?.db_path ?? "未取得";
    $: logDir = runtimeStatus?.log_dir ?? "未取得";
    $: latestVisit = runtimeStatus?.latest_visit_at
        ? `${runtimeStatus.latest_visit_at} ${runtimeStatus.latest_world_name ?? ""}`.trim()
        : "なし";
</script>

<section
    class="bg-white border border-[#E2DFD3] rounded-2xl shadow-sm p-4 space-y-3"
>
    <div
        class="flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between"
    >
        <div class="flex items-center gap-2">
            <div
                class="w-9 h-9 rounded-xl bg-[#EAF3F1] border border-[#CFE3DE] text-[#1e5854] flex items-center justify-center"
            >
                <Database class="w-4 h-4" />
            </div>
            <div>
                <p
                    class="text-[10px] font-bold text-[#0f4743] tracking-widest uppercase"
                >
                    Runtime Status
                </p>
                <h2 class="text-sm font-extrabold text-zinc-800">
                    DB と VRChat ログ監視
                </h2>
            </div>
        </div>
        <div class="flex flex-wrap gap-2">
            <button
                onclick={onReload}
                disabled={isLoading || runtimeStatusLoading}
                class="inline-flex items-center gap-1.5 rounded-xl border border-[#D8D3C1] bg-white px-3 py-2 text-xs font-bold text-zinc-700 shadow-sm hover:bg-[#FAF9F5] disabled:opacity-50"
            >
                <RefreshCw
                    class={`w-3.5 h-3.5 ${isLoading || runtimeStatusLoading ? "animate-spin" : ""}`}
                />
                再読み込み
            </button>
            <button
                onclick={onStartWatcher}
                disabled={isLoading ||
                    runtimeStatusLoading ||
                    runtimeStatus?.watcher_running}
                class="inline-flex items-center gap-1.5 rounded-xl bg-[#1e5854] px-3 py-2 text-xs font-bold text-white shadow-sm hover:bg-[#133c39] disabled:bg-zinc-400"
            >
                <Play class="w-3.5 h-3.5" />
                監視開始
            </button>
        </div>
    </div>

    <div class="grid grid-cols-1 gap-3 text-xs xl:grid-cols-[minmax(0,1.1fr)_minmax(360px,0.9fr)]">
        <div class="grid grid-cols-1 gap-3">
            <div
                class="rounded-xl border border-[#ECE9DD] bg-[#FAF9F5] p-3 min-w-0"
            >
                <div class="flex items-center gap-1.5 text-zinc-500 font-bold mb-1">
                    <Database class="w-3.5 h-3.5" />DB保存先
                </div>
                <p
                    class="font-mono text-[11px] text-zinc-700 truncate"
                    title={dbPath}
                >
                    {dbPath}
                </p>
            </div>
            <div
                class="rounded-xl border border-[#ECE9DD] bg-[#FAF9F5] p-3 min-w-0"
            >
                <div class="flex items-center gap-1.5 text-zinc-500 font-bold mb-1">
                    <FolderOpen class="w-3.5 h-3.5" />VRChatログフォルダ
                </div>
                <p
                    class="font-mono text-[11px] text-zinc-700 truncate"
                    title={logDir}
                >
                    {logDir}
                </p>
            </div>
        </div>

        <div class="grid grid-cols-1 gap-3 sm:grid-cols-2">
            <div class="rounded-xl border border-[#ECE9DD] bg-[#FAF9F5] p-3">
                <div class="flex items-center gap-1.5 text-zinc-500 font-bold mb-1">
                    <Activity class="w-3.5 h-3.5" />VRChat
                </div>

                {#if runtimeStatusLoading}
                    <p class="text-sm font-extrabold text-amber-700">確認中...</p>
                    <p class="text-[11px] text-zinc-500">
                        起動状態を確認しています
                    </p>
                {:else if runtimeStatus?.vrchat_running}
                    <p class="text-sm font-extrabold text-emerald-700">起動中</p>
                    <p class="text-[11px] text-zinc-500">ログ監視を利用できます</p>
                {:else}
                    <p class="text-sm font-extrabold text-zinc-700">未起動</p>
                    <p class="text-[11px] text-zinc-500">
                        VRChat を起動すると記録できます
                    </p>
                {/if}
            </div>
            <div class="rounded-xl border border-[#ECE9DD] bg-[#FAF9F5] p-3">
                <div class="flex items-center gap-1.5 text-zinc-500 font-bold mb-1">
                    <Activity class="w-3.5 h-3.5" />Watcher
                </div>
                <p class="text-sm font-extrabold text-zinc-800">{watcherLabel}</p>
                <p class="text-[11px] text-zinc-500">
                    訪問履歴 {runtimeStatus?.visit_count ?? 0} 件
                </p>
            </div>
            <div
                class="rounded-xl border border-[#ECE9DD] bg-[#FAF9F5] p-3 min-w-0 sm:col-span-2"
            >
                <div class="grid grid-cols-1 gap-3 md:grid-cols-2">
                    <div class="min-w-0">
                        <div class="text-zinc-500 font-bold mb-1">現在滞在中</div>
                        {#if runtimeStatus?.current_visit}
                            <p
                                class="text-sm font-extrabold text-zinc-800 line-clamp-2"
                                title={runtimeStatus.current_visit.world_name}
                            >
                                {runtimeStatus.current_visit.world_name}
                            </p>
                            <p
                                class="text-[11px] text-zinc-500 truncate"
                                title={runtimeStatus.current_visit.visited_at}
                            >
                                入室 {runtimeStatus.current_visit.visited_at}
                            </p>
                            <p class="text-[11px] text-zinc-500">滞在時間 滞在中</p>
                        {:else}
                            <p class="text-[11px] font-semibold text-zinc-700">
                                なし
                            </p>
                        {/if}
                    </div>
                    <div class="min-w-0 border-t border-[#ECE9DD] pt-3 md:border-l md:border-t-0 md:pl-3 md:pt-0">
                        <div class="text-zinc-500 font-bold mb-1">最新の訪問</div>
                        <p
                            class="text-[11px] font-semibold text-zinc-700 line-clamp-2"
                            title={latestVisit}
                        >
                            {latestVisit}
                        </p>
                    </div>
                </div>
            </div>
        </div>
    </div>

    {#if error}
        <div
            class="flex items-start gap-2 rounded-xl border border-amber-200 bg-amber-50 px-3 py-2 text-xs text-amber-900"
        >
            <TriangleAlert class="w-4 h-4 shrink-0 mt-0.5" />
            <span class="break-words">{error}</span>
        </div>
    {/if}
</section>
