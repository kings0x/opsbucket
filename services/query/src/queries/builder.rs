use crate::{DateRange, FunnelSpec, RetentionSpec, SegmentSpec};
use chrono::{DateTime, Duration, Utc};

const MAX_DATE_RANGE_DAYS: i64 = 366;
const MAX_FUNNEL_STEPS: usize = 20;
const MAX_SEGMENT_CONDITIONS: usize = 10;
const MAX_STRING_LENGTH: usize = 200;
const MAX_SEGMENT_LIMIT: u64 = 10_000;
const MAX_RETENTION_PERIODS: u64 = 366;
const MAX_SEGMENT_WITHIN_DAYS: u64 = 366;

pub fn validate_date_range(range: &DateRange) -> Result<(), String> {
    validate_date_range_with_max(range, MAX_DATE_RANGE_DAYS as u64)
}

pub fn validate_date_range_with_max(range: &DateRange, max_days: u64) -> Result<(), String> {
    let start: DateTime<Utc> = range
        .start
        .parse()
        .map_err(|_| "Invalid start date".to_string())?;
    let end: DateTime<Utc> = range
        .end
        .parse()
        .map_err(|_| "Invalid end date".to_string())?;

    if start >= end {
        return Err("start must be before end".into());
    }

    let max_end = Utc::now() + Duration::minutes(1);
    if end > max_end {
        return Err("end date cannot be in the future".into());
    }

    let span_seconds = (end - start).num_seconds();
    let max_seconds = (max_days as i64) * 24 * 60 * 60;
    if span_seconds > max_seconds {
        return Err(format!("date range cannot exceed {} days", max_days));
    }

    Ok(())
}

pub fn validate_string(s: &str, name: &str) -> Result<(), String> {
    if s.is_empty() {
        return Err(format!("{} must not be empty", name));
    }
    if s.len() > MAX_STRING_LENGTH {
        return Err(format!(
            "{} must not exceed {} characters",
            name, MAX_STRING_LENGTH
        ));
    }
    Ok(())
}

pub fn validate_funnel(spec: &FunnelSpec) -> Result<(), String> {
    validate_funnel_with_max_date_range(spec, MAX_DATE_RANGE_DAYS as u64)
}

pub fn validate_funnel_with_max_date_range(
    spec: &FunnelSpec,
    max_date_range_days: u64,
) -> Result<(), String> {
    validate_string(&spec.project_id, "projectId")?;
    if spec.steps.is_empty() {
        return Err("funnel must have at least one step".into());
    }
    if spec.steps.len() > MAX_FUNNEL_STEPS {
        return Err(format!("funnel cannot exceed {} steps", MAX_FUNNEL_STEPS));
    }
    for step in &spec.steps {
        validate_string(step, "step name")?;
    }
    validate_date_range_with_max(&spec.date_range, max_date_range_days)?;
    Ok(())
}

pub fn validate_retention(spec: &RetentionSpec) -> Result<(), String> {
    validate_retention_with_max_date_range(spec, MAX_DATE_RANGE_DAYS as u64)
}

pub fn validate_retention_with_max_date_range(
    spec: &RetentionSpec,
    max_date_range_days: u64,
) -> Result<(), String> {
    validate_string(&spec.project_id, "projectId")?;
    validate_string(&spec.event_name, "eventName")?;
    if spec.interval != "week" && spec.interval != "day" {
        return Err("interval must be 'week' or 'day'".into());
    }
    if spec.periods == 0 {
        return Err("periods must be greater than 0".into());
    }
    if spec.periods > MAX_RETENTION_PERIODS {
        return Err(format!("periods cannot exceed {}", MAX_RETENTION_PERIODS));
    }
    validate_date_range_with_max(&spec.date_range, max_date_range_days)?;
    Ok(())
}

