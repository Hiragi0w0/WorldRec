<script lang="ts">
  import { ArrowRight, Send, Sparkles } from "lucide-svelte";

  const chips = ["落ち着ける場所", "綺麗な景色", "夜景", "カフェ"];

  let prompt = "";

  function applyChip(chip: string) {
    prompt = chip;
  }
</script>

<section class="panel-card explorer-panel">
  <div class="panel-heading">
    <div>
      <p class="section-label">AI 探索</p>
      <h2>次に行くワールドを探してみよう</h2>
    </div>
    <span class="beta-badge">ベータ</span>
  </div>

  <div class="explorer-hero">
    <Sparkles size={16} />
    <span>旅の気分を一言で入れるだけで、次の候補を探す補助になります。</span>
  </div>

  <label class="explorer-input" for="explorer-prompt">
    <span class="sr-only">探索キーワード</span>
    <input
      id="explorer-prompt"
      bind:value={prompt}
      type="text"
      placeholder="どんな気分で旅したいですか？"
    />
    <button class="send-button" type="button" aria-label="送信">
      <Send size={16} />
    </button>
  </label>

  <div class="chip-row" aria-label="例の入力">
    {#each chips as chip}
      <button class="suggestion-chip" type="button" on:click={() => applyChip(chip)}>
        {chip}
      </button>
    {/each}
  </div>

  <button class="explorer-link" type="button">
    <span>探索のヒントを見る</span>
    <ArrowRight size={16} />
  </button>
</section>

