<script lang="ts">
    import {
        AlertCircle,
        Clock,
        Image,
        Loader2,
        MapPin,
        X,
    } from "lucide-svelte";
    import {
        getLibraryWorldDetail,
        getVrchatWorldDetail,
        type LibraryWorld,
        type LibraryWorldDetail,
        type LibraryWorldVisit,
    } from "../../api/commands";
    import type { VisitRecord } from "../../data/visitTypes";
    import { formatStayDuration } from "../../data/visitMapper";
    import {
        mapVrchatWorldDetailToPreview,
        type WorldDetailPreview,
    } from "../../world/worldDetailPreview";

    export let isOpen: boolean;
    export let worldId: string | null;
    export let worldName: string;
    export let onClose: () => void;

    let detail: LibraryWorldDetail | null = null;
    let detailLoading = false;
    let detailError: string | null = null;
    let preview: WorldDetailPreview | null = null;
    let previewLoading = false;
    let previewError: string | null = null;
    let activeLoadKey = "";
    let requestSeq = 0;

    $: if (isOpen) {
        const nextKey = `${worldId ?? ""}|${worldName}`;
        if (nextKey !== activeLoadKey) {
            activeLoadKey = nextKey;
            void loadDrawerDetail(worldId, worldName);
        }
    } else if (activeLoadKey) {
        activeLoadKey = "";
        requestSeq += 1;
        detail = null;
        detailError = null;
        detailLoading = false;
        preview = null;
        previewError = null;
        previewLoading = false;
    }

    async function loadDrawerDetail(nextWorldId: string | null, nextWorldName: string) {
        const currentSeq = ++requestSeq;
        detail = null;
        detailError = null;
        detailLoading = true;
        preview = null;
        previewError = null;
        previewLoading = false;

        try {
            const loadedDetail = await getLibraryWorldDetail(
                nextWorldId,
                nextWorldName,
            );
            if (currentSeq !== requestSeq) return;

            detail = loadedDetail;
            void loadPreview(loadedDetail.world, currentSeq);
        } catch (error) {
            if (currentSeq === requestSeq) {
                detailError = toErrorMessage(error);
            }
        } finally {
            if (currentSeq === requestSeq) {
                detailLoading = false;
            }
        }
    }

    async function loadPreview(world: LibraryWorld, currentSeq: number) {
        if (!world.world_id?.startsWith("wrld_")) return;

        previewLoading = true;
        previewError = null;

        try {
            const apiDetail = await getVrchatWorldDetail(
                world.world_id,
                world.world_name,
            );
            const nextPreview = mapVrchatWorldDetailToPreview(
                buildPreviewRecord(world),
                apiDetail,
            );
            if (currentSeq === requestSeq) {
                preview = nextPreview;
            }
        } catch (error) {
            console.warn(
                `get_vrchat_world_detail failed for library detail ${world.world_id}: ${toErrorMessage(error)}`,
            );
            if (currentSeq === requestSeq) {
                previewError = "VRChat API から詳細を取得できませんでした。";
            }
        } finally {
            if (currentSeq === requestSeq) {
                previewLoading = false;
            }
        }
    }

    function buildPreviewRecord(world: LibraryWorld): VisitRecord {
        return {
            id: world.key,
            worldId: world.world_id ?? "",
            worldName: world.world_name,
            time: "--:--",
            dateKey: dateKey(world.last_visited_at),
            instanceType: "UNKNOWN",
            stayMinutes: 0,
            staySeconds: world.total_stay_duration_seconds,
            stayLabel: formatStayDuration(world.total_stay_duration_seconds),
            memo: "",
        };
    }

    function visitStayLabel(visit: LibraryWorldVisit) {
        return visit.stay_duration_seconds === null
            ? "未確定"
            : formatStayDuration(visit.stay_duration_seconds);
    }

    function formatMaybeDate(value: string | null | undefined) {
        if (!value) return "不明";
        const date = parseDateTime(value);
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

    function dateKey(value: string) {
        const date = parseDateTime(value);
        if (Number.isNaN(date.getTime())) return value.slice(0, 10) || "";

        const year = date.getFullYear();
        const month = String(date.getMonth() + 1).padStart(2, "0");
        const day = String(date.getDate()).padStart(2, "0");
        return `${year}-${month}-${day}`;
    }

    function parseDateTime(value: string) {
        return new Date(value.includes("T") ? value : value.replace(" ", "T"));
    }

    function formatCapacity(value: number | null | undefined) {
        return value === null || value === undefined ? "不明" : `${value}人`;
    }

    function formatText(value: string | null | undefined, fallback = "不明") {
        return value?.trim() || fallback;
    }

    function handleOverlayClick(event: MouseEvent) {
        if (event.target === event.currentTarget) {
            onClose();
        }
    }

    function toErrorMessage(error: unknown) {
        if (error instanceof Error) return error.message;
        if (typeof error === "string") return error;
        return "Tauri command failed.";
    }
</script>

{#if isOpen}
    <div
        class="fixed inset-0 z-50 flex items-center justify-center bg-black/35 px-4 py-6 backdrop-blur-[1px] animate-fadeIn"
        onclick={handleOverlayClick}
        role="presentation"
    >
        <div
            class="max-h-[90vh] w-full max-w-4xl overflow-y-auto rounded-3xl border border-[#DEDAC4] bg-[#FAF9F5] p-5 shadow-2xl sm:p-6"
            role="dialog"
            aria-modal="true"
            aria-labelledby="library-world-detail-title"
            tabindex="-1"
        >
            <div
                class="flex items-center justify-between gap-4 border-b border-[#E4DEC9] pb-4"
            >
                <div class="min-w-0">
                    <span
                        class="text-[10px] font-bold tracking-widest text-[#1e5854] uppercase"
                    >
                        Library Detail
                    </span>
                    <h2
                        id="library-world-detail-title"
                        class="mt-1 truncate text-2xl font-black tracking-tight text-zinc-800"
                    >
                        {detail?.world.world_name ?? worldName}
                    </h2>
                </div>
                <button
                    type="button"
                    onclick={onClose}
                    class="shrink-0 rounded-lg border border-[#DEDAC4] bg-white p-2 text-zinc-400 shadow-sm transition-colors hover:border-[#1e5854]/40 hover:text-[#1e5854]"
                    title="閉じる"
                >
                    <X class="h-4 w-4" />
                    <span class="sr-only">閉じる</span>
                </button>
            </div>

            {#if detailLoading}
                <div
                    class="mt-5 flex items-center justify-center gap-3 rounded-2xl border border-[#E3DFC9] bg-white p-14 text-sm font-bold text-[#1e5854] shadow-sm"
                >
                    <Loader2 class="h-5 w-5 animate-spin" />
                    詳細を読み込んでいます
                </div>
            {:else if detailError}
                <div
                    class="mt-5 flex items-start gap-3 rounded-2xl border border-rose-200 bg-rose-50 p-4 text-sm text-rose-700 shadow-sm"
                >
                    <AlertCircle class="mt-0.5 h-5 w-5 shrink-0" />
                    <div>
                        <p class="font-bold">詳細を読み込めませんでした</p>
                        <p class="mt-1 text-xs font-semibold">{detailError}</p>
                    </div>
                </div>
            {:else if detail}
                <div class="mt-5 space-y-5">
                    <section
                        class="overflow-hidden rounded-2xl border border-dashed border-[#D9CBA8] bg-[#FCFAF7] shadow-sm"
                    >
                        <div class="grid grid-cols-1 lg:grid-cols-[320px_1fr]">
                            <div
                                class="flex min-h-64 items-center justify-center overflow-hidden bg-[#F3F0E6] text-xs font-black tracking-widest text-[#8C7B58] uppercase"
                            >
                                {#if preview?.thumbnailUrl}
                                    <img
                                        src={preview.thumbnailUrl}
                                        alt={detail.world.world_name}
                                        class="h-full w-full object-cover"
                                    />
                                {:else}
                                    <div class="flex flex-col items-center gap-2">
                                        <Image class="h-8 w-8 text-[#B8AA86]" />
                                        <span>画像なし</span>
                                    </div>
                                {/if}
                            </div>

                            <div class="space-y-4 p-5">
                                <div>
                                    <p
                                        class="break-all font-mono text-[11px] font-bold text-zinc-400"
                                    >
                                        {formatText(detail.world.world_id, "world_id なし")}
                                    </p>
                                    <h3
                                        class="mt-2 text-xl font-black tracking-tight text-zinc-800"
                                    >
                                        {detail.world.world_name}
                                    </h3>
                                    {#if previewLoading}
                                        <p
                                            class="mt-2 text-xs font-bold text-[#1e5854]"
                                        >
                                            VRChat API からワールド情報を取得中...
                                        </p>
                                    {/if}
                                    {#if previewError}
                                        <p
                                            class="mt-2 rounded-lg border border-amber-200 bg-amber-50 px-3 py-2 text-xs font-bold text-amber-700"
                                        >
                                            {previewError}
                                        </p>
                                    {/if}
                                </div>

                                <p class="text-sm leading-7 text-zinc-700">
                                    {formatText(
                                        preview?.description,
                                        "VRChat API の説明文は未取得です。",
                                    )}
                                </p>

                                <dl class="grid grid-cols-1 gap-3 sm:grid-cols-3">
                                    <div
                                        class="rounded-xl border border-[#EFE8D7] bg-white/75 p-3"
                                    >
                                        <dt
                                            class="text-[10px] font-bold uppercase text-zinc-400"
                                        >
                                            Author
                                        </dt>
                                        <dd class="mt-1 font-bold text-zinc-700">
                                            {formatText(preview?.authorName)}
                                        </dd>
                                    </div>
                                    <div
                                        class="rounded-xl border border-[#EFE8D7] bg-white/75 p-3"
                                    >
                                        <dt
                                            class="text-[10px] font-bold uppercase text-zinc-400"
                                        >
                                            Platform
                                        </dt>
                                        <dd class="mt-1 font-bold text-zinc-700">
                                            {preview?.platform ?? "Unknown"}
                                        </dd>
                                    </div>
                                    <div
                                        class="rounded-xl border border-[#EFE8D7] bg-white/75 p-3"
                                    >
                                        <dt
                                            class="text-[10px] font-bold uppercase text-zinc-400"
                                        >
                                            Capacity
                                        </dt>
                                        <dd class="mt-1 font-bold text-zinc-700">
                                            {formatCapacity(preview?.capacity)}
                                        </dd>
                                    </div>
                                </dl>
                            </div>
                        </div>
                    </section>

                    <section
                        class="rounded-2xl border border-[#E4DEC9] bg-white/80 p-4 shadow-sm"
                    >
                        <h4
                            class="text-xs font-black tracking-widest text-zinc-500 uppercase"
                        >
                            Visit Summary
                        </h4>
                        <dl class="mt-3 grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-4">
                            <div
                                class="rounded-xl border border-[#EFE8D7] bg-[#FCFAF7] p-3"
                            >
                                <dt class="text-[10px] font-bold uppercase text-zinc-400">
                                    訪問回数
                                </dt>
                                <dd class="mt-1 text-lg font-black text-zinc-800">
                                    {detail.world.visit_count}回
                                </dd>
                            </div>
                            <div
                                class="rounded-xl border border-[#EFE8D7] bg-[#FCFAF7] p-3"
                            >
                                <dt class="text-[10px] font-bold uppercase text-zinc-400">
                                    初回訪問
                                </dt>
                                <dd class="mt-1 font-bold text-zinc-700">
                                    {formatMaybeDate(detail.world.first_visited_at)}
                                </dd>
                            </div>
                            <div
                                class="rounded-xl border border-[#EFE8D7] bg-[#FCFAF7] p-3"
                            >
                                <dt class="text-[10px] font-bold uppercase text-zinc-400">
                                    最終訪問
                                </dt>
                                <dd class="mt-1 font-bold text-zinc-700">
                                    {formatMaybeDate(detail.world.last_visited_at)}
                                </dd>
                            </div>
                            <div
                                class="rounded-xl border border-[#EFE8D7] bg-[#FCFAF7] p-3"
                            >
                                <dt
                                    class="flex items-center gap-1 text-[10px] font-bold uppercase text-zinc-400"
                                >
                                    <Clock class="h-3 w-3" />
                                    合計滞在
                                </dt>
                                <dd class="mt-1 font-bold text-zinc-700">
                                    {formatStayDuration(
                                        detail.world
                                            .total_stay_duration_seconds,
                                    )}
                                </dd>
                            </div>
                        </dl>

                        {#if detail.world.tags.length > 0}
                            <div class="mt-4 flex flex-wrap gap-2">
                                {#each detail.world.tags as tag}
                                    <span
                                        class="rounded-md border border-zinc-200 bg-[#FAF9F5] px-2 py-0.5 text-[10px] font-bold text-zinc-500"
                                        >#{tag}</span
                                    >
                                {/each}
                            </div>
                        {/if}
                    </section>

                    <section
                        class="rounded-2xl border border-[#E4DEC9] bg-white/80 p-4 shadow-sm"
                    >
                        <div class="flex items-center justify-between gap-3">
                            <h4
                                class="text-xs font-black tracking-widest text-zinc-500 uppercase"
                            >
                                Visit History
                            </h4>
                            <span class="text-[10px] font-bold text-zinc-400">
                                {detail.visits.length}件
                            </span>
                        </div>

                        <div class="mt-3 divide-y divide-[#EFE8D7]">
                            {#each detail.visits as visit}
                                <article class="py-4">
                                    <div
                                        class="flex flex-wrap items-center gap-x-4 gap-y-2"
                                    >
                                        <span
                                            class="inline-flex items-center gap-1 font-mono text-xs font-bold text-[#1e5854]"
                                        >
                                            <MapPin class="h-3.5 w-3.5" />
                                            {formatMaybeDate(visit.visited_at)}
                                        </span>
                                        <span
                                            class="rounded-full border border-zinc-200 bg-[#FAF9F5] px-2.5 py-0.5 text-xs font-bold text-zinc-500"
                                        >
                                            {visitStayLabel(visit)}
                                        </span>
                                    </div>
                                    <p
                                        class="mt-2 rounded-xl border border-dashed border-[#EFE8D7] bg-[#FCFAF7] p-3 text-sm leading-6 text-zinc-700"
                                    >
                                        {formatText(visit.memo, "メモなし")}
                                    </p>
                                </article>
                            {/each}
                        </div>
                    </section>
                </div>
            {/if}
        </div>
    </div>
{/if}
