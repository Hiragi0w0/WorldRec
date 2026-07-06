import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { LogWatcherStatus } from "./commands";

export const VISIT_SAVED_EVENT = "visit_saved";
export const LOG_WATCH_STATE_CHANGED_EVENT = "log_watch_state_changed";
export const LOG_WATCH_ERROR_EVENT = "log_watch_error";
export const CURRENT_VISIT_CHANGED_EVENT = "current_visit_changed";
export const OPEN_SETTINGS_EVENT = "open_settings";

export type VisitSavedPayload = {
  visited_at: string;
  world_name: string;
  world_id: string | null;
  stay_duration_seconds: number | null;
};

export function listenVisitSaved(
  handler: (payload: VisitSavedPayload) => void,
): Promise<UnlistenFn> {
  return listen<VisitSavedPayload>(VISIT_SAVED_EVENT, (event) => {
    handler(event.payload);
  });
}

export function listenLogWatchStateChanged(
  handler: (payload: LogWatcherStatus) => void,
): Promise<UnlistenFn> {
  return listen<LogWatcherStatus>(LOG_WATCH_STATE_CHANGED_EVENT, (event) => {
    handler(event.payload);
  });
}

export function listenLogWatchError(
  handler: (payload: string) => void,
): Promise<UnlistenFn> {
  return listen<string>(LOG_WATCH_ERROR_EVENT, (event) => {
    handler(event.payload);
  });
}

export function listenCurrentVisitChanged(
  handler: () => void,
): Promise<UnlistenFn> {
  return listen(CURRENT_VISIT_CHANGED_EVENT, () => {
    handler();
  });
}

export function listenOpenSettings(handler: () => void): Promise<UnlistenFn> {
  return listen(OPEN_SETTINGS_EVENT, () => {
    handler();
  });
}
