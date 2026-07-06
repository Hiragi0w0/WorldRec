use crate::db::models::{PendingVisit, VisitInput};
use crate::log_watcher::finalize::finalize_pending_visit;
use crate::log_watcher::stay_duration::parse_timestamp_seconds;

#[derive(Debug, Clone, Default)]
pub struct VisitMetadataPatch {
    pub world_id: Option<String>,
    pub instance_id: Option<String>,
    pub instance_access_type: Option<String>,
    pub instance_nonce: Option<String>,
    pub instance_raw_tags: Option<String>,
    pub source_log_file: Option<String>,
}

#[derive(Debug, Default)]
pub struct VisitSession {
    pending_visit: Option<PendingVisit>,
}

impl VisitSession {
    pub fn new() -> Self {
        Self {
            pending_visit: None,
        }
    }

    pub fn current_visit(&self) -> Option<&PendingVisit> {
        self.pending_visit.as_ref()
    }

    pub fn discard_current_visit(&mut self) {
        self.pending_visit = None;
    }

    pub fn enter_world(&mut self, next_visit: PendingVisit) -> Option<VisitInput> {
        if let Some(current_visit) = self.pending_visit.as_mut() {
            if is_duplicate_visit_event(current_visit, &next_visit) {
                merge_pending_visit_metadata(current_visit, &next_visit);
                return None;
            }
        }

        self.pending_visit = Some(next_visit);

        None
    }

    pub fn apply_metadata(&mut self, metadata_patch: VisitMetadataPatch) -> bool {
        let Some(current_visit) = self.pending_visit.as_mut() else {
            return false;
        };

        merge_metadata_patch(current_visit, metadata_patch)
    }

    pub fn leave_world(&mut self, end_timestamp: &str) -> Option<VisitInput> {
        let pending_visit = self.pending_visit.take()?;

        finalize_pending_visit(pending_visit, end_timestamp)
    }
}

fn is_duplicate_visit_event(current_visit: &PendingVisit, next_visit: &PendingVisit) -> bool {
    is_same_source_log_file(current_visit, next_visit)
        && is_same_world(current_visit, next_visit)
        && timestamps_within_duplicate_window(&current_visit.visited_at, &next_visit.visited_at)
}

fn is_same_world(current_visit: &PendingVisit, next_visit: &PendingVisit) -> bool {
    let same_world_id = match (&current_visit.world_id, &next_visit.world_id) {
        (Some(current_world_id), Some(next_world_id)) => current_world_id == next_world_id,
        _ => false,
    };

    same_world_id || current_visit.world_name == next_visit.world_name
}

fn is_same_source_log_file(current_visit: &PendingVisit, next_visit: &PendingVisit) -> bool {
    match (&current_visit.source_log_file, &next_visit.source_log_file) {
        (Some(current_source), Some(next_source)) => current_source == next_source,
        _ => false,
    }
}

fn timestamps_within_duplicate_window(current_timestamp: &str, next_timestamp: &str) -> bool {
    let Some(current_seconds) = parse_timestamp_seconds(current_timestamp) else {
        return false;
    };
    let Some(next_seconds) = parse_timestamp_seconds(next_timestamp) else {
        return false;
    };

    let delta = next_seconds - current_seconds;

    (0..=5).contains(&delta)
}

fn merge_pending_visit_metadata(
    current_visit: &mut PendingVisit,
    next_visit: &PendingVisit,
) -> bool {
    merge_metadata_patch(
        current_visit,
        VisitMetadataPatch {
            world_id: next_visit.world_id.clone(),
            instance_id: next_visit.instance_id.clone(),
            instance_access_type: next_visit.instance_access_type.clone(),
            instance_nonce: next_visit.instance_nonce.clone(),
            instance_raw_tags: next_visit.instance_raw_tags.clone(),
            source_log_file: next_visit.source_log_file.clone(),
        },
    )
}

fn merge_metadata_patch(
    current_visit: &mut PendingVisit,
    metadata_patch: VisitMetadataPatch,
) -> bool {
    let mut changed = false;

    changed |= fill_if_missing(&mut current_visit.world_id, metadata_patch.world_id);
    changed |= fill_if_missing(&mut current_visit.instance_id, metadata_patch.instance_id);
    changed |= prefer_specific_access_type(
        &mut current_visit.instance_access_type,
        metadata_patch.instance_access_type,
    );
    changed |= fill_if_missing(
        &mut current_visit.instance_nonce,
        metadata_patch.instance_nonce,
    );
    changed |= update_raw_tags(
        &mut current_visit.instance_raw_tags,
        metadata_patch.instance_raw_tags,
    );
    changed |= fill_if_missing(
        &mut current_visit.source_log_file,
        metadata_patch.source_log_file,
    );

    changed
}

