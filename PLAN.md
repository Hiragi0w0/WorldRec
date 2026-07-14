# WorldRec 設定パス安全適用計画

## 目的

DB 保存先または VRChat ログフォルダの実効パスが変わるとき、Rust 側の単一処理で事前検証、watcher の同期停止、設定保存、必要な再起動、失敗時の旧状態復旧まで行う。Frontend は結果 DTO が示す現在有効な設定と runtime 状態を表示し、旧 DB 由来の非同期結果を破棄する。

## 制約

- DB schema、履歴のコピー・移動・マージ、DB/ログ削除、重複防止キーを変更しない。
- 設定変更を理由に current visit を強制確定しない。
- watcher が停止中なら設定変更後も自動起動しない。
- Frontend から stop/save/start を個別制御しない。
- 既存 UI 構造を維持し、新規依存を追加しない。
- `CLAUDE.md` と `codex_ベストプラクティス.md` はこの checkout 内に存在しないため、添付要件、ルート AGENTS.md、既存コードを判断根拠とする。

## 対象ファイル

- `src-tauri/src/settings/mod.rs`: 設定の正規化と保存処理の再利用可能化。
- `src-tauri/src/log_watcher/service.rs`: 実効パス正規化、ログ/DB の事前検証、失敗可能な worker spawn。
- `src-tauri/src/log_watcher/state.rs`: 実使用 DB パス保持、停止 join エラー通知、安全な再起動用状態操作。
- `src-tauri/src/commands/mod.rs`: 設定適用 DTO と原子的な状態遷移、runtime status の実使用パス反映。
- `src-tauri/src/lib.rs`: 必要な command/service 登録。
- `src/lib/api/commands.ts`: Rust DTO と watcher status の型同期。
- `src/lib/state/histories.svelte.ts`: load generation による古い履歴/runtime 応答の破棄。
- `src/App.svelte`: 設定適用中イベント抑止、完了後の履歴/runtime/library 一括再読込、DB 詳細 drawer のクローズ。
- `src/lib/components/views/SettingsView.svelte`: パス変更確認、保存中の入力無効化、outcome 別表示、effective settings への draft 復元。

## 非対象

- DB schema と repository のデータモデル。
- ログ解析・重複防止・滞在時間算出の仕様。
- 設定画面全体の再設計。
- 新旧 DB 間のデータ操作。
- README や運用仕様の変更（今回の挙動要件は添付仕様で確定済み）。

## 実装手順

1. 解決済みパスを canonicalize 可能なら canonicalize し、Windows では区切り・大文字小文字を吸収して比較する。安全に絶対化できない相対パスは拒否する。
2. ログディレクトリの探索可能性と、DB の初期化・quick_check・ロールバックされる書込確認を watcher 停止前に検証する。
3. `LogWatcherStatus` に実使用 `db_path` を追加し、worker spawn を `thread::Builder::spawn` の `Result` に変更する。
4. stop 時に停止要求、EOF drain、join、SQLite connection 解放の順序を保証し、join 失敗を `Err` で返す。current visit は確定せず破棄する既存仕様を保つ。
5. 設定適用 command で旧 settings と実 watcher 状態を保存し、パス変更時だけ stop/save/restart を行う。各失敗で旧 settings と旧実使用パスへ復旧し、結果 DTO を返す。
6. runtime status は実行中なら watcher state の実使用パス、停止中なら現在設定の実効パスを返す。
7. Frontend 型を DTO に合わせ、App で設定適用中イベントを保留し、完了後に generation を更新して履歴/runtime/library を一括再読込する。
8. SettingsView はパス変更時だけ確認し、結果 DTO の settings に draft を戻し、outcome 別の利用者向け文言と内部詳細を分ける。
9. Rust のパス比較、検証、state、適用/rollback のテストを追加し、既存 watcher/重複防止/滞在時間テストも実行する。
10. 差分をスコープ・状態遷移・エラー経路・Frontend 競合の観点で自己レビューする。

## 検証手順

