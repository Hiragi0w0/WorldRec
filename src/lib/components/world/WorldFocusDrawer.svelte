<script lang="ts">
    import { X } from "lucide-svelte";
    import type { VisitRecord } from "../../data/visitTypes";
    import {
        calculateWorldVisitStats,
        type WorldVisitStats,
    } from "../../utils/history";
    import type { WorldDetailPreview } from "../../world/worldDetailPreview";

    export let isOpen: boolean;
    export let selectedRecord: VisitRecord | null;
    export let detail: WorldDetailPreview | null;
    export let detailLoading = false;
    export let detailError: string | null = null;
    export let relatedVisits: VisitRecord[];
    export let onClose: () => void;

    let thumbnailFailed = false;
    let lastThumbnailUrl: string | null = null;

    $: stats = calculateWorldVisitStats(relatedVisits);
    $: recentVisits = [...relatedVisits]
        .sort((a, b) =>
            `${b.dateKey}T${b.time}`.localeCompare(`${a.dateKey}T${a.time}`),
        )
        .slice(0, 5);
    $: {
        const nextThumbnailUrl = detail?.thumbnailUrl ?? null;
        if (nextThumbnailUrl !== lastThumbnailUrl) {
            lastThumbnailUrl = nextThumbnailUrl;
            thumbnailFailed = false;
        }
    }

    function formatValue(value: string | null | undefined) {
        return value?.trim() || "不明";
    }

    function formatCapacity(value: number | null) {
        return value === null ? "不明" : `${value}人`;
    }

    function formatStay(seconds: number) {
        if (seconds <= 0) return "0分";

        const minutes = Math.floor(seconds / 60);
        const hours = Math.floor(minutes / 60);
        const remainingMinutes = minutes % 60;

        if (hours > 0 && remainingMinutes > 0) {
            return `${hours}時間${remainingMinutes}分`;
        }
        if (hours > 0) return `${hours}時間`;
        return `${minutes}分`;
    }

    function formatVisitDate(record: VisitRecord | null) {
        if (!record) return "不明";
        return `${record.dateKey} ${record.time}`;
    }

    function statItems(value: WorldVisitStats) {
        return [
            ["訪問回数", `${value.visitCount}回`],
            ["合計滞在", formatStay(value.totalStaySeconds)],
            ["初回訪問", formatVisitDate(value.firstVisit)],
            ["最終訪問", formatVisitDate(value.latestVisit)],
        ];
    }

    function handleOverlayClick(event: MouseEvent) {
        if (event.target === event.currentTarget) {
            onClose();
        }
    }

    function hideBrokenThumbnail() {
        thumbnailFailed = true;
    }
</script>

