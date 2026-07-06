<script lang="ts">
    import {
        AlertCircle,
        ChevronLeft,
        ChevronRight,
        FileText,
        Image,
        Loader2,
        Search,
        Tags,
    } from "lucide-svelte";
    import { onMount } from "svelte";
    import {
        type LibrarySortDirection,
        type LibrarySortKey,
        type LibraryWorld,
    } from "../../api/commands";
    import type { TapeStyle } from "../../data/visitTypes";
    import type { LibraryState } from "../../state/library.svelte";
    import {
        getCachedWorldPreview,
        loadWorldPreviews,
    } from "../../state/worldPreviews.svelte";
    import LibraryWorldCard from "../library/LibraryWorldCard.svelte";

    let {
        library,
        tapeStyle,
        onOpenDetail,
    }: {
        library: LibraryState;
        tapeStyle: TapeStyle;
        onOpenDetail: (world: LibraryWorld) => void;
    } = $props();

    let formQuery = $state("");
    let formVisitedFrom = $state("");
    let formVisitedTo = $state("");
    let formTagQuery = $state("");
    let formMemoQuery = $state("");

    let currentPage = $derived(Math.floor(library.offset / library.limit) + 1);
    let totalPages = $derived(
        Math.max(1, Math.ceil(library.totalCount / library.limit)),
    );
    let hasPreviousPage = $derived(library.offset > 0);
    let hasNextPage = $derived(
        library.offset + library.limit < library.totalCount,
    );
    let hasSearchCriteria = $derived(
        library.query.trim().length > 0 ||
            library.visitedFrom.trim().length > 0 ||
            library.visitedTo.trim().length > 0 ||
            library.tagQuery.trim().length > 0 ||
            library.memoQuery.trim().length > 0,
    );

    $effect(() => {
        void loadWorldPreviews(
            library.items,
            (world) => world.world_id,
            (world) => ({
                worldId: world.world_id,
                worldName: world.world_name,
            }),
            3,
            (world) => ({
                imageUrl: world.thumbnail_url ?? world.image_url,
                fetchedAt: world.world_preview_fetched_at,
            }),
        );
    });

    onMount(() => {
        formQuery = library.query;
        formVisitedFrom = library.visitedFrom;
        formVisitedTo = library.visitedTo;
        formTagQuery = library.tagQuery;
        formMemoQuery = library.memoQuery;
        void library.loadLibrary();
    });

    function handleSearchSubmit(event: SubmitEvent) {
        event.preventDefault();
        void library.applySearchCriteria({
            query: formQuery,
            visitedFrom: formVisitedFrom,
            visitedTo: formVisitedTo,
            tagQuery: formTagQuery,
            memoQuery: formMemoQuery,
        });
    }

    function handleSortChange(value: string) {
        const [sortKey, sortDirection] = value.split(":") as [
            LibrarySortKey,
            LibrarySortDirection,
        ];
        void library.applySort(sortKey, sortDirection);
    }

    function handleLimitChange(value: string) {
        void library.applyLimit(value === "10" ? 10 : 25);
    }

    function handlePageChange(page: number) {
        void library.goToPageAndLoad(page);
    }

    function clearSearchCriteria() {
        formQuery = "";
        formVisitedFrom = "";
        formVisitedTo = "";
        formTagQuery = "";
        formMemoQuery = "";
        void library.clearSearchCriteriaAndLoad();
    }

    function previewFor(world: LibraryWorld) {
        return getCachedWorldPreview(world.world_id);
    }

    function thumbnailFor(world: LibraryWorld) {
        const preview = previewFor(world);
        return (
            world.thumbnail_url ??
            world.image_url ??
            preview?.thumbnailUrl ??
            preview?.imageUrl ??
            null
        );
    }
</script>

