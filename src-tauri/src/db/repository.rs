#[cfg(test)]
use crate::db::models::VisitHistory;
use crate::db::models::{
    DuplicateVisitCleanupPlan, DuplicateVisitKey, DuplicateVisitRow, VisitInput, WorldPreviewCache,
    WorldPreviewUpsert,
};
use rusqlite::{params, params_from_iter, types::Value, Connection, OptionalExtension};
use std::collections::HashSet;

const LIBRARY_TAG_SEPARATOR: char = '\u{1f}';

// 重複判定キーは source_log_file を含める。別ログファイル（別セッション・別インスタンス）の
// 履歴を誤って同一視しないようにするため。
const VISIT_EVENT_KEY_SELECT: &str = "
    COALESCE(world_id, world_name) AS visit_key,
    source_log_file,
    visited_at
";

const VISIT_EVENT_KEY_GROUP_BY: &str = "
    COALESCE(world_id, world_name),
    source_log_file,
    visited_at
";

pub fn add_visit_if_missing(connection: &Connection, visit: &VisitInput) -> rusqlite::Result<bool> {
    if visit_exists(connection, visit)? {
        return Ok(false);
    }

    let inserted = connection.execute(
        "
        INSERT OR IGNORE INTO visit_histories (
            visited_at,
            world_name,
            world_id,
            instance_id,
            instance_access_type,
            instance_nonce,
            instance_raw_tags,
            stay_duration_seconds,
            memo,
            tags,
            source_log_file,
            created_at,
            updated_at
        ) VALUES (
            ?1,
            ?2,
            ?3,
            ?4,
            ?5,
            ?6,
            ?7,
            ?8,
            NULL,
            NULL,
            ?9,
            CURRENT_TIMESTAMP,
            CURRENT_TIMESTAMP
        )
        ",
        params![
            visit.visited_at,
            visit.world_name,
            visit.world_id,
            visit.instance_id,
            visit.instance_access_type,
            visit.instance_nonce,
            visit.instance_raw_tags,
            visit.stay_duration_seconds,
            visit.source_log_file,
        ],
    )?;

    Ok(inserted > 0)
}

#[cfg(test)]
pub fn count_visit_histories(connection: &Connection) -> rusqlite::Result<i64> {
    connection.query_row(
        "
        SELECT COUNT(*)
        FROM visit_histories
        ",
        [],
        |row| row.get(0),
    )
}

pub fn get_world_preview_cache(
    connection: &Connection,
    world_id: &str,
) -> rusqlite::Result<Option<WorldPreviewCache>> {
    connection
        .query_row(
            "
            SELECT
                world_id,
                world_name,
                image_url,
                thumbnail_image_url,
                author_name,
                description,
                fetched_at,
                fetch_failed_at,
                created_at,
                updated_at
            FROM world_preview_cache
            WHERE world_id = ?1
            ",
            params![world_id],
            |row| {
                Ok(WorldPreviewCache {
                    world_id: row.get(0)?,
                    world_name: row.get(1)?,
                    image_url: row.get(2)?,
                    thumbnail_image_url: row.get(3)?,
                    author_name: row.get(4)?,
                    description: row.get(5)?,
                    fetched_at: row.get(6)?,
                    fetch_failed_at: row.get(7)?,
                    created_at: row.get(8)?,
                    updated_at: row.get(9)?,
                })
            },
        )
        .optional()
}

pub fn upsert_world_preview_cache(
    connection: &Connection,
    input: &WorldPreviewUpsert,
    now: &str,
) -> rusqlite::Result<()> {
    connection.execute(
        "
        INSERT INTO world_preview_cache (
            world_id,
            world_name,
            image_url,
            thumbnail_image_url,
            author_name,
            description,
            fetched_at,
            fetch_failed_at,
            created_at,
            updated_at
        ) VALUES (
            ?1,
            ?2,
            ?3,
            ?4,
            ?5,
            ?6,
            ?7,
            NULL,
            ?7,
            ?7
        )
        ON CONFLICT(world_id) DO UPDATE SET
            world_name = excluded.world_name,
            image_url = excluded.image_url,
            thumbnail_image_url = excluded.thumbnail_image_url,
            author_name = excluded.author_name,
            description = excluded.description,
            fetched_at = excluded.fetched_at,
            fetch_failed_at = NULL,
            updated_at = excluded.updated_at
        ",
        params![
            input.world_id,
            input.world_name,
            input.image_url,
            input.thumbnail_image_url,
            input.author_name,
            input.description,
            now,
        ],
    )?;

    Ok(())
}

pub fn mark_world_preview_fetch_failed(
    connection: &Connection,
    world_id: &str,
    world_name: &str,
    now: &str,
) -> rusqlite::Result<()> {
    connection.execute(
        "
        INSERT INTO world_preview_cache (
            world_id,
            world_name,
            fetch_failed_at,
            created_at,
            updated_at
        ) VALUES (
            ?1,
            ?2,
            ?3,
            ?3,
            ?3
        )
        ON CONFLICT(world_id) DO UPDATE SET
            fetch_failed_at = excluded.fetch_failed_at,
            updated_at = excluded.updated_at
        ",
        params![world_id, world_name, now],
    )?;

    Ok(())
}

pub fn count_duplicate_visit_keys(connection: &Connection) -> rusqlite::Result<i64> {
    let sql = format!(
        "
        SELECT COUNT(*)
        FROM (
            SELECT
                {VISIT_EVENT_KEY_SELECT}
            FROM visit_histories
            GROUP BY
                {VISIT_EVENT_KEY_GROUP_BY}
            HAVING COUNT(*) > 1
        )
        "
    );

    connection.query_row(&sql, [], |row| row.get(0))
}

#[cfg(test)]
pub fn count_duplicate_visit_rows_to_delete(connection: &Connection) -> rusqlite::Result<i64> {
    let sql = format!(
        "
        SELECT COALESCE(SUM(duplicate_count - 1), 0)
        FROM (
            SELECT COUNT(*) AS duplicate_count
            FROM visit_histories
            GROUP BY
                {VISIT_EVENT_KEY_GROUP_BY}
            HAVING COUNT(*) > 1
        )
        "
    );

    connection.query_row(&sql, [], |row| row.get(0))
}

pub fn list_duplicate_visit_keys(
    connection: &Connection,
    limit: i64,
) -> rusqlite::Result<Vec<DuplicateVisitKey>> {
    let sql = format!(
        "
        SELECT
            {VISIT_EVENT_KEY_SELECT},
            COUNT(*) AS duplicate_count
        FROM visit_histories
        GROUP BY
            {VISIT_EVENT_KEY_GROUP_BY}
        HAVING COUNT(*) > 1
        ORDER BY
            duplicate_count DESC,
            visited_at DESC
        LIMIT ?1
        "
    );

    let mut statement = connection.prepare(&sql)?;

    let rows = statement.query_map(params![limit], |row| {
        Ok(DuplicateVisitKey {
            visit_key: row.get(0)?,
            source_log_file: row.get(1)?,
            visited_at: row.get(2)?,
            count: row.get(3)?,
        })
    })?;

    let mut duplicate_keys = Vec::new();

    for row in rows {
        duplicate_keys.push(row?);
    }

    Ok(duplicate_keys)
}

