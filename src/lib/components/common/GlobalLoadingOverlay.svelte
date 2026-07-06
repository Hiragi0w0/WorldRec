<script lang="ts">
    import { fade } from "svelte/transition";
    import { Loader2 } from "lucide-svelte";

    let {
        visible,
        label = "Loading...",
    }: {
        visible: boolean;
        label?: string;
    } = $props();
</script>

{#if visible}
    <div
        class="fixed inset-0 z-[9999] grid place-items-center bg-zinc-900/45 backdrop-blur-[2px]"
        aria-live="polite"
        aria-busy="true"
        transition:fade={{ duration: 120 }}
    >
        <div
            role="status"
            class="flex items-center gap-3 rounded-2xl border border-white/60 bg-white px-6 py-4 text-zinc-800 shadow-2xl"
        >
            <Loader2 class="h-6 w-6 motion-safe:animate-spin text-[#1e5854]" />
            <span class="text-sm font-black tracking-wide">
                {label}
            </span>
        </div>
    </div>
{/if}