<section class="space-y-6 animate-fadeIn">
    <div
        class="flex flex-col gap-4 rounded-xl border border-[#E7E4D9] bg-white/50 p-4 md:flex-row md:items-end md:justify-between"
    >
        <div>
            <span
                class="text-[10px] font-bold tracking-widest text-[#0f4743] uppercase"
                >World Library</span
            >
            <h2 class="mt-0.5 text-2xl font-black tracking-tight text-zinc-800">
                ライブラリ
            </h2>
            <p class="mt-1 text-sm text-zinc-500">
                SQLite に保存された訪問済みワールドを検索・並び替えできます。
            </p>
        </div>
        <div class="text-sm font-bold text-zinc-500">
            {library.totalCount.toLocaleString()} 件
        </div>
    </div>

    <div class="rounded-2xl border border-[#E3DFC9] bg-white p-4 shadow-sm">
        <form onsubmit={handleSearchSubmit}>
            <div class="grid grid-cols-1 gap-3 lg:grid-cols-4">
                <label class="space-y-1.5">
                    <span class="text-[10px] font-bold uppercase text-zinc-400"
                        >ワールド名</span
                    >
                    <div class="relative">
                        <Search
                            class="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-zinc-400"
                        />
                        <input
                            type="text"
                            bind:value={formQuery}
                            placeholder="ワールド名で検索"
                            class="w-full rounded-xl border border-[#DDD8C6] bg-[#FAF9F5] py-2 pl-9 pr-3 text-sm font-semibold text-zinc-700 outline-none transition focus:border-[#1e5854] focus:bg-white"
                        />
                    </div>
                </label>

                <label class="space-y-1.5">
                    <span class="text-[10px] font-bold uppercase text-zinc-400"
                        >訪問日 From</span
                    >
                    <input
                        type="date"
                        bind:value={formVisitedFrom}
                        class="w-full rounded-xl border border-[#DDD8C6] bg-[#FAF9F5] px-3 py-2 text-sm font-semibold text-zinc-700 outline-none transition focus:border-[#1e5854] focus:bg-white"
                    />
                </label>

                <label class="space-y-1.5">
                    <span class="text-[10px] font-bold uppercase text-zinc-400"
                        >訪問日 To</span
                    >
                    <input
                        type="date"
                        bind:value={formVisitedTo}
                        class="w-full rounded-xl border border-[#DDD8C6] bg-[#FAF9F5] px-3 py-2 text-sm font-semibold text-zinc-700 outline-none transition focus:border-[#1e5854] focus:bg-white"
                    />
                </label>

                <label class="space-y-1.5">
                    <span class="text-[10px] font-bold uppercase text-zinc-400"
                        >タグ</span
                    >
                    <div class="relative">
                        <Tags
                            class="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-zinc-400"
                        />
                        <input
                            type="text"
                            bind:value={formTagQuery}
                            placeholder="タグで検索"
                            class="w-full rounded-xl border border-[#DDD8C6] bg-[#FAF9F5] py-2 pl-9 pr-3 text-sm font-semibold text-zinc-700 outline-none transition focus:border-[#1e5854] focus:bg-white"
                        />
                    </div>
                </label>
            </div>

            <div class="mt-3 grid grid-cols-1 gap-3 lg:grid-cols-[1fr_140px]">
                <label class="space-y-1.5">
                    <span class="text-[10px] font-bold uppercase text-zinc-400"
                        >メモ</span
                    >
                    <div class="relative">
                        <FileText
                            class="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-zinc-400"
                        />
                        <input
                            type="text"
                            bind:value={formMemoQuery}
                            placeholder="メモ本文で検索"
                            class="w-full rounded-xl border border-[#DDD8C6] bg-[#FAF9F5] py-2 pl-9 pr-3 text-sm font-semibold text-zinc-700 outline-none transition focus:border-[#1e5854] focus:bg-white"
                        />
                    </div>
                </label>

                <button
                    type="submit"
                    disabled={library.isLoading}
                    class="mt-auto inline-flex items-center justify-center rounded-xl bg-[#1e5854] px-4 py-2 text-sm font-bold text-white shadow-sm transition hover:bg-[#133c39] disabled:cursor-not-allowed disabled:opacity-40"
                >
                    検索
                </button>
            </div>
        </form>

        <div class="mt-3 grid grid-cols-1 gap-3 sm:grid-cols-[220px_140px]">
            <label class="space-y-1.5">
                <span class="text-[10px] font-bold uppercase text-zinc-400"
                    >ソート</span
                >
                <select
                    value={`${library.sortKey}:${library.sortDirection}`}
                    onchange={(event) =>
                        handleSortChange(event.currentTarget.value)}
                    class="w-full rounded-xl border border-[#DDD8C6] bg-[#FAF9F5] px-3 py-2 text-sm font-bold text-zinc-700 outline-none transition focus:border-[#1e5854] focus:bg-white"
                >
                    <option value="world_name:asc">ワールド名 昇順</option>
                    <option value="world_name:desc">ワールド名 降順</option>
                    <option value="visit_count:desc">訪問回数 多い順</option>
                    <option value="visit_count:asc">訪問回数 少ない順</option>
                    <option value="total_stay_duration_seconds:desc"
                        >合計滞在時間 長い順</option
                    >
                    <option value="total_stay_duration_seconds:asc"
                        >合計滞在時間 短い順</option
                    >
                </select>
            </label>

            <label class="space-y-1.5">
                <span class="text-[10px] font-bold uppercase text-zinc-400"
                    >表示件数</span
                >
                <select
                    value={String(library.limit)}
                    onchange={(event) =>
                        handleLimitChange(event.currentTarget.value)}
                    class="w-full rounded-xl border border-[#DDD8C6] bg-[#FAF9F5] px-3 py-2 text-sm font-bold text-zinc-700 outline-none transition focus:border-[#1e5854] focus:bg-white"
                >
                    <option value="10">10件</option>
                    <option value="25">25件</option>
                </select>
            </label>
        </div>
    </div>

    {#if library.error}
        <div
            class="flex items-start gap-3 rounded-2xl border border-rose-200 bg-rose-50 p-4 text-sm text-rose-700 shadow-sm"
        >
            <AlertCircle class="mt-0.5 h-5 w-5 shrink-0" />
            <div>
                <p class="font-bold">ライブラリを読み込めませんでした</p>
                <p class="mt-1 text-xs font-semibold">{library.error}</p>
            </div>
        </div>
    {/if}

    {#if library.isLoading && library.items.length === 0}
        <div
            class="flex items-center justify-center gap-3 rounded-2xl border border-[#E3DFC9] bg-white p-16 text-sm font-bold text-[#1e5854] shadow-sm"
        >
            <Loader2 class="h-5 w-5 animate-spin" />
            ライブラリを読み込んでいます
        </div>
    {:else if library.totalCount === 0}
        <div
            class="rounded-2xl border border-[#E3DFC9] bg-white p-16 text-center shadow-sm"
        >
            <div
                class="mx-auto mb-4 flex h-16 w-16 items-center justify-center rounded-full border border-dashed border-[#DFCFA9] bg-amber-50 text-[#7E6941]"
            >
                <Image class="h-8 w-8" />
            </div>
            {#if hasSearchCriteria}
                <h3 class="text-lg font-bold text-zinc-800">
                    条件に一致するワールドがありません
                </h3>
                <p class="mx-auto mt-2 max-w-sm text-sm text-zinc-500">
                    検索条件を減らすか、すべての条件をクリアしてください。
                </p>
                <button
                    type="button"
                    onclick={clearSearchCriteria}
                    class="mt-6 rounded-xl border border-[#C5BFAB] bg-white px-5 py-2 text-xs font-bold text-zinc-700 shadow-sm transition hover:bg-[#FAF9F5]"
                >
                    検索条件をクリア
                </button>
            {:else}
                <h3 class="text-lg font-bold text-zinc-800">
                    まだライブラリがありません
                </h3>
                <p class="mx-auto mt-2 max-w-sm text-sm text-zinc-500">
                    VRChat
                    ログから訪問履歴が保存されると、ここにワールド単位で表示されます。
                </p>
            {/if}
        </div>
    {:else}
        <div class="grid grid-cols-1 gap-6 sm:grid-cols-2 xl:grid-cols-3">
            {#each library.items as world (world.key)}
                <LibraryWorldCard
                    {world}
                    thumbnail={thumbnailFor(world)}
                    preview={previewFor(world)}
                    {tapeStyle}
                    {onOpenDetail}
                />
            {/each}
        </div>

        <div
            class="flex flex-col gap-3 rounded-2xl border border-[#E3DFC9] bg-white p-4 shadow-sm sm:flex-row sm:items-center sm:justify-between"
        >
            <p class="text-sm font-bold text-zinc-500">
                {currentPage} / {totalPages} ページ
            </p>
            <div class="flex items-center gap-2">
                <button
                    type="button"
                    disabled={!hasPreviousPage || library.isLoading}
                    onclick={() => handlePageChange(currentPage - 1)}
                    class="inline-flex items-center gap-1 rounded-xl border border-[#C5BFAB] bg-white px-3 py-2 text-xs font-bold text-zinc-700 shadow-sm transition hover:bg-[#FAF9F5] disabled:cursor-not-allowed disabled:opacity-40"
                >
                    <ChevronLeft class="h-4 w-4" />
                    前へ
                </button>
                <button
                    type="button"
                    disabled={!hasNextPage || library.isLoading}
                    onclick={() => handlePageChange(currentPage + 1)}
                    class="inline-flex items-center gap-1 rounded-xl bg-[#1e5854] px-3 py-2 text-xs font-bold text-white shadow-sm transition hover:bg-[#133c39] disabled:cursor-not-allowed disabled:opacity-40"
                >
                    次へ
                    <ChevronRight class="h-4 w-4" />
                </button>
            </div>
        </div>
    {/if}
</section>
