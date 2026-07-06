import { defineConfig } from "vite";
import type { Plugin, ViteDevServer } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import tailwindcss from "@tailwindcss/vite";
import { execFile } from "node:child_process";
import type { IncomingMessage, ServerResponse } from "node:http";
import { promisify } from "node:util";

const execFileAsync = promisify(execFile);
const devDbPath = `${process.env.LOCALAPPDATA ?? "."}\\WorldRec\\worldrec.db`;
const devLogDir = `${process.env.LOCALAPPDATA ?? "."}\\..\\LocalLow\\VRChat\\VRChat`;

export default defineConfig({
  plugins: [svelte(), tailwindcss(), worldRecDevDbBridge()],
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
  },
});

function worldRecDevDbBridge(): Plugin {
  return {
    name: "worldrec-dev-db-bridge",
    configureServer(server: ViteDevServer) {
      server.middlewares.use(async (req: IncomingMessage, res: ServerResponse, next: () => void) => {
        if (!req.url?.startsWith("/__worldrec_dev/")) {
          next();
          return;
        }

        try {
          const url = new URL(req.url, "http://localhost");
          const criteria = JSON.parse(url.searchParams.get("criteria") ?? "{}");
          const data =
            url.pathname === "/__worldrec_dev/list_visits"
              ? await queryDevDb({ command: "list_visits", criteria })
              : url.pathname === "/__worldrec_dev/runtime_status"
                ? await queryDevDb({ command: "runtime_status" })
                : null;

          if (data === null) {
            res.statusCode = 404;
            res.end(JSON.stringify({ error: "Unknown WorldRec dev endpoint." }));
            return;
          }

          res.setHeader("content-type", "application/json; charset=utf-8");
          res.end(JSON.stringify(data));
        } catch (error) {
          res.statusCode = 500;
          res.setHeader("content-type", "application/json; charset=utf-8");
          res.end(JSON.stringify({ error: error instanceof Error ? error.message : String(error) }));
        }
      });
    },
  };
}

