<script lang="ts">
    import {
        AlertTriangle,
        Check,
        ChevronLeft,
        ChevronRight,
        FolderOpen,
        Sparkles,
    } from "lucide-svelte";
    import type { AppSettings, RuntimeStatusDto } from "../../api/commands";

    let {
        settings,
        runtimeStatus,
        onOpenSettings,
        onComplete,
    }: {
        settings: AppSettings;
        runtimeStatus: RuntimeStatusDto | null;
        onOpenSettings: () => void;
        onComplete: () => Promise<void> | void;
    } = $props();

    type Step = {
        eyebrow: string;
        title: string;
        description: string;
    };

    const steps: Step[] = [
        {
            eyebrow: "1 / 3",
            title: "WorldRec が記録する内容",
            description:
                "VRChat のログファイルを読み取り、あとから訪問記録を振り返るための履歴をローカルに保存します。",
        },
        {
            eyebrow: "2 / 3",
            title: "VRChat ログフォルダ確認",
            description:
                "WorldRec が参照するログフォルダと、現在のログ監視状態を確認します。",
        },
        {
            eyebrow: "3 / 3",
            title: "AI 探索と外部送信",
            description:
                "AI 探索は任意機能です。使う前に、外部 AI へ送信される可能性がある情報を確認してください。",
        },
    ];

    let currentStep = $state(0);
    let isCompleting = $state(false);
    let stepViewKey = $state(0);

    let step = $derived(steps[currentStep]);
    let rawLogDir = $derived(runtimeStatus?.log_dir ?? settings.log_dir);
    let logDir = $derived(rawLogDir.trim());
    let watcherLabel = $derived(
        runtimeStatus?.watcher_running
            ? "ログ監視中"
            : "ログ監視はまだ開始されていません",
    );

    function goToStep(nextStep: number) {
        currentStep = Math.min(steps.length - 1, Math.max(0, nextStep));
        stepViewKey += 1;
    }

    async function completeOnboarding() {
        if (isCompleting) return;

        isCompleting = true;
        try {
            await onComplete();
        } finally {
            isCompleting = false;
        }
    }
</script>

<div
    class="fixed inset-0 z-[80] flex items-center justify-center bg-zinc-950/35 px-4 py-6 backdrop-blur-[2px] animate-fadeIn"
    role="presentation"
