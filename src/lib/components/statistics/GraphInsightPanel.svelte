<script lang="ts">
    import type {
        TransitionGraphEdge,
        TransitionGraphNode,
        TransitionGraphSummary,
    } from "../../api/commands";

    export let summary: TransitionGraphSummary;
    export let nodes: TransitionGraphNode[];
    export let edges: TransitionGraphEdge[];
    export let selectedNodeKey: string | null = null;
    export let selectedEdgeKey: string | null = null;
    export let onOpenWorldDetail: (
        worldId: string | null,
        worldName: string,
    ) => void;

    $: selectedNode = selectedNodeKey
        ? nodes.find((n) => n.key === selectedNodeKey) ?? null
        : null;
    $: selectedEdge = selectedEdgeKey
        ? edges.find((e) => e.key === selectedEdgeKey) ?? null
        : null;

    function formatSeconds(seconds: number): string {
        const h = Math.floor(seconds / 3600);
        const m = Math.floor((seconds % 3600) / 60);
        if (h > 0) return `${h}時間${m}分`;
        return `${m}分`;
    }

    function formatDate(iso: string): string {
        try {
            return new Date(iso).toLocaleString("ja-JP", {
                year: "numeric",
                month: "2-digit",
                day: "2-digit",
                hour: "2-digit",
                minute: "2-digit",
            });
        } catch {
            return iso;
        }
    }

    $: inboundEdges = selectedNode
        ? edges
              .filter((e) => e.to === selectedNode!.key)
              .sort((a, b) => b.transition_count - a.transition_count)
              .slice(0, 5)
        : [];
    $: outboundEdges = selectedNode
        ? edges
              .filter((e) => e.from === selectedNode!.key)
              .sort((a, b) => b.transition_count - a.transition_count)
              .slice(0, 5)
        : [];
    $: connectedCount = selectedNode
        ? new Set(
              edges
                  .filter(
                      (e) =>
                          e.from === selectedNode!.key ||
                          e.to === selectedNode!.key,
                  )
                  .flatMap((e) => [e.from, e.to])
                  .filter((k) => k !== selectedNode!.key),
          ).size
        : 0;
</script>

