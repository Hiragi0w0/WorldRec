export type Screen = "record" | "library" | "ai_guide" | "stats" | "settings";
export type Category = "cozy" | "cyberpunk" | "scenic" | "game" | "music";
export type InstanceType = "PUBLIC" | "FRIENDS" | "INVITE+" | "GROUP" | "UNKNOWN";
export type TapeStyle = "kraft" | "mint" | "lavender" | "pink";
export type PaperStyle = "dotted" | "lined" | "blank";
export type ViewFormat = "list" | "grid";

export interface VRCWorld {
    id: string;
    name: string;
    author: string;
    category: Category;
    description: string;
    bgGradient: string;
    tags: string[];
    capacity: number;
}

export interface VisitRecord {
    id: string;
    worldId: string;
    worldName: string;
    thumbnailUrl?: string | null;
    imageUrl?: string | null;
    worldPreviewFetchedAt?: string | null;
    time: string;
    dateKey: string;
    instanceType: InstanceType;
    stayMinutes: number;
    staySeconds?: number | null;
    stayLabel?: string;
    memo: string;
    stamp?: string;
    tapeColor?: TapeStyle;
    instanceId?: string | null;
    visitId?: number;
}

export interface DateListItem {
    key: string;
    display: string;
}
