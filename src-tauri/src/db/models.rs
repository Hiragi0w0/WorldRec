#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct VisitHistory {
    pub id: i64,
    pub visited_at: String,
    pub world_name: String,
    pub world_id: Option<String>,
    pub instance_id: Option<String>,
    pub instance_access_type: Option<String>,
    pub instance_nonce: Option<String>,
    pub instance_raw_tags: Option<String>,
    pub stay_duration_seconds: Option<i64>,
    pub memo: Option<String>,
    pub tags: Option<String>,
    pub source_log_file: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct VisitInput {
    pub visited_at: String,
    pub world_name: String,
    pub world_id: Option<String>,
    pub instance_id: Option<String>,
    pub instance_access_type: Option<String>,
    pub instance_nonce: Option<String>,
    pub instance_raw_tags: Option<String>,
    pub stay_duration_seconds: i64,
    pub source_log_file: Option<String>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct WorldPreviewCache {
    pub world_id: String,
    pub world_name: String,
    pub image_url: Option<String>,
    pub thumbnail_image_url: Option<String>,
    pub author_name: Option<String>,
    pub description: Option<String>,
    pub fetched_at: Option<String>,
    pub fetch_failed_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct WorldPreviewUpsert {
    pub world_id: String,
    pub world_name: String,
    pub image_url: Option<String>,
    pub thumbnail_image_url: Option<String>,
    pub author_name: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PendingVisit {
    pub visited_at: String,
    pub world_name: String,
    pub world_id: Option<String>,
    pub instance_id: Option<String>,
    pub instance_access_type: Option<String>,
    pub instance_nonce: Option<String>,
    pub instance_raw_tags: Option<String>,
    pub source_log_file: Option<String>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct DuplicateVisitKey {
    pub visit_key: String,
    pub source_log_file: Option<String>,
    pub visited_at: String,
    pub count: i64,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct DuplicateVisitRow {
    pub id: i64,
    pub visited_at: String,
    pub world_name: String,
    pub world_id: Option<String>,
    pub stay_duration_seconds: Option<i64>,
    pub memo: Option<String>,
    pub tags: Option<String>,
    pub source_log_file: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct DuplicateVisitCleanupPlan {
    pub visit_key: String,
    pub visited_at: String,
    pub keep_id: i64,
    pub delete_ids: Vec<i64>,
}

impl PendingVisit {
    pub fn into_visit_input(self, stay_duration_seconds: i64) -> VisitInput {
        VisitInput {
            visited_at: self.visited_at,
            world_name: self.world_name,
            world_id: self.world_id,
            instance_id: self.instance_id,
            instance_access_type: self.instance_access_type,
            instance_nonce: self.instance_nonce,
            instance_raw_tags: self.instance_raw_tags,
            stay_duration_seconds,
            source_log_file: self.source_log_file,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_visit_can_be_converted_into_visit_input() {
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

        let visit_input = pending_visit.into_visit_input(3600);

        assert_eq!(visit_input.visited_at, "2026-06-09T20:00:00+09:00");
        assert_eq!(visit_input.world_name, "Example World");
        assert_eq!(visit_input.world_id, Some("wrld_example".to_string()));
        assert_eq!(visit_input.instance_id, Some("12345".to_string()));
        assert_eq!(visit_input.instance_access_type, Some("public".to_string()));
        assert_eq!(visit_input.instance_nonce, None);
        assert_eq!(visit_input.instance_raw_tags, None);
        assert_eq!(visit_input.stay_duration_seconds, 3600);
        assert_eq!(
            visit_input.source_log_file,
            Some("output_log_2026-06-09.txt".to_string())
        );
    }
}
