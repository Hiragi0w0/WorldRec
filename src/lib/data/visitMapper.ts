import type { VisitHistoryDto } from "../api/commands";
import type {
  InstanceType,
  TapeStyle,
  VisitRecord as DisplayVisitRecord,
} from "./visitTypes";
import type {
  AccessType,
  DateRailItem,
  DailyVisitGroup,
  ThumbnailTone,
  VisitRecord,
} from "./worldrec";

const DEFAULT_TAPE_COLORS: TapeStyle[] = ["kraft", "mint", "lavender", "pink"];

const thumbnailTones: ThumbnailTone[] = [
  "sunset",
  "warm-house",
  "forest",
  "neon",
  "sky",
  "night",
  "aurora",
  "cafe",
  "lantern",
];

export function mapVisitHistoryToDisplayRecord(
  visit: VisitHistoryDto,
  options: { isCurrentVisit?: boolean } = {},
): DisplayVisitRecord {
  const visitedAt = parseVisitDate(visit.visited_at);
  const worldName = visit.world_name || "Unknown World";
  const worldId = visit.world_id?.trim() || stableWorldId(worldName);

  return {
    id: `db_${visit.id}`,
    worldId,
    worldName,
    thumbnailUrl: visit.thumbnail_url,
    imageUrl: visit.image_url,
    worldPreviewFetchedAt: visit.world_preview_fetched_at,
    time: formatTime(visitedAt, visit.visited_at),
    dateKey: formatDateKey(visit.visited_at),
    instanceType: mapDisplayInstanceType(visit.instance_access_type),
    stayMinutes: secondsToRoundedMinutes(visit.stay_duration_seconds),
    staySeconds: visit.stay_duration_seconds,
    stayLabel: formatStayDuration(visit.stay_duration_seconds, {
      isCurrentVisit: options.isCurrentVisit,
    }),
    memo: visit.memo?.trim() ?? "",
    stamp: firstTag(visit.tags),
    tapeColor:
      DEFAULT_TAPE_COLORS[
        Math.abs(hashString(worldId)) % DEFAULT_TAPE_COLORS.length
      ],
    instanceId: visit.instance_id ?? null,
    visitId: visit.id,
  };
}

export function mapVisitHistoryToRecord(visit: VisitHistoryDto): VisitRecord {
  return {
    id: visit.id,
    dateKey: formatDateKey(visit.visited_at),
    visitedAt: formatVisitedTime(visit.visited_at),
    worldName: visit.world_name,
    worldId: visit.world_id ?? "",
    accessType: mapAccessType(visit.instance_access_type),
    stayDuration: formatStayDuration(visit.stay_duration_seconds),
    tags: splitTags(visit.tags),
    favorite: false,
    thumbnailTone: selectThumbnailTone(visit.world_name),
    thumbnailLabel: buildThumbnailLabel(visit.world_name),
  };
}

export function buildDailyVisitGroups(
  visits: VisitRecord[],
): DailyVisitGroup[] {
  const groups = new Map<string, VisitRecord[]>();

  for (const visit of visits) {
    groups.set(visit.dateKey, [...(groups.get(visit.dateKey) ?? []), visit]);
  }

  return [...groups.entries()]
    .sort(([leftDateKey], [rightDateKey]) =>
      leftDateKey.localeCompare(rightDateKey),
    )
    .map(([dateKey, groupedVisits]) => ({
      dateKey,
      label: formatDateLabel(dateKey),
      visits: groupedVisits,
    }));
}

export function buildDateRailItems(groups: DailyVisitGroup[]): DateRailItem[] {
  return groups.map((group) => ({
    dateKey: group.dateKey,
    label: group.label,
    hasActivity: group.visits.length > 0,
    visitCount: group.visits.length,
  }));
}

export function dateDisplayLabel(dateKey: string) {
  const date = parseDateKey(dateKey);
  if (Number.isNaN(date.getTime())) return dateKey;

  const weekdays = ["日", "月", "火", "水", "木", "金", "土"];
  return `${date.getMonth() + 1}/${date.getDate()}(${weekdays[date.getDay()]})`;
}

export function formatStayDuration(
  seconds: number | null,
  options: { isCurrentVisit?: boolean } = {},
): string {
  if (seconds === null) {
    return options.isCurrentVisit ? "滞在中" : "忘れた";
  }

  if (!Number.isFinite(seconds)) {
    return "忘れた";
  }

  if (seconds <= 0) {
    return "1分未満";
  }

  if (seconds < 60) {
    return "1分未満";
  }

  const minutes = Math.floor(seconds / 60);
  const hours = Math.floor(minutes / 60);
  const remainingMinutes = minutes % 60;

  if (hours > 0 && remainingMinutes > 0)
    return `${hours}時間${remainingMinutes}分`;
  if (hours > 0) return `${hours}時間`;
  return `${minutes}分`;
}

