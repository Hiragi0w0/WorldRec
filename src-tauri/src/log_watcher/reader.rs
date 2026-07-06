use std::{
    fs,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    time::SystemTime,
};

use chrono::NaiveDateTime;

const VRCHAT_LOG_FILE_TIMESTAMP_FORMAT: &str = "%Y-%m-%d_%H-%M-%S";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LogFileSnapshot {
    pub(crate) path: PathBuf,
    pub(crate) size: u64,
}

#[derive(Debug, Clone)]
#[cfg_attr(test, allow(dead_code))]
pub(crate) struct WatchedLogFile {
    pub(crate) path: PathBuf,
    pub(crate) offset: u64,
}

pub(crate) struct ReadLogLinesResult {
    pub(crate) lines: Vec<String>,
    pub(crate) next_offset: u64,
}

pub(crate) fn latest_log_snapshot(log_dir: &Path) -> std::io::Result<Option<LogFileSnapshot>> {
    let mut latest_by_filename_timestamp: Option<(NaiveDateTime, LogFileSnapshot)> = None;
    let mut latest_by_creation_time: Option<(SystemTime, LogFileSnapshot)> = None;

    for entry in fs::read_dir(log_dir)? {
        let entry = entry?;
        let path = entry.path();

        if !is_vrchat_output_log(&path) {
            continue;
        }

        let metadata = entry.metadata()?;
        let snapshot = LogFileSnapshot {
            path,
            size: metadata.len(),
        };

        if let Some(timestamp) = parse_vrchat_output_log_timestamp(&snapshot.path) {
            match &latest_by_filename_timestamp {
                Some((latest_timestamp, _)) if timestamp <= *latest_timestamp => {}
                _ => latest_by_filename_timestamp = Some((timestamp, snapshot)),
            }

            continue;
        }

        let created = metadata.created()?;
        match &latest_by_creation_time {
            Some((latest_created, _)) if created <= *latest_created => {}
            _ => latest_by_creation_time = Some((created, snapshot)),
        }
    }

    if let Some((_, snapshot)) = latest_by_filename_timestamp {
        return Ok(Some(snapshot));
    }

    Ok(latest_by_creation_time.map(|(_, snapshot)| snapshot))
}

pub(crate) fn read_lines_from_offset(
    path: &Path,
    offset: u64,
) -> std::io::Result<ReadLogLinesResult> {
    let mut file = fs::File::open(path)?;
    file.seek(SeekFrom::Start(offset))?;

    let mut text = String::new();
    file.read_to_string(&mut text)?;
    let next_offset = file.stream_position()?;
    let lines = text.lines().map(|line| line.to_string()).collect();

    Ok(ReadLogLinesResult { lines, next_offset })
}

fn is_vrchat_output_log(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.starts_with("output_log_") && name.ends_with(".txt"))
        .unwrap_or(false)
}

