use chrono::{Local, NaiveDateTime, TimeZone};

use crate::db::models::PendingVisit;
use crate::log_watcher::visit_session::VisitMetadataPatch;

const TIMESTAMP_FORMAT: &str = "%Y.%m.%d %H:%M:%S";
const ENTERING_ROOM_MARKERS: [&str; 2] = ["Entering Room:", "Joining or Creating Room:"];
const LEAVE_WORLD_MARKERS: [&str; 3] = [
    "[Behaviour] OnLeftRoom",
    "listeners for ExitWorld event",
    "VRCApplication: HandleApplicationQuit",
];

#[derive(Debug, Clone)]
pub enum ParsedLogEvent {
    EnterWorld(PendingVisit),
    VisitMetadata(VisitMetadataPatch),
    LeaveWorld(String),
}

pub fn parse_log_event(line: &str, source_log_file: Option<String>) -> Option<ParsedLogEvent> {
    if let Some(pending_visit) = parse_pending_visit(line, source_log_file.clone()) {
        return Some(ParsedLogEvent::EnterWorld(pending_visit));
    }

    if let Some(timestamp) = parse_leave_world_timestamp(line) {
        return Some(ParsedLogEvent::LeaveWorld(timestamp));
    }

    parse_visit_metadata_patch(line, source_log_file).map(ParsedLogEvent::VisitMetadata)
}

pub fn parse_pending_visit(line: &str, source_log_file: Option<String>) -> Option<PendingVisit> {
    let timestamp = parse_log_timestamp(line)?;
    let world_name = parse_world_name(line)?;

    if is_home_world_name(&world_name) {
        return None;
    }

    let world_id = parse_world_id(line);
    let instance_id = parse_instance_id(line, world_id.as_deref());

    Some(PendingVisit {
        visited_at: timestamp,
        world_name,
        world_id,
        instance_access_type: instance_id
            .as_deref()
            .map(parse_instance_access_type)
            .or(Some("public".to_string())),
        instance_nonce: instance_id.as_deref().and_then(parse_instance_nonce),
        instance_raw_tags: instance_id.clone(),
        instance_id,
        source_log_file,
    })
}

fn parse_visit_metadata_patch(
    line: &str,
    source_log_file: Option<String>,
) -> Option<VisitMetadataPatch> {
    if !line.contains("[Behaviour] Joining wrld_") {
        return None;
    }

    let world_id = parse_world_id(line);
    let instance_id = parse_instance_id(line, world_id.as_deref());

    if world_id.is_none() && instance_id.is_none() {
        return None;
    }

    Some(VisitMetadataPatch {
        world_id,
        instance_id: instance_id.clone(),
        instance_access_type: instance_id.as_deref().map(parse_instance_access_type),
        instance_nonce: instance_id.as_deref().and_then(parse_instance_nonce),
        instance_raw_tags: instance_id,
        source_log_file,
    })
}

fn parse_leave_world_timestamp(line: &str) -> Option<String> {
    if LEAVE_WORLD_MARKERS
        .iter()
        .any(|marker| line.contains(marker))
    {
        return parse_log_timestamp(line);
    }

    None
}

fn parse_log_timestamp(line: &str) -> Option<String> {
    let timestamp = line.get(..19)?;
    let naive_timestamp = NaiveDateTime::parse_from_str(timestamp, TIMESTAMP_FORMAT).ok()?;

    // VRChat output logs record local wall-clock time without an explicit offset.
    // Ambiguous or nonexistent local times are ignored rather than guessed so that
    // visit duration calculations do not persist misleading data.
    Local
        .from_local_datetime(&naive_timestamp)
        .single()
        .map(|timestamp| timestamp.to_rfc3339())
}

fn parse_world_name(line: &str) -> Option<String> {
    for marker in ENTERING_ROOM_MARKERS {
        if let Some((_, world_name)) = line.split_once(marker) {
            let world_name = world_name
                .split(" wrld_")
                .next()
                .unwrap_or(world_name)
                .trim();

            if !world_name.is_empty() {
                return Some(world_name.to_string());
            }
        }
    }

    None
}

fn is_home_world_name(world_name: &str) -> bool {
    let normalized = world_name.trim().to_ascii_lowercase();

    normalized == "home" || normalized == "home world" || normalized.ends_with("'s home")
}

fn parse_world_id(line: &str) -> Option<String> {
    let start = line.find("wrld_")?;
    let world_id = line[start..]
        .chars()
        .take_while(|character| {
            character.is_ascii_alphanumeric() || *character == '_' || *character == '-'
        })
        .collect::<String>();

    if world_id == "wrld_" {
        None
    } else {
        Some(world_id)
    }
}

fn parse_instance_id(line: &str, world_id: Option<&str>) -> Option<String> {
    let world_id = world_id?;
    let start = line.find(world_id)? + world_id.len();
    let remainder = line.get(start..)?;
    let instance_start = remainder.strip_prefix(':')?;

    let instance_id = instance_start
        .chars()
        .take_while(|character| {
            !character.is_whitespace() && *character != ']' && *character != ','
        })
        .collect::<String>();

    if instance_id.is_empty() {
        None
    } else {
        Some(instance_id)
    }
}

