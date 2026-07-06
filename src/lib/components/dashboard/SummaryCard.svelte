<script lang="ts">
    import { MapPin, Compass, Clock } from "phosphor-svelte";

    const stats = [
        {
            label: "今日の訪問ワールド",
            value: "5件",
            icon: MapPin,
            tone: "teal",
        },
        {
            label: "総訪問ワールド数",
            value: "142件",
            icon: Compass,
            tone: "coral",
        },
        {
            label: "総滞在時間",
            value: "28h 34m",
            icon: Clock,
            tone: "blue",
        },
    ];
</script>

<section class="summary-card">
    <div class="summary-copy">
        <p class="section-label summary-section-label">Journey Summary</p>
        <!-- <h2>最近の旅のまとめ</h2> -->

        <div class="summary-stats">
            {#each stats as stat}
                <article data-tone={stat.tone}>
                    <span class="stat-icon">
                        <svelte:component
                            this={stat.icon}
                            size={16}
                            aria-hidden="true"
                        />
                    </span>
                    <span class="stat-label">{stat.label}</span>
                    <strong>{stat.value}</strong>
                </article>
            {/each}
        </div>
    </div>
</section>