pub fn list_duplicate_visit_rows(
    connection: &Connection,
    duplicate_key: &DuplicateVisitKey,
) -> rusqlite::Result<Vec<DuplicateVisitRow>> {
    let mut statement = connection.prepare(
        "
        SELECT
            id,
            visited_at,
            world_name,
            world_id,
            stay_duration_seconds,
            memo,
            tags,
            source_log_file,
            created_at,
            updated_at
        FROM visit_histories
        WHERE COALESCE(world_id, world_name) = ?1
          AND COALESCE(source_log_file, '') = COALESCE(?2, '')
          AND visited_at = ?3
        ORDER BY
            stay_duration_seconds IS NULL ASC,
            stay_duration_seconds DESC,
            id DESC
        ",
    )?;

    let rows = statement.query_map(
        params![
            duplicate_key.visit_key,
            duplicate_key.source_log_file,
            duplicate_key.visited_at
        ],
        |row| {
            Ok(DuplicateVisitRow {
                id: row.get(0)?,
                visited_at: row.get(1)?,
                world_name: row.get(2)?,
                world_id: row.get(3)?,
                stay_duration_seconds: row.get(4)?,
                memo: row.get(5)?,
                tags: row.get(6)?,
                source_log_file: row.get(7)?,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        },
    )?;

    let mut duplicate_rows = Vec::new();

    for row in rows {
        duplicate_rows.push(row?);
    }

    Ok(duplicate_rows)
}

pub fn select_duplicate_visit_row_id_to_keep(rows: &[DuplicateVisitRow]) -> Option<i64> {
    rows.iter()
        .max_by_key(|row| duplicate_visit_row_score(row))
        .map(|row| row.id)
}

pub fn select_duplicate_visit_row_ids_to_delete(rows: &[DuplicateVisitRow]) -> Vec<i64> {
    let Some(keep_id) = select_duplicate_visit_row_id_to_keep(rows) else {
        return Vec::new();
    };

    rows.iter()
        .filter(|row| row.id != keep_id)
        .map(|row| row.id)
        .collect()
}

pub fn list_duplicate_visit_cleanup_plans(
    connection: &Connection,
    limit: i64,
) -> rusqlite::Result<Vec<DuplicateVisitCleanupPlan>> {
    let duplicate_keys = list_duplicate_visit_keys(connection, limit)?;

    let mut cleanup_plans = Vec::new();

    for duplicate_key in duplicate_keys {
        let duplicate_rows = list_duplicate_visit_rows(connection, &duplicate_key)?;

        let Some(keep_id) = select_duplicate_visit_row_id_to_keep(&duplicate_rows) else {
            continue;
        };

        let delete_ids = select_duplicate_visit_row_ids_to_delete(&duplicate_rows);

        if delete_ids.is_empty() {
            continue;
        }

        cleanup_plans.push(DuplicateVisitCleanupPlan {
            visit_key: duplicate_key.visit_key,
            visited_at: duplicate_key.visited_at,
            keep_id,
            delete_ids,
        });
    }

    Ok(cleanup_plans)
}

pub fn list_duplicate_visit_row_ids_to_delete(
    connection: &Connection,
    limit: i64,
) -> rusqlite::Result<Vec<i64>> {
    let cleanup_plans = list_duplicate_visit_cleanup_plans(connection, limit)?;

    let delete_ids = cleanup_plans
        .into_iter()
        .flat_map(|plan| plan.delete_ids)
        .collect();

    Ok(delete_ids)
}

#[cfg(test)]
pub fn count_visit_histories_by_ids(connection: &Connection, ids: &[i64]) -> rusqlite::Result<i64> {
    if ids.is_empty() {
        return Ok(0);
    }

    let mut total_count = 0;

    for chunk in ids.chunks(900) {
        let placeholders = build_placeholders(chunk.len());

        let sql = format!(
            "
            SELECT COUNT(*)
            FROM visit_histories
            WHERE id IN ({})
            ",
            placeholders
        );

        let count: i64 =
            connection.query_row(&sql, params_from_iter(chunk.iter()), |row| row.get(0))?;

        total_count += count;
    }

    Ok(total_count)
}

pub fn delete_visit_histories_by_ids(
    connection: &Connection,
    ids: &[i64],
) -> rusqlite::Result<usize> {
    if ids.is_empty() {
        return Ok(0);
    }

    let mut deleted_count = 0;

    for chunk in ids.chunks(900) {
        let placeholders = build_placeholders(chunk.len());

        let sql = format!(
            "
            DELETE FROM visit_histories
            WHERE id IN ({})
            ",
            placeholders
        );

        let deleted = connection.execute(&sql, params_from_iter(chunk.iter()))?;

        deleted_count += deleted;
    }

    Ok(deleted_count)
}

fn build_placeholders(length: usize) -> String {
    std::iter::repeat("?")
        .take(length)
        .collect::<Vec<_>>()
        .join(", ")
}

fn duplicate_visit_row_score(row: &DuplicateVisitRow) -> (i64, i64, i64, i64, i64) {
    let memo_score = has_text(&row.memo);
    let tags_score = has_text(&row.tags);
    let stay_duration_exists_score = if row.stay_duration_seconds.is_some() {
        1
    } else {
        0
    };
    let stay_duration_value = row.stay_duration_seconds.unwrap_or(-1);

    (
        memo_score,
        tags_score,
        stay_duration_exists_score,
        stay_duration_value,
        row.id,
    )
}

fn has_text(value: &Option<String>) -> i64 {
    match value {
        Some(text) if !text.trim().is_empty() => 1,
        _ => 0,
    }
}

fn visit_exists(connection: &Connection, visit: &VisitInput) -> rusqlite::Result<bool> {
    if visit_exists_by_visit_key(connection, visit)? {
        return Ok(true);
    }

    // source_log_file も判定に含める。別ログファイル（別セッション）の履歴を
    // 同一視して保存を抑止しないようにするため。
    let count: i64 = match &visit.world_id {
        Some(world_id) => connection.query_row(
            "
            SELECT COUNT(*)
            FROM visit_histories
            WHERE world_id = ?1
              AND visited_at = ?2
              AND COALESCE(source_log_file, '') = COALESCE(?3, '')
            ",
            params![world_id, visit.visited_at, visit.source_log_file],
            |row| row.get(0),
        )?,
        None => connection.query_row(
            "
            SELECT COUNT(*)
            FROM visit_histories
            WHERE world_id IS NULL
              AND world_name = ?1
              AND visited_at = ?2
              AND COALESCE(source_log_file, '') = COALESCE(?3, '')
            ",
            params![visit.world_name, visit.visited_at, visit.source_log_file],
            |row| row.get(0),
        )?,
    };

    Ok(count > 0)
}

fn visit_exists_by_visit_key(
    connection: &Connection,
    visit: &VisitInput,
) -> rusqlite::Result<bool> {
    let visit_key = visit.world_id.as_ref().unwrap_or(&visit.world_name);

    let count: i64 = connection.query_row(
        "
        SELECT COUNT(*)
        FROM visit_histories
        WHERE COALESCE(world_id, world_name) = ?1
          AND visited_at = ?2
          AND COALESCE(source_log_file, '') = COALESCE(?3, '')
        ",
        params![visit_key, visit.visited_at, visit.source_log_file],
        |row| row.get(0),
    )?;

    Ok(count > 0)
}

#[cfg(test)]
pub fn list_recent_visit_histories(
    connection: &Connection,
    limit: i64,
) -> rusqlite::Result<Vec<VisitHistory>> {
    let mut statement = connection.prepare(
        "
        SELECT
            id,
            visited_at,
            world_name,
            world_id,
            instance_id,
            instance_access_type,
            instance_nonce,
            instance_raw_tags,
            stay_duration_seconds,
            memo,
            tags,
            source_log_file,
            created_at,
            updated_at
        FROM visit_histories
        ORDER BY visited_at DESC, id DESC
        LIMIT ?1
        ",
    )?;

    let rows = statement.query_map(params![limit], |row| {
        Ok(VisitHistory {
            id: row.get(0)?,
            visited_at: row.get(1)?,
            world_name: row.get(2)?,
            world_id: row.get(3)?,
            instance_id: row.get(4)?,
            instance_access_type: row.get(5)?,
            instance_nonce: row.get(6)?,
            instance_raw_tags: row.get(7)?,
            stay_duration_seconds: row.get(8)?,
            memo: row.get(9)?,
            tags: row.get(10)?,
            source_log_file: row.get(11)?,
            created_at: row.get(12)?,
            updated_at: row.get(13)?,
        })
    })?;

    let mut visits = Vec::new();

    for row in rows {
        visits.push(row?);
    }

    Ok(visits)
}

pub struct LibraryQueryCriteria {
    pub query: Option<String>,
    pub visited_from: Option<String>,
    pub visited_to: Option<String>,
    pub tags: Vec<String>,
    pub memo_query: Option<String>,
    pub sort_key: String,
    pub sort_direction: String,
    pub limit: i64,
    pub offset: i64,
}

pub struct LibraryWorldRow {
    pub key: String,
    pub world_id: Option<String>,
    pub world_name: String,
    pub thumbnail_url: Option<String>,
    pub image_url: Option<String>,
    pub world_preview_fetched_at: Option<String>,
    pub visit_count: i64,
    pub first_visited_at: String,
    pub last_visited_at: String,
    pub total_stay_duration_seconds: i64,
    pub tags: Vec<String>,
    pub memo_count: i64,
}

pub struct LibraryWorldVisitRow {
    pub id: i64,
    pub visited_at: String,
    pub stay_duration_seconds: Option<i64>,
    pub memo: Option<String>,
}

pub fn list_library_worlds(
    connection: &Connection,
    criteria: &LibraryQueryCriteria,
) -> rusqlite::Result<(Vec<LibraryWorldRow>, i64)> {
    let (where_sql, params) = build_library_world_filters(criteria);
    let order_by = library_world_order_by(&criteria.sort_key, &criteria.sort_direction);

    let count_sql = format!(
        "{} SELECT COUNT(*) FROM agg JOIN latest ON latest.group_key = agg.group_key{}",
        library_world_cte_sql(),
        where_sql
    );
    let total_count: i64 =
        connection.query_row(&count_sql, params_from_iter(params.clone()), |row| {
            row.get(0)
        })?;

    let query_sql = format!(
        "{} \
         SELECT \
             agg.group_key, \
             latest.world_id, \
             latest.world_name, \
             w.thumbnail_image_url AS thumbnail_url, \
             w.image_url AS image_url, \
             w.fetched_at AS world_preview_fetched_at, \
             agg.visit_count, \
             agg.first_visited_at, \
             agg.last_visited_at, \
             agg.total_stay_duration_seconds, \
             agg.all_tags, \
             agg.memo_count \
         FROM agg \
         JOIN latest ON latest.group_key = agg.group_key \
         LEFT JOIN world_preview_cache w ON w.world_id = latest.world_id \
         {} \
         {} \
         LIMIT ? OFFSET ?",
        library_world_cte_sql(),
        where_sql,
        order_by
    );

    let mut query_params = params;
    query_params.push(Value::Integer(criteria.limit));
    query_params.push(Value::Integer(criteria.offset));

    let mut statement = connection.prepare(&query_sql)?;
    let rows = statement.query_map(params_from_iter(query_params), library_world_from_row)?;

    let mut worlds = Vec::new();
    for row in rows {
        worlds.push(row?);
    }

    Ok((worlds, total_count))
}

pub fn get_library_world_group(
    connection: &Connection,
    world_id: Option<&str>,
    world_name: &str,
) -> rusqlite::Result<Option<(LibraryWorldRow, Vec<LibraryWorldVisitRow>)>> {
    let (target_where, params) = match world_id {
        Some(world_id) => ("world_id = ?", vec![Value::Text(world_id.to_string())]),
        None => (
            "world_id IS NULL AND LOWER(TRIM(world_name)) = LOWER(TRIM(?))",
            vec![Value::Text(world_name.to_string())],
        ),
    };

    let world_sql = format!(
        "
        WITH
        target AS (
            SELECT
                id,
                visited_at,
                world_name,
                world_id,
                stay_duration_seconds,
                memo,
                tags,
                COALESCE(world_id, LOWER(TRIM(world_name))) AS group_key
            FROM visit_histories
            WHERE {target_where}
        ),
        latest AS (
            SELECT group_key, world_id, world_name
            FROM (
                SELECT
                    group_key,
                    world_id,
                    world_name,
                    ROW_NUMBER() OVER (
                        PARTITION BY group_key
                        ORDER BY visited_at DESC, id DESC
                    ) AS rn
                FROM target
            )
            WHERE rn = 1
        ),
        agg AS (
            SELECT
                group_key,
                COUNT(*) AS visit_count,
                MIN(visited_at) AS first_visited_at,
                MAX(visited_at) AS last_visited_at,
                COALESCE(SUM(COALESCE(stay_duration_seconds, 0)), 0) AS total_stay_duration_seconds,
                GROUP_CONCAT(COALESCE(tags, ''), CHAR(31)) AS all_tags,
                SUM(CASE WHEN memo IS NOT NULL AND TRIM(memo) <> '' THEN 1 ELSE 0 END) AS memo_count
            FROM target
            GROUP BY group_key
        )
        SELECT
            agg.group_key,
            latest.world_id,
            latest.world_name,
            w.thumbnail_image_url AS thumbnail_url,
            w.image_url AS image_url,
            w.fetched_at AS world_preview_fetched_at,
            agg.visit_count,
            agg.first_visited_at,
            agg.last_visited_at,
            agg.total_stay_duration_seconds,
            agg.all_tags,
            agg.memo_count
        FROM agg
        JOIN latest ON latest.group_key = agg.group_key
        LEFT JOIN world_preview_cache w ON w.world_id = latest.world_id
        "
    );

    let world = connection
        .query_row(
            &world_sql,
            params_from_iter(params.clone()),
            library_world_from_row,
        )
        .optional()?;

    let Some(world) = world else {
        return Ok(None);
    };

    let visits_sql = format!(
        "
        SELECT id, visited_at, stay_duration_seconds, memo
        FROM visit_histories
        WHERE {target_where}
        ORDER BY visited_at DESC, id DESC
        "
    );
    let mut statement = connection.prepare(&visits_sql)?;
    let rows = statement.query_map(params_from_iter(params), |row| {
        Ok(LibraryWorldVisitRow {
            id: row.get(0)?,
            visited_at: row.get(1)?,
            stay_duration_seconds: row.get(2)?,
            memo: row.get(3)?,
        })
    })?;

    let mut visits = Vec::new();
    for row in rows {
        visits.push(row?);
    }

    Ok(Some((world, visits)))
}

fn library_world_cte_sql() -> &'static str {
    "
    WITH
    base AS (
        SELECT
            id,
            visited_at,
            world_name,
            world_id,
            stay_duration_seconds,
            memo,
            tags,
            COALESCE(world_id, LOWER(TRIM(world_name))) AS group_key
        FROM visit_histories
    ),
    latest AS (
        SELECT group_key, world_id, world_name
        FROM (
            SELECT
                group_key,
                world_id,
                world_name,
                ROW_NUMBER() OVER (
                    PARTITION BY group_key
                    ORDER BY visited_at DESC, id DESC
                ) AS rn
            FROM base
        )
        WHERE rn = 1
    ),
    agg AS (
        SELECT
            group_key,
            COUNT(*) AS visit_count,
            MIN(visited_at) AS first_visited_at,
            MAX(visited_at) AS last_visited_at,
            COALESCE(SUM(COALESCE(stay_duration_seconds, 0)), 0) AS total_stay_duration_seconds,
            GROUP_CONCAT(COALESCE(tags, ''), CHAR(31)) AS all_tags,
            SUM(CASE WHEN memo IS NOT NULL AND TRIM(memo) <> '' THEN 1 ELSE 0 END) AS memo_count
        FROM base
        GROUP BY group_key
    )
    "
}

fn build_library_world_filters(criteria: &LibraryQueryCriteria) -> (String, Vec<Value>) {
    let mut clauses = Vec::new();
    let mut params = Vec::new();

    if let Some(query) = trimmed_text(criteria.query.as_deref()) {
        clauses.push(
            "EXISTS (
                SELECT 1
                FROM base b2
                WHERE b2.group_key = agg.group_key
                  AND (b2.world_name LIKE ? OR COALESCE(b2.world_id, '') LIKE ?)
            )"
            .to_string(),
        );
        let pattern = format!("%{query}%");
        params.push(Value::Text(pattern.clone()));
        params.push(Value::Text(pattern));
    }

    let mut range_parts = Vec::new();
    if let Some(visited_from) = trimmed_text(criteria.visited_from.as_deref()) {
        range_parts.push("b2.visited_at >= ?");
        params.push(Value::Text(visited_from));
    }
    if let Some(visited_to) = trimmed_text(criteria.visited_to.as_deref()) {
        range_parts.push("b2.visited_at < ?");
        params.push(Value::Text(visited_to));
    }
    if !range_parts.is_empty() {
        clauses.push(format!(
            "EXISTS (
                SELECT 1
                FROM base b2
                WHERE b2.group_key = agg.group_key
                  AND {}
            )",
            range_parts.join(" AND ")
        ));
    }

    for tag in criteria
        .tags
        .iter()
        .filter_map(|tag| trimmed_text(Some(tag)))
    {
        clauses.push(
            "EXISTS (
                SELECT 1
                FROM base b2
                WHERE b2.group_key = agg.group_key
                  AND COALESCE(b2.tags, '') LIKE ?
            )"
            .to_string(),
        );
        params.push(Value::Text(format!("%{tag}%")));
    }

    if let Some(memo_query) = trimmed_text(criteria.memo_query.as_deref()) {
        clauses.push(
            "EXISTS (
                SELECT 1
                FROM base b2
                WHERE b2.group_key = agg.group_key
                  AND LOWER(COALESCE(b2.memo, '')) LIKE ?
            )"
            .to_string(),
        );
        params.push(Value::Text(format!("%{}%", memo_query.to_lowercase())));
    }

    if clauses.is_empty() {
        (String::new(), params)
    } else {
        (format!(" WHERE {}", clauses.join(" AND ")), params)
    }
}

