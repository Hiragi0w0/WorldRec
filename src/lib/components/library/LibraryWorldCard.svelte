<script lang="ts">
    import { Clock, Image } from "lucide-svelte";
    import type { LibraryWorld } from "../../api/commands";
    import { formatStayDuration } from "../../data/visitMapper";
    import type { TapeStyle } from "../../data/visitTypes";
    import { tapeClass } from "../../utils/history";
    import type { WorldDetailPreview } from "../../world/worldDetailPreview";

    let {
        world,
        thumbnail,
        preview,
        tapeStyle,
        onOpenDetail,
    }: {
        world: LibraryWorld;
        thumbnail: string | null;
        preview: WorldDetailPreview | null;
        tapeStyle: TapeStyle;
        onOpenDetail: (world: LibraryWorld) => void;
    } = $props();

    let thumbnailFailed = $state(false);

    $effect(() => {
        thumbnail;
        thumbnailFailed = false;
    });

    function hideBrokenThumbnail() {
        thumbnailFailed = true;
    }

    function visibleTags(tags: string[]) {
        return tags.slice(0, 4);
    }

    function hiddenTagCount(tags: string[]) {
        return Math.max(0, tags.length - 4);
    }

    function formatVisitedAt(value: string) {
        const date = parseDateTime(value);
        if (Number.isNaN(date.getTime())) return value || "不明";

        return new Intl.DateTimeFormat("ja-JP", {
            year: "numeric",
            month: "2-digit",
            day: "2-digit",
            hour: "2-digit",
            minute: "2-digit",
            hour12: false,
        }).format(date);
    }

    function parseDateTime(value: string) {
        return new Date(value.includes("T") ? value : value.replace(" ", "T"));
    }
</script>

<button
    type="button"
    onclick={() => onOpenDetail(world)}
    class="library-world-card group h-full border-0 bg-transparent p-0 text-left text-inherit focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#1e5854]/60"
    aria-label={`${world.world_name} の詳細を開く`}
