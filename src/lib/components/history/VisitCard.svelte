<script lang="ts">
    import { Clock, Image, MapPin, Trash2 } from "lucide-svelte";
    import type { TapeStyle, VisitRecord } from "../../data/visitTypes";
    import { tapeClass } from "../../utils/history";

    export let record: VisitRecord;
    export let tapeStyle: TapeStyle;
    export let viewFormat: "list" | "grid";
    export let thumbnail: string | null = null;
    export let eagerLoad = false;
    export let onOpenWorldDetail: (record: VisitRecord) => void;
    export let onDeleteRecord: (record: VisitRecord) => void;

    $: finalTape = tapeStyle;
    $: stayLabel = record.stayLabel ?? `${record.stayMinutes}分`;
    let thumbnailFailed = false;
    $: (thumbnail, (thumbnailFailed = false));
    $: showThumbnail = Boolean(thumbnail) && !thumbnailFailed;

    function hideBrokenThumbnail() {
        thumbnailFailed = true;
    }
</script>

<article
    class={`relative ${viewFormat === "list" ? "flex flex-col md:flex-row items-start gap-6 md:gap-10 group" : "flex flex-col group justify-between h-full bg-white p-5 rounded-xl border border-zinc-200 shadow-sm"}`}
>
    {#if viewFormat === "list"}
        <div
            class="flex md:flex-col items-center gap-2 md:gap-1 md:w-16 shrink-0 z-10 md:mt-10"
        >
            <span
                class="text-xs font-bold text-[#A68F63] tracking-widest font-mono"
                >{record.time}</span
            >
            <div
                class="w-3.5 h-3.5 rounded-full bg-[#FAF9F5] border-[3px] border-[#1e5854] shadow-sm"
            ></div>
        </div>
    {/if}

    <div
        class={`relative ${viewFormat === "list" ? "w-full md:w-[320px] shrink-0" : "w-full"}`}
    >
        <div
            class={`absolute -top-3.5 left-1/2 -translate-x-1/2 w-24 h-5 px-3 z-20 shadow-sm flex items-center justify-between rotate-1 border ${tapeClass(finalTape)}`}
        >
            <div class="w-1.5 h-1.5 rounded-full bg-white/60"></div>
            <div class="w-1.5 h-1.5 rounded-full bg-white/60"></div>
        </div>
        <div
            class="bg-white p-3.5 pb-6 border border-[#ECE9DD] rounded-sm shadow-md group-hover:shadow-xl group-hover:-rotate-1 transition-all duration-300 relative overflow-hidden"
        >
            {#if record.stamp}
                <div
                    class="absolute right-4 top-4 z-10 pointer-events-none select-none rotate-12 opacity-85"
                >
                    <div
                        class="border-[2.5px] border-amber-600/70 text-amber-700/70 font-travel text-[10px] font-black px-2 py-1.5 rounded-full uppercase tracking-wider bg-white/95 flex flex-col items-center"
                    >
                        <span
                            class="text-[7px] tracking-widest font-sans font-bold"
                            >VRC PORT</span
                        >
                        {record.stamp}
                    </div>
                </div>
            {/if}
            <button
                type="button"
                class="w-full aspect-[4/3] rounded-sm flex flex-col justify-end p-3 relative overflow-hidden shadow-inner border border-zinc-100 text-left transition-shadow hover:shadow-lg focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#1e5854]/60"
                style="background: linear-gradient(135deg, #f4f1e8 0%, #d8d1c1 100%)"
                onclick={() => onOpenWorldDetail(record)}
                aria-label={`${record.worldName} の詳細を開く`}
            >
                {#if showThumbnail}
                    <img
                        src={thumbnail}
                        alt={record.worldName}
                        loading={eagerLoad ? "eager" : "lazy"}
                        class="absolute inset-0 h-full w-full object-cover"
                        onerror={hideBrokenThumbnail}
                    />
                {:else}
                    <span
                        class="absolute inset-0 flex flex-col items-center justify-center gap-2 text-[10px] font-bold tracking-widest text-[#8C7B58] uppercase"
                    >
                        <Image class="w-6 h-6 text-[#B8AA86]" />
                        画像なし
                    </span>
                {/if}
                <div
                    class="absolute inset-0 bg-[radial-gradient(circle_at_30%_20%,rgba(255,255,255,0.18),transparent_35%),linear-gradient(to_bottom,transparent,rgba(0,0,0,0.24))] pointer-events-none"
                ></div>
                <span
                    class="text-[9px] bg-black/40 text-neutral-100 backdrop-blur-xs font-bold px-2 py-0.5 rounded-full w-max z-10"
                    >WORLD</span
                >
            </button>
            <div
                class="mt-4 pt-1 text-center font-handwritten text-xl font-bold text-[#4B3E2F] select-none tracking-wider flex items-center justify-center gap-1"
            >
                <button
                    type="button"
                    class="text-left transition hover:text-[#1e5854] hover:underline focus:outline-none focus-visible:ring-2 focus-visible:ring-[#1e5854]/60 rounded"
                    aria-label={`${record.worldName} の詳細を開く`}
                    onclick={() => onOpenWorldDetail(record)}
                >
                    {record.worldName}
                </button>
            </div>
        </div>
    </div>

    <div
        class="flex-1 bg-white/95 border border-[#EDEADF] p-6 rounded-2xl shadow-sm relative flex flex-col justify-between group-hover:bg-white transition-colors"
    >
        <div class="absolute top-2.5 right-4 flex gap-1 items-center">
            <span class="w-2 h-2 rounded-full bg-zinc-300"></span>
            <span class="w-2 h-2 rounded-full bg-zinc-300"></span>
        </div>
        <div class="space-y-4">
            <div class="flex flex-wrap items-center gap-3">
                <h4 class="text-lg font-extrabold text-zinc-800 tracking-tight">
                    <button
                        type="button"
                        class="rounded-sm text-left text-lg font-extrabold tracking-tight hover:text-[#1e5854] hover:underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#1e5854]/60"
                        onclick={() => onOpenWorldDetail(record)}
                        aria-label={`${record.worldName} の詳細を開く`}
                    >
                        {record.worldName}
                    </button>
                </h4>
                <span
                    class="text-xs px-2.5 py-0.5 rounded-full font-bold border border-zinc-200 bg-neutral-50 text-zinc-500 uppercase tracking-widest"
                    >{record.instanceType}</span
                >
                <span
                    class="text-xs text-zinc-400 font-semibold font-mono break-all"
                    title={record.worldId}
                >
                    ID: {record.worldId}
                </span>
            </div>
            <p
                class="text-sm md:text-base text-zinc-700 leading-relaxed font-handwritten font-medium tracking-wide bg-gradient-to-b from-transparent to-neutral-50/10 p-2.5 rounded-lg border border-dashed border-[#F0ECE1]"
            >
                {record.memo}
            </p>
        </div>
        <div
            class="mt-6 pt-3 border-t border-zinc-100 flex items-center justify-between gap-4"
        >
            <div
                class="flex flex-wrap items-center gap-5 text-xs text-zinc-400 font-bold tracking-wider uppercase"
            >
                <span class="flex items-center gap-1.5"
                    ><MapPin class="w-3.5 h-3.5" />INSTANCE: {record.instanceType}</span
                >
                <span class="flex items-center gap-1.5"
                    ><Clock class="w-3.5 h-3.5" />STAY: {stayLabel}</span
                >
            </div>
            <div class="flex items-center gap-2">
                <button
                    onclick={() => onDeleteRecord(record)}
                    class="p-2 bg-white hover:bg-rose-50 text-zinc-400 hover:text-rose-600 border border-zinc-200 hover:border-rose-200 rounded-lg shadow-sm transition-colors"
                    title="この記録を消去"
                    ><Trash2 class="w-3.5 h-3.5" /></button
                >
            </div>
        </div>
    </div>
</article>
