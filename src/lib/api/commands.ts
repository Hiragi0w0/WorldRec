import { invoke } from "@tauri-apps/api/core";
import type { PaperStyle, TapeStyle, ViewFormat } from "../data/visitTypes";

export type Theme = "system" | "light" | "dark";
export type FontSize = "standard" | "large";

export type AppSettings = {
  schema_version: number;
  theme: Theme;
  font_size: FontSize;
  tape_style: TapeStyle;
  paper_style: PaperStyle;
  view_format: ViewFormat;
  log_dir: string;
  db_path: string;
  ai_enabled: boolean;
  has_gemini_api_key: boolean;
  gemini_model: string;
  has_dify_api_key: boolean;
  dify_enabled: boolean;
  dify_endpoint: string;
  batch_flush_seconds: number;
  batch_max_events: number;
  vrchat_autostart_enabled: boolean;
  app_autostart_enabled: boolean;
  onboarding_completed: boolean;
};

export type AppAutostartStatus = {
  enabled: boolean;
};

export type VisitFilterCriteria = {
  mode?: "recent" | "all" | "today" | "yesterday" | "range";
  start?: string;
  end?: string;
  world_name?: string;
  tag?: string;
  instance_access_type?: string;
  limit?: number;
};

export type LogWatcherStatus = {
  running: boolean;
  log_dir: string;
  last_error: string | null;
};

export type CurrentVisitDto = {
  visited_at: string;
  world_name: string;
  world_id: string | null;
  instance_id: string | null;
  instance_access_type: string | null;
  source_log_file: string | null;
};

export type VisitHistoryDto = {
  id: number;
  visited_at: string;
  world_name: string;
  world_id: string | null;
  instance_id: string | null;
  instance_access_type: string | null;
  stay_duration_seconds: number | null;
  memo: string | null;
  tags: string | null;
  source_log_file: string | null;
  thumbnail_url: string | null;
  image_url: string | null;
  world_preview_fetched_at: string | null;
  created_at: string;
  updated_at: string;
};

export type LogWatcherStatusDto = LogWatcherStatus;

export type RuntimeStatusDto = {
  db_path: string;
  log_dir: string;
  watcher_running: boolean;
  vrchat_running: boolean;
  watcher_last_error: string | null;
  visit_count: number;
  latest_visit_at: string | null;
  latest_world_name: string | null;
  current_visit: CurrentVisitDto | null;
};

export type SyncLatestLogResult = {
  latest_log_file: string | null;
  processed: boolean;
  processed_line_count: number;
  saved_visit_count: number;
  watcher_running: boolean;
  skipped_reason: string | null;
  current_visit: CurrentVisitDto | null;
};

export type VrchatWorldDetailDto = {
  worldId: string;
  name: string;
  authorName: string | null;
  description: string | null;
  imageUrl: string | null;
  thumbnailImageUrl: string | null;
  capacity: number | null;
  recommendedCapacity: number | null;
  visits: number | null;
  favorites: number | null;
  occupants: number | null;
  publicOccupants: number | null;
  privateOccupants: number | null;
  releaseStatus: string | null;
  tags: string[];
  platforms: string[];
};

export type LibrarySortKey =
  | "world_name"
  | "visit_count"
  | "total_stay_duration_seconds";

export type LibrarySortDirection = "asc" | "desc";

export type LibrarySearchCriteria = {
  query: string | null;
  visited_from: string | null;
  visited_to: string | null;
  tags: string[];
  memo_query: string | null;
  sort_key: LibrarySortKey;
  sort_direction: LibrarySortDirection;
  limit: 10 | 25;
  offset: number;
};

export type LibraryWorld = {
  key: string;
  world_id: string | null;
  world_name: string;
  visit_count: number;
  first_visited_at: string;
  last_visited_at: string;
  total_stay_duration_seconds: number;
  tags: string[];
  memo_count: number;
  thumbnail_url: string | null;
  image_url: string | null;
  world_preview_fetched_at: string | null;
};

