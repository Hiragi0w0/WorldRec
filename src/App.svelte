<script lang="ts">
    import { getCurrentWindow } from "@tauri-apps/api/window";
    import { onDestroy, onMount } from "svelte";
    import GlobalLoadingOverlay from "./lib/components/common/GlobalLoadingOverlay.svelte";
    import LogSyncModal from "./lib/components/history/LogSyncModal.svelte";
    import LibraryWorldDetailDrawer from "./lib/components/library/LibraryWorldDetailDrawer.svelte";
    import OnboardingFlow from "./lib/components/onboarding/OnboardingFlow.svelte";
    import WorldFocusDrawer from "./lib/components/world/WorldFocusDrawer.svelte";
    import AppShell from "./lib/components/shell/AppShell.svelte";
    import Sidebar from "./lib/components/shell/Sidebar.svelte";
    import TopBar from "./lib/components/shell/TopBar.svelte";
    import {
        deleteAllHistory,
        deleteVisitHistory,
        getSettings,
        getVrchatWorldDetail,
        saveSettings,
        type AppSettings,
        type LibraryWorld,
        type SettingsApplyResult,
    } from "./lib/api/commands";
    import {
        listenCurrentVisitChanged,
        listenLogWatchError,
        listenLogWatchStateChanged,
        listenOpenSettings,
        listenVisitSaved,
    } from "./lib/api/events";
    import AiGuideView from "./lib/components/views/AiGuideView.svelte";
    import LibraryView from "./lib/components/views/LibraryView.svelte";
    import RecordView from "./lib/components/views/RecordView.svelte";
    import SettingsView from "./lib/components/views/SettingsView.svelte";
    import StatsView from "./lib/components/views/StatsView.svelte";
    import {
        type PaperStyle,
        type Screen,
        type TapeStyle,
        type ViewFormat,
        type VisitRecord,
    } from "./lib/data/visitTypes";
    import { createHistoriesState } from "./lib/state/histories.svelte";
    import { createLibraryState } from "./lib/state/library.svelte";
    import {
        getCachedWorldPreview,
        loadWorldPreviews,
    } from "./lib/state/worldPreviews.svelte";
    import {
        buildTimelineRecords,
        calculateJourneySummary,
    } from "./lib/utils/history";
    import {
        loadWorldDetailPreview,
        mapVrchatWorldDetailToPreview,
        type WorldDetailPreview,
    } from "./lib/world/worldDetailPreview";

    const worldDetailCache = new Map<string, WorldDetailPreview>();

    let activeScreen = $state<Screen>("record");
    const histories = createHistoriesState();
    const library = createLibraryState();
    let tapeStyle = $state<TapeStyle>("kraft");
    let paperStyle = $state<PaperStyle>("dotted");
    let viewFormat = $state<ViewFormat>("list");
    let isSyncModalOpen = $state(false);
    let isSyncing = $state(false);
    let syncLogs = $state<string[]>([]);
    let eventUnlisteners: Array<() => void> = [];
    let runtimeInitialized = false;
    let appBootLoading = $state(false);
    let appSettings = $state<AppSettings | null>(null);
    let showOnboarding = $state(false);
    let selectedWorldRecord = $state<VisitRecord | null>(null);
    let selectedWorldDetail = $state<WorldDetailPreview | null>(null);
    let selectedWorldDetailLoading = $state(false);
    let selectedWorldDetailError = $state<string | null>(null);
    let worldDetailRequestSeq = 0;
    let selectedLibraryWorldId = $state<string | null>(null);
    let selectedLibraryWorldName = $state("");
    let isLibraryDetailOpen = $state(false);
    let isApplyingSettings = $state(false);
    let isSwitchingSettingsPaths = $state(false);

    let visitRecords = $derived(histories.visitRecords);
    let dateList = $derived(histories.dateList);
    let selectedDate = $derived(histories.selectedDate);
    let searchQuery = $derived(histories.searchQuery);
    let runtimeStatus = $derived(histories.runtimeStatus);
    let runtimeStatusLoading = $derived(histories.runtimeStatusLoading);
    let isLoading = $derived(histories.isLoading);
    let historyError = $derived(histories.error);
    let summary = $derived(calculateJourneySummary(visitRecords, selectedDate));
    let timelineRecords = $derived(
        buildTimelineRecords(visitRecords, selectedDate, searchQuery),
    );
    let uniquePreviewSourceRecords = $derived(
        uniqueLatestWorldRecords(visitRecords),
    );
    let selectedDateLabel = $derived(
        dateList.find((day) => day.key === selectedDate)?.display ||
            selectedDate,
    );
    let selectedWorldRelatedVisits = $derived(
        relatedVisitsForSelectedWorld(selectedWorldRecord, visitRecords),
    );
    let globalLoadingVisible = $derived(
        appBootLoading ||
            isLoading ||
            library.isLoading ||
            runtimeStatusLoading ||
            isSyncing ||
            isApplyingSettings,
    );
    let globalLoadingLabel = $derived(
        isSyncing
            ? "ログを同期しています..."
            : isApplyingSettings
              ? "設定を適用しています..."
            : appBootLoading
              ? "起動処理中..."
              : library.isLoading
                ? "ライブラリを読み込んでいます..."
                : isLoading
                  ? "DBを読み込んでいます..."
                  : runtimeStatusLoading
                    ? "状態を更新しています..."
                    : "Loading...",
    );

    $effect(() => {
        void loadWorldPreviews(
            uniquePreviewSourceRecords,
            (record) => record.worldId,
            (record) => ({
                worldId: record.worldId,
                worldName: record.worldName,
            }),
            3,
            (record) => ({
                imageUrl: record.thumbnailUrl ?? record.imageUrl,
                fetchedAt: record.worldPreviewFetchedAt,
            }),
        );
    });

    onMount(() => {
        startRuntimeOnce();
    });

    onDestroy(() => {
        eventUnlisteners.forEach((unlisten) => unlisten());
        eventUnlisteners = [];

        // アプリ終了時には watcher 停止 command を呼ばない。
        // 未確定 visit は RuntimeStatus の current_visit として扱い、
        // 確定済み visit だけを DB 履歴として保存する。
    });

    function startRuntimeOnce() {
        if (runtimeInitialized) return;
        runtimeInitialized = true;
        void initializeRuntime();
    }

    async function initializeRuntime() {
        appBootLoading = true;

        try {
            try {
                const settings = await getSettings();
                appSettings = settings;
                applyUiSettings(settings);
                showOnboarding = !settings.onboarding_completed;
            } catch (error) {
                histories.error = toErrorMessage(error);
            }

            await subscribeRuntimeEvents();
            await histories.refreshRuntimeStatus({ showLoading: true });
            await histories.startWatcher({ settleCurrentVisit: true });
            await histories.loadVisits(histories.mainVisitCriteria);
            await histories.refreshRuntimeStatus();
        } finally {
            appBootLoading = false;
        }
    }

    async function subscribeRuntimeEvents() {
        try {
            eventUnlisteners = [
                await listenVisitSaved(() => {
                    if (isSwitchingSettingsPaths) return;
                    void histories.handleVisitSaved();
                }),
                await listenLogWatchStateChanged((status) => {
                    if (isSwitchingSettingsPaths) return;
                    histories.setWatcherStatus(
                        status.running,
                        status.last_error,
                    );
                }),
                await listenLogWatchError((message) => {
                    if (isSwitchingSettingsPaths) return;
                    histories.setWatcherStatus(false, message);
                }),
                await listenCurrentVisitChanged(() => {
                    if (isSwitchingSettingsPaths) return;
                    void histories.refreshRuntimeStatus();
                }),
                await listenOpenSettings(() => {
                    navigate("settings");
                }),
                await subscribeCloseRequested(),
            ];
        } catch (error) {
            histories.error = toErrorMessage(error);
        }
    }

    async function subscribeCloseRequested() {
        const currentWindow = getCurrentWindow();

        return await currentWindow.onCloseRequested(async (event) => {
            // 常駐仕様: × はアプリ終了ではなくウィンドウ非表示。
            // 終了（ログ同期含む）はトレイメニューの「終了」で行う。
            event.preventDefault();
            await currentWindow.hide();
        });
    }

    function thumbnailForVisit(record: VisitRecord): string | null {
        const cached = getCachedWorldPreview(record.worldId);

        return (
            record.thumbnailUrl ??
            record.imageUrl ??
            cached?.thumbnailUrl ??
            cached?.imageUrl ??
            null
        );
    }

    function navigate(screen: Screen) {
        if (screen !== activeScreen && histories.searchQuery) {
            histories.setSearchQuery("");
        }
        activeScreen = screen;
    }

    async function handleDeleteVisitRecord(record: VisitRecord) {
        if (
            !confirm(
                "この訪問記録を削除しますか？この操作は元に戻せません。",
            )
        )
            return;

        if (record.visitId !== undefined) {
            try {
                await deleteVisitHistory(record.visitId);
            } catch (error) {
                histories.error = toErrorMessage(error);
                return;
            }
        } else {
            histories.visitRecords = histories.visitRecords.filter(
                (r) => r.id !== record.id,
            );
            return;
        }

        if (selectedWorldRecord?.id === record.id) {
            closeWorldDetail();
        }
        await histories.loadVisits(histories.currentCriteria);
        await histories.refreshRuntimeStatus();
        try {
            await library.loadLibrary();
        } catch (error) {
            console.error("library reload after delete failed", error);
        }
    }

    async function handleDeleteAllHistory(): Promise<number | null> {
        if (
            !confirm(
                "すべての訪問履歴を削除します。元に戻せません。続行しますか？",
            )
        )
            return null;
        if (!confirm("本当にすべて削除しますか？")) return null;

        let result;
        try {
            result = await deleteAllHistory();
        } catch (error) {
            histories.error = toErrorMessage(error);
            throw error;
        }

        closeWorldDetail();
        selectedLibraryWorldId = null;
        selectedLibraryWorldName = "";
        isLibraryDetailOpen = false;

        await histories.loadVisits(histories.currentCriteria);
        await histories.refreshRuntimeStatus();
        try {
            await library.loadLibrary();
        } catch (error) {
            console.error("library reload after delete all failed", error);
        }

        return result.deleted_count;
    }

    async function openWorldDetail(record: VisitRecord) {
        const requestSeq = ++worldDetailRequestSeq;
        selectedWorldRecord = record;
        selectedWorldDetail = loadWorldDetailPreview(record);
        selectedWorldDetailError = null;

        if (!record.worldId?.startsWith("wrld_")) {
            selectedWorldDetailLoading = false;
            return;
        }

        const cached = worldDetailCache.get(record.worldId);
        if (cached) {
            selectedWorldDetail = cached;
            selectedWorldDetailLoading = false;
            return;
        }

        selectedWorldDetailLoading = true;

        try {
            const apiDetail = await getVrchatWorldDetail(
                record.worldId,
                record.worldName,
            );
            const preview = mapVrchatWorldDetailToPreview(record, apiDetail);
            worldDetailCache.set(record.worldId, preview);

            if (requestSeq === worldDetailRequestSeq) {
                selectedWorldDetail = preview;
            }
        } catch (error) {
            console.warn(
                `get_vrchat_world_detail failed: ${toErrorMessage(error)}`,
            );
            if (requestSeq === worldDetailRequestSeq) {
                selectedWorldDetailError =
                    "ワールド詳細を取得できませんでした。VRChatログイン状態、ネットワーク接続、またはVRChat APIの応答を確認してください。";
            }
        } finally {
            if (requestSeq === worldDetailRequestSeq) {
                selectedWorldDetailLoading = false;
            }
        }
    }

    function closeWorldDetail() {
        worldDetailRequestSeq += 1;
        selectedWorldRecord = null;
        selectedWorldDetail = null;
        selectedWorldDetailLoading = false;
        selectedWorldDetailError = null;
    }

    function openLibraryWorldDetail(world: LibraryWorld) {
        selectedLibraryWorldId = world.world_id;
        selectedLibraryWorldName = world.world_name;
        isLibraryDetailOpen = true;
    }

    function openLibraryWorldDetailByIdentity(worldId: string | null, worldName: string) {
        selectedLibraryWorldId = worldId;
        selectedLibraryWorldName = worldName;
        isLibraryDetailOpen = true;
    }

    function closeLibraryWorldDetail() {
        isLibraryDetailOpen = false;
    }

    function relatedVisitsForSelectedWorld(
        selectedRecord: VisitRecord | null,
        records: VisitRecord[],
    ) {
        if (!selectedRecord) return [];

        return records.filter((record) => {
            if (selectedRecord.worldId) {
                return record.worldId === selectedRecord.worldId;
            }
            return record.worldName === selectedRecord.worldName;
        });
    }

    function openSyncModal() {
        isSyncModalOpen = true;
    }

    function closeSyncModal() {
        isSyncModalOpen = false;
        syncLogs = [];
    }

    async function startLogSync() {
        if (isSyncing) return;

        isSyncing = true;
        syncLogs = [
            `[${new Date().toLocaleTimeString()}] 最新 VRChat ログを同期します...`,
        ];

        try {
            const result = await histories.syncLatestLog();
            const latestLogFile = result.latest_log_file ?? "なし";
            const processedLabel = result.processed
                ? "読み込みました"
                : "対象ログなし";
            const watcherLabel = result.watcher_running ? "監視中" : "停止中";

            syncLogs = [
                ...syncLogs,
                `[${new Date().toLocaleTimeString()}] 最新ログ: ${latestLogFile} (${processedLabel})`,
                `[${new Date().toLocaleTimeString()}] 処理行数: ${result.processed_line_count}`,
                `[${new Date().toLocaleTimeString()}] DB反映: ${result.saved_visit_count}件`,
                `[${new Date().toLocaleTimeString()}] watcher: ${watcherLabel}`,
                `[${new Date().toLocaleTimeString()}] SQLite DB と runtime status を再読み込みしました。`,
            ];
        } catch (error) {
            syncLogs = [
                ...syncLogs,
                `[${new Date().toLocaleTimeString()}] エラー: ${toErrorMessage(error)}`,
            ];
        } finally {
            isSyncing = false;
        }
    }

    async function handleSaveSettings(
        next: AppSettings,
    ): Promise<SettingsApplyResult> {
        const previousSettings = appSettings;
        const dbPathWasEdited =
            previousSettings?.db_path.trim() !== next.db_path.trim();
        const logPathWasEdited =
            previousSettings?.log_dir.trim() !== next.log_dir.trim();
        const pathWasEdited = dbPathWasEdited || logPathWasEdited;

        isApplyingSettings = true;
        isSwitchingSettingsPaths = pathWasEdited;
        if (pathWasEdited) {
            histories.invalidateLoads();
        }
        try {
            const result = await saveSettings(next);
            appSettings = result.settings;
            applyUiSettings(result.settings);

            if (dbPathWasEdited) {
                closeWorldDetail();
                closeLibraryWorldDetail();
            }

            if (pathWasEdited) {
                histories.invalidateLoads();
                await Promise.all([
                    histories.loadVisits(histories.mainVisitCriteria),
                    histories.refreshRuntimeStatus({ showLoading: true }),
                    library.loadLibrary(),
                ]);
            } else {
                await histories.refreshRuntimeStatus();
            }

            return result;
        } finally {
            isSwitchingSettingsPaths = false;
            isApplyingSettings = false;
        }
    }

    async function handleReloadSettings(): Promise<AppSettings> {
        const settings = await getSettings();
        appSettings = settings;
        applyUiSettings(settings);
        showOnboarding = !settings.onboarding_completed;
        return settings;
    }

    async function handleCompleteOnboarding() {
        if (!appSettings) return;

        try {
            const result = await saveSettings({
                ...appSettings,
                onboarding_completed: true,
            });

            appSettings = result.settings;
            applyUiSettings(result.settings);
            showOnboarding = false;
        } catch (error) {
            histories.error = toErrorMessage(error);
        }
    }

    function applyUiSettings(settings: AppSettings) {
        document.documentElement.dataset.theme = settings.theme;
        document.documentElement.dataset.fontSize = settings.font_size;
        tapeStyle = settings.tape_style;
        paperStyle = settings.paper_style;
        viewFormat = settings.view_format;
    }

    function toErrorMessage(error: unknown) {
        if (error instanceof Error) return error.message;
        if (typeof error === "string") return error;
        return "Tauri command failed.";
    }

    function uniqueLatestWorldRecords(records: VisitRecord[]): VisitRecord[] {
        const seen = new Set<string>();
        const result: VisitRecord[] = [];

        for (const record of records) {
            const worldId = record.worldId;
            if (!worldId?.startsWith("wrld_")) continue;
            if (seen.has(worldId)) continue;

            seen.add(worldId);
            result.push(record);
        }

        return result;
    }
