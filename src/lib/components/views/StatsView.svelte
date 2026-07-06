<script lang="ts">
    import { onMount } from "svelte";
    import {
        getStatsDateRange,
        getVisitTransitionGraph as getVisitTransitionStats,
    } from "../../api/commands";
    import StatsDateRangePicker from "../statistics/StatsDateRangePicker.svelte";

    type Props = {
        onOpenWorldDetail: (worldId: string | null, worldName: string) => void;
    };

    let { onOpenWorldDetail }: Props = $props();

    function toDateString(d: Date): string {
        const y = d.getFullYear();
        const m = String(d.getMonth() + 1).padStart(2, "0");
        const day = String(d.getDate()).padStart(2, "0");
        return `${y}-${m}-${day}`;
    }

    const today = new Date();
    const initialEnd = toDateString(today);
    const d20 = new Date(today);
    d20.setDate(d20.getDate() - 20);
    const initialStart = toDateString(d20);

    let startDate = $state(initialStart);
    let endDate = $state(initialEnd);
    let dateError: string | null = $state(null);
    type VisitTransitionStatsApiResponse = Awaited<
        ReturnType<typeof getVisitTransitionStats>
    >;
    type WorldStats = VisitTransitionStatsApiResponse["nodes"][number];
    type TransitionStats = VisitTransitionStatsApiResponse["edges"][number];

    type StatsSummary = {
        visitCount: number;
        uniqueWorldCount: number;
        transitionCount: number;
        topTransition: TransitionStats | null;
        topLongestVisit: VisitTransitionStatsApiResponse["summary"]["top_longest_visit"];
        hiddenWorldCount: number;
        hiddenTransitionCount: number;
    };

    type StatsData = {
        summary: StatsSummary;
        worlds: WorldStats[];
        transitions: TransitionStats[];
    };

    let statsData: StatsData | null = $state(null);
    let loading = $state(false);
    let loadError: string | null = $state(null);

    function normalizeStats(
        apiData: VisitTransitionStatsApiResponse,
    ): StatsData {
        return {
            summary: {
                visitCount: apiData.summary.visit_count,
                uniqueWorldCount: apiData.summary.unique_world_count,
                transitionCount: apiData.summary.transition_count,
                topTransition: apiData.summary.top_transition,
                topLongestVisit: apiData.summary.top_longest_visit,
                hiddenWorldCount: apiData.summary.hidden_node_count,
                hiddenTransitionCount: apiData.summary.hidden_edge_count,
            },
            worlds: apiData.top_worlds,
            transitions: apiData.top_transitions,
        };
    }

    function getTopWorlds(currentStats: StatsData | null): WorldStats[] {
        return currentStats
            ? [...currentStats.worlds]
                  .sort(
                      (a, b) =>
                          b.visit_count - a.visit_count ||
                          b.total_stay_seconds - a.total_stay_seconds ||
                          b.last_visited_at.localeCompare(a.last_visited_at) ||
                          a.world_name.localeCompare(b.world_name),
                  )
                  .slice(0, 5)
            : [];
    }

    function getTopTransitions(
        currentStats: StatsData | null,
    ): TransitionStats[] {
        return currentStats
            ? [...currentStats.transitions]
                  .sort(
                      (a, b) =>
                          b.transition_count - a.transition_count ||
                          b.latest_transition_at.localeCompare(
                              a.latest_transition_at,
                          ) ||
                          a.from_world_name.localeCompare(b.from_world_name) ||
                          a.to_world_name.localeCompare(b.to_world_name),
                  )
                  .slice(0, 5)
            : [];
    }

    let topWorlds = $derived(getTopWorlds(statsData));

    let topTransitions = $derived(getTopTransitions(statsData));

    let maxVisitCount = $derived(
        topWorlds.reduce((max, world) => Math.max(max, world.visit_count), 0),
    );

    let maxTransitionCount = $derived(
        topTransitions.reduce(
            (max, transition) => Math.max(max, transition.transition_count),
            0,
        ),
    );

    async function loadStatsData(start: string, end: string) {
        loading = true;
        loadError = null;
        try {
            statsData = normalizeStats(
                await getVisitTransitionStats({ start, end }),
            );
        } catch (e) {
            loadError = "統計データを読み込めませんでした";
        } finally {
            loading = false;
        }
    }

    function handleDateChange(newStart: string, newEnd: string) {
        startDate = newStart;
        endDate = newEnd;
        dateError = null;
        loadStatsData(newStart, newEnd);
    }

    function formatDisplayDate(d: string): string {
        return d.replace(/-/g, "/");
    }

    function formatDateTime(value: string): string {
        const date = new Date(
            value.includes("T") ? value : value.replace(" ", "T"),
        );
        if (Number.isNaN(date.getTime())) return value;

        return new Intl.DateTimeFormat("ja-JP", {
            year: "numeric",
            month: "2-digit",
            day: "2-digit",
            hour: "2-digit",
            minute: "2-digit",
            hour12: false,
        }).format(date);
    }

    function formatSeconds(seconds: number): string {
        if (seconds <= 0) return "0分";

        const hours = Math.floor(seconds / 3600);
        const minutes = Math.floor((seconds % 3600) / 60);

        if (hours > 0 && minutes > 0) return `${hours}時間${minutes}分`;
        if (hours > 0) return `${hours}時間`;
        return `${minutes}分`;
    }

    function barWidth(value: number, maxValue: number): number {
        if (maxValue <= 0 || value <= 0) return 0;
        return Math.max(8, Math.round((value / maxValue) * 100));
    }

    const fallbackStart = initialStart;
    const fallbackEnd = initialEnd;

    async function initializeStats() {
        loading = true;
        loadError = null;

        let nextStart = fallbackStart;
        let nextEnd = fallbackEnd;

        try {
            const range = await getStatsDateRange();
            nextStart = range.start ?? fallbackStart;
            nextEnd = range.end ?? fallbackEnd;
        } catch {
            // フォールバック値を使う
        }

        startDate = nextStart;
        endDate = nextEnd;

        try {
            statsData = normalizeStats(
                await getVisitTransitionStats({
                    start: nextStart,
                    end: nextEnd,
                }),
            );
        } catch {
            loadError = "統計データを読み込めませんでした";
        } finally {
            loading = false;
        }
    }

    onMount(() => {
        initializeStats();
    });