export type LibraryWorldPage = {
  items: LibraryWorld[];
  total_count: number;
  limit: number;
  offset: number;
};

export type LibraryWorldVisit = {
  id: number;
  visited_at: string;
  stay_duration_seconds: number | null;
  memo: string | null;
};

export type LibraryWorldDetail = {
  world: LibraryWorld;
  visits: LibraryWorldVisit[];
};

export async function getSettings(): Promise<AppSettings> {
  return await invoke<AppSettings>("get_settings");
}

export async function saveSettings(settings: AppSettings): Promise<AppSettings> {
  return await invoke<AppSettings>("save_settings", { settings });
}

export async function getAppAutostartStatus(): Promise<AppAutostartStatus> {
  return await invoke<AppAutostartStatus>("get_app_autostart_status");
}

export async function setAppAutostartEnabled(
  enabled: boolean,
): Promise<AppAutostartStatus> {
  return await invoke<AppAutostartStatus>("set_app_autostart_enabled", {
    enabled,
  });
}

export async function listRecentVisits(
  limit?: number,
): Promise<VisitHistoryDto[]> {
  return await listVisits({ mode: "recent", limit });
}

export async function listVisits(
  criteria: VisitFilterCriteria = {},
): Promise<VisitHistoryDto[]> {
  return await invoke<VisitHistoryDto[]>("list_visits", { criteria });
}

export async function getRuntimeStatus(): Promise<RuntimeStatusDto> {
  return await invoke<RuntimeStatusDto>("get_runtime_status");
}

export async function getVrchatWorldDetail(
  worldId: string,
  worldName?: string,
): Promise<VrchatWorldDetailDto> {
  return await invoke<VrchatWorldDetailDto>("get_vrchat_world_detail", {
    worldId,
    worldName: worldName ?? null,
  });
}

export async function listLibraryWorlds(
  criteria: LibrarySearchCriteria,
): Promise<LibraryWorldPage> {
  return await invoke<LibraryWorldPage>("list_library_worlds", { criteria });
}

export async function getLibraryWorldDetail(
  worldId: string | null,
  worldName: string,
): Promise<LibraryWorldDetail> {
  return await invoke<LibraryWorldDetail>("get_library_world_detail", {
    worldId,
    worldName,
  });
}

export async function getLogWatcherStatus(): Promise<LogWatcherStatusDto> {
  return await invoke<LogWatcherStatusDto>("get_log_watcher_status");
}

export async function startLogWatcher(): Promise<LogWatcherStatusDto> {
  return await invoke<LogWatcherStatusDto>("start_log_watcher");
}

export async function syncLatestVrchatLog(): Promise<SyncLatestLogResult> {
  return await invoke<SyncLatestLogResult>("sync_latest_vrchat_log");
}

export async function syncLatestVrchatLogBeforeExit(): Promise<SyncLatestLogResult> {
  return await invoke<SyncLatestLogResult>(
    "sync_latest_vrchat_log_before_exit",
  );
}

export type TransitionGraphCriteria = {
  start: string;
  end: string;
};

export type TransitionGraphNode = {
  key: string;
  world_id: string | null;
  world_name: string;
  visit_count: number;
  total_stay_seconds: number;
  last_visited_at: string;
  degree: number;
};

export type TransitionGraphEdge = {
  key: string;
  from: string;
  to: string;
  from_world_name: string;
  to_world_name: string;
  transition_count: number;
  latest_transition_at: string;
  transition_times: string[];
};

export type LongestVisit = {
  key: string;
  world_id: string | null;
  world_name: string;
  visited_at: string;
  stay_duration_seconds: number;
};

export type TransitionGraphSummary = {
  visit_count: number;
  unique_world_count: number;
  transition_count: number;
  top_transition: TransitionGraphEdge | null;
  top_longest_visit: LongestVisit | null;
  hidden_node_count: number;
  hidden_edge_count: number;
};

