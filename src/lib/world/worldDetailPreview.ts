import type { VrchatWorldDetailDto } from "../api/commands";
import type { VisitRecord } from "../data/visitTypes";

export type WorldDetailSource = "local" | "api";

export type WorldDetailPreviewFallback = {
    worldId?: string | null;
    worldName: string;
};

export type WorldDetailPreview = {
    worldId: string | null;
    worldName: string;
    authorName: string;
    description: string;
    thumbnailUrl: string | null;
    imageUrl: string | null;
    capacity: number | null;
    platform: "PC" | "Quest" | "PC/Quest" | "Unknown";
    source: WorldDetailSource;
    visits?: number | null;
    favorites?: number | null;
    occupants?: number | null;
    releaseStatus?: string | null;
    tags?: string[];
};

export function loadWorldDetailPreview(
    record: VisitRecord,
): WorldDetailPreview {
    return {
        worldId: record.worldId ?? null,
        worldName: record.worldName,
        authorName: "",
        description: "このワールドの詳細情報はまだ取得できていません。",
        thumbnailUrl: null,
        imageUrl: null,
        capacity: null,
        platform: "Unknown",
        source: "local",
    };
}

export function mapVrchatWorldDetailToPreview(
    record: WorldDetailPreviewFallback,
    apiDetail: VrchatWorldDetailDto,
): WorldDetailPreview {
    return {
        worldId: apiDetail.worldId,
        worldName: apiDetail.name || record.worldName,
        authorName: apiDetail.authorName ?? "Unknown Author",
        description:
            apiDetail.description?.trim() ||
            "このワールドには説明文が設定されていません。",
        thumbnailUrl: apiDetail.thumbnailImageUrl ?? apiDetail.imageUrl ?? null,
        imageUrl: apiDetail.imageUrl ?? null,
        capacity: apiDetail.recommendedCapacity ?? apiDetail.capacity ?? null,
        platform: mapPlatforms(apiDetail.platforms),
        source: "api",
        visits: apiDetail.visits ?? null,
        favorites: apiDetail.favorites ?? null,
        occupants: apiDetail.occupants ?? null,
        releaseStatus: apiDetail.releaseStatus ?? null,
        tags: apiDetail.tags ?? [],
    };
}

function mapPlatforms(platforms: string[]): WorldDetailPreview["platform"] {
    const lower = platforms.map((platform) => platform.toLowerCase());
    const hasPc = lower.some((platform) =>
        platform.includes("standalonewindows"),
    );
    const hasQuest = lower.some((platform) => platform.includes("android"));
    if (hasPc && hasQuest) return "PC/Quest";
    if (hasPc) return "PC";
    if (hasQuest) return "Quest";
    return "Unknown";
}
