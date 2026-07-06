<script lang="ts">
    import type { VisitRecord } from "../../data/worldrec";

    export let visit: VisitRecord;
    export let viewMode: "list" | "grid" = "list";
    export let featured = false;

    $: visitDescription =
        `${visit.thumbnailLabel || visit.worldName}の余韻を、` +
        `あとから読み返せる短いメモとして残しておく。`;
</script>

<article
    class:featured
    class:view-grid={viewMode === "grid"}
    class="visit-card"
>
    <div class="visit-card__timeline">
        <time class="visit-card__time">{visit.visitedAt}</time>
        <span class="visit-card__dot" aria-hidden="true"></span>
    </div>

    <div class="visit-card__spread">
        <section
            class="visit-card__page visit-card__page--photo"
            aria-label="写真ページ"
        >
            <div class="visit-card__photo-stack">
                <span class="visit-card__photo-tape" aria-hidden="true"></span>
                <div class="visit-thumb" data-tone={visit.thumbnailTone}>
                    <span class="visit-thumb__overlay"></span>
                </div>
                <div class="visit-card__photo-caption">
                    <h3 class="visit-card__photo-caption-title">
                        {visit.worldName}
                    </h3>

                    <div class="visit-card__world-info">
                        <p class="visit-card__photo-caption-note">
                            {visitDescription}
                        </p>

                        <div class="visit-card__caption-tags">
                            {#each visit.tags as tag}
                                <span class="tag-pill">{tag}</span>
                            {/each}
                        </div>
                    </div>

                    <dl class="visit-card__visit-meta">
                        <div>
                            <dt>Instance: {visit.accessType}</dt>
                        </div>
                        <div>
                            <dt>Stay: {visit.stayDuration}</dt>
                        </div>
                    </dl>
                </div>
            </div>
        </section>
    </div>
</article>
