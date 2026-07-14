<script lang="ts">
    import { onMount } from "svelte";
    import { KeyRound, RefreshCcw, Save, Trash2 } from "lucide-svelte";
    import type {
        AiStatus,
        AppSettings,
        RuntimeStatusDto,
        SettingsApplyResult,
        VrchatAuthStatus,
        VrchatAuthResult,
    } from "../../api/commands";
    import {
        clearGeminiApiKey,
        getAiStatus,
        getAppAutostartStatus,
        getVrchatAuthStatus,
        saveGeminiApiKey,
        setAppAutostartEnabled,
        vrchatLogin,
        vrchatCompleteEmail2fa,
        clearVrchatLoginData,
    } from "../../api/commands";
    import type { PaperStyle, TapeStyle } from "../../data/visitTypes";
    import { paperClass, tapeClass } from "../../utils/history";
    import RuntimeStatusCard from "../history/RuntimeStatusCard.svelte";

    export let tapeStyle: TapeStyle;
    export let paperStyle: PaperStyle;
    export let runtimeStatus: RuntimeStatusDto | null;
    export let runtimeStatusLoading: boolean;
    export let isLoading: boolean;
    export let error: string | null;
    export let settings: AppSettings | null;
    export let onTapeStyleChange: (style: TapeStyle) => void;
    export let onPaperStyleChange: (style: PaperStyle) => void;
    export let onReloadRuntimeStatus: () => void;
    export let onStartWatcher: () => void;
    export let onSaveSettings: (
        settings: AppSettings,
    ) => Promise<SettingsApplyResult>;
    export let onReloadSettings: () => Promise<AppSettings>;
    export let onDeleteAllHistory: () => Promise<number | null>;

    let draft: AppSettings | null = null;
    let isSavingSettings = false;
    let isReloadingSettings = false;
    let settingsSaveState: "idle" | "success" | "error" = "idle";
    let settingsSaveError: string | null = null;
    let settingsSaveMessage: string | null = null;
    let settingsSaveDetails: string | null = null;
    let isDeletingAllHistory = false;
    let deleteAllHistoryResult: number | null = null;
    let deleteAllHistoryError: string | null = null;

    let appAutostartEnabled: boolean | null = null;
    let autostartUpdating = false;
    let autostartError: string | null = null;

    let aiStatus: AiStatus | null = null;
    let aiStatusLoading = false;
    let geminiApiKeyDraft = "";
    let aiKeySaving = false;
    let aiKeyClearing = false;
    let aiKeyMessage: string | null = null;
    let aiKeyError: string | null = null;

    // VRChat login state
    let vrchatStatus: VrchatAuthStatus | null = null;
    let vrchatStatusLoading = false;
    let vrchatLoginLoading = false;
    let vrchat2faLoading = false;
    let vrchatClearLoading = false;
    let vrchatUsername = "";
    let vrchatPassword = "";
    let vrchatOtpCode = "";
    let vrchatError: string | null = null;

    onMount(async () => {
        await Promise.all([
            reloadAiStatus(),
            reloadVrchatStatus(),
            reloadAutostartStatus(),
        ]);
    });

    async function reloadAutostartStatus() {
        autostartError = null;
        try {
            const status = await getAppAutostartStatus();
            appAutostartEnabled = status.enabled;
        } catch (err) {
            autostartError = err instanceof Error ? err.message : String(err);
        }
    }

    async function handleAppAutostartChange(checked: boolean) {
        if (autostartUpdating) return;

        const previous =
            appAutostartEnabled ?? draft?.app_autostart_enabled ?? false;
        autostartUpdating = true;
        autostartError = null;
        appAutostartEnabled = checked;

        try {
            const status = await setAppAutostartEnabled(checked);
            appAutostartEnabled = status.enabled;
            if (draft) {
                draft = { ...draft, app_autostart_enabled: status.enabled };
            }
        } catch (err) {
            appAutostartEnabled = previous;
            autostartError = err instanceof Error ? err.message : String(err);
        } finally {
            autostartUpdating = false;
        }
    }

    async function reloadAiStatus() {
        aiStatusLoading = true;
        aiKeyError = null;
        try {
            aiStatus = await getAiStatus();
            if (draft) {
                draft = {
                    ...draft,
                    has_gemini_api_key: aiStatus.hasGeminiApiKey,
                };
            }
        } catch (err) {
            aiKeyError = err instanceof Error ? err.message : String(err);
        } finally {
            aiStatusLoading = false;
        }
    }

    async function reloadVrchatStatus() {
        vrchatStatusLoading = true;
        vrchatError = null;
        try {
            vrchatStatus = await getVrchatAuthStatus();
        } catch (err) {
            vrchatError = err instanceof Error ? err.message : String(err);
        } finally {
            vrchatStatusLoading = false;
        }
    }

    async function handleVrchatLogin() {
        vrchatLoginLoading = true;
        vrchatError = null;
        try {
            const result: VrchatAuthResult = await vrchatLogin(
                vrchatUsername,
                vrchatPassword,
            );
            if (result.status === "logged_in") {
                vrchatPassword = "";
                await reloadVrchatStatus();
            } else if (result.status === "requires_email_2fa") {
                vrchatPassword = "";
                vrchatStatus = {
                    loggedIn: false,
                    requiresEmail2fa: true,
                    displayName: null,
                    userId: null,
                    message: result.message,
                };
            } else {
                vrchatError =
                    result.message ?? "ログインに失敗しました。";
            }
        } catch (err) {
            vrchatError = err instanceof Error ? err.message : String(err);
        } finally {
            vrchatLoginLoading = false;
        }
    }

    async function handleVrchat2fa() {
        vrchat2faLoading = true;
        vrchatError = null;
        try {
            const result: VrchatAuthResult = await vrchatCompleteEmail2fa(vrchatOtpCode);
            if (result.status === "logged_in") {
                vrchatOtpCode = "";
                await reloadVrchatStatus();
            } else {
                vrchatError = result.message ?? "2FA認証に失敗しました。";
            }
        } catch (err) {
            vrchatError = err instanceof Error ? err.message : String(err);
        } finally {
            vrchat2faLoading = false;
        }
    }

    async function handleVrchatClearLogin() {
        if (
            !confirm(
                "VRChatのログインデータを破棄します。続行しますか？",
            )
        )
            return;
        vrchatClearLoading = true;
        vrchatError = null;
        try {
            await clearVrchatLoginData();
            vrchatUsername = "";
            vrchatPassword = "";
            vrchatOtpCode = "";
            await reloadVrchatStatus();
        } catch (err) {
            vrchatError = err instanceof Error ? err.message : String(err);
        } finally {
            vrchatClearLoading = false;
        }
    }

    const tapeOptions = [
        { key: "kraft", name: "クラフト", color: "#EADDC9", border: "#D2C3AB" },
        { key: "mint", name: "ミント", color: "#A7F3D0", border: "#86EFAC" },
        {
            key: "lavender",
            name: "ラベンダー",
            color: "#DDD6FE",
            border: "#C084FC",
        },
        { key: "pink", name: "ピンク", color: "#FBCFE8", border: "#F472B6" },
    ] as const;

    const paperOptions = [
        { key: "dotted", name: "グリッド", desc: "ドット方眼" },
        { key: "lined", name: "罫線", desc: "横罫線" },
        { key: "blank", name: "白無地", desc: "プレーン" },
    ] as const;

    $: if (settings && draft === null) {
        draft = { ...settings };
    }

    $: if (
        draft &&
        aiStatus &&
        draft.has_gemini_api_key !== aiStatus.hasGeminiApiKey
    ) {
        draft = { ...draft, has_gemini_api_key: aiStatus.hasGeminiApiKey };
    }

    function updateDraft(patch: Partial<AppSettings>) {
        if (!draft) return;
        draft = { ...draft, ...patch };
        settingsSaveState = "idle";
        settingsSaveMessage = null;
        settingsSaveDetails = null;
    }

    function pathWasEdited(current: AppSettings, next: AppSettings) {
        return (
            current.log_dir.trim() !== next.log_dir.trim() ||
            current.db_path.trim() !== next.db_path.trim()
        );
    }

    function confirmPathChange(current: AppSettings, next: AppSettings) {
        if (!pathWasEdited(current, next)) return true;

        const dbChanged = current.db_path.trim() !== next.db_path.trim();
        const message = dbChanged
            ? "DB保存先またはVRChatログフォルダを変更します。\n\n既存DBの履歴は自動コピーされません。新しいDBが空の場合、履歴は0件に見えます。元のDBパスへ戻せば元の履歴を再表示できます。DBファイルは自動削除されません。\n\n続行しますか？"
            : "VRChatログフォルダを変更します。ログ監視を安全に停止し、新しいフォルダで再開します。続行しますか？";

        return confirm(message);
    }

    function applySaveResult(result: SettingsApplyResult) {
        draft = { ...result.settings };
        settingsSaveMessage = result.message;
        settingsSaveDetails = [result.primary_error, result.rollback_error]
            .filter((value): value is string => Boolean(value))
            .join("\n") || null;
        settingsSaveState = result.outcome === "applied" ? "success" : "error";
    }

    async function handleSaveSettings() {
        if (!draft) return;
        if (settings && !confirmPathChange(settings, draft)) return;

        isSavingSettings = true;
        settingsSaveState = "idle";
        settingsSaveError = null;
        settingsSaveMessage = null;
        settingsSaveDetails = null;
        try {
            const result = await onSaveSettings(draft);
            applySaveResult(result);
        } catch (error) {
            settingsSaveState = "error";
            settingsSaveError =
                error instanceof Error ? error.message : String(error);
        } finally {
            isSavingSettings = false;
        }
    }

    async function handleAiEnabledChange(checked: boolean) {
        if (!draft || isSavingSettings) return;

        const previous = draft;
        const next = { ...draft, ai_enabled: checked };
        draft = next;
        settingsSaveState = "idle";
        settingsSaveError = null;
        settingsSaveMessage = null;
        settingsSaveDetails = null;
        isSavingSettings = true;

        try {
            const result = await onSaveSettings(next);
            applySaveResult(result);
        } catch (error) {
            draft = previous;
            settingsSaveState = "error";
            settingsSaveError =
                error instanceof Error ? error.message : String(error);
        } finally {
            isSavingSettings = false;
        }
    }

    async function handleReloadSettings() {
        isReloadingSettings = true;
        settingsSaveState = "idle";
        settingsSaveError = null;
        settingsSaveMessage = null;
        settingsSaveDetails = null;
        try {
            const fresh = await onReloadSettings();
            draft = { ...fresh };
        } catch (error) {
            settingsSaveState = "error";
            settingsSaveError =
                error instanceof Error ? error.message : String(error);
        } finally {
            isReloadingSettings = false;
        }
    }

    async function refreshSettingsAfterAiSecretChange(status: AiStatus) {
        aiStatus = status;
        const fresh = await onReloadSettings();
        draft = {
            ...fresh,
            has_gemini_api_key: status.hasGeminiApiKey,
        };
    }

    async function handleSaveGeminiApiKey() {
        const apiKey = geminiApiKeyDraft.trim();
        if (!apiKey) {
            aiKeyMessage = null;
            aiKeyError = "Gemini APIキーを入力してください。";
            return;
        }

        aiKeySaving = true;
        aiKeyMessage = null;
        aiKeyError = null;
        try {
            const status = await saveGeminiApiKey(apiKey);
            geminiApiKeyDraft = "";
            await refreshSettingsAfterAiSecretChange(status);
            aiKeyMessage = "Gemini APIキーを保存しました。";
        } catch (err) {
            aiKeyError = err instanceof Error ? err.message : String(err);
        } finally {
            aiKeySaving = false;
        }
    }

    async function handleClearGeminiApiKey() {
        if (
            !confirm(
                "保存済みのGemini APIキーを削除します。続行しますか？",
            )
        )
            return;

        aiKeyClearing = true;
        aiKeyMessage = null;
        aiKeyError = null;
        try {
            const status = await clearGeminiApiKey();
            geminiApiKeyDraft = "";
            await refreshSettingsAfterAiSecretChange(status);
            aiKeyMessage = "Gemini APIキーを削除しました。";
        } catch (err) {
            aiKeyError = err instanceof Error ? err.message : String(err);
        } finally {
            aiKeyClearing = false;
        }
    }

    async function handleDeleteAllHistory() {
        isDeletingAllHistory = true;
        deleteAllHistoryResult = null;
        deleteAllHistoryError = null;
        try {
            const count = await onDeleteAllHistory();
            deleteAllHistoryResult = count;
        } catch (err) {
            deleteAllHistoryError =
                err instanceof Error ? err.message : String(err);
        } finally {
            isDeletingAllHistory = false;
        }
    }