1. `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
2. `cargo test --manifest-path src-tauri/Cargo.toml`
3. `cargo check --manifest-path src-tauri/Cargo.toml`
4. `npm run check`
5. `npm run build`
6. `git diff --check` と対象差分の自己レビュー。

## 前提・未解決事項

- Windows の ACL に依存する書込不能テストは避け、ディレクトリ指定や test seam で決定的に再現する。
- 設定ファイル保存失敗を含む rollback テストは、テスト時だけ小さな関数差し替えを用いる。
- watcher の thread panic は stop 失敗として扱い、設定適用では旧設定復旧を試みる。

## PR #9 レビュー・CI 対応追補

### 目的

PR #9 の未解決 P2 レビュー5件と `Rust & Tauri` CI失敗を、設定切替の安全性を維持したまま解消する。あわせて差分レビューで確認した、通常設定の保存中に訪問・watcherイベントを取りこぼす競合を修正する。

### 制約

- DB schema、履歴データ、CI workflow、設定画面のデザインは変更しない。
- DB書込確認は main database を実際に更新できることを transaction 内で確認し、必ず rollback して検証用データを残さない。
- 設定ファイルの復旧失敗と watcher の復旧失敗を独立した best-effort 操作として扱い、一方の失敗で他方を省略しない。
- Windows の長いパスと8.3短縮パスは文字列一致ではなく実効パスとして比較する。
- GitHub のレビュー返信・thread resolve・commit・push は、この計画の実装承認後に別途行う。

### 対象ファイル

- `src-tauri/src/settings_apply.rs`: 不正な旧保存パスからの修復、rollbackのbest-effort化、実効パス比較テスト。
- `src-tauri/src/log_watcher/service.rs`: main DB書込検証、platform separator対応、DB検証テスト。
- `src/lib/state/histories.svelte.ts`: runtime loading の世代交代時も確実に解除する管理。
- `src/App.svelte`: パス変更を伴わない設定保存中のruntime event取りこぼし防止。

### 非対象

- `.github/workflows/ci.yml` のrunner、OS、Action versionの変更。
- watcher、ログ解析、履歴確定、DB移行の仕様変更。
- レビュー指摘とCI失敗に関係しないリファクタリング。

### 実装手順

1. `settings_apply.rs` のテストに実効パス比較 helper を追加し、CIで失敗した5件の長いパス対8.3短縮パス比較を置き換える。文字列DTOの内容ではなく、同じ保存先を指すことを検証する。
2. 旧 settings の実効パス解決失敗を、requested settings の評価前の致命エラーにしない。watcher稼働中は実使用パスをbaselineにし、停止中で旧パスが不正な場合も有効なabsolute pathへの置換を保存できる分岐と回帰テストを追加する。
3. rollback処理を「旧設定の保存」と「旧watcherの再起動」に分けて両方を必ず試す。各エラーを `rollback_error` に集約し、設定保存が失敗しても以前の実使用パスでwatcherを再開できることをテストする。
4. `validate_db_path` の `TEMP` table確認を、main schema上の検証用tableをtransaction内で作成・書込・rollbackする確認へ変更する。main DBがquery-only/read-onlyのとき拒否し、成功時に検証用tableが残らないテストを追加する。
5. trailing separatorテストを `std::path::MAIN_SEPARATOR` で組み立て、Windows以外でも同じ意味のパスを比較する。
6. `refreshRuntimeStatus` のloading状態を単一requestの最新判定だけに依存させず、loading要求のtokenまたは件数で管理する。loading要求が非loading要求に追い越された場合と、loading要求同士が重なった場合を破綻させない。
7. `App.svelte` のイベント抑止とload無効化を実際のパス切替時に限定するか、抑止中イベントを保留して保存完了後に再読込する。少なくとも通常設定保存と同時の `visit-saved` を失わないようにする。
8. 対象差分を、旧設定修復、rollback順序、DB非破壊性、非同期loading、イベント競合の観点で再レビューする。

### 検証手順

1. `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
2. `cargo test --manifest-path src-tauri/Cargo.toml settings_apply::tests -- --nocapture`
3. `cargo test --manifest-path src-tauri/Cargo.toml log_watcher::service::tests -- --nocapture`
4. `cargo test --manifest-path src-tauri/Cargo.toml`
5. `npm run check`
6. `npm run build`
7. `git diff --check`
8. push後に `gh pr checks 9` で `Rust & Tauri`、`Frontend check & build`、`Go sidecar`、CodeQLを再確認する。

### 未解決事項・前提

- 現在のCI直接原因はWindowsの長いパスと8.3短縮パスの表現差であり、workflow変更は不要と判断する。
- frontendには独立したunit test runnerがないため、非同期競合は小さな純粋helperへ切り出せる場合のみRust同等の自動回帰テストを追加し、それ以外は `npm run check` / `npm run build` とコードレビューで確認する。
- 付属のCI検査scriptはWindows CP932で `UnicodeDecodeError` になったため、今回は `gh pr checks` と `gh run view --log` の直接結果を根拠にした。script自体の修正はこのリポジトリの対象外とする。

## PR #9 再レビュー対応計画（reviewed commit `5222b853d3`）

### 目的

最新レビューで未解決のP2 3件を、設定切替の安全性と既存MSRVを維持しながら解消する。通常設定保存中のvisit event取りこぼし、Rust 1.77.2でのcompile失敗、AI自動保存による未確認のDB/log path適用を対象とする。

### 制約

