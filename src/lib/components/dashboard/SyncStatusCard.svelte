<script lang="ts">
  import { CheckCircle2 } from "lucide-svelte";
</script>

<section class="panel-card sync-status-card">
  <div class="panel-heading">
    <div>
      <p class="section-label">同期ステータス</p>
      <h2>VRChat ログの状態</h2>
    </div>
    <span class="status-icon">
      <CheckCircle2 size={18} aria-hidden="true" />
    </span>
  </div>

  <p class="sync-status-text">
    VRChat ログの監視と同期は正常に動作しています。
  </p>

  <button class="detail-button" type="button">詳細を見る</button>
</section>

