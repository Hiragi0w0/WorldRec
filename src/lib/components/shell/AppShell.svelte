<script lang="ts">
    let { children, sidebar, topbar } = $props();
</script>

<div class="min-h-screen flex text-zinc-800 bg-[#F6F4F0] selection:bg-amber-100 selection:text-amber-900 antialiased font-sans">
    {@render sidebar?.()}
    <main class="flex-1 flex flex-col min-w-0 overflow-y-auto">
        {@render topbar?.()}
        <div class="p-8 max-w-[1400px] w-full mx-auto space-y-8">
            {@render children?.()}
        </div>
    </main>
</div>
