import {
  getRuntimeStatus,
  listVisits,
  startLogWatcher,
  syncLatestVrchatLog,
  type RuntimeStatusDto,
  type SyncLatestLogResult,
  type VisitHistoryDto,
  type VisitFilterCriteria,
} from "../api/commands";
import type { DateListItem, VisitRecord } from "../data/visitTypes";
import {
  dateDisplayLabel,
  mapVisitHistoryToDisplayRecord,
} from "../data/visitMapper";
import { dateKeySortValue } from "../utils/history";

const MAIN_VISIT_RANGE_DAYS = 14;
const MAIN_VISIT_LIMIT = 100;

export class HistoriesState {
  private readonly mainDateAnchor = new Date();
  readonly mainVisitCriteria = createMainVisitCriteria(this.mainDateAnchor);

  visitRecords = $state<VisitRecord[]>([]);
  dateList = $state<DateListItem[]>(buildDateList(this.mainDateAnchor));
  selectedDate = $state(formatLocalDateKey(this.mainDateAnchor));
  searchQuery = $state("");
  isLoading = $state(false);
  runtimeStatusLoading = $state(false);
  runtimeStatus = $state<RuntimeStatusDto | null>(null);
  error = $state<string | null>(null);
  watcherRunning = $state(false);
  currentCriteria = $state<VisitFilterCriteria>(this.mainVisitCriteria);

  async loadVisits(criteria?: VisitFilterCriteria) {
    const effectiveCriteria = criteria ?? this.mainVisitCriteria;

    this.isLoading = true;
    this.error = null;
    this.currentCriteria = effectiveCriteria;
    try {
      const visits = await listVisits(effectiveCriteria);
      const currentVisitId = findCurrentVisitId(visits);
      this.visitRecords = visits.map((visit) =>
        mapVisitHistoryToDisplayRecord(visit, {
          isCurrentVisit: visit.id === currentVisitId,
        }),
      );
      this.dateList = buildDateList(this.mainDateAnchor);
      this.selectedDate = pickSelectedDate(
        this.selectedDate,
        this.mainDateAnchor,
      );
    } catch (error) {
      this.error = toErrorMessage(error);
      this.visitRecords = [];
      this.dateList = buildDateList(this.mainDateAnchor);
      this.selectedDate = formatLocalDateKey(this.mainDateAnchor);
    } finally {
      this.isLoading = false;
    }
  }

  async refreshRuntimeStatus(options: { showLoading?: boolean } = {}) {
    const showLoading = options.showLoading ?? false;

    if (showLoading) {
      this.runtimeStatusLoading = true;
    }

    try {
      this.runtimeStatus = await getRuntimeStatus();
      this.watcherRunning = this.runtimeStatus.watcher_running;
      if (this.runtimeStatus.watcher_last_error) {
        this.error = this.runtimeStatus.watcher_last_error;
      }
    } catch (error) {
      this.error = toErrorMessage(error);
    } finally {
      if (showLoading) {
        this.runtimeStatusLoading = false;
      }
    }
  }

  async startWatcher(options: { settleCurrentVisit?: boolean } = {}) {
    try {
      const status = await startLogWatcher();
      this.watcherRunning = status.running;
      if (status.last_error) this.error = status.last_error;
      if (options.settleCurrentVisit) {
        await this.refreshRuntimeStatusUntilCurrentVisitSettles();
      } else {
        await this.refreshRuntimeStatus();
      }
    } catch (error) {
      this.error = toErrorMessage(error);
    }
  }

  async syncLatestLog(): Promise<SyncLatestLogResult> {
    try {
      const result = await syncLatestVrchatLog();
      this.watcherRunning = result.watcher_running;
      await this.loadVisits(this.mainVisitCriteria);
      await this.refreshRuntimeStatus();
      return result;
    } catch (error) {
      this.error = toErrorMessage(error);
      throw error;
    }
  }

