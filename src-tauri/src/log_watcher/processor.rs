use crate::{
    db::models::{PendingVisit, VisitInput},
    log_watcher::{
        parser::{parse_log_event, ParsedLogEvent},
        visit_session::VisitSession,
    },
};

#[derive(Debug, Clone)]
pub enum ProcessedLogAction {
    CurrentVisitChanged(Option<PendingVisit>),
    FinalizedVisit(VisitInput),
}

pub fn process_log_lines_to_actions(
    lines: Vec<String>,
    source_log_file: Option<&str>,
    visit_session: &mut VisitSession,
) -> Vec<ProcessedLogAction> {
    let mut actions = Vec::new();
    let source_log_file = source_log_file.map(|file_name| file_name.to_string());

    for line in lines {
        let Some(event) = parse_log_event(&line, source_log_file.clone()) else {
            continue;
        };

        match event {
            ParsedLogEvent::EnterWorld(pending_visit) => {
                if let Some(visit_input) = visit_session.enter_world(pending_visit) {
                    actions.push(ProcessedLogAction::FinalizedVisit(visit_input));
                }

                actions.push(ProcessedLogAction::CurrentVisitChanged(
                    visit_session.current_visit().cloned(),
                ));
            }
            ParsedLogEvent::VisitMetadata(metadata_patch) => {
                if visit_session.apply_metadata(metadata_patch) {
                    actions.push(ProcessedLogAction::CurrentVisitChanged(
                        visit_session.current_visit().cloned(),
                    ));
                }
            }
            ParsedLogEvent::LeaveWorld(end_timestamp) => {
                let finalized_visit = visit_session.leave_world(&end_timestamp);

                actions.push(ProcessedLogAction::CurrentVisitChanged(
                    visit_session.current_visit().cloned(),
                ));

                if let Some(visit_input) = finalized_visit {
                    actions.push(ProcessedLogAction::FinalizedVisit(visit_input));
                }
            }
        }
    }

    actions
}

#[cfg(test)]
mod tests {
    use super::*;

    fn process_test_lines(
        visit_session: &mut VisitSession,
        lines: Vec<&str>,
    ) -> Vec<ProcessedLogAction> {
        process_log_lines_to_actions(
            lines.into_iter().map(|line| line.to_string()).collect(),
            Some("output_log_2026-06-10_20-00-00.txt"),
            visit_session,
        )
    }

    fn finalized_visits(actions: &[ProcessedLogAction]) -> Vec<&VisitInput> {
        actions
            .iter()
            .filter_map(|action| match action {
                ProcessedLogAction::FinalizedVisit(visit_input) => Some(visit_input),
                _ => None,
            })
            .collect()
    }

