use crate::ch::client::{QueryParam, QueryPlan};
use crate::FunnelSpec;

pub fn build_plan(spec: &FunnelSpec) -> QueryPlan {
    let step_conditions = vec!["event_name = ?"; spec.steps.len()].join(", ");
    let step_names = vec!["?"; spec.steps.len()].join(", ");

    let sql = format!(
        "SELECT level, count() AS users_reached
FROM (
    SELECT
        COALESCE(user_id, anonymous_id) AS user_key,
        windowFunnel(?)(timestamp, {})
            AS level
    FROM events
    WHERE project_id = ?
      AND timestamp BETWEEN ? AND ?
      AND event_name IN ({})
    GROUP BY user_key
)
GROUP BY level
ORDER BY level ASC",
        step_conditions, step_names,
    );

    let mut params = Vec::with_capacity(1 + spec.steps.len() + 3 + spec.steps.len());
    params.push(QueryParam::U64(spec.window_seconds));
    params.extend(spec.steps.iter().cloned().map(QueryParam::String));
    params.push(QueryParam::String(spec.project_id.clone()));
    params.push(QueryParam::String(to_ch_datetime(&spec.date_range.start)));
    params.push(QueryParam::String(to_ch_datetime(&spec.date_range.end)));
    params.extend(spec.steps.iter().cloned().map(QueryParam::String));

    QueryPlan::new(sql, params)
}

fn to_ch_datetime(s: &str) -> String {
    s.replace('T', " ").replace('Z', "")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_ch_datetime_converts_iso_to_ch_format() {
        assert_eq!(
            to_ch_datetime("2026-01-01T00:00:00Z"),
            "2026-01-01 00:00:00"
        );
    }
}