</script>

<section
    class="space-y-6 bg-white border border-zinc-200 p-8 rounded-3xl animate-fadeIn"
>
    <header
        class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between"
    >
        <h2 class="text-xl font-extrabold text-zinc-800 tracking-tight">
            設定
        </h2>
        <div class="flex gap-2">
            <button
                onclick={handleReloadSettings}
                disabled={isReloadingSettings || isSavingSettings}
                class="bg-white hover:bg-neutral-50 disabled:opacity-50 disabled:cursor-not-allowed text-zinc-700 border border-zinc-300 py-2 px-3 rounded-xl font-bold text-xs flex items-center justify-center gap-2 shadow-sm"
            >
                <RefreshCcw class="w-4 h-4 text-zinc-400" />
                設定を再読み込み
            </button>
            <button
                onclick={handleSaveSettings}
                disabled={isSavingSettings || !draft}
                class="bg-[#1e5854] hover:bg-[#164743] disabled:opacity-50 disabled:cursor-not-allowed text-white border border-[#164743] py-2 px-3 rounded-xl font-bold text-xs flex items-center justify-center gap-2 shadow-sm"
            >
                <Save class="w-4 h-4" />
                設定を保存
            </button>
        </div>
    </header>

    {#if draft === null}
        <p class="text-xs text-zinc-400">設定を読み込んでいます...</p>
    {:else}
        <div class="grid grid-cols-1 xl:grid-cols-2 gap-6">
            <section
                class="bg-neutral-50/70 border border-zinc-200 rounded-2xl p-5 space-y-5"
            >
                <h3
                    class="text-xs font-black text-zinc-400 tracking-wider uppercase"
                >
                    全般設定
                </h3>

                <label class="block">
                    <span class="text-xs font-bold text-zinc-600 block"
                        >テーマ</span
                    >
                    <select
                        value={draft.theme}
                        onchange={(event) =>
                            updateDraft({
                                theme: (event.currentTarget as HTMLSelectElement)
                                    .value as AppSettings["theme"],
                            })}
                        class="w-full bg-white border border-zinc-200 rounded-xl p-3 text-sm focus:outline-none focus:border-[#1e5854] mt-1.5 text-zinc-700 font-bold"
                    >
                        <option value="system">システム</option>
                        <option value="light">ライト</option>
                        <option value="dark">ダーク</option>
                    </select>
                </label>

                <label class="block">
                    <span class="text-xs font-bold text-zinc-600 block"
                        >文字サイズ</span
                    >
                    <select
                        value={draft.font_size}
                        onchange={(event) =>
                            updateDraft({
                                font_size: (event.currentTarget as HTMLSelectElement)
                                    .value as AppSettings["font_size"],
                            })}
                        class="w-full bg-white border border-zinc-200 rounded-xl p-3 text-sm focus:outline-none focus:border-[#1e5854] mt-1.5 text-zinc-700 font-bold"
                    >
                        <option value="standard">標準</option>
                        <option value="large">大きめ</option>
                    </select>
                </label>

                <div class="space-y-3">
                    <span class="text-xs font-bold text-zinc-600 block"
                        >マスキングテープ色</span
                    >
                    <div class="grid grid-cols-2 sm:grid-cols-4 gap-2">
                        {#each tapeOptions as style}
                            <button
                                onclick={() => {
                                    updateDraft({ tape_style: style.key });
                                    onTapeStyleChange(style.key);
                                }}
                                class={`flex items-center gap-2 p-2.5 text-xs font-bold border rounded-xl transition-all ${(draft?.tape_style ?? tapeStyle) === style.key ? "bg-[#1e5854]/10 text-[#0f4743] border-[#1e5854] ring-2 ring-[#1e5854]/20" : "bg-white text-zinc-700 border-zinc-200 hover:bg-zinc-50/50"}`}
                            >
                                <span
                                    class="w-3.5 h-3.5 rounded-full border shadow-inner block shrink-0"
                                    style={`background-color: ${style.color}; border-color: ${style.border}`}
                                ></span>
                                <span>{style.name}</span>
                            </button>
                        {/each}
                    </div>
                </div>

                <div class="space-y-3">
                    <span class="text-xs font-bold text-zinc-600 block"
                        >手帳の用紙テクスチャ</span
                    >
                    <div class="grid grid-cols-1 sm:grid-cols-3 gap-2">
                        {#each paperOptions as style}
                            <button
                                onclick={() => {
                                    updateDraft({ paper_style: style.key });
                                    onPaperStyleChange(style.key);
                                }}
                                class={`flex flex-col items-start p-3 text-left border rounded-xl transition-all ${(draft?.paper_style ?? paperStyle) === style.key ? "bg-[#1e5854]/10 text-[#0f4743] border-[#1e5854] ring-2 ring-[#1e5854]/20" : "bg-white text-zinc-700 border-zinc-200 hover:bg-zinc-50/50"}`}
                            >
                                <span class="text-xs font-bold mb-0.5"
                                    >{style.name}</span
                                >
                                <span
                                    class="text-[10px] text-zinc-400 font-normal"
                                    >{style.desc}</span
                                >
                            </button>
                        {/each}
                    </div>
                </div>

                <label class="block">
                    <span class="text-xs font-bold text-zinc-600 block"
                        >履歴の表示形式</span
                    >
                    <select
                        value={draft.view_format}
                        onchange={(event) =>
                            updateDraft({
                                view_format: (event.currentTarget as HTMLSelectElement)
                                    .value as AppSettings["view_format"],
                            })}
                        class="w-full bg-white border border-zinc-200 rounded-xl p-3 text-sm focus:outline-none focus:border-[#1e5854] mt-1.5 text-zinc-700 font-bold"
                    >
                        <option value="list">リスト</option>
                        <option value="grid">グリッド</option>
                    </select>
                </label>

                <label
                    class="flex items-center justify-between gap-4 bg-white border border-zinc-200 rounded-xl p-4"
                >
                    <span>
                        <span class="text-sm font-extrabold text-zinc-700 block"
                            >Windowsログイン時にWorldRecを自動起動する</span
                        >
                        <span class="text-xs text-zinc-500 block mt-1"
                            >有効にすると、Windowsログイン後にWorldRecを通知領域で起動し、VRChatログ監視をバックグラウンドで開始します。画面は自動表示されません。タスクバー右側の通知領域アイコンから開けます。</span
                        >
                    </span>
                    <input
                        type="checkbox"
                        checked={appAutostartEnabled ??
                            draft.app_autostart_enabled}
                        disabled={autostartUpdating}
                        onchange={(event) =>
                            void handleAppAutostartChange(
                                (event.currentTarget as HTMLInputElement)
                                    .checked,
                            )}
                        class="w-5 h-5 accent-[#1e5854] shrink-0 disabled:opacity-50 disabled:cursor-not-allowed"
                    />
                </label>
                <p
                    class="text-xs text-zinc-500 bg-white border border-zinc-200 rounded-xl px-3 py-2 leading-relaxed"
                >
                    ×ボタンではWorldRecは終了せず、通知領域にしまわれます。完全に終了する場合は、タスクバー右側の通知領域アイコンを右クリックして「終了」を選んでください。
                </p>
                {#if autostartError}
                    <p
                        class="text-xs font-bold text-rose-700 bg-rose-50 border border-rose-100 rounded-xl px-3 py-2"
                    >
                        {autostartError}
                    </p>
                {/if}

                <div
                    class="bg-[#FAF9F5]/70 p-4 rounded-2xl border border-[#DFCFA9] relative overflow-hidden"
                >
                    <span
                        class="text-[9px] font-black text-[#A68F63] uppercase tracking-widest block mb-2"
                        >手帳デザイン ライブプレビュー</span
                    >
                    <div
                        class={`relative p-5 rounded-xl border border-[#D5D0BC] min-h-[110px] transition-all shadow-inner overflow-hidden ${paperClass(draft?.paper_style ?? paperStyle)}`}
                    >
                        <div
                            class="absolute top-0 bottom-0 left-3 flex flex-col justify-around pointer-events-none opacity-20"
                        >
                            <div
                                class="w-2 h-2 rounded-full bg-zinc-400/50"
                            ></div>
                            <div
                                class="w-2 h-2 rounded-full bg-zinc-400/50"
                            ></div>
                            <div
                                class="w-2 h-2 rounded-full bg-zinc-400/50"
                            ></div>
                        </div>
                        <div
                            class="ml-6 bg-white p-3 rounded-lg border border-zinc-200/85 shadow-sm relative max-w-sm"
                        >
                            <div
                                class={`absolute -top-2.5 left-1/2 -translate-x-1/2 w-14 h-4 border border-dashed rounded-sm rotate-1 text-[8px] font-bold text-center flex items-center justify-center opacity-90 shadow-sm ${tapeClass(draft?.tape_style ?? tapeStyle)}`}
                            >
                                PREVIEW
                            </div>
                            <div class="mt-1 text-center font-travel">
                                <span
                                    class="text-[10px] font-bold text-zinc-400 block tracking-wider uppercase"
                                    >World Travel Live</span
                                ><span
                                    class="text-xs font-extrabold text-[#1a4441] block mt-0.5"
                                    >表示プレビュー</span
                                >
                            </div>
                        </div>
                    </div>
                </div>
            </section>

            <section
                class="bg-neutral-50/70 border border-zinc-200 rounded-2xl p-5 space-y-5"
            >
                <h3
                    class="text-xs font-black text-zinc-400 tracking-wider uppercase"
                >
                    DB・ログ監視設定
                </h3>

                <RuntimeStatusCard
                    {runtimeStatus}
                    {runtimeStatusLoading}
                    {isLoading}
                    {error}
                    onReload={onReloadRuntimeStatus}
                    {onStartWatcher}
                />

                <label class="block">
                    <span class="text-xs font-bold text-zinc-600 block"
                        >VRChatログフォルダ</span
                    >
                    <input
                        type="text"
                        value={draft.log_dir}
                        disabled={isSavingSettings}
                        oninput={(event) =>
                            updateDraft({
                                log_dir: (event.currentTarget as HTMLInputElement).value,
                            })}
                        class="w-full bg-white border border-zinc-200 rounded-xl p-3 text-sm focus:outline-none focus:border-[#1e5854] mt-1.5 text-zinc-700 font-bold disabled:opacity-50 disabled:cursor-not-allowed"
                    />
                </label>

                <label class="block">
                    <span class="text-xs font-bold text-zinc-600 block"
                        >DB保存先</span
                    >
                    <input
                        type="text"
                        value={draft.db_path}
                        disabled={isSavingSettings}
                        oninput={(event) =>
                            updateDraft({
                                db_path: (event.currentTarget as HTMLInputElement).value,
                            })}
                        class="w-full bg-white border border-zinc-200 rounded-xl p-3 text-sm focus:outline-none focus:border-[#1e5854] mt-1.5 text-zinc-700 font-bold disabled:opacity-50 disabled:cursor-not-allowed"
                    />
                </label>

                <div class="grid grid-cols-1 sm:grid-cols-2 gap-4">
                    <label class="block">
                        <span class="text-xs font-bold text-zinc-600 block"
                            >保存間隔</span
                        >
                        <input
                            type="number"
                            step="0.5"
                            value={draft.batch_flush_seconds}
                            oninput={(event) =>
                                updateDraft({
                                    batch_flush_seconds: Number(
                                        (event.currentTarget as HTMLInputElement)
                                            .value,
                                    ),
                                })}
                            class="w-full bg-white border border-zinc-200 rounded-xl p-3 text-sm focus:outline-none focus:border-[#1e5854] mt-1.5 text-zinc-700 font-bold"
                        />
                    </label>

                    <label class="block">
                        <span class="text-xs font-bold text-zinc-600 block"
                            >まとめて保存する件数</span
                        >
                        <input
                            type="number"
                            step="1"
                            value={draft.batch_max_events}
                            oninput={(event) =>
                                updateDraft({
                                    batch_max_events: Math.trunc(
                                        Number(
                                            (
                                                event.currentTarget as HTMLInputElement
                                            ).value,
                                        ),
                                    ),
                                })}
                            class="w-full bg-white border border-zinc-200 rounded-xl p-3 text-sm focus:outline-none focus:border-[#1e5854] mt-1.5 text-zinc-700 font-bold"
                        />
                    </label>
                </div>
            </section>
        </div>

        <section
            class="bg-neutral-50/70 border border-zinc-200 rounded-2xl p-5 space-y-5"
        >
            <div
                class="flex flex-col gap-2 sm:flex-row sm:items-start sm:justify-between"
            >
                <div>
                    <h3
                        class="text-xs font-black text-zinc-400 tracking-wider uppercase"
                    >
                        AI設定
                    </h3>
                </div>
                <p
                    class={`text-xs font-bold rounded-xl px-3 py-2 border w-fit ${draft.has_gemini_api_key ? "text-emerald-700 bg-emerald-50 border-emerald-200" : "text-amber-700 bg-amber-50 border-amber-200"}`}
                >
                    {aiStatusLoading
                        ? "確認中..."
                        : draft.has_gemini_api_key
                          ? "Gemini APIキー保存済み"
                          : "Gemini APIキー未設定"}
                </p>
            </div>

            <p class="text-sm text-zinc-600 bg-white border border-zinc-200 rounded-xl p-3">
                AI探索を使うと、入力した相談内容と、訪問済みワールドの名前・ワールドID・訪問回数を Gemini に送信します。メモ、タグ、ログファイル名、インスタンス情報は送信しません。
            </p>

            <label
                class="flex items-center justify-between gap-4 bg-white border border-zinc-200 rounded-xl p-4"
            >
                <span>
                    <span class="text-sm font-extrabold text-zinc-700 block"
                        >AI探索を有効にする</span
                    >
                    <span class="text-xs text-zinc-500 block mt-1"
                        >オフの間は、Gemini への送信は行いません。</span
                    >
                </span>
                <input
                    type="checkbox"
                    checked={draft.ai_enabled}
                    disabled={isSavingSettings}
                    onchange={(event) =>
                        void handleAiEnabledChange(
                            (event.currentTarget as HTMLInputElement).checked,
                        )}
                    class="w-5 h-5 accent-[#1e5854] shrink-0 disabled:opacity-50 disabled:cursor-not-allowed"
                />
            </label>

            <div class="grid grid-cols-1 lg:grid-cols-[1fr_auto] gap-3 items-end">
                <label class="block">
                    <span class="text-xs font-bold text-zinc-600 block"
                        >Gemini APIキー</span
                    >
                    <input
                        type="password"
                        autocomplete="off"
                        bind:value={geminiApiKeyDraft}
                        placeholder={draft.has_gemini_api_key
                            ? "新しいキーを入力すると上書きします"
                            : "Gemini APIキーを入力"}
                        disabled={aiKeySaving || aiKeyClearing}
                        class="w-full bg-white border border-zinc-200 rounded-xl p-3 text-sm focus:outline-none focus:border-[#1e5854] mt-1.5 text-zinc-700 font-bold disabled:opacity-60"
                    />
                    <span class="text-xs text-zinc-500 block mt-2"
                        >使用モデル: gemini-2.5-flash</span
                    >
                </label>
                <div class="flex flex-wrap gap-2">
                    <button
                        onclick={() => void handleSaveGeminiApiKey()}
                        disabled={aiKeySaving || aiKeyClearing || !geminiApiKeyDraft.trim()}
                        class="bg-[#1e5854] hover:bg-[#164743] disabled:opacity-50 disabled:cursor-not-allowed text-white border border-[#164743] py-2 px-3 rounded-xl font-bold text-xs flex items-center justify-center gap-2 shadow-sm"
                    >
                        <KeyRound class="w-4 h-4" />
                        {aiKeySaving ? "保存中..." : "APIキーを保存"}
                    </button>
                    <button
                        onclick={() => void handleClearGeminiApiKey()}
                        disabled={aiKeySaving || aiKeyClearing || !draft.has_gemini_api_key}
                        class="bg-white hover:bg-neutral-50 disabled:opacity-50 disabled:cursor-not-allowed text-rose-700 border border-rose-200 py-2 px-3 rounded-xl font-bold text-xs flex items-center justify-center gap-2 shadow-sm"
                    >
                        <Trash2 class="w-4 h-4" />
                        {aiKeyClearing ? "削除中..." : "APIキーを削除"}
                    </button>
                    <button
                        onclick={() => void reloadAiStatus()}
                        disabled={aiStatusLoading}
                        class="bg-white hover:bg-neutral-50 disabled:opacity-50 disabled:cursor-not-allowed text-zinc-700 border border-zinc-300 py-2 px-3 rounded-xl font-bold text-xs shadow-sm"
                    >
                        状態を再確認
                    </button>
                </div>
            </div>

            {#if aiKeyMessage}
                <p class="text-xs font-bold text-emerald-700 bg-emerald-50 border border-emerald-100 rounded-xl px-3 py-2">
                    {aiKeyMessage}
                </p>
            {/if}
            {#if aiKeyError}
                <p class="text-xs font-bold text-rose-700 bg-rose-50 border border-rose-100 rounded-xl px-3 py-2">
                    {aiKeyError}
                </p>
            {/if}
        </section>

        <section
            class="bg-neutral-50/70 border border-zinc-200 rounded-2xl p-5 space-y-5"
        >
            <h3
                class="text-xs font-black text-zinc-400 tracking-wider uppercase"
            >
                VRChatアカウント連携
            </h3>
            <p class="text-xs text-zinc-400">
                パスワードと2FAコードは保存しません。保存するのはVRChatのauth cookieのみです。ログインしなくてもローカル履歴・ログ同期は利用できます。
            </p>

            {#if vrchatStatusLoading}
                <p class="text-xs text-zinc-400">確認中...</p>
            {:else if vrchatStatus?.loggedIn}
                <p class="text-xs font-bold text-emerald-700 bg-emerald-50 border border-emerald-200 rounded-xl px-3 py-2">
                    ログイン済み{vrchatStatus.displayName ? `：${vrchatStatus.displayName}` : ""}
                </p>
            {:else if vrchatStatus?.requiresEmail2fa}
                <p class="text-xs font-bold text-amber-700 bg-amber-50 border border-amber-200 rounded-xl px-3 py-2">
                    email 2FA認証待ち
                </p>
            {:else if vrchatStatus?.message}
                <p class="text-xs font-bold text-amber-700 bg-amber-50 border border-amber-200 rounded-xl px-3 py-2">
                    {vrchatStatus.message}
                </p>
            {:else}
                <p class="text-xs font-bold text-zinc-500 bg-zinc-50 border border-zinc-200 rounded-xl px-3 py-2">
                    未ログイン
                </p>
            {/if}

            <div class="flex flex-col gap-3">
                <label class="block">
                    <span class="text-xs font-bold text-zinc-600 block">ユーザー名またはメール</span>
                    <input
                        type="text"
                        autocomplete="username"
                        bind:value={vrchatUsername}
                        disabled={vrchatStatus?.loggedIn || vrchatStatus?.requiresEmail2fa || vrchatStatusLoading || vrchatLoginLoading}
                        placeholder="user@example.com"
                        class={`w-full bg-white border border-zinc-200 rounded-xl p-3 text-sm focus:outline-none focus:border-[#1e5854] mt-1.5 text-zinc-700 font-bold ${vrchatStatus?.loggedIn || vrchatStatus?.requiresEmail2fa || vrchatStatusLoading || vrchatLoginLoading ? "opacity-50 cursor-not-allowed" : ""}`}
                    />
                </label>
                <label class="block">
                    <span class="text-xs font-bold text-zinc-600 block">パスワード</span>
                    <input
                        type="password"
                        autocomplete="current-password"
                        bind:value={vrchatPassword}
                        disabled={vrchatStatus?.loggedIn || vrchatStatus?.requiresEmail2fa || vrchatStatusLoading || vrchatLoginLoading}
                        placeholder="••••••••"
                        class={`w-full bg-white border border-zinc-200 rounded-xl p-3 text-sm focus:outline-none focus:border-[#1e5854] mt-1.5 text-zinc-700 font-bold ${vrchatStatus?.loggedIn || vrchatStatus?.requiresEmail2fa || vrchatStatusLoading || vrchatLoginLoading ? "opacity-50 cursor-not-allowed" : ""}`}
                    />
                </label>
                <button
                    onclick={handleVrchatLogin}
                    disabled={vrchatStatus?.loggedIn || vrchatStatus?.requiresEmail2fa || vrchatStatusLoading || vrchatLoginLoading || !vrchatUsername.trim() || !vrchatPassword}
                    class={`bg-[#1e5854] hover:bg-[#164743] disabled:opacity-50 text-white border border-[#164743] py-2 px-3 rounded-xl font-bold text-xs shadow-sm self-start ${vrchatStatus?.loggedIn || vrchatStatus?.requiresEmail2fa || vrchatStatusLoading || vrchatLoginLoading ? "cursor-not-allowed" : ""}`}
                >
                    {vrchatLoginLoading ? "ログイン中..." : "ログイン"}
                </button>
            </div>

            {#if vrchatStatus?.requiresEmail2fa}
                <div class="flex flex-col gap-3">
                    <p class="text-xs text-zinc-600 bg-amber-50 border border-amber-200 rounded-xl px-3 py-2">
                        {vrchatStatus.message ?? "email 2FA認証が必要です。"}
                    </p>
                    <label class="block">
                        <span class="text-xs font-bold text-zinc-600 block">認証コード（6桁）</span>
                        <input
                            type="text"
                            inputmode="numeric"
                            maxlength={6}
                            bind:value={vrchatOtpCode}
                            placeholder="123456"
                            class="w-full bg-white border border-zinc-200 rounded-xl p-3 text-sm focus:outline-none focus:border-[#1e5854] mt-1.5 text-zinc-700 font-bold"
                        />
                    </label>
                    <button
                        onclick={handleVrchat2fa}
                        disabled={vrchat2faLoading || !vrchatOtpCode.trim()}
                        class="bg-[#1e5854] hover:bg-[#164743] disabled:opacity-50 text-white border border-[#164743] py-2 px-3 rounded-xl font-bold text-xs shadow-sm self-start"
                    >
                        {vrchat2faLoading ? "確認中..." : "2FA確認"}
                    </button>
                </div>
            {/if}

            <div class="flex gap-2 flex-wrap">
                <button
                    onclick={reloadVrchatStatus}
                    disabled={vrchatStatusLoading}
                    class="bg-white hover:bg-neutral-50 disabled:opacity-50 text-zinc-700 border border-zinc-300 py-2 px-3 rounded-xl font-bold text-xs shadow-sm"
                >
                    認証状態を再確認
                </button>
                {#if vrchatStatus?.loggedIn || vrchatStatus?.requiresEmail2fa}
                    <button
                        onclick={handleVrchatClearLogin}
                        disabled={vrchatClearLoading}
                        class="bg-rose-600 hover:bg-rose-700 disabled:opacity-50 text-white border border-rose-700 py-2 px-3 rounded-xl font-bold text-xs shadow-sm"
                    >
                        {vrchatClearLoading ? "処理中..." : "VRChatログインデータを破棄"}
                    </button>
                {/if}
            </div>

            {#if vrchatError}
                <p class="text-xs font-bold text-rose-700 bg-rose-50 border border-rose-100 rounded-xl px-3 py-2">
                    {vrchatError}
                </p>
            {/if}
        </section>

        <section
            class="bg-neutral-50/70 border border-zinc-200 rounded-2xl p-5 space-y-5"
        >
            <h3
                class="text-xs font-black text-zinc-400 tracking-wider uppercase"
            >
                データ管理
            </h3>
            <p class="text-xs text-zinc-500">
                ローカルDBに保存された訪問履歴を削除できます。画像URLキャッシュは削除しません。
            </p>
            <div class="flex flex-col gap-3">
                <button
                    onclick={handleDeleteAllHistory}
                    disabled={isDeletingAllHistory}
                    class="bg-rose-600 hover:bg-rose-700 disabled:opacity-50 disabled:cursor-not-allowed text-white border border-rose-700 py-2 px-4 rounded-xl font-bold text-xs flex items-center justify-center gap-2 shadow-sm w-fit"
                >
                    {isDeletingAllHistory ? "削除中..." : "すべての訪問履歴を削除"}
                </button>
                {#if deleteAllHistoryResult !== null}
                    <p class="text-xs font-bold text-emerald-700 bg-emerald-50 border border-emerald-100 rounded-xl px-3 py-2">
                        訪問履歴を {deleteAllHistoryResult} 件削除しました。
                    </p>
                {/if}
                {#if deleteAllHistoryError !== null}
                    <p class="text-xs font-bold text-rose-700 bg-rose-50 border border-rose-100 rounded-xl px-3 py-2">
                        {deleteAllHistoryError}
                    </p>
                {/if}
            </div>
        </section>

        {#if settingsSaveState === "success"}
            <p
                class="text-xs font-bold text-emerald-700 bg-emerald-50 border border-emerald-100 rounded-xl px-3 py-2"
            >
                {settingsSaveMessage ?? "設定を保存しました。"}
            </p>
        {:else if settingsSaveState === "error"}
            <div class="space-y-2">
                <p
                    class="text-xs font-bold text-rose-700 bg-rose-50 border border-rose-100 rounded-xl px-3 py-2"
                >
                    {settingsSaveMessage ?? settingsSaveError}
                </p>
                {#if settingsSaveDetails}
                    <details class="text-xs text-zinc-600 bg-zinc-50 border border-zinc-200 rounded-xl px-3 py-2 whitespace-pre-wrap">
                        <summary class="font-bold cursor-pointer">詳細エラー</summary>
                        <div class="mt-2">{settingsSaveDetails}</div>
                    </details>
                {/if}
            </div>
        {/if}
    {/if}
</section>
