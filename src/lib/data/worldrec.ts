export type AccessType = "Public" | "Friends" | "Friends+" | "Private" | "Unknown";

export type ThumbnailTone =
  | "sunset"
  | "warm-house"
  | "forest"
  | "neon"
  | "sky"
  | "night"
  | "aurora"
  | "cafe"
  | "lantern";

export type VisitRecord = {
  id: number;
  dateKey: string;
  visitedAt: string;
  worldName: string;
  worldId: string;
  accessType: AccessType;
  stayDuration: string;
  tags: string[];
  favorite: boolean;
  thumbnailTone: ThumbnailTone;
  thumbnailLabel: string;
};

export type RecommendationRecord = {
  id: number;
  name: string;
  description: string;
  tags: string[];
  thumbnailTone: Exclude<ThumbnailTone, "sunset" | "warm-house" | "forest" | "neon" | "sky"> | "night" | "aurora" | "cafe" | "lantern";
};

export type DateRailItem = {
  dateKey: string;
  label: string;
  hasActivity: boolean;
  visitCount: number;
};

export type DailyVisitGroup = {
  dateKey: string;
  label: string;
  visits: VisitRecord[];
};