{#if isOpen && selectedRecord && detail}
    <div
        class="fixed inset-0 z-50 flex items-center justify-center bg-black/35 px-4 py-6 backdrop-blur-[1px] animate-fadeIn"
        onclick={handleOverlayClick}
        role="presentation"
    >
        <div
            class="max-h-[90vh] w-full max-w-3xl overflow-y-auto rounded-3xl border border-[#DEDAC4] bg-[#FAF9F5] p-5 shadow-2xl sm:p-6"
            role="dialog"
            aria-modal="true"
            aria-labelledby="world-focus-drawer-title"
            tabindex="-1"
        >
            <div
                class="flex items-center justify-between gap-4 border-b border-[#E4DEC9] pb-4"
            >
                <div>
                    <span
                        class="text-[10px] text-[#1e5854] font-bold tracking-widest uppercase"
                    >
                        World Detail
                    </span>
                    <h2
                        id="world-focus-drawer-title"
                        class="mt-1 text-2xl font-black text-zinc-800 tracking-tight"
                    >
                        {detail.worldName}
                    </h2>
                </div>
                <button
                    type="button"
                    onclick={onClose}
                    class="shrink-0 p-2 rounded-lg bg-white text-zinc-400 hover:text-[#1e5854] border border-[#DEDAC4] hover:border-[#1e5854]/40 shadow-sm transition-colors"
                    title="閉じる"
                >
                    <X class="w-4 h-4" />
                    <span class="sr-only">閉じる</span>
                </button>
            </div>

            <div class="mt-5 space-y-5">
                <section
                    class="overflow-hidden rounded-2xl border border-dashed border-[#D9CBA8] bg-[#FCFAF7] shadow-sm"
                >
                    {#if detailLoading || detailError}
                        <div class="border-b border-[#EFE8D7] p-4">
                            {#if detailLoading}
                                <p class="text-xs font-bold text-[#1e5854]">
                                    VRChat API から詳細を取得中...
                                </p>
                            {/if}
                            {#if detailError}
                                <p
                                    class="mt-2 rounded-lg border border-amber-200 bg-amber-50 px-3 py-2 text-xs font-bold text-amber-700"
                                >
                                    {detailError}
                                </p>
                            {/if}
                        </div>
                    {/if}

                    <div
                        class="mx-4 mt-4 flex h-64 items-center justify-center overflow-hidden rounded-2xl border border-[#DEDAC4] bg-[#F3F0E6] text-xs font-bold tracking-widest text-[#8C7B58] uppercase"
                    >
                        {#if detail.thumbnailUrl && !thumbnailFailed}
                            <img
                                src={detail.thumbnailUrl}
                                alt={detail.worldName}
                                loading="lazy"
                                onerror={hideBrokenThumbnail}
                                class="h-full w-full object-cover"
                            />
                        {:else}
                            <span>画像なし</span>
                        {/if}
                    </div>

                    <div class="p-4">
                        <p class="mt-3 text-sm leading-7 text-zinc-700">
                            {detail.description}
                        </p>

                        <dl
                            class="mt-4 grid grid-cols-1 gap-3 text-sm sm:grid-cols-3"
                        >
                            <div>
                                <dt
                                    class="text-[10px] font-bold text-zinc-400 uppercase"
                                >
                                    Author
                                </dt>
                                <dd class="font-bold text-zinc-700">
                                    {detail.authorName}
                                </dd>
                            </div>
                            <div>
                                <dt
                                    class="text-[10px] font-bold text-zinc-400 uppercase"
                                >
                                    Platform
                                </dt>
                                <dd class="font-bold text-zinc-700">
                                    {detail.platform}
                                </dd>
                            </div>
                            <div>
                                <dt
                                    class="text-[10px] font-bold text-zinc-400 uppercase"
                                >
                                    Capacity
                                </dt>
                                <dd class="font-bold text-zinc-700">
                                    {formatCapacity(detail.capacity)}
                                </dd>
                            </div>
                        </dl>
                    </div>
                </section>

                <section
                    class="rounded-2xl border border-[#E4DEC9] bg-white/80 p-4 shadow-sm"
                >
                    <div class="flex items-center justify-between gap-3">
                        <h4
                            class="text-xs font-black text-zinc-500 tracking-widest uppercase"
                        >
                            Visit Record
                        </h4>
                    </div>
                    <dl class="mt-3 grid grid-cols-1 gap-3 text-sm">
                        <div
                            class="rounded-xl border border-[#EFE8D7] bg-[#FCFAF7] p-3"
                        >
                            <dt
                                class="text-[10px] font-bold text-zinc-400 uppercase"
                            >
                                Access Type
                            </dt>
                            <dd class="font-bold text-zinc-700">
                                {selectedRecord.instanceType}
                            </dd>
                        </div>
                        <div
                            class="rounded-xl border border-[#EFE8D7] bg-[#FCFAF7] p-3"
                        >
                            <dt
                                class="text-[10px] font-bold text-zinc-400 uppercase"
                            >
                                World ID
                            </dt>
                            <dd
                                class="mt-1 break-all font-mono text-[11px] text-zinc-500"
                            >
                                {formatValue(selectedRecord.worldId)}
                            </dd>
                        </div>
                    </dl>

                    <h5
                        class="mt-5 text-xs font-black text-zinc-500 tracking-widest uppercase"
                    >
                        Visit Stats
                    </h5>
                    <div class="mt-3 grid grid-cols-1 gap-3 sm:grid-cols-2">
                        {#each statItems(stats) as [label, value]}
                            <div
                                class="rounded-xl border border-[#EFE8D7] bg-[#FCFAF7] p-3"
                            >
                                <div
                                    class="text-[10px] font-bold uppercase text-zinc-400"
                                >
                                    {label}
                                </div>
                                <div
                                    class="mt-1 text-base font-black text-zinc-800 sm:text-lg"
                                >
                                    {value}
                                </div>
                            </div>
                        {/each}
                    </div>

                    <div class="mt-5 flex items-center justify-between gap-3">
                        <h5
                            class="text-xs font-black text-zinc-500 tracking-widest uppercase"
                        >
                            Recent Visits
                        </h5>
                        <span class="text-[10px] font-bold text-zinc-400">
                            最新{recentVisits.length}件
                        </span>
                    </div>
                    <div class="mt-3 divide-y divide-[#EFE8D7]">
                        {#each recentVisits as visit}
                            <div class="py-3">
                                <div
                                    class="flex flex-wrap items-center gap-x-3 gap-y-1"
                                >
                                    <span
                                        class="font-mono text-xs font-bold text-[#1e5854]"
                                    >
                                        {visit.dateKey}
                                        {visit.time}
                                    </span>
                                    <span
                                        class="rounded-full bg-[#FAF9F5] border border-zinc-200 px-2 py-0.5 text-[10px] font-bold text-zinc-500"
                                    >
                                        {visit.instanceType}
                                    </span>
                                    <span
                                        class="text-xs font-bold text-zinc-400"
                                    >
                                        {visit.stayLabel ??
                                            formatStay(
                                                visit.staySeconds ??
                                                    visit.stayMinutes * 60,
                                            )}
                                    </span>
                                </div>
                                <p
                                    class="mt-1 line-clamp-2 text-sm text-zinc-600"
                                >
                                    {visit.memo}
                                </p>
                            </div>
                        {/each}
                    </div>
                </section>
            </div>
        </div>
    </div>
{/if}
