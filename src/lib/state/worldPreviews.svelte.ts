import { untrack } from "svelte";
import { getVrchatWorldDetail } from "../api/commands";
import {
    mapVrchatWorldDetailToPreview,
    type WorldDetailPreview,
} from "../world/worldDetailPreview";

let previewCache = $state(new Map<string, WorldDetailPreview | null>());
const pendingPreviewIds = new Set<string>();
const WORLD_PREVIEW_SUCCESS_TTL_MS = 30 * 24 * 60 * 60 * 1000;

export type WorldPreviewFallback = {
    worldId?: string | null;
    worldName: string;
};

export function isVrchatWorldId(
    worldId: string | null | undefined,
): worldId is string {
    return typeof worldId === "string" && worldId.startsWith("wrld_");
}

export function getCachedWorldPreview(
    worldId: string | null | undefined,
): WorldDetailPreview | null {
    if (!isVrchatWorldId(worldId)) return null;
    return previewCache.get(worldId) ?? null;
}

export async function loadWorldPreview(
    worldId: string | null | undefined,
    fallback: WorldPreviewFallback,
): Promise<WorldDetailPreview | null> {
    if (!isVrchatWorldId(worldId)) return null;

    const gate = untrack(() => {
        if (previewCache.has(worldId) || pendingPreviewIds.has(worldId)) {
            return { shouldFetch: false, cached: previewCache.get(worldId) ?? null };
        }
        pendingPreviewIds.add(worldId);
        return { shouldFetch: true, cached: null };
    });

    if (!gate.shouldFetch) {
        return gate.cached;
    }

    try {
        const apiDetail = await getVrchatWorldDetail(
            worldId,
            fallback.worldName,
        );
        const preview = mapVrchatWorldDetailToPreview(fallback, apiDetail);
        previewCache = new Map(previewCache).set(worldId, preview);
        return preview;
    } catch (error) {
        console.warn(
            `get_vrchat_world_detail failed for world preview ${worldId}: ${toErrorMessage(error)}`,
        );
        return null;
    } finally {
        pendingPreviewIds.delete(worldId);
    }
}

export async function loadWorldPreviews<T>(
    items: T[],
    getWorldId: (item: T) => string | null | undefined,
    getFallback: (item: T) => WorldPreviewFallback,
    concurrency = 3,
    getExistingPreview?: (item: T) => {
        imageUrl: string | null | undefined;
        fetchedAt: string | null | undefined;
    } | null,
): Promise<void> {
    const queue: Array<{ worldId: string; fallback: WorldPreviewFallback }> = [];
    const queuedWorldIds = new Set<string>();

    for (const item of items) {
        const worldId = getWorldId(item);
        if (!isVrchatWorldId(worldId)) continue;
        if (queuedWorldIds.has(worldId)) continue;

        const alreadyHandled = untrack(
            () => previewCache.has(worldId) || pendingPreviewIds.has(worldId),
        );
        if (alreadyHandled) continue;

        const existingPreview = getExistingPreview?.(item);
        if (isFreshExistingPreview(existingPreview)) continue;

        queuedWorldIds.add(worldId);
        queue.push({ worldId, fallback: getFallback(item) });
    }

    const workerCount = Math.max(1, Math.min(concurrency, queue.length));
    let nextIndex = 0;

    async function worker() {
        while (nextIndex < queue.length) {
            const entry = queue[nextIndex];
            nextIndex += 1;
            await loadWorldPreview(entry.worldId, entry.fallback);
        }
    }

    await Promise.all(Array.from({ length: workerCount }, worker));
}

function isFreshExistingPreview(
    preview:
        | {
              imageUrl: string | null | undefined;
              fetchedAt: string | null | undefined;
          }
        | null
        | undefined,
) {
    if (!preview?.imageUrl || !preview.fetchedAt) return false;

    const fetchedTime = new Date(preview.fetchedAt).getTime();
    if (Number.isNaN(fetchedTime)) return false;

    return Date.now() - fetchedTime <= WORLD_PREVIEW_SUCCESS_TTL_MS;
}

function toErrorMessage(error: unknown) {
    if (error instanceof Error) return error.message;
    if (typeof error === "string") return error;
    return "Tauri command failed.";
}