function parseVisitDate(value: string) {
  const normalized = value.includes("T") ? value : value.replace(" ", "T");
  return new Date(normalized);
}

function parseDateKey(dateKey: string) {
  if (/^\d{4}-\d{2}-\d{2}$/.test(dateKey))
    return new Date(`${dateKey}T00:00:00`);

  return new Date(Number.NaN);
}

function formatDateKey(value: string): string {
  const date = parseVisitDate(value);

  if (!Number.isNaN(date.getTime())) {
    const year = date.getFullYear();
    const month = String(date.getMonth() + 1).padStart(2, "0");
    const day = String(date.getDate()).padStart(2, "0");
    return `${year}-${month}-${day}`;
  }

  return value.slice(0, 10) || "unknown";
}

function formatDateLabel(dateKey: string): string {
  const date = new Date(`${dateKey}T00:00:00`);

  if (Number.isNaN(date.getTime())) {
    return "日付未設定";
  }

  return new Intl.DateTimeFormat("ja-JP", {
    month: "numeric",
    day: "numeric",
    weekday: "short",
  }).format(date);
}

function formatVisitedTime(value: string): string {
  const date = parseVisitDate(value);

  if (!Number.isNaN(date.getTime())) {
    return new Intl.DateTimeFormat("ja-JP", {
      hour: "2-digit",
      minute: "2-digit",
      hour12: false,
    }).format(date);
  }

  const match = value.match(/\b(\d{2}):(\d{2})/);
  return match ? `${match[1]}:${match[2]}` : "--:--";
}

function formatTime(date: Date, fallback: string) {
  if (Number.isNaN(date.getTime())) {
    const match = fallback.match(/\b(\d{2}):(\d{2})/);
    return match ? `${match[1]}:${match[2]}` : "--:--";
  }

  return `${date.getHours().toString().padStart(2, "0")}:${date.getMinutes().toString().padStart(2, "0")}`;
}

function mapAccessType(value: string | null): AccessType {
  switch (value?.toLowerCase()) {
    case "public":
      return "Public";
    case "friends":
      return "Friends";
    case "hidden":
      return "Friends+";
    case "private":
    case "invite":
      return "Private";
    default:
      return "Unknown";
  }
}

function mapDisplayInstanceType(value: string | null): InstanceType {
  const normalized = value?.trim().toUpperCase();
  if (normalized === "PUBLIC") return "PUBLIC";
  if (normalized === "FRIENDS" || normalized === "FRIENDS+") return "FRIENDS";
  if (normalized === "HIDDEN") return "FRIENDS";
  if (normalized === "PRIVATE") return "INVITE+";
  if (
    normalized === "INVITE" ||
    normalized === "INVITE+" ||
    normalized === "INVITEPLUS"
  )
    return "INVITE+";
  if (normalized === "GROUP" || normalized === "GROUP+") return "GROUP";
  return "UNKNOWN";
}

function secondsToRoundedMinutes(value: number | null): number {
  if (value === null || !Number.isFinite(value)) {
    return 0;
  }

  if (value <= 0) {
    return 1;
  }

  if (value < 60) {
    return 1;
  }

  return Math.max(0, Math.floor(value / 60));
}

function splitTags(value: string | null): string[] {
  if (!value?.trim()) {
    return [];
  }

  return value
    .split(/[,、\s]+/)
    .map((tag) => tag.trim())
    .filter(Boolean);
}

function firstTag(value: string | null) {
  return splitTags(value).find(Boolean);
}

function stableWorldId(worldName: string) {
  return `world_${
    worldName
      .trim()
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "_")
      .replace(/^_+|_+$/g, "") || "unknown"
  }`;
}

function hashString(value: string) {
  let hash = 0;
  for (let index = 0; index < value.length; index += 1) {
    hash = (hash << 5) - hash + value.charCodeAt(index);
    hash |= 0;
  }
  return hash;
}

function selectThumbnailTone(worldName: string): ThumbnailTone {
  const index = [...worldName].reduce(
    (total, character) => total + character.charCodeAt(0),
    0,
  );

  return thumbnailTones[index % thumbnailTones.length];
}

function buildThumbnailLabel(worldName: string): string {
  const label = [...worldName.trim()].slice(0, 2).join("");

  return label || "VR";
}