fn library_world_order_by(sort_key: &str, sort_direction: &str) -> &'static str {
    match (sort_key, sort_direction) {
        ("visit_count", "desc") => {
            "ORDER BY agg.visit_count DESC, latest.world_name COLLATE NOCASE ASC"
        }
        ("visit_count", _) => "ORDER BY agg.visit_count ASC, latest.world_name COLLATE NOCASE ASC",
        ("total_stay_duration_seconds", "desc") => {
            "ORDER BY agg.total_stay_duration_seconds DESC, latest.world_name COLLATE NOCASE ASC"
        }
        ("total_stay_duration_seconds", _) => {
            "ORDER BY agg.total_stay_duration_seconds ASC, latest.world_name COLLATE NOCASE ASC"
        }
        ("world_name", "desc") => "ORDER BY latest.world_name COLLATE NOCASE DESC",
        _ => "ORDER BY latest.world_name COLLATE NOCASE ASC",
    }
}

fn library_world_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<LibraryWorldRow> {
    let raw_tags: Option<String> = row.get("all_tags")?;

    Ok(LibraryWorldRow {
        key: row.get("group_key")?,
        world_id: row.get("world_id")?,
        world_name: row.get("world_name")?,
        thumbnail_url: row.get("thumbnail_url")?,
        image_url: row.get("image_url")?,
        world_preview_fetched_at: row.get("world_preview_fetched_at")?,
        visit_count: row.get("visit_count")?,
        first_visited_at: row.get("first_visited_at")?,
        last_visited_at: row.get("last_visited_at")?,
        total_stay_duration_seconds: row.get("total_stay_duration_seconds")?,
        tags: split_library_tags(raw_tags.as_deref()),
        memo_count: row.get("memo_count")?,
    })
}

fn split_library_tags(raw_tags: Option<&str>) -> Vec<String> {
    let Some(raw_tags) = raw_tags else {
        return Vec::new();
    };

    let mut tags = Vec::new();
    let mut seen = HashSet::new();

    for tag in raw_tags
        .split(LIBRARY_TAG_SEPARATOR)
        .flat_map(|part| {
            part.split(|character: char| {
                character == ',' || character == '、' || character.is_whitespace()
            })
        })
        .filter_map(|tag| trimmed_text(Some(tag)))
    {
        if seen.insert(tag.to_lowercase()) {
            tags.push(tag);
        }
    }

    tags
}

