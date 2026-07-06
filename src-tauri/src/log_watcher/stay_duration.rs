use chrono::DateTime;

pub fn parse_timestamp_seconds(timestamp: &str) -> Option<i64> {
    DateTime::parse_from_rfc3339(timestamp)
        .ok()
        .map(|datetime| datetime.timestamp())
}

pub fn calculate_stay_duration_seconds(start_seconds: i64, end_seconds: i64) -> i64 {
    let duration = end_seconds - start_seconds;

    if duration < 0 {
        0
    } else {
        duration
    }
}

pub fn calculate_stay_duration_from_timestamps(
    start_timestamp: &str,
    end_timestamp: &str,
) -> Option<i64> {
    let start_seconds = parse_timestamp_seconds(start_timestamp)?;
    let end_seconds = parse_timestamp_seconds(end_timestamp)?;

    Some(calculate_stay_duration_seconds(start_seconds, end_seconds))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calculates_stay_duration_seconds() {
        let start_seconds = 1000;
        let end_seconds = 4600;

        let result = calculate_stay_duration_seconds(start_seconds, end_seconds);

        assert_eq!(result, 3600);
    }

    #[test]
    fn returns_zero_when_end_is_before_start() {
        let start_seconds = 4600;
        let end_seconds = 1000;

        let result = calculate_stay_duration_seconds(start_seconds, end_seconds);

        assert_eq!(result, 0);
    }

    #[test]
    fn parses_rfc3339_timestamp_to_seconds() {
        let timestamp = "2026-06-09T20:00:00+09:00";

        let result = parse_timestamp_seconds(timestamp);

        assert_eq!(result, Some(1781002800));
    }

    #[test]
    fn calculates_stay_duration_from_timestamps() {
        let start_timestamp = "2026-06-09T20:00:00+09:00";
        let end_timestamp = "2026-06-09T21:00:00+09:00";

        let result = calculate_stay_duration_from_timestamps(start_timestamp, end_timestamp);

        assert_eq!(result, Some(3600));
    }

    #[test]
    fn returns_none_when_timestamp_is_invalid() {
        let start_timestamp = "invalid timestamp";
        let end_timestamp = "2026-06-09T21:00:00+09:00";

        let result = calculate_stay_duration_from_timestamps(start_timestamp, end_timestamp);

        assert_eq!(result, None);
    }
}
