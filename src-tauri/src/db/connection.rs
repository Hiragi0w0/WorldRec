use rusqlite::Connection;
#[cfg(test)]
use rusqlite::OpenFlags;
use std::path::Path;

pub fn open_database(path: &Path) -> rusqlite::Result<Connection> {
    Connection::open(path)
}

#[cfg(test)]
pub fn open_database_read_only(path: &Path) -> rusqlite::Result<Connection> {
    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_in_memory_database() {
        let connection = open_database(Path::new(":memory:"));

        assert!(connection.is_ok());
    }

    #[test]
    fn opens_file_database_read_only() {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();

        let db_path = std::env::temp_dir().join(format!(
            "worldrec_read_only_test_{}_{}.db",
            std::process::id(),
            timestamp
        ));

        {
            let connection = open_database(&db_path).expect("temporary database should open");

            connection
                .execute_batch(
                    "
                    CREATE TABLE test_table (
                        id INTEGER PRIMARY KEY AUTOINCREMENT,
                        name TEXT NOT NULL
                    );
                    ",
                )
                .expect("test table should be created");
        }

        let read_only_connection = open_database_read_only(&db_path);

        assert!(read_only_connection.is_ok());

        drop(read_only_connection);

        let _ = std::fs::remove_file(&db_path);
    }
}
