use crate::db::repository::{
    count_duplicate_visit_keys, delete_visit_histories_by_ids,
    list_duplicate_visit_row_ids_to_delete,
};
use rusqlite::Connection;

// 重複判定仕様（確定事項）:
//   - 5秒以内の Entering/Joining 連続イベントの統合は、保存前にアプリケーション側
//     (log_watcher::visit_session::VisitSession) で行う。DB制約では扱わない。
//   - DB側は「完全に同一の訪問イベントの二重insert」を防ぐ最低限の制約に留める想定。
//     候補キーは source_log_file を含めた
//     (source_log_file, COALESCE(world_id, world_name), visited_at) とする
//     （同一ログファイル内の同一イベントのみを対象とし、別ログ・別インスタンスの
//     履歴を誤って同一視しないようにする）。
//   - ただし、既存DBに新キーで衝突する行が残っている場合に自動削除はしない方針のため、
//     このindexはinitialize_database()からは呼び出していない（無効化のまま）。
//     有効化する場合は、既存データの衝突有無を確認した上で明示的に対応すること。
#[allow(dead_code)]
const UNIQUE_VISIT_HISTORIES_EVENT_INDEX_NAME: &str = "uq_visit_histories_event";
#[allow(dead_code)]
const UNIQUE_VISIT_HISTORIES_EVENT_INDEX_SQL: &str = "
CREATE UNIQUE INDEX IF NOT EXISTS uq_visit_histories_event
    ON visit_histories (
        source_log_file,
        COALESCE(world_id, world_name),
        visited_at
    )
";

pub fn initialize_database(connection: &Connection) -> rusqlite::Result<()> {
    create_visit_histories_table_if_missing(connection)?;
    ensure_visit_histories_columns(connection)?;
    create_visit_histories_indexes(connection)?;
    create_world_preview_cache_table_if_missing(connection)?;
    ensure_world_preview_cache_columns(connection)?;

    Ok(())
}

fn create_visit_histories_table_if_missing(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS visit_histories
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
}

fn ensure_visit_histories_columns(connection: &Connection) -> rusqlite::Result<()> {
    ensure_column(
        connection,
        "visited_at",
        "visited_at TEXT NOT NULL DEFAULT ''",
    )?;
    ensure_column(
        connection,
        "world_name",
        "world_name TEXT NOT NULL DEFAULT ''",
    )?;
    ensure_column(connection, "world_id", "world_id TEXT NULL")?;
    ensure_column(connection, "instance_id", "instance_id TEXT NULL")?;
    ensure_column(
        connection,
        "instance_access_type",
        "instance_access_type TEXT NULL",
    )?;
    ensure_column(connection, "instance_nonce", "instance_nonce TEXT NULL")?;
    ensure_column(
        connection,
        "instance_raw_tags",
        "instance_raw_tags TEXT NULL",
    )?;
    ensure_column(
        connection,
        "stay_duration_seconds",
        "stay_duration_seconds INTEGER NULL",
    )?;
    ensure_column(connection, "memo", "memo TEXT NULL")?;
    ensure_column(connection, "tags", "tags TEXT NULL")?;
    ensure_column(connection, "source_log_file", "source_log_file TEXT NULL")?;
    ensure_column(
        connection,
        "created_at",
        "created_at TEXT NOT NULL DEFAULT ''",
    )?;
    ensure_column(
        connection,
        "updated_at",
        "updated_at TEXT NOT NULL DEFAULT ''",
    )?;

    Ok(())
}

fn ensure_column(
    connection: &Connection,
    column_name: &str,
    column_definition: &str,
) -> rusqlite::Result<()> {
    ensure_column_in_table(
        connection,
        "visit_histories",
        column_name,
        column_definition,
    )
}

fn ensure_world_preview_cache_columns(connection: &Connection) -> rusqlite::Result<()> {
    ensure_column_in_table(
        connection,
        "world_preview_cache",
        "description",
        "description TEXT NULL",
    )?;

    Ok(())
}

fn ensure_column_in_table(
    connection: &Connection,
    table_name: &str,
    column_name: &str,
    column_definition: &str,
) -> rusqlite::Result<()> {
    if column_exists(connection, table_name, column_name)? {
        return Ok(());
    }

    let sql = format!(
        "ALTER TABLE {} ADD COLUMN {}",
        table_name, column_definition
    );

    connection.execute(&sql, [])?;

    Ok(())
}