fn fill_if_missing(slot: &mut Option<String>, next_value: Option<String>) -> bool {
    if slot.is_some() || next_value.is_none() {
        return false;
    }

    *slot = next_value;
    true
}

fn prefer_specific_access_type(slot: &mut Option<String>, next_value: Option<String>) -> bool {
    let Some(next_value) = next_value else {
        return false;
    };

    match slot {
        None => {
            *slot = Some(next_value);
            true
        }
        Some(current_value)
            if current_value.eq_ignore_ascii_case("public")
                && !next_value.eq_ignore_ascii_case("public") =>
        {
            *slot = Some(next_value);
            true
        }
        _ => false,
    }
}

fn update_raw_tags(slot: &mut Option<String>, next_value: Option<String>) -> bool {
    let Some(next_value) = next_value else {
        return false;
    };

    match slot {
        Some(current_value) if current_value.len() >= next_value.len() => false,
        _ => {
            *slot = Some(next_value);
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_pending_visit(visited_at: &str, world_name: &str) -> PendingVisit {
        PendingVisit {
            visited_at: visited_at.to_string(),
            world_name: world_name.to_string(),
            world_id: Some(format!("wrld_{}", world_name)),
            instance_id: Some("12345".to_string()),
            instance_access_type: Some("public".to_string()),
            instance_nonce: None,
            instance_raw_tags: None,
            source_log_file: Some("output_log_2026-06-09.txt".to_string()),
        }
    }

    #[test]
    fn first_world_is_kept_as_pending_visit() {
        let mut session = VisitSession::new();

        let result = session.enter_world(make_pending_visit("2026-06-09T20:00:00+09:00", "WorldA"));

        assert!(result.is_none());

        let current_visit = session.current_visit().expect("current visit should exist");

        assert_eq!(current_visit.world_name, "WorldA");
    }

    #[test]
    fn entering_next_world_replaces_pending_visit_without_finalizing_previous_visit() {
        let mut session = VisitSession::new();

        session.enter_world(make_pending_visit("2026-06-09T20:00:00+09:00", "WorldA"));

        let result = session.enter_world(make_pending_visit("2026-06-09T21:30:00+09:00", "WorldB"));

        assert!(result.is_none());

        let current_visit = session.current_visit().expect("current visit should exist");

        assert_eq!(current_visit.world_name, "WorldB");
    }

    #[test]
    fn duplicate_world_entry_within_five_seconds_keeps_single_pending_visit() {
        let mut session = VisitSession::new();

        session.enter_world(make_pending_visit("2026-06-09T20:00:00+09:00", "WorldA"));

        let duplicate = PendingVisit {
            visited_at: "2026-06-09T20:00:04+09:00".to_string(),
            world_name: "WorldA".to_string(),
            world_id: Some("wrld_WorldA".to_string()),
            instance_id: Some("12345~hidden(usr)".to_string()),
            instance_access_type: Some("hidden".to_string()),
            instance_nonce: Some("nonce".to_string()),
            instance_raw_tags: Some("12345~hidden(usr)".to_string()),
            source_log_file: Some("output_log_2026-06-09.txt".to_string()),
        };

        let result = session.enter_world(duplicate);

        assert!(result.is_none());

        let current_visit = session
            .current_visit()
            .expect("current visit should remain");

        assert_eq!(current_visit.world_name, "WorldA");
        assert_eq!(
            current_visit.instance_access_type,
            Some("hidden".to_string())
        );
        assert_eq!(current_visit.instance_nonce, Some("nonce".to_string()));
        assert_eq!(
            current_visit.instance_raw_tags,
            Some("12345~hidden(usr)".to_string())
        );
    }

    #[test]
    fn duplicate_world_entry_at_exactly_five_seconds_keeps_single_pending_visit() {
        let mut session = VisitSession::new();

        session.enter_world(make_pending_visit("2026-06-09T20:00:00+09:00", "WorldA"));

        let result = session.enter_world(make_pending_visit("2026-06-09T20:00:05+09:00", "WorldA"));

        assert!(result.is_none());

        let current_visit = session
            .current_visit()
            .expect("current visit should remain");

        assert_eq!(current_visit.world_name, "WorldA");
        assert_eq!(current_visit.visited_at, "2026-06-09T20:00:00+09:00");
    }

    #[test]
    fn duplicate_world_name_entry_within_five_seconds_keeps_single_pending_visit_without_world_id()
    {
        let mut session = VisitSession::new();

        let mut first_visit = make_pending_visit("2026-06-09T20:00:00+09:00", "WorldA");
        first_visit.world_id = None;
        session.enter_world(first_visit);

        let mut next_visit = make_pending_visit("2026-06-09T20:00:04+09:00", "WorldA");
        next_visit.world_id = None;

        let result = session.enter_world(next_visit);

        assert!(result.is_none());

        let current_visit = session
            .current_visit()
            .expect("current visit should remain");

        assert_eq!(current_visit.world_name, "WorldA");
        assert_eq!(current_visit.visited_at, "2026-06-09T20:00:00+09:00");
    }

    #[test]
    fn same_world_entry_after_duplicate_window_replaces_pending_without_finalizing() {
        let mut session = VisitSession::new();

        session.enter_world(make_pending_visit("2026-06-09T20:00:00+09:00", "WorldA"));

        let result = session.enter_world(make_pending_visit("2026-06-09T20:00:06+09:00", "WorldA"));

        assert!(result.is_none());

        let current_visit = session.current_visit().expect("current visit should exist");

        assert_eq!(current_visit.world_name, "WorldA");
        assert_eq!(current_visit.visited_at, "2026-06-09T20:00:06+09:00");
    }

    #[test]
    fn same_world_entry_from_different_log_file_replaces_pending_without_finalizing() {
        let mut session = VisitSession::new();

        session.enter_world(make_pending_visit("2026-06-09T20:00:00+09:00", "WorldA"));

        let mut next_visit = make_pending_visit("2026-06-09T20:00:04+09:00", "WorldA");
        next_visit.source_log_file = Some("output_log_2026-06-10.txt".to_string());

        let result = session.enter_world(next_visit);

        assert!(result.is_none());

        let current_visit = session.current_visit().expect("current visit should exist");

        assert_eq!(current_visit.world_name, "WorldA");
        assert_eq!(current_visit.visited_at, "2026-06-09T20:00:04+09:00");
        assert_eq!(
            current_visit.source_log_file,
            Some("output_log_2026-06-10.txt".to_string())
        );
    }

    #[test]
    fn metadata_patch_is_merged_into_current_visit() {
        let mut session = VisitSession::new();

        let mut pending_visit = make_pending_visit("2026-06-09T20:00:00+09:00", "WorldA");
        pending_visit.world_id = None;
        pending_visit.instance_id = None;
        pending_visit.instance_raw_tags = None;
        pending_visit.instance_nonce = None;

        session.enter_world(pending_visit);

        let changed = session.apply_metadata(VisitMetadataPatch {
            world_id: Some("wrld_worlda".to_string()),
            instance_id: Some("12345~private(usr)".to_string()),
            instance_access_type: Some("private".to_string()),
            instance_nonce: Some("nonce".to_string()),
            instance_raw_tags: Some("12345~private(usr)".to_string()),
            source_log_file: Some("output_log_2026-06-09.txt".to_string()),
        });

        assert!(changed);

        let current_visit = session.current_visit().expect("current visit should exist");

        assert_eq!(current_visit.world_id, Some("wrld_worlda".to_string()));
        assert_eq!(
            current_visit.instance_id,
            Some("12345~private(usr)".to_string())
        );
        assert_eq!(current_visit.instance_nonce, Some("nonce".to_string()));
    }

    #[test]
    fn leaving_world_finalizes_current_visit() {
        let mut session = VisitSession::new();

        session.enter_world(make_pending_visit("2026-06-09T20:00:00+09:00", "WorldA"));

        let result = session.leave_world("2026-06-09T20:00:25+09:00");

        let visit_input = result.expect("leave should finalize visit");

        assert_eq!(visit_input.stay_duration_seconds, 25);
        assert!(session.current_visit().is_none());
    }

    #[test]
    fn discarding_current_visit_drops_pending_without_finalizing() {
        let mut session = VisitSession::new();

        session.enter_world(make_pending_visit("2026-06-09T20:00:00+09:00", "WorldA"));

        session.discard_current_visit();

        assert!(session.current_visit().is_none());
    }
}
