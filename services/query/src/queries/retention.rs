use crate::ch::client::{QueryParam, QueryPlan};
use crate::RetentionSpec;

pub fn build_plan(spec: &RetentionSpec) -> QueryPlan {
    let (first_seen_bucket, ts_bucket, diff_unit) = if spec.interval == "day" {
        ("toDate(first_seen)", "toDate(ts)", "day")
    } else {
        (
            "toStartOfWeek(first_seen, 1)",
            "toStartOfWeek(ts, 1)",
            "week",
        )
    };

    let sql = format!(
        "SELECT
    toInt32({}) AS cohort_date,
    toInt32(dateDiff(?, {}, {})) AS period,
    count(DISTINCT user_key) AS users
FROM (
    SELECT
        COALESCE(user_id, anonymous_id) AS user_key,
        min(timestamp) OVER (PARTITION BY COALESCE(user_id, anonymous_id)) AS first_seen,
        timestamp AS ts
    FROM events
    WHERE project_id = ?
      AND event_name = ?
      AND timestamp BETWEEN ? AND ?
)
WHERE user_key IS NOT NULL
GROUP BY cohort_date, period
ORDER BY cohort_date ASC, period ASC",
        first_seen_bucket, first_seen_bucket, ts_bucket,
    );

    QueryPlan::new(
        sql,
        vec![
            QueryParam::String(diff_unit.to_string()),
            QueryParam::String(spec.project_id.clone()),
            QueryParam::String(spec.event_name.clone()),
            QueryParam::String(to_ch_datetime(&spec.date_range.start)),
            QueryParam::String(to_ch_datetime(&spec.date_range.end)),
        ],
    )
}

fn to_ch_datetime(s: &str) -> String {
    s.replace('T', " ").replace('Z', "")
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