export type VisitTransitionGraph = {
  start: string;
  end: string;
  summary: TransitionGraphSummary;
  nodes: TransitionGraphNode[];
  edges: TransitionGraphEdge[];
  top_worlds: TransitionGraphNode[];
  top_transitions: TransitionGraphEdge[];
};

export async function getVisitTransitionGraph(
  criteria: TransitionGraphCriteria,
): Promise<VisitTransitionGraph> {
  return await invoke<VisitTransitionGraph>("get_visit_transition_graph", {
    criteria,
  });
}

export type StatsDateRange = {
  start: string | null;
  end: string | null;
};

export async function getStatsDateRange(): Promise<StatsDateRange> {
  return await invoke<StatsDateRange>("get_stats_date_range");
}

export type DeleteHistoryResult = {
  deleted_count: number;
};

export async function deleteAllHistory(): Promise<DeleteHistoryResult> {
  return await invoke<DeleteHistoryResult>("delete_all_history");
}

export async function deleteVisitHistory(
  visitId: number,
): Promise<DeleteHistoryResult> {
  return await invoke<DeleteHistoryResult>("delete_visit_history", { visitId });
}

export type AiStatus = {
  hasGeminiApiKey: boolean;
};

export type AiVisitedWorld = {
  world_key: string;
  world_name: string;
  world_id: string | null;
  visit_count: number;
  matched: boolean;
  searched_world_name: string | null;
  world_overview: string | null;
  recommendedNumberOfPeople: number;
};

export type AiNewWorld = {
  world_name: string;
  overview: string;
  recommended_number_of_people: string;
};

export type AiRecommendationMode = "visited_only" | "unvisited_only" | "mixed";

export type AiRecommendationSource =
  | "ai"
  | "ai_degraded"
  | "ai_failed"
  | "settings_required";

export type AiRecommendation = {
  source: AiRecommendationSource;
  retryable: boolean;
  warning?: string;
  error_message?: string;
  text: string;
  wants_unvisited: boolean;
  recommendation_mode: AiRecommendationMode;
  visited_worlds: AiVisitedWorld[];
  new_worlds: AiNewWorld[];
};

export async function getAiStatus(): Promise<AiStatus> {
  return await invoke<AiStatus>("get_ai_status");
}

export async function saveGeminiApiKey(apiKey: string): Promise<AiStatus> {
  return await invoke<AiStatus>("save_gemini_api_key", { apiKey });
}

export async function clearGeminiApiKey(): Promise<AiStatus> {
  return await invoke<AiStatus>("clear_gemini_api_key");
}

export async function recommendWorlds(
  query: string,
  queryHistory: string,
): Promise<AiRecommendation> {
  return await invoke<AiRecommendation>("recommend_worlds", {
    query,
    queryHistory,
  });
}

export type VrchatAuthStatus = {
  loggedIn: boolean;
  requiresEmail2fa: boolean;
  displayName: string | null;
  userId: string | null;
  message: string | null;
};

export type VrchatAuthResult = {
  status: "logged_in" | "requires_email_2fa" | "logged_out";
  displayName: string | null;
  userId: string | null;
  message: string | null;
};

export async function getVrchatAuthStatus(): Promise<VrchatAuthStatus> {
  return await invoke<VrchatAuthStatus>("get_vrchat_auth_status");
}

export async function vrchatLogin(
  username: string,
  password: string,
): Promise<VrchatAuthResult> {
  return await invoke<VrchatAuthResult>("vrchat_login", { username, password });
}

export async function vrchatCompleteEmail2fa(
  code: string,
): Promise<VrchatAuthResult> {
  return await invoke<VrchatAuthResult>("vrchat_complete_email_2fa", { code });
}

export async function clearVrchatLoginData(): Promise<VrchatAuthStatus> {
  return await invoke<VrchatAuthStatus>("clear_vrchat_login_data");
}