</script>

<section class="space-y-6 animate-fadeIn">
    <div class="rounded-3xl border border-[#DEDAC4] bg-[#FAF9F5] p-6 shadow-sm">
        <div class="mb-4">
            <h2 class="text-2xl font-black text-zinc-800 tracking-tight">
                統計
            </h2>
            <p class="text-sm text-zinc-500 mt-0.5">
                選択期間の訪問傾向を集計します
            </p>
            <p class="text-xs text-zinc-400 mt-1">
                {formatDisplayDate(startDate)} 〜 {formatDisplayDate(endDate)}
            </p>
        </div>
        <StatsDateRangePicker
            {startDate}
            {endDate}
            error={dateError}
            onChange={handleDateChange}
        />
    </div>

    {#if loading}
        <div
            class="rounded-3xl border border-zinc-100 bg-white p-12 text-center shadow-sm"
        >
            <span class="text-sm text-zinc-400"
                >統計データを集計しています...</span
            >
        </div>
    {:else if loadError}
        <div
            class="rounded-3xl border border-zinc-100 bg-white p-12 text-center shadow-sm"
        >
            <div class="flex flex-col items-center justify-center gap-3">
                <span class="text-sm text-zinc-500">{loadError}</span>
                <button
                    onclick={() => loadStatsData(startDate, endDate)}
                    class="text-xs px-4 py-2 bg-[#1e5854] text-white rounded-lg hover:bg-[#133c39] transition-colors"
                >
                    再読み込み
                </button>
            </div>
        </div>
    {:else if statsData && statsData.worlds.length === 0}
        <div
            class="rounded-3xl border border-zinc-100 bg-white p-12 text-center shadow-sm"
        >
            <div class="flex flex-col items-center justify-center gap-2 px-6">
                <span class="text-sm font-medium text-zinc-500"
                    >この期間の訪問履歴はありません</span
                >
                <span class="text-xs text-zinc-400"
                    >VRChatを起動してワールドを訪問すると、ここに統計が表示されます。</span
                >
            </div>
        </div>
    {:else if statsData}
        <div class="grid grid-cols-1 sm:grid-cols-2 xl:grid-cols-4 gap-4">
            <div
                class="rounded-2xl border border-zinc-100 bg-white p-5 shadow-sm"
            >
                <div class="text-xs text-zinc-400 font-semibold">訪問回数</div>
                <div class="mt-2 text-3xl font-black text-zinc-800">
                    {statsData.summary.visitCount.toLocaleString()}
                </div>
            </div>
            <div
                class="rounded-2xl border border-zinc-100 bg-white p-5 shadow-sm"
            >
                <div class="text-xs text-zinc-400 font-semibold">
                    訪れたワールド
                </div>
                <div class="mt-2 text-3xl font-black text-zinc-800">
                    {statsData.summary.uniqueWorldCount.toLocaleString()}
                </div>
            </div>
            <div
                class="rounded-2xl border border-zinc-100 bg-white p-5 shadow-sm"
            >
                <div class="text-xs text-zinc-400 font-semibold">移動回数</div>
                <div class="mt-2 text-3xl font-black text-zinc-800">
                    {statsData.summary.transitionCount.toLocaleString()}
                </div>
            </div>
            <div
                class="rounded-2xl border border-zinc-100 bg-white p-5 shadow-sm"
            >
                <div class="text-xs text-zinc-400 font-semibold">
                    最も滞在時間が長いワールド
                </div>
                {#if statsData.summary.topLongestVisit}
                    <div
                        class="mt-2 text-sm font-bold text-zinc-700 line-clamp-2 break-words"
                        title={statsData.summary.topLongestVisit.world_name}
                    >
                        {statsData.summary.topLongestVisit.world_name}
                    </div>
                    <div class="mt-1 text-xs text-zinc-400">
                        滞在 {formatSeconds(statsData.summary.topLongestVisit.stay_duration_seconds)} ・ 訪問 {formatDateTime(statsData.summary.topLongestVisit.visited_at)}
                    </div>
                {:else}
                    <div class="mt-2 text-2xl font-black text-zinc-300">—</div>
                {/if}
            </div>
        </div>

        <div class="grid grid-cols-1 xl:grid-cols-2 gap-6">
            <div
                class="rounded-3xl border border-zinc-100 bg-white p-6 shadow-sm"
            >
                <div class="mb-5">
                    <h3 class="text-lg font-black text-zinc-800">
                        よく行ったワールド
                    </h3>
                    <p class="text-xs text-zinc-400 mt-1">
                        訪問回数順の上位5件
                    </p>
                </div>

                <div class="space-y-4">
                    {#each topWorlds as world, index (world.key)}
                        <div
                            class="rounded-2xl border border-zinc-100 bg-[#FAF9F5] p-4"
                        >
                            <div class="flex items-start gap-3">
                                <div
                                    class="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-[#1e5854] text-xs font-black text-white"
                                >
                                    {index + 1}
                                </div>
                                <div class="min-w-0 flex-1">
                                    <div
                                        class="flex items-start justify-between gap-3"
                                    >
                                        <div class="min-w-0">
                                            <div
                                                class="line-clamp-2 break-words text-sm font-black text-zinc-800"
                                                title={world.world_name}
                                            >
                                                {world.world_name}
                                            </div>
                                            <div
                                                class="mt-1 flex flex-wrap gap-x-3 gap-y-1 text-xs text-zinc-500"
                                            >
                                                <span
                                                    >{world.visit_count.toLocaleString()}回</span
                                                >
                                                <span
                                                    >滞在 {formatSeconds(
                                                        world.total_stay_seconds,
                                                    )}</span
                                                >
                                                <span
                                                    >最終 {formatDateTime(
                                                        world.last_visited_at,
                                                    )}</span
                                                >
                                            </div>
                                        </div>
                                        {#if world.world_id}
                                            <button
                                                type="button"
                                                onclick={() =>
                                                    onOpenWorldDetail(
                                                        world.world_id,
                                                        world.world_name,
                                                    )}
                                                class="shrink-0 rounded-lg border border-[#1e5854]/20 bg-white px-3 py-1.5 text-xs font-bold text-[#1e5854] hover:bg-[#1e5854] hover:text-white transition-colors"
                                            >
                                                詳細
                                            </button>
                                        {/if}
                                    </div>
                                    <div
                                        class="mt-3 h-2 overflow-hidden rounded-full bg-zinc-100"
                                    >
                                        <div
                                            class="h-full rounded-full bg-[#1e5854]"
                                            style={`width: ${barWidth(world.visit_count, maxVisitCount)}%`}
                                        ></div>
                                    </div>
                                </div>
                            </div>
                        </div>
                    {/each}
                </div>
            </div>

            <div
                class="rounded-3xl border border-zinc-100 bg-white p-6 shadow-sm"
            >
                <div class="mb-5">
                    <h3 class="text-lg font-black text-zinc-800">
                        よくある移動
                    </h3>
                    <p class="text-xs text-zinc-400 mt-1">
                        移動回数順の上位5件
                    </p>
                </div>

                {#if topTransitions.length === 0}
                    <div
                        class="rounded-2xl border border-dashed border-zinc-200 bg-[#FAF9F5] p-8 text-center text-sm text-zinc-400"
                    >
                        この期間の移動データはまだありません
                    </div>
                {:else}
                    <div class="space-y-4">
                        {#each topTransitions as transition, index (transition.key)}
                            <div
                                class="rounded-2xl border border-zinc-100 bg-[#FAF9F5] p-4"
                            >
                                <div class="flex items-start gap-3">
                                    <div
                                        class="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-[#6B5B3E] text-xs font-black text-white"
                                    >
                                        {index + 1}
                                    </div>
                                    <div class="min-w-0 flex-1">
                                        <div class="min-w-0 space-y-1 text-sm font-black text-zinc-800">
                                            <div class="min-w-0">
                                                <span
                                                    class="mr-1 text-[10px] font-bold uppercase tracking-wider text-zinc-400"
                                                    >From</span
                                                >
                                                <span
                                                    class="align-middle line-clamp-2 break-words"
                                                    title={transition.from_world_name}
                                                    >{transition.from_world_name}</span
                                                >
                                            </div>
                                            <div class="min-w-0">
                                                <span
                                                    class="mr-1 text-[10px] font-bold uppercase tracking-wider text-zinc-400"
                                                    >To</span
                                                >
                                                <span
                                                    class="align-middle line-clamp-2 break-words"
                                                    title={transition.to_world_name}
                                                    >{transition.to_world_name}</span
                                                >
                                            </div>
                                        </div>
                                        <div
                                            class="mt-1 flex flex-wrap gap-x-3 gap-y-1 text-xs text-zinc-500"
                                        >
                                            <span
                                                >{transition.transition_count.toLocaleString()}回</span
                                            >
                                            <span
                                                >直近 {formatDateTime(
                                                    transition.latest_transition_at,
                                                )}</span
                                            >
                                        </div>
                                        <div
                                            class="mt-3 h-2 overflow-hidden rounded-full bg-zinc-100"
                                        >
                                            <div
                                                class="h-full rounded-full bg-[#6B5B3E]"
                                                style={`width: ${barWidth(transition.transition_count, maxTransitionCount)}%`}
                                            ></div>
                                        </div>
                                    </div>
                                </div>
                            </div>
                        {/each}
                    </div>
                {/if}
            </div>
        </div>
    {/if}
</section>
