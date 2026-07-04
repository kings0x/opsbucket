use crate::FunnelSpec;

pub fn build(spec: &FunnelSpec) -> String {
    let step_conditions: Vec<String> = spec
        .steps
        .iter()
        .map(|s| format!("event_name = '{}'", sql_escape(s)))
        .collect();

    let step_names: Vec<String> = spec
        .steps
        .iter()
        .map(|s| format!("'{}'", sql_escape(s)))
        .collect();

    format!(
        "SELECT level, count() AS users_reached
FROM (
    SELECT
        COALESCE(user_id, anonymous_id) AS user_key,
        windowFunnel({})(timestamp, {})
            AS level
    FROM events
    WHERE project_id = '{}'
      AND timestamp BETWEEN '{}' AND '{}'
      AND event_name IN ({})
    GROUP BY user_key
)
GROUP BY level
ORDER BY level ASC",
        spec.window_seconds,
        step_conditions.join(", "),
        sql_escape(&spec.project_id),
        to_ch_datetime(&spec.date_range.start),
        to_ch_datetime(&spec.date_range.end),
        step_names.join(", "),
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

    fn make_spec(steps: &[&str], window_seconds: u64) -> FunnelSpec {
        FunnelSpec {
            project_id: "proj_test".to_string(),
            steps: steps.iter().map(|s| s.to_string()).collect(),
            window_seconds,
            date_range: DateRange {
                start: "2026-01-01T00:00:00Z".to_string(),
                end: "2026-06-30T23:59:59Z".to_string(),
            },
        }
    }

    #[test]
    fn build_single_step() {
        let sql = build(&make_spec(&["Page Viewed"], 86400));
        assert!(sql.contains("windowFunnel(86400)"));
        assert!(sql.contains("event_name = 'Page Viewed'"));
        assert!(sql.contains("event_name IN ('Page Viewed')"));
    }

    #[test]
    fn build_multi_step() {
        let sql = build(&make_spec(
            &["Page Viewed", "Button Clicked", "Account Created"],
            3600,
        ));
        assert!(sql.contains("windowFunnel(3600)"));
        assert!(sql.contains("event_name = 'Page Viewed'"));
        assert!(sql.contains("event_name = 'Button Clicked'"));
        assert!(sql.contains("event_name = 'Account Created'"));
        assert!(sql.contains("event_name IN ('Page Viewed', 'Button Clicked', 'Account Created')"));
    }

    #[test]
    fn build_contains_project_id_filter() {
        let sql = build(&make_spec(&["Page Viewed"], 86400));
        assert!(sql.contains("project_id = 'proj_test'"));
    }

    #[test]
    fn build_contains_date_range() {
        let sql = build(&make_spec(&["Page Viewed"], 86400));
        assert!(sql.contains("BETWEEN '2026-01-01 00:00:00' AND '2026-06-30 23:59:59'"));
    }

    #[test]
    fn build_uses_coalesce_for_user_key() {
        let sql = build(&make_spec(&["Page Viewed"], 86400));
        assert!(sql.contains("COALESCE(user_id, anonymous_id) AS user_key"));
    }

    #[test]
    fn build_groups_by_user_key_and_level() {
        let sql = build(&make_spec(&["Page Viewed"], 86400));
        assert!(sql.contains("GROUP BY user_key"));
        assert!(sql.contains("GROUP BY level"));
        assert!(sql.contains("ORDER BY level ASC"));
    }

    #[test]
    fn build_selects_level_and_count() {
        let sql = build(&make_spec(&["Page Viewed"], 86400));
        assert!(sql.contains("SELECT level, count() AS users_reached"));
    }

    #[test]
    fn build_escapes_single_quotes_in_step_names() {
        let sql = build(&make_spec(&["User's Action"], 86400));
        assert!(sql.contains("event_name = 'User\\'s Action'"));
        assert!(sql.contains("event_name IN ('User\\'s Action')"));
        assert!(!sql.contains("User's Action"));
    }

    #[test]
    fn to_ch_datetime_converts_iso_to_ch_format() {
        assert_eq!(
            to_ch_datetime("2026-01-01T00:00:00Z"),
            "2026-01-01 00:00:00"
        );
    }

    #[test]
    fn sql_escape_escapes_single_quotes() {
        assert_eq!(sql_escape("normal"), "normal");
        assert_eq!(sql_escape("it's"), "it\\'s");
    }
}