fn parse_instance_access_type(instance_id: &str) -> String {
    if instance_id.contains("~private") {
        "private".to_string()
    } else if instance_id.contains("~hidden") {
        "hidden".to_string()
    } else if instance_id.contains("~friends") {
        "friends".to_string()
    } else {
        "public".to_string()
    }
}

fn parse_instance_nonce(instance_id: &str) -> Option<String> {
    let marker = "~nonce(";
    let start = instance_id.find(marker)? + marker.len();
    let nonce = instance_id.get(start..)?.split(')').next()?.trim();

    if nonce.is_empty() {
        None
    } else {
        Some(nonce.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_enter_world_log_event() {
        let line = "2026.06.10 20:00:00 Log        -  [Behaviour] Entering Room: Example World";

        let event = parse_log_event(line, Some("output_log_2026-06-10.txt".to_string()))
            .expect("log event should parse");

        match event {
            ParsedLogEvent::EnterWorld(visit) => {
                assert_eq!(visit.world_name, "Example World");
            }
            _ => panic!("expected enter world event"),
        }
    }

    #[test]
    fn parses_entering_room_line_into_pending_visit() {
        let line = "2026.06.10 20:00:00 Log        -  [Behaviour] Entering Room: Example World";

        let visit = parse_pending_visit(line, Some("output_log_2026-06-10.txt".to_string()))
            .expect("world visit should parse");

        assert!(visit.visited_at.starts_with("2026-06-10T20:00:00"));
        assert_eq!(visit.world_name, "Example World");
        assert_eq!(visit.world_id, None);
        assert_eq!(visit.instance_access_type, Some("public".to_string()));
        assert_eq!(
            visit.source_log_file,
            Some("output_log_2026-06-10.txt".to_string())
        );
    }

    #[test]
    fn parses_world_id_and_instance_from_room_line_when_available() {
        let line = "2026.06.10 20:00:00 Log        -  [Behaviour] Entering Room: Example World wrld_123:456~private(id)~nonce(abc)";

        let visit = parse_pending_visit(line, None).expect("world visit should parse");

        assert_eq!(visit.world_name, "Example World");
        assert_eq!(visit.world_id, Some("wrld_123".to_string()));
        assert_eq!(
            visit.instance_id,
            Some("456~private(id)~nonce(abc)".to_string())
        );
        assert_eq!(visit.instance_access_type, Some("private".to_string()));
        assert_eq!(visit.instance_nonce, Some("abc".to_string()));
    }

    #[test]
    fn ignores_unrelated_log_line() {
        let line = "2026.06.10 20:00:00 Log        -  unrelated message";

        assert!(parse_pending_visit(line, None).is_none());
        assert!(parse_log_event(line, None).is_none());
    }

    #[test]
    fn ignores_home_world_line() {
        let line = "2026.06.10 20:00:00 Log        -  [Behaviour] Entering Room: Home";

        assert!(parse_pending_visit(line, None).is_none());
    }

    #[test]
    fn invalid_timestamp_does_not_panic() {
        let line = "invalid timestamp Log        -  [Behaviour] Entering Room: Example World";

        assert!(parse_pending_visit(line, None).is_none());
    }

    #[test]
    fn parses_joining_world_metadata_line() {
        let line = "2026.06.10 20:00:02 Log        -  [Behaviour] Joining wrld_123:456~hidden(usr)~nonce(abc)";

        let event = parse_log_event(line, Some("output_log_2026-06-10.txt".to_string()))
            .expect("metadata event should parse");

        match event {
            ParsedLogEvent::VisitMetadata(patch) => {
                assert_eq!(patch.world_id, Some("wrld_123".to_string()));
                assert_eq!(
                    patch.instance_id,
                    Some("456~hidden(usr)~nonce(abc)".to_string())
                );
                assert_eq!(patch.instance_access_type, Some("hidden".to_string()));
                assert_eq!(patch.instance_nonce, Some("abc".to_string()));
            }
            _ => panic!("expected metadata event"),
        }
    }

    #[test]
    fn parses_leave_world_event() {
        let line = "2026.06.10 20:08:00 Log        -  [Behaviour] OnLeftRoom";

        let event = parse_log_event(line, None).expect("leave event should parse");

        match event {
            ParsedLogEvent::LeaveWorld(timestamp) => {
                assert!(timestamp.starts_with("2026-06-10T20:08:00"));
            }
            _ => panic!("expected leave world event"),
        }
    }

    #[test]
    fn parses_application_quit_as_leave_world_event() {
        let line =
            "2026.06.24 01:27:44 Debug      -  VRCApplication: HandleApplicationQuit at 8366.161";

        let event = parse_log_event(line, None).expect("application quit event should parse");

        match event {
            ParsedLogEvent::LeaveWorld(timestamp) => {
                assert!(timestamp.starts_with("2026-06-24T01:27:44"));
            }
            _ => panic!("expected leave world event"),
        }
    }
}