>
    <div
        class="animate-onboarding-panel w-full max-w-2xl overflow-hidden rounded-3xl border border-[#DEDAC4] bg-[#FAF9F5] shadow-2xl"
        role="dialog"
        aria-modal="true"
        aria-labelledby="onboarding-title"
    >
        <div class="border-b border-[#E4DEC9] bg-white/80 px-5 py-4 sm:px-7">
            <div class="flex items-center justify-between gap-4">
                <div>
                    <p
                        class="text-[10px] font-black uppercase tracking-[0.22em] text-[#1e5854]"
                    >
                        First Setup
                    </p>
                    <h2
                        id="onboarding-title"
                        class="mt-1 text-xl font-black tracking-tight text-zinc-800 sm:text-2xl"
                    >
                        WorldRec を始める前に
                    </h2>
                </div>
                <span
                    class="shrink-0 rounded-full border border-[#D9CBA8] bg-[#FCFAF7] px-3 py-1 text-xs font-black text-[#7E6941]"
                >
                    {step.eyebrow}
                </span>
            </div>

            <div class="mt-4 grid grid-cols-3 gap-2" aria-hidden="true">
                {#each steps as _, index}
                    <span
                        class={`h-1.5 rounded-full transition-colors duration-200 ${index <= currentStep ? "bg-[#1e5854]" : "bg-[#E7E1D0]"}`}
                    ></span>
                {/each}
            </div>
        </div>

        {#key stepViewKey}
            <div class="animate-onboarding-step px-5 py-6 sm:px-7">
                <p
                    class="text-[10px] font-black uppercase tracking-[0.2em] text-[#A68F63]"
                >
                    {step.eyebrow}
                </p>
                <h3 class="mt-2 text-2xl font-black tracking-tight text-zinc-800">
                    {step.title}
                </h3>
                <p class="mt-2 text-sm leading-7 text-zinc-600">
                    {step.description}
                </p>

                {#if currentStep === 0}
                    <div class="mt-5 rounded-2xl border border-[#E4DEC9] bg-white p-4">
                        <p class="text-sm font-bold leading-7 text-zinc-700">
                            記録する主な情報
                        </p>
                        <div class="mt-3 grid grid-cols-1 gap-2 sm:grid-cols-2">
                            {#each ["訪問日時", "ワールド名", "world_id", "instance_id", "インスタンス種別", "滞在時間", "メモ", "タグ"] as item}
                                <div
                                    class="flex items-center gap-2 rounded-xl border border-[#EFE8D7] bg-[#FCFAF7] px-3 py-2 text-sm font-semibold text-zinc-700"
                                >
                                    <Check class="h-4 w-4 shrink-0 text-[#1e5854]" />
                                    {item}
                                </div>
                            {/each}
                        </div>
                        <p class="mt-4 text-xs leading-6 text-zinc-500">
                            これらは訪問履歴の整理と振り返りのために使われます。VRChat ログインや過去ログ取り込みは、この初回導線では行いません。
                        </p>
                    </div>
                {:else if currentStep === 1}
                    <div class="mt-5 space-y-3">
                        <div
                            class="rounded-2xl border border-[#E4DEC9] bg-white p-4"
                        >
                            <div class="flex items-start gap-3">
                                <FolderOpen
                                    class="mt-0.5 h-5 w-5 shrink-0 text-[#1e5854]"
                                />
                                <div class="min-w-0">
                                    <p class="text-xs font-black uppercase tracking-widest text-zinc-400">
                                        Log Folder
                                    </p>
                                    {#if logDir}
                                        <p
                                            class="mt-1 break-all font-mono text-xs font-bold text-zinc-700"
                                            title={logDir}
                                        >
                                            {logDir}
                                        </p>
                                    {:else}
                                        <p class="mt-1 text-sm font-semibold text-zinc-600">
                                            ログフォルダを確認できませんでした。設定画面で確認してください。
                                        </p>
                                    {/if}
                                </div>
                            </div>
                        </div>

                        <div
                            class="rounded-2xl border border-[#E4DEC9] bg-white p-4"
                        >
                            <p class="text-xs font-black uppercase tracking-widest text-zinc-400">
                                Watcher Status
                            </p>
                            <p class="mt-1 text-sm font-bold text-zinc-700">
                                {watcherLabel}
                            </p>
                            {#if runtimeStatus?.watcher_last_error}
                                <div
                                    class="mt-3 flex items-start gap-2 rounded-xl border border-rose-200 bg-rose-50 px-3 py-2 text-xs font-bold text-rose-700"
                                >
                                    <AlertTriangle class="mt-0.5 h-4 w-4 shrink-0" />
                                    <span>{runtimeStatus.watcher_last_error}</span>
                                </div>
                            {/if}
                        </div>
                    </div>
                {:else}
                    <div class="mt-5 rounded-2xl border border-[#E4DEC9] bg-white p-4">
                        <div class="flex items-start gap-3">
                            <Sparkles class="mt-0.5 h-5 w-5 shrink-0 text-[#1e5854]" />
                            <div class="space-y-3 text-sm leading-7 text-zinc-700">
                                <p>
                                    AI探索を使うと、あなたが入力した相談内容、これまでの会話内容、訪問済みワールドの名前・ワールドID・訪問回数が
                                    Gemini に送信されます。
                                </p>
                                <p>
                                    メモ、タグ、ログファイル名、インスタンスID、滞在時間、DB保存先、VRChatログフォルダは送信しません。
                                </p>
                                <p>
                                    回答の作成には Gemini API と Google検索を使用します。
                                </p>
                            </div>
                        </div>
                    </div>
                {/if}
            </div>
        {/key}

        <div
            class="flex flex-col-reverse gap-3 border-t border-[#E4DEC9] bg-white/75 px-5 py-4 sm:flex-row sm:items-center sm:justify-between sm:px-7"
        >
            <div class="flex gap-2">
                {#if currentStep > 0}
                    <button
                        type="button"
                        onclick={() => goToStep(currentStep - 1)}
                        class="inline-flex items-center justify-center gap-1.5 rounded-xl border border-[#C5BFAB] bg-white px-4 py-2 text-xs font-bold text-zinc-600 shadow-sm transition-colors hover:bg-[#FAF9F5]"
                    >
                        <ChevronLeft class="h-4 w-4" />
                        戻る
                    </button>
                {/if}
                {#if currentStep === 1}
                    <button
                        type="button"
                        onclick={onOpenSettings}
                        class="inline-flex items-center justify-center rounded-xl border border-[#C5BFAB] bg-white px-4 py-2 text-xs font-bold text-zinc-700 shadow-sm transition-colors hover:bg-[#FAF9F5]"
                    >
                        設定を開く
                    </button>
                {/if}
            </div>

            {#if currentStep < steps.length - 1}
                <button
                    type="button"
                    onclick={() => goToStep(currentStep + 1)}
                    class="inline-flex items-center justify-center gap-1.5 rounded-xl border border-[#164340] bg-[#1e5854] px-5 py-2.5 text-xs font-extrabold text-white shadow-md transition-colors hover:bg-[#133c39]"
                >
                    次へ
                    <ChevronRight class="h-4 w-4" />
                </button>
            {:else}
                <button
                    type="button"
                    onclick={() => void completeOnboarding()}
                    disabled={isCompleting}
                    class="inline-flex items-center justify-center rounded-xl border border-[#164340] bg-[#1e5854] px-5 py-2.5 text-xs font-extrabold text-white shadow-md transition-colors hover:bg-[#133c39] disabled:cursor-not-allowed disabled:bg-zinc-300 disabled:border-zinc-300"
                >
                    {isCompleting ? "保存しています..." : "WorldRecを始める"}
                </button>
            {/if}
        </div>
    </div>
</div>
