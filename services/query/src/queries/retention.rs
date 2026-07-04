use crate::RetentionSpec;

pub fn build(spec: &RetentionSpec) -> String {
    let (first_seen_bucket, ts_bucket, diff_unit) = if spec.interval == "day" {
        ("toDate(first_seen)", "toDate(ts)", "day")
    } else {
        (
            "toStartOfWeek(first_seen, 1)",
            "toStartOfWeek(ts, 1)",
            "week",
        )
    };

    format!(
        "SELECT
    toInt32({}) AS cohort_date,
    toInt32(dateDiff('{}', {}, {})) AS period,
    count(DISTINCT user_key) AS users
FROM (
    SELECT
        COALESCE(user_id, anonymous_id) AS user_key,
        min(timestamp) OVER (PARTITION BY COALESCE(user_id, anonymous_id)) AS first_seen,
        timestamp AS ts
    FROM events
    WHERE project_id = '{}'
      AND event_name = '{}'
      AND timestamp BETWEEN '{}' AND '{}'
)
WHERE user_key IS NOT NULL
GROUP BY cohort_date, period
ORDER BY cohort_date ASC, period ASC",
        first_seen_bucket,
        diff_unit,
        first_seen_bucket,
        ts_bucket,
        sql_escape(&spec.project_id),
        sql_escape(&spec.event_name),
        to_ch_datetime(&spec.date_range.start),
        to_ch_datetime(&spec.date_range.end),
    )
}

fn to_ch_datetime(s: &str) -> String {
    s.replace('T', " ").replace('Z', "")
}

fn sql_escape(s: &str) -> String {
    s.replace('\'', "\\'")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DateRange;

    fn make_spec(interval: &str, project_id: &str, event_name: &str) -> RetentionSpec {
        RetentionSpec {
            project_id: project_id.to_string(),
            event_name: event_name.to_string(),
            interval: interval.to_string(),
            periods: 8,
            date_range: DateRange {
                start: "2026-01-01T00:00:00Z".to_string(),
                end: "2026-06-30T23:59:59Z".to_string(),
            },
        }
    }

    #[test]
    fn build_weekly_default_interval() {
        let sql = build(&make_spec("week", "proj_test", "App Opened"));
        assert!(sql.contains("toInt32(toStartOfWeek(first_seen, 1)) AS cohort_date"));
        assert!(sql.contains(
            "toInt32(dateDiff('week', toStartOfWeek(first_seen, 1), toStartOfWeek(ts, 1))) AS period"
        ));
    }

    #[test]
    fn build_daily_interval() {
        let sql = build(&make_spec("day", "proj_test", "App Opened"));
        assert!(sql.contains("toInt32(toDate(first_seen)) AS cohort_date"));
        assert!(sql.contains("toInt32(dateDiff('day', toDate(first_seen), toDate(ts))) AS period"));
    }

    #[test]
    fn build_fallback_to_week_for_unknown_interval() {
        let sql = build(&make_spec("month", "proj_test", "App Opened"));
        assert!(sql.contains("toStartOfWeek"));
        assert!(sql.contains("dateDiff('week'"));
    }

    #[test]
    fn build_contains_project_id_filter() {
        let sql = build(&make_spec("week", "proj_abc", "App Opened"));
        assert!(sql.contains("project_id = 'proj_abc'"));
    }

    #[test]
    fn build_contains_event_name_filter() {
        let sql = build(&make_spec("week", "proj_test", "Button Clicked"));
        assert!(sql.contains("event_name = 'Button Clicked'"));
    }

    #[test]
    fn build_contains_date_range_filter() {
        let sql = build(&make_spec("week", "proj_test", "App Opened"));
        assert!(sql.contains("BETWEEN '2026-01-01 00:00:00' AND '2026-06-30 23:59:59'"));
    }

    #[test]
    fn build_escapes_single_quotes_in_project_id() {
        let sql = build(&make_spec("week", "proj_'test", "App Opened"));
        assert!(sql.contains("proj_\\'test"));
        assert!(!sql.contains("proj_'test"));
    }

    #[test]
    fn build_escapes_single_quotes_in_event_name() {
        let sql = build(&make_spec("week", "proj_test", "It's a test"));
        assert!(sql.contains("It\\'s a test"));
        assert!(!sql.contains("It's a test"));
    }

    #[test]
    fn build_group_by_and_order_by() {
        let sql = build(&make_spec("week", "proj_test", "App Opened"));
        assert!(sql.contains("GROUP BY cohort_date, period"));
        assert!(sql.contains("ORDER BY cohort_date ASC, period ASC"));
    }

    #[test]
    fn build_user_key_is_not_null() {
        let sql = build(&make_spec("week", "proj_test", "App Opened"));
        assert!(sql.contains("WHERE user_key IS NOT NULL"));
    }

    #[test]
    fn build_uses_coalesce_for_user_key() {
        let sql = build(&make_spec("week", "proj_test", "App Opened"));
        assert!(sql.contains("COALESCE(user_id, anonymous_id) AS user_key"));
    }

    #[test]
    fn build_uses_min_timestamp_window() {
        let sql = build(&make_spec("week", "proj_test", "App Opened"));
        assert!(sql.contains(
            "min(timestamp) OVER (PARTITION BY COALESCE(user_id, anonymous_id)) AS first_seen"
        ));
    }

    #[test]
    fn to_ch_datetime_replaces_t_and_z() {
        assert_eq!(
            to_ch_datetime("2026-01-01T00:00:00Z"),
            "2026-01-01 00:00:00"
        );
        assert_eq!(
            to_ch_datetime("2026-06-30T23:59:59Z"),
            "2026-06-30 23:59:59"
        );
    }

    #[test]
    fn sql_escape_handles_single_quotes() {
        assert_eq!(sql_escape("test"), "test");
        assert_eq!(sql_escape("it's"), "it\\'s");
        assert_eq!(sql_escape("'quoted'"), "\\'quoted\\'");
    }

    #[test]
    fn build_count_distinct_user_key() {
        let sql = build(&make_spec("week", "proj_test", "App Opened"));
        assert!(sql.contains("count(DISTINCT user_key) AS users"));
    }
}