fn column_exists(
    connection: &Connection,
    table_name: &str,
    column_name: &str,
) -> rusqlite::Result<bool> {
    let sql = format!("PRAGMA table_info({})", table_name);
    let mut statement = connection.prepare(&sql)?;
    let mut rows = statement.query([])?;

    while let Some(row) = rows.next()? {
        let current_column_name: String = row.get(1)?;

        if current_column_name == column_name {
            return Ok(true);
        }
    }

    Ok(false)
}

#[cfg(test)]
pub fn list_columns(connection: &Connection, table_name: &str) -> rusqlite::Result<Vec<String>> {
    let sql = format!("PRAGMA table_info({})", table_name);
    let mut statement = connection.prepare(&sql)?;
    let mut rows = statement.query([])?;

    let mut columns = Vec::new();

    while let Some(row) = rows.next()? {
        let column_name: String = row.get(1)?;
        columns.push(column_name);
    }

    Ok(columns)
}

fn create_visit_histories_indexes(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(
        "
        CREATE INDEX IF NOT EXISTS idx_visit_histories_visited_at
            ON visit_histories (visited_at);

        CREATE INDEX IF NOT EXISTS idx_visit_histories_world_name
            ON visit_histories (world_name);

        CREATE INDEX IF NOT EXISTS idx_visit_histories_world_id_visited_at
            ON visit_histories (world_id, visited_at);
        ",
    )
}

fn create_world_preview_cache_table_if_missing(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS world_preview_cache
        (
            world_id TEXT PRIMARY KEY,
            world_name TEXT NOT NULL,
            image_url TEXT NULL,
            thumbnail_image_url TEXT NULL,
            author_name TEXT NULL,
            description TEXT NULL,
            fetched_at TEXT NULL,
            fetch_failed_at TEXT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        ",
    )
}

// NOTE: 起動時の自動重複削除は廃止した（既存データを誤って削除しないため）。
// 重複判定キーの仕様確定後、明示的なコマンド経由でのみ使う想定で残している。
#[allow(dead_code)]
fn delete_duplicate_visit_histories(connection: &Connection) -> rusqlite::Result<()> {
    let delete_ids = list_duplicate_visit_row_ids_to_delete(connection, 10_000)?;

    if delete_ids.is_empty() {
        return Ok(());
    }

    delete_visit_histories_by_ids(connection, &delete_ids)?;

    Ok(())
}

#[allow(dead_code)]
fn ensure_no_duplicate_visit_histories(connection: &Connection) -> rusqlite::Result<()> {
    let duplicate_count = count_duplicate_visit_keys(connection)?;

    if duplicate_count == 0 {
        return Ok(());
    }

    Err(rusqlite::Error::InvalidParameterName(format!(
        "{} duplicate visit key group(s) remain before creating {}",
        duplicate_count, UNIQUE_VISIT_HISTORIES_EVENT_INDEX_NAME
    )))
}

// NOTE: 起動時の自動作成・再作成は廃止した（強すぎるunique indexで既存DBを破壊しないため）。
#[allow(dead_code)]
fn create_unique_visit_histories_event_index(connection: &Connection) -> rusqlite::Result<()> {
    match existing_index_sql(connection, UNIQUE_VISIT_HISTORIES_EVENT_INDEX_NAME)? {
        Some(sql)
            if normalize_sql(&sql) == normalize_sql(UNIQUE_VISIT_HISTORIES_EVENT_INDEX_SQL) =>
        {
            return Ok(());
        }
        Some(_) => {
            ensure_no_duplicate_visit_histories(connection)?;
            connection.execute(
                &format!("DROP INDEX {}", UNIQUE_VISIT_HISTORIES_EVENT_INDEX_NAME),
                [],
            )?;
        }
        None => {}
    }

    ensure_no_duplicate_visit_histories(connection)?;
    connection.execute(UNIQUE_VISIT_HISTORIES_EVENT_INDEX_SQL, [])?;

    Ok(())
}

#[allow(dead_code)]
fn existing_index_sql(
    connection: &Connection,
    index_name: &str,
) -> rusqlite::Result<Option<String>> {
    let mut statement = connection.prepare(
        "
        SELECT sql
        FROM sqlite_master
        WHERE type = 'index'
          AND name = ?1
        ",
    )?;

    let mut rows = statement.query([index_name])?;

    if let Some(row) = rows.next()? {
        let sql: Option<String> = row.get(0)?;
        return Ok(sql);
    }

    Ok(None)
}

