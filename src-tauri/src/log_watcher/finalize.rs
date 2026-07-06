use crate::db::models::{PendingVisit, VisitInput};
use crate::log_watcher::stay_duration::calculate_stay_duration_from_timestamps;

pub fn finalize_pending_visit(
    pending_visit: PendingVisit,
    end_timestamp: &str,
) -> Option<VisitInput> {
    let stay_duration_seconds =
        calculate_stay_duration_from_timestamps(&pending_visit.visited_at, end_timestamp)?;

    Some(pending_visit.into_visit_input(stay_duration_seconds))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finalizes_pending_visit_into_visit_input() {
        let pending_visit = PendingVisit {
            visited_at: "2026-06-09T20:00:00+09:00".to_string(),
            world_name: "Example World".to_string(),
            world_id: Some("wrld_example".to_string()),
            instance_id: Some("12345".to_string()),
            instance_access_type: Some("public".to_string()),
            instance_nonce: None,
            instance_raw_tags: None,
            source_log_file: Some("output_log_2026-06-09.txt".to_string()),
        };

        let result = finalize_pending_visit(pending_visit, "2026-06-09T21:30:00+09:00");

        let visit_input = result.expect("pending visit should be finalized");

        assert_eq!(visit_input.visited_at, "2026-06-09T20:00:00+09:00");
        assert_eq!(visit_input.world_name, "Example World");
        assert_eq!(visit_input.world_id, Some("wrld_example".to_string()));
        assert_eq!(visit_input.stay_duration_seconds, 5400);
        assert_eq!(
            visit_input.source_log_file,
            Some("output_log_2026-06-09.txt".to_string())
        );
    }

    #[test]
    fn returns_none_when_pending_visit_timestamp_is_invalid() {
        let pending_visit = PendingVisit {
            visited_at: "invalid timestamp".to_string(),
            world_name: "Example World".to_string(),
            world_id: Some("wrld_example".to_string()),
            instance_id: Some("12345".to_string()),
            instance_access_type: Some("public".to_string()),
            instance_nonce: None,
            instance_raw_tags: None,
            source_log_file: Some("output_log_2026-06-09.txt".to_string()),
        };

        let result = finalize_pending_visit(pending_visit, "2026-06-09T21:30:00+09:00");

        assert!(result.is_none());
    }
}