  async refreshRuntimeStatusUntilCurrentVisitSettles(
    options: { attempts?: number; intervalMs?: number } = {},
  ): Promise<void> {
    const attempts = options.attempts ?? 5;
    const intervalMs = options.intervalMs ?? 600;

    for (let attempt = 0; attempt < attempts; attempt++) {
      await this.refreshRuntimeStatus();

      const status = this.runtimeStatus;
      if (!status) return;
      if (status.current_visit) return;
      if (!status.watcher_running) return;
      if (!status.vrchat_running) return;

      if (attempt < attempts - 1) {
        await delay(intervalMs);
      }
    }
  }

  async handleVisitSaved() {
    await this.loadVisits(this.currentCriteria);
    await this.refreshRuntimeStatus();
  }

  setSelectedDate(dateKey: string) {
    this.selectedDate = dateKey;
  }

  setSearchQuery(query: string) {
    this.searchQuery = query;
  }

  setWatcherStatus(running: boolean, lastError: string | null = null) {
    this.watcherRunning = running;
    if (this.runtimeStatus) {
      this.runtimeStatus = {
        ...this.runtimeStatus,
        watcher_running: running,
        watcher_last_error: lastError,
      };
    }
    if (lastError) this.error = lastError;
  }
}

export function createHistoriesState() {
  return new HistoriesState();
}

export function createMainVisitCriteria(anchor: Date): VisitFilterCriteria {
  return {
    mode: "range",
    start: formatLocalDateKey(
      addLocalDays(anchor, -(MAIN_VISIT_RANGE_DAYS - 1)),
    ),
    end: `${formatLocalDateKey(addLocalDays(anchor, 1))}T00:00:00`,
    limit: MAIN_VISIT_LIMIT,
  };
}

function buildDateList(anchor: Date): DateListItem[] {
  return Array.from({ length: MAIN_VISIT_RANGE_DAYS }, (_, index) => {
    const dateKey = formatLocalDateKey(
      addLocalDays(anchor, index - (MAIN_VISIT_RANGE_DAYS - 1)),
    );

    return {
      key: dateKey,
      display: dateDisplayLabel(dateKey),
    };
  }).sort((left, right) =>
    dateKeySortValue(left.key).localeCompare(dateKeySortValue(right.key)),
  );
}

function findCurrentVisitId(visits: VisitHistoryDto[]): number | null {
  const latestVisit = visits.reduce<VisitHistoryDto | null>((latest, visit) => {
    if (!latest) return visit;

    const visitTime = parseVisitedAtTime(visit.visited_at);
    const latestTime = parseVisitedAtTime(latest.visited_at);

    if (visitTime > latestTime) return visit;
    if (visitTime < latestTime) return latest;
    return visit.id > latest.id ? visit : latest;
  }, null);

  if (latestVisit?.stay_duration_seconds !== null) return null;

  return latestVisit.id;
}

function parseVisitedAtTime(value: string) {
  const normalized = value.includes("T") ? value : value.replace(" ", "T");
  const parsed = Date.parse(normalized);

  return Number.isNaN(parsed) ? Number.NEGATIVE_INFINITY : parsed;
}

function pickSelectedDate(currentDate: string, anchor: Date) {
  const anchorDate = formatLocalDateKey(anchor);
  if (isInMainDateRange(currentDate, anchorDate)) {
    return currentDate;
  }

  return anchorDate;
}

function isInMainDateRange(dateKey: string, anchorDateKey: string) {
  if (!dateKey) return false;

  const startDateKey = formatLocalDateKey(
    addLocalDays(parseLocalDateKey(anchorDateKey), -(MAIN_VISIT_RANGE_DAYS - 1)),
  );

  return dateKey >= startDateKey && dateKey <= anchorDateKey;
}

function addLocalDays(date: Date, days: number) {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate() + days);
}

function formatLocalDateKey(date: Date) {
  const year = date.getFullYear();
  const month = `${date.getMonth() + 1}`.padStart(2, "0");
  const day = `${date.getDate()}`.padStart(2, "0");

  return `${year}-${month}-${day}`;
}

function parseLocalDateKey(dateKey: string) {
  const [year, month, day] = dateKey.split("-").map(Number);
  return new Date(year, month - 1, day);
}

function toErrorMessage(error: unknown) {
  if (error instanceof Error) return error.message;
  if (typeof error === "string") return error;
  return "Tauri command failed.";
}

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