    fn current_visit_changes(actions: &[ProcessedLogAction]) -> Vec<&Option<PendingVisit>> {
        actions
            .iter()
            .filter_map(|action| match action {
                ProcessedLogAction::CurrentVisitChanged(current_visit) => Some(current_visit),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn first_enter_world_updates_current_visit_without_finalized_visit() {
        let mut visit_session = VisitSession::new();

        let actions = process_test_lines(
            &mut visit_session,
            vec![
                "2026.06.10 20:00:00 Log        -  [Behaviour] Entering Room: World A wrld_a:12345",
            ],
        );

        assert!(finalized_visits(&actions).is_empty());

        let changes = current_visit_changes(&actions);
        assert_eq!(changes.len(), 1);
        assert_eq!(
            changes[0]
                .as_ref()
                .expect("current visit should exist")
                .world_name,
            "World A"
        );
    }

    #[test]
    fn next_enter_world_updates_current_visit_without_finalized_visit() {
        let mut visit_session = VisitSession::new();

        let actions = process_test_lines(
            &mut visit_session,
            vec![
                "2026.06.10 20:00:00 Log        -  [Behaviour] Entering Room: World A wrld_a:12345",
                "2026.06.10 20:30:00 Log        -  [Behaviour] Entering Room: World B wrld_b:67890",
            ],
        );

        assert!(finalized_visits(&actions).is_empty());

        let changes = current_visit_changes(&actions);
        assert_eq!(changes.len(), 2);
        assert_eq!(
            changes[1]
                .as_ref()
                .expect("current visit should exist")
                .world_name,
            "World B"
        );

        assert_eq!(
            visit_session
                .current_visit()
                .expect("current visit should exist")
                .world_name,
            "World B"
        );
    }

    #[test]
    fn duplicate_enter_world_within_five_seconds_updates_current_visit_without_finalized_visit() {
        let mut visit_session = VisitSession::new();

        let actions = process_test_lines(
            &mut visit_session,
            vec![
                "2026.06.10 20:00:00 Log        -  [Behaviour] Entering Room: World A wrld_a:12345",
                "2026.06.10 20:00:04 Log        -  [Behaviour] Entering Room: World A wrld_a:12345~hidden(usr)~nonce(abc)",
            ],
        );

        let current_visit = visit_session
            .current_visit()
            .expect("current visit should remain");

        assert!(finalized_visits(&actions).is_empty());
        assert_eq!(current_visit.world_name, "World A");
        assert_eq!(
            current_visit.instance_access_type,
            Some("hidden".to_string())
        );
        assert_eq!(current_visit.instance_nonce, Some("abc".to_string()));
    }

    #[test]
    fn visit_metadata_updates_current_visit_without_finalized_visit() {
        let mut visit_session = VisitSession::new();

        let actions = process_test_lines(
            &mut visit_session,
            vec![
                "2026.06.10 20:00:00 Log        -  [Behaviour] Entering Room: World A",
                "2026.06.10 20:00:02 Log        -  [Behaviour] Joining wrld_a:12345~private(usr)~nonce(abc)",
            ],
        );

        let current_visit = visit_session
            .current_visit()
            .expect("current visit should remain");

        assert!(finalized_visits(&actions).is_empty());
        assert_eq!(current_visit.world_id, Some("wrld_a".to_string()));
        assert_eq!(
            current_visit.instance_id,
            Some("12345~private(usr)~nonce(abc)".to_string())
        );
        assert_eq!(current_visit.instance_nonce, Some("abc".to_string()));
    }

    #[test]
    fn leave_world_finalizes_current_visit_and_clears_current_visit() {
        let mut visit_session = VisitSession::new();

        let actions = process_test_lines(
            &mut visit_session,
            vec![
                "2026.06.10 20:00:00 Log        -  [Behaviour] Entering Room: World A wrld_a:12345",
                "2026.06.10 20:10:00 Log        -  [Behaviour] OnLeftRoom",
            ],
        );

        let finalized = finalized_visits(&actions);
        assert_eq!(finalized.len(), 1);
        assert_eq!(finalized[0].world_name, "World A");
        assert_eq!(finalized[0].stay_duration_seconds, 600);
        assert!(visit_session.current_visit().is_none());

        let changes = current_visit_changes(&actions);
        assert!(changes.iter().any(|change| change.is_none()));
    }

    #[test]
    fn application_quit_finalizes_current_visit_and_clears_current_visit() {
        let mut visit_session = VisitSession::new();

        let actions = process_test_lines(
            &mut visit_session,
            vec![
                "2026.06.10 20:00:00 Log        -  [Behaviour] Entering Room: World A wrld_a:12345",
                "2026.06.10 20:10:00 Debug      -  VRCApplication: HandleApplicationQuit at 600.000",
            ],
        );

        let finalized = finalized_visits(&actions);
        assert_eq!(finalized.len(), 1);
        assert_eq!(finalized[0].world_name, "World A");
        assert_eq!(finalized[0].stay_duration_seconds, 600);
        assert!(visit_session.current_visit().is_none());

        let changes = current_visit_changes(&actions);
        assert!(changes.iter().any(|change| change.is_none()));
    }
}