#[allow(dead_code)]
fn normalize_sql(sql: &str) -> String {
    sql.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_uppercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::open_database;
    use crate::db::models::VisitInput;
    use crate::db::repository::{
        add_visit_if_missing, count_duplicate_visit_keys, count_visit_histories,
    };
    use std::path::Path;

    // 実 DB を参照する `#[ignore]` テスト用のパス。個人環境に依存しないよう
    // 環境変数 WORLDREC_TEST_DB で既存 DB のパスを与える。
    fn existing_worldrec_db_path() -> std::path::PathBuf {
        std::path::PathBuf::from(
            std::env::var_os("WORLDREC_TEST_DB")
                .expect("set WORLDREC_TEST_DB to the path of an existing WorldRec database"),
        )
    }

    fn unique_visit_event_index_sql(connection: &Connection) -> String {
        connection
            .query_row(
                "
                SELECT sql
                FROM sqlite_master
                WHERE type = 'index'
                  AND name = 'uq_visit_histories_event'
                ",
                [],
                |row| row.get(0),
            )
            .expect("unique visit event index sql should be queried")
    }

    #[test]
    fn initializes_visit_histories_table() {
        let connection =
            open_database(Path::new(":memory:")).expect("in-memory database should open");

        initialize_database(&connection).expect("database schema should be initialized");

        let table_count: i64 = connection
            .query_row(
                "
                SELECT COUNT(*)
                FROM sqlite_master
                WHERE type = 'table'
                  AND name = 'visit_histories'
                ",
                [],
                |row| row.get(0),
            )
            .expect("table count should be queried");

        assert_eq!(table_count, 1);
    }

    #[test]
    fn initializes_world_preview_cache_table() {
        let connection =
            open_database(Path::new(":memory:")).expect("in-memory database should open");

        initialize_database(&connection).expect("database schema should be initialized");

        let table_count: i64 = connection
            .query_row(
                "
                SELECT COUNT(*)
                FROM sqlite_master
                WHERE type = 'table'
                  AND name = 'world_preview_cache'
                ",
                [],
                |row| row.get(0),
            )
            .expect("table count should be queried");
        let columns =
            list_columns(&connection, "world_preview_cache").expect("columns should be listed");

        assert_eq!(table_count, 1);
        assert!(columns.contains(&"world_id".to_string()));
        assert!(columns.contains(&"thumbnail_image_url".to_string()));
        assert!(columns.contains(&"description".to_string()));
        assert!(columns.contains(&"fetch_failed_at".to_string()));
    }

    #[test]
    fn migrates_world_preview_cache_description_column_idempotently() {
        let connection =
            open_database(Path::new(":memory:")).expect("in-memory database should open");

        connection
            .execute_batch(
                "
                CREATE TABLE world_preview_cache
                (
                    world_id TEXT PRIMARY KEY,
                    world_name TEXT NOT NULL,
                    image_url TEXT NULL,
                    thumbnail_image_url TEXT NULL,
                    author_name TEXT NULL,
                    fetched_at TEXT NULL,
                    fetch_failed_at TEXT NULL,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );
                ",
            )
            .expect("legacy world_preview_cache table should be created");

        initialize_database(&connection).expect("first migration should succeed");
        initialize_database(&connection).expect("second migration should succeed");

        let columns =
            list_columns(&connection, "world_preview_cache").expect("columns should be listed");

        assert!(columns.contains(&"description".to_string()));
        assert_eq!(
            columns
                .iter()
                .filter(|column| column.as_str() == "description")
                .count(),
            1
        );
    }

    #[test]
    fn does_not_create_unique_index_for_visit_histories_on_initialize() {
        let connection =
            open_database(Path::new(":memory:")).expect("in-memory database should open");

        initialize_database(&connection).expect("database schema should be initialized");

        let index_count: i64 = connection
            .query_row(
                "
                SELECT COUNT(*)
                FROM sqlite_master
                WHERE type = 'index'
                  AND name = 'uq_visit_histories_event'
                ",
                [],
                |row| row.get(0),
            )
            .expect("index count should be queried");

        assert_eq!(
            index_count, 0,
            "initialize_database should not auto-create a strong unique index"
        );
    }

    #[test]
    fn database_initialize_is_idempotent_after_unique_index_creation() {
        let connection =
            open_database(Path::new(":memory:")).expect("in-memory database should open");

        initialize_database(&connection).expect("first database initialization should succeed");

        let visit = VisitInput {
            visited_at: "2026-06-09T20:00:00+09:00".to_string(),
            world_name: "Example World".to_string(),
            world_id: Some("wrld_example".to_string()),
            instance_id: Some("12345".to_string()),
            instance_access_type: Some("public".to_string()),
            instance_nonce: None,
            instance_raw_tags: None,
            stay_duration_seconds: 3600,
            source_log_file: Some("output_log_2026-06-09.txt".to_string()),
        };

        add_visit_if_missing(&connection, &visit).expect("visit should be inserted");

        initialize_database(&connection).expect("second database initialization should succeed");

        let visit_count =
            count_visit_histories(&connection).expect("visit count should be queried");
        let duplicate_count =
            count_duplicate_visit_keys(&connection).expect("duplicate count should be queried");

        assert_eq!(visit_count, 1);
        assert_eq!(duplicate_count, 0);
    }

    #[test]
    fn initialize_does_not_delete_existing_duplicate_visit_histories() {
        let connection =
            open_database(Path::new(":memory:")).expect("in-memory database should open");

        create_visit_histories_table_if_missing(&connection)
            .expect("legacy visit_histories table should be created");

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
            .expect("legacy duplicate visits should be inserted");

        initialize_database(&connection)
            .expect("database initialization should not fail on existing duplicates");

        let visit_count =
            count_visit_histories(&connection).expect("visit count should be queried");

        assert_eq!(
            visit_count, 2,
            "initialize_database must not delete existing visit_histories rows on startup"
        );
    }

    #[test]
    fn initialize_does_not_touch_existing_legacy_unique_index() {
        let connection =
            open_database(Path::new(":memory:")).expect("in-memory database should open");

        create_visit_histories_table_if_missing(&connection)
            .expect("legacy visit_histories table should be created");

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
                    'output_log_2.txt',
                    CURRENT_TIMESTAMP,
                    CURRENT_TIMESTAMP
                );

                CREATE UNIQUE INDEX uq_visit_histories_event
                    ON visit_histories (
                        world_name,
                        COALESCE(source_log_file, ''),
                        visited_at
                    );
                ",
            )
            .expect("legacy visits and unique index should be inserted");

        initialize_database(&connection)
            .expect("database initialization should not fail with an existing legacy index");

        let visit_count =
            count_visit_histories(&connection).expect("visit count should be queried");
        let compact_index_sql = unique_visit_event_index_sql(&connection)
            .replace(' ', "")
            .to_lowercase();

        assert_eq!(
            visit_count, 2,
            "initialize_database must not delete rows while migrating a legacy index"
        );
        assert!(
            compact_index_sql.contains("source_log_file"),
            "initialize_database must not replace an existing safe index"
        );
    }

    #[test]
    fn adds_missing_columns_to_existing_visit_histories_table() {
        let connection =
            open_database(Path::new(":memory:")).expect("in-memory database should open");

        connection
            .execute_batch(
                "
                CREATE TABLE visit_histories (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    visited_at TEXT NOT NULL,
                    world_name TEXT NOT NULL
                );
                ",
            )
            .expect("minimal visit_histories table should be created");

        initialize_database(&connection).expect("database schema should be initialized");

        assert!(column_exists(&connection, "visit_histories", "world_id")
            .expect("world_id column check should succeed"));
        assert!(column_exists(&connection, "visit_histories", "instance_id")
            .expect("instance_id column check should succeed"));
        assert!(
            column_exists(&connection, "visit_histories", "stay_duration_seconds")
                .expect("stay_duration_seconds column check should succeed")
        );
        assert!(
            column_exists(&connection, "visit_histories", "source_log_file")
                .expect("source_log_file column check should succeed")
        );
        assert!(column_exists(&connection, "visit_histories", "created_at")
            .expect("created_at column check should succeed"));
        assert!(column_exists(&connection, "visit_histories", "updated_at")
            .expect("updated_at column check should succeed"));
    }

    #[test]
    fn lists_visit_histories_columns() {
        let connection =
            open_database(Path::new(":memory:")).expect("in-memory database should open");

        initialize_database(&connection).expect("database schema should be initialized");

        let columns =
            list_columns(&connection, "visit_histories").expect("columns should be listed");

        assert!(columns.contains(&"id".to_string()));
        assert!(columns.contains(&"visited_at".to_string()));
        assert!(columns.contains(&"world_name".to_string()));
        assert!(columns.contains(&"world_id".to_string()));
        assert!(columns.contains(&"stay_duration_seconds".to_string()));
    }

    #[test]
    #[ignore]
    fn lists_existing_worldrec_database_columns_read_only() {
        use crate::db::connection::open_database_read_only;

        let db_path_buf = existing_worldrec_db_path();
        let db_path = db_path_buf.as_path();

        let connection = open_database_read_only(db_path)
            .expect("existing WorldRec database should open read-only");

        let columns = list_columns(&connection, "visit_histories")
            .expect("existing visit_histories columns should be listed");

        println!("existing visit_histories columns:");
        for column in columns {
            println!("- {}", column);
        }
    }
}