fn trimmed_text(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub fn delete_all_visit_histories(connection: &Connection) -> rusqlite::Result<usize> {
    connection.execute("DELETE FROM visit_histories", [])
}

pub fn delete_visit_history_by_id(
    connection: &Connection,
    visit_id: i64,
) -> rusqlite::Result<usize> {
    connection.execute(
        "DELETE FROM visit_histories WHERE id = ?1",
        params![visit_id],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::open_database;
    use crate::db::schema::initialize_database;
    use std::path::Path;

    // 実 DB を参照する `#[ignore]` テスト用のパス。個人環境に依存しないよう
    // 環境変数 WORLDREC_TEST_DB で既存 DB のパスを与える。
    fn existing_worldrec_db_path() -> std::path::PathBuf {
        std::path::PathBuf::from(
            std::env::var_os("WORLDREC_TEST_DB")
                .expect("set WORLDREC_TEST_DB to the path of an existing WorldRec database"),
        )
    }

    fn make_visit_input() -> VisitInput {
        VisitInput {
            visited_at: "2026-06-09T20:00:00+09:00".to_string(),
            world_name: "Example World".to_string(),
            world_id: Some("wrld_example".to_string()),
            instance_id: Some("12345".to_string()),
            instance_access_type: Some("public".to_string()),
            instance_nonce: None,
            instance_raw_tags: None,
            stay_duration_seconds: 3600,
            source_log_file: Some("output_log_2026-06-09.txt".to_string()),
        }
    }

    fn create_legacy_visit_histories_table(connection: &Connection) {
        connection
            .execute_batch(
                "
                CREATE TABLE visit_histories
                (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    visited_at TEXT NOT NULL,
                    world_name TEXT NOT NULL,
                    world_id TEXT NULL,
                    instance_id TEXT NULL,
                    instance_access_type TEXT NULL,
                    instance_nonce TEXT NULL,
                    instance_raw_tags TEXT NULL,
                    stay_duration_seconds INTEGER NULL,
                    memo TEXT NULL,
                    tags TEXT NULL,
                    source_log_file TEXT NULL,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );
                ",
            )
            .expect("legacy visit_histories table should be created");
    }

    fn setup_library_test_database() -> Connection {
        let connection =
            open_database(Path::new(":memory:")).expect("in-memory database should open");

        initialize_database(&connection).expect("database schema should be initialized");

        connection
    }

    #[test]
    fn upserts_world_preview_cache_and_clears_failed_timestamp() {
        let connection = setup_library_test_database();

        mark_world_preview_fetch_failed(
            &connection,
            "wrld_example",
            "Old World",
            "2026-06-23T01:00:00+09:00",
        )
        .expect("failed timestamp should be inserted");

        upsert_world_preview_cache(
            &connection,
            &WorldPreviewUpsert {
                world_id: "wrld_example".to_string(),
                world_name: "Example World".to_string(),
                image_url: Some("https://example.com/image.jpg".to_string()),
                thumbnail_image_url: Some("https://example.com/thumb.jpg".to_string()),
                author_name: Some("Example Author".to_string()),
                description: Some("Example Description".to_string()),
            },
            "2026-06-23T02:00:00+09:00",
        )
        .expect("preview cache should upsert");

        let cache = get_world_preview_cache(&connection, "wrld_example")
            .expect("preview cache should query")
            .expect("preview cache should exist");

        assert_eq!(cache.world_name, "Example World");
        assert_eq!(
            cache.image_url.as_deref(),
            Some("https://example.com/image.jpg")
        );
        assert_eq!(
            cache.thumbnail_image_url.as_deref(),
            Some("https://example.com/thumb.jpg")
        );
        assert_eq!(cache.author_name.as_deref(), Some("Example Author"));
        assert_eq!(cache.description.as_deref(), Some("Example Description"));
        assert_eq!(
            cache.fetched_at.as_deref(),
            Some("2026-06-23T02:00:00+09:00")
        );
        assert_eq!(cache.fetch_failed_at, None);
    }

    #[test]
    fn marking_world_preview_fetch_failed_preserves_success_cache() {
        let connection = setup_library_test_database();

        upsert_world_preview_cache(
            &connection,
            &WorldPreviewUpsert {
                world_id: "wrld_example".to_string(),
                world_name: "Example World".to_string(),
                image_url: Some("https://example.com/image.jpg".to_string()),
                thumbnail_image_url: Some("https://example.com/thumb.jpg".to_string()),
                author_name: Some("Example Author".to_string()),
                description: Some("Example Description".to_string()),
            },
            "2026-06-23T02:00:00+09:00",
        )
        .expect("preview cache should upsert");

        mark_world_preview_fetch_failed(
            &connection,
            "wrld_example",
            "Fallback World",
            "2026-06-23T03:00:00+09:00",
        )
        .expect("failed timestamp should update");

        let cache = get_world_preview_cache(&connection, "wrld_example")
            .expect("preview cache should query")
            .expect("preview cache should exist");

        assert_eq!(cache.world_name, "Example World");
        assert_eq!(
            cache.image_url.as_deref(),
            Some("https://example.com/image.jpg")
        );
        assert_eq!(
            cache.thumbnail_image_url.as_deref(),
            Some("https://example.com/thumb.jpg")
        );
        assert_eq!(
            cache.fetched_at.as_deref(),
            Some("2026-06-23T02:00:00+09:00")
        );
        assert_eq!(cache.description.as_deref(), Some("Example Description"));
        assert_eq!(
            cache.fetch_failed_at.as_deref(),
            Some("2026-06-23T03:00:00+09:00")
        );
    }

    fn default_library_criteria() -> LibraryQueryCriteria {
        LibraryQueryCriteria {
            query: None,
            visited_from: None,
            visited_to: None,
            tags: Vec::new(),
            memo_query: None,
            sort_key: "world_name".to_string(),
            sort_direction: "asc".to_string(),
            limit: 25,
            offset: 0,
        }
    }

    fn insert_library_visit(
        connection: &Connection,
        visited_at: &str,
        world_name: &str,
        world_id: Option<&str>,
        stay_duration_seconds: Option<i64>,
        memo: Option<&str>,
        tags: Option<&str>,
    ) {
        connection
            .execute(
                "
                INSERT INTO visit_histories (
                    visited_at,
                    world_name,
                    world_id,
                    instance_id,
                    instance_access_type,
                    instance_nonce,
                    instance_raw_tags,
                    stay_duration_seconds,
                    memo,
                    tags,
                    source_log_file,
                    created_at,
                    updated_at
                ) VALUES (
                    ?1,
                    ?2,
                    ?3,
                    NULL,
                    'public',
                    NULL,
                    NULL,
                    ?4,
                    ?5,
                    ?6,
                    'output_log_library_test.txt',
                    CURRENT_TIMESTAMP,
                    CURRENT_TIMESTAMP
                )
                ",
                params![
                    visited_at,
                    world_name,
                    world_id,
                    stay_duration_seconds,
                    memo,
                    tags
                ],
            )
            .expect("library visit should be inserted");
    }

    #[test]
    fn aggregates_library_world_by_world_id() {
        let connection = setup_library_test_database();

        insert_library_visit(
            &connection,
            "2026-06-09T20:00:00",
            "Old Name",
            Some("wrld_library"),
            Some(120),
            Some("first memo"),
            Some("Cozy, photo"),
        );
        insert_library_visit(
            &connection,
            "2026-06-10T20:00:00",
            "New Name",
            Some("wrld_library"),
            Some(180),
            Some("second memo"),
            Some("cozy、Night photo"),
        );

        let (worlds, total_count) = list_library_worlds(&connection, &default_library_criteria())
            .expect("library worlds should be listed");

        assert_eq!(total_count, 1);
        assert_eq!(worlds.len(), 1);
        assert_eq!(worlds[0].key, "wrld_library");
        assert_eq!(worlds[0].world_id, Some("wrld_library".to_string()));
        assert_eq!(worlds[0].world_name, "New Name");
        assert_eq!(worlds[0].visit_count, 2);
        assert_eq!(worlds[0].total_stay_duration_seconds, 300);
        assert_eq!(
            worlds[0].tags,
            vec!["Cozy".to_string(), "photo".to_string(), "Night".to_string()]
        );
        assert_eq!(worlds[0].memo_count, 2);
    }

    #[test]
    fn list_library_worlds_includes_world_preview_cache_fields() {
        let connection = setup_library_test_database();

        insert_library_visit(
            &connection,
            "2026-06-09T20:00:00",
            "Preview World",
            Some("wrld_preview"),
            Some(120),
            None,
            None,
        );
        upsert_world_preview_cache(
            &connection,
            &WorldPreviewUpsert {
                world_id: "wrld_preview".to_string(),
                world_name: "Preview World".to_string(),
                image_url: Some("https://example.com/image.jpg".to_string()),
                thumbnail_image_url: Some("https://example.com/thumb.jpg".to_string()),
                author_name: Some("Preview Author".to_string()),
                description: None,
            },
            "2026-06-23T21:00:00+09:00",
        )
        .expect("preview cache should upsert");

        let (worlds, total_count) = list_library_worlds(&connection, &default_library_criteria())
            .expect("library worlds should be listed");

        assert_eq!(total_count, 1);
        assert_eq!(worlds.len(), 1);
        assert_eq!(
            worlds[0].thumbnail_url.as_deref(),
            Some("https://example.com/thumb.jpg")
        );
        assert_eq!(
            worlds[0].image_url.as_deref(),
            Some("https://example.com/image.jpg")
        );
        assert_eq!(
            worlds[0].world_preview_fetched_at.as_deref(),
            Some("2026-06-23T21:00:00+09:00")
        );
    }

    #[test]
    fn aggregates_null_world_id_by_normalized_world_name() {
        let connection = setup_library_test_database();

        insert_library_visit(
            &connection,
            "2026-06-09T20:00:00",
            "  Null World  ",
            None,
            Some(60),
            None,
            None,
        );
        insert_library_visit(
            &connection,
            "2026-06-10T20:00:00",
            "null world",
            None,
            Some(90),
            None,
            None,
        );

        let (worlds, total_count) = list_library_worlds(&connection, &default_library_criteria())
            .expect("library worlds should be listed");

        assert_eq!(total_count, 1);
        assert_eq!(worlds[0].key, "null world");
        assert_eq!(worlds[0].world_id, None);
        assert_eq!(worlds[0].world_name, "null world");
        assert_eq!(worlds[0].visit_count, 2);
        assert_eq!(worlds[0].total_stay_duration_seconds, 150);
    }

    #[test]
    fn treats_null_stay_duration_as_zero_in_library_total() {
        let connection = setup_library_test_database();

        insert_library_visit(
            &connection,
            "2026-06-09T20:00:00",
            "Duration World",
            Some("wrld_duration"),
            None,
            None,
            None,
        );
        insert_library_visit(
            &connection,
            "2026-06-10T20:00:00",
            "Duration World",
            Some("wrld_duration"),
            Some(90),
            None,
            None,
        );

        let (worlds, _) = list_library_worlds(&connection, &default_library_criteria())
            .expect("library worlds should be listed");

        assert_eq!(worlds[0].total_stay_duration_seconds, 90);
    }

    #[test]
    fn filters_library_worlds_by_world_name_tags_and_memo_query() {
        let connection = setup_library_test_database();

        insert_library_visit(
            &connection,
            "2026-06-09T20:00:00",
            "Searchable World",
            Some("wrld_searchable"),
            Some(120),
            Some("Photo spot near spawn"),
            Some("Scenic, Night"),
        );
        insert_library_visit(
            &connection,
            "2026-06-10T20:00:00",
            "Other World",
            Some("wrld_other"),
            Some(180),
            Some("quiet area"),
            Some("Quest"),
        );

        let criteria = LibraryQueryCriteria {
            query: Some("Searchable".to_string()),
            tags: vec!["Night".to_string()],
            memo_query: Some("photo".to_string()),
            ..default_library_criteria()
        };
        let (worlds, total_count) =
            list_library_worlds(&connection, &criteria).expect("filtered worlds should be listed");

        assert_eq!(total_count, 1);
        assert_eq!(worlds.len(), 1);
        assert_eq!(worlds[0].world_id, Some("wrld_searchable".to_string()));
    }

    #[test]
    fn filters_library_worlds_by_any_visit_in_date_range() {
        let connection = setup_library_test_database();

        insert_library_visit(
            &connection,
            "2026-06-01T20:00:00",
            "Range World",
            Some("wrld_range"),
            Some(100),
            None,
            None,
        );
        insert_library_visit(
            &connection,
            "2026-06-20T20:00:00",
            "Range World",
            Some("wrld_range"),
            Some(200),
            None,
            None,
        );
        insert_library_visit(
            &connection,
            "2026-06-30T20:00:00",
            "Outside World",
            Some("wrld_outside"),
            Some(300),
            None,
            None,
        );

        let criteria = LibraryQueryCriteria {
            visited_from: Some("2026-06-01T00:00:00".to_string()),
            visited_to: Some("2026-06-02T00:00:00".to_string()),
            ..default_library_criteria()
        };
        let (worlds, total_count) =
            list_library_worlds(&connection, &criteria).expect("range worlds should be listed");

        assert_eq!(total_count, 1);
        assert_eq!(worlds[0].world_id, Some("wrld_range".to_string()));
        assert_eq!(
            worlds[0].last_visited_at, "2026-06-20T20:00:00",
            "aggregate values should still use all visits in the world group"
        );
        assert_eq!(worlds[0].total_stay_duration_seconds, 300);
    }

    #[test]
    fn sorts_library_worlds_by_supported_keys_and_directions() {
        let connection = setup_library_test_database();

        insert_library_visit(
            &connection,
            "2026-06-09T20:00:00",
            "Bravo",
            Some("wrld_bravo"),
            Some(200),
            None,
            None,
        );
        insert_library_visit(
            &connection,
            "2026-06-10T20:00:00",
            "Bravo",
            Some("wrld_bravo"),
            Some(200),
            None,
            None,
        );
        insert_library_visit(
            &connection,
            "2026-06-09T20:00:00",
            "Alpha",
            Some("wrld_alpha"),
            Some(100),
            None,
            None,
        );
        insert_library_visit(
            &connection,
            "2026-06-09T20:00:00",
            "Charlie",
            Some("wrld_charlie"),
            Some(300),
            None,
            None,
        );

        let cases = [
            ("world_name", "asc", "wrld_alpha"),
            ("world_name", "desc", "wrld_charlie"),
            ("visit_count", "asc", "wrld_alpha"),
            ("visit_count", "desc", "wrld_bravo"),
            ("total_stay_duration_seconds", "asc", "wrld_alpha"),
            ("total_stay_duration_seconds", "desc", "wrld_bravo"),
        ];

        for (sort_key, sort_direction, expected_first_world_id) in cases {
            let criteria = LibraryQueryCriteria {
                sort_key: sort_key.to_string(),
                sort_direction: sort_direction.to_string(),
                ..default_library_criteria()
            };
            let (worlds, _) = list_library_worlds(&connection, &criteria)
                .expect("sorted worlds should be listed");

            assert_eq!(
                worlds[0].world_id,
                Some(expected_first_world_id.to_string()),
                "{sort_key} {sort_direction} should sort expected world first"
            );
        }
    }

    #[test]
    fn pages_library_worlds_and_returns_total_count() {
        let connection = setup_library_test_database();

        insert_library_visit(
            &connection,
            "2026-06-09T20:00:00",
            "Alpha",
            Some("wrld_alpha"),
            Some(100),
            None,
            None,
        );
        insert_library_visit(
            &connection,
            "2026-06-09T20:00:00",
            "Bravo",
            Some("wrld_bravo"),
            Some(100),
            None,
            None,
        );
        insert_library_visit(
            &connection,
            "2026-06-09T20:00:00",
            "Charlie",
            Some("wrld_charlie"),
            Some(100),
            None,
            None,
        );

        let criteria = LibraryQueryCriteria {
            limit: 1,
            offset: 1,
            ..default_library_criteria()
        };
        let (worlds, total_count) =
            list_library_worlds(&connection, &criteria).expect("paged worlds should be listed");

        assert_eq!(total_count, 3);
        assert_eq!(worlds.len(), 1);
        assert_eq!(worlds[0].world_id, Some("wrld_bravo".to_string()));
    }

    #[test]
    fn gets_library_world_detail_with_visits() {
        let connection = setup_library_test_database();

        insert_library_visit(
            &connection,
            "2026-06-09T20:00:00",
            "Detail World",
            Some("wrld_detail"),
            Some(100),
            Some("old memo"),
            Some("Calm"),
        );
        insert_library_visit(
            &connection,
            "2026-06-10T20:00:00",
            "Detail World",
            Some("wrld_detail"),
            Some(200),
            Some("new memo"),
            Some("calm, Photo"),
        );

        let (world, visits) = get_library_world_group(&connection, Some("wrld_detail"), "ignored")
            .expect("detail query should succeed")
            .expect("detail world should exist");

        assert_eq!(world.visit_count, 2);
        assert_eq!(world.total_stay_duration_seconds, 300);
        assert_eq!(world.tags, vec!["Calm".to_string(), "Photo".to_string()]);
        assert_eq!(visits.len(), 2);
        assert_eq!(visits[0].visited_at, "2026-06-10T20:00:00");
        assert_eq!(visits[1].visited_at, "2026-06-09T20:00:00");
    }

    #[test]
    fn adds_visit_to_database() {
        let connection =
            open_database(Path::new(":memory:")).expect("in-memory database should open");

        initialize_database(&connection).expect("database schema should be initialized");

        let visit = make_visit_input();

        let inserted = add_visit_if_missing(&connection, &visit).expect("visit should be inserted");

        assert!(inserted);

        let count: i64 = connection
            .query_row(
                "
                SELECT COUNT(*)
                FROM visit_histories
                ",
                [],
                |row| row.get(0),
            )
            .expect("visit count should be queried");

        assert_eq!(count, 1);
    }

    #[test]
    fn does_not_insert_duplicate_world_id_and_visited_at() {
        let connection =
            open_database(Path::new(":memory:")).expect("in-memory database should open");

        initialize_database(&connection).expect("database schema should be initialized");

        let visit = make_visit_input();

        let first_inserted =
            add_visit_if_missing(&connection, &visit).expect("first visit should be inserted");
        let second_inserted =
            add_visit_if_missing(&connection, &visit).expect("duplicate visit should be ignored");

        assert!(first_inserted);
        assert!(!second_inserted);

        let count: i64 = connection
            .query_row(
                "
                SELECT COUNT(*)
                FROM visit_histories
                ",
                [],
                |row| row.get(0),
            )
            .expect("visit count should be queried");

        assert_eq!(count, 1);
    }

    #[test]
    fn ignores_duplicate_visit_with_null_unique_key_parts() {
        let connection =
            open_database(Path::new(":memory:")).expect("in-memory database should open");

        initialize_database(&connection).expect("database schema should be initialized");

        let mut visit = make_visit_input();
        visit.world_id = None;
        visit.instance_id = None;
        visit.source_log_file = None;

        let duplicate_visit = visit.clone();

        let first_inserted =
            add_visit_if_missing(&connection, &visit).expect("first visit should be inserted");
        let second_inserted = add_visit_if_missing(&connection, &duplicate_visit)
            .expect("duplicate visit should be ignored");

        assert!(first_inserted);
        assert!(!second_inserted);

        let count = count_visit_histories(&connection).expect("visit count should be queried");

        assert_eq!(count, 1);
    }

    #[test]
    fn inserts_visit_when_source_log_file_differs_for_same_world_and_visited_at() {
        let connection =
            open_database(Path::new(":memory:")).expect("in-memory database should open");

        initialize_database(&connection).expect("database schema should be initialized");

        let mut first_visit = make_visit_input();
        first_visit.source_log_file = Some("output_log_1.txt".to_string());

        let mut second_visit = first_visit.clone();
        second_visit.source_log_file = Some("output_log_2.txt".to_string());

        let first_inserted = add_visit_if_missing(&connection, &first_visit)
            .expect("first visit should be inserted");
        let second_inserted = add_visit_if_missing(&connection, &second_visit)
            .expect("visit from a different source_log_file should be inserted, not ignored");

        assert!(first_inserted);
        assert!(
            second_inserted,
            "differing source_log_file must not be treated as the same visit event"
        );

        let count = count_visit_histories(&connection).expect("visit count should be queried");

        assert_eq!(count, 2);
    }

    #[test]
    fn inserts_visit_when_world_id_differs_for_same_world_name_source_and_visited_at() {
        let connection =
            open_database(Path::new(":memory:")).expect("in-memory database should open");

        initialize_database(&connection).expect("database schema should be initialized");

        let mut first_visit = make_visit_input();
        first_visit.world_id = Some("wrld_example_1".to_string());

        let mut second_visit = first_visit.clone();
        second_visit.world_id = Some("wrld_example_2".to_string());

        let first_inserted = add_visit_if_missing(&connection, &first_visit)
            .expect("first visit should be inserted");
        let second_inserted = add_visit_if_missing(&connection, &second_visit)
            .expect("different world_id visit should be inserted");

        assert!(first_inserted);
        assert!(second_inserted);

        let count = count_visit_histories(&connection).expect("visit count should be queried");

        assert_eq!(count, 2);
    }

    #[test]
    fn counts_zero_visits_when_table_is_empty() {
        let connection =
            open_database(Path::new(":memory:")).expect("in-memory database should open");

        initialize_database(&connection).expect("database schema should be initialized");

        let count = count_visit_histories(&connection).expect("visit count should be queried");

        assert_eq!(count, 0);
    }

    #[test]
    fn counts_inserted_visits() {
        let connection =
            open_database(Path::new(":memory:")).expect("in-memory database should open");

        initialize_database(&connection).expect("database schema should be initialized");

        let visit = make_visit_input();

        add_visit_if_missing(&connection, &visit).expect("visit should be inserted");

        let count = count_visit_histories(&connection).expect("visit count should be queried");

        assert_eq!(count, 1);
    }

    #[test]
    #[ignore]
    fn reads_existing_worldrec_database_count_read_only() {
        use crate::db::connection::open_database_read_only;

        let db_path_buf = existing_worldrec_db_path();
        let db_path = db_path_buf.as_path();

        let connection = open_database_read_only(db_path)
            .expect("existing WorldRec database should open read-only");

        let count =
            count_visit_histories(&connection).expect("existing visit count should be queried");

        println!("existing visit_histories count: {}", count);

        assert!(count >= 0);
    }

    #[test]
    fn lists_recent_visit_histories() {
        let connection =
            open_database(Path::new(":memory:")).expect("in-memory database should open");

        initialize_database(&connection).expect("database schema should be initialized");

        let visit = make_visit_input();

        add_visit_if_missing(&connection, &visit).expect("visit should be inserted");

        let visits = list_recent_visit_histories(&connection, 10).expect("visits should be listed");

        assert_eq!(visits.len(), 1);
        assert_eq!(visits[0].world_name, "Example World");
        assert_eq!(visits[0].world_id, Some("wrld_example".to_string()));
        assert_eq!(visits[0].stay_duration_seconds, Some(3600));
    }

    #[test]
    #[ignore]
    fn reads_recent_existing_worldrec_visits_read_only() {
        use crate::db::connection::open_database_read_only;

        let db_path_buf = existing_worldrec_db_path();
        let db_path = db_path_buf.as_path();

        let connection = open_database_read_only(db_path)
            .expect("existing WorldRec database should open read-only");

        let visits =
            list_recent_visit_histories(&connection, 3).expect("recent visits should be listed");

        println!("recent existing visits:");
        for visit in &visits {
            println!(
                "- id: {}, visited_at: {}, world_name: {}, world_id: {:?}, stay_duration_seconds: {:?}",
                visit.id,
                visit.visited_at,
                visit.world_name,
                visit.world_id,
                visit.stay_duration_seconds
            );
        }

        assert!(visits.len() <= 3);
    }

    #[test]
    fn counts_duplicate_visit_keys() {
        let connection =
            open_database(Path::new(":memory:")).expect("in-memory database should open");

        create_legacy_visit_histories_table(&connection);

        connection
            .execute_batch(
                "
                INSERT INTO visit_histories (
                    visited_at,
                    world_name,
                    world_id,
                    instance_id,
                    instance_access_type,
                    instance_nonce,
                    instance_raw_tags,
                    stay_duration_seconds,
                    memo,
                    tags,
                    source_log_file,
                    created_at,
                    updated_at
                ) VALUES
                (
                    '2026-06-09T21:00:00+09:00',
                    'Example World',
                    'wrld_example',
                    '12345',
                    'public',
                    NULL,
                    NULL,
                    3600,
                    NULL,
                    NULL,
                    'output_log_1.txt',
                    CURRENT_TIMESTAMP,
                    CURRENT_TIMESTAMP
                ),
                (
                    '2026-06-09T21:00:00+09:00',
                    'Example World',
                    'wrld_example',
                    '12345',
                    'public',
                    NULL,
                    NULL,
                    0,
                    NULL,
                    NULL,
                    'output_log_1.txt',
                    CURRENT_TIMESTAMP,
                    CURRENT_TIMESTAMP
                );
                ",
            )
            .expect("duplicate visits should be inserted directly");

        let duplicate_count = count_duplicate_visit_keys(&connection)
            .expect("duplicate visit keys should be counted");

        assert_eq!(duplicate_count, 1);
    }

    #[test]
    #[ignore]
    fn counts_existing_worldrec_duplicate_visit_keys_read_only() {
        use crate::db::connection::open_database_read_only;

        let db_path_buf = existing_worldrec_db_path();
        let db_path = db_path_buf.as_path();

        let connection = open_database_read_only(db_path)
            .expect("existing WorldRec database should open read-only");

        let duplicate_count = count_duplicate_visit_keys(&connection)
            .expect("existing duplicate visit keys should be counted");

        println!("existing duplicate visit key groups: {}", duplicate_count);

        assert!(duplicate_count >= 0);
    }

    #[test]
    fn lists_duplicate_visit_keys() {
        let connection =
            open_database(Path::new(":memory:")).expect("in-memory database should open");

        create_legacy_visit_histories_table(&connection);

        connection
            .execute_batch(
                "
                INSERT INTO visit_histories (
                    visited_at,
                    world_name,
                    world_id,
                    instance_id,
                    instance_access_type,
                    instance_nonce,
                    instance_raw_tags,
                    stay_duration_seconds,
                    memo,
                    tags,
                    source_log_file,
                    created_at,
                    updated_at
                ) VALUES
                (
                    '2026-06-09T20:00:00+09:00',
                    'Example World',
                    'wrld_example',
                    '12345',
                    'public',
                    NULL,
                    NULL,
                    3600,
                    NULL,
                    NULL,
                    'output_log_1.txt',
                    CURRENT_TIMESTAMP,
                    CURRENT_TIMESTAMP
                ),
                (
                    '2026-06-09T20:00:00+09:00',
                    'Example World',
                    'wrld_example',
                    '12345',
                    'public',
                    NULL,
                    NULL,
                    0,
                    NULL,
                    NULL,
                    'output_log_1.txt',
                    CURRENT_TIMESTAMP,
                    CURRENT_TIMESTAMP
                );
                ",
            )
            .expect("duplicate visits should be inserted directly");

        let duplicate_keys = list_duplicate_visit_keys(&connection, 10)
            .expect("duplicate visit keys should be listed");

        assert_eq!(duplicate_keys.len(), 1);
        assert_eq!(duplicate_keys[0].visit_key, "wrld_example");
        assert_eq!(
            duplicate_keys[0].source_log_file,
            Some("output_log_1.txt".to_string())
        );
        assert_eq!(duplicate_keys[0].visited_at, "2026-06-09T20:00:00+09:00");
        assert_eq!(duplicate_keys[0].count, 2);
    }

    #[test]
    fn does_not_count_rows_with_different_source_log_file_as_duplicates() {
        let connection =
            open_database(Path::new(":memory:")).expect("in-memory database should open");

        create_legacy_visit_histories_table(&connection);

        connection
            .execute_batch(
                "
                INSERT INTO visit_histories (
                    visited_at,
                    world_name,
                    world_id,
                    instance_id,
                    instance_access_type,
                    instance_nonce,
                    instance_raw_tags,
                    stay_duration_seconds,
                    memo,
                    tags,
                    source_log_file,
                    created_at,
                    updated_at
                ) VALUES
                (
                    '2026-06-09T20:00:00+09:00',
                    'Example World',
                    'wrld_example',
                    '12345',
                    'public',
                    NULL,
                    NULL,
                    3600,
                    NULL,
                    NULL,
                    'output_log_1.txt',
                    CURRENT_TIMESTAMP,
                    CURRENT_TIMESTAMP
                ),
                (
                    '2026-06-09T20:00:00+09:00',
                    'Example World',
                    'wrld_example',
                    '12345',
                    'public',
                    NULL,
                    NULL,
                    0,
                    NULL,
                    NULL,
                    'output_log_2.txt',
                    CURRENT_TIMESTAMP,
                    CURRENT_TIMESTAMP
                );
                ",
            )
            .expect("same-key visits from different log files should be inserted directly");

        let duplicate_count = count_duplicate_visit_keys(&connection)
            .expect("duplicate visit keys should be counted");
        let duplicate_keys = list_duplicate_visit_keys(&connection, 10)
            .expect("duplicate visit keys should be listed");

        assert_eq!(
            duplicate_count, 0,
            "rows from different source_log_file must not be treated as duplicates"
        );
        assert!(duplicate_keys.is_empty());
    }

    #[test]
    #[ignore]
    fn lists_existing_worldrec_duplicate_visit_keys_read_only() {
        use crate::db::connection::open_database_read_only;

        let db_path_buf = existing_worldrec_db_path();
        let db_path = db_path_buf.as_path();

        let connection = open_database_read_only(db_path)
            .expect("existing WorldRec database should open read-only");

        let duplicate_keys = list_duplicate_visit_keys(&connection, 10)
            .expect("existing duplicate visit keys should be listed");

        println!("existing duplicate visit keys:");
        for duplicate_key in &duplicate_keys {
            println!(
                "- visit_key: {}, visited_at: {}, count: {}",
                duplicate_key.visit_key, duplicate_key.visited_at, duplicate_key.count
            );
        }

        assert!(duplicate_keys.len() <= 10);
    }

    #[test]
    fn lists_duplicate_visit_rows() {
        let connection =
            open_database(Path::new(":memory:")).expect("in-memory database should open");

        create_legacy_visit_histories_table(&connection);

        connection
            .execute_batch(
                "
                INSERT INTO visit_histories (
                    visited_at,
                    world_name,
                    world_id,
                    instance_id,
                    instance_access_type,
                    instance_nonce,
                    instance_raw_tags,
                    stay_duration_seconds,
                    memo,
                    tags,
                    source_log_file,
                    created_at,
                    updated_at
                ) VALUES
                (
                    '2026-06-09T20:00:00+09:00',
                    'Example World',
                    'wrld_example',
                    '12345',
                    'public',
                    NULL,
                    NULL,
                    NULL,
                    NULL,
                    NULL,
                    'output_log_1.txt',
                    CURRENT_TIMESTAMP,
                    CURRENT_TIMESTAMP
                ),
                (
                    '2026-06-09T20:00:00+09:00',
                    'Example World',
                    'wrld_example',
                    '12345',
                    'public',
                    NULL,
                    NULL,
                    3600,
                    NULL,
                    NULL,
                    'output_log_1.txt',
                    CURRENT_TIMESTAMP,
                    CURRENT_TIMESTAMP
                );
                ",
            )
            .expect("duplicate visits should be inserted directly");

        let duplicate_keys = list_duplicate_visit_keys(&connection, 10)
            .expect("duplicate visit keys should be listed");
        let duplicate_rows = list_duplicate_visit_rows(&connection, &duplicate_keys[0])
            .expect("duplicate visit rows should be listed");

        assert_eq!(duplicate_rows.len(), 2);
        assert_eq!(duplicate_rows[0].stay_duration_seconds, Some(3600));
        assert_eq!(duplicate_rows[1].stay_duration_seconds, None);
    }

    #[test]
    #[ignore]
    fn lists_existing_worldrec_duplicate_visit_rows_read_only() {
        use crate::db::connection::open_database_read_only;

        let db_path_buf = existing_worldrec_db_path();
        let db_path = db_path_buf.as_path();

        let connection = open_database_read_only(db_path)
            .expect("existing WorldRec database should open read-only");

        let duplicate_keys = list_duplicate_visit_keys(&connection, 10)
            .expect("existing duplicate visit keys should be listed");
        let duplicate_rows = list_duplicate_visit_rows(&connection, &duplicate_keys[0])
            .expect("existing duplicate visit rows should be listed");

        println!("existing duplicate visit rows:");
        for row in &duplicate_rows {
            println!(
                "- id: {}, visited_at: {}, world_name: {}, world_id: {:?}, stay_duration_seconds: {:?}, memo: {:?}, tags: {:?}, source_log_file: {:?}, created_at: {}, updated_at: {}",
                row.id,
                row.visited_at,
                row.world_name,
                row.world_id,
                row.stay_duration_seconds,
                row.memo,
                row.tags,
                row.source_log_file,
                row.created_at,
                row.updated_at,
            );
        }

        assert!(duplicate_rows.len() >= 2);
    }

    fn make_duplicate_visit_row(
        id: i64,
        stay_duration_seconds: Option<i64>,
        memo: Option<&str>,
        tags: Option<&str>,
    ) -> DuplicateVisitRow {
        DuplicateVisitRow {
            id,
            visited_at: "2026-06-09T20:00:00+09:00".to_string(),
            world_name: "Example World".to_string(),
            world_id: Some("wrld_example".to_string()),
            stay_duration_seconds,
            memo: memo.map(|value| value.to_string()),
            tags: tags.map(|value| value.to_string()),
            source_log_file: Some("output_log.txt".to_string()),
            created_at: "2026-06-09T20:00:00+09:00".to_string(),
            updated_at: "2026-06-09T20:00:00+09:00".to_string(),
        }
    }

    #[test]
    fn selects_row_with_stay_duration_over_none() {
        let rows = vec![
            make_duplicate_visit_row(8584, None, None, None),
            make_duplicate_visit_row(8583, Some(0), None, None),
        ];

        let keep_id = select_duplicate_visit_row_id_to_keep(&rows);

        assert_eq!(keep_id, Some(8583));
    }

    #[test]
    fn selects_row_with_memo_over_longer_duration() {
        let rows = vec![
            make_duplicate_visit_row(1, Some(3600), None, None),
            make_duplicate_visit_row(2, Some(0), Some("important memo"), None),
        ];

        let keep_id = select_duplicate_visit_row_id_to_keep(&rows);

        assert_eq!(keep_id, Some(2));
    }

    #[test]
    fn returns_none_when_duplicate_rows_are_empty() {
        let rows = Vec::new();

        let keep_id = select_duplicate_visit_row_id_to_keep(&rows);

        assert_eq!(keep_id, None);
    }

    #[test]
    #[ignore]
    fn selects_existing_worldrec_duplicate_visit_row_to_keep_read_only() {
        use crate::db::connection::open_database_read_only;

        let db_path_buf = existing_worldrec_db_path();
        let db_path = db_path_buf.as_path();

        let connection = open_database_read_only(db_path)
            .expect("existing WorldRec database should open read-only");

        let duplicate_keys = list_duplicate_visit_keys(&connection, 10)
            .expect("existing duplicate visit keys should be listed");
        let duplicate_rows = list_duplicate_visit_rows(&connection, &duplicate_keys[0])
            .expect("existing duplicate visit rows should be listed");

        let keep_id = select_duplicate_visit_row_id_to_keep(&duplicate_rows)
            .expect("one duplicate visit row should be selected to keep");

        println!("selected duplicate visit row to keep: {}", keep_id);
        println!("existing duplicate visit rows:");

        for row in &duplicate_rows {
            let decision = if row.id == keep_id {
                "KEEP"
            } else {
                "DELETE_CANDIDATE"
            };

            println!(
                "- decision: {}, id: {}, visited_at: {}, world_name: {}, world_id: {:?}, stay_duration_seconds: {:?}, memo: {:?}, tags: {:?}, source_log_file: {:?}",
                decision,
                row.id,
                row.visited_at,
                row.world_name,
                row.world_id,
                row.stay_duration_seconds,
                row.memo,
                row.tags,
                row.source_log_file,
            );
        }

        assert_eq!(keep_id, 8583);
    }

    #[test]
    fn selects_duplicate_visit_row_ids_to_delete() {
        let rows = vec![
            make_duplicate_visit_row(8584, None, None, None),
            make_duplicate_visit_row(8583, Some(0), None, None),
        ];

        let delete_ids = select_duplicate_visit_row_ids_to_delete(&rows);

        assert_eq!(delete_ids, vec![8584]);
    }

    #[test]
    fn returns_empty_delete_ids_when_only_one_row_exists() {
        let rows = vec![make_duplicate_visit_row(8583, Some(0), None, None)];

        let delete_ids = select_duplicate_visit_row_ids_to_delete(&rows);

        assert!(delete_ids.is_empty());
    }

    #[test]
    fn returns_empty_delete_ids_when_rows_are_empty() {
        let rows = Vec::new();

        let delete_ids = select_duplicate_visit_row_ids_to_delete(&rows);

        assert!(delete_ids.is_empty());
    }

    #[test]
    #[ignore]
    fn selects_existing_worldrec_duplicate_visit_row_ids_to_delete_read_only() {
        use crate::db::connection::open_database_read_only;

        let db_path_buf = existing_worldrec_db_path();
        let db_path = db_path_buf.as_path();

        let connection = open_database_read_only(db_path)
            .expect("existing WorldRec database should open read-only");

        let duplicate_keys = list_duplicate_visit_keys(&connection, 10)
            .expect("existing duplicate visit keys should be listed");
        let duplicate_rows = list_duplicate_visit_rows(&connection, &duplicate_keys[0])
            .expect("existing duplicate visit rows should be listed");

        let keep_id = select_duplicate_visit_row_id_to_keep(&duplicate_rows)
            .expect("one duplicate visit row should be selected to keep");

        let delete_ids = select_duplicate_visit_row_ids_to_delete(&duplicate_rows);

        println!("selected duplicate visit row to keep: {}", keep_id);
        println!(
            "selected duplicate visit row ids to delete: {:?}",
            delete_ids
        );

        assert_eq!(keep_id, 8583);
        assert_eq!(delete_ids, vec![8584]);
    }

    #[test]
    fn lists_duplicate_visit_cleanup_plans() {
        let connection =
            open_database(Path::new(":memory:")).expect("in-memory database should open");

        create_legacy_visit_histories_table(&connection);

        connection
            .execute_batch(
                "
                INSERT INTO visit_histories (
                    visited_at,
                    world_name,
                    world_id,
                    instance_id,
                    instance_access_type,
                    instance_nonce,
                    instance_raw_tags,
                    stay_duration_seconds,
                    memo,
                    tags,
                    source_log_file,
                    created_at,
                    updated_at
                ) VALUES
                (
                    '2026-06-09T20:00:00+09:00',
                    'Example World',
                    'wrld_example',
                    '12345',
                    'public',
                    NULL,
                    NULL,
                    NULL,
                    NULL,
                    NULL,
                    'output_log_1.txt',
                    CURRENT_TIMESTAMP,
                    CURRENT_TIMESTAMP
                ),
                (
                    '2026-06-09T20:00:00+09:00',
                    'Example World',
                    'wrld_example',
                    '12345',
                    'public',
                    NULL,
                    NULL,
                    3600,
                    NULL,
                    NULL,
                    'output_log_1.txt',
                    CURRENT_TIMESTAMP,
                    CURRENT_TIMESTAMP
                );
                ",
            )
            .expect("duplicate visits should be inserted directly");

        let cleanup_plans = list_duplicate_visit_cleanup_plans(&connection, 10)
            .expect("duplicate visit cleanup plans should be listed");

        assert_eq!(cleanup_plans.len(), 1);
        assert_eq!(cleanup_plans[0].visit_key, "wrld_example");
        assert_eq!(cleanup_plans[0].visited_at, "2026-06-09T20:00:00+09:00");
        assert_eq!(cleanup_plans[0].keep_id, 2);
        assert_eq!(cleanup_plans[0].delete_ids, vec![1]);
    }

    #[test]
    #[ignore]
    fn lists_existing_worldrec_duplicate_cleanup_plans_read_only() {
        use crate::db::connection::open_database_read_only;

        let db_path_buf = existing_worldrec_db_path();
        let db_path = db_path_buf.as_path();

        let connection = open_database_read_only(db_path)
            .expect("existing WorldRec database should open read-only");

        let cleanup_plans = list_duplicate_visit_cleanup_plans(&connection, 10)
            .expect("existing duplicate cleanup plans should be listed");

        println!("existing duplicate cleanup plans:");
        for plan in &cleanup_plans {
            println!(
                "- visit_key: {}, visited_at: {}, keep_id: {}, delete_ids: {:?}",
                plan.visit_key, plan.visited_at, plan.keep_id, plan.delete_ids,
            );
        }

        assert!(cleanup_plans.len() <= 10);
    }

    #[test]
    fn counts_duplicate_visit_rows_to_delete() {
        let connection =
            open_database(Path::new(":memory:")).expect("in-memory database should open");

        create_legacy_visit_histories_table(&connection);

        connection
            .execute_batch(
                "
                INSERT INTO visit_histories (
                    visited_at,
                    world_name,
                    world_id,
                    instance_id,
                    instance_access_type,
                    instance_nonce,
                    instance_raw_tags,
                    stay_duration_seconds,
                    memo,
                    tags,
                    source_log_file,
                    created_at,
                    updated_at
                ) VALUES
                (
                    '2026-06-09T20:00:00+09:00',
                    'Example World',
                    'wrld_example',
                    '12345',
                    'public',
                    NULL,
                    NULL,
                    NULL,
                    NULL,
                    NULL,
                    'output_log_1.txt',
                    CURRENT_TIMESTAMP,
                    CURRENT_TIMESTAMP
                ),
                (
                    '2026-06-09T20:00:00+09:00',
                    'Example World',
                    'wrld_example',
                    '12345',
                    'public',
                    NULL,
                    NULL,
                    3600,
                    NULL,
                    NULL,
                    'output_log_1.txt',
                    CURRENT_TIMESTAMP,
                    CURRENT_TIMESTAMP
                ),
                (
                    '2026-06-09T21:00:00+09:00',
                    'Another World',
                    'wrld_another',
                    '12345',
                    'public',
                    NULL,
                    NULL,
                    1800,
                    NULL,
                    NULL,
                    'output_log_3.txt',
                    CURRENT_TIMESTAMP,
                    CURRENT_TIMESTAMP
                );
                ",
            )
            .expect("visits should be inserted directly");

        let delete_count = count_duplicate_visit_rows_to_delete(&connection)
            .expect("duplicate visit rows to delete should be counted");

        assert_eq!(delete_count, 1);
    }

    #[test]
    fn counts_zero_duplicate_visit_rows_to_delete_when_no_duplicates_exist() {
        let connection =
            open_database(Path::new(":memory:")).expect("in-memory database should open");

        initialize_database(&connection).expect("database schema should be initialized");

        let visit = make_visit_input();

        add_visit_if_missing(&connection, &visit).expect("visit should be inserted");

        let delete_count = count_duplicate_visit_rows_to_delete(&connection)
            .expect("duplicate visit rows to delete should be counted");

        assert_eq!(delete_count, 0);
    }

    #[test]
    #[ignore]
    fn counts_existing_worldrec_duplicate_visit_rows_to_delete_read_only() {
        use crate::db::connection::open_database_read_only;

        let db_path_buf = existing_worldrec_db_path();
        let db_path = db_path_buf.as_path();

        let connection = open_database_read_only(db_path)
            .expect("existing WorldRec database should open read-only");

        let delete_count = count_duplicate_visit_rows_to_delete(&connection)
            .expect("existing duplicate visit rows to delete should be counted");

        println!("existing duplicate visit rows to delete: {}", delete_count);

        assert!(delete_count >= 0);
    }

    #[test]
    fn lists_duplicate_visit_row_ids_to_delete() {
        let connection =
            open_database(Path::new(":memory:")).expect("in-memory database should open");

        create_legacy_visit_histories_table(&connection);

        connection
            .execute_batch(
                "
                INSERT INTO visit_histories (
                    visited_at,
                    world_name,
                    world_id,
                    instance_id,
                    instance_access_type,
                    instance_nonce,
                    instance_raw_tags,
                    stay_duration_seconds,
                    memo,
                    tags,
                    source_log_file,
                    created_at,
                    updated_at
                ) VALUES
                (
                    '2026-06-09T20:00:00+09:00',
                    'Example World',
                    'wrld_example',
                    '12345',
                    'public',
                    NULL,
                    NULL,
                    NULL,
                    NULL,
                    NULL,
                    'output_log_1.txt',
                    CURRENT_TIMESTAMP,
                    CURRENT_TIMESTAMP
                ),
                (
                    '2026-06-09T20:00:00+09:00',
                    'Example World',
                    'wrld_example',
                    '12345',
                    'public',
                    NULL,
                    NULL,
                    3600,
                    NULL,
                    NULL,
                    'output_log_1.txt',
                    CURRENT_TIMESTAMP,
                    CURRENT_TIMESTAMP
                ),
                (
                    '2026-06-09T21:00:00+09:00',
                    'Another World',
                    'wrld_another',
                    '12345',
                    'public',
                    NULL,
                    NULL,
                    NULL,
                    NULL,
                    NULL,
                    'output_log_3.txt',
                    CURRENT_TIMESTAMP,
                    CURRENT_TIMESTAMP
                ),
                (
                    '2026-06-09T21:00:00+09:00',
                    'Another World',
                    'wrld_another',
                    '12345',
                    'public',
                    NULL,
                    NULL,
                    1800,
                    NULL,
                    NULL,
                    'output_log_3.txt',
                    CURRENT_TIMESTAMP,
                    CURRENT_TIMESTAMP
                );
                ",
            )
            .expect("duplicate visits should be inserted directly");

        let delete_ids = list_duplicate_visit_row_ids_to_delete(&connection, 10)
            .expect("duplicate visit row ids to delete should be listed");

        assert_eq!(delete_ids, vec![3, 1]);
    }

    #[test]
    #[ignore]
    fn lists_existing_worldrec_duplicate_visit_row_ids_to_delete_read_only() {
        use crate::db::connection::open_database_read_only;

        let db_path_buf = existing_worldrec_db_path();
        let db_path = db_path_buf.as_path();

        let connection = open_database_read_only(db_path)
            .expect("existing WorldRec database should open read-only");

        let delete_ids = list_duplicate_visit_row_ids_to_delete(&connection, 20)
            .expect("existing duplicate visit row ids to delete should be listed");

        println!("existing duplicate visit row ids to delete:");
        println!("{:?}", delete_ids);

        assert!(delete_ids.len() <= 20);
    }

    #[test]
    fn counts_visit_histories_by_ids() {
        let connection =
            open_database(Path::new(":memory:")).expect("in-memory database should open");

        initialize_database(&connection).expect("database schema should be initialized");

        let visit = make_visit_input();

        add_visit_if_missing(&connection, &visit).expect("visit should be inserted");

        let count = count_visit_histories_by_ids(&connection, &[1])
            .expect("visit histories should be counted by ids");

        assert_eq!(count, 1);
    }

    #[test]
    fn counts_zero_when_visit_history_ids_do_not_exist() {
        let connection =
            open_database(Path::new(":memory:")).expect("in-memory database should open");

        initialize_database(&connection).expect("database schema should be initialized");

        let count = count_visit_histories_by_ids(&connection, &[999])
            .expect("visit histories should be counted by ids");

        assert_eq!(count, 0);
    }

    #[test]
    fn counts_zero_when_visit_history_ids_are_empty() {
        let connection =
            open_database(Path::new(":memory:")).expect("in-memory database should open");

        initialize_database(&connection).expect("database schema should be initialized");

        let count = count_visit_histories_by_ids(&connection, &[])
            .expect("visit histories should be counted by ids");

        assert_eq!(count, 0);
    }

    #[test]
    fn deletes_visit_histories_by_ids() {
        let connection =
            open_database(Path::new(":memory:")).expect("in-memory database should open");

        initialize_database(&connection).expect("database schema should be initialized");

        connection
            .execute_batch(
                "
                INSERT INTO visit_histories (
                    visited_at,
                    world_name,
                    world_id,
                    instance_id,
                    instance_access_type,
                    instance_nonce,
                    instance_raw_tags,
                    stay_duration_seconds,
                    memo,
                    tags,
                    source_log_file,
                    created_at,
                    updated_at
                ) VALUES
                (
                    '2026-06-09T20:00:00+09:00',
                    'Example World',
                    'wrld_example',
                    '12345',
                    'public',
                    NULL,
                    NULL,
                    NULL,
                    NULL,
                    NULL,
                    'output_log_1.txt',
                    CURRENT_TIMESTAMP,
                    CURRENT_TIMESTAMP
                ),
                (
                    '2026-06-09T21:00:00+09:00',
                    'Example World',
                    'wrld_example',
                    '12345',
                    'public',
                    NULL,
                    NULL,
                    3600,
                    NULL,
                    NULL,
                    'output_log_1.txt',
                    CURRENT_TIMESTAMP,
                    CURRENT_TIMESTAMP
                );
                ",
            )
            .expect("visits should be inserted directly");

        let before_count =
            count_visit_histories(&connection).expect("before count should be queried");

        let deleted_count = delete_visit_histories_by_ids(&connection, &[1])
            .expect("visit history should be deleted");

        let after_count =
            count_visit_histories(&connection).expect("after count should be queried");

        assert_eq!(before_count, 2);
        assert_eq!(deleted_count, 1);
        assert_eq!(after_count, 1);
    }

    #[test]
    fn does_not_delete_when_visit_history_ids_are_empty() {
        let connection =
            open_database(Path::new(":memory:")).expect("in-memory database should open");

        initialize_database(&connection).expect("database schema should be initialized");

        let deleted_count =
            delete_visit_histories_by_ids(&connection, &[]).expect("empty delete should succeed");

        assert_eq!(deleted_count, 0);
    }

    #[test]
    fn does_not_delete_when_visit_history_ids_do_not_exist() {
        let connection =
            open_database(Path::new(":memory:")).expect("in-memory database should open");

        initialize_database(&connection).expect("database schema should be initialized");

        let visit = make_visit_input();

        add_visit_if_missing(&connection, &visit).expect("visit should be inserted");

        let deleted_count = delete_visit_histories_by_ids(&connection, &[999])
            .expect("missing id delete should succeed");

        let count = count_visit_histories(&connection).expect("visit count should be queried");

        assert_eq!(deleted_count, 0);
        assert_eq!(count, 1);
    }

    #[test]
    #[ignore]
    fn deletes_existing_worldrec_duplicate_visit_rows() {
        use crate::db::connection::open_database;
        use std::time::{SystemTime, UNIX_EPOCH};

        let db_path_buf = existing_worldrec_db_path();
        let db_path = db_path_buf.as_path();

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_secs();

        let backup_path =
            db_path.with_file_name(format!("worldrec.before-deduplicate.{}.db", timestamp));

        std::fs::copy(db_path, &backup_path)
            .expect("database backup should be created before delete");

        let connection = open_database(db_path).expect("existing WorldRec database should open");

        let before_count =
            count_visit_histories(&connection).expect("before visit count should be queried");

        let before_duplicate_group_count = count_duplicate_visit_keys(&connection)
            .expect("before duplicate group count should be queried");

        let before_delete_candidate_count = count_duplicate_visit_rows_to_delete(&connection)
            .expect("before delete candidate count should be queried");

        let delete_ids = list_duplicate_visit_row_ids_to_delete(&connection, 10_000)
            .expect("duplicate visit row ids to delete should be listed");

        let existing_delete_candidate_count =
            count_visit_histories_by_ids(&connection, &delete_ids)
                .expect("delete candidate ids should exist");

        println!("backup path: {}", backup_path.display());
        println!("before visit count: {}", before_count);
        println!(
            "before duplicate group count: {}",
            before_duplicate_group_count
        );
        println!(
            "before delete candidate count: {}",
            before_delete_candidate_count
        );
        println!("planned delete ids count: {}", delete_ids.len());
        println!(
            "existing planned delete ids count: {}",
            existing_delete_candidate_count
        );

        assert_eq!(existing_delete_candidate_count, delete_ids.len() as i64);

        assert_eq!(before_delete_candidate_count, delete_ids.len() as i64);

        let deleted_count = delete_visit_histories_by_ids(&connection, &delete_ids)
            .expect("duplicate visit rows should be deleted");

        let after_count =
            count_visit_histories(&connection).expect("after visit count should be queried");

        let after_duplicate_group_count = count_duplicate_visit_keys(&connection)
            .expect("after duplicate group count should be queried");

        let after_delete_candidate_count = count_duplicate_visit_rows_to_delete(&connection)
            .expect("after delete candidate count should be queried");

        println!("deleted count: {}", deleted_count);
        println!("after visit count: {}", after_count);
        println!(
            "after duplicate group count: {}",
            after_duplicate_group_count
        );
        println!(
            "after delete candidate count: {}",
            after_delete_candidate_count
        );

        assert_eq!(deleted_count, delete_ids.len());
        assert_eq!(after_count, before_count - deleted_count as i64);
        assert_eq!(after_duplicate_group_count, 0);
        assert_eq!(after_delete_candidate_count, 0);

        initialize_database(&connection)
            .expect("database schema should be initialized after deduplicate");

        let final_duplicate_group_count = count_duplicate_visit_keys(&connection)
            .expect("final duplicate group count should be queried");

        assert_eq!(final_duplicate_group_count, 0);
    }
}