- `src-tauri/Cargo.toml` の `rust-version = "1.77.2"` は変更しない。
- パス変更中に旧DB由来の非同期結果を破棄し、完了後に履歴・runtime・libraryを再読込する既存方針は維持する。
- パス変更を伴わない設定保存では、visit/watcher eventを抑止せず、進行中のloadも無効化しない。
- AI有効化の自動保存は既存の `confirmPathChange` を再利用し、新しいbackend commandや設定保存仕様は追加しない。
- GitHubへの返信、thread resolve、commit、pushは実装承認後に別途行う。

### 対象ファイル

- `src-tauri/src/settings_apply.rs`: MSRV非互換の `Option::is_none_or` をRust 1.77.2互換の式へ置換。
- `src/App.svelte`: 全設定保存を示すloading状態と、path切替中だけ必要なevent抑止・load無効化を分離。
- `src/lib/components/views/SettingsView.svelte`: AI toggleの自動保存前にも未保存path変更の確認を行う。

### 非対象

- `src-tauri/Cargo.toml` のMSRV引き上げとdependency更新。
- 設定DTO、Rust command、DB schema、watcher lifecycleの変更。
- 既に解決済みの以前のP2 5件と、今回の3件に関係しないUI変更。
- CI workflowやGitHub review状態の更新。

### 実装手順

1. `baseline_paths.is_none_or(...)` を `map_or(true, ...)` または等価な `match` に置換し、現在のpaths_changed判定を変えずにRust 1.77.2互換へ戻す。MSRV引き上げは行わない。
2. `App.svelte` にpath切替専用の状態を設け、runtime event listenerはその状態だけを見て中間eventを抑止する。`isApplyingSettings` はglobal loading表示専用として残す。
3. `handleSaveSettings` の最初の `histories.invalidateLoads()` をpath編集時だけ実行する。path編集時は従来どおり適用後に履歴・runtime・libraryを一括再読込し、非path保存時はevent listenerを通常動作させてruntime statusだけを更新する。
4. `handleAiEnabledChange` で `next` を作った直後、draftを更新する前に `settings` と `confirmPathChange(settings, next)` を評価する。キャンセル時はAI値と未保存path draftを変えず、`onSaveSettings`を呼ばない。承認時のみ従来の全設定保存へ進む。
5. 変更差分を、非path保存中のvisit event、path切替中の旧DB応答、確認キャンセル時のdraft保持、MSRV互換性の観点で再レビューする。

### 検証手順

1. Rust 1.77.2が利用可能なら `cargo +1.77.2 check --manifest-path src-tauri/Cargo.toml` を実行し、宣言MSRVでcompileできることを確認する。
2. `cargo test --manifest-path src-tauri/Cargo.toml settings_apply::tests -- --nocapture`
3. `npm run check`
4. `npm run build`
5. 手動確認: 非path設定保存中の `visit-saved` が履歴更新へ到達する。
6. 手動確認: path変更保存では中間eventを表示せず、完了後の一括再読込で新しいDB/runtime状態になる。
7. 手動確認: 未保存のDB/log pathがある状態でAI toggleを変更すると確認が表示され、キャンセル時は保存されない。path未編集時は確認なしでAI設定だけが保存される。
8. `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` と `git diff --check`

### 未解決事項・前提

- 最新のremote headとlocal `HEAD` はともに `5222b853d34a18b36bdd551315706aec9a01fb03` で、作業ツリーは計画更新前にcleanだった。
- GitHubのthread-aware取得では、以前のP2 5件は解決済み、最新レビューの上記3件だけが未解決だった。
- frontendにはcomponent/unit test runnerがないため、event競合とconfirm分岐は静的検査・build・対象手動確認を受入条件とする。テスト基盤の新設は今回のスコープ外とする。
- 付属 `fetch_comments.py` はactive accountが認証済みでも、別のinactive accountの無効tokenにより `gh auth status` が非0終了となって取得前に停止した。レビュー状態はGitHub connectorのreview thread結果で再確認した。

### 実装結果

- `Option::is_none_or` をRust 1.77.2で利用可能な `Option::map_or` に置換し、paths_changed判定は維持した。
- `isApplyingSettings` と `isSwitchingSettingsPaths` を分離し、非path保存ではruntime eventと進行中loadを抑止しないようにした。
- AI toggleの自動保存前にも `confirmPathChange` を通し、キャンセル時はdraftとcheckbox表示を元へ戻すようにした。
- `cargo fmt --check`、設定適用test 11件、`npm run check`、`npm run build`、`git diff --check` は成功した。
- Rust 1.77.2のtoolchainは導入できたが、`cargo +1.77.2 check` は既存lockfileの `rand_core 0.10.1` がedition 2024対応Cargoを要求するため、今回のコードをcompileする前に停止した。dependency/MSRV整合の修正は今回のP2 3件の対象外として未変更。