>
    <article class="library-world-card__body">
        <div
            class={`absolute -top-3.5 left-1/2 -translate-x-1/2 w-24 h-5 px-3 z-20 shadow-sm flex items-center justify-between rotate-1 border ${tapeClass(tapeStyle)}`}
            aria-hidden="true"
        >
            <div class="w-1.5 h-1.5 rounded-full bg-white/60"></div>
            <div class="w-1.5 h-1.5 rounded-full bg-white/60"></div>
        </div>

        <div class="library-world-card__photo aspect-[4/3]">
            {#if thumbnail && !thumbnailFailed}
                <img
                    src={thumbnail}
                    alt={world.world_name}
                    loading="lazy"
                    onerror={hideBrokenThumbnail}
                    class="h-full w-full object-cover transition duration-300 group-hover:scale-[1.03]"
                />
            {:else}
                <div class="library-world-card__placeholder">
                    <Image class="h-8 w-8 text-[#B8AA86]" />
                    <span>World Preview</span>
                </div>
            {/if}
        </div>

        <div class="library-world-card__caption">
            <div class="flex min-w-0 items-start justify-between gap-3">
                <div class="min-w-0">
                    <div class="library-world-card__eyebrow">
                        <span>{world.visit_count} visits</span>
                    </div>
                    <h3 class="library-world-card__title line-clamp-2">
                        {world.world_name}
                    </h3>
                </div>

                {#if world.memo_count > 0}
                    <span class="library-world-card__memo">
                        メモ{world.memo_count}件
                    </span>
                {/if}
            </div>

            {#if preview?.authorName}
                <p class="library-world-card__author">
                    Author: {preview.authorName}
                </p>
            {/if}

            <dl class="library-world-card__meta">
                <div>
                    <dt>訪問回数</dt>
                    <dd>{world.visit_count.toLocaleString()}回</dd>
                </div>
                <div>
                    <dt class="inline-flex items-center gap-1">
                        <Clock class="h-3 w-3" />
                        合計滞在
                    </dt>
                    <dd>
                        {formatStayDuration(world.total_stay_duration_seconds)}
                    </dd>
                </div>
                <div class="library-world-card__meta-wide">
                    <dt>最終訪問</dt>
                    <dd>{formatVisitedAt(world.last_visited_at)}</dd>
                </div>
            </dl>

            <div class="library-world-card__tags">
                {#each visibleTags(world.tags) as tag}
                    <span class="library-world-card__tag">#{tag}</span>
                {/each}
                {#if hiddenTagCount(world.tags) > 0}
                    <span
                        class="library-world-card__tag library-world-card__tag-more"
                    >
                        +{hiddenTagCount(world.tags)}
                    </span>
                {/if}
            </div>
        </div>
    </article>
</button>

<style>
    .library-world-card {
        display: block;
        width: 100%;
        border-radius: 8px;
    }

    .library-world-card__body {
        position: relative;
        display: grid;
        gap: 14px;
        height: 100%;
        min-width: 0;
        padding: 14px 14px 18px;
        border: 1px solid rgba(218, 207, 191, 0.98);
        border-radius: 8px;
        background: var(--color-surface, #fffdf8);
        box-shadow:
            0 16px 30px rgba(49, 39, 26, 0.13),
            0 1px 0 rgba(255, 255, 255, 0.9) inset,
            0 -1px 0 rgba(218, 208, 194, 0.34) inset;
        transition:
            transform 180ms ease,
            border-color 180ms ease,
            box-shadow 180ms ease;
    }

    .library-world-card:hover .library-world-card__body,
    .library-world-card:focus-visible .library-world-card__body {
        border-color: color-mix(
            in srgb,
            var(--color-primary, #1e5854) 42%,
            rgba(218, 207, 191, 0.98)
        );
        box-shadow:
            0 22px 38px rgba(49, 39, 26, 0.18),
            0 1px 0 rgba(255, 255, 255, 0.9) inset,
            0 -1px 0 rgba(218, 208, 194, 0.34) inset;
        transform: translateY(-3px) rotate(-0.25deg);
    }

    .library-world-card__photo {
        position: relative;
        overflow: hidden;
        min-width: 0;
        border: 1px solid rgba(214, 205, 192, 0.72);
        border-radius: 3px;
        background:
            linear-gradient(180deg, transparent 48%, rgba(34, 24, 17, 0.18)),
            linear-gradient(135deg, #93c8d1, #d4b38d);
        box-shadow:
            inset 0 0 0 1px rgba(255, 255, 255, 0.18),
            inset 0 -30px 38px rgba(24, 18, 12, 0.12),
            0 10px 18px rgba(49, 39, 26, 0.08);
    }

    .library-world-card__placeholder {
        display: flex;
        height: 100%;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        gap: 8px;
        color: #8c7b58;
        font-size: 0.72rem;
        font-weight: 900;
        letter-spacing: 0.08em;
        text-transform: uppercase;
    }

    .library-world-card__caption {
        display: grid;
        align-content: start;
        gap: 12px;
        min-width: 0;
        padding-inline: 2px;
    }

    .library-world-card__eyebrow {
        display: flex;
        flex-wrap: wrap;
        align-items: center;
        gap: 7px;
        color: var(--color-accent, #a68f63);
        font-size: 0.68rem;
        font-weight: 800;
        letter-spacing: 0.08em;
        line-height: 1.2;
        text-transform: uppercase;
    }

    .library-world-card__title {
        margin-top: 5px;
        color: var(--color-text, #3f3125);
        font-size: 1.2rem;
        font-weight: 900;
        line-height: 1.12;
        overflow-wrap: anywhere;
    }

    .library-world-card__memo {
        flex-shrink: 0;
        border: 1px solid #d9cba8;
        border-radius: 999px;
        background: #fff7ed;
        padding: 4px 8px;
        color: #7e6941;
        font-size: 0.68rem;
        font-weight: 800;
        line-height: 1;
        white-space: nowrap;
    }

    .library-world-card__author {
        margin: -4px 0 0;
        overflow: hidden;
        color: var(--color-text-muted, #8a8173);
        font-size: 0.76rem;
        font-weight: 700;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .library-world-card__meta {
        display: grid;
        grid-template-columns: repeat(2, minmax(0, 1fr));
        gap: 8px;
        margin: 0;
    }

    .library-world-card__meta div {
        min-width: 0;
        border: 1px solid #efe8d7;
        border-radius: 8px;
        background: #fcfaf7;
        padding: 9px 10px;
    }

    .library-world-card__meta-wide {
        grid-column: 1 / -1;
    }

    .library-world-card__meta dt {
        margin: 0;
        color: #9c9387;
        font-size: 0.66rem;
        font-weight: 800;
        letter-spacing: 0.04em;
        line-height: 1.2;
        text-transform: uppercase;
    }

    .library-world-card__meta dd {
        margin: 5px 0 0;
        color: #3f3f46;
        font-size: 0.82rem;
        font-weight: 800;
        line-height: 1.35;
        overflow-wrap: anywhere;
    }

    .library-world-card__tags {
        display: flex;
        flex-wrap: wrap;
        gap: 7px;
        min-width: 0;
        padding-top: 2px;
    }

    .library-world-card__tag {
        max-width: 100%;
        overflow: hidden;
        border: 1px solid #e4e4e7;
        border-radius: 6px;
        background: #faf9f5;
        padding: 3px 8px;
        color: #65605a;
        font-size: 0.68rem;
        font-weight: 800;
        line-height: 1.2;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .library-world-card__tag-more {
        background: #fff;
        color: #a1a1aa;
    }

    @media (max-width: 640px) {
        .library-world-card__body {
            padding: 13px 13px 15px;
        }

        .library-world-card__title {
            font-size: 1.08rem;
            line-height: 1.18;
        }
    }
</style>
