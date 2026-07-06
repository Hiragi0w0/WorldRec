<script lang="ts">
    import { onMount } from "svelte";
    import {
        Check,
        Compass,
        Copy,
        KeyRound,
        MapPin,
        RotateCw,
        Send,
        Sparkles,
        Users,
    } from "lucide-svelte";
    import type {
        AiNewWorld,
        AiStatus,
        AiVisitedWorld,
        AppSettings,
    } from "../../api/commands";
    import { getAiStatus, getSettings, recommendWorlds } from "../../api/commands";

    export let onOpenSettings: () => void;
    export let onOpenVisitedWorldDetail: (
        worldId: string | null,
        worldName: string,
    ) => void;

    type ChatMessage = {
        role: "user" | "assistant";
        text: string;
        visitedWorlds?: AiVisitedWorld[];
        newWorlds?: AiNewWorld[];
        warning?: string;
    };

    const MAX_HISTORY_MESSAGES = 10;

    let aiStatus: AiStatus | null = null;
    let aiSettings: AppSettings | null = null;
    let aiStatusLoading = true;
    let messages: ChatMessage[] = [];
    let inputText = "";
    let isGenerating = false;
    let errorMessage: string | null = null;
    let copiedWorldName: string | null = null;
    let copyErrorMessage: string | null = null;
    let messageListElement: HTMLDivElement | null = null;

    const examplePrompts = [
        "静かにのんびりできるワールドを教えて",
        "フレンド4人で遊べるゲームワールドがいい",
        "まだ行ったことのない綺麗な景色のワールドを開拓したい",
    ];

    onMount(async () => {
        try {
            const [status, settings] = await Promise.all([
                getAiStatus(),
                getSettings(),
            ]);
            aiStatus = status;
            aiSettings = settings;
        } catch (error) {
            errorMessage = toErrorMessage(error);
        } finally {
            aiStatusLoading = false;
        }
    });

    function buildQueryHistory(history: ChatMessage[]): string {
        return history
            .slice(-MAX_HISTORY_MESSAGES)
            .map(
                (message) =>
                    `${message.role === "user" ? "ユーザー" : "AI"}: ${message.text}`,
            )
            .join("\n\n");
    }

    async function handleSend() {
        const query = inputText.trim();
        if (!query || isGenerating) return;

        const queryHistory = buildQueryHistory(messages);
        messages = [...messages, { role: "user", text: query }];
        inputText = "";
        isGenerating = true;
        errorMessage = null;
        scrollToLatest();

        try {
            const result = await recommendWorlds(query, queryHistory);
            if (result.source === "ai" || result.source === "ai_degraded") {
                messages = [
                    ...messages,
                    {
                        role: "assistant",
                        text: result.text,
                        visitedWorlds:
                            result.recommendation_mode === "unvisited_only"
                                ? []
                                : result.visited_worlds.filter(
                                      (world) => world.matched,
                                  ),
                        newWorlds: result.new_worlds,
                        ...(result.source === "ai_degraded" && result.warning
                            ? { warning: result.warning }
                            : {}),
                    },
                ];
            } else {
                errorMessage =
                    result.error_message ??
                    "AI推薦の生成中に問題が発生しました。\n時間をおいて再度お試しください。";
            }
        } catch (error) {
            errorMessage = toErrorMessage(error);
        } finally {
            isGenerating = false;
            scrollToLatest();
        }
    }

    function handleKeydown(event: KeyboardEvent) {
        if (event.key === "Enter" && !event.shiftKey && !event.isComposing) {
            event.preventDefault();
            void handleSend();
        }
    }

    function scrollToLatest() {
        requestAnimationFrame(() => {
            messageListElement?.scrollTo({
                top: messageListElement.scrollHeight,
                behavior: "smooth",
            });
        });
    }

    function toErrorMessage(error: unknown) {
        if (error instanceof Error) return error.message;
        if (typeof error === "string") return error;
        return "AIリクエストに失敗しました。";
    }

    function visitCountLabel(count: number) {
        return count >= 10 ? "よく訪れている" : `${count}回訪問`;
    }

    async function copyWorldName(worldName: string) {
        try {
            await navigator.clipboard.writeText(worldName);
            copiedWorldName = worldName;
            copyErrorMessage = null;
            window.setTimeout(() => {
                if (copiedWorldName === worldName) copiedWorldName = null;
            }, 2000);
        } catch {
            copyErrorMessage = "コピーに失敗しました。";
        }
    }