</script>

<svelte:head>
    <title>WorldRec</title>
</svelte:head>

<AppShell>
    {#snippet sidebar()}
        <Sidebar
            {activeScreen}
            visitCount={visitRecords.length}
            onNavigate={navigate}
            onOpenSync={openSyncModal}
        />
    {/snippet}

    {#snippet topbar()}
        <TopBar
            {searchQuery}
            {isSyncing}
            onSearchChange={(query) => histories.setSearchQuery(query)}
            onQuickSync={() => {
                isSyncModalOpen = true;
                void startLogSync();
            }}
        />
    {/snippet}

    {#if activeScreen === "record"}
        <RecordView
            {summary}
            {dateList}
            {selectedDate}
            {selectedDateLabel}
            {visitRecords}
            {timelineRecords}
            {searchQuery}
            isLoadingVisits={isLoading}
            visitLoadError={historyError}
            {paperStyle}
            {tapeStyle}
            {viewFormat}
            {runtimeStatus}
            onSelectDate={(dateKey) => histories.setSelectedDate(dateKey)}
            onViewFormatChange={(format) => (viewFormat = format)}
            onOpenSettings={() => navigate("settings")}
            onOpenSync={openSyncModal}
            onShowAllVisits={() => {
                void histories.loadVisits(histories.mainVisitCriteria);
                void histories.refreshRuntimeStatus();
            }}
            onOpenWorldDetail={openWorldDetail}
            {thumbnailForVisit}
            onDeleteRecord={handleDeleteVisitRecord}
        />
    {:else if activeScreen === "library"}
        <LibraryView
            {library}
            {tapeStyle}
            onOpenDetail={openLibraryWorldDetail}
        />
    {:else if activeScreen === "ai_guide"}
        <AiGuideView
            onOpenSettings={() => navigate("settings")}
            onOpenVisitedWorldDetail={openLibraryWorldDetailByIdentity}
        />
    {:else if activeScreen === "stats"}
        <StatsView onOpenWorldDetail={openLibraryWorldDetailByIdentity} />
    {:else if activeScreen === "settings"}
        <SettingsView
            {tapeStyle}
            {paperStyle}
            {runtimeStatus}
            {runtimeStatusLoading}
            {isLoading}
            error={historyError}
            settings={appSettings}
            onTapeStyleChange={(style) => (tapeStyle = style)}
            onPaperStyleChange={(style) => (paperStyle = style)}
            onReloadRuntimeStatus={() => {
                void histories.loadVisits(histories.currentCriteria);
                void histories.refreshRuntimeStatus({ showLoading: true });
            }}
            onStartWatcher={() => {
                void histories.startWatcher();
            }}
            onSaveSettings={handleSaveSettings}
            onReloadSettings={handleReloadSettings}
            onDeleteAllHistory={handleDeleteAllHistory}
        />
    {/if}
</AppShell>

{#if runtimeStatus || historyError}
    <span class="sr-only" aria-live="polite">
        VRChat Log Sync:
        {historyError
            ? "エラー"
            : runtimeStatus?.watcher_running
              ? "監視中"
              : "停止中"}
    </span>
{/if}

<WorldFocusDrawer
    isOpen={selectedWorldRecord !== null && selectedWorldDetail !== null}
    selectedRecord={selectedWorldRecord}
    detail={selectedWorldDetail}
    detailLoading={selectedWorldDetailLoading}
    detailError={selectedWorldDetailError}
    relatedVisits={selectedWorldRelatedVisits}
    onClose={closeWorldDetail}
/>

<LibraryWorldDetailDrawer
    isOpen={isLibraryDetailOpen}
    worldId={selectedLibraryWorldId}
    worldName={selectedLibraryWorldName}
    onClose={closeLibraryWorldDetail}
/>

{#if isSyncModalOpen}
    <LogSyncModal
        {syncLogs}
        {isSyncing}
        onStartSync={startLogSync}
        onClose={closeSyncModal}
    />
{/if}

<GlobalLoadingOverlay
    visible={globalLoadingVisible}
    label={globalLoadingLabel}
/>

{#if showOnboarding && appSettings}
    <OnboardingFlow
        settings={appSettings}
        {runtimeStatus}
        onOpenSettings={() => navigate("settings")}
        onComplete={handleCompleteOnboarding}
    />
{/if}