<div class="h-full overflow-y-auto p-4 space-y-4">
    {#if selectedNode}
        <div class="space-y-3">
            <div>
                <div class="text-xs text-zinc-400 font-semibold uppercase tracking-wider mb-1">ワールド</div>
                <div class="text-sm font-bold text-zinc-800 break-words">{selectedNode.world_name}</div>
                {#if selectedNode.world_id}
                    <div class="text-[10px] text-zinc-400 mt-0.5 break-all">{selectedNode.world_id}</div>
                {/if}
            </div>
            <div class="grid grid-cols-2 gap-2">
                <div class="bg-zinc-50 rounded-lg p-2">
                    <div class="text-[10px] text-zinc-400">訪問回数</div>
                    <div class="text-sm font-bold text-zinc-700">{selectedNode.visit_count}回</div>
                </div>
                <div class="bg-zinc-50 rounded-lg p-2">
                    <div class="text-[10px] text-zinc-400">合計滞在</div>
                    <div class="text-sm font-bold text-zinc-700">{formatSeconds(selectedNode.total_stay_seconds)}</div>
                </div>
                <div class="bg-zinc-50 rounded-lg p-2">
                    <div class="text-[10px] text-zinc-400">最終訪問</div>
                    <div class="text-xs font-medium text-zinc-700">{formatDate(selectedNode.last_visited_at)}</div>
                </div>
                <div class="bg-zinc-50 rounded-lg p-2">
                    <div class="text-[10px] text-zinc-400">接続ワールド</div>
                    <div class="text-sm font-bold text-zinc-700">{connectedCount}件</div>
                </div>
            </div>
            {#if inboundEdges.length > 0}
                <div>
                    <div class="text-xs text-zinc-400 font-semibold mb-1">主な移動元</div>
                    <div class="space-y-1">
                        {#each inboundEdges as edge}
                            <div class="text-xs text-zinc-600 bg-zinc-50 rounded px-2 py-1 flex justify-between">
                                <span class="truncate">{edge.from_world_name}</span>
                                <span class="text-zinc-400 ml-2 shrink-0">{edge.transition_count}回</span>
                            </div>
                        {/each}
                    </div>
                </div>
            {/if}
            {#if outboundEdges.length > 0}
                <div>
                    <div class="text-xs text-zinc-400 font-semibold mb-1">主な移動先</div>
                    <div class="space-y-1">
                        {#each outboundEdges as edge}
                            <div class="text-xs text-zinc-600 bg-zinc-50 rounded px-2 py-1 flex justify-between">
                                <span class="truncate">{edge.to_world_name}</span>
                                <span class="text-zinc-400 ml-2 shrink-0">{edge.transition_count}回</span>
                            </div>
                        {/each}
                    </div>
                </div>
            {/if}
            {#if selectedNode.world_id}
                <button
                    onclick={() =>
                        onOpenWorldDetail(
                            selectedNode!.world_id,
                            selectedNode!.world_name,
                        )}
                    class="w-full text-xs py-2 bg-[#1e5854] text-white rounded-lg hover:bg-[#133c39] transition-colors font-medium"
                >
                    ワールド詳細を開く
                </button>
            {/if}
        </div>
    {:else if selectedEdge}
        <div class="space-y-3">
            <div>
                <div class="text-xs text-zinc-400 font-semibold uppercase tracking-wider mb-1">移動</div>
                <div class="text-sm font-medium text-zinc-700 break-words">{selectedEdge.from_world_name}</div>
                <div class="text-xs text-zinc-400 my-1">↓</div>
                <div class="text-sm font-medium text-zinc-700 break-words">{selectedEdge.to_world_name}</div>
            </div>
            <div class="grid grid-cols-2 gap-2">
                <div class="bg-zinc-50 rounded-lg p-2">
                    <div class="text-[10px] text-zinc-400">移動回数</div>
                    <div class="text-sm font-bold text-zinc-700">{selectedEdge.transition_count}回</div>
                </div>
                <div class="bg-zinc-50 rounded-lg p-2">
                    <div class="text-[10px] text-zinc-400">直近の移動</div>
                    <div class="text-xs font-medium text-zinc-700">{formatDate(selectedEdge.latest_transition_at)}</div>
                </div>
            </div>
            {#if selectedEdge.transition_times.length > 0}
                <div>
                    <div class="text-xs text-zinc-400 font-semibold mb-1">移動日時一覧</div>
                    <div class="space-y-1 max-h-48 overflow-y-auto">
                        {#each selectedEdge.transition_times as time}
                            <div class="text-xs text-zinc-600 bg-zinc-50 rounded px-2 py-1">{formatDate(time)}</div>
                        {/each}
                    </div>
                </div>
            {/if}
        </div>
    {:else}
        <div class="space-y-3">
            <div class="text-xs text-zinc-400 font-semibold uppercase tracking-wider">期間の概要</div>
            <div class="space-y-2">
                <div class="bg-zinc-50 rounded-lg p-3">
                    <div class="text-[10px] text-zinc-400">訪問回数</div>
                    <div class="text-lg font-bold text-zinc-800">{summary.visit_count.toLocaleString()}</div>
                </div>
                <div class="bg-zinc-50 rounded-lg p-3">
                    <div class="text-[10px] text-zinc-400">ユニークワールド数</div>
                    <div class="text-lg font-bold text-zinc-800">{summary.unique_world_count.toLocaleString()}</div>
                </div>
                <div class="bg-zinc-50 rounded-lg p-3">
                    <div class="text-[10px] text-zinc-400">移動回数</div>
                    <div class="text-lg font-bold text-zinc-800">{summary.transition_count.toLocaleString()}</div>
                </div>
                {#if summary.top_transition}
                    <div class="bg-zinc-50 rounded-lg p-3">
                        <div class="text-[10px] text-zinc-400 mb-1">最も多い移動</div>
                        <div class="text-xs text-zinc-700">{summary.top_transition.from_world_name}</div>
                        <div class="text-[10px] text-zinc-400">↓ {summary.top_transition.transition_count}回</div>
                        <div class="text-xs text-zinc-700">{summary.top_transition.to_world_name}</div>
                    </div>
                {/if}
            </div>
        </div>
    {/if}
</div>