fn parse_vrchat_output_log_timestamp(path: &Path) -> Option<NaiveDateTime> {
    let file_name = path.file_name()?.to_str()?;
    let timestamp = file_name
        .strip_prefix("output_log_")?
        .strip_suffix(".txt")?;

    NaiveDateTime::parse_from_str(timestamp, VRCHAT_LOG_FILE_TIMESTAMP_FORMAT).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{env, fs::File, io::Write, thread, time::Duration};

    fn unique_temp_dir() -> PathBuf {
        env::temp_dir().join(format!(
            "worldrec-log-watcher-reader-test-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ))
    }

    fn create_file(path: &Path) {
        File::create(path).expect("test file should be created");
    }

    #[test]
    fn finds_latest_output_log() {
        let dir = unique_temp_dir();
        fs::create_dir_all(&dir).expect("temp log dir should be created");
        create_file(&dir.join("not_a_log.txt"));
        create_file(&dir.join("output_log_2026-06-10_10-00-00.txt"));

        let snapshot = latest_log_snapshot(&dir)
            .expect("scan should succeed")
            .expect("output log should be found");

        assert_eq!(
            snapshot.path,
            dir.join("output_log_2026-06-10_10-00-00.txt")
        );

        fs::remove_dir_all(dir).expect("temp log dir should be removed");
    }

    #[test]
    fn selects_latest_log_by_filename_timestamp() {
        let dir = unique_temp_dir();
        fs::create_dir_all(&dir).expect("temp log dir should be created");
        create_file(&dir.join("output_log_2026-06-10_10-00-00.txt"));
        create_file(&dir.join("output_log_2026-06-11_10-00-00.txt"));

        let snapshot = latest_log_snapshot(&dir)
            .expect("scan should succeed")
            .expect("output log should be found");

        assert_eq!(
            snapshot.path,
            dir.join("output_log_2026-06-11_10-00-00.txt")
        );

        fs::remove_dir_all(dir).expect("temp log dir should be removed");
    }

    #[test]
    fn filename_timestamp_wins_even_when_file_is_not_most_recently_modified() {
        let dir = unique_temp_dir();
        fs::create_dir_all(&dir).expect("temp log dir should be created");
        create_file(&dir.join("output_log_2026-06-11_10-00-00.txt"));
        thread::sleep(Duration::from_millis(50));
        create_file(&dir.join("output_log_2026-06-10_10-00-00.txt"));

        let snapshot = latest_log_snapshot(&dir)
            .expect("scan should succeed")
            .expect("output log should be found");

        assert_eq!(
            snapshot.path,
            dir.join("output_log_2026-06-11_10-00-00.txt")
        );

        fs::remove_dir_all(dir).expect("temp log dir should be removed");
    }

    #[test]
    fn falls_back_to_creation_time_when_filename_timestamp_cannot_be_parsed() {
        let dir = unique_temp_dir();
        fs::create_dir_all(&dir).expect("temp log dir should be created");
        create_file(&dir.join("output_log_not-a-timestamp.txt"));
        thread::sleep(Duration::from_millis(50));
        create_file(&dir.join("output_log_also-not-a-timestamp.txt"));

        let snapshot = latest_log_snapshot(&dir)
            .expect("scan should succeed")
            .expect("output log should be found");

        assert_eq!(
            snapshot.path,
            dir.join("output_log_also-not-a-timestamp.txt")
        );

        fs::remove_dir_all(dir).expect("temp log dir should be removed");
    }

    #[test]
    fn filename_timestamp_candidate_wins_over_newer_unparseable_log() {
        let dir = unique_temp_dir();
        fs::create_dir_all(&dir).expect("temp log dir should be created");
        create_file(&dir.join("output_log_2026-06-10_10-00-00.txt"));
        thread::sleep(Duration::from_millis(50));
        create_file(&dir.join("output_log_not-a-timestamp.txt"));

        let snapshot = latest_log_snapshot(&dir)
            .expect("scan should succeed")
            .expect("output log should be found");

        assert_eq!(
            snapshot.path,
            dir.join("output_log_2026-06-10_10-00-00.txt")
        );

        fs::remove_dir_all(dir).expect("temp log dir should be removed");
    }

    #[test]
    fn ignores_files_that_do_not_match_vrchat_output_log_pattern() {
        let dir = unique_temp_dir();
        fs::create_dir_all(&dir).expect("temp log dir should be created");
        create_file(&dir.join("output_log_2026-06-10_10-00-00.log"));
        create_file(&dir.join("output-log_2026-06-11_10-00-00.txt"));
        create_file(&dir.join("output_log_2026-06-09_10-00-00.txt"));

        let snapshot = latest_log_snapshot(&dir)
            .expect("scan should succeed")
            .expect("output log should be found");

        assert_eq!(
            snapshot.path,
            dir.join("output_log_2026-06-09_10-00-00.txt")
        );

        fs::remove_dir_all(dir).expect("temp log dir should be removed");
    }

    #[test]
    fn read_lines_from_offset_returns_lines_and_next_offset() {
        let dir = unique_temp_dir();
        fs::create_dir_all(&dir).expect("temp dir should be created");
        let log_path = dir.join("output_log_2026-06-10.txt");
        fs::write(&log_path, "first\nsecond\nthird\n").expect("log file should be written");

        let offset = "first\n".len() as u64;
        let result = read_lines_from_offset(&log_path, offset).expect("log lines should be read");

        assert_eq!(
            result.lines,
            vec!["second".to_string(), "third".to_string()]
        );
        assert_eq!(result.next_offset, fs::metadata(&log_path).unwrap().len());

        fs::remove_dir_all(dir).expect("temp dir should be removed");
    }

    #[test]
    fn read_lines_from_offset_advances_to_actual_file_end() {
        let dir = unique_temp_dir();
        fs::create_dir_all(&dir).expect("temp dir should be created");
        let log_path = dir.join("output_log_2026-06-10.txt");
        fs::write(&log_path, "initial\n").expect("log file should be written");
        let stale_snapshot_size = fs::metadata(&log_path).unwrap().len();

        let mut file = fs::OpenOptions::new()
            .append(true)
            .open(&log_path)
            .expect("log file should open for append");
        writeln!(file, "appended").expect("log line should be appended");
        drop(file);

        let result = read_lines_from_offset(&log_path, 0).expect("log lines should be read");

        assert!(result.next_offset > stale_snapshot_size);
        assert_eq!(result.next_offset, fs::metadata(&log_path).unwrap().len());

        fs::remove_dir_all(dir).expect("temp dir should be removed");
    }

    #[test]
    fn next_offset_prevents_reprocessing_appended_lines() {
        let dir = unique_temp_dir();
        fs::create_dir_all(&dir).expect("temp dir should be created");
        let log_path = dir.join("output_log_2026-06-10.txt");
        fs::write(&log_path, "initial\n").expect("log file should be written");
        let stale_snapshot_size = fs::metadata(&log_path).unwrap().len();

        let mut file = fs::OpenOptions::new()
            .append(true)
            .open(&log_path)
            .expect("log file should open for append");
        writeln!(file, "appended").expect("log line should be appended");
        drop(file);

        let first_read = read_lines_from_offset(&log_path, 0).expect("first read should succeed");
        let second_read = read_lines_from_offset(&log_path, first_read.next_offset)
            .expect("second read should succeed");

        assert!(first_read.next_offset > stale_snapshot_size);
        assert_eq!(
            first_read.lines,
            vec!["initial".to_string(), "appended".to_string()]
        );
        assert!(second_read.lines.is_empty());
        assert_eq!(second_read.next_offset, first_read.next_offset);

        fs::remove_dir_all(dir).expect("temp dir should be removed");
    }
}
