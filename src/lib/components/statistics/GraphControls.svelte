<script lang="ts">
    export let onReset: () => void;
    export let onFitAll: () => void;
</script>

<div class="flex items-center gap-2">
    <button
        onclick={onReset}
        class="text-xs px-3 py-1.5 bg-white border border-zinc-200 rounded-lg hover:bg-zinc-50 text-zinc-700 font-medium transition-colors"
    >
        リセット
    </button>
    <button
        onclick={onFitAll}
        class="text-xs px-3 py-1.5 bg-white border border-zinc-200 rounded-lg hover:bg-zinc-50 text-zinc-700 font-medium transition-colors"
    >
        全体表示
    </button>
    <span class="text-[10px] text-zinc-400 ml-2">ドラッグ: 移動　ホイール: 拡大縮小　クリック: 選択</span>
</div>
