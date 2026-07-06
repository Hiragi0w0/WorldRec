import type { VisitRecord } from "../data/visitTypes";

export function buildTimelineRecords(records: VisitRecord[], dateKey: string, query: string) {
    const q = query.trim().toLowerCase();

    return records
        .filter((record) => record.dateKey === dateKey)
        .filter((record) => {
            if (!q) return true;

            return (
                record.worldName.toLowerCase().includes(q) ||
                record.memo.toLowerCase().includes(q) ||
                (record.stamp ?? "").toLowerCase().includes(q)
            );
        })
        .sort((a, b) => visitSortValue(b).localeCompare(visitSortValue(a)));
}

export function calculateJourneySummary(records: VisitRecord[], dateKey: string) {
    const totalSeconds = records.reduce(
        (sum, record) => sum + (record.staySeconds ?? record.stayMinutes * 60),
        0,
    );
    const totalMinutes = Math.floor(totalSeconds / 60);
    const selectedDateCount = records.filter((record) => record.dateKey === dateKey).length;

    return {
        selectedDateCount,
        totalCount: records.length,
        totalDifferentWorlds: new Set(records.map((record) => record.worldId)).size,
        timeFormatted: `${Math.floor(totalMinutes / 60)}h ${totalMinutes % 60}m`,
    };
}

export type WorldVisitStats = {
    visitCount: number;
    totalStaySeconds: number;
    firstVisit: VisitRecord | null;
    latestVisit: VisitRecord | null;
};

export function calculateWorldVisitStats(records: VisitRecord[]): WorldVisitStats {
    if (records.length === 0) {
        return {
            visitCount: 0,
            totalStaySeconds: 0,
            firstVisit: null,
            latestVisit: null,
        };
    }

    const totalStaySeconds = records.reduce(
        (sum, record) =>
            sum + (record.staySeconds ?? record.stayMinutes * 60),
        0,
    );
    const sortedVisits = [...records].sort((a, b) =>
        visitSortValue(a).localeCompare(visitSortValue(b)),
    );

    return {
        visitCount: records.length,
        totalStaySeconds,
        firstVisit: sortedVisits[0] ?? null,
        latestVisit: sortedVisits.at(-1) ?? null,
    };
}

export function dateKeySortValue(dateKey: string) {
    return dateKey;
}

export function latestDateKey(records: VisitRecord[], fallback: string) {
    if (records.length === 0) return fallback;
    return records.map((record) => record.dateKey).sort((a, b) => dateKeySortValue(a).localeCompare(dateKeySortValue(b))).at(-1) ?? fallback;
}

function visitSortValue(record: VisitRecord) {
    return `${dateKeySortValue(record.dateKey)}T${record.time}`;
}

export function paperClass(paperStyle: string) {
    if (paperStyle === "dotted") return "notebook-dotted";
    if (paperStyle === "lined") return "notebook-lined";
    return "bg-[#FCFAF7]";
}

export function tapeClass(colorStyle: string) {
    if (colorStyle === "mint") return "bg-emerald-100/80 border-emerald-200/50 text-emerald-800/80";
    if (colorStyle === "lavender") return "bg-violet-100/80 border-violet-200/50 text-violet-800/80";
    if (colorStyle === "pink") return "bg-rose-100/80 border-rose-200/50 text-rose-800/80";
    return "bg-[#EADDC9]/90 border-[#D2C3AB]/60 text-amber-900/60";
}

export function categoryTheme(category: string) {
    if (category === "cozy") return "bg-amber-50 text-amber-700 border-amber-200";
    if (category === "music") return "bg-purple-50 text-purple-700 border-purple-200";
    if (category === "scenic") return "bg-sky-50 text-sky-700 border-sky-200";
    if (category === "game") return "bg-emerald-50 text-emerald-700 border-emerald-200";
    if (category === "cyberpunk") return "bg-cyan-50 text-cyan-700 border-cyan-200";
    return "bg-zinc-50 text-zinc-600 border-zinc-200";
}

