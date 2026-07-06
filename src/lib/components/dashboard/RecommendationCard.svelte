<script lang="ts">
  import { Bookmark, Star } from "lucide-svelte";
  import type { RecommendationRecord } from "../../data/worldrec";

  export let recommendations: RecommendationRecord[] = [];
</script>

<section class="panel-card recommendation-card">
  <div class="panel-heading">
    <div>
      <p class="section-label">おすすめワールド</p>
      <h2>旅の気分に合う候補</h2>
    </div>
    <button class="text-button" type="button">もっと見る</button>
  </div>

  <div class="recommendation-list">
    {#each recommendations as recommendation}
      <article class="recommendation-item">
        <div class="recommendation-thumb" data-tone={recommendation.thumbnailTone}>
          <span class="recommendation-thumb__badge">
            <Star size={14} />
          </span>
        </div>

        <div class="recommendation-copy">
          <div class="recommendation-copy__head">
            <div>
              <h3>{recommendation.name}</h3>
              <p>{recommendation.description}</p>
            </div>
            <button class="ghost-icon" type="button" aria-label="保存">
              <Bookmark size={16} />
            </button>
          </div>

          <div class="tag-list">
            {#each recommendation.tags as tag}
              <span class="tag-pill">{tag}</span>
            {/each}
          </div>
        </div>
      </article>
    {/each}
  </div>
</section>