async function queryDevDb(payload: unknown) {
  const script = String.raw`
import json
import os
import re
import sqlite3
import sys
from datetime import date, datetime, timedelta, timezone

payload = json.loads(sys.argv[1])
db_path = sys.argv[2]
log_dir = sys.argv[3]

def row_to_visit(row):
    keys = [
        "id",
        "visited_at",
        "world_name",
        "world_id",
        "instance_id",
        "instance_access_type",
        "stay_duration_seconds",
        "memo",
        "tags",
        "source_log_file",
        "created_at",
        "updated_at",
    ]
    return {key: row[key] for key in keys}

def normalize_date_bound(value, end=False):
    if not value:
        return None
    if len(value) == 10 and value[4] == "-":
        return value + ("T23:59:59" if end else "T00:00:00")
    return value

ENTRY_MARKERS = ["Entering Room:", "Joining or Creating Room:"]
LEAVE_MARKERS = ["[Behaviour] OnLeftRoom", "listeners for ExitWorld event"]
TOKYO = timezone(timedelta(hours=9))

def parse_log_timestamp(line):
    if len(line) < 19:
        return None
    try:
        dt = datetime.strptime(line[:19], "%Y.%m.%d %H:%M:%S")
    except ValueError:
        return None
    return dt.replace(tzinfo=TOKYO).isoformat()

def parse_world_name(line):
    for marker in ENTRY_MARKERS:
        if marker in line:
            world_name = line.split(marker, 1)[1].split(" wrld_", 1)[0].strip()
            return world_name or None
    return None

def parse_world_id(line):
    match = re.search(r"(wrld_[A-Za-z0-9_-]+)", line)
    return match.group(1) if match else None

def parse_instance_id(line, world_id):
    if not world_id:
        return None
    marker = world_id + ":"
    if marker not in line:
        return None
    tail = line.split(marker, 1)[1]
    chars = []
    for ch in tail:
        if ch.isspace() or ch in "],":
            break
        chars.append(ch)
    value = "".join(chars)
    return value or None

def parse_instance_access_type(instance_id):
    if not instance_id:
        return None
    lowered = instance_id.lower()
    if "~private" in lowered:
        return "private"
    if "~hidden" in lowered:
        return "hidden"
    if "~friends" in lowered:
        return "friends"
    return "public"

def parse_instance_nonce(instance_id):
    if not instance_id or "~nonce(" not in instance_id:
        return None
    return instance_id.split("~nonce(", 1)[1].split(")", 1)[0].strip() or None

def parse_entry_visit(line, source_log_file):
    timestamp = parse_log_timestamp(line)
    world_name = parse_world_name(line)
    if not timestamp or not world_name:
        return None
    world_id = parse_world_id(line)
    instance_id = parse_instance_id(line, world_id)
    return {
        "visited_at": timestamp,
        "world_name": world_name,
        "world_id": world_id,
        "instance_id": instance_id,
        "instance_access_type": parse_instance_access_type(instance_id) or "public",
        "instance_nonce": parse_instance_nonce(instance_id),
        "instance_raw_tags": instance_id,
        "source_log_file": source_log_file,
    }

def parse_metadata_patch(line, source_log_file):
    if "[Behaviour] Joining wrld_" not in line:
        return None
    world_id = parse_world_id(line)
    instance_id = parse_instance_id(line, world_id)
    if not world_id and not instance_id:
        return None
    return {
        "world_id": world_id,
        "instance_id": instance_id,
        "instance_access_type": parse_instance_access_type(instance_id),
        "instance_nonce": parse_instance_nonce(instance_id),
        "instance_raw_tags": instance_id,
        "source_log_file": source_log_file,
    }

def is_leave_event(line):
    return any(marker in line for marker in LEAVE_MARKERS)

def parse_seconds(timestamp):
    if not timestamp:
        return None
    try:
        return int(datetime.fromisoformat(timestamp).timestamp())
    except ValueError:
        return None

def same_world(current_visit, next_visit):
    if current_visit.get("world_id") and next_visit.get("world_id"):
        return current_visit["world_id"] == next_visit["world_id"]
    return current_visit.get("world_name") == next_visit.get("world_name")

def is_duplicate_visit(current_visit, next_visit):
    if not current_visit or not next_visit:
        return False
    if current_visit.get("source_log_file") != next_visit.get("source_log_file"):
        return False
    if not same_world(current_visit, next_visit):
        return False
    current_seconds = parse_seconds(current_visit.get("visited_at"))
    next_seconds = parse_seconds(next_visit.get("visited_at"))
    if current_seconds is None or next_seconds is None:
        return False
    return 0 <= (next_seconds - current_seconds) <= 5

def merge_patch(current_visit, patch):
    if not current_visit or not patch:
        return
    for key in ["world_id", "instance_id", "instance_nonce", "source_log_file"]:
        if not current_visit.get(key) and patch.get(key):
            current_visit[key] = patch[key]
    if (not current_visit.get("instance_access_type") or current_visit.get("instance_access_type") == "public") and patch.get("instance_access_type") and patch.get("instance_access_type") != "public":
        current_visit["instance_access_type"] = patch["instance_access_type"]
    if patch.get("instance_raw_tags") and len(patch["instance_raw_tags"]) > len(current_visit.get("instance_raw_tags") or ""):
        current_visit["instance_raw_tags"] = patch["instance_raw_tags"]

def latest_pending_visit(log_dir):
    if not os.path.isdir(log_dir):
        return None
    files = [
        os.path.join(log_dir, name)
        for name in os.listdir(log_dir)
        if name.startswith("output_log_") and name.endswith(".txt")
    ]
    if not files:
        return None
    latest = max(files, key=os.path.getmtime)
    pending = None
    source_log_file = os.path.basename(latest)
    with open(latest, "r", encoding="utf-8", errors="ignore") as fh:
        for raw_line in fh:
            line = raw_line.rstrip("\n")
            entry = parse_entry_visit(line, source_log_file)
            if entry:
                if pending and is_duplicate_visit(pending, entry):
                    merge_patch(pending, entry)
                else:
                    pending = entry
                continue
            patch = parse_metadata_patch(line, source_log_file)
            if patch and pending:
                merge_patch(pending, patch)
                continue
            if is_leave_event(line):
                pending = None
    return pending

if not os.path.exists(db_path):
    raise SystemExit(f"WorldRec DB was not found: {db_path}")

conn = sqlite3.connect(db_path)
conn.row_factory = sqlite3.Row

if payload["command"] == "runtime_status":
    count = conn.execute("SELECT COUNT(*) FROM visit_histories").fetchone()[0]
    latest = conn.execute("SELECT visited_at, world_name, world_id, instance_id, instance_access_type, source_log_file FROM visit_histories ORDER BY visited_at DESC, id DESC LIMIT 1").fetchone()
    print(json.dumps({
        "db_path": db_path,
        "log_dir": os.path.abspath(log_dir),
        "watcher_running": False,
        "vrchat_running": False,
        "watcher_last_error": None,
        "visit_count": count,
        "latest_visit_at": latest["visited_at"] if latest else None,
        "latest_world_name": latest["world_name"] if latest else None,
        "current_visit": latest_pending_visit(log_dir),
    }, ensure_ascii=True))
    raise SystemExit(0)

criteria = payload.get("criteria") or {}
mode = criteria.get("mode") or "recent"
where = []
params = []

if mode == "today":
    start = date.today().isoformat() + "T00:00:00"
    end = (date.today() + timedelta(days=1)).isoformat() + "T00:00:00"
    where.extend(["visited_at >= ?", "visited_at < ?"])
    params.extend([start, end])
elif mode == "yesterday":
    start_day = date.today() - timedelta(days=1)
    where.extend(["visited_at >= ?", "visited_at < ?"])
    params.extend([start_day.isoformat() + "T00:00:00", date.today().isoformat() + "T00:00:00"])
elif mode == "range":
    start = normalize_date_bound(criteria.get("start"))
    end = normalize_date_bound(criteria.get("end"), True)
    if start:
        where.append("visited_at >= ?")
        params.append(start)
    if end:
        where.append("visited_at < ?")
        params.append(end)

if criteria.get("world_name"):
    where.append("world_name LIKE ?")
    params.append("%" + criteria["world_name"].strip() + "%")
if criteria.get("tag"):
    where.append("tags LIKE ?")
    params.append("%" + criteria["tag"].strip() + "%")
if criteria.get("instance_access_type"):
    where.append("instance_access_type = ?")
    params.append(criteria["instance_access_type"].strip())

sql = """
SELECT id, visited_at, world_name, world_id, instance_id, instance_access_type,
       stay_duration_seconds, memo, tags, source_log_file, created_at, updated_at
FROM visit_histories
"""
if where:
    sql += " WHERE " + " AND ".join(where)
sql += " ORDER BY visited_at DESC, id DESC"
if mode != "all":
    limit = max(1, min(int(criteria.get("limit") or 100), 500))
    sql += " LIMIT ?"
    params.append(limit)

print(json.dumps([row_to_visit(row) for row in conn.execute(sql, params)], ensure_ascii=True))
`;
  const { stdout } = await execFileAsync("python", ["-c", script, JSON.stringify(payload), devDbPath, devLogDir], {
    env: { ...process.env, PYTHONIOENCODING: "utf-8" },
    windowsHide: true,
    maxBuffer: 1024 * 1024 * 10,
  });
  return JSON.parse(stdout);
}