pub fn validate_segment(spec: &SegmentSpec) -> Result<(), String> {
    validate_string(&spec.project_id, "projectId")?;
    if spec.conditions.len() > MAX_SEGMENT_CONDITIONS {
        return Err(format!(
            "segment cannot exceed {} conditions",
            MAX_SEGMENT_CONDITIONS
        ));
    }
    if spec.limit == 0 || spec.limit > MAX_SEGMENT_LIMIT {
        return Err(format!("limit must be between 1 and {}", MAX_SEGMENT_LIMIT));
    }
    for condition in &spec.conditions {
        match condition.condition_type.as_str() {
            "event_count" => {
                let event_name = condition
                    .event_name
                    .as_deref()
                    .ok_or("event_count condition requires eventName")?;
                validate_string(event_name, "eventName")?;
                let within = condition.within_days.unwrap_or(30);
                if within == 0 || within > MAX_SEGMENT_WITHIN_DAYS {
                    return Err(format!(
                        "withinDays must be between 1 and {}",
                        MAX_SEGMENT_WITHIN_DAYS
                    ));
                }
            }
            "trait" => {
                let key = condition
                    .key
                    .as_deref()
                    .ok_or("trait condition requires key")?;
                validate_string(key, "key")?;
            }
            _ => {
                return Err(format!(
                    "unsupported condition type: {}",
                    condition.condition_type
                ))
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_range() -> DateRange {
        let now = Utc::now();
        DateRange {
            start: (now - Duration::days(10)).to_rfc3339(),
            end: (now - Duration::days(1)).to_rfc3339(),
        }
    }

    #[test]
    fn date_range_exactly_366_days_accepted() {
        let now = Utc::now();
        let range = DateRange {
            start: (now - Duration::days(366)).to_rfc3339(),
            end: (now - Duration::hours(1)).to_rfc3339(),
        };
        assert!(validate_date_range(&range).is_ok());
    }

    #[test]
    fn date_range_367_days_rejected() {
        let now = Utc::now();
        let range = DateRange {
            start: (now - Duration::days(368)).to_rfc3339(),
            end: (now - Duration::hours(1)).to_rfc3339(),
        };
        assert!(validate_date_range(&range).is_err());
    }

    #[test]
    fn end_before_start_rejected() {
        let now = Utc::now();
        let range = DateRange {
            start: (now - Duration::days(1)).to_rfc3339(),
            end: (now - Duration::days(10)).to_rfc3339(),
        };
        assert!(validate_date_range(&range).is_err());
    }

    #[test]
    fn future_end_date_rejected() {
        let now = Utc::now();
        let range = DateRange {
            start: (now - Duration::days(10)).to_rfc3339(),
            end: (now + Duration::hours(2)).to_rfc3339(),
        };
        assert!(validate_date_range(&range).is_err());
    }

    #[test]
    fn more_than_20_funnel_steps_rejected() {
        let spec = FunnelSpec {
            project_id: "proj_1".into(),
            steps: (0..21).map(|i| format!("step_{}", i)).collect(),
            window_seconds: 86400,
            date_range: valid_range(),
        };
        assert!(validate_funnel(&spec).is_err());
    }

    #[test]
    fn more_than_10_segment_conditions_rejected() {
        let spec = SegmentSpec {
            project_id: "proj_1".into(),
            conditions: (0..11)
                .map(|_| crate::SegmentCondition {
                    condition_type: "event_count".into(),
                    event_name: Some("test".into()),
                    op: "gt".into(),
                    value: serde_json::json!(1),
                    within_days: Some(30),
                    key: None,
                })
                .collect(),
            limit: 100,
        };
        assert!(validate_segment(&spec).is_err());
    }

    #[test]
    fn empty_project_id_rejected() {
        let spec = FunnelSpec {
            project_id: "".into(),
            steps: vec!["click".into()],
            window_seconds: 86400,
            date_range: valid_range(),
        };
        assert!(validate_funnel(&spec).is_err());
    }
}