</script>

<section class="space-y-6 animate-fadeIn">
    <div
        class="border border-[#DFCFA9] p-8 rounded-3xl relative overflow-hidden bg-white"
    >
        <div
            class="absolute top-4 right-4 text-xs font-handwritten text-[#A68F63] border border-[#DFCFA9] rotate-6 px-3 py-1 rounded bg-[#FCFAF7] shadow-sm select-none"
        >
            AI探索ガイド
        </div>
        <div class="max-w-2xl">
            <span
                class="text-[10px] bg-amber-100 border border-amber-300 text-amber-800 font-black px-2.5 py-1 rounded-sm uppercase tracking-widest"
                >AI TRIP AGENCIES</span
            >
            <h2 class="text-3xl font-black text-zinc-800 tracking-tight mt-3">
                AI ワールドマッチメーカー
            </h2>
            <p class="text-sm text-zinc-600 mt-2 leading-relaxed">
                行きたい場所や気分をチャットで伝えると、あなたの訪問履歴とWeb検索をもとに、AIがベストなVRChatワールドをレコメンドします。
            </p>
        </div>
    </div>

    {#if aiStatusLoading}
        <div
            class="bg-white border border-dashed border-[#DFCFA9] p-10 rounded-2xl flex flex-col items-center"
        >
            <div
                class="w-8 h-8 border-4 border-[#1e5854] border-t-transparent rounded-full animate-spin"
            ></div>
            <p class="text-sm font-bold text-zinc-500 mt-4">
                AI設定を確認しています...
            </p>
        </div>
    {:else if !aiSettings?.ai_enabled}
        <div
            class="bg-amber-50/70 border border-amber-200 p-8 rounded-2xl flex flex-col items-start gap-4"
        >
            <div class="flex items-center gap-2">
                <Sparkles class="w-5 h-5 text-amber-700" />
                <h3 class="text-base font-extrabold text-amber-900">
                    AI探索が無効です
                </h3>
            </div>
            <p class="text-xs text-amber-800 leading-relaxed">
                AI探索ガイドを使うには、設定画面の「AI設定」でAI探索を有効にしてください。
            </p>
            <button
                onclick={onOpenSettings}
                class="bg-[#1e5854] hover:bg-[#164743] text-white border border-[#164743] py-2 px-4 rounded-xl font-bold text-xs shadow-sm"
            >
                設定画面を開く
            </button>
        </div>
    {:else if !aiStatus?.hasGeminiApiKey}
        <div
            class="bg-amber-50/70 border border-amber-200 p-8 rounded-2xl flex flex-col items-start gap-4"
        >
            <div class="flex items-center gap-2">
                <KeyRound class="w-5 h-5 text-amber-700" />
                <h3 class="text-base font-extrabold text-amber-900">
                    Gemini APIキーが未設定です
                </h3>
            </div>
            <p class="text-xs text-amber-800 leading-relaxed">
                AI探索ガイドを使うには、設定画面の「AI設定」からGemini
                APIキーを登録してください。キーはOSの資格情報ストアに暗号化して保存されます。
            </p>
            <button
                onclick={onOpenSettings}
                class="bg-[#1e5854] hover:bg-[#164743] text-white border border-[#164743] py-2 px-4 rounded-xl font-bold text-xs shadow-sm"
            >
                設定画面を開く
            </button>
        </div>
    {:else}
        <div
            class="bg-white border border-zinc-200 rounded-3xl overflow-hidden flex flex-col"
        >
            <div
                bind:this={messageListElement}
                class="p-6 space-y-6 overflow-y-auto max-h-[60vh] min-h-[240px]"
            >
                {#if messages.length === 0}
                    <div
                        class="text-center text-zinc-400 py-10 space-y-5"
                    >
                        <Compass class="w-8 h-8 mx-auto text-[#A68F63]" />
                        <p class="text-sm font-semibold">
                            今日はどんなワールドに行きたいですか？
                        </p>
                        <div class="flex flex-wrap justify-center gap-2">
                            {#each examplePrompts as prompt}
                                <button
                                    onclick={() => {
                                        inputText = prompt;
                                        void handleSend();
                                    }}
                                    class="text-xs font-bold text-[#0f4743] bg-[#1e5854]/5 hover:bg-[#1e5854]/10 border border-[#1e5854]/20 rounded-full px-3 py-1.5 transition-all"
                                >
                                    {prompt}
                                </button>
                            {/each}
                        </div>
                    </div>
                {/if}

                {#each messages as message}
                    {#if message.role === "user"}
                        <div class="flex justify-end">
                            <div
                                class="bg-[#1e5854] text-white text-sm font-semibold rounded-2xl rounded-br-sm px-4 py-3 max-w-[80%] whitespace-pre-wrap leading-relaxed"
                            >
                                {message.text}
                            </div>
                        </div>
                    {:else}
                        <div class="space-y-4">
                            {#if message.warning}
                                <p
                                    class="text-xs font-bold text-amber-800 bg-amber-50 border border-amber-200 rounded-xl px-3 py-2 whitespace-pre-wrap"
                                >
                                    {message.warning}
                                </p>
                            {/if}

                            {#if message.newWorlds && message.newWorlds.length > 0}
                                <div class="space-y-3">
                                    <h4
                                        class="text-[10px] font-black text-[#7E6941] tracking-widest uppercase flex items-center gap-2"
                                    >
                                        <Sparkles class="w-3.5 h-3.5 text-[#1e5854]" />
                                        未訪問のおすすめワールド
                                    </h4>
                                    <div
                                        class="grid grid-cols-1 md:grid-cols-2 gap-3"
                                    >
                                        {#each message.newWorlds as world}
                                            <article
                                                class="bg-white border border-emerald-100 rounded-2xl p-4 space-y-2 shadow-sm"
                                            >
                                                <div
                                                    class="flex items-center justify-between gap-2"
                                                >
                                                    <h5
                                                        class="text-sm font-extrabold text-[#111] leading-snug"
                                                    >
                                                        {world.world_name}
                                                    </h5>
                                                    <div
                                                        class="flex items-center gap-1.5 shrink-0"
                                                    >
                                                        <button
                                                            type="button"
                                                            onclick={() =>
                                                                void copyWorldName(
                                                                    world.world_name,
                                                                )}
                                                            class="text-[9px] bg-white hover:bg-emerald-50 text-emerald-700 border border-emerald-200 font-black px-2 py-0.5 rounded-full flex items-center gap-1 transition-colors"
                                                            aria-label={`${world.world_name}をコピー`}
                                                        >
                                                            {#if copiedWorldName === world.world_name}
                                                                <Check
                                                                    class="w-2.5 h-2.5"
                                                                />
                                                                コピー済み
                                                            {:else}
                                                                <Copy
                                                                    class="w-2.5 h-2.5"
                                                                />
                                                                コピー
                                                            {/if}
                                                        </button>
                                                        <span
                                                            class="text-[9px] bg-emerald-500 text-white font-black px-2 py-0.5 rounded-full"
                                                            >NEW</span
                                                        >
                                                    </div>
                                                </div>
                                                {#if copyErrorMessage}
                                                    <p
                                                        class="text-[10px] font-bold text-rose-600"
                                                    >
                                                        {copyErrorMessage}
                                                    </p>
                                                {/if}
                                                {#if world.overview}
                                                    <p
                                                        class="text-xs text-zinc-500 leading-relaxed"
                                                    >
                                                        {world.overview}
                                                    </p>
                                                {/if}
                                                {#if world.recommended_number_of_people}
                                                    <p
                                                        class="text-[10px] text-zinc-400 font-bold flex items-center gap-1"
                                                    >
                                                        <Users
                                                            class="w-3 h-3"
                                                        />
                                                        推奨人数: {world.recommended_number_of_people}
                                                    </p>
                                                {/if}
                                            </article>
                                        {/each}
                                    </div>
                                </div>
                            {/if}

                            {#if message.visitedWorlds && message.visitedWorlds.length > 0}
                                <div class="space-y-3">
                                    <h4
                                        class="text-[10px] font-black text-[#7E6941] tracking-widest uppercase flex items-center gap-2"
                                    >
                                        <MapPin class="w-3.5 h-3.5 text-[#1e5854]" />
                                        訪問履歴からのおすすめ
                                    </h4>
                                    <div
                                        class="grid grid-cols-1 md:grid-cols-2 gap-3"
                                    >
                                        {#each message.visitedWorlds as world}
                                            <article
                                                class="bg-[#FCFAF7] border border-[#DFCFA9] rounded-2xl p-4 space-y-2 shadow-sm"
                                            >
                                                <div
                                                    class="flex items-center justify-between gap-2"
                                                >
                                                    <button
                                                        type="button"
                                                        onclick={() =>
                                                            onOpenVisitedWorldDetail(
                                                                world.world_id,
                                                                world.world_name,
                                                            )}
                                                        class="text-left text-sm font-extrabold text-[#111] leading-snug hover:text-[#0f4743] hover:underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#1e5854]/30 rounded-sm"
                                                    >
                                                        {world.world_name}
                                                    </button>
                                                    <span
                                                        class="text-[9px] bg-[#1e5854]/10 text-[#0f4743] font-black px-2 py-0.5 rounded-full shrink-0 flex items-center gap-1"
                                                    >
                                                        <MapPin
                                                            class="w-2.5 h-2.5"
                                                        />
                                                        {visitCountLabel(
                                                            world.visit_count,
                                                        )}
                                                    </span>
                                                </div>
                                                {#if world.world_overview}
                                                    <p
                                                        class="text-xs text-zinc-500 leading-relaxed"
                                                    >
                                                        {world.world_overview}
                                                    </p>
                                                {/if}
                                                <div
                                                    class="flex items-center gap-2 flex-wrap"
                                                >
                                                    {#if world.recommendedNumberOfPeople > 0}
                                                        <span
                                                            class="text-[10px] text-zinc-400 font-bold flex items-center gap-1"
                                                        >
                                                            <Users
                                                                class="w-3 h-3"
                                                            />
                                                            推奨人数: {world.recommendedNumberOfPeople}人
                                                        </span>
                                                    {/if}
                                                </div>
                                            </article>
                                        {/each}
                                    </div>
                                </div>
                            {/if}
                        </div>
                    {/if}
                {/each}

                {#if isGenerating}
                    <div class="flex items-center gap-3 pl-1">
                        <div
                            class="w-8 h-8 rounded-full bg-[#1e5854]/10 border border-[#1e5854]/20 flex items-center justify-center shrink-0"
                        >
                            <RotateCw
                                class="w-4 h-4 text-[#1e5854] animate-spin"
                            />
                        </div>
                        <p class="text-xs font-bold text-zinc-500">
                            訪問履歴とWeb検索からベストな旅先を構成しています...（数十秒かかることがあります）
                        </p>
                    </div>
                {/if}

                {#if errorMessage}
                    <p
                        class="text-xs font-bold text-rose-700 bg-rose-50 border border-rose-100 rounded-xl px-3 py-2 whitespace-pre-wrap"
                    >
                        {errorMessage}
                    </p>
                {/if}
            </div>

            <div class="border-t border-zinc-100 p-4 bg-neutral-50/60">
                <div class="flex items-end gap-2">
                    <textarea
                        bind:value={inputText}
                        onkeydown={handleKeydown}
                        rows={2}
                        placeholder="例: フレンドとまったり話せる夜景の綺麗なワールドを探して"
                        disabled={isGenerating}
                        class="flex-1 bg-white border border-zinc-200 rounded-xl p-3 text-sm focus:outline-none focus:border-[#1e5854] text-zinc-700 font-semibold resize-none disabled:opacity-60"
                    ></textarea>
                    <button
                        onclick={() => void handleSend()}
                        disabled={isGenerating || !inputText.trim()}
                        class="bg-[#1e5854] hover:bg-[#133c39] disabled:bg-zinc-300 text-white font-extrabold text-sm py-3 px-5 rounded-xl shadow-md transition-all flex items-center gap-2 shrink-0"
                    >
                        <Send class="w-4 h-4" />
                        送信
                    </button>
                </div>
                <p class="text-[10px] text-zinc-400 mt-2">
                    Enterで送信 / Shift+Enterで改行。相談内容と訪問済みワールドの名前・ワールドID・訪問回数を使い、Gemini APIとGoogle検索で回答します。
                </p>
            </div>
        </div>
    {/if}
</section>
